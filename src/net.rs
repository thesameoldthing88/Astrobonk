//! Co-op networking layer (host-authoritative).
//!
//! Design (per the GDD): one machine is the HOST and owns the simulation — the swarm,
//! the clock, bosses, loot. Clients send only *input intent* and locally predict their
//! own astronaut; everything else is replicated down. Auto-attacks mean there's no
//! "did my shot hit?" latency pain — the host decides and it looks the same to everyone.
//!
//! What replicates (deliberately small — with ~1200 enemies, bandwidth is the budget):
//!   * `PlayerId`        — who an astronaut belongs to
//!   * `NetTransform`    — position/facing of each astronaut
//!   * `PlayerVitals`    — hp / max_hp / level, for teammate HUD + the down-state
//! A player's *build* (weapons, items, cards) stays local — each player picks their own
//! upgrades, so only its visible effects need to cross the wire.
//!
//! Enemies are NOT replicated per-entity yet; see `NETCODE NOTES` at the bottom.

use crate::player::{InputIntent, LocalPlayer, PlayerId};
use std::collections::HashMap;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon_renet::{
    netcode::{
        ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
        ServerConfig,
    },
    renet::ConnectionConfig,
    RenetChannelsExt, RenetClient, RenetServer, RepliconRenetPlugins,
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use bevy::time::common_conditions::on_timer;
use std::time::{Duration, SystemTime};

/// Bumped whenever the wire format changes — mismatched builds refuse to connect
/// instead of desyncing in confusing ways.
pub const PROTOCOL_ID: u64 = 0xA570B0_2; // bumped: enemy streaming changed the wire
pub const DEFAULT_PORT: u16 = 5011;
pub const MAX_PLAYERS: usize = 4;

/// How this instance is participating. Solo is the default and behaves exactly like
/// the single-player game (no sockets opened).
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum NetRole {
    #[default]
    Solo,
    Host,
    Client,
}

impl NetRole {
    pub fn is_networked(&self) -> bool {
        !matches!(self, NetRole::Solo)
    }
    /// The host simulates; solo also "simulates" (it's its own host).
    pub fn simulates(&self) -> bool {
        !matches!(self, NetRole::Client)
    }
}

/// Replicated astronaut pose. We send a compact surface-direction + facing rather than a
/// full Transform: on a sphere the position IS a unit direction plus a height, so this is
/// both smaller and reconstructable exactly.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct NetTransform {
    pub dir: Vec3,
    pub height: f32,
    pub facing: Vec3,
}

/// Replicated teammate vitals — what another player's HUD marker needs to show.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PlayerVitals {
    pub hp: f32,
    pub max_hp: f32,
    pub level: u32,
    pub down: bool,
}

/// Host -> client: "you are player N". The client cannot infer this: replicon 0.40 exposes
/// no local-client-id API, and PlayerId alone can't identify "me" — the client's own
/// predicted astronaut and the HOST's astronaut are both PlayerId(0). Without this the
/// client can't tell which replicated astronaut is the server's copy of itself, and would
/// draw a ghost twin of itself standing wherever the host thinks it is.
///
/// Sent on an ORDERED channel (a dropped identity packet would strand the client
/// permanently) and re-sent on a timer, because a non-independent server message aimed at
/// a not-yet-`AuthorizedClient` is dropped with only an error log.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct AssignPlayerId(pub u8);

/// The authoritative run: seed, world chain, clock and shared counters.
///
/// Without this a joiner generates its world from its OWN `fresh_seed()`, so every rock,
/// pot, chest, shop and shrine lands somewhere different — and the streamed horde walks
/// through scenery that isn't there. The seed is what makes both machines build the same
/// planet; everything else here keeps the joiner's HUD from lying.
///
/// ~70 bytes at 4 Hz is 0.3 KB/s — noise next to the crowd stream.
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct RunSnapMsg {
    pub run_seed: u64,
    pub stage: u8,
    /// Planet chain as kind codes. Sent rather than derived: the chain comes from the
    /// start planet and tier picked in each machine's OWN menu, so a joiner would
    /// otherwise generate a different sequence of worlds.
    pub chain: Vec<u8>,
    pub timer: f32,
    pub elapsed: f32,
    pub total_elapsed: f32,
    pub difficulty: f32,
    pub kills: u64,
    pub gold_collected: u64,
    pub silver_run: u64,
    pub static_active: bool,
    pub static_timer: f32,
    pub boss_spawned: bool,
    pub boss_dead: bool,
    pub teleporter_open: bool,
}

