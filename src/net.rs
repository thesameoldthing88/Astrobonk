//! Co-op networking layer (host-authoritative).
//!
//! Design (per the GDD): one machine is the HOST and owns the simulation — the swarm,
//! the clock, bosses, loot. Clients send only *input intent* and locally predict their
//! own astronaut; everything else is replicated down. Auto-attacks mean there's no
//! "did my shot hit?" latency pain — the host decides and it looks the same to everyone.
//!
//! What replicates (deliberately small — with ~1200 enemies, bandwidth is the budget):
//!   * `PlayerId`        — who an astronaut belongs to
//!   * `NetTransform`    — position/facing/slide of each astronaut
//!   * `PlayerVitals`    — hp / max_hp / level, for teammate HUD + the down-state
//!   * `NetHero`         — which hero it is, so every machine draws the right suit
//!   * `NetComet`        — its Comet Combo, for its owner's HUD and everyone's tail sparks
//! A player's *build* (weapons, items, cards) stays local — each player picks their own
//! upgrades, so only its visible effects need to cross the wire.
//!
//! Enemies are NOT replicated per-entity yet; see `NETCODE NOTES` at the bottom.

use crate::player::{InputIntent, LocalPlayer, PlayerId};
use std::collections::HashMap;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;
use bevy_replicon_renet::{
    netcode::{
        ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport, ServerAuthentication,
        ServerConfig, NETCODE_USER_DATA_BYTES,
    },
    renet::ConnectionConfig,
    RenetChannelsExt, RenetClient, RenetServer, RepliconRenetPlugins,
};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use bevy::time::common_conditions::on_timer;
use std::time::{Duration, SystemTime};

/// Bumped whenever the wire format changes — mismatched builds refuse to connect instead of
/// desyncing in confusing ways. The refusal is a netcode handshake that never completes (a
/// wrong id means the packets do not even decrypt), so `watch_client_connection` is what
/// turns the resulting timeout into a readable "different version?" line.
pub const PROTOCOL_ID: u64 = 0xA570B0_5; // bumped: §13 assists in RunSnapMsg, revives in PlayerVitals, burrow cracks on the hazard lane (P04)
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
    /// Mid-slide. Rides the pose rather than being derived on the client: a slide is a
    /// speed burst the finite-differenced speed can't tell apart from a bhop, and the rig
    /// has to TUCK for the whole slide, not just while it happens to be fast.
    pub sliding: bool,
}

/// Which hero an astronaut is, as an explicit wire code (`hero_code`). Replicated so every
/// machine draws a teammate in THEIR suit — without it a client could only guess a palette
/// by slot. Written only when it changes, so it costs nothing after the join.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetHero(pub u8);

/// An astronaut's Comet Combo as the HUD needs it. The HOST computes every astronaut's
/// combo (it owns the horde the tail is counted from) and mirrors it here; each machine's
/// HUD — the host's included — reads only this, so there is one presentation path.
///
/// `fires` is a lifetime counter rather than a "just cashed out" flag: replication sends
/// the LATEST value, so a one-frame flag can be skipped entirely, while a counter that went
/// up is noticed no matter how many updates were lost. `peak` is that cash-out's tail.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetComet {
    pub active: bool,
    pub count: u16,
    /// Charge progress quantized to 0..=255 — the HUD bar has 12 cells, and a coarse value
    /// only changes (and so only replicates) a few times a second.
    pub progress: u8,
    pub fires: u16,
    pub peak: u16,
}

/// Replicated teammate vitals — what another player's HUD marker needs to show.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PlayerVitals {
    pub hp: f32,
    pub max_hp: f32,
    pub level: u32,
    pub down: bool,
    /// "One more chance" revives spent (§13). A counter, so a joiner's HUD sees the token
    /// go and announces the revive even if a replication update in between was dropped.
    pub revives: u8,
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
    /// Which of the host's runs this is (`SessionState::run_gen`). A session outlives its
    /// runs, so a snapshot of the run that just ENDED can still be in flight (this lane is
    /// unordered) when the joiner is already back in the lobby. The generation is how it
    /// tells that straggler from the next run; the seed cannot, two daily runs share one.
    pub run_gen: u32,
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
    /// The guaranteed miniboss-#1 cache (§3): where it stands while unopened. A client
    /// spawns/pops its copy from this alone (`interact::sync_reward_cache`).
    pub reward_chest: Option<[f32; 3]>,
    /// Mars's MIGRATING DUST STORM. The host rolls where it forms and when; a client
    /// dead-reckons the drift between snapshots from the heading, so 4 Hz is plenty for a
    /// cell that moves 3 m/s. Without it a joiner neither sees the storm nor gets the haze
    /// that tells them the ranged horde has lost them.
    pub storm_active: bool,
    pub storm_dir: [f32; 3],
    pub storm_heading: [f32; 3],
    pub storm_radius: f32,
    /// §13 "difficulty as options": the HOST's assists govern the whole squad's run, and a
    /// joiner's HUD shows the ASSISTED tag and its "one more chance" token from these.
    pub assist: crate::save::AssistOptions,
    pub assisted: bool,
}

/// HOST -> CLIENT: the session is over. Sent just before the host drops the connection, so
/// a joiner lands back on the menu with a reason instead of a timeout. Only the host ending
/// co-op sends it; a finished RUN does not end the session (see `RunOverMsg`).
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct SessionEndMsg {
    /// SESSION_END_* code. Explicit numbers: this is a wire format.
    pub reason: u8,
}

pub const SESSION_END_BY_HOST: u8 = 0;

/// HOST -> CLIENT: the host's run is over, the session is not. Every joiner goes back to
/// the main menu, still connected, and follows the host into its next run from there — a
/// squad plays run after run without hosting and joining again each time.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct RunOverMsg {
    /// The run that ended; its late snapshots are ignored from now on.
    pub run_gen: u32,
    /// RUN_OVER_* code. Explicit numbers: this is a wire format.
    pub reason: u8,
}

pub const RUN_OVER_VICTORY: u8 = 0;
pub const RUN_OVER_WIPED: u8 = 1;
pub const RUN_OVER_ABANDONED: u8 = 2;

/// LOCAL (never networked): the pause menu's LEAVE / END SESSION button. A client leaves
/// (the host plays on); a host ends the session for everyone and keeps its run solo.
#[derive(Message, Clone, Copy, Debug)]
pub struct LeaveSession;

