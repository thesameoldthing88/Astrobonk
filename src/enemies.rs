//! The horde: spawning director, great-circle steering with separation,
//! contact damage, special attackers (spitter / UFO / burrower), elites,
//! minibosses, stage bosses, and THE STATIC.

use crate::config::*;
use crate::content::enemies::{BossKind, EliteMods, EnemyKind};
use crate::content::palettes::Palette;
use crate::events_world::InStorm;
use crate::fx::{self, Pcolor, ParticleAssets, Shake};
use crate::messages::*;
use crate::planet::{random_dir, CurrentPlanet, StageScoped};
use crate::player::Player;
use crate::run::scaling::{self, Scaling};
use crate::run::RunState;
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub dir: Vec3,
    pub hover: f32,
    pub speed: f32,
    pub damage: f32,
    pub xp: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub elite: bool,
    pub contact_cd: f32,
    pub slow: f32,
    pub knock: Vec3,
    pub flash: f32,
    pub scale: f32,
    pub wobble: f32,
    /// Gait phase, advanced by DISTANCE travelled so the waddle matches real movement
    /// (code-art-animation skill, crowd tier — whole-transform animation only).
    pub stride: f32,
}

#[derive(Component)]
pub struct Boss {
    pub kind: BossKind,
    pub attack_timer: f32,
    pub burst_timer: f32,
    pub phase: u8, // 0 = P1 (>66% HP), 1 = P2 (33-66%), 2 = P3 (<33%)
}

/// Which stage mark a miniboss was summoned for (0 = the 7:00 spike, 1 = the 2:00 one).
/// Host-only: it decides the guaranteed chest (§3), which reaches clients via RunSnapMsg.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinibossSlot(pub u8);

/// The Craterpillar's head records the ground it has crossed so its body can
/// follow — a worm that literally laps the tiny planet.
#[derive(Component)]
pub struct CraterpillarHead {
    pub trail: std::collections::VecDeque<Vec3>, // recent head directions, front = newest
}

/// One body chunk following the head's trail at a fixed stride behind it.
#[derive(Component)]
pub struct CraterpillarSegment {
    pub head: Entity,
    pub idx: usize,
    pub damage: f32,
    pub scale: f32,
}

pub const WORM_SEGMENTS: usize = 12;
const WORM_STRIDE: usize = 2; // trail points between segments
const WORM_TRAIL_STEP: f32 = 0.55; // meters between recorded trail points

/// Judge Anubot's signature: a rotating "Verdict Beam" lighthouse sweep.
#[derive(Component)]
pub struct AnubotBeam {
    pub angle: f32,   // current sweep angle around the boss's up-axis
    pub state: u8,    // 0 idle, 1 charging (telegraph), 2 firing
    pub timer: f32,   // time left in the current state
}

impl Default for AnubotBeam {
    fn default() -> Self {
        Self { angle: 0.0, state: 0, timer: 2.5 }
    }
}

impl AnubotBeam {
    /// Sweep speed in rad/s: slow while charging (the telegraph), fast while firing, and
    /// faster each phase. Shared with the co-op client, which spins its copy of the beam
    /// between boss snapshots with exactly these numbers.
    pub fn spin(state: u8, phase: u8) -> f32 {
        let ph = phase as f32;
        match state {
            1 => 0.5,
            2 => 1.15 * (1.0 + 0.3 * ph),
            _ => 0.25,
        }
    }
    /// How long a state lasts once entered. The client restarts its local timer from this
    /// when a snapshot reports a new state, which is all the charge-up pose needs.
    pub fn state_secs(state: u8, phase: u8) -> f32 {
        let ph = phase as f32;
        match state {
            1 => 1.3 - 0.3 * ph,          // shorter telegraph as he enrages
            2 => 3.0 + 0.6 * ph,          // longer sweep
            _ => (2.6 - 0.7 * ph).max(0.8), // shorter rest
        }
    }
    /// World-space heading of the beam for a boss standing at `up`.
    pub fn heading(&self, up: Vec3) -> Vec3 {
        let base = sphere::tangent_frame(up).0;
        (Quat::from_axis_angle(up, self.angle) * base).normalize_or_zero()
    }
}

#[derive(Component)]
pub struct AnubotBeamVis {
    pub boss: Entity,
}

const BEAM_LENGTH: f32 = 34.0;
const BEAM_WIDTH: f32 = 2.4;

#[derive(Component)]
pub struct Spitter {
    pub cd: f32,
}

/// Long-range sniper: tracks the player with an aim line, then fires a railbolt.
#[derive(Component)]
pub struct Beamer {
    pub cd: f32,
    pub charging: f32, // >0 while painting the aim line
    pub aim: Vec3,     // locked tangent heading near the end of the charge
    /// Who this beamer committed to when the charge began. LATCHED on purpose: re-picking
    /// the nearest astronaut every frame during a 1.1s telegraph would make the aim line
    /// snap between players mid-sweep, turning the dodge tell into a lie.
    pub target: Option<Entity>,
}

/// The visible aim line while a Beamer charges: a row of dashes that MARCH toward the
/// target while the beamer tracks, then freeze and thicken once it locks (§13: the tell
/// reads by motion and shape, never by color alone).
#[derive(Component)]
pub struct AimLine {
    pub owner: Entity,
    /// Dash phase, 0..1 of one dash period. Stored (not derived from the clock) so a lock
    /// can freeze the pattern where it stands instead of snapping it.
    pub march: f32,
}

/// A Burrower's approach: a crack decal spreading across the ground over the spot it will
/// erupt from — the §13 "Burrower = cracking decal" tell. Host-spawned with the burrower,
/// streamed to joiners on the hazard lane, integrated by `crack_decals` everywhere.
#[derive(Component)]
pub struct CrackDecal {
    pub dir: Vec3,
    pub timer: f32,
    pub max: f32,
}

/// Presentation-only children that make a hazard readable without color: the sweep that
/// fills a telegraph toward impact, the inner safe ring of a slam, and the white outline
/// hull of high-contrast mode. Built by `decorate_hazards`; never simulated.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum HazardDecor {
    /// Grows from the center (disc) or the safe edge (ring) to the lethal edge at impact.
    Sweep,
    /// A slam's inner edge: inside it is safe.
    SafeRing,
    /// White inverted hull (high-contrast "danger = white outline").
    Outline,
}

/// Artillery: mortars the player's position with an AoE telegraph.
#[derive(Component)]
pub struct Lobber {
    pub cd: f32,
}

/// A mortar shell arcing between two surface points (visual; damage is the telegraph's).
#[derive(Component)]
pub struct MortarShell {
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub dur: f32,
}

#[derive(Component)]
pub struct Buried {
    pub timer: f32,
}

#[derive(Component)]
pub struct EnemyProjectile {
    pub dir: Vec3,
    pub heading: Vec3,
    pub speed: f32,
    pub damage: f32,
    pub life: f32,
    pub hover: f32,
}

/// Telegraph for boss slams / burrower emergence / mortar impacts.
/// `ring: true` hurts only near the edge band (dodge inward or out);
/// `ring: false` hurts the whole disc (get out entirely).
#[derive(Component)]
pub struct Telegraph {
    pub timer: f32,
    pub max: f32,
    pub radius: f32,
    pub damage: f32,
    pub dir: Vec3,
    pub ring: bool,
}

#[derive(Resource)]
pub struct EnemyAssets {
    pub meshes: HashMap<EnemyKind, Handle<Mesh>>,
    pub mats: HashMap<EnemyKind, Handle<StandardMaterial>>,
    pub elite_mat: Handle<StandardMaterial>,
    pub flash_mat: Handle<StandardMaterial>,
    pub boss_mat: Handle<StandardMaterial>,
    pub worm_head_mesh: Handle<Mesh>,
    pub worm_seg_mesh: Handle<Mesh>,
    pub worm_mat: Handle<StandardMaterial>,
    pub anubot_mesh: Handle<Mesh>,
    pub anubot_mat: Handle<StandardMaterial>,
    pub beam_mesh: Handle<Mesh>,
    pub beam_charge_mat: Handle<StandardMaterial>,
    pub beam_fire_mat: Handle<StandardMaterial>,
    pub proj_mesh: Handle<Mesh>,
    pub proj_mat: Handle<StandardMaterial>,
    pub ring_mesh: Handle<Mesh>,
    pub ring_mat: Handle<StandardMaterial>,
    /// See-through fill for the sweep disc inside a telegraph ring.
    pub ring_fill_mat: Handle<StandardMaterial>,
    pub disc_mesh: Handle<Mesh>,
    /// A Beamer's dashed aim line (and its outline hull).
    pub aim_mesh: Handle<Mesh>,
    pub aim_outline_mesh: Handle<Mesh>,
    /// The Burrower's crack decal (and its outline hull).
    pub crack_mesh: Handle<Mesh>,
    pub crack_outline_mesh: Handle<Mesh>,
    /// Outline hulls for the telegraph ring and enemy shots.
    pub ring_outline_mesh: Handle<Mesh>,
    pub proj_outline_mesh: Handle<Mesh>,
    /// Flat white, front faces culled: drawn slightly larger than a hazard, only its far
    /// side shows — a white rim around the shape (the inverted-hull outline).
    pub outline_mat: Handle<StandardMaterial>,
}

/// Original material to restore after a hit-flash.
#[derive(Component)]
pub struct BaseMat(pub Handle<StandardMaterial>);

#[derive(Resource, Default)]
pub struct SpatialHash {
    pub map: HashMap<IVec3, Vec<(Entity, Vec3)>>,
}

impl SpatialHash {
    pub fn key(pos: Vec3) -> IVec3 {
        (pos / ENEMY_SEPARATION_CELL).floor().as_ivec3()
    }
    /// All enemies within `radius` of `pos` (approximate, cell-based).
    pub fn near<'a>(&'a self, pos: Vec3, radius: f32) -> impl Iterator<Item = (Entity, Vec3)> + 'a {
        let r = (radius / ENEMY_SEPARATION_CELL).ceil() as i32;
        let c = Self::key(pos);
        (-r..=r).flat_map(move |x| {
            (-r..=r).flat_map(move |y| {
                (-r..=r).flat_map(move |z| {
                    self.map
                        .get(&(c + IVec3::new(x, y, z)))
                        .into_iter()
                        .flatten()
                        .copied()
                })
            })
        })
    }
}

#[derive(Resource)]
pub struct Director {
    pub spawn_bank: f32,
    /// Seconds to the next elite roll (`ELITE_ROLL_SECS` cadence).
    pub elite_timer: f32,
    /// Seconds since the last elite — the pity guarantee's clock.
    pub since_elite: f32,
    /// A roll succeeded: the next crowd spawn comes out elite.
    pub elite_pending: bool,
    /// Seconds of post-kill "exhale" left (§3 run arc).
    pub exhale: f32,
    /// Bosses alive last tick; a drop means one just fell and the horde exhales.
    pub bosses_alive: usize,
    pub tick: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self {
            spawn_bank: 0.0,
            elite_timer: ELITE_ROLL_SECS,
            since_elite: 0.0,
            elite_pending: false,
            exhale: 0.0,
            bosses_alive: 0,
            tick: 0.0,
        }
    }
}