/// Client-side: has the authoritative run arrived yet? A joiner must NOT build its world
/// until it has the host's seed, or it builds the wrong one and has to tear it down.
#[derive(Resource, Default)]
pub struct RunSync {
    pub seeded: bool,
    pub world_built: bool,
}

/// One boss on the wire. Bosses get their OWN lane rather than riding the crowd stream for
/// two reasons: `spawn_boss` hardcodes `Enemy.kind = Bruiser` and picks the mesh from
/// `BossKind` instead, so a crowd record would draw THE CRATERPILLAR as a Bruiser; and the
/// HUD edge marker has to point at a boss from the far side of the planet, so bosses must
/// never be interest-culled.
///
/// There are at most three at once, so this lane uses exact f32 positions and plain
/// postcard encoding — quantizing would save bytes nobody is short of.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct BossRec {
    pub id: u16,
    pub kind: u8,
    pub dir: [f32; 3],
    /// Fraction of max HP. The client's proxy carries max_hp = 1.0, so the boss bar
    /// (which reads Enemy.hp / Enemy.max_hp) is correct without sending absolute numbers.
    pub hp_frac: f32,
    pub phase: u8,
    /// Anubot's verdict beam: angle plus state (0 idle, 1 charging, 2 firing).
    pub beam_angle: f32,
    pub beam_state: u8,
}

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct BossSnapMsg {
    pub bosses: Vec<BossRec>,
}

/// Transient attack visuals: shots, telegraph rings, mortar arcs.
///
/// An EVENT lane, not a state lane. Every one of these is fully determined by its spawn
/// conditions plus time, and a client already has the identical `CurrentPlanet` (same seed
/// via RunSnapMsg) and the same `sphere::advance` math — so one event lets it integrate the
/// whole flight locally instead of streaming positions every frame. A boss slam ring costs
/// one packet rather than 1.4 seconds of updates.
///
/// Reliable (Unordered) on purpose: a telegraph is the tell for an attack that kills you,
/// so it must never be the packet that gets dropped.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum HazardEvent {
    Projectile {
        dir: [f32; 3],
        heading: [f32; 3],
        speed: f32,
        life: f32,
        /// 0 = spitter/UFO needle, 1 = beamer railbolt (different material and scale)
        style: u8,
    },
    Telegraph {
        dir: [f32; 3],
        radius: f32,
        max: f32,
        ring: bool,
    },
    Mortar {
        from: [f32; 3],
        to: [f32; 3],
        dur: f32,
    },
}

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct HazardEventMsg {
    pub events: Vec<HazardEvent>,
}

/// One chunk of a crowd snapshot. The records are HAND-PACKED into `data` rather than
/// serialized as a Vec of structs: postcard would varint-encode every field, making record
/// size data-dependent, and we need it exact to keep each chunk under renet's 1200-byte
/// slice limit (a fragmented chunk becomes all-or-nothing on an unreliable channel).
///
/// Layout of `data`: n_spawn × 8-byte descriptors, then n_update × 6-byte updates, then
/// n_despawn × 2-byte despawns.
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct EnemySnapMsg {
    pub seq: u16,
    pub chunk: u8,
    pub chunks: u8,
    /// The EXACT anchor the host quantized against — the receiving client's own astronaut
    /// direction. Sent rather than assumed: the client's predicted position differs
    /// slightly, and decoding against a different frame would skew the whole horde.
    pub anchor: [f32; 3],
    pub n_spawn: u16,
    pub n_update: u16,
    pub n_despawn: u16,
    pub data: Vec<u8>,
}

/// Client-side: which PlayerId the host says we are. `None` until the handshake lands.
#[derive(Resource, Default, Debug)]
pub struct MyPlayerId(pub Option<u8>);

/// Client -> host input intent. The host is authoritative: it applies these to the
/// matching astronaut and simulates the result.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PlayerInputMsg {
    /// Desired move direction in world space (tangent to the sphere), already normalized.
    pub wish: Vec3,
    /// Camera forward, so the host can resolve aim/facing the same way the client sees it.
    pub forward: Vec3,
    pub jump: bool,
    pub slide: bool,
    pub interact: bool,
}

/// Host-side: which `PlayerId` we handed to each connected client entity.
/// Slots are reused when someone leaves, so a 4-player lobby never runs out of ids.
#[derive(Resource, Default)]
pub struct PeerSlots {
    map: HashMap<Entity, u8>,
}