/// Client-side: has the authoritative run arrived yet? A joiner must NOT build its world
/// until it has the host's seed, or it builds the wrong one and has to tear it down.
#[derive(Resource, Default)]
pub struct RunSync {
    pub seeded: bool,
    /// A world is standing (set by `enter_run`, cleared when the run is left). Only then
    /// can a snapshot ask for a rebuild — before it, `enter_run` builds the right one.
    pub world_built: bool,
    /// The (run seed, stage) the standing world was generated from. Compared with every
    /// snapshot rather than trusting the stage number alone: a world built from any other
    /// seed (the host's next run, which also starts on stage 0) has every rock, pot and
    /// shrine somewhere the host never put them.
    pub built_for: Option<(u64, usize)>,
    /// Set when the host's snapshot describes a world we are not standing on. A client
    /// cannot reach `stage_transition` on its own — that runs off the teleporter
    /// interaction, which is host-only — so this is the only signal it gets.
    pub pending_stage: Option<usize>,
    /// The oldest run generation still accepted (see `RunSnapMsg::run_gen`). Raised past a
    /// run when it ends, and to each run adopted, so a straggler can never pull us back.
    pub min_gen: u32,
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

/// CLIENT -> HOST: "this is my build".
///
/// The one piece of client->host traffic besides input, and it is unavoidable: the host
/// spawns a peer with a FRESH LEVEL-1 SHEET (`carried: None`), card picks happen on the
/// client, and combat damage rolls use `rand::thread_rng()` throughout combat.rs — so there
/// is no lockstep or derived-hit scheme that would let the host recompute the peer's numbers
/// itself. The host must be told them.
///
/// Sends the DERIVED Stats rather than the item list: items only reach combat through
/// `recompute_stats`, so the derived sheet is sufficient and immune to the two machines
/// disagreeing about how an item is applied.
///
/// ~130 B on change plus a 2 s heartbeat — the `announce_player_ids` idiom, so a dropped
/// update self-heals instead of leaving the peer permanently weak.
#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct PlayerBuildMsg {
    /// NOT cosmetic: the character gates hero branches the host evaluates every frame
    /// (Nova's sprint attack-speed bonus, etc.). The connect token already seated the peer
    /// as this hero; the heartbeat keeps it so, and would correct the sheet, the host-side
    /// rig (`refit_astronaut_rigs`) and, through NetHero, every client's view of it.
    pub character: crate::content::characters::AstronautKind,
    pub level: u32,
    pub stats: crate::stats::Stats,
    /// (weapon, level) pairs. Cooldowns are deliberately NOT sent — they are host-side
    /// firing cadence, and overwriting them every heartbeat would stutter the peer's guns.
    pub weapons: Vec<(crate::content::weapons::WeaponKind, u32)>,
}

/// LOCAL (never networked): the simulation says "someone earned this". A relay turns it
/// into the right wire message. Keeps pickups.rs free of any notion of clients or channels.
#[derive(Message, Clone, Copy, Debug)]
pub enum GrantOut {
    /// shared pool — every machine applies it
    Xp(f32),
    /// collector-only — addressed to whichever machine drives that PlayerId
    Loot(u8, crate::pickups::PickupKind),
}

/// Loot on the wire. Like hazards this is an EVENT lane, not a state lane: an idle
/// pickup's transform is a pure function of (dir, time, bob), and the fly-to-player phase
/// is derived locally, so a spawn and a despawn are all a client needs to draw the whole
/// life of a gem. Reliable (Unordered): a lost despawn leaves a ghost gem forever.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub enum PickupEvent {
    Spawn { id: u16, kind: u8, value: u32, dir: [f32; 3], bob: f32 },
    Despawn { id: u16 },
}

#[derive(Message, Serialize, Deserialize, Clone, Debug)]
pub struct PickupEventMsg {
    pub events: Vec<PickupEvent>,
}

/// XP is a SHARED pool (the project's own rule, GDD: shared pool, individual level curve),
/// so a grant goes to every client. Each machine applies it to its OWN PlayerState, which
/// is what keeps level-ups local: a player still picks their own cards on their own screen.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct XpGrantMsg(pub f32);

/// Gold, food and powerups belong to whoever picked them up, so this one is addressed.
#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct LootGrantMsg {
    pub gold: u64,
    pub heal: f32,
    /// 0 = none, 1 = Damage2x, 2 = Magnet, 3 = Speed
    pub powerup: u8,
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
    // ---- appended: variant order is the wire code, never reorder ----
    /// A Beamer started painting its aim line. The line TRACKS its target until the last
    /// quarter second, so rather than streaming the line every frame we send who it is
    /// locked on: the client knows where every astronaut is and derives the same sweep.
    AimLine {
        /// the beamer's crowd-stream NetId
        enemy: u16,
        /// the PlayerId it latched onto
        target: u8,
        /// charge left, seconds
        charge: f32,
    },
    /// The host's aim line is gone: it fired, the beamer lost its target in the dust storm,
    /// or the beamer died. A client also retires a line itself when the charge runs out, so
    /// an End that arrives late (this lane is unordered) never leaves one hanging.
    AimLineEnd { enemy: u16 },
    /// A Burrower went under: its crack decal spreads over `dir` for `dur` seconds, then it
    /// erupts (the eruption's own pop arrives as a Telegraph). §13's cracking-decal tell.
    Crack { dir: [f32; 3], dur: f32 },
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

/// Session lifecycle — the bits that outlive a single frame while a session winds down.
#[derive(Resource, Default)]
pub struct SessionState {
    /// HOST: the session is winding down — SessionEndMsg went out, the transport goes next.
    pub closing: Option<SessionClose>,
    /// CLIENT: why the session ended, when someone told us (the host's SessionEndMsg,
    /// replicon's ProtocolMismatch, or our own LEAVE). Shown on the menu once we are out.
    pub end_note: Option<String>,
    /// HOST: counts the runs this machine has entered; stamped on every RunSnapMsg and on
    /// the RunOverMsg that closes one, so joiners can tell this run from the last.
    pub run_gen: u32,
}

/// HOST: how long until the transport is torn down. The delay is the point: disconnect
/// packets are sent instantly and would overtake the reliable goodbye, leaving joiners with
/// "the host ended the session" instead of why. Frames as well as seconds, because one slow
/// frame (a software-rendered menu rebuild, say) can itself be longer than the whole delay.
#[derive(Clone, Copy, Debug)]
pub struct SessionClose {
    pub secs: f32,
    pub frames: u8,
}

impl SessionClose {
    fn start() -> Self {
        Self { secs: crate::config::NET_SESSION_CLOSE_SECS, frames: 3 }
    }
}

/// CLIENT: the address we dialled, so a failed join can say WHERE it failed.
#[derive(Resource, Clone, Copy)]
pub struct JoinTarget(pub SocketAddr);