/// Body parts show the material color (WHITE multiplier); accents are darker.
const BODY: Color = Color::WHITE;
const DARK: Color = Color::srgb(0.34, 0.34, 0.40);
const MID: Color = Color::srgb(0.68, 0.68, 0.74);

/// Compose a detailed single mesh per enemy kind (still one instanced draw call).
fn enemy_mesh(kind: EnemyKind) -> Mesh {
    use crate::meshkit::at;
    use EnemyKind::*;
    let mut m = crate::meshkit::MeshData::new();
    match kind {
        Shambler => {
            m.add_capsule(0.32, 0.5, at(Vec3::new(0.0, 0.05, 0.0)), BODY);
            m.add_sphere(0.25, 1, at(Vec3::new(0.0, 0.52, 0.0)), BODY);
            m.add_box(Vec3::new(0.28, 0.14, 0.08), at(Vec3::new(0.0, 0.52, -0.2)), DARK); // visor
            for s in [-1.0, 1.0] {
                m.add_cylinder(0.09, 0.42, 6, Transform::from_translation(Vec3::new(0.34 * s, 0.08, 0.0)).with_rotation(Quat::from_rotation_z(0.5 * s)), MID); // arm
                m.add_cylinder(0.11, 0.42, 6, at(Vec3::new(0.15 * s, -0.42, 0.0)), MID); // leg
            }
        }
        Sprinter => {
            m.add_cone(0.32, 0.95, 8, at(Vec3::new(0.0, 0.0, 0.0)), BODY);
            m.add_sphere(0.17, 1, at(Vec3::new(0.0, 0.5, 0.0)), BODY);
            for s in [-1.0, 1.0] {
                m.add_box(Vec3::new(0.05, 0.5, 0.22), Transform::from_translation(Vec3::new(0.22 * s, 0.05, 0.15)).with_rotation(Quat::from_rotation_y(0.6 * s)), DARK); // swept fin
            }
        }
        Bruiser => {
            m.add_box(Vec3::new(0.78, 0.9, 0.66), at(Vec3::new(0.0, 0.02, 0.0)), BODY);
            m.add_sphere(0.2, 1, at(Vec3::new(0.0, 0.56, 0.0)), DARK); // small head
            for s in [-1.0, 1.0] {
                m.add_box(Vec3::new(0.28, 0.26, 0.5), at(Vec3::new(0.5 * s, 0.34, 0.0)), MID); // shoulder plate
                m.add_sphere(0.27, 1, at(Vec3::new(0.62 * s, -0.12, 0.0)), BODY); // fist
            }
        }
        Spitter => {
            m.add_ellipsoid(Vec3::new(0.5, 0.44, 0.5), 1, at(Vec3::ZERO), BODY);
            m.add_cylinder(0.22, 0.12, 8, Transform::from_translation(Vec3::new(0.0, -0.02, -0.42)).with_rotation(Quat::from_rotation_x(1.57)), DARK); // mouth
            for s in [-1.0, 1.0] {
                m.add_sphere(0.2, 1, at(Vec3::new(0.24 * s, 0.28, 0.28)), MID); // back sac
            }
        }
        Ufo => {
            m.add_ellipsoid(Vec3::new(0.62, 0.2, 0.62), 1, at(Vec3::ZERO), BODY);
            m.add_ellipsoid(Vec3::new(0.3, 0.28, 0.3), 1, at(Vec3::new(0.0, 0.2, 0.0)), MID); // dome
            m.add_cylinder(0.5, 0.06, 12, at(Vec3::new(0.0, -0.14, 0.0)), DARK); // underside ring
            for i in 0..3 {
                let a = i as f32 / 3.0 * std::f32::consts::TAU;
                m.add_sphere(0.07, 0, at(Vec3::new(a.cos() * 0.42, -0.16, a.sin() * 0.42)), DARK); // lights
            }
        }
        Burrower => {
            m.add_cone(0.4, 0.5, 6, at(Vec3::new(0.0, -0.28, 0.0)), MID);
            m.add_cone(0.3, 0.45, 6, at(Vec3::new(0.0, 0.05, 0.0)), BODY);
            m.add_cone(0.19, 0.45, 6, at(Vec3::new(0.0, 0.42, 0.0)), DARK); // drill tip
        }
        Beamer => {
            m.add_cone(0.28, 1.4, 5, at(Vec3::new(0.0, 0.0, 0.0)), BODY); // main crystal
            m.add_sphere(0.15, 1, at(Vec3::new(0.0, 0.12, 0.0)), DARK); // core
            for s in [-1.0, 1.0] {
                m.add_cone(0.12, 0.7, 4, Transform::from_translation(Vec3::new(0.2 * s, -0.15, 0.0)).with_rotation(Quat::from_rotation_z(0.5 * s)), MID); // shard
            }
        }
        Lobber => {
            m.add_ellipsoid(Vec3::new(0.58, 0.4, 0.58), 1, at(Vec3::new(0.0, -0.1, 0.0)), BODY); // dome
            m.add_cylinder(0.16, 0.7, 8, Transform::from_translation(Vec3::new(0.0, 0.18, 0.12)).with_rotation(Quat::from_rotation_x(-0.6)), DARK); // barrel
            for i in 0..3 {
                let a = i as f32 / 3.0 * std::f32::consts::TAU;
                m.add_cylinder(0.07, 0.4, 5, Transform::from_translation(Vec3::new(a.cos() * 0.4, -0.35, a.sin() * 0.4)).with_rotation(Quat::from_rotation_z(a.cos() * 0.3)), MID); // leg
            }
        }
        Ghost => {
            m.add_cone(0.42, 1.1, 8, at(Vec3::new(0.0, 0.0, 0.0)), BODY);
            m.add_sphere(0.22, 1, at(Vec3::new(0.0, 0.48, 0.0)), BODY);
            m.add_box(Vec3::new(0.24, 0.12, 0.08), at(Vec3::new(0.0, 0.48, -0.18)), DARK); // visor
        }
    }
    m.build()
}

/// The Craterpillar's armored head: mandibles, ridge plates, dark eye sockets.
fn worm_head_mesh() -> Mesh {
    use crate::meshkit::at;
    let mut m = crate::meshkit::MeshData::new();
    m.add_ellipsoid(Vec3::new(0.8, 0.75, 0.95), 2, at(Vec3::ZERO), BODY); // head
    m.add_box(Vec3::new(0.5, 0.18, 0.5), at(Vec3::new(0.0, 0.55, 0.05)), MID); // crest plate
    for s in [-1.0, 1.0] {
        m.add_cone(0.16, 0.7, 5, Transform::from_translation(Vec3::new(0.4 * s, -0.2, -0.7)).with_rotation(Quat::from_rotation_x(1.4) * Quat::from_rotation_z(0.3 * s)), DARK); // mandible
        m.add_sphere(0.13, 1, at(Vec3::new(0.32 * s, 0.15, -0.62)), DARK); // eye socket
        m.add_box(Vec3::new(0.14, 0.3, 0.5), at(Vec3::new(0.66 * s, 0.1, 0.1)), MID); // cheek plate
    }
    m.build()
}

/// One armored body chunk of the Craterpillar.
fn worm_seg_mesh() -> Mesh {
    use crate::meshkit::at;
    let mut m = crate::meshkit::MeshData::new();
    m.add_sphere(0.7, 1, at(Vec3::ZERO), BODY);
    m.add_box(Vec3::new(0.42, 0.28, 0.7), at(Vec3::new(0.0, 0.5, 0.0)), MID); // dorsal ridge
    for s in [-1.0, 1.0] {
        m.add_cone(0.1, 0.4, 4, Transform::from_translation(Vec3::new(0.6 * s, 0.15, 0.0)).with_rotation(Quat::from_rotation_z(1.4 * s)), DARK); // side spike
    }
    m.build()
}

/// Judge Anubot — a jackal-headed rover-god: tracked rover body, riser neck, an
/// elongated jackal head with a snout and tall pointed ears, glowing eyes.
fn anubot_mesh() -> Mesh {
    use crate::meshkit::at;
    let mut m = crate::meshkit::MeshData::new();
    // rover chassis + treads
    m.add_box(Vec3::new(1.3, 0.55, 1.5), at(Vec3::new(0.0, 0.0, 0.0)), BODY);
    for s in [-1.0, 1.0] {
        m.add_box(Vec3::new(0.34, 0.42, 1.65), at(Vec3::new(0.72 * s, -0.18, 0.0)), DARK); // tread
    }
    // riser + jackal head
    m.add_box(Vec3::new(0.5, 0.7, 0.5), at(Vec3::new(0.0, 0.55, -0.35)), MID); // neck
    m.add_box(Vec3::new(0.55, 0.55, 0.85), at(Vec3::new(0.0, 1.0, -0.55)), BODY); // head
    m.add_box(Vec3::new(0.34, 0.34, 0.5), at(Vec3::new(0.0, 0.9, -1.05)), MID); // snout
    for s in [-1.0, 1.0] {
        m.add_cone(0.13, 0.7, 4, at(Vec3::new(0.2 * s, 1.5, -0.4)), DARK); // pointed ear
        m.add_sphere(0.08, 1, at(Vec3::new(0.16 * s, 1.05, -0.92)), DARK); // eye socket
    }
    m.build()
}

/// A hulking generic boss silhouette: heavy body, plated shoulders, horned head,
/// back spikes. Low-count so detail is free. (Per-boss unique meshes are future work.)
pub fn boss_mesh() -> Mesh {
    use crate::meshkit::at;
    let mut m = crate::meshkit::MeshData::new();
    m.add_ellipsoid(Vec3::new(0.75, 0.85, 0.7), 2, at(Vec3::new(0.0, 0.0, 0.0)), BODY); // torso
    m.add_sphere(0.34, 1, at(Vec3::new(0.0, 0.75, 0.0)), DARK); // head
    for s in [-1.0, 1.0] {
        m.add_box(Vec3::new(0.34, 0.34, 0.6), at(Vec3::new(0.62 * s, 0.5, 0.0)), MID); // shoulder plate
        m.add_cone(0.14, 0.5, 5, Transform::from_translation(Vec3::new(0.18 * s, 1.0, 0.0)).with_rotation(Quat::from_rotation_z(0.3 * s)), MID); // horn
        m.add_sphere(0.3, 1, at(Vec3::new(0.7 * s, -0.2, 0.0)), BODY); // fist
    }
    for i in 0..5 {
        let x = (i as f32 / 4.0 - 0.5) * 0.7;
        m.add_cone(0.1, 0.4, 4, at(Vec3::new(x, 0.3, 0.55)), DARK); // back spikes
    }
    m.build()
}