impl PeerSlots {
    /// Lowest free id in 1..MAX_PLAYERS (0 is always the host's own astronaut).
    fn claim(&mut self, client: Entity) -> Option<u8> {
        let taken: Vec<u8> = self.map.values().copied().collect();
        let id = (1..MAX_PLAYERS as u8).find(|i| !taken.contains(i))?;
        self.map.insert(client, id);
        Some(id)
    }
    fn release(&mut self, client: Entity) -> Option<u8> {
        self.map.remove(&client)
    }
    pub fn player_id(&self, client: Entity) -> Option<u8> {
        self.map.get(&client).copied()
    }
}

/// Dev harness. `--botinput` makes this instance walk without a human at the keyboard,
/// so two launched instances can prove input routing end-to-end. `--netlog` prints each
/// astronaut's surface position once a second on the host.
#[derive(Resource, Default)]
pub struct NetDebug {
    pub bot: bool,
    pub log: bool,
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetRole>()
            .add_plugins((RepliconPlugins, RepliconRenetPlugins))
            // what crosses the wire
            .replicate::<PlayerId>()
            .replicate::<NetTransform>()
            .replicate::<PlayerVitals>()
            // client -> host intent
            .add_client_message::<PlayerInputMsg>(Channel::Unreliable)
            .add_server_message::<AssignPlayerId>(Channel::Ordered)
            // Registered LAST of the server messages on purpose: registration order is
            // renet priority order, and the crowd is what should starve first if the
            // link is tight.
            .add_server_message::<RunSnapMsg>(Channel::Unordered)
            .make_message_independent::<RunSnapMsg>()
            .init_resource::<RunSync>()
            .add_server_message::<HazardEventMsg>(Channel::Unordered)
            .make_message_independent::<HazardEventMsg>()
            .add_server_message::<BossSnapMsg>(Channel::Unreliable)
            .make_message_independent::<BossSnapMsg>()
            .add_server_message::<EnemySnapMsg>(Channel::Unreliable)
            // Without this the stream is gated on ServerTick and silently dropped for any
            // client that isn't AuthorizedClient yet.
            .make_message_independent::<EnemySnapMsg>()
            .init_resource::<MyPlayerId>()
            .add_systems(Startup, apply_cli_net)
            .init_resource::<PeerSlots>()
            .add_systems(
                Update,
                push_run_snapshot
                    .run_if(is_hosting)
                    .run_if(on_timer(Duration::from_millis(250))),
            )
            .add_systems(
                PreUpdate,
                apply_run_snapshot
                    .after(ClientSystems::Receive)
                    .run_if(is_client),
            )
            .add_systems(
                Update,
                announce_player_ids
                    .run_if(is_hosting)
                    .run_if(on_timer(Duration::from_millis(500))),
            )
            .add_systems(
                PreUpdate,
                receive_player_id
                    .after(ClientSystems::Receive)
                    .run_if(is_client),
            )
            .add_systems(Update, report_connection)
            // CLIENT: our keyboard intent goes up the wire every frame.
            .init_resource::<NetDebug>()
            .add_systems(
                Update,
                bot_input
                    .after(crate::player::gather_local_input)
                    .before(crate::player::player_input)
                    .run_if(|d: Res<NetDebug>| d.bot),
            )
            .add_systems(Update, log_astronauts.run_if(|d: Res<NetDebug>| d.log))
            .add_systems(
                Update,
                crate::netenemy::log_stream_stats.run_if(|d: Res<NetDebug>| d.log),
            )
            .add_systems(
                Update,
                send_local_input
                    // Must be the LAST touch of InputIntent before movement — anything
                    // that writes intent after this point would move us locally but
                    // never reach the host, which desyncs silently.
                    .after(crate::player::gather_local_input)
                    .after(bot_input)
                    .before(crate::player::player_input)
                    .run_if(is_client),
            )
            // HOST: seat/unseat joining players, then apply their intent to their astronaut.
            // Ordered before movement so intent lands the same frame it arrives.
            .add_systems(
                Update,
                (seat_joining_players, unseat_leaving_players, apply_remote_input)
                    .chain()
                    .before(crate::player::player_input)
                    .run_if(is_hosting),
            )
            .add_systems(
                Update,
                (push_net_transform, push_player_vitals).run_if(is_simulating),
            );
    }
}

pub fn is_simulating(role: Res<NetRole>) -> bool {
    role.simulates()
}
pub fn is_client(role: Res<NetRole>) -> bool {
    matches!(*role, NetRole::Client)
}
fn is_hosting(role: Res<NetRole>) -> bool {
    matches!(*role, NetRole::Host)
}