/// HOST: the port we listen on, so the lobby line between runs can repeat where to join.
#[derive(Resource, Clone, Copy)]
pub struct HostPort(pub u16);

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
    /// The hero each seated peer plays: named in its connect token (`join_user_data`), kept
    /// current by its PlayerBuildMsg. Kept per SLOT so an astronaut re-embodied after a
    /// stage change, or in the squad's next run, comes back as the same hero.
    heroes: HashMap<u8, crate::content::characters::AstronautKind>,
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
        let id = self.map.remove(&client)?;
        self.heroes.remove(&id);
        Some(id)
    }
    /// The hero to embody this slot as. The fallback (the host's own pick) only covers a
    /// token that named no hero; the peer's first build heartbeat would then re-suit it
    /// through `refit_astronaut_rigs`.
    fn hero_for(&self, id: u8, fallback: crate::content::characters::AstronautKind) -> crate::content::characters::AstronautKind {
        self.heroes.get(&id).copied().unwrap_or(fallback)
    }
    /// Reverse lookup: which connected client owns this PlayerId. Needed to address
    /// collector-only loot at the right machine.
    pub fn client_for(&self, id: u8) -> Option<Entity> {
        self.map.iter().find(|(_, v)| **v == id).map(|(k, _)| *k)
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
            .replicate::<NetHero>()
            .replicate::<NetComet>()
            // client -> host intent
            .add_client_message::<PlayerInputMsg>(Channel::Unreliable)
            .add_client_message::<PlayerBuildMsg>(Channel::Ordered)
            .add_systems(
                Update,
                send_player_build
                    .run_if(is_client)
                    .run_if(on_timer(Duration::from_millis(500))),
            )
            .add_systems(Update, apply_player_build.run_if(is_hosting))
            .add_systems(Update, adopt_my_vitals.run_if(is_client))
            .add_systems(
                Update,
                reconcile_own_astronaut
                    .before(crate::player::player_physics)
                    .run_if(in_state(crate::AppState::InRun))
                    .run_if(is_client),
            )
            .add_server_message::<AssignPlayerId>(Channel::Ordered)
            // Independent: it can be the last thing a joiner hears, possibly before it was
            // ever authorized (a version-mismatched or mid-handshake client).
            .add_server_message::<SessionEndMsg>(Channel::Ordered)
            .make_message_independent::<SessionEndMsg>()
            // Independent like the snapshots it retires: it must not wait on a ServerTick.
            .add_server_message::<RunOverMsg>(Channel::Ordered)
            .make_message_independent::<RunOverMsg>()
            // Registered LAST of the server messages on purpose: registration order is
            // renet priority order, and the crowd is what should starve first if the
            // link is tight.
            .add_server_message::<RunSnapMsg>(Channel::Unordered)
            .make_message_independent::<RunSnapMsg>()
            .init_resource::<RunSync>()
            .add_message::<GrantOut>()
            .add_systems(Update, relay_grants.run_if(is_hosting))
            .add_systems(
                PreUpdate,
                (apply_xp_grant, apply_loot_grant)
                    .after(ClientSystems::Receive)
                    .run_if(is_client),
            )
            .add_server_message::<PickupEventMsg>(Channel::Unordered)
            .make_message_independent::<PickupEventMsg>()
            .add_server_message::<XpGrantMsg>(Channel::Unordered)
            .make_message_independent::<XpGrantMsg>()
            .add_server_message::<LootGrantMsg>(Channel::Unordered)
            .make_message_independent::<LootGrantMsg>()
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
                    // Only a LIVE run is authoritative. A host still picking its hero sends
                    // the boot placeholder's seed, which a waiting joiner would adopt and
                    // build a world from — then never rebuild, because the real run starts
                    // on the same stage number.
                    .run_if(in_state(crate::AppState::InRun))
                    .run_if(is_hosting)
                    .run_if(on_timer(Duration::from_millis(250))),
            )
            .add_systems(
                PreUpdate,
                (receive_run_over, apply_run_snapshot)
                    .chain()
                    .after(ClientSystems::Receive)
                    .run_if(is_client),
            )
            .add_systems(OnEnter(crate::AppState::InRun), count_run.run_if(is_hosting))
            .add_systems(OnExit(crate::AppState::InRun), forget_built_world)
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
                (
                    // Seating needs a planet to stand on; unseating must still run from the
                    // results screen so a slot frees up however the peer left.
                    seat_joining_players.run_if(in_state(crate::AppState::InRun)),
                    unseat_leaving_players,
                    apply_remote_input,
                )
                    .chain()
                    .before(crate::player::player_input)
                    .run_if(is_hosting),
            )
            .add_systems(
                Update,
                (push_net_transform, push_player_vitals, push_net_hero).run_if(is_simulating),
            )
            // ---- session lifecycle: leaving, the host ending it, the host vanishing ----
            .init_resource::<SessionState>()
            .add_message::<LeaveSession>()
            .add_observer(on_protocol_mismatch)
            .add_systems(
                PreUpdate,
                receive_session_end.after(ClientSystems::Receive).run_if(is_client),
            )
            .add_systems(
                Update,
                (
                    handle_leave_session,
                    finish_session_close.run_if(is_hosting),
                    watch_client_connection.run_if(is_client),
                    // Runs on the frame the role flips back to Solo, from any path.
                    reset_after_session.run_if(
                        resource_changed::<NetRole>.and(|r: Res<NetRole>| *r == NetRole::Solo),
                    ),
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(crate::AppState::Results),
                // BEFORE banking: bank_results clears run.result, which is the reason we send.
                announce_run_over
                    .before(crate::director::bank_results)
                    .run_if(is_hosting),
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
fn push_run_snapshot(
    run: Res<crate::run::RunState>,
    storm: Res<crate::events_world::DustStorm>,
    session: Res<SessionState>,
    mut out: MessageWriter<ToClients<RunSnapMsg>>,
) {
    out.write(ToClients {
        targets: SendTargets::CLIENTS_ONLY,
        message: RunSnapMsg {
            run_gen: session.run_gen,
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
            reward_chest: run.reward_chest.map(|d| d.to_array()),
            storm_active: storm.active,
            storm_dir: storm.dir.to_array(),
            storm_heading: storm.heading.to_array(),
            storm_radius: storm.radius,
            assist: run.assist,
            assisted: run.assisted,
        },
    });
}

/// CLIENT: adopt the host's run wholesale. These fields are the host's to own — the client
/// never simulates them, it only displays them.
fn apply_run_snapshot(
    mut msgs: MessageReader<RunSnapMsg>,
    mut run: ResMut<crate::run::RunState>,
    mut sync: ResMut<RunSync>,
    mut storm: ResMut<crate::events_world::DustStorm>,
) {
    for m in msgs.read() {
        // A straggler from a run that has already ended (or been superseded).
        if m.run_gen < sync.min_gen {
            continue;
        }
        sync.min_gen = m.run_gen;
        let first = !sync.seeded;
        run.run_seed = m.run_seed;
        // The edge that triggers the client's rebuild: the host is on a world other than
        // the one standing here — its next stage, or a different run altogether. Only once
        // a world stands; before that `enter_run` builds the right one from these fields.
        let host_world = (m.run_seed, m.stage as usize);
        if sync.world_built && sync.built_for != Some(host_world) {
            sync.pending_stage = Some(m.stage as usize);
        }
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
        run.reward_chest = m.reward_chest.map(Vec3::from_array);
        // The storm's shape only; the host alone decides when one forms or blows out, and
        // `dust_storm_visuals` dead-reckons the drift between these snapshots.
        storm.active = m.storm_active;
        storm.dir = Vec3::from(m.storm_dir);
        storm.heading = Vec3::from(m.storm_heading);
        storm.radius = m.storm_radius;
        run.assist = m.assist;
        run.assisted = m.assisted;
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

/// Explicit hero wire codes (NetHero). Same rule as planets: never renumber, only append.
pub fn hero_code(k: crate::content::characters::AstronautKind) -> u8 {
    use crate::content::characters::AstronautKind::*;
    match k {
        Buzz => 0,
        Valentina => 1,
        B0nk => 2,
        Yuki => 3,
        ChimpO => 4,
        Doug => 5,
        Reticle => 6,
        Nova => 7,
        Ironclad => 8,
        Fortuna => 9,
        Aurora => 10,
        Gristle => 11,
    }
}
pub fn hero_from_code(c: u8) -> crate::content::characters::AstronautKind {
    use crate::content::characters::AstronautKind::*;
    match c {
        1 => Valentina,
        2 => B0nk,
        3 => Yuki,
        4 => ChimpO,
        5 => Doug,
        6 => Reticle,
        7 => Nova,
        8 => Ironclad,
        9 => Fortuna,
        10 => Aurora,
        11 => Gristle,
        _ => Buzz,
    }
}

/// Marks a connect token's user data as carrying a hero. A zeroed block (a token that says
/// nothing) must not read as hero code 0, Buzz.
const JOIN_DATA_HERO_TAG: u8 = b'H';

/// CLIENT: the netcode connect token's user data — which hero we are joining as.
///
/// The host seats a joiner with a body the moment it connects (its PlayerId, the crowd
/// stream's interest centre and its drop slot all hang off that body), and the token is
/// the only thing it holds about the joiner at that moment: client messages flow only
/// after the handshake. Riding the token is what lets the host seat a peer as THEIR hero
/// from the first frame, instead of as the host's hero until a build heartbeat corrects it.
fn join_user_data(hero: crate::content::characters::AstronautKind) -> [u8; NETCODE_USER_DATA_BYTES] {
    let mut data = [0u8; NETCODE_USER_DATA_BYTES];
    data[0] = JOIN_DATA_HERO_TAG;
    data[1] = hero_code(hero);
    data
}

/// HOST: the hero a connect token names, if it names one.
fn hero_from_user_data(data: &[u8; NETCODE_USER_DATA_BYTES]) -> Option<crate::content::characters::AstronautKind> {
    (data[0] == JOIN_DATA_HERO_TAG).then(|| hero_from_code(data[1]))
}

/// CLIENT: take our OWN hp from the host.
///
/// Once a client stops simulating, its local PlayerState.hp is frozen — nothing damages or
/// heals it locally — so the HUD would cheerfully report full health while the host had us
/// nearly dead. The host already replicates PlayerVitals for every astronaut; this reads the
/// one carrying OUR PlayerId (the server's copy of us, which remote.rs deliberately skips
/// when drawing teammates) and mirrors it into the local sheet the HUD reads.
fn adopt_my_vitals(
    mine: Res<MyPlayerId>,
    vitals: Query<(&crate::player::PlayerId, &PlayerVitals), Without<crate::player::Player>>,
    mut q: Query<&mut crate::run::PlayerState, With<crate::player::LocalPlayer>>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
    mut sfx: MessageWriter<crate::messages::SfxMsg>,
) {
    let Some(my_id) = mine.0 else { return };
    let Ok(mut ps) = q.single_mut() else { return };
    for (pid, v) in &vitals {
        if pid.0 == my_id {
            ps.hp = v.hp;
            ps.dead = v.down;
            // The host spent our "one more chance": say so here, where the player is.
            if v.revives as u32 > ps.revives {
                banners.write(crate::messages::BannerMsg("ONE MORE CHANCE!".into()));
                sfx.write(crate::messages::SfxMsg(crate::messages::Sfx::Shrine));
                info!("NET the host spent our one-more-chance (hp {:.0})", v.hp);
            }
            ps.revives = v.revives as u32;
            break;
        }
    }
}

/// CLIENT: pull our predicted astronaut toward the host's authoritative copy of it.
///
/// We move ourselves locally for crisp controls (GDD §11), but the HOST decides where we
/// really are — the horde chases that copy, the comet tail is counted around it, the dust
/// storm hides it. Unreconciled, the two drifted 3–10 m apart within a minute: every host
/// hitstop or open panel freezes our server-side body while we keep running here.
///
/// The copy always TRAILS us by speed × latency, so it is compared against the path we ran
/// over the last NET_RECONCILE_WINDOW_SECS, not our current spot: a copy sitting on that
/// path is just late, and is left alone. Only the offset from the nearest point of the
/// path is corrected — carried over to where we are now and closed exponentially — so a
/// real divergence eases home without dragging a joiner backwards through its own lag.
fn reconcile_own_astronaut(
    time: Res<Time>,
    mine: Res<MyPlayerId>,
    planet: Option<Res<crate::planet::CurrentPlanet>>,
    server_copy: Query<(&PlayerId, &NetTransform), Without<crate::player::Player>>,
    mut q: Query<&mut crate::player::Player, With<LocalPlayer>>,
    mut path: Local<std::collections::VecDeque<(f32, Vec3)>>,
) {
    use crate::config::{NET_RECONCILE_DEADZONE, NET_RECONCILE_RATE, NET_RECONCILE_WINDOW_SECS};
    let dt = time.delta_secs();
    let (Some(my_id), Some(planet)) = (mine.0, planet) else { return };
    let Ok(mut p) = q.single_mut() else { return };
    if dt <= 0.0 {
        return;
    }
    let now = time.elapsed_secs();
    path.push_back((now, p.dir));
    while path.front().is_some_and(|(t, _)| now - *t > NET_RECONCILE_WINDOW_SECS) {
        path.pop_front();
    }
    let Some((_, host)) = server_copy.iter().find(|(pid, _)| pid.0 == my_id) else { return };
    let Some(nearest) = path
        .iter()
        .map(|(_, d)| *d)
        .max_by(|a, b| a.dot(host.dir).total_cmp(&b.dot(host.dir)))
    else {
        return;
    };
    let err = crate::sphere::arc_dist(nearest, host.dir, planet.radius);
    if err <= NET_RECONCILE_DEADZONE {
        return;
    }
    // The offset from our path to the copy, re-applied where we stand now.
    let target = (Quat::from_rotation_arc(nearest, host.dir) * p.dir).normalize();
    let share = (err - NET_RECONCILE_DEADZONE) / err * (1.0 - (-NET_RECONCILE_RATE * dt).exp());
    p.dir = crate::sphere::step_toward(p.dir, target, p.dir.angle_between(target) * share);
}

/// CLIENT: push our build up. Rate-limited rather than on-change because the sheet is
/// small and a heartbeat is what makes a dropped update self-heal.
fn send_player_build(
    q: Query<&crate::run::PlayerState, With<crate::player::LocalPlayer>>,
    mut out: MessageWriter<PlayerBuildMsg>,
) {
    let Ok(ps) = q.single() else { return };
    out.write(PlayerBuildMsg {
        character: ps.character,
        level: ps.level,
        stats: ps.stats.clone(),
        weapons: ps.weapons.iter().map(|w| (w.kind, w.level)).collect(),
    });
}

/// HOST: adopt a peer's build onto our authoritative copy.
///
/// Applies ONLY build fields. Never hp, shield, iframes, powerups or `dead`: those are
/// host-authoritative live state, and a 2 Hz full-sheet overwrite would undo damage the host
/// had just applied — the player would appear to heal every time a heartbeat landed.
fn apply_player_build(
    mut msgs: MessageReader<FromClient<PlayerBuildMsg>>,
    mut slots: ResMut<PeerSlots>,
    mut q: Query<(&crate::player::PlayerId, &mut crate::run::PlayerState)>,
) {
    for FromClient { client_id, message } in msgs.read() {
        let Some(client) = client_id.entity() else { continue };
        let Some(pid) = slots.player_id(client) else { continue };
        // Remember the hero per slot: a stage change re-embodies peers, and they must come
        // back as themselves. The astronaut's rig follows `ps.character` via
        // `refit_astronaut_rigs`, and every client follows via the replicated NetHero.
        slots.heroes.insert(pid, message.character);
        for (id, mut ps) in &mut q {
            if id.0 != pid {
                continue;
            }
            ps.character = message.character;
            ps.level = message.level;
            // A grown max_hp must ADD to current hp, not silently look like damage: the
            // client's own recompute_stats preserves hp as a FRACTION of max_hp, so a plain
            // assignment here would make a peer appear to have taken a hit every time they
            // picked a health upgrade. Host hp is authoritative, so carry the delta.
            let d = message.stats.max_hp - ps.stats.max_hp;
            ps.stats = message.stats.clone();
            if d > 0.0 {
                ps.hp += d;
            }
            ps.hp = ps.hp.min(ps.stats.max_hp);
            // NOTE: the host's copy of `items` is deliberately left stale and no longer
            // matches `stats`. That is intended — nothing in the host's combat path reads
            // items, and recompute_stats is CLIENT-ONLY for a peer sheet (it would fold in
            // the HOST's meta tomes and save, producing numbers the peer never had).
            // Keep each weapon's existing cooldown so the heartbeat does not reset firing
            // cadence; only levels and membership come from the client.
            let mut next: Vec<crate::run::WeaponInstance> = Vec::new();
            for (kind, level) in &message.weapons {
                let cd = ps.weapons.iter().find(|w| w.kind == *kind).map(|w| w.cd).unwrap_or(0.0);
                next.push(crate::run::WeaponInstance { kind: *kind, level: *level, cd });
            }
            ps.weapons = next;
        }
    }
}

/// HOST: turn simulation grants into addressed wire messages.
fn relay_grants(
    mut inbox: MessageReader<GrantOut>,
    slots: Res<PeerSlots>,
    mut xp: MessageWriter<ToClients<XpGrantMsg>>,
    mut loot: MessageWriter<ToClients<LootGrantMsg>>,
) {
    use crate::pickups::PickupKind;
    use crate::run::PowerupKind;
    for g in inbox.read() {
        match *g {
            GrantOut::Xp(v) => {
                xp.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: XpGrantMsg(v) });
            }
            GrantOut::Loot(pid, kind) => {
                let Some(client) = slots.client_for(pid) else { continue };
                let msg = match kind {
                    PickupKind::Gold(g) => LootGrantMsg { gold: g, heal: 0.0, powerup: 0 },
                    PickupKind::Food => LootGrantMsg { gold: 0, heal: -1.0, powerup: 0 },
                    PickupKind::Powerup(k) => LootGrantMsg {
                        gold: 0,
                        heal: 0.0,
                        powerup: match k {
                            PowerupKind::Damage2x => 1,
                            PowerupKind::Magnet => 2,
                            PowerupKind::Speed => 3,
                        },
                    },
                    // Silver is banked run-globally and rides RunSnapMsg; XP has its own lane.
                    _ => continue,
                };
                loot.write(ToClients { targets: SendTargets::Single(ClientId::Client(client)), message: msg });
            }
        }
    }
}

/// CLIENT: shared XP lands on OUR PlayerState, so our own level-up panel fires locally.
fn apply_xp_grant(
    mut msgs: MessageReader<XpGrantMsg>,
    mut q: Query<&mut crate::run::PlayerState, With<crate::player::LocalPlayer>>,
) {
    let Ok(mut ps) = q.single_mut() else { return };
    for m in msgs.read() {
        if !ps.dead {
            ps.gain_xp(m.0);
        }
    }
}

/// CLIENT: our own loot.
fn apply_loot_grant(
    mut msgs: MessageReader<LootGrantMsg>,
    mut q: Query<&mut crate::run::PlayerState, With<crate::player::LocalPlayer>>,
) {
    use crate::run::PowerupKind;
    let Ok(mut ps) = q.single_mut() else { return };
    for m in msgs.read() {
        ps.gold += m.gold;
        if m.heal < 0.0 {
            let heal = ps.stats.max_hp * 0.2;
            ps.hp = (ps.hp + heal).min(ps.stats.max_hp);
        }
        let k = match m.powerup {
            1 => Some((PowerupKind::Damage2x, 20.0)),
            2 => Some((PowerupKind::Magnet, 12.0)),
            3 => Some((PowerupKind::Speed, 15.0)),
            _ => None,
        };
        if let Some((pk, secs)) = k {
            ps.powerups.retain(|(x, _)| *x != pk);
            ps.powerups.push((pk, secs));
        }
    }
}

/// CLIENT. Latch our identity. Logged only on change, since the host repeats the message.
fn receive_player_id(mut msgs: MessageReader<AssignPlayerId>, mut mine: ResMut<MyPlayerId>) {
    for m in msgs.read() {
        if mine.0 != Some(m.0) {
            info!("NET assigned PlayerId {}", m.0);
            crate::playlog::line(format!("NET assigned PlayerId {}", m.0));
            mine.0 = Some(m.0);
        }
    }
}

/// CLIENT -> HOST. We send intent every frame rather than on-change: it's a handful of
/// bytes on an unreliable channel, and a dropped "I'm still holding W" packet would
/// otherwise read as a stutter-stop on the host.
fn send_local_input(
    q: Query<&InputIntent, With<LocalPlayer>>,
    phase: Res<crate::run::RunPhase>,
    mut out: MessageWriter<PlayerInputMsg>,
    time: Res<Time>,
    dbg: Res<NetDebug>,
    mut next: Local<f32>,
    mut sent: Local<u32>,
) {
    let Ok(intent) = q.single() else { return };
    // While a panel is up, `gather_local_input` stops running but this does not — so we
    // would re-send the last intent forever, and the host ASSIGNS wish. A joiner who was
    // holding W when their level-up opened kept sprinting on the host. Send a neutral
    // intent instead of the stale one; this also stops the interact bit latching.
    if *phase != crate::run::RunPhase::Playing {
        out.write(PlayerInputMsg {
            wish: Vec3::ZERO,
            forward: intent.forward,
            jump: false,
            slide: false,
            interact: false,
        });
        return;
    }
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
    q_inter: Query<&Transform, Or<(With<crate::interact::Interactable>, With<crate::interact::ChargeShrine>)>>,
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
    // Interactables are drawn from the same seeded stream as the props, but from a
    // DIFFERENT code path, so they can desync independently (the cage's conditional draw did
    // exactly that). Checksumming them separately is what makes "we are standing in the same
    // ring" checkable instead of assumed.
    let inter_sum: i64 = q_inter
        .iter()
        .map(|t| (t.translation.x as f64 * 1e3) as i64 + (t.translation.z as f64 * 1e3) as i64)
        .sum();
    info!(
        "NET[{:?}] seed={} stage={} props={} layout_sum={} inter={} inter_sum={} timer={:.1} kills={}",
        *role,
        run.run_seed,
        run.stage,
        n_props,
        layout,
        q_inter.iter().count(),
        inter_sum,
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
#[allow(clippy::too_many_arguments)]
fn seat_joining_players(
    mut commands: Commands,
    mut slots: ResMut<PeerSlots>,
    // NOT `Added<ConnectedClient>`: a client can connect while the host is still in the
    // menu, and a one-shot Added event would be consumed and never retried. Reconciling
    // against the live set instead means they get seated whenever the run actually starts.
    joined: Query<(Entity, Option<&NetworkId>), With<ConnectedClient>>,
    // Each joiner's connect token, which names the hero it picked (`join_user_data`).
    transport: Option<Res<NetcodeServerTransport>>,
    planet: Option<Res<crate::planet::CurrentPlanet>>,
    run: Option<Res<crate::run::RunState>>,
    save: Option<Res<crate::save::MetaSave>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<&PlayerId>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
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

    for (client, network_id) in &joined {
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
        // Seated as the hero its connect token names, so no machine ever draws (and the
        // host never simulates) this peer as anyone else.
        let token_hero = network_id
            .zip(transport.as_deref())
            .and_then(|(nid, t)| t.user_data(nid.get()))
            .and_then(|data| hero_from_user_data(&data));
        if let Some(hero) = token_hero {
            slots.heroes.insert(id, hero);
        }
        let hero = slots.hero_for(id, run.character);
        crate::player::spawn_player(
            &mut commands,
            &mut meshes,
            &mut materials,
            &planet,
            &run,
            &save,
            id,
            hero,
            false, // remote: no LocalPlayer marker, no camera, driven by their input
            None,
        );
        info!("NET seated client {client} as player {id} ({})", hero.def().name);
        crate::playlog::line(format!("NET seated client {client} as player {id} ({})", hero.def().name));
        banners.write(crate::messages::BannerMsg(format!("PLAYER {} JOINED", id + 1)));
    }

    // Re-embody peers whose astronaut was reaped: by a stage change, or by the end of the
    // last run (the session outlives it, and the squad drops into the next one together).
    for (id, client) in respawn {
        let hero = slots.hero_for(id, run.character);
        crate::player::spawn_player(
            &mut commands,
            &mut meshes,
            &mut materials,
            &planet,
            &run,
            &save,
            id,
            hero,
            false,
            None,
        );
        info!("NET re-seated client {client} as player {id} ({}) in the new world", hero.def().name);
    }
}

/// HOST. Someone dropped: free their slot and remove their astronaut. The run itself
/// carries on — one player leaving must never end it for the rest.
fn unseat_leaving_players(
    mut commands: Commands,
    mut slots: ResMut<PeerSlots>,
    mut residency: ResMut<crate::netenemy::ClientResidency>,
    connected: Query<Entity, With<ConnectedClient>>,
    astronauts: Query<(Entity, &PlayerId), With<crate::player::Player>>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
) {
    let live: Vec<Entity> = connected.iter().collect();
    let gone: Vec<Entity> = slots
        .map
        .keys()
        .copied()
        .filter(|c| !live.contains(c))
        .collect();
    for client in gone {
        // Their crowd residency too: a later joiner can be handed the same client entity
        // index, and would otherwise inherit a set of "already sent" enemies it never got.
        residency.forget(client);
        let Some(id) = slots.release(client) else { continue };
        for (e, pid) in &astronauts {
            if pid.0 == id {
                commands.entity(e).despawn();
            }
        }
        info!("NET client {client} left — freed player {id}");
        crate::playlog::line(format!("NET client {client} left — freed player {id}"));
        banners.write(crate::messages::BannerMsg(format!("PLAYER {} LEFT", id + 1)));
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
            crate::playlog::line(format!("NET client state -> {now:?}"));
            *last_client = Some(now);
        }
    }
    if role.simulates() {
        let peers = clients.iter().count();
        if peers != *last_peers {
            let running = server_state.map(|s| *s.get() == ServerState::Running).unwrap_or(false);
            info!("NET peers connected: {peers} (server running: {running})");
            crate::playlog::line(format!("NET peers connected: {peers}"));
            *last_peers = peers;
        }
    }
}

/// This machine's address on the local network, for the "tell your friend what to type"
/// line. Uses the standard UDP trick: connecting a datagram socket sends NOTHING, it just
/// makes the OS pick the interface it would route through, which is exactly the address a
/// machine on the same LAN needs. Falls back to loopback if there is no route.
pub fn local_ip() -> String {
    let probe = |target: &str| -> Option<String> {
        let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
        sock.connect(target).ok()?;
        Some(sock.local_addr().ok()?.ip().to_string())
    };
    probe("8.8.8.8:80")
        .or_else(|| probe("192.168.1.1:80"))
        .unwrap_or_else(|| "127.0.0.1".into())
}

/// The menu's "you are hosting" line: the address a machine on the SAME NETWORK must type.
/// Without it the other player has to go find ipconfig.
pub fn hosting_note(port: u16) -> String {
    format!("HOSTING: tell the other player to join  {}   (port {port})   HOST CO-OP again stops", local_ip())
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
    commands.insert_resource(HostPort(port));
    commands.insert_resource(NetRole::Host);
    info!("hosting on port {port}");
    crate::playlog::line(format!("NET hosting on port {port}"));
    Ok(())
}

/// Join a host at `addr:port`, as `hero` — the connect token tells the host who to seat.
pub fn start_join(
    commands: &mut Commands,
    channels: &RepliconChannels,
    addr: IpAddr,
    port: u16,
    hero: crate::content::characters::AstronautKind,
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
            user_data: Some(join_user_data(hero)),
        },
        socket,
    )
    .map_err(|e| e.to_string())?;

    commands.insert_resource(client);
    commands.insert_resource(transport);
    commands.insert_resource(JoinTarget(SocketAddr::new(addr, port)));
    commands.insert_resource(NetRole::Client);
    info!("joining {addr}:{port} as {}", hero.def().name);
    crate::playlog::line(format!("NET joining {addr}:{port} as {}", hero.def().name));
    Ok(())
}

/// Tear the connection down and go back to solo. Drops the transports (freeing the port
/// for the next HOST CO-OP); `reset_after_session` then clears everything the session left
/// behind, whichever path got us here. Send any goodbye packets BEFORE calling this.
pub fn disconnect(commands: &mut Commands) {
    commands.remove_resource::<RenetServer>();
    commands.remove_resource::<NetcodeServerTransport>();
    commands.remove_resource::<RenetClient>();
    commands.remove_resource::<NetcodeClientTransport>();
    commands.remove_resource::<JoinTarget>();
    commands.remove_resource::<HostPort>();
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
        // The same `--hero` that `boot` builds this instance's RunState from.
        let hero = args
            .iter()
            .position(|a| a == "--hero")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| crate::content::characters::AstronautKind::from_name(s))
            .unwrap_or(crate::content::characters::AstronautKind::Buzz);
        if let Err(e) = start_join(&mut commands, &channels, ip, port, hero) {
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
        nt.sliding = p.slide_timer > 0.0;
    }
}