/// A Beamer's aim line: AIM_DASHES dashes along local -Z (the `frame_quat` forward) over one
/// unit of length, unit cross-section — the transform stretches it to the line's length and
/// thickness, and slides it forward by the march phase. `pad` fattens every dash for the
/// outline hull.
fn aim_dash_mesh(pad: f32) -> Mesh {
    let mut m = crate::meshkit::MeshData::new();
    let period = 1.0 / AIM_DASHES as f32;
    let dash = period * 0.55;
    for i in 0..AIM_DASHES {
        let z = -(i as f32 * period + dash * 0.5);
        m.add_box(Vec3::new(1.0 + pad, 1.0 + pad, dash + pad * 0.02), crate::meshkit::at(Vec3::new(0.0, 0.0, z)), Color::WHITE);
    }
    m.build_ccw()
}

/// The Burrower's crack decal: seven jagged three-segment cracks radiating from a small
/// hub, flat in the local XZ plane (the ground once `frame_quat` stands it on the surface),
/// radius 1. Hand-laid rather than random so every machine draws the same crack.
fn crack_mesh(width: f32) -> Mesh {
    let mut m = crate::meshkit::MeshData::new();
    const CRACKS: [(f32, [f32; 3]); 7] = [
        (0.0, [0.25, -0.30, 0.20]),
        (0.95, [-0.20, 0.35, -0.10]),
        (1.75, [0.30, 0.10, -0.35]),
        (2.60, [-0.25, -0.20, 0.30]),
        (3.45, [0.15, 0.30, -0.25]),
        (4.40, [-0.30, 0.05, 0.25]),
        (5.35, [0.20, -0.25, -0.20]),
    ];
    for (base, bends) in CRACKS {
        let mut from = Vec2::ZERO;
        let mut ang = base;
        for (k, bend) in bends.iter().enumerate() {
            ang += bend;
            let len = [0.38, 0.34, 0.28][k];
            let to = from + Vec2::new(ang.cos(), ang.sin()) * len;
            let mid = (from + to) * 0.5;
            let w = width * (1.0 - 0.25 * k as f32); // cracks taper toward their tips
            m.add_box(
                Vec3::new(w, 0.02 + width * 0.2, len + w * 0.5),
                Transform::from_translation(Vec3::new(mid.x, 0.0, mid.y))
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - ang)),
                Color::WHITE,
            );
            from = to;
        }
    }
    m.add_cylinder(0.12 + width, 0.03 + width * 0.2, 7, crate::meshkit::at(Vec3::ZERO), Color::WHITE);
    m.build_ccw()
}

/// Danger material colors in a palette: ring, ring fill, shot, beam charge, beam fire. The
/// materials are unlit — they draw exactly this base color — and the Standard palette keeps
/// the exact canon look.
fn danger_looks(p: Palette) -> [Color; 5] {
    let d = p.danger();
    [d, d.with_alpha(0.2), p.danger_shot(), p.danger_charge(), p.beam_fire()]
}

pub fn setup_enemy_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mesh_map = HashMap::new();
    let mut mat_map = HashMap::new();
    for kind in [
        EnemyKind::Shambler,
        EnemyKind::Sprinter,
        EnemyKind::Bruiser,
        EnemyKind::Spitter,
        EnemyKind::Ufo,
        EnemyKind::Burrower,
        EnemyKind::Beamer,
        EnemyKind::Lobber,
        EnemyKind::Ghost,
    ] {
        let def = kind.def();
        mesh_map.insert(kind, meshes.add(enemy_mesh(kind)));
        let ghost = kind == EnemyKind::Ghost;
        mat_map.insert(
            kind,
            materials.add(StandardMaterial {
                base_color: if ghost { def.color.with_alpha(0.55) } else { def.color },
                emissive: if ghost {
                    def.color.to_linear() * 1.8
                } else {
                    def.color.to_linear() * 0.15
                },
                alpha_mode: if ghost { AlphaMode::Blend } else { AlphaMode::Opaque },
                perceptual_roughness: 0.8,
                ..default()
            }),
        );
    }

    commands.insert_resource(EnemyAssets {
        meshes: mesh_map,
        mats: mat_map,
        elite_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.55, 0.1),
            emissive: LinearRgba::rgb(1.6, 0.7, 0.05),
            perceptual_roughness: 0.5,
            ..default()
        }),
        flash_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::rgb(4.0, 4.0, 4.0),
            unlit: true,
            ..default()
        }),
        boss_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.2, 0.6),
            emissive: LinearRgba::rgb(0.8, 0.15, 0.9),
            perceptual_roughness: 0.4,
            ..default()
        }),
        worm_head_mesh: meshes.add(worm_head_mesh()),
        worm_seg_mesh: meshes.add(worm_seg_mesh()),
        worm_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.54, 0.60),
            emissive: LinearRgba::rgb(0.06, 0.06, 0.09),
            perceptual_roughness: 0.55,
            metallic: 0.25,
            ..default()
        }),
        anubot_mesh: meshes.add(anubot_mesh()),
        anubot_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.66, 0.28), // sandstone gold
            emissive: LinearRgba::rgb(0.10, 0.07, 0.02),
            perceptual_roughness: 0.5,
            metallic: 0.35,
            ..default()
        }),
        beam_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        beam_charge_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.7, 0.2, 0.35),
            emissive: LinearRgba::rgb(1.2, 0.7, 0.1),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        beam_fire_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.3, 0.15),
            emissive: LinearRgba::rgb(4.0, 0.8, 0.2),
            unlit: true,
            ..default()
        }),
        proj_mesh: meshes.add(Mesh::from(Sphere::new(0.28))),
        proj_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.9),
            emissive: LinearRgba::rgb(2.4, 0.5, 2.4),
            unlit: true,
            ..default()
        }),
        ring_mesh: meshes.add(Mesh::from(Torus::new(0.9, 1.0))),
        // Telegraph looks are (re)applied from the palette by `apply_danger_palette`; these
        // are the canon values the Standard palette keeps. The depth bias keeps a flat ring
        // readable where it crosses a bump in the terrain instead of vanishing into it.
        ring_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.25, 0.1),
            emissive: LinearRgba::rgb(3.0, 0.5, 0.1),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: 40.0,
            ..default()
        }),
        ring_fill_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.25, 0.1, 0.2),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            depth_bias: 20.0,
            ..default()
        }),
        disc_mesh: meshes.add(Mesh::from(Cylinder::new(1.0, 0.004))),
        aim_mesh: meshes.add(aim_dash_mesh(0.0)),
        aim_outline_mesh: meshes.add(aim_dash_mesh(0.45)),
        crack_mesh: meshes.add(crack_mesh(0.07)),
        crack_outline_mesh: meshes.add(crack_mesh(0.11)),
        ring_outline_mesh: meshes.add(Mesh::from(Torus::new(0.88, 1.02))),
        proj_outline_mesh: meshes.add(Mesh::from(Sphere::new(0.38))),
        outline_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            unlit: true,
            cull_mode: Some(bevy::render::render_resource::Face::Front),
            depth_bias: 30.0,
            ..default()
        }),
    });
}

pub fn rebuild_hash(mut hash: ResMut<SpatialHash>, q: Query<(Entity, &Transform), With<Enemy>>) {
    for v in hash.map.values_mut() {
        v.clear();
    }
    for (e, tf) in &q {
        hash.map.entry(SpatialHash::key(tf.translation)).or_default().push((e, tf.translation));
    }
    hash.map.retain(|_, v| !v.is_empty());
}

/// One crowd enemy. Public so the headless probes can stage an exact scene (a comet tail).
#[allow(clippy::too_many_arguments)]
pub fn spawn_enemy(
    commands: &mut Commands,
    assets: &EnemyAssets,
    planet: &CurrentPlanet,
    kind: EnemyKind,
    dir: Vec3,
    elite: bool,
    hp_mult: f32,
    dmg_mult: f32,
    rng: &mut impl Rng,
) {
    let def = kind.def();
    let scale = def.scale * if elite { EliteMods::SCALE } else { 1.0 } * rng.gen_range(0.92..1.1);
    let hp = def.hp * hp_mult * if elite { EliteMods::HP } else { 1.0 };
    let mat = if elite { assets.elite_mat.clone() } else { assets.mats[&kind].clone() };
    let pos = planet.surface_point(dir) + dir * (def.hover + scale * 0.6);
    let mut cmd = commands.spawn((
        Enemy {
            kind,
            dir,
            hover: def.hover,
            speed: def.speed * rng.gen_range(0.9..1.15),
            damage: def.damage * dmg_mult * if elite { EliteMods::DMG } else { 1.0 },
            xp: def.xp * if elite { EliteMods::XP } else { 1.0 },
            hp,
            max_hp: hp,
            elite,
            contact_cd: 0.0,
            slow: 0.0,
            knock: Vec3::ZERO,
            flash: 0.0,
            scale,
            wobble: rng.gen_range(0.0..6.28),
            stride: rng.gen_range(0.0..6.28),
        },
        Mesh3d(assets.meshes[&kind].clone()),
        MeshMaterial3d(mat.clone()),
        BaseMat(mat),
        Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
        StageScoped,
    ));
    if kind == EnemyKind::Spitter || kind == EnemyKind::Ufo {
        cmd.insert(Spitter { cd: rng.gen_range(1.0..3.0) });
    }
    if kind == EnemyKind::Burrower {
        cmd.insert(Buried { timer: BURROW_SECS });
    }
    if kind == EnemyKind::Beamer {
        cmd.insert(Beamer { cd: rng.gen_range(2.0..4.0), charging: 0.0, aim: Vec3::ZERO, target: None });
    }
    if kind == EnemyKind::Lobber {
        cmd.insert(Lobber { cd: rng.gen_range(2.5..5.0) });
    }
    if kind == EnemyKind::Burrower {
        // the ground cracks over the burrow for exactly as long as it stays under
        spawn_crack_decal(commands, assets, planet, dir, BURROW_SECS);
    }
}