/// HOST. Keep telling each authorized client which player it is. Deliberately repeated
/// rather than sent once on connect: replicon buffers server messages until the client is
/// authorized and only flushes on a ServerTick change, so a one-shot send from the seating
/// code can vanish on the first frames without any visible error.
///
/// `SendTargets::Single`, never `All` — `All` also writes the message into the HOST's own
/// message queue (for listen-server support), which would make the host think it had been
/// assigned a client's id.
fn announce_player_ids(
    slots: Res<PeerSlots>,
    clients: Query<Entity, (With<ConnectedClient>, With<AuthorizedClient>)>,
    mut out: MessageWriter<ToClients<AssignPlayerId>>,
) {
    for client in &clients {
        if let Some(id) = slots.player_id(client) {
            out.write(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: AssignPlayerId(id),
            });
        }
    }
}

/// HOST -> clients: the authoritative run. `CLIENTS_ONLY`, never `All` — `All` also writes
/// the message into the host's own queue for listen-server support, and the host would
/// then apply its own snapshot back over its authoritative RunState.
fn push_run_snapshot(run: Res<crate::run::RunState>, mut out: MessageWriter<ToClients<RunSnapMsg>>) {
    out.write(ToClients {
        targets: SendTargets::CLIENTS_ONLY,
        message: RunSnapMsg {
            run_seed: run.run_seed,
            stage: run.stage as u8,
            chain: run.chain.iter().map(|k| planet_code(*k)).collect(),
            timer: run.timer,
            elapsed: run.elapsed,
            total_elapsed: run.total_elapsed,
            difficulty: run.difficulty,
            kills: run.kills,
            gold_collected: run.gold_collected,
            silver_run: run.silver_run,
            static_active: run.static_active,
            static_timer: run.static_timer,
            boss_spawned: run.boss_spawned,
            boss_dead: run.boss_dead,
            teleporter_open: run.teleporter_open,
        },
    });
}

/// CLIENT: adopt the host's run wholesale. These fields are the host's to own — the client
/// never simulates them, it only displays them.
fn apply_run_snapshot(
    mut msgs: MessageReader<RunSnapMsg>,
    mut run: ResMut<crate::run::RunState>,
    mut sync: ResMut<RunSync>,
) {
    for m in msgs.read() {
        let first = !sync.seeded;
        run.run_seed = m.run_seed;
        run.stage = m.stage as usize;
        if !m.chain.is_empty() {
            run.chain = m.chain.iter().map(|c| planet_from_code(*c)).collect();
        }
        run.timer = m.timer;
        run.elapsed = m.elapsed;
        run.total_elapsed = m.total_elapsed;
        run.difficulty = m.difficulty;
        run.kills = m.kills;
        run.gold_collected = m.gold_collected;
        run.silver_run = m.silver_run;
        run.static_active = m.static_active;
        run.static_timer = m.static_timer;
        run.boss_spawned = m.boss_spawned;
        run.boss_dead = m.boss_dead;
        run.teleporter_open = m.teleporter_open;
        sync.seeded = true;
        if first {
            info!("NET adopted host run: seed={} stage={}", m.run_seed, m.stage);
        }
    }
}

/// Explicit wire codes — this is a wire format, so it must not shift if the enum is
/// ever reordered.
fn planet_code(k: crate::content::planets::PlanetKind) -> u8 {
    use crate::content::planets::PlanetKind::*;
    match k {
        Moon => 0,
        Mars => 1,
        DarkMoon => 2,
    }
}
fn planet_from_code(c: u8) -> crate::content::planets::PlanetKind {
    use crate::content::planets::PlanetKind::*;
    match c {
        1 => Mars,
        2 => DarkMoon,
        _ => Moon,
    }
}

/// CLIENT. Latch our identity. Logged only on change, since the host repeats the message.
fn receive_player_id(mut msgs: MessageReader<AssignPlayerId>, mut mine: ResMut<MyPlayerId>) {
    for m in msgs.read() {
        if mine.0 != Some(m.0) {
            info!("NET assigned PlayerId {}", m.0);
            mine.0 = Some(m.0);
        }
    }
}

