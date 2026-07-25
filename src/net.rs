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
use std::time::SystemTime;

/// Bumped whenever the wire format changes — mismatched builds refuse to connect
/// instead of desyncing in confusing ways.
pub const PROTOCOL_ID: u64 = 0xA570B0_1;
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
            .add_systems(Startup, apply_cli_net)
            .init_resource::<PeerSlots>()
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

fn is_simulating(role: Res<NetRole>) -> bool {
    role.simulates()
}
fn is_client(role: Res<NetRole>) -> bool {
    matches!(*role, NetRole::Client)
}
fn is_hosting(role: Res<NetRole>) -> bool {
    matches!(*role, NetRole::Host)
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
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 1.0;
    info!(
        "NET[{:?}] replicated astronauts received: {}",
        *role,
        replicated.iter().count()
    );
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
) {
    let (Some(planet), Some(run), Some(save)) = (planet, run, save) else {
        return; // not in a run yet — they'll be seated when the drop happens
    };
    for client in &joined {
        if slots.player_id(client).is_some() {
            continue; // already seated
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
        );
        info!("NET seated client {client} as player {id}");
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
// 2b. REMOTE PLAYER VISUALS — NEXT, and the smallest useful step. Replication is
//    confirmed delivering: a client sees 2 entities carrying PlayerId + NetTransform +
//    PlayerVitals, but with no `Player` component and no mesh, so nothing is drawn.
//    Needed: (a) on the client, build the astronaut rig for each replicated PlayerId and
//    drive its Transform from NetTransform (smoothed — raw snapshots will jitter);
//    (b) tell the client its OWN PlayerId so it can skip the server copy of itself,
//    otherwise every player sees a ghost twin of themselves standing where the host
//    thinks they are.
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