/// Timer-driven wave spawner. Runs while playing.
///
/// Budget per second = `Rate_base` (the §3 arc beats) × the breathing modifier (hold while a
/// miniboss is up, exhale after a boss falls) × `Scaling::spawn` (run time, depth, Δ, party).
#[allow(clippy::too_many_arguments)]
pub fn director_spawn(
    mut commands: Commands,
    time: Res<Time>,
    mut director: ResMut<Director>,
    mut game_rng: ResMut<crate::run::GameRng>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_player: Query<(&Player, &crate::run::PlayerState)>,
    q_enemies: Query<(), With<Enemy>>,
    q_boss: Query<&Boss>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Anchors to spawn around — one per living astronaut, round-robined below so each
    // player gets their own share of the horde arriving over THEIR horizon. A Signal Flare
    // carrier (§7: "enemies always know where you are") is listed twice — a double share —
    // and theirs lands closer (the flag).
    let mut anchors: Vec<(Vec3, bool)> = Vec::new();
    for (p, ps) in &q_player {
        anchors.push((p.dir, ps.revealed()));
        if ps.revealed() {
            anchors.push((p.dir, true));
        }
    }
    if anchors.is_empty() {
        return;
    }
    let party = q_player.iter().count();
    let rng = &mut game_rng.0; // deterministic spawn stream from the run seed

    let alive = q_enemies.iter().count();
    // Party scaling (GDD §11) lives in the Scaling too: more players means more horde, but
    // sub-linearly — a full budget per player doubles density and blows the cap, while no
    // bump at all gives each player half a horde.
    let sc = Scaling::for_run(&run, party);

    // Breathing: a boss count that dropped since last tick means one just fell.
    let bosses_now = q_boss.iter().count();
    if bosses_now < director.bosses_alive {
        director.exhale = SPAWN_EXHALE_SECS;
    }
    director.bosses_alive = bosses_now;
    director.exhale = (director.exhale - dt).max(0.0);
    let miniboss_alive = q_boss.iter().any(|b| !b.kind.def().is_stage_boss);

    let rate = scaling::spawn_rate_base(run.timer, run.static_active, run.static_timer)
        * if run.static_active { sc.static_rate } else { scaling::beat_modifier(miniboss_alive, director.exhale) }
        * sc.spawn;
    director.spawn_bank += rate * dt;
    director.tick += dt;

    // Elite rolls (see config::ELITE_ROLL_SECS). The Static has no elites.
    if !run.static_active {
        director.elite_timer -= dt;
        director.since_elite += dt;
        if director.elite_timer <= 0.0 {
            director.elite_timer += ELITE_ROLL_SECS;
            if rng.gen_bool(sc.elite_chance as f64) {
                director.elite_pending = true;
            }
        }
        // the pity guarantee waits out the cold open ("I have room")
        let cold_open = run.timer > SPAWN_RATE_BEATS[1].0;
        if director.since_elite >= ELITE_PITY_SECS && !cold_open {
            director.elite_pending = true;
        }
    }

    if director.tick < 0.25 {
        return;
    }
    director.tick = 0.0;

    let budget = director.spawn_bank.floor() as usize;
    if budget == 0 {
        return;
    }
    director.spawn_bank -= budget as f32;

    let cap = sc.live_cap;
    let room = cap.saturating_sub(alive);
    let n = budget.min(room);
    for i in 0..n {
        let (anchor, flare) = anchors[i % anchors.len()];
        let heading = {
            let (t, b) = sphere::tangent_frame(anchor);
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            t * a.cos() + b * a.sin()
        };
        // one draw either way: the band changes, the stream does not
        let band = if flare {
            SIGNAL_FLARE_SPAWN_ARC_MIN..SIGNAL_FLARE_SPAWN_ARC_MAX
        } else {
            SPAWN_ARC_MIN..SPAWN_ARC_MAX
        };
        let arc = rng.gen_range(band);
        let dir = sphere::offset_dir(anchor, heading, arc, planet.radius);

        if run.static_active {
            let (hp, dmg) = (sc.hp * sc.static_hp, sc.dmg * sc.static_dmg);
            spawn_enemy(&mut commands, &assets, &planet, EnemyKind::Ghost, dir, false, hp, dmg, rng);
            continue;
        }

        let mix = EnemyKind::mix(run.elapsed);
        let kind = mix[rng.gen_range(0..mix.len())];
        let elite = std::mem::take(&mut director.elite_pending);
        if elite {
            director.since_elite = 0.0;
        }
        // Burrowers ambush: spawn close.
        let dir = if kind == EnemyKind::Burrower {
            let arc = rng.gen_range(9.0..16.0);
            sphere::offset_dir(anchor, heading, arc, planet.radius)
        } else {
            dir
        };
        spawn_enemy(&mut commands, &assets, &planet, kind, dir, elite, sc.hp, sc.dmg, rng);
    }
}

/// Spawn a boss or miniboss 30 m from `player_dir`; returns the head entity.
pub fn spawn_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &EnemyAssets,
    planet: &CurrentPlanet,
    player_dir: Vec3,
    kind: BossKind,
    sc: &Scaling,
) -> Entity {
    let def = kind.def();
    let mut rng = rand::thread_rng();
    let heading = {
        let (t, b) = sphere::tangent_frame(player_dir);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        t * a.cos() + b * a.sin()
    };
    let dir = sphere::offset_dir(player_dir, heading, 30.0, planet.radius);
    let hp = def.hp * sc.boss_hp;
    let pos = planet.surface_point(dir) + dir * def.scale * 0.8;
    let is_worm = kind == BossKind::Craterpillar;
    let is_anubot = kind == BossKind::Anubot;
    let (mesh, mat) = if is_worm {
        (assets.worm_head_mesh.clone(), assets.worm_mat.clone())
    } else if is_anubot {
        (assets.anubot_mesh.clone(), assets.anubot_mat.clone())
    } else {
        (meshes.add(boss_mesh()), assets.boss_mat.clone())
    };
    let contact_dmg = def.damage * sc.boss_dmg;
    let head = commands
        .spawn((
            Enemy {
                kind: EnemyKind::Bruiser,
                dir,
                hover: 0.0,
                speed: def.speed,
                damage: contact_dmg,
                xp: 50.0,
                hp,
                max_hp: hp,
                elite: true,
                contact_cd: 0.0,
                slow: 0.0,
                knock: Vec3::ZERO,
                flash: 0.0,
                scale: def.scale,
                wobble: 0.0,
                stride: 0.0,
            },
            Boss { kind, attack_timer: 4.0, burst_timer: 7.0, phase: 0 },
            Mesh3d(mesh),
            MeshMaterial3d(mat.clone()),
            BaseMat(mat),
            Transform::from_translation(pos).with_scale(Vec3::splat(def.scale)),
            StageScoped,
        ))
        .id();

    if is_worm {
        commands
            .entity(head)
            .insert(CraterpillarHead { trail: std::collections::VecDeque::new() });
        // Body chunks trail behind, tapering toward the tail.
        for i in 0..WORM_SEGMENTS {
            let seg_scale = def.scale * (0.85 - 0.03 * i as f32).max(0.4);
            commands.spawn((
                CraterpillarSegment { head, idx: i, damage: contact_dmg * 0.7, scale: seg_scale },
                Mesh3d(assets.worm_seg_mesh.clone()),
                MeshMaterial3d(assets.worm_mat.clone()),
                Transform::from_translation(pos).with_scale(Vec3::splat(seg_scale)),
                StageScoped,
            ));
        }
    }

    if is_anubot {
        commands.entity(head).insert(AnubotBeam::default());
        spawn_anubot_beam_vis(commands, assets, head, pos);
    }
    head
}

/// Bosses escalate as their HP drops: at 66% and 33% they ENRAGE (faster, hit harder,
/// attack more often) and erupt a ring of adds around the player — the Craterpillar's
/// "burrow bloom" and Anubot's "sandstorm court". Reads as the fight changing shape.
#[allow(clippy::too_many_arguments)]
pub fn boss_phase_system(
    mut commands: Commands,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    mut shake: ResMut<Shake>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState)>,
    mut q_boss: Query<(&mut Enemy, &mut Boss)>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let snaps: Vec<crate::player::AstronautSnap> = q_player
        .iter()
        .filter(|(_, _, ps)| !ps.dead)
        .map(|(e, p, _)| crate::player::AstronautSnap { entity: e, dir: p.dir, pos: Vec3::ZERO })
        .collect();
    let mut rng = rand::thread_rng();
    let sc = Scaling::for_run(&run, q_player.iter().count());

    for (mut enemy, mut boss) in &mut q_boss {
        if enemy.hp <= 0.0 || enemy.max_hp <= 0.0 {
            continue;
        }
        let frac = enemy.hp / enemy.max_hp;
        let want = if frac < 0.33 { 2 } else if frac < 0.66 { 1 } else { 0 };
        if want <= boss.phase {
            continue;
        }
        boss.phase = want;

        // enrage
        enemy.speed *= 1.28;
        enemy.damage *= 1.22;
        boss.attack_timer = boss.attack_timer.min(1.5);
        boss.burst_timer = boss.burst_timer.min(2.0);

        // announce + juice
        let name = boss.kind.def().name;
        let label = match want {
            1 => match boss.kind {
                BossKind::Craterpillar => "BURROW BLOOM",
                BossKind::Anubot => "SANDSTORM COURT",
                _ => "ENRAGED",
            },
            _ => match boss.kind {
                BossKind::Craterpillar => "HELMET CHOIR",
                BossKind::Anubot => "FINAL JUDGMENT",
                _ => "ENRAGED",
            },
        };
        banners.write(BannerMsg(format!("{name} — {label}!")));
        sfx.write(SfxMsg(Sfx::BossRoar));
        shake.add(0.55);

        // encirclement burst: a ring of adds crests the horizon around the player
        let ring = 8 + want as usize * 3;
        // Encircle whoever this boss is closest to; if nobody is up, skip the ring but
        // keep the enrage above — phases must not depend on finding a player.
        let Some(victim) = crate::player::nearest_astronaut(enemy.dir, &snaps, planet.radius) else {
            continue;
        };
        let (t, b) = sphere::tangent_frame(victim.dir);
        for i in 0..ring {
            let a = i as f32 / ring as f32 * std::f32::consts::TAU + rng.gen_range(-0.2..0.2);
            let heading = t * a.cos() + b * a.sin();
            let arc = rng.gen_range(SPAWN_ARC_MIN..SPAWN_ARC_MAX);
            let dir = sphere::offset_dir(victim.dir, heading, arc, planet.radius);
            let mix = EnemyKind::mix(run.elapsed.max(300.0));
            let kind = mix[rng.gen_range(0..mix.len())];
            let elite = want == 2 && rng.gen_bool(0.25);
            spawn_enemy(&mut commands, &assets, &planet, kind, dir, elite, sc.hp, sc.dmg, &mut rng);
        }
    }
}

/// Judge Anubot's Verdict Beam — a lighthouse railbeam that telegraphs, then sweeps the
/// surface. Idle → charge (dim, slow rotate) → fire (bright, faster, damaging) → idle.
/// HOST-only: this advances the beam and deals the damage. What it looks like is
/// `anubot_beam_visuals`, which a co-op client runs on its streamed copy of the boss.
pub fn anubot_beam_system(
    time: Res<Time>,
    q_player: Query<(Entity, &Transform), (With<Player>, Without<AnubotBeam>)>,
    mut q_boss: Query<(Entity, &Transform, &Enemy, &Boss, &mut AnubotBeam)>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Every astronaut in the corridor is hit — a sweeping beam is AoE, not single-target.
    let ppos: Vec<(Entity, Vec3)> = q_player.iter().map(|(e, t)| (e, t.translation)).collect();
    for (e, tf, enemy, boss, mut beam) in &mut q_boss {
        beam.timer -= dt;
        beam.angle = (beam.angle + AnubotBeam::spin(beam.state, boss.phase) * dt) % std::f32::consts::TAU;
        if beam.timer <= 0.0 {
            beam.state = match beam.state {
                0 => 1,
                1 => 2,
                _ => 0,
            };
            beam.timer = AnubotBeam::state_secs(beam.state, boss.phase);
        }
        // damage while firing
        if beam.state != 2 {
            continue;
        }
        let up = tf.translation.normalize_or_zero();
        let heading = beam.heading(up);
        for (pe, pp) in ppos.iter().copied() {
            let v = pp - tf.translation;
            let along = v.dot(heading);
            let perp = (v - heading * along - up * v.dot(up)).length();
            if along > 0.0 && along < BEAM_LENGTH && perp < BEAM_WIDTH {
                writer.write(PlayerHitMsg {
                    victim: pe,
                    amount: enemy.damage * 1.2,
                    from: tf.translation,
                    attacker: Some(e),
                });
            }
        }
    }
}