/// Host: keep each astronaut's replicated hero in step with its sheet. Compared before
/// writing, because replicon sends a component whenever it is touched and this one only
/// changes once, when a peer's first build heartbeat lands.
pub fn push_net_hero(mut q: Query<(&crate::run::PlayerState, &mut NetHero)>) {
    for (ps, mut hero) in &mut q {
        let code = hero_code(ps.character);
        if hero.0 != code {
            hero.0 = code;
        }
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
        v.revives = ps.revives.min(u8::MAX as u32) as u8;
    }
}

// ─── session lifecycle ────────────────────────────────────────────────────────
//
// A co-op session outlives its runs: the squad is the session, a run is one trip out. When
// the host's run ends, every joiner goes back to the main menu STILL CONNECTED and follows
// the host into its next run from there, exactly as it followed it into the first.
//
// What keeps a joiner out of a stale world between runs is `RunSync`, not the connection:
//   * a run's end resets it (`receive_run_over`), so the joiner waits for a new seed;
//   * every snapshot carries its run generation, so a straggler from the ended run (the
//     snapshot lane is unordered) is dropped instead of adopted;
//   * `built_for` compares the world actually standing with the one the host describes, so
//     any mismatch at all (next stage, next run) rebuilds instead of silently diverging.
//
// Every way OUT of a session funnels into two places: `watch_client_connection` (a client
// going back to the menu, with a readable reason) and `reset_after_session` (whatever the
// session left behind).

