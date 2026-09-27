//! Central tuning constants. Change numbers here, not in systems.

pub const PLAYER_RUN_SPEED: f32 = 8.5;
pub const PLAYER_ACCEL: f32 = 55.0;
pub const PLAYER_FRICTION: f32 = 38.0;
pub const PLAYER_AIR_CONTROL: f32 = 0.35;
pub const PLAYER_JUMP_VEL: f32 = 8.0;
pub const PLAYER_GRAVITY: f32 = 22.0;
pub const PLAYER_HEIGHT: f32 = 1.7;
pub const PLAYER_RADIUS: f32 = 0.45;

pub const SLIDE_BOOST: f32 = 1.65; // multiplier over run speed
pub const SLIDE_TIME: f32 = 0.85;
pub const SLIDE_COOLDOWN: f32 = 1.1;
pub const BHOP_WINDOW: f32 = 0.16; // seconds after landing to keep slide speed
pub const SPEED_HARD_CAP: f32 = 2.1; // × run speed, bhop chains can't exceed

pub const CAM_DISTANCE: f32 = 7.5;
pub const CAM_HEIGHT: f32 = 3.2;
pub const CAM_STIFFNESS: f32 = 14.0;

/// How hard a co-op teammate's drawn pose chases the last replicated snapshot. Raise it and
/// motion becomes stuttery in lockstep with the packet rate; lower it and teammates lag.
pub const REMOTE_SMOOTH_RATE: f32 = 14.0;
/// Arc metres of error past which we teleport a teammate instead of easing (stage changes,
/// teleporter use — easing across half a planet would look like a ghost gliding through it).
pub const REMOTE_SNAP_ARC: f32 = 8.0;
pub const CAM_SENS: f32 = 0.0032;

pub const ENEMY_CAP: usize = 1200;
pub const ENEMY_SEPARATION_CELL: f32 = 2.2;
pub const SPAWN_ARC_MIN: f32 = 42.0; // meters beyond the player (over the horizon)
pub const SPAWN_ARC_MAX: f32 = 58.0;
pub const CONTACT_TICK: f32 = 0.5; // seconds between contact hits from one enemy
/// A Beamer's telegraph: how long it paints the aim line, and the final stretch where the
/// line stops tracking and holds (the dodge window). A co-op client derives the same sweep
/// from these, so they are the one copy of the numbers.
pub const BEAMER_CHARGE_SECS: f32 = 1.1;
pub const BEAMER_LOCK_SECS: f32 = 0.25;

pub const GEM_CAP: usize = 550;
pub const PICKUP_BASE_RANGE: f32 = 3.2;
pub const PICKUP_FLY_SPEED: f32 = 26.0;

pub const STAGE_SECONDS: [f32; 3] = [600.0, 540.0, 480.0];
pub const MINIBOSS_MARKS: [f32; 2] = [420.0, 120.0]; // timer values (counting down)
pub const BOSS_MARK: f32 = 90.0;

pub const XP_BASE: f32 = 6.0;
pub const XP_PER_LEVEL: f32 = 3.4;
pub const XP_QUAD: f32 = 0.18;

pub const CHEST_BASE_COST: u64 = 25;
pub const CHEST_COST_GROWTH: f32 = 1.75;

pub const WEAPON_SLOTS: usize = 4;
pub const MAX_WEAPON_LEVEL: u32 = 7;

pub const DAMAGE_NUMBER_POOL: usize = 64;

pub const INTERACT_RANGE: f32 = 3.0;

// Comet Combo — the signature scored lap-kill.
pub const WAKE_RADIUS: f32 = 14.0; // enemies this close (and behind you) join the tail
pub const COMET_MIN_TAIL: u32 = 8; // tail must reach this to start charging
pub const COMET_CHARGE_GOAL: f32 = 900.0; // charge = Σ(tail_count · speed · dt)
pub const COMET_RADIUS: f32 = 22.0; // cash-out detonation radius
pub const COMET_GRACE: f32 = 0.7; // seconds the tail can dip before the combo breaks

pub const SAVE_DIR: &str = "astrobonk";
pub const SAVE_FILE: &str = "save.json";