/// The Verdict Beam as everyone SEES it: the boss's charge-up pose plus the beam slab.
/// Runs on host and client alike — on a client the boss is a streamed proxy whose
/// `AnubotBeam` is kept in step from BossRec — so a joiner gets the same dodge tell the
/// host does. Must run after whatever placed the boss this frame (enemy_move on the host,
/// drive_boss_proxies on a client): the pose layers on top of that transform.
#[allow(clippy::type_complexity)]
pub fn anubot_beam_visuals(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    mut q_boss: Query<(Entity, &mut Transform, &Boss, &AnubotBeam)>,
    mut q_vis: Query<
        (Entity, &AnubotBeamVis, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        Without<AnubotBeam>,
    >,
) {
    let t_now = time.elapsed_secs();
    let mut snap: HashMap<Entity, (Vec3, Vec3, u8)> = HashMap::new();
    for (e, mut tf, boss, beam) in &mut q_boss {
        let up = tf.translation.normalize_or_zero();
        // Hero-tier body tell (code-art-animation skill): the boss's POSE announces the
        // attack, not just the light — anticipation while charging, follow-through firing.
        match beam.state {
            1 => {
                // rear up as the charge builds (anticipation)
                let full = AnubotBeam::state_secs(1, boss.phase).max(0.2);
                let wind = 1.0 - (beam.timer / full).clamp(0.0, 1.0);
                tf.rotation *= Quat::from_rotation_x(0.24 * wind);
                tf.scale.y *= 1.0 + 0.07 * wind;
            }
            2 => {
                // lurch into the sweep + a high-frequency shudder while it fires
                tf.rotation *= Quat::from_rotation_x(-0.14);
                tf.rotation *= Quat::from_rotation_z((t_now * 34.0).sin() * 0.022);
            }
            _ => {}
        }
        snap.insert(e, (tf.translation, beam.heading(up), beam.state));
    }

    // place / colour / show the beam slabs
    for (ve, vis, mut tf, mut visibility, mut mat) in &mut q_vis {
        let Some((bpos, heading, state)) = snap.get(&vis.boss).copied() else {
            // Its boss is gone (dead, or a client's proxy reaped): the slab goes with it.
            commands.entity(ve).try_despawn();
            continue;
        };
        if state == 0 {
            *visibility = Visibility::Hidden;
            continue;
        }
        *visibility = Visibility::Visible;
        let up = bpos.normalize_or_zero();
        let width = if state == 2 { BEAM_WIDTH * 2.0 } else { BEAM_WIDTH * 0.6 };
        tf.translation = bpos + heading * (BEAM_LENGTH * 0.5) + up * 0.8;
        tf.rotation = sphere::frame_quat(up, heading);
        tf.scale = Vec3::new(width, 0.3, BEAM_LENGTH);
        let want = if state == 2 { &assets.beam_fire_mat } else { &assets.beam_charge_mat };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
}

/// Spawn the (hidden) beam slab that `anubot_beam_visuals` drives for `boss`. Shared by the
/// host's `spawn_boss` and the client's boss proxy, so both build the identical visual.
pub fn spawn_anubot_beam_vis(commands: &mut Commands, assets: &EnemyAssets, boss: Entity, pos: Vec3) {
    commands.spawn((
        AnubotBeamVis { boss },
        Mesh3d(assets.beam_mesh.clone()),
        MeshMaterial3d(assets.beam_charge_mat.clone()),
        Transform::from_translation(pos),
        Visibility::Hidden,
        StageScoped,
    ));
}

/// DEV: press B during play to summon the current planet's stage boss immediately
/// (so the Craterpillar is testable without surviving 8+ minutes). Host/solo only — a
/// client summoning a boss would spawn one its host never simulates. P28 moves it behind
/// `--dev`.
pub fn debug_spawn_boss(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    q_player: Query<&Player, With<crate::player::LocalPlayer>>,
    q_party: Query<(), With<Player>>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
) {
    if !keys.just_pressed(KeyCode::KeyB) {
        return;
    }
    let Ok(p) = q_player.single() else { return };
    let kind = match planet.kind {
        crate::content::planets::PlanetKind::Moon => BossKind::Craterpillar,
        _ => BossKind::Anubot,
    };
    let sc = Scaling::for_run(&run, q_party.iter().count());
    spawn_boss(&mut commands, &mut meshes, &assets, &planet, p.dir, kind, &sc);
    run.boss_spawned = true;
    banners.write(crate::messages::BannerMsg(format!("[DEV] {} SUMMONED", kind.def().name)));
}

/// Records the Craterpillar head's path and threads its body segments along it;
/// segments deal contact damage and vanish when the head dies.
#[allow(clippy::type_complexity)]
pub fn craterpillar_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_player: Query<(Entity, &Transform), (With<Player>, Without<CraterpillarSegment>, Without<CraterpillarHead>)>,
    mut set: ParamSet<(
        Query<(Entity, &Transform, &mut CraterpillarHead)>,
        Query<(Entity, &CraterpillarSegment, &mut Transform)>,
    )>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    use std::collections::HashMap;
    let t_now = time.elapsed_secs();
    // 1) extend each head's trail (distance-based so it's framerate-independent)
    let mut snapshots: HashMap<Entity, Vec<Vec3>> = HashMap::new();
    for (e, tf, mut head) in &mut set.p0() {
        let d = tf.translation.normalize_or_zero();
        let need = head.trail.front().map(|f| f.distance(d) * planet.radius > WORM_TRAIL_STEP).unwrap_or(true);
        if need && d != Vec3::ZERO {
            head.trail.push_front(d);
        }
        let cap = WORM_SEGMENTS * WORM_STRIDE + 4;
        while head.trail.len() > cap {
            head.trail.pop_back();
        }
        snapshots.insert(e, head.trail.iter().copied().collect());
    }

    // 2) place segments along their head's trail + contact damage
    let ptf: Vec<(Entity, Vec3)> = q_player.iter().map(|(e, t)| (e, t.translation)).collect();
    for (se, seg, mut stf) in &mut set.p1() {
        let Some(trail) = snapshots.get(&seg.head) else {
            commands.entity(se).despawn(); // head is gone → worm dies
            continue;
        };
        if trail.is_empty() {
            continue;
        }
        let i = (seg.idx * WORM_STRIDE).min(trail.len() - 1);
        let dir = trail[i];
        // ---- undulation: a wave travelling down the body (skill recipe R6).
        // Each segment lags the one ahead by a fixed phase, so the worm ripples
        // instead of sliding along the trail like a flat train.
        let phase = t_now * 3.9 - seg.idx as f32 * 0.8;
        let ripple = phase.sin();
        // Nearly uniform amplitude: a big whip growth down the body reads as FLOPPY
        // (tail flailing loose). A muscular worm holds its shape — barely any growth.
        let whip = 1.0 + seg.idx as f32 * 0.015;
        let lift = ripple * 0.40 * seg.scale * whip;
        let pos = planet.surface_point(dir) + dir * (seg.scale * 0.6 + lift);
        stf.translation = pos;
        // orient along the trail toward the next-newer point
        let ahead = trail[i.saturating_sub(1)];
        let fwd = (planet.surface_point(ahead) - pos).normalize_or_zero();
        let fwd = if fwd == Vec3::ZERO { sphere::tangent_frame(dir).0 } else { fwd };
        let base = sphere::frame_quat(dir, fwd);
        // serpentine: roll into the wave AND yaw side-to-side a quarter-phase later,
        // so the body snakes rather than just bobbing.
        stf.rotation = base
            * Quat::from_rotation_z(ripple * 0.26 * whip)
            * Quat::from_rotation_y((phase - 1.57).sin() * 0.13);
        // squash on the down-beat, stretch on the crest (volume preserved)
        let sy = 1.0 + ripple * 0.12;
        stf.scale = Vec3::new(seg.scale / sy.sqrt(), seg.scale * sy, seg.scale / sy.sqrt());

        {
            for (pe, pp) in ptf.iter().copied() {
                let reach = seg.scale * 0.6 + PLAYER_RADIUS + 0.25;
                if pos.distance_squared(pp) < reach * reach {
                    writer.write(PlayerHitMsg { victim: pe, amount: seg.damage, from: pos, attacker: None });
                }
            }
        }
    }
}

/// Steering + separation + transform write for every enemy.
pub fn enemy_move(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState, &Transform), Without<Enemy>>,
    mut q: Query<
        (Entity, &mut Enemy, &mut Transform),
        (Without<Buried>, Without<crate::interact::Pot>),
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Snapshot BEFORE the mutable loop: reading q_player inside `&mut q` is a B0001
    // conflict, and with up to 1200 enemies this must not be rebuilt per enemy.
    // Downed astronauts are not targets.
    let snaps: Vec<crate::player::AstronautSnap> = q_player
        .iter()
        .filter(|(_, _, ps, _)| !ps.dead)
        .map(|(e, p, _, tf)| crate::player::AstronautSnap { entity: e, dir: p.dir, pos: tf.translation })
        .collect();
    if snaps.is_empty() {
        return;
    }
    // How far each astronaut SEEMS to the horde: a Signal Flare carrier reads closer than
    // they are, so the chase prefers them over a nearer teammate (§7).
    let lure: Vec<f32> = q_player
        .iter()
        .filter(|(_, _, ps, _)| !ps.dead)
        .map(|(_, _, ps, _)| if ps.revealed() { SIGNAL_FLARE_LURE } else { 1.0 })
        .collect();
    let t_now = time.elapsed_secs();

    for (entity, mut e, mut tf) in &mut q {
        // Each enemy chases whoever is closest ALONG THE SURFACE (as the lure reads it).
        let target = snaps
            .iter()
            .zip(&lure)
            .min_by(|(a, la), (b, lb)| {
                (sphere::arc_dist(e.dir, a.dir, planet.radius) * **la)
                    .total_cmp(&(sphere::arc_dist(e.dir, b.dir, planet.radius) * **lb))
            })
            .map(|(s, _)| *s)
            .unwrap_or(snaps[0]);
        let player_dir = target.dir;
        let player_pos = target.pos;
        e.contact_cd = (e.contact_cd - dt).max(0.0);
        e.flash = (e.flash - dt * 6.0).max(0.0);
        e.slow = (e.slow - dt * 0.35).clamp(0.0, 0.9);
        e.knock *= (1.0 - 7.0 * dt).max(0.0);

        let r = planet.surface(e.dir);
        let eff_speed = e.speed * (1.0 - e.slow);

        // Ranged kinds hold their preferred distance and strafe; melee beelines.
        let standoff = e.kind.def().standoff;
        let to_player_arc = sphere::arc_dist(e.dir, player_dir, planet.radius);
        let steer_target = if standoff > 0.0 && to_player_arc < standoff {
            let (t, _) = sphere::tangent_frame(e.dir);
            let tangent_to = {
                let v = (player_dir - e.dir * player_dir.dot(e.dir)).normalize_or_zero();
                if v == Vec3::ZERO {
                    t
                } else {
                    v
                }
            };
            let side = tangent_to.cross(e.dir);
            if to_player_arc < standoff * 0.65 {
                // too close: back away along the great circle
                (e.dir * 2.0 - player_dir).normalize()
            } else {
                // in the pocket: circle-strafe
                (e.dir + (tangent_to * 0.2 + side * 0.8) * 0.08).normalize()
            }
        } else {
            player_dir
        };

        let angle = eff_speed * dt / r;
        let mut new_dir = sphere::step_toward(e.dir, steer_target, angle);

        // separation from neighbors (cheap cell lookup)
        let mut push = Vec3::ZERO;
        let mut n = 0;
        for (other, opos) in hash.near(tf.translation, ENEMY_SEPARATION_CELL) {
            if other == entity {
                continue;
            }
            let d = tf.translation - opos;
            let l = d.length();
            if l < e.scale * 0.9 + 0.6 && l > 0.0001 {
                push += d / l;
                n += 1;
            }
            if n >= 6 {
                break;
            }
        }
        if n > 0 {
            let push_t = push - e.dir * push.dot(e.dir);
            new_dir = (new_dir + push_t * (0.35 * dt)).normalize();
        }

        // knockback (world-space tangent impulse decaying)
        if e.knock.length_squared() > 0.001 {
            let (kd, _) = sphere::advance(new_dir, e.knock, r, dt);
            new_dir = kd;
        }

        e.dir = new_dir;
        let up = e.dir;

        animate_crowd(&mut e, &mut tf, &planet, player_pos, eff_speed, dt, t_now);
    }
}