/// HOST: a run starts. Numbered so joiners can tell it from the last one.
fn count_run(mut session: ResMut<SessionState>) {
    session.run_gen += 1;
}

/// Every machine: the world is gone once the run is left, so nothing may ask to rebuild
/// it — a rebuild request in the menu would raise a planet under the menu.
fn forget_built_world(mut sync: ResMut<RunSync>) {
    sync.world_built = false;
    sync.built_for = None;
    sync.pending_stage = None;
}

/// HOST: the run is over — victory, a wipe, or ABANDON. Tell every joiner why; they go back
/// to the menu and wait there, still connected, while we look at the results and pick the
/// next run.
fn announce_run_over(
    run: Res<crate::run::RunState>,
    session: Res<SessionState>,
    port: Option<Res<HostPort>>,
    mut residency: ResMut<crate::netenemy::ClientResidency>,
    mut out: MessageWriter<ToClients<RunOverMsg>>,
    mut note: ResMut<crate::ui::menus::CoopNote>,
) {
    let reason = match run.result {
        Some(crate::run::RunResult::Victory) => RUN_OVER_VICTORY,
        Some(crate::run::RunResult::Abandoned) => RUN_OVER_ABANDONED,
        _ => RUN_OVER_WIPED,
    };
    out.write(ToClients {
        targets: SendTargets::CLIENTS_ONLY,
        message: RunOverMsg { run_gen: session.run_gen, reason },
    });
    // Every joiner drops its streamed horde with its world; the next run's crowd stream
    // starts from nothing instead of diffing against enemies that no longer exist.
    residency.clear();
    // Seen on our main menu once the results are dismissed.
    note.0 = format!(
        "YOUR SQUAD IS WAITING: LAUNCH or DAILY takes everyone into the next run.\n{}",
        hosting_note(port.map(|p| p.0).unwrap_or(DEFAULT_PORT))
    );
    info!("NET run {} over (reason {reason}); the session stays open", session.run_gen);
    crate::playlog::line(format!("NET run {} over (reason {reason}); the session stays open", session.run_gen));
}

