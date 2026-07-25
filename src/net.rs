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

use crate::player::PlayerId;
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
            .add_systems(Update, report_connection)
            .add_systems(
                Update,
                (push_net_transform, push_player_vitals).run_if(is_simulating),
            );
    }
}

fn is_simulating(role: Res<NetRole>) -> bool {
    role.simulates()
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
// 1. TRANSPORT WIRING: open a renet server socket on Host / connect on Client.
//    Replicon is transport-agnostic; bevy_replicon_renet provides the sockets but the
//    host/join flow (address entry, lobby) is still ours to build.
//
// 2. INPUT ROUTING: on Client, stop running `player_input` locally against the sim and
//    instead send `PlayerInputMsg` each frame while predicting our own astronaut. On
//    Host, drain `FromClient<PlayerInputMsg>` and apply to the matching PlayerId.
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