/// CLIENT -> HOST. We send intent every frame rather than on-change: it's a handful of
/// bytes on an unreliable channel, and a dropped "I'm still holding W" packet would
/// otherwise read as a stutter-stop on the host.
fn send_local_input(
    q: Query<&InputIntent, With<LocalPlayer>>,
    mut out: MessageWriter<PlayerInputMsg>,
    time: Res<Time>,
    dbg: Res<NetDebug>,
    mut next: Local<f32>,
    mut sent: Local<u32>,
) {
    let Ok(intent) = q.single() else { return };
    *sent += 1;
    if dbg.log && time.elapsed_secs() >= *next {
        *next = time.elapsed_secs() + 1.0;
        info!("NET tx: {} input msgs, wish=({:.2},{:.2},{:.2})", *sent, intent.wish.x, intent.wish.y, intent.wish.z);
        *sent = 0;
    }
    out.write(PlayerInputMsg {
        wish: intent.wish,
        forward: intent.forward,
        jump: intent.jump,
        slide: intent.slide,
        interact: intent.interact,
    });
}

/// Dev harness: override the local intent with a slow circle-strafe. Runs after the
/// keyboard gather, so it stands in for a human holding W and easing the stick over.
fn bot_input(time: Res<Time>, mut q: Query<(&crate::player::Player, &mut InputIntent), With<LocalPlayer>>) {
    let Ok((p, mut intent)) = q.single_mut() else { return };
    let (t, b) = crate::sphere::tangent_frame(p.dir);
    let a = time.elapsed_secs() * 0.35;
    intent.wish = (t * a.cos() + b * a.sin()).normalize_or_zero();
    intent.forward = t;
}

/// Dev harness: once a second, print where every astronaut actually is. This is how we
/// check a remote player is being MOVED by their input, not merely connected.
fn log_astronauts(
    time: Res<Time>,
    role: Res<NetRole>,
    mut next: Local<f32>,
    q: Query<(&PlayerId, &crate::player::Player, Option<&LocalPlayer>)>,
    // Entities that arrived over the wire: they carry PlayerId/NetTransform but no local
    // `Player` component or mesh yet — that's the next step (remote player visuals).
    replicated: Query<&NetTransform, Without<crate::player::Player>>,
    rigs: Query<(&PlayerId, &crate::remote::RemoteAstronaut)>,
    n_players: Query<(), With<crate::player::Player>>,
    n_states: Query<(), With<crate::run::PlayerState>>,
    props: Option<Res<crate::planet::PropColliders>>,
    run: Res<crate::run::RunState>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 1.0;
    // World-layout checksum. This is the ONLY thing that proves both machines generated
    // the same planet from the same seed — a mismatch means the joiner is walking a
    // different world, which otherwise only shows up much later as streamed enemies
    // clipping through rocks that aren't there.
    let (layout, n_props): (i64, usize) = props
        .as_ref()
        .map(|p| {
            (
                p.0.iter()
                    .map(|c| (c.dir.x as f64 * 1e6) as i64 + (c.dir.z as f64 * 1e6) as i64)
                    .sum(),
                p.0.len(),
            )
        })
        .unwrap_or((0, 0));
    info!(
        "NET[{:?}] seed={} stage={} props={} layout_sum={} timer={:.1} kills={}",
        *role,
        run.run_seed,
        run.stage,
        n_props,
        layout,
        run.timer,
        run.kills
    );

    // Player/PlayerState counts are the load-bearing invariant: ~30 systems find the player
    // with .single(), so on a CLIENT these must stay at exactly 1 no matter how many
    // teammates are drawn. If either climbs above 1 on a client, remote visuals have leaked
    // a simulation component and the HUD/camera/enemy-targeting have gone silently dead.
    info!(
        "NET[{:?}] replicated={} local_players={} player_states={}",
        *role,
        replicated.iter().count(),
        n_players.iter().count(),
        n_states.iter().count()
    );
    for (pid, r) in &rigs {
        info!(
            "NET[{:?}] remote rig player {} drawn at ({:.3},{:.3},{:.3}) speed={:.2}",
            *role, pid.0, r.dir.x, r.dir.y, r.dir.z, r.speed
        );
    }
    for (pid, p, local) in &q {
        let tag = if local.is_some() { "local" } else { "remote" };
        info!(
            "NET[{:?}] player {} ({tag}) dir=({:.3},{:.3},{:.3}) speed={:.2}",
            *role, pid.0, p.dir.x, p.dir.y, p.dir.z, p.vel_t.length()
        );
    }
}

