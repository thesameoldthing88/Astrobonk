//! The horde: spawning director, great-circle steering with separation,
//! contact damage, special attackers (spitter / UFO / burrower), elites,
//! minibosses, stage bosses, and THE STATIC.

use crate::config::*;
use crate::content::enemies::{time_scaling, BossKind, EliteMods, EnemyKind};
use crate::fx::{self, Pcolor, ParticleAssets, Shake};
use crate::messages::*;
use crate::planet::{random_dir, CurrentPlanet, StageScoped};
use crate::player::Player;
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

const WORM_SEGMENTS: usize = 12;
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
}

/// The visible aim line while a Beamer charges.
#[derive(Component)]
pub struct AimLine {
    pub owner: Entity,
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
    pub elite_timer: f32,
    pub tick: f32,
}

impl Default for Director {
    fn default() -> Self {
        Self { spawn_bank: 0.0, elite_timer: 45.0, tick: 0.0 }
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
fn boss_mesh() -> Mesh {
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
        ring_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.25, 0.1),
            emissive: LinearRgba::rgb(3.0, 0.5, 0.1),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
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

fn spawn_enemy(
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
        cmd.insert(Buried { timer: 1.3 });
    }
    if kind == EnemyKind::Beamer {
        cmd.insert(Beamer { cd: rng.gen_range(2.0..4.0), charging: 0.0, aim: Vec3::ZERO });
    }
    if kind == EnemyKind::Lobber {
        cmd.insert(Lobber { cd: rng.gen_range(2.5..5.0) });
    }
}

/// Timer-driven wave spawner. Runs while playing.
pub fn director_spawn(
    mut commands: Commands,
    time: Res<Time>,
    mut director: ResMut<Director>,
    mut game_rng: ResMut<crate::run::GameRng>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_player: Query<&Player>,
    q_enemies: Query<(), With<Enemy>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(player) = q_player.single() else { return };
    let rng = &mut game_rng.0; // deterministic spawn stream from the run seed

    let alive = q_enemies.iter().count();
    let (hp_mult, dmg_mult) = time_scaling(run.elapsed, run.stats.difficulty);

    let rate = if run.static_active {
        10.0 + run.static_timer * 0.15
    } else {
        // gentler opening so a level-1 player can learn; ramp still bites by mid-game.
        let t = run.elapsed / 60.0;
        (1.0 + t * 2.1) * (1.0 + run.stats.difficulty)
    };
    director.spawn_bank += rate * dt;
    director.tick += dt;
    director.elite_timer -= dt;

    if director.tick < 0.25 {
        return;
    }
    director.tick = 0.0;

    let budget = director.spawn_bank.floor() as usize;
    if budget == 0 {
        return;
    }
    director.spawn_bank -= budget as f32;

    let room = ENEMY_CAP.saturating_sub(alive);
    let n = budget.min(room);
    for _ in 0..n {
        let heading = {
            let (t, b) = sphere::tangent_frame(player.dir);
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            t * a.cos() + b * a.sin()
        };
        let arc = rng.gen_range(SPAWN_ARC_MIN..SPAWN_ARC_MAX);
        let dir = sphere::offset_dir(player.dir, heading, arc, planet.radius);

        if run.static_active {
            spawn_enemy(&mut commands, &assets, &planet, EnemyKind::Ghost, dir, false, hp_mult, dmg_mult, rng);
            continue;
        }

        let mix = EnemyKind::mix(run.elapsed);
        let kind = mix[rng.gen_range(0..mix.len())];
        let mut elite = run.elapsed > 150.0 && rng.gen_bool(0.012);
        if director.elite_timer <= 0.0 {
            elite = true;
            director.elite_timer = 40.0;
        }
        // Burrowers ambush: spawn close.
        let dir = if kind == EnemyKind::Burrower {
            let arc = rng.gen_range(9.0..16.0);
            sphere::offset_dir(player.dir, heading, arc, planet.radius)
        } else {
            dir
        };
        spawn_enemy(&mut commands, &assets, &planet, kind, dir, elite, hp_mult, dmg_mult, rng);
    }
}

pub fn spawn_boss(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    assets: &EnemyAssets,
    planet: &CurrentPlanet,
    player_dir: Vec3,
    kind: BossKind,
    difficulty: f32,
) {
    let def = kind.def();
    let mut rng = rand::thread_rng();
    let heading = {
        let (t, b) = sphere::tangent_frame(player_dir);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        t * a.cos() + b * a.sin()
    };
    let dir = sphere::offset_dir(player_dir, heading, 30.0, planet.radius);
    let hp = def.hp * (1.0 + difficulty);
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
    let contact_dmg = def.damage * (1.0 + difficulty * 0.5);
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
        commands.spawn((
            AnubotBeamVis { boss: head },
            Mesh3d(assets.beam_mesh.clone()),
            MeshMaterial3d(assets.beam_charge_mat.clone()),
            Transform::from_translation(pos),
            Visibility::Hidden,
            StageScoped,
        ));
    }
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
    q_player: Query<&Player>,
    mut q_boss: Query<(&mut Enemy, &mut Boss)>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let Ok(player) = q_player.single() else { return };
    let mut rng = rand::thread_rng();
    let (hp_mult, dmg_mult) = time_scaling(run.elapsed, run.stats.difficulty);

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
        let (t, b) = sphere::tangent_frame(player.dir);
        for i in 0..ring {
            let a = i as f32 / ring as f32 * std::f32::consts::TAU + rng.gen_range(-0.2..0.2);
            let heading = t * a.cos() + b * a.sin();
            let arc = rng.gen_range(SPAWN_ARC_MIN..SPAWN_ARC_MAX);
            let dir = sphere::offset_dir(player.dir, heading, arc, planet.radius);
            let mix = EnemyKind::mix(run.elapsed.max(300.0));
            let kind = mix[rng.gen_range(0..mix.len())];
            let elite = want == 2 && rng.gen_bool(0.25);
            spawn_enemy(&mut commands, &assets, &planet, kind, dir, elite, hp_mult, dmg_mult, &mut rng);
        }
    }
}