/// CLIENT: the host's run ended. Back to the menu, still connected, to wait for its next
/// one — which `client_follow_host_run` enters like the first, from a fresh snapshot.
fn receive_run_over(
    mut msgs: MessageReader<RunOverMsg>,
    mut sync: ResMut<RunSync>,
    mut note: ResMut<crate::ui::menus::CoopNote>,
    mut phase: ResMut<crate::run::RunPhase>,
    state: Res<State<crate::AppState>>,
    mut next: ResMut<NextState<crate::AppState>>,
) {
    let Some(m) = msgs.read().last().copied() else { return };
    if m.run_gen < sync.min_gen {
        return; // we are already in a newer run
    }
    let why = match m.reason {
        RUN_OVER_VICTORY => "PLANET SAVED!",
        RUN_OVER_ABANDONED => "the host abandoned the run.",
        _ => "the squad got BONKED.",
    };
    note.0 = format!("RUN OVER: {why}\nWaiting for the host's next run...   JOIN CO-OP again leaves the session");
    // Forget the run outright: nothing of it may seed the next world.
    *sync = RunSync { min_gen: m.run_gen + 1, ..default() };
    // A panel or the pause menu may be up; the menu must not inherit a stopped clock.
    *phase = crate::run::RunPhase::Playing;
    if *state.get() == crate::AppState::InRun {
        next.set(crate::AppState::MainMenu);
    }
    info!("NET host's run {} is over; waiting for the next", m.run_gen);
    crate::playlog::line(format!("NET host's run {} is over; waiting for the next", m.run_gen));
}