/// HOST. Give each newly-connected client a PlayerId and an astronaut on the surface.
fn seat_joining_players(
    mut commands: Commands,
    mut slots: ResMut<PeerSlots>,
    // NOT `Added<ConnectedClient>`: a client can connect while the host is still in the
    // menu, and a one-shot Added event would be consumed and never retried. Reconciling
    // against the live set instead means they get seated whenever the run actually starts.
    joined: Query<Entity, With<ConnectedClient>>,
    planet: Option<Res<crate::planet::CurrentPlanet>>,
    run: Option<Res<crate::run::RunState>>,
    save: Option<Res<crate::save::MetaSave>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<&PlayerId>,
) {
    let mut respawn: Vec<(u8, Entity)> = Vec::new();
    let (Some(planet), Some(run), Some(save)) = (planet, run, save) else {
        return; // not in a run yet — they'll be seated when the drop happens
    };
    // Which player ids currently have a body on the surface. Checked every frame rather
    // than trusting the slot table: astronauts are StageScoped, so the stage transition
    // despawns peers and respawns only player 0. Without this reconciliation a teammate
    // vanishes for good after the first planet — and on the client their rig disappears
    // with the replicated entity, which looks exactly like a netcode fault.
    let alive: Vec<u8> = existing.iter().map(|pid| pid.0).collect();

    for client in &joined {
        if let Some(id) = slots.player_id(client) {
            if alive.contains(&id) {
                continue; // seated and embodied
            }
            respawn.push((id, client));
            continue;
        }
        let Some(id) = slots.claim(client) else {
            warn!("lobby full — refusing extra client {client}");
            continue;
        };
        crate::player::spawn_player(
            &mut commands,
            &mut meshes,
            &mut materials,
            &planet,
            &run,
            &save,
            id,
            run.character,
            false, // remote: no LocalPlayer marker, no camera, driven by their input
            None,
        );
        info!("NET seated client {client} as player {id}");
    }

    // Re-embody peers whose astronaut was reaped by a stage change.
    for (id, client) in respawn {
        crate::player::spawn_player(
            &mut commands,
            &mut meshes,
            &mut materials,
            &planet,
            &run,
            &save,
            id,
            run.character,
            false,
            None,
        );
        info!("NET re-seated client {client} as player {id} after stage change");
    }
}

/// HOST. Someone dropped: free their slot and remove their astronaut.
fn unseat_leaving_players(
    mut commands: Commands,
    mut slots: ResMut<PeerSlots>,
    connected: Query<Entity, With<ConnectedClient>>,
    astronauts: Query<(Entity, &PlayerId)>,
) {
    let live: Vec<Entity> = connected.iter().collect();
    let gone: Vec<Entity> = slots
        .map
        .keys()
        .copied()
        .filter(|c| !live.contains(c))
        .collect();
    for client in gone {
        let Some(id) = slots.release(client) else { continue };
        for (e, pid) in &astronauts {
            if pid.0 == id {
                commands.entity(e).despawn();
            }
        }
        info!("NET client {client} left — freed player {id}");
    }
}

/// HOST. Route each client's intent onto the astronaut it owns. This is the whole point
/// of `InputIntent`: from here down, a remote player is indistinguishable from the local
/// one, so movement/physics/combat need no networking awareness at all.
fn apply_remote_input(
    slots: Res<PeerSlots>,
    mut incoming: MessageReader<FromClient<PlayerInputMsg>>,
    mut astronauts: Query<(&PlayerId, &mut InputIntent), Without<LocalPlayer>>,
    time: Res<Time>,
    dbg: Res<NetDebug>,
    mut next: Local<f32>,
    mut got: Local<u32>,
) {
    if dbg.log && time.elapsed_secs() >= *next {
        *next = time.elapsed_secs() + 1.0;
        info!("NET rx: {} input msgs this second", *got);
        *got = 0;
    }
    for FromClient { client_id, message } in incoming.read() {
        *got += 1;
        let Some(client) = client_id.entity() else {
            warn!("NET rx: message from ClientId::Server (listen-server loopback?)");
            continue;
        };
        let Some(id) = slots.player_id(client) else {
            warn!("NET rx: no slot for client {client}");
            continue;
        };
        for (pid, mut intent) in &mut astronauts {
            if pid.0 == id {
                intent.wish = message.wish;
                intent.forward = message.forward;
                // Edge-triggered actions are OR-ed in rather than assigned: several input
                // packets can arrive in one host frame, and a jump in any of them counts.
                intent.jump |= message.jump;
                intent.slide |= message.slide;
                intent.interact |= message.interact;
            }
        }
    }
}