/// Crowd-tier animation, shared by the host's simulated horde and a client's streamed
/// proxies so both wear exactly the same gait. Whole-transform only — 1200 enemies still
/// batch into one draw call per kind.
///
/// `eff_speed` is the enemy's current surface speed: integrated locally on the host,
/// finite-differenced from interpolated network positions on a client.
pub fn animate_crowd(
    e: &mut Enemy,
    tf: &mut Transform,
    planet: &CurrentPlanet,
    player_pos: Vec3,
    eff_speed: f32,
    dt: f32,
    t_now: f32,
) {
    let up = e.dir;
    // ---- crowd-tier animation (code-art-animation skill: whole-transform only,
    // so 1200 enemies still batch into one draw call per kind) ----
    // gait advances by DISTANCE travelled, so the waddle always matches real motion
    let moved = eff_speed * dt;
    e.stride = (e.stride + moved * 2.3) % std::f32::consts::TAU;
    let gait = (e.stride + e.wobble).sin();
    let speed_frac = (eff_speed / 6.0).clamp(0.0, 1.4);
    // just-attacked lunge, decaying — the follow-through of a contact hit
    let lunge = (e.contact_cd / CONTACT_TICK).clamp(0.0, 1.0);

    let bob = if e.hover > 0.0 {
        // fliers: stacked sines never visibly loop
        (t_now * 2.2 + e.wobble).sin() * 0.45 + (t_now * 3.7 + e.wobble * 2.0).sin() * 0.16
    } else {
        match e.kind {
            // sprinters BOUND: a big hop arc, twice per stride
            EnemyKind::Sprinter => (gait * 2.0).sin().max(0.0) * 0.55 * e.scale * speed_frac,
            // heavies stomp: shorter, weightier rise
            EnemyKind::Bruiser => (1.0 - gait.abs()) * 0.12 * e.scale * speed_frac,
            // everything else rises between footfalls
            _ => (1.0 - gait.abs()) * 0.20 * e.scale * speed_frac,
        }
    };
    let pos = planet.surface_point(up) + up * (e.hover + bob + e.scale * 0.6);
    tf.translation = pos;

    let face = (player_pos - pos).normalize_or_zero();
    let mut rot = sphere::frame_quat(up, face);
    if e.hover > 0.0 {
        // fliers bank hard into their drift instead of walking
        rot *= Quat::from_rotation_z((t_now * 1.9 + e.wobble).sin() * 0.22);
        rot *= Quat::from_rotation_x(-0.22 * speed_frac);
    } else {
        // waddle roll + lean into the chase + lunge pitch on a fresh hit
        let waddle = match e.kind {
            EnemyKind::Bruiser => 0.34, // heavy things rock hard
            EnemyKind::Sprinter => 0.12,
            _ => 0.26,
        };
        rot *= Quat::from_rotation_z(gait * waddle * speed_frac);
        // forward-back nod on the gait as well as the constant chase-lean
        rot *= Quat::from_rotation_x(
            -0.30 * speed_frac + (gait * 2.0).cos() * 0.10 * speed_frac - lunge * 0.70,
        );
    }
    tf.rotation = rot;

    // squash & stretch: compress hard on the lunge, stretch at speed.
    // volume-preserved so nothing looks like it's melting.
    let flash_pulse = 1.0 + e.flash * 0.25;
    let sy = (1.0 + gait.abs() * 0.11 * speed_frac - lunge * 0.22).max(0.6);
    let sxz = 1.0 / sy.sqrt();
    tf.scale = Vec3::new(e.scale * sxz, e.scale * sy, e.scale * sxz) * flash_pulse;

}

/// Buried burrowers erupt after their telegraph.
pub fn burrower_emerge(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
    mut q: Query<(Entity, &Enemy, &mut Buried, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let ppos: Vec<(Entity, Vec3)> = q_player.iter().map(|(e, t)| (e, t.translation)).collect();
    for (e, enemy, mut b, mut tf) in &mut q {
        b.timer -= dt;
        // rumble under the surface
        let depth = (b.timer / BURROW_SECS).clamp(0.0, 1.0);
        tf.translation = planet.surface_point(enemy.dir) - enemy.dir * (depth * 1.2);
        if b.timer <= 0.0 {
            commands.entity(e).remove::<Buried>();
            // eruption damage to anyone standing on top of it
            for (pe, pp) in ppos.iter().copied() {
                if tf.translation.distance(pp) < BURROW_ERUPT_RADIUS {
                    writer.write(PlayerHitMsg { victim: pe, amount: enemy.damage, from: tf.translation, attacker: Some(e) });
                }
            }
            commands.spawn(telegraph_bundle(
                &assets,
                &planet,
                Telegraph { timer: 0.25, max: 0.25, radius: 0.0, damage: 0.0, dir: enemy.dir, ring: false },
            ));
        }
    }
}

/// Touching the swarm hurts.
pub fn enemy_contact(
    time: Res<Time>,
    run: Res<RunState>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &Transform), Without<Buried>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    if time.delta_secs() <= 0.0 {
        return;
    }
    let snaps: Vec<crate::player::AstronautSnap> = q_player
        .iter()
        .filter(|(_, _, ps, _)| !ps.dead)
        .map(|(e, p, _, tf)| crate::player::AstronautSnap { entity: e, dir: p.dir, pos: tf.translation })
        .collect();
    if snaps.is_empty() {
        return;
    }
    for (entity, mut e, tf) in &mut q {
        if e.contact_cd > 0.0 || e.speed == 0.0 {
            continue;
        }
        let reach = e.scale * 0.55 + PLAYER_RADIUS + 0.25;
        let Some(victim) = crate::player::nearest_astronaut(e.dir, &snaps, 1.0) else { continue };
        if tf.translation.distance_squared(victim.pos) < reach * reach {
            e.contact_cd = CONTACT_TICK;
            writer.write(PlayerHitMsg {
                victim: victim.entity,
                amount: e.damage,
                from: tf.translation,
                attacker: Some(entity),
            });
        }
    }
}

/// Spitters lob shots when in range.
pub fn spitter_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState, Has<InStorm>)>,
    mut q: Query<(&Enemy, &mut Spitter, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Anyone hidden in the dust storm is invisible to ranged enemies — per astronaut, so a
    // teammate outside the cell is still a target while you ride it.
    // (a Signal Flare carrier is never hidden: the horde always knows where they are)
    let snaps: Vec<crate::player::AstronautSnap> = q_player
        .iter()
        .filter(|(_, _, ps, hidden)| !ps.dead && (!hidden || ps.revealed()))
        .map(|(e, p, _, _)| crate::player::AstronautSnap { entity: e, dir: p.dir, pos: Vec3::ZERO })
        .collect();
    if snaps.is_empty() {
        return;
    }
    for (e, mut s, tf) in &mut q {
        s.cd -= dt;
        if s.cd > 0.0 {
            continue;
        }
        let Some(player) = crate::player::nearest_astronaut(e.dir, &snaps, planet.radius) else { continue };
        let arc = sphere::arc_dist(e.dir, player.dir, planet.radius);
        // UFOs zap faster, harder-to-dodge bolts from above
        let (range, cooldown, speed) = if e.kind == crate::content::enemies::EnemyKind::Ufo {
            (15.0, 2.2, 16.0)
        } else {
            (20.0, 2.8, 11.0)
        };
        if arc < range {
            s.cd = cooldown;
            let to = (player.dir - e.dir * player.dir.dot(e.dir)).normalize_or_zero();
            let heading = (planet.surface_point(player.dir) - tf.translation).normalize_or_zero();
            let heading_t = (heading - e.dir * heading.dot(e.dir)).normalize_or_zero();
            let h = if heading_t == Vec3::ZERO { to } else { heading_t };
            commands.spawn((
                EnemyProjectile { dir: e.dir, heading: h, speed, damage: e.damage, life: 4.0, hover: 1.0 },
                Mesh3d(assets.proj_mesh.clone()),
                MeshMaterial3d(assets.proj_mat.clone()),
                Transform::from_translation(tf.translation),
                StageScoped,
            ));
        }
    }
}