/// The pause menu's LEAVE / END SESSION.
///   * CLIENT: say goodbye (netcode's disconnect packet goes out instantly) and let
///     `watch_client_connection` take us to the menu. The host plays on without us.
///   * HOST: end it for everyone, and keep playing this run solo — ABANDON RUN is the button
///     for "I want out of the run as well".
fn handle_leave_session(
    mut msgs: MessageReader<LeaveSession>,
    role: Res<NetRole>,
    mut session: ResMut<SessionState>,
    client: Option<ResMut<NetcodeClientTransport>>,
    mut out: MessageWriter<ToClients<SessionEndMsg>>,
    mut phase: ResMut<crate::run::RunPhase>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
    state: Res<State<crate::AppState>>,
    mine: Res<MyPlayerId>,
) {
    if msgs.read().count() == 0 {
        return;
    }
    match *role {
        NetRole::Client => {
            // From the menu this is JOIN CO-OP pressed again: while still waiting to get in
            // (nobody seated us yet), or in the lobby between two of the host's runs.
            let seated = *state.get() == crate::AppState::InRun || mine.0.is_some();
            session.end_note =
                Some(if seated { "You left the session." } else { "Join cancelled." }.into());
            if let Some(mut transport) = client {
                transport.disconnect();
            }
        }
        NetRole::Host => {
            if session.closing.is_some() {
                return;
            }
            out.write(ToClients {
                targets: SendTargets::CLIENTS_ONLY,
                message: SessionEndMsg { reason: SESSION_END_BY_HOST },
            });
            session.closing = Some(SessionClose::start());
            // Close the pause menu: the host's run carries on, now solo.
            *phase = crate::run::RunPhase::Playing;
            banners.write(crate::messages::BannerMsg("SESSION ENDED: PLAYING ON SOLO".into()));
            info!("NET host ended the session");
            crate::playlog::line("NET host ended the session".to_string());
        }
        NetRole::Solo => {}
    }
}

/// HOST: once the goodbye has had time to flush, drop every client and the socket.
/// Real time, not virtual: END SESSION is pressed from the pause menu, where virtual time
/// is stopped, and a countdown on it would never finish.
fn finish_session_close(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut session: ResMut<SessionState>,
    server: Option<ResMut<RenetServer>>,
    transport: Option<ResMut<NetcodeServerTransport>>,
    mut note: ResMut<crate::ui::menus::CoopNote>,
) {
    let Some(left) = session.closing.as_mut() else { return };
    left.secs -= time.delta_secs();
    left.frames = left.frames.saturating_sub(1);
    if left.secs > 0.0 || left.frames > 0 {
        return;
    }
    session.closing = None;
    // Explicit disconnect packets: without them every joiner sits out netcode's timeout.
    if let (Some(mut server), Some(mut transport)) = (server, transport) {
        transport.disconnect_all(&mut server);
    }
    disconnect(&mut commands);
    // Replace the "HOSTING — tell the other player…" line, which is no longer true.
    note.0 = "Co-op session over. HOST CO-OP to open a new one.".into();
    info!("NET session closed");
}