/// Judge Anubot's Verdict Beam — a lighthouse railbeam that telegraphs, then sweeps the
/// surface. Idle → charge (dim, slow rotate) → fire (bright, faster, damaging) → idle.
#[allow(clippy::type_complexity)]
pub fn anubot_beam_system(
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    run: Res<RunState>,
    q_player: Query<&Transform, (With<Player>, Without<AnubotBeam>, Without<AnubotBeamVis>)>,
    mut q_boss: Query<(Entity, &mut Transform, &Enemy, &Boss, &mut AnubotBeam)>,
    mut q_vis: Query<
        (&AnubotBeamVis, &mut Transform, &mut Visibility, &mut MeshMaterial3d<StandardMaterial>),
        (Without<AnubotBeam>, Without<Player>),
    >,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let ppos = q_player.single().ok().map(|t| t.translation);
    let t_now = time.elapsed_secs();
    // pass 1: advance each beam, apply damage, snapshot for the visuals
    let mut snap: std::collections::HashMap<Entity, (Vec3, Vec3, u8, f32)> = std::collections::HashMap::new();
    for (e, mut tf, enemy, boss, mut beam) in &mut q_boss {
        beam.timer -= dt;
        let ph = boss.phase as f32;
        // rotate: slow while charging (telegraph), fast while firing; faster each phase
        let spin = match beam.state {
            1 => 0.5,
            2 => 1.15 * (1.0 + 0.3 * ph),
            _ => 0.25,
        };
        beam.angle = (beam.angle + spin * dt) % std::f32::consts::TAU;
        if beam.timer <= 0.0 {
            beam.state = match beam.state {
                0 => {
                    beam.timer = 1.3 - 0.3 * ph; // shorter telegraph as he enrages
                    1
                }
                1 => {
                    beam.timer = 3.0 + 0.6 * ph; // longer sweep
                    2
                }
                _ => {
                    beam.timer = (2.6 - 0.7 * ph).max(0.8); // shorter rest
                    0
                }
            };
        }
        let up = tf.translation.normalize_or_zero();
        let base = sphere::tangent_frame(up).0;
        let heading = (Quat::from_axis_angle(up, beam.angle) * base).normalize_or_zero();

        // damage while firing
        if beam.state == 2 {
            if let Some(pp) = ppos {
                if run.iframes <= 0.0 {
                    let v = pp - tf.translation;
                    let along = v.dot(heading);
                    let perp = (v - heading * along - up * v.dot(up)).length();
                    if along > 0.0 && along < BEAM_LENGTH && perp < BEAM_WIDTH {
                        writer.write(PlayerHitMsg {
                            amount: enemy.damage * 1.2,
                            from: tf.translation,
                            attacker: None,
                        });
                    }
                }
            }
        }
        // Hero-tier body tell (code-art-animation skill): the boss's POSE announces the
        // attack, not just the light — anticipation while charging, follow-through firing.
        // Layers on top of enemy_move's write, which ran earlier in the chain.
        match beam.state {
            1 => {
                // rear up as the charge builds (anticipation)
                let wind = 1.0 - (beam.timer / (1.3 - 0.3 * ph).max(0.2)).clamp(0.0, 1.0);
                tf.rotation *= Quat::from_rotation_x(0.24 * wind);
                let s = 1.0 + 0.07 * wind;
                tf.scale.y *= s;
            }
            2 => {
                // lurch into the sweep + a high-frequency shudder while it fires
                tf.rotation *= Quat::from_rotation_x(-0.14);
                tf.rotation *= Quat::from_rotation_z((t_now * 34.0).sin() * 0.022);
            }
            _ => {}
        }

        snap.insert(e, (tf.translation, heading, beam.state, up.dot(Vec3::Y)));
        let _ = up;
    }

    // pass 2: place / colour / show the beam visuals
    for (vis, mut tf, mut visibility, mut mat) in &mut q_vis {
        let Some((bpos, heading, state, _)) = snap.get(&vis.boss).copied() else {
            *visibility = Visibility::Hidden;
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

/// DEV: press B during play to summon the current planet's stage boss immediately
/// (so the Craterpillar is testable without surviving 8+ minutes). Remove before ship.
pub fn debug_spawn_boss(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    q_player: Query<&Player>,
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
    spawn_boss(&mut commands, &mut meshes, &assets, &planet, p.dir, kind, run.stats.difficulty);
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
    q_player: Query<&Transform, (With<Player>, Without<CraterpillarSegment>, Without<CraterpillarHead>)>,
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
    let ptf = q_player.single().ok().map(|t| t.translation);
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

        if run.iframes <= 0.0 {
            if let Some(pp) = ptf {
                let reach = seg.scale * 0.6 + PLAYER_RADIUS + 0.25;
                if pos.distance_squared(pp) < reach * reach {
                    writer.write(PlayerHitMsg { amount: seg.damage, from: pos, attacker: None });
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
    q_player: Query<(&Player, &Transform), Without<Enemy>>,
    mut q: Query<
        (Entity, &mut Enemy, &mut Transform),
        (Without<Buried>, Without<crate::interact::Pot>),
    >,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };
    let player_dir = player.dir;
    let player_pos = ptf.translation;
    let t_now = time.elapsed_secs();

    for (entity, mut e, mut tf) in &mut q {
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
}

/// Buried burrowers erupt after their telegraph.
pub fn burrower_emerge(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    q_player: Query<&Transform, With<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
    mut q: Query<(Entity, &Enemy, &mut Buried, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(ptf) = q_player.single() else { return };
    for (e, enemy, mut b, mut tf) in &mut q {
        b.timer -= dt;
        // rumble under the surface
        let depth = (b.timer / 1.3).clamp(0.0, 1.0);
        tf.translation = planet.surface_point(enemy.dir) - enemy.dir * (depth * 1.2);
        if b.timer <= 0.0 {
            commands.entity(e).remove::<Buried>();
            // eruption damage if the player is on top of it
            if tf.translation.distance(ptf.translation) < 2.6 {
                writer.write(PlayerHitMsg { amount: enemy.damage, from: tf.translation, attacker: Some(e) });
            }
            commands.spawn((
                Mesh3d(assets.ring_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                Transform::from_translation(planet.surface_point(enemy.dir) + enemy.dir * 0.1)
                    .with_rotation(sphere::frame_quat(enemy.dir, sphere::tangent_frame(enemy.dir).0) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(0.5)),
                Telegraph { timer: 0.25, max: 0.25, radius: 0.0, damage: 0.0, dir: enemy.dir, ring: false },
                StageScoped,
            ));
        }
    }
}

/// Touching the swarm hurts.
pub fn enemy_contact(
    time: Res<Time>,
    run: Res<RunState>,
    q_player: Query<(&Player, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &Transform), Without<Buried>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    if time.delta_secs() <= 0.0 || run.iframes > 0.0 {
        return;
    }
    let Ok((_, ptf)) = q_player.single() else { return };
    for (entity, mut e, tf) in &mut q {
        if e.contact_cd > 0.0 || e.speed == 0.0 {
            continue;
        }
        let reach = e.scale * 0.55 + PLAYER_RADIUS + 0.25;
        if tf.translation.distance_squared(ptf.translation) < reach * reach {
            e.contact_cd = CONTACT_TICK;
            writer.write(PlayerHitMsg { amount: e.damage, from: tf.translation, attacker: Some(entity) });
        }
    }
}

/// Spitters lob shots when in range.
pub fn spitter_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    storm: Res<crate::events_world::DustStorm>,
    q_player: Query<&Player>,
    mut q: Query<(&Enemy, &mut Spitter, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || storm.player_inside {
        return; // hidden in the dust storm — ranged enemies can't see you
    }
    let Ok(player) = q_player.single() else { return };
    for (e, mut s, tf) in &mut q {
        s.cd -= dt;
        if s.cd > 0.0 {
            continue;
        }
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
#[allow(clippy::too_many_arguments)]
pub fn beamer_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    storm: Res<crate::events_world::DustStorm>,
    q_player: Query<(&Player, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &Enemy, &mut Beamer, &Transform), Without<Buried>>,
    mut q_lines: Query<(Entity, &AimLine, &mut Transform), (Without<Enemy>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // Lost the target in the dust — drop every aim line and hold fire.
    if storm.player_inside {
        for (le, _, _) in &q_lines {
            commands.entity(le).despawn();
        }
        for (_, _, mut b, _) in &mut q {
            b.charging = 0.0;
        }
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };

    for (entity, e, mut b, tf) in &mut q {
        let arc = sphere::arc_dist(e.dir, player.dir, planet.radius);
        if b.charging > 0.0 {
            b.charging -= dt;
            // track the player until the final quarter second, then hold the lock
            if b.charging > 0.25 {
                let v = ptf.translation - tf.translation;
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
                // drop the aim line
                for (le, line, _) in q_lines.iter() {
                    if line.owner == entity {
                        commands.entity(le).despawn();
                    }
                }
            }
            continue;
        }
        b.cd -= dt;
        if b.cd <= 0.0 && arc < 26.0 {
            b.charging = 1.1;
            let v = ptf.translation - tf.translation;
            b.aim = (v - e.dir * v.dot(e.dir)).normalize_or_zero();
            commands.spawn((
                AimLine { owner: entity },
                Mesh3d(assets.proj_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                Transform::from_translation(tf.translation),
                StageScoped,
            ));
        }
    }

    // stretch each live aim line from its beamer toward the current aim
    for (le, line, mut ltf) in &mut q_lines {
        let Ok((_, e, b, tf)) = q.get(line.owner) else {
            commands.entity(le).despawn();
            continue;
        };
        if b.aim == Vec3::ZERO {
            continue;
        }
        let len = 24.0;
        let mid = tf.translation + e.dir * 1.0 + b.aim * (len * 0.5);
        ltf.translation = mid;
        ltf.rotation = sphere::frame_quat(e.dir, b.aim);
        // thin pulsing line, thickening as the shot locks in
        let lock = 1.0 - (b.charging / 1.1).clamp(0.0, 1.0);
        ltf.scale = Vec3::new(0.10 + lock * 0.16, 0.10 + lock * 0.16, len / 0.56);
    }
}

/// Lobbers mortar the player's position: landing telegraph + arcing shell.
pub fn lobber_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    storm: Res<crate::events_world::DustStorm>,
    q_player: Query<&Player>,
    mut q: Query<(&Enemy, &mut Lobber, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || storm.player_inside {
        return; // can't range you through the dust
    }
    let Ok(player) = q_player.single() else { return };
    for (e, mut l, tf) in &mut q {
        l.cd -= dt;
        if l.cd > 0.0 {
            continue;
        }
        let arc = sphere::arc_dist(e.dir, player.dir, planet.radius);
        if arc < 24.0 {
            l.cd = 4.5;
            let target = player.dir;
            let flight = 1.6;
            commands.spawn((
                Mesh3d(assets.ring_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                Transform::from_translation(planet.surface_point(target) + target * 0.15)
                    .with_rotation(sphere::frame_quat(target, sphere::tangent_frame(target).0) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(0.1)),
                Telegraph { timer: flight, max: flight, radius: 3.2, damage: e.damage, dir: target, ring: false },
                StageScoped,
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
    q_player: Query<&Transform, With<Player>>,
    mut q: Query<(Entity, &mut EnemyProjectile, &mut Transform), Without<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(ptf) = q_player.single() else { return };
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
        if run.iframes <= 0.0 && tf.translation.distance_squared(ptf.translation) < 1.1 {
            writer.write(PlayerHitMsg { amount: p.damage, from: tf.translation, attacker: None });
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
            commands.spawn((
                Mesh3d(assets.ring_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                Transform::from_translation(planet.surface_point(e.dir) + e.dir * 0.15)
                    .with_rotation(sphere::frame_quat(e.dir, sphere::tangent_frame(e.dir).0) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(0.1)),
                Telegraph { timer: 1.4, max: 1.4, radius: 7.0, damage: e.damage * 1.6, dir: e.dir, ring: true },
                StageScoped,
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

/// Telegraphs expand, then detonate against the player.
pub fn telegraphs(
    mut commands: Commands,
    time: Res<Time>,
    mut shake: ResMut<Shake>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<&Transform, With<Player>>,
    mut q: Query<(Entity, &mut Telegraph, &mut Transform), Without<Player>>,
    mut writer: MessageWriter<PlayerHitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(ptf) = q_player.single() else { return };
    for (e, mut tg, mut tf) in &mut q {
        tg.timer -= dt;
        let t = 1.0 - (tg.timer / tg.max).clamp(0.0, 1.0);
        tf.scale = Vec3::splat(0.1 + t * tg.radius.max(0.6));
        if tg.timer <= 0.0 {
            if tg.damage > 0.0 {
                let d = tf.translation.distance(ptf.translation);
                let hit = if tg.ring {
                    d < tg.radius + 1.0 && d > tg.radius * 0.35
                } else {
                    d < tg.radius + 0.6
                };
                if hit {
                    writer.write(PlayerHitMsg { amount: tg.damage, from: tf.translation, attacker: None });
                }
                shake.add(0.22);
                if let Some(pa) = &particles {
                    fx::burst(&mut commands, pa, tf.translation, tg.dir, Pcolor::Red, 18, 9.0);
                }
            }
            commands.entity(e).despawn();
        }
    }
}

/// Restore materials after hit-flash.
pub fn enemy_flash(
    assets: Res<EnemyAssets>,
    mut q: Query<(&Enemy, &BaseMat, &mut MeshMaterial3d<StandardMaterial>), Changed<Enemy>>,
) {
    for (e, base, mut mat) in &mut q {
        if e.flash > 0.0 {
            if mat.0 != assets.flash_mat {
                mat.0 = assets.flash_mat.clone();
            }
        } else if mat.0 != base.0 {
            mat.0 = base.0.clone();
        }
    }
}