// ── Co-op enemy streaming ────────────────────────────────────────────────────
// A client cannot receive 1200+ enemies as replicated ENTITIES, so the crowd rides a
// custom quantized batch message. Positions are sent as a great-circle offset from the
// receiving client's OWN astronaut, in that astronaut's tangent frame, in arc metres —
// on a sphere a position is a direction, so two numbers beat three floats.

/// Half-width of the quantized position range, in arc metres. Must exceed the largest
/// interest radius. 16 bits over ±128 m gives a 3.9 mm step — the slowest enemy (Lobber,
/// 1.8 m/s) still advances ~31 steps per snapshot, so nothing stair-steps.
pub const NET_ENEMY_RANGE: f32 = 128.0;
/// Snapshot rate. Matches NET_ENEMY_SMOOTH_RATE's time constant, so the interpolation
/// filter is itself the reconstructor and 30 Hz would buy nothing visible.
pub const NET_ENEMY_HZ: f32 = 15.0;
/// Inside this arc, an enemy is sent EVERY snapshot. Comfortably covers the whole combat
/// volume (auto-target 40 m, longest weapon 26 m, director spawn band 42–58 m).
pub const NET_ENEMY_NEAR_ARC: f32 = 50.0;
/// Enters interest at IN, leaves at OUT — the gap is hysteresis so an enemy pacing the
/// boundary doesn't spawn/despawn every snapshot. Indexed like PlanetKind::ALL.
pub const NET_ENEMY_INTEREST_IN: [f32; 3] = [95.0, 110.0, 82.0];
pub const NET_ENEMY_INTEREST_OUT: [f32; 3] = [107.0, 122.0, 93.0];
/// Hard ceiling on records per snapshot — the bandwidth backstop.
pub const NET_ENEMY_MAX_RECORDS: usize = 1200;
/// Payload bytes per chunk. Under renet's SLICE_SIZE (1200) so a chunk is never
/// auto-fragmented, which would make it all-or-nothing on an unreliable channel.
pub const NET_ENEMY_CHUNK_BYTES: usize = 1024;
/// How hard a proxy chases its last streamed position (framerate-independent).
pub const NET_ENEMY_SMOOTH_RATE: f32 = 12.0;
/// Arc error past which a proxy teleports instead of easing.
pub const NET_ENEMY_SNAP_ARC: f32 = 12.0;
/// Seconds an unseen proxy survives before despawning — long enough that a dropped
/// unreliable packet doesn't flicker the horde.
pub const NET_ENEMY_GRACE: f32 = 2.0;

// ── Co-op session lifecycle ──────────────────────────────────────────────────
/// Real seconds between the host sending SessionEndMsg and dropping the transport. Disconnect
/// packets go out instantly, so without this gap they overtake the reliable goodbye and a
/// joiner only ever learns "connection lost", never why.
pub const NET_SESSION_CLOSE_SECS: f32 = 0.5;
/// A connected client that has heard NOTHING from the host for this long (not even renet's
/// keep-alives, which flow while the host is paused or on a menu) treats it as gone. netcode's
/// own timeout is 15 s — a long time to stare at a frozen planet.
pub const NET_HOST_SILENCE_SECS: f32 = 6.0;
/// CLIENT reconciliation: how far the host's authoritative copy of our astronaut may sit
/// off the path we ran before we pull ourselves toward it. Measured against our recent PATH,
/// not our current position — the copy trails us by speed × latency (metres at a low frame
/// rate or mid-bhop), and correcting that lag would drag every joiner backwards. What it
/// catches is real divergence: a host hitstop or open panel that froze our server-side body
/// while we ran on, a lost jump packet.
pub const NET_RECONCILE_DEADZONE: f32 = 1.0;
/// How much of our own path to remember for that comparison. Must exceed the copy's lag.
pub const NET_RECONCILE_WINDOW_SECS: f32 = 1.0;
/// How fast the excess beyond the deadzone is closed, per second (exponential, so a big
/// error eases home in about a second instead of snapping the camera).
pub const NET_RECONCILE_RATE: f32 = 2.5;