/// Log real connection state transitions so "it connected" is verifiable, not assumed.
fn report_connection(
    role: Res<NetRole>,
    client_state: Option<Res<State<ClientState>>>,
    server_state: Option<Res<State<ServerState>>>,
    clients: Query<(), With<ConnectedClient>>,
    mut last_client: Local<Option<ClientState>>,
    mut last_peers: Local<usize>,
) {
    if let Some(cs) = client_state {
        let now = *cs.get();
        if last_client.map(|p| p != now).unwrap_or(true) {
            info!("NET client state -> {now:?}");
            *last_client = Some(now);
        }
    }
    if role.simulates() {
        let peers = clients.iter().count();
        if peers != *last_peers {
            let running = server_state.map(|s| *s.get() == ServerState::Running).unwrap_or(false);
            info!("NET peers connected: {peers} (server running: {running})");
            *last_peers = peers;
        }
    }
}

/// Start hosting on `port`. The host keeps simulating locally — it's a listen server,
/// so the hosting player plays too (no dedicated box needed).
pub fn start_host(commands: &mut Commands, channels: &RepliconChannels, port: u16) -> Result<(), String> {
    let server = RenetServer::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).map_err(|e| {
        format!("couldn't open port {port} (already hosting, or the port is taken): {e}")
    })?;
    let transport = NetcodeServerTransport::new(
        ServerConfig {
            current_time,
            max_clients: MAX_PLAYERS - 1,
            protocol_id: PROTOCOL_ID,
            authentication: ServerAuthentication::Unsecure,
            public_addresses: Default::default(),
        },
        socket,
    )
    .map_err(|e| e.to_string())?;

    commands.insert_resource(server);
    commands.insert_resource(transport);
    commands.insert_resource(NetRole::Host);
    info!("hosting on port {port}");
    Ok(())
}

/// Join a host at `addr:port`.
pub fn start_join(
    commands: &mut Commands,
    channels: &RepliconChannels,
    addr: IpAddr,
    port: u16,
) -> Result<(), String> {
    let client = RenetClient::new(ConnectionConfig {
        server_channels_config: channels.server_configs(),
        client_channels_config: channels.client_configs(),
        ..Default::default()
    });
    let current_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| e.to_string())?;
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).map_err(|e| e.to_string())?;
    let transport = NetcodeClientTransport::new(
        current_time,
        ClientAuthentication::Unsecure {
            client_id: current_time.as_millis() as u64,
            protocol_id: PROTOCOL_ID,
            server_addr: SocketAddr::new(addr, port),
            user_data: None,
        },
        socket,
    )
    .map_err(|e| e.to_string())?;

    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(NetRole::Client);
    info!("joining {addr}:{port}");
    Ok(())
}

/// Tear the connection down and go back to solo.
pub fn disconnect(commands: &mut Commands) {
    commands.remove_resource::<RenetServer>();
    commands.remove_resource::<NetcodeServerTransport>();
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
    commands.insert_resource(NetRole::Solo);
}