/// Beamers paint the player with a tracking aim line, lock late, then fire a railbolt.
/// HOST-only (it decides and fires); the line itself is drawn by `aim_line_visuals`,
/// which a co-op client also runs on the lines it rebuilt from the hazard lane.
#[allow(clippy::too_many_arguments)]
pub fn beamer_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState, &Transform, Has<InStorm>), Without<Enemy>>,
    mut q: Query<(Entity, &Enemy, &mut Beamer, &Transform), Without<Buried>>,
    q_lines: Query<(Entity, &AimLine)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // (astronaut, hidden in the dust storm)
    let all: Vec<(crate::player::AstronautSnap, bool)> = q_player
        .iter()
        .filter(|(_, _, ps, _, _)| !ps.dead)
        // a Signal Flare carrier is never hidden: the horde always knows where they are
        .map(|(en, p, ps, tf, hidden)| {
            (crate::player::AstronautSnap { entity: en, dir: p.dir, pos: tf.translation }, hidden && !ps.revealed())
        })
        .collect();
    // Only astronauts OUT of the dust can be picked as a new target.
    let visible: Vec<crate::player::AstronautSnap> =
        all.iter().filter(|(_, hidden)| !hidden).map(|(s, _)| *s).collect();
    let drop_line = |commands: &mut Commands, owner: Entity| {
        for (le, line) in q_lines.iter() {
            if line.owner == owner {
                commands.entity(le).try_despawn();
            }
        }
    };

    for (entity, e, mut b, tf) in &mut q {
        // While charging, stay on the LATCHED target (if it still exists); otherwise pick
        // the nearest visible astronaut fresh.
        let latched = b
            .target
            .filter(|_| b.charging > 0.0)
            .and_then(|t| all.iter().copied().find(|(s, _)| s.entity == t));
        if let Some((_, true)) = latched {
            // Its mark ducked into the dust storm: the telegraph was for them, so drop it
            // and hold fire rather than swing the line onto somebody else mid-sweep.
            b.charging = 0.0;
            b.target = None;
            drop_line(&mut commands, entity);
            continue;
        }
        let Some(player) = latched
            .map(|(s, _)| s)
            .or_else(|| crate::player::nearest_astronaut(e.dir, &visible, planet.radius))
        else {
            // nobody to see: an unfinished charge fizzles
            if b.charging > 0.0 {
                b.charging = 0.0;
                b.target = None;
                drop_line(&mut commands, entity);
            }
            continue;
        };
        let ptf = player;
        let arc = sphere::arc_dist(e.dir, player.dir, planet.radius);
        if b.charging > 0.0 {
            b.charging -= dt;
            // track the player until the final quarter second, then hold the lock
            if b.charging > BEAMER_LOCK_SECS {
                let v = ptf.pos - tf.translation;
                let vt = (v - e.dir * v.dot(e.dir)).normalize_or_zero();
                if vt != Vec3::ZERO {
                    b.aim = vt;
                }
            }
            if b.charging <= 0.0 {
                // FIRE
                b.cd = 4.0;
                commands.spawn((
                    EnemyProjectile {
                        dir: e.dir,
                        heading: b.aim,
                        speed: 40.0,
                        damage: e.damage,
                        life: 1.4,
                        hover: 1.0,
                    },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.ring_mat.clone()),
                    Transform::from_translation(tf.translation + e.dir * 1.2)
                        .with_scale(Vec3::new(0.5, 0.5, 2.2)),
                    StageScoped,
                ));
                drop_line(&mut commands, entity);
            }
            continue;
        }
        b.cd -= dt;
        if b.cd <= 0.0 && arc < 26.0 {
            b.charging = BEAMER_CHARGE_SECS;
            b.target = Some(player.entity); // commit for the whole telegraph
            let v = ptf.pos - tf.translation;
            b.aim = (v - e.dir * v.dot(e.dir)).normalize_or_zero();
            commands.spawn((
                AimLine { owner: entity, march: 0.0 },
                Mesh3d(assets.aim_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                // zero-scaled until `aim_line_visuals` lays it along the aim
                Transform::from_translation(tf.translation).with_scale(Vec3::ZERO),
                StageScoped,
            ));
        }
    }
}

/// Lay each live aim line from its beamer along the current aim. While the beamer tracks,
/// its dashes march toward the target and the line thickens with the charge; in the final
/// BEAMER_LOCK_SECS they freeze and go fat — "it has stopped aiming, dodge now". Shared by
/// the host and a co-op client, so a joiner reads the exact tell the host would. Lines whose
/// beamer is gone are cleared here.
pub fn aim_line_visuals(
    mut commands: Commands,
    time: Res<Time>,
    q: Query<(&Enemy, &Beamer, &Transform)>,
    mut q_lines: Query<(Entity, &mut AimLine, &mut Transform), Without<Enemy>>,
) {
    let dt = time.delta_secs();
    let period = AIM_LINE_LEN / AIM_DASHES as f32;
    for (le, mut line, mut ltf) in &mut q_lines {
        let Ok((e, b, tf)) = q.get(line.owner) else {
            commands.entity(le).try_despawn();
            continue;
        };
        if b.aim == Vec3::ZERO {
            continue;
        }
        let locked = b.charging <= BEAMER_LOCK_SECS;
        if !locked {
            line.march = (line.march + dt * AIM_DASH_SPEED / period).fract();
        }
        let charge = 1.0 - (b.charging / BEAMER_CHARGE_SECS).clamp(0.0, 1.0);
        let width = if locked { 0.26 } else { 0.09 + charge * 0.1 };
        // The dash mesh runs from the origin along local -Z (frame_quat's forward).
        ltf.translation = tf.translation + e.dir * 1.0 + b.aim * (line.march * period);
        ltf.rotation = sphere::frame_quat(e.dir, b.aim);
        ltf.scale = Vec3::new(width, width, AIM_LINE_LEN);
    }
}

/// Lobbers mortar the player's position: landing telegraph + arcing shell.
pub fn lobber_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(Entity, &Player, &crate::run::PlayerState, Has<InStorm>)>,
    mut q: Query<(&Enemy, &mut Lobber, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // can't range anyone through the dust
    // (a Signal Flare carrier is never hidden: the horde always knows where they are)
    let snaps: Vec<crate::player::AstronautSnap> = q_player
        .iter()
        .filter(|(_, _, ps, hidden)| !ps.dead && (!hidden || ps.revealed()))
        .map(|(e, p, _, _)| crate::player::AstronautSnap { entity: e, dir: p.dir, pos: Vec3::ZERO })
        .collect();
    if snaps.is_empty() {
        return;
    }
    for (e, mut l, tf) in &mut q {
        l.cd -= dt;
        if l.cd > 0.0 {
            continue;
        }
        let Some(player) = crate::player::nearest_astronaut(e.dir, &snaps, planet.radius) else { continue };
        let arc = sphere::arc_dist(e.dir, player.dir, planet.radius);
        if arc < 24.0 {
            l.cd = 4.5;
            let target = player.dir;
            let flight = 1.6;
            commands.spawn(telegraph_bundle(
                &assets,
                &planet,
                Telegraph { timer: flight, max: flight, radius: 3.2, damage: e.damage, dir: target, ring: false },
            ));
            commands.spawn((
                MortarShell { from: e.dir, to: target, t: 0.0, dur: flight },
                Mesh3d(assets.proj_mesh.clone()),
                MeshMaterial3d(assets.proj_mat.clone()),
                Transform::from_translation(tf.translation).with_scale(Vec3::splat(1.6)),
                StageScoped,
            ));
        }
    }
}

/// Shells fly a slerp arc with a parabolic height bump; the telegraph lands the hit.
pub fn mortar_shells(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(Entity, &mut MortarShell, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut s, mut tf) in &mut q {
        s.t += dt / s.dur;
        if s.t >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let dir = s.from.slerp(s.to, s.t).normalize();
        let peak = 9.0;
        let height = peak * 4.0 * s.t * (1.0 - s.t);
        tf.translation = planet.surface_point(dir) + dir * (1.0 + height);
    }
}

pub fn enemy_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    mut q: Query<(Entity, &mut EnemyProjectile, &mut Transform), Without<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let ppos: Vec<(Entity, Vec3)> = q_player.iter().map(|(e, t)| (e, t.translation)).collect();
    for (e, mut p, mut tf) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let r = planet.surface(p.dir) + p.hover;
        let vel = p.heading * p.speed;
        let (nd, nv) = sphere::advance(p.dir, vel, r, dt);
        p.dir = nd;
        p.heading = nv.normalize_or_zero();
        tf.translation = planet.surface_point(p.dir) + p.dir * p.hover;
        // first astronaut it touches eats it (a shot is consumed by one body)
        if let Some((pe, _)) = ppos
            .iter()
            .copied()
            .find(|(_, pp)| tf.translation.distance_squared(*pp) < 1.1)
        {
            writer.write(PlayerHitMsg { victim: pe, amount: p.damage, from: tf.translation, attacker: None });
            commands.entity(e).despawn();
        }
    }
}

/// Boss specials: expanding slam telegraph + radial projectile bursts.
pub fn boss_attacks(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(&Enemy, &mut Boss, &Transform)>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut boss, tf) in &mut q {
        boss.attack_timer -= dt;
        boss.burst_timer -= dt;
        if boss.attack_timer <= 0.0 {
            boss.attack_timer = 6.5;
            commands.spawn(telegraph_bundle(
                &assets,
                &planet,
                Telegraph { timer: 1.4, max: 1.4, radius: 7.0, damage: e.damage * 1.6, dir: e.dir, ring: true },
            ));
            sfx.write(SfxMsg(Sfx::BossRoar));
        }
        if boss.burst_timer <= 0.0 {
            boss.burst_timer = 9.0;
            let (t, b) = sphere::tangent_frame(e.dir);
            for i in 0..12 {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                let h = t * a.cos() + b * a.sin();
                commands.spawn((
                    EnemyProjectile { dir: e.dir, heading: h, speed: 9.0, damage: e.damage * 0.8, life: 5.0, hover: 1.0 },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.proj_mat.clone()),
                    Transform::from_translation(tf.translation),
                    StageScoped,
                ));
            }
        }
    }
}

/// Everything a telegraph spawns with, laid flat on the ground at `tg.dir` — shared by the
/// host's attacks and a client's streamed copy, so both draw the same ring in the same place.
pub fn telegraph_bundle(assets: &EnemyAssets, planet: &CurrentPlanet, tg: Telegraph) -> impl Bundle {
    let start = if tg.radius > 0.0 { tg.radius } else { 0.1 };
    (
        Mesh3d(assets.ring_mesh.clone()),
        MeshMaterial3d(assets.ring_mat.clone()),
        ground_decal(planet, tg.dir, TELEGRAPH_LIFT).with_scale(Vec3::splat(start)),
        tg,
        StageScoped,
    )
}

/// A transform lying flat on the ground at `dir` (mesh local XZ = the ground, +Y = up).
pub fn ground_decal(planet: &CurrentPlanet, dir: Vec3, lift: f32) -> Transform {
    Transform::from_translation(planet.surface_point(dir) + dir * lift)
        .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
}

/// Start a Burrower's crack decal (host at spawn, client from the hazard lane). Its spin is
/// hashed from where it is, so two machines draw the same crack.
pub fn spawn_crack_decal(commands: &mut Commands, assets: &EnemyAssets, planet: &CurrentPlanet, dir: Vec3, secs: f32) -> Entity {
    let spin = ((dir.x * 37.13 + dir.y * 11.7 + dir.z * 91.71).fract().abs()) * std::f32::consts::TAU;
    let mut tf = ground_decal(planet, dir, 0.12);
    tf.rotation *= Quat::from_rotation_y(spin);
    commands
        .spawn((
            CrackDecal { dir, timer: secs, max: secs },
            Mesh3d(assets.crack_mesh.clone()),
            MeshMaterial3d(assets.ring_mat.clone()),
            tf.with_scale(Vec3::new(0.2 * BURROW_ERUPT_RADIUS, 1.0, 0.2 * BURROW_ERUPT_RADIUS)),
            StageScoped,
        ))
        .id()
}