/// CLIENT: the host told us the session is over. Leave straight away — our goodbye is
/// instant — rather than wait for the host's own teardown half a second later.
fn receive_session_end(
    mut msgs: MessageReader<SessionEndMsg>,
    mut session: ResMut<SessionState>,
    transport: Option<ResMut<NetcodeClientTransport>>,
) {
    // SESSION_END_BY_HOST is the only reason code today: a finished run no longer ends the
    // session (that is RunOverMsg), only the host closing co-op does.
    if msgs.read().count() == 0 {
        return;
    }
    session.end_note = Some("The host ended the session.".into());
    if let Some(mut transport) = transport {
        transport.disconnect();
    }
}

/// CLIENT: replicon's own handshake found the host registers a different protocol (same
/// PROTOCOL_ID, different build). The host disconnects us right after; this is the reason.
fn on_protocol_mismatch(_: On<ProtocolMismatch>, mut session: ResMut<SessionState>) {
    session.end_note =
        Some("The host runs a different ASTROBONK build. Both players need the same version.".into());
}

/// CLIENT: notice the connection is gone — however it went — and go back to the menu
/// with a line that says why. This is also where a failed JOIN reports: a wrong address
/// and a version mismatch look identical on the wire (the handshake never answers), so
/// the line names both.
#[allow(clippy::too_many_arguments)]
fn watch_client_connection(
    mut commands: Commands,
    client: Option<Res<RenetClient>>,
    transport: Option<ResMut<NetcodeClientTransport>>,
    target: Option<Res<JoinTarget>>,
    mut session: ResMut<SessionState>,
    mut note: ResMut<crate::ui::menus::CoopNote>,
    state: Res<State<crate::AppState>>,
    mut next: ResMut<NextState<crate::AppState>>,
    mut phase: ResMut<crate::run::RunPhase>,
) {
    use bevy_replicon_renet::netcode::NetcodeDisconnectReason as Why;
    let (Some(client), Some(mut transport)) = (client, transport) else { return };
    // A host that crashed or lost its network sends nothing at all, not even keep-alives.
    if client.is_connected()
        && transport.time_since_last_received_packet().as_secs_f32() > crate::config::NET_HOST_SILENCE_SECS
    {
        session.end_note.get_or_insert_with(|| "Lost connection to the host.".into());
        transport.disconnect();
    }
    let why = transport.disconnect_reason();
    if why.is_none() && !client.is_disconnected() {
        return;
    }
    let addr = target.map(|t| t.0.to_string()).unwrap_or_else(|| "the host".into());
    let text = session.end_note.take().unwrap_or_else(|| match why {
        Some(Why::ConnectionRequestTimedOut | Why::ConnectionResponseTimedOut | Why::ConnectTokenExpired) => {
            format!(
                "No answer from {addr}.\nNobody is hosting there, or the host runs a different \
                 ASTROBONK version (this build: protocol {PROTOCOL_ID:X})."
            )
        }
        Some(Why::ConnectionDenied) => format!("{addr} refused the join: the lobby is full ({MAX_PLAYERS} players)."),
        Some(Why::DisconnectedByServer) => "The host ended the session.".into(),
        Some(Why::ConnectionTimedOut) => "Lost connection to the host.".into(),
        _ => "Disconnected from the host.".into(),
    });
    info!("NET session over: {text}");
    crate::playlog::line(format!("NET session over: {text}"));
    note.0 = text;
    disconnect(&mut commands);
    // A panel or the pause menu may be up; the menu must not inherit a stopped clock.
    *phase = crate::run::RunPhase::Playing;
    if *state.get() != crate::AppState::MainMenu {
        next.set(crate::AppState::MainMenu);
    }
}

/// Whatever path ended the session (leave, host closed, host vanished, run over), this
/// runs on the frame the role flips back to Solo and clears what it left behind — so the
/// next HOST CO-OP or JOIN starts clean instead of inheriting a stale seed or slot table.
#[allow(clippy::too_many_arguments)]
fn reset_after_session(
    mut commands: Commands,
    mut mine: ResMut<MyPlayerId>,
    mut sync: ResMut<RunSync>,
    mut slots: ResMut<PeerSlots>,
    mut residency: ResMut<crate::netenemy::ClientResidency>,
    mut session: ResMut<SessionState>,
    // CLIENT: teammates' replicated entities. replicon only forgets its entity map on
    // disconnect; it never despawns, so without this a rejoin draws the old squad as ghosts.
    remote: Query<Entity, With<Remote>>,
    // HOST playing on solo: the peers' astronauts would otherwise stand there, still firing.
    peers: Query<Entity, (With<crate::player::Player>, Without<LocalPlayer>)>,
) {
    mine.0 = None;
    *sync = RunSync::default();
    *slots = PeerSlots::default();
    residency.clear();
    session.closing = None;
    for e in remote.iter().chain(peers.iter()) {
        commands.entity(e).try_despawn();
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
// 2d. Teammates wear their own hero (NetHero, re-suited live) and tuck on slides
//    (NetTransform.sliding) — DONE. The host seats a joiner as its hero from the first
//    frame: the hero rides the netcode connect token's user data (`join_user_data`), the
//    only thing the host holds before any client message can arrive.
//    Still open: cache the rig's meshes/materials and drop
//    shadow-casting on remote flashlights (each rig allocates 8 meshes, 5 materials and a
//    shadow-casting spotlight).
//
// 2e. CLIENT PARITY — DONE: every astronaut's Comet Combo (NetComet), Mars's dust storm
//    (RunSnapMsg + per-astronaut InStorm on the host), Anubot's verdict beam (BossRec
//    angle/state -> the shared anubot_beam_visuals) and Beamer aim lines (hazard lane,
//    locked-target form). Session lifecycle: see "session lifecycle" above.
//    Pattern for the next one: a host-only SIM system plus an ungated VISUALS system both
//    machines run on the replicated/streamed state (anubot_beam_visuals, aim_line_visuals,
//    dust_storm_visuals, comet_presentation). Repro:
//        headless: --coop2 --comet-peer | --coop2 --peer-hero valentina |
//                  --fast-boss --coop2 --planet mars --storm-peer
//        windowed: host  --host --autodrop --botinput --autopick --netlog --planet mars --bossnow
//                  client --join 127.0.0.1 --autodrop --botinput --autopick --netlog --hero valentina
//    and compare the two sides' NETPARITY lines.
//    §13 (P04): the host's assists ride RunSnapMsg (joiner HUD tag + token), a revive
//    rides PlayerVitals.revives, and Burrower crack decals ride the hazard lane
//    (HazardEvent::Crack). Accessibility settings are per-machine presentation and never
//    cross the wire. Repro: coop.sh with `--dev --assist` on both instances.
//    Client prediction now reconciles softly against the host's copy of us
//    (`reconcile_own_astronaut`); a proper rewind-and-replay of unacknowledged inputs is
//    still open, and is only worth it once latency beyond a LAN matters.
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