/// Convenience for the CLI/test path: `--host` or `--join <ip>` at startup.
pub fn apply_cli_net(mut commands: Commands, channels: Res<RepliconChannels>) {
    let args: Vec<String> = std::env::args().collect();
    commands.insert_resource(NetDebug {
        bot: args.iter().any(|a| a == "--botinput"),
        log: args.iter().any(|a| a == "--netlog"),
    });
    if args.iter().any(|a| a == "--host") {
        let port = args
            .iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        if let Err(e) = start_host(&mut commands, &channels, port) {
            error!("host failed: {e}");
        }
    } else if let Some(i) = args.iter().position(|a| a == "--join") {
        let ip: IpAddr = args
            .get(i + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
        let port = args
            .iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        if let Err(e) = start_join(&mut commands, &channels, ip, port) {
            error!("join failed: {e}");
        }
    }
}

/// Host: mirror the authoritative sim state onto the replicated components.
/// (Solo runs this too and it's a couple of writes — harmless, and it means the host
/// path is always exercised, so co-op can't silently rot.)
fn push_net_transform(
    mut q: Query<(&crate::player::Player, &mut NetTransform)>,
) {
    for (p, mut nt) in &mut q {
        nt.dir = p.dir;
        nt.height = p.height;
        nt.facing = p.facing;
    }
}

fn push_player_vitals(
    mut q: Query<(&crate::run::PlayerState, &mut PlayerVitals)>,
) {
    for (ps, mut v) in &mut q {
        v.hp = ps.hp;
        v.max_hp = ps.stats.max_hp;
        v.level = ps.level;
        v.down = ps.dead;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NETCODE NOTES — the remaining work, in the order it should be tackled
//
// 1. TRANSPORT WIRING — DONE. Verified handshake over UDP with two live instances.
//
// 2. INPUT ROUTING — DONE. Verified: a bot-driven client walks a circle and the HOST's
//    astronaut for that player traces the same path, while the host's own astronaut sits
//    still. Repro:
//        astrobonk.exe --host --autodrop --netlog
//        astrobonk.exe --join 127.0.0.1 --autodrop --botinput --netlog
//    The shape that made this simple is `InputIntent`: hardware input and network input
//    both write the same component, so movement/physics/combat contain zero net code.
//    ORDERING TRAP (cost a debug cycle): the send system must run AFTER everything that
//    writes intent. It was ordered after the keyboard gather but not after the bot
//    harness, so it shipped an empty wish while the client moved locally — a silent
//    desync that looked exactly like a dropped-packet problem. Anything that writes
//    intent must be `.before(send_local_input)`.
//
// 2b. REMOTE PLAYER VISUALS — DONE (see remote.rs). A client draws each teammate with the
//    real astronaut rig, eased toward the replicated pose and animated by the same
//    `animate_rig` the local player uses. Verified: client draws exactly ONE rig (the
//    host's, not its own server-side copy), and its derived speed matches the host's
//    authoritative |vel_t| within 0.5% — which matters because gait amplitude is
//    speed/PLAYER_RUN_SPEED, so a wrong speed means skating feet.
//    Two derivation traps, both silent under-estimates, both cost a measurement cycle:
//      * integrate with the LOCAL surface radius (planet.surface(dir) + height), the same
//        one sphere::advance uses — not the nominal planet.radius;
//      * measure the CHORD, not angle_between: the per-frame angle is ~1e-3 rad, so
//        acos(dot) lands where dot ~ 1 - 5e-7 and f32 quantizes it toward 1.0.
//    Identity comes from an AssignPlayerId server message — replicon 0.40 exposes no
//    local-client-id API, and PlayerId alone can't answer "which one is me?" because the
//    client's predicted body and the HOST's body are both PlayerId(0).
//
// 2c. HOST-SIDE MULTI-PLAYER QUERIES — DONE (Stage 3). Was the biggest hole; see the
//    commit for the full list. Repro any regression with `--headless --coop2`.
//    HISTORICAL NOTE, kept because it explains the shape of the fix:
//    MEASURED, not theoretical: with one client joined the host reports
//    `local_players=2 player_states=2`. About thirty systems find the player with
//    `.single()`, so on a 2-player host they all return Err(MultipleEntities) and silently
//    early-return: HUD, weapon fire, pickups, interaction, level-up panels, enemy
//    targeting. Two fail even quieter — the Anubot verdict beam and Craterpillar contact
//    damage just stop dealing damage. The client is unaffected (it keeps exactly one
//    Player by design), so co-op currently LOOKS right on the joiner and is broken on the
//    host. Fix: `With<LocalPlayer>` for camera/HUD/panels, iterate for per-player sim, and
//    nearest-of-many for enemy targeting. Note apply_player_hits needs a message-shape
//    change, not a query change — PlayerHitMsg carries no victim entity. (Done: it does
//    now.)
//
// 2d. Smaller follow-ups: replicate each player's AstronautKind so teammates wear their own
//    suit (remote.rs currently picks a stable palette by slot, and seat_joining_players
//    spawns peers with the HOST's character); add slide state to NetTransform so remotes
//    tuck; cache the rig's meshes/materials and drop shadow-casting on remote flashlights
//    (each rig currently allocates 8 meshes, 5 materials and a shadow-casting spotlight).
//
// 3. ENEMY STREAMING — the real performance problem. With a 1200-enemy cap, per-entity
//    replication is not viable. Plan (per the GDD): send compact quantized batches with
//    INTEREST MANAGEMENT — full fidelity for the horde on the client's arc of the planet,
//    coarse/aggregated for the far side they can barely see. The sphere is a bandwidth
//    gift here: the far horizon is naturally low-detail. Prototype this with two local
//    instances BEFORE building any lobby UI, because it decides whether the design holds.
//
// 4. CO-OP RULES: shared XP grant on gem pickup, per-player gold, revives (the Tumbling
//    Beacon), enemy scaling by player count.
// ─────────────────────────────────────────────────────────────────────────────