/// Cracks race outward from the burrow and creep to the eruption's full reach just as it
/// breaks the surface. Runs everywhere a crack can exist (host, client, headless).
pub fn crack_decals(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut CrackDecal, &mut Transform)>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut c, mut tf) in &mut q {
        c.timer -= dt;
        if c.timer <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let t = 1.0 - (c.timer / c.max).clamp(0.0, 1.0);
        let reach = BURROW_ERUPT_RADIUS * (0.2 + 0.8 * (1.0 - (1.0 - t) * (1.0 - t)));
        tf.scale = Vec3::new(reach, 1.0, reach);
    }
}

/// Telegraphs mark their TRUE lethal edge from the first frame and pulse inward from it,
/// faster as impact nears (a rising chirp, held under 3/s in photosensitivity mode), then
/// detonate against every astronaut. The eruption pop (radius 0) just blooms outward. The
/// fill that counts down to impact is presentation (`animate_hazard_decor`).
#[allow(clippy::too_many_arguments)]
pub fn telegraphs(
    mut commands: Commands,
    time: Res<Time>,
    save: Res<crate::save::MetaSave>,
    mut shake: ResMut<Shake>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    mut q: Query<(Entity, &mut Telegraph, &mut Transform), Without<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let (f0, f1) = if save.accessibility.photosensitive {
        (TELEGRAPH_PULSE_HZ.0.min(TELEGRAPH_PULSE_HZ_PHOTO), TELEGRAPH_PULSE_HZ_PHOTO)
    } else {
        TELEGRAPH_PULSE_HZ
    };
    let ppos: Vec<(Entity, Vec3)> = q_player.iter().map(|(e, t)| (e, t.translation)).collect();
    for (e, mut tg, mut tf) in &mut q {
        tg.timer -= dt;
        let t = 1.0 - (tg.timer / tg.max).clamp(0.0, 1.0);
        if tg.radius > 0.0 {
            // phase of a chirp whose rate ramps f0 -> f1 across the telegraph's life
            let phase = std::f32::consts::TAU * tg.max * (f0 * t + (f1 - f0) * t * t * 0.5);
            // inward only: the ring never draws the lethal edge further out than it is
            let pulse = 1.0 - TELEGRAPH_PULSE_AMP * (0.5 + 0.5 * phase.sin());
            tf.scale = Vec3::splat(tg.radius * pulse);
        } else {
            tf.scale = Vec3::splat(0.1 + t * 0.6);
        }
        if tg.timer <= 0.0 {
            if tg.damage > 0.0 {
                for (pe, pp) in ppos.iter().copied() {
                    let d = tf.translation.distance(pp);
                    let hit = if tg.ring {
                        d < tg.radius + 1.0 && d > tg.radius * SLAM_SAFE_FRACTION
                    } else {
                        d < tg.radius + 0.6
                    };
                    if hit {
                        writer.write(PlayerHitMsg { victim: pe, amount: tg.damage, from: tf.translation, attacker: None });
                    }
                }
                shake.add(0.22);
                if let Some(pa) = &particles {
                    fx::burst(&mut commands, pa, tf.translation, tg.dir, Pcolor::Danger, 18, 9.0);
                }
            }
            commands.entity(e).despawn();
        }
    }
}

/// PRESENTATION: give each new hazard the parts that let it read without color (§13) — a
/// fill that sweeps out to the lethal edge by impact, a slam's inner safe ring — and, in
/// high-contrast mode, a white outline hull on every telegraph, aim line, crack, shot and
/// the verdict beam. Children, so they move, scale and despawn with their hazard; there are
/// at most a few dozen hazards alive, never one per crowd enemy. Toggling high contrast
/// mid-run (pause menu) outlines, or strips, every hazard already standing.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn decorate_hazards(
    mut commands: Commands,
    assets: Res<EnemyAssets>,
    save: Res<crate::save::MetaSave>,
    tels: Query<(Entity, Ref<Telegraph>)>,
    lines: Query<(Entity, Ref<AimLine>)>,
    cracks: Query<(Entity, Ref<CrackDecal>)>,
    shots: Query<(Entity, Option<Ref<EnemyProjectile>>, Option<Ref<MortarShell>>), Or<(With<EnemyProjectile>, With<MortarShell>)>>,
    beams: Query<(Entity, Ref<AnubotBeamVis>)>,
    decor: Query<(Entity, &HazardDecor)>,
    mut last_hc: Local<Option<bool>>,
) {
    let hc = save.accessibility.high_contrast;
    let toggled = last_hc.is_some_and(|was| was != hc);
    *last_hc = Some(hc);
    if toggled && !hc {
        for (e, d) in &decor {
            if *d == HazardDecor::Outline {
                commands.entity(e).try_despawn();
            }
        }
    }
    // a hazard gets an outline when it appears, or when outlines were just switched on
    let wants_outline = |added: bool| hc && (added || toggled);
    let outline = |mesh: &Handle<Mesh>, tf: Transform| {
        (HazardDecor::Outline, Mesh3d(mesh.clone()), MeshMaterial3d(assets.outline_mat.clone()), tf)
    };
    for (e, tg) in &tels {
        let added = tg.is_added();
        if !added && !wants_outline(false) {
            continue;
        }
        // `get_entity`: a hazard can be retired in the very frame it is first seen here.
        let Ok(mut ec) = commands.get_entity(e) else { continue };
        let safe = Transform::from_scale(Vec3::splat(SLAM_SAFE_FRACTION));
        ec.with_children(|c| {
            if added && tg.radius > 0.0 {
                if tg.ring {
                    c.spawn((HazardDecor::SafeRing, Mesh3d(assets.ring_mesh.clone()), MeshMaterial3d(assets.ring_mat.clone()), safe));
                    c.spawn((HazardDecor::Sweep, Mesh3d(assets.ring_mesh.clone()), MeshMaterial3d(assets.ring_mat.clone()), safe));
                } else {
                    c.spawn((
                        HazardDecor::Sweep,
                        Mesh3d(assets.disc_mesh.clone()),
                        MeshMaterial3d(assets.ring_fill_mat.clone()),
                        Transform::from_scale(Vec3::new(0.0, 1.0, 0.0)),
                    ));
                }
            }
            if wants_outline(added) {
                c.spawn(outline(&assets.ring_outline_mesh, Transform::IDENTITY));
                if tg.radius > 0.0 && tg.ring {
                    c.spawn(outline(&assets.ring_outline_mesh, safe));
                }
            }
        });
    }
    if !hc {
        return;
    }
    let beam_hull = Transform::from_scale(Vec3::new(1.08, 1.6, 1.004));
    let hulls = lines
        .iter()
        .map(|(e, r)| (e, r.is_added(), &assets.aim_outline_mesh, Transform::IDENTITY))
        .chain(cracks.iter().map(|(e, r)| (e, r.is_added(), &assets.crack_outline_mesh, Transform::IDENTITY)))
        .chain(shots.iter().map(|(e, p, m)| {
            let added = p.is_some_and(|r| r.is_added()) || m.is_some_and(|r| r.is_added());
            (e, added, &assets.proj_outline_mesh, Transform::IDENTITY)
        }))
        .chain(beams.iter().map(|(e, r)| (e, r.is_added(), &assets.beam_mesh, beam_hull)));
    for (e, added, mesh, tf) in hulls {
        if !wants_outline(added) {
            continue;
        }
        let Ok(mut ec) = commands.get_entity(e) else { continue };
        ec.with_children(|c| {
            c.spawn(outline(mesh, tf));
        });
    }
}

/// PRESENTATION: sweep each telegraph's fill out toward the lethal edge as impact nears —
/// the countdown, told by motion. A disc fills from the center; a slam's band fills from
/// its safe inner edge outward.
pub fn animate_hazard_decor(
    q_tel: Query<(&Telegraph, &Children)>,
    mut q_decor: Query<(&HazardDecor, &mut Transform), Without<Telegraph>>,
) {
    for (tg, children) in &q_tel {
        let t = 1.0 - (tg.timer / tg.max).clamp(0.0, 1.0);
        for child in children.iter() {
            let Ok((decor, mut tf)) = q_decor.get_mut(child) else { continue };
            if *decor != HazardDecor::Sweep {
                continue;
            }
            tf.scale = if tg.ring {
                Vec3::splat(SLAM_SAFE_FRACTION + (1.0 - SLAM_SAFE_FRACTION) * t)
            } else {
                Vec3::new(t, 1.0, t)
            };
        }
    }
}

/// PRESENTATION: put the danger materials in the viewer's palette, and the hit-flash at the
/// flash-reduction setting. The materials are shared handles, so recoloring every telegraph,
/// shot and beam on the planet is a handful of asset writes — no per-entity work. All of
/// them are unlit, so it is the base color that carries both the hue and the brightness.
pub fn apply_danger_palette(
    save: Res<crate::save::MetaSave>,
    assets: Option<Res<EnemyAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut applied: Local<Option<(Palette, bool)>>,
) {
    let want = (save.accessibility.palette, save.accessibility.flash_reduction);
    if *applied == Some(want) {
        return;
    }
    let Some(assets) = assets else { return };
    let targets = [&assets.ring_mat, &assets.ring_fill_mat, &assets.proj_mat, &assets.beam_charge_mat, &assets.beam_fire_mat];
    for (handle, base) in targets.into_iter().zip(danger_looks(want.0)) {
        if let Some(m) = materials.get_mut(handle) {
            m.base_color = base;
        }
    }
    if let Some(m) = materials.get_mut(&assets.flash_mat) {
        let g = if want.1 { HIT_FLASH_GREY_REDUCED } else { 1.0 };
        m.base_color = Color::srgb(g, g, g);
    }
    *applied = Some(want);
}

/// Swap the hit-flash material in and out. In photosensitivity mode an enemy may START a
/// flash at most once per PHOTO_MIN_FLASH_INTERVAL: a crowd under an aura otherwise strobes
/// white at the aura's tick rate. A held flash (hits landing faster than it fades) stays
/// steadily white, which is not a flash at all.
pub fn enemy_flash(
    time: Res<Time<Real>>,
    save: Res<crate::save::MetaSave>,
    assets: Res<EnemyAssets>,
    mut q: Query<(Entity, &Enemy, &BaseMat, &mut MeshMaterial3d<StandardMaterial>), Changed<Enemy>>,
    // enemy -> when its current flash began; only enemies flashed in the last interval
    mut recent: Local<HashMap<Entity, f32>>,
) {
    let photo = save.accessibility.photosensitive;
    let now = time.elapsed_secs();
    if photo {
        recent.retain(|_, t| now - *t < PHOTO_MIN_FLASH_INTERVAL);
    } else if !recent.is_empty() {
        recent.clear();
    }
    for (entity, e, base, mut mat) in &mut q {
        if e.flash > 0.0 {
            if mat.0 != assets.flash_mat {
                if photo {
                    if recent.contains_key(&entity) {
                        continue;
                    }
                    recent.insert(entity, now);
                }
                mat.0 = assets.flash_mat.clone();
            }
        } else if mat.0 != base.0 {
            mat.0 = base.0.clone();
        }
    }
}
