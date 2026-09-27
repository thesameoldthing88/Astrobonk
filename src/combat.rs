//! Player weapons: firing, projectiles, beams, orbitals, chains, auras —
//! plus damage resolution for both sides.

use crate::config::*;
use crate::content::weapons::{Behavior, WeaponKind};
use crate::arsenal;
use crate::enemies::{Boss, Buried, Enemy, SpatialHash};
use crate::fx::{self, Hitstop, Pcolor, ParticleAssets, Shake};
use crate::interact::Pot;
use crate::messages::*;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::Player;
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

#[derive(Resource)]
pub struct WeaponAssets {
    pub proj_mesh: Handle<Mesh>,
    pub drone_mesh: Handle<Mesh>,
    pub beam_mesh: Handle<Mesh>,
    /// A melee swing's swoosh: a flat crescent per swing width (degrees, rounded), 360
    /// always present for ring volleys. Unit outer radius, pointing along -Z.
    pub sweep_arcs: HashMap<u32, Handle<Mesh>>,
    /// See-through, glowing: a swing is a streak you read at a glance, not a slab.
    pub sweep_mats: HashMap<WeaponKind, Handle<StandardMaterial>>,
    pub aura_mesh: Handle<Mesh>,
    pub mats: HashMap<WeaponKind, Handle<StandardMaterial>>,
    /// Faint see-through variants for the aura sphere so it doesn't blind the player.
    pub aura_mats: HashMap<WeaponKind, Handle<StandardMaterial>>,
    // ---- the Tier-1 weapons (`arsenal.rs`) and the evolution fanfare ----
    /// A flat disc: a meatball's sauce splat.
    pub splat_mesh: Handle<Mesh>,
    /// A unit torus: toll rings, the nova, the repulsor, bell-mark halos, the fanfare ring.
    pub ring_mesh: Handle<Mesh>,
    /// The Sonic Whoopee's flat fan, unit range, pointing along -Z.
    pub cone_mesh: Handle<Mesh>,
    pub disc_mesh: Handle<Mesh>,
    pub bell_mesh: Handle<Mesh>,
    pub yoyo_mesh: Handle<Mesh>,
    /// The fanfare's gold (dimmed under flash reduction by `apply_weapon_photosensitivity`).
    pub fanfare_mat: Handle<StandardMaterial>,
    /// THE ANGELUS's friendly wisps: warm and see-through — a ghost that is on your side.
    pub wisp_mat: Handle<StandardMaterial>,
}

/// A melee swing's swoosh: a flat crescent (an annular sector from SWEEP_INNER to 1) of
/// `arc_deg` round -Z, both faces. A crescent reads as the arc the weapon travelled; the old
/// solid box read as an orange slab lying on the ground.
fn sweep_arc_mesh(arc_deg: f32) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let n = ((arc_deg / 8.0).ceil() as u32).max(8);
    let half = arc_deg.to_radians() / 2.0;
    let mut pos: Vec<[f32; 3]> = Vec::new();
    for i in 0..=n {
        let (s, c) = (-half + 2.0 * half * i as f32 / n as f32).sin_cos();
        pos.push([s * SWEEP_INNER, 0.0, -c * SWEEP_INNER]);
        pos.push([s, 0.0, -c]);
    }
    let mut idx: Vec<u32> = Vec::new();
    for i in 0..n {
        let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 2, 2 * i + 3);
        idx.extend([a, c, b, b, c, d]);
        idx.extend([a, b, c, b, d, c]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(idx))
}

/// The Whoopee's fan: a flat sector of `arc_deg` round -Z, unit radius, both faces.
fn cone_fan_mesh(arc_deg: f32) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    let n = 14;
    let half = arc_deg.to_radians() / 2.0;
    let mut pos: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0]];
    for i in 0..=n {
        let a = -half + 2.0 * half * i as f32 / n as f32;
        pos.push([a.sin(), 0.0, -a.cos()]);
    }
    let mut idx: Vec<u32> = Vec::new();
    for i in 1..=n as u32 {
        idx.extend([0, i + 1, i]);
        idx.extend([0, i, i + 1]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; pos.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(idx))
}

pub fn setup_weapon_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mats = HashMap::new();
    let mut aura_mats = HashMap::new();
    let mut sweep_mats = HashMap::new();
    let mut sweep_arcs: HashMap<u32, Handle<Mesh>> = HashMap::new();
    sweep_arcs.insert(360, meshes.add(sweep_arc_mesh(360.0)));
    for kind in [
        WeaponKind::Wrench,
        WeaponKind::LaserPistol,
        WeaponKind::RivetGun,
        WeaponKind::Kunai,
        WeaponKind::Boomerang,
        WeaponKind::MiningLaser,
        WeaponKind::Drones,
        WeaponKind::Tesla,
        WeaponKind::RocketPod,
        WeaponKind::CryoVent,
        WeaponKind::MegaWrench,
        WeaponKind::GatlingLaser,
        WeaponKind::Riveter9000,
        WeaponKind::BladeStorm,
        WeaponKind::SatelliteArray,
        WeaponKind::DeathRay,
        WeaponKind::DroneSwarm,
        WeaponKind::StormCore,
        WeaponKind::MirvPod,
        WeaponKind::AbsoluteZero,
        WeaponKind::MeatballComet,
        WeaponKind::StaticCling,
        WeaponKind::RicochetDisc,
        WeaponKind::SonicWhoopee,
        WeaponKind::CosmonautsBell,
        WeaponKind::YoYo,
        WeaponKind::RaguRain,
        WeaponKind::FullDischarge,
        WeaponKind::Omnidisc,
        WeaponKind::BrownNote,
        WeaponKind::Angelus,
        WeaponKind::SwordYo,
    ] {
        let c = kind.def().color;
        mats.insert(
            kind,
            materials.add(StandardMaterial {
                base_color: c,
                emissive: c.to_linear() * 3.0,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        );
        // A faint, see-through version for the aura bubble that surrounds the player. A hug
        // field is fainter still: it sits right round you, and its zaps already say "biting".
        let alpha = if matches!(kind.def().behavior, Behavior::Hug { .. }) { 0.06 } else { 0.12 };
        if let Behavior::MeleeArc { arc_deg, .. } = kind.def().behavior {
            sweep_arcs.entry(arc_deg.round() as u32).or_insert_with(|| meshes.add(sweep_arc_mesh(arc_deg)));
            sweep_mats.insert(
                kind,
                materials.add(StandardMaterial {
                    base_color: c.with_alpha(SWEEP_ALPHA),
                    emissive: c.to_linear() * 1.2,
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                }),
            );
        }
        aura_mats.insert(
            kind,
            materials.add(StandardMaterial {
                base_color: c.with_alpha(alpha),
                emissive: c.to_linear() * 0.6,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
        );
    }
    let gold = Color::srgb(1.0, 0.82, 0.25);
    let whoopee_arc = match WeaponKind::SonicWhoopee.def().behavior {
        Behavior::Cone { arc_deg, .. } => arc_deg,
        _ => 80.0,
    };
    commands.insert_resource(WeaponAssets {
        proj_mesh: meshes.add(Mesh::from(Sphere::new(0.22))),
        drone_mesh: meshes.add(Mesh::from(Cuboid::new(0.4, 0.25, 0.55))),
        beam_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        sweep_arcs,
        sweep_mats,
        aura_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
        splat_mesh: meshes.add(Mesh::from(Cylinder::new(1.0, 0.04))),
        ring_mesh: meshes.add(Mesh::from(Torus::new(0.9, 1.0))),
        cone_mesh: meshes.add(cone_fan_mesh(whoopee_arc)),
        disc_mesh: meshes.add(Mesh::from(Cylinder::new(0.42, 0.09))),
        bell_mesh: meshes.add(Mesh::from(ConicalFrustum { radius_top: 0.12, radius_bottom: 0.4, height: 0.55 })),
        yoyo_mesh: meshes.add(Mesh::from(Cylinder::new(0.34, 0.26))),
        fanfare_mat: materials.add(StandardMaterial {
            base_color: fanfare_gold(false),
            emissive: gold.to_linear() * 4.0,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        wisp_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.93, 0.62, 0.62),
            emissive: LinearRgba::rgb(2.0, 1.8, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
        mats,
        aura_mats,
    })
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProjKind {
    Straight,
    Seek,
    Boomerang { age: f32, out_time: f32 },
    Rocket { aoe: f32 },
}

#[derive(Component)]
pub struct Projectile {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    /// The weapon that fired it — hit attribution (`HitBy`, the §11 duo combos).
    pub weapon: WeaponKind,
    pub dir: Vec3,     // unit direction from core
    pub heading: Vec3, // tangent unit
    pub speed: f32,
    pub damage: f32,
    pub pierce: i32,
    pub life: f32,
    pub size: f32,
    pub kind: ProjKind,
    pub hit_cd: HashMap<Entity, f32>,
    /// Skips off the ground left (Tome of Ricochet): when the shot comes down it takes one
    /// more leg instead of fizzling. Rolled when fired.
    pub bounces: u8,
    /// Seconds left of the skip's hop (0 = flying level).
    pub hop: f32,
}

impl Projectile {
    /// Does this shot skip? One roll per shot, at the owner's Ricochet chance.
    fn roll_bounce(stats: &crate::stats::Stats, rng: &mut impl Rng) -> u8 {
        u8::from(stats.ricochet > 0.0 && rng.gen_bool(stats.ricochet.clamp(0.0, 1.0) as f64))
    }
}

#[derive(Component)]
pub struct Drone {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    pub weapon: WeaponKind,
    pub idx: usize,
    pub count: usize,
    pub damage: f32,
    pub radius: f32,
    pub deg_per_sec: f32,
    pub tick: f32,
    /// Body size (Tome of Orbit swells the drones along with their ring).
    pub scale: f32,
}

#[derive(Component)]
pub struct Beam {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    /// The weapon that fired it — hit attribution (`HitBy`, the §11 duo combos).
    pub weapon: WeaponKind,
    pub heading: Vec3,
    pub range: f32,
    pub width: f32,
    pub damage: f32,
    pub ticks_left: u32,
    pub tick_cd: f32,
    /// Fired by Second Astronaut's ghost: it streams from where the ghost floats.
    pub ghost: bool,
}

#[derive(Component, Clone, Copy)]
pub struct Fader {
    pub life: f32,
    pub max: f32,
    /// A soft fade (photosensitivity mode's chain zaps): the cross-section swells from
    /// nothing to this width and back over the life, so the light ramps instead of popping.
    /// `None` is the canon quick shrink.
    pub swell: Option<f32>,
}

#[derive(Component)]
pub struct AuraVis {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    pub weapon: WeaponKind,
}

/// The foes a weapon can see: every enemy-side body above ground (pots included — they
/// piggyback on `Enemy`; the targeting skips them).
pub type Foes<'w, 's> = Query<'w, 's, (Entity, &'static Transform, &'static Enemy), (Without<Buried>, Without<Player>)>;

/// Nearest live enemy to `pos` within `max_d` meters — pottery is scenery, not a target.
fn nearest_enemy(
    pos: Vec3,
    max_d: f32,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
) -> Option<(Entity, Vec3)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;
    for (e, tf, _) in enemies.iter() {
        if pots.get(e).is_ok() {
            continue;
        }
        let d = tf.translation.distance_squared(pos);
        if d < max_d * max_d && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
            best = Some((e, tf.translation, d));
        }
    }
    best.map(|(e, p, _)| (e, p))
}

pub fn roll_crit(crit_chance: f32, crit_damage: f32, rng: &mut impl Rng) -> (f32, bool) {
    let mut chance = crit_chance;
    let mut mult = 1.0;
    let mut crit = false;
    while chance > 0.0 {
        if rng.gen_bool((chance.min(1.0)) as f64) {
            mult *= crit_damage;
            crit = true;
        }
        chance -= 1.0;
    }
    (mult, crit)
}

/// One volley of one weapon: who fires it, from where, at what, and how hard.
pub struct Volley {
    pub owner: Entity,
    /// The owner's PlayerId (what a wire event names it by).
    pub pid: u8,
    pub kind: WeaponKind,
    pub dmg: f32,
    pub count: u32,
    pub size: f32,
    /// Where the shooter stands (melee reach and chain hops measure from here) and where
    /// its shots leave from (a little above that).
    pub center: Vec3,
    pub origin: Vec3,
    pub up: Vec3,
    pub aim: Vec3,
    /// Anti-Grav Boots, airborne: every directional weapon fires a full 360° ring.
    pub ring: bool,
    /// Second Astronaut's ghost fired this (its beams track the ghost, not the owner).
    pub ghost: bool,
    pub crit_ch: f32,
}

/// What one of `ps`'s weapons fires right now: damage per hit, bodies/projectiles, size.
/// The one formula `weapon_fire` and the always-on weapons (`arsenal::tether_update`) share.
pub fn weapon_numbers(ps: &PlayerState, stats: &crate::stats::Stats, wi: &crate::run::WeaponInstance) -> (f32, u32, f32) {
    let def = wi.kind.def();
    let (lvl_dmg, lvl_extra, lvl_size) = wi.kind.level_scaling(wi.level);
    // Tome of Ascension's later ranks: evolved weapons hit harder
    let evo = if wi.kind.is_evolution() { stats.evo_damage } else { 1.0 };
    let dmg = def.damage * lvl_dmg * ps.damage_mult() * evo;
    let count = (def.projectiles + lvl_extra + stats.projectiles.max(0) as u32).max(1);
    (dmg, count, lvl_size * stats.size)
}

/// Evenly spaced headings all the way round `up`, starting at `aim` (a 360° ring).
pub fn ring_headings(up: Vec3, aim: Vec3, n: u32) -> impl Iterator<Item = Vec3> {
    (0..n).map(move |i| Quat::from_axis_angle(up, i as f32 / n as f32 * std::f32::consts::TAU) * aim)
}

/// How this machine may draw chain-lightning zaps (§13 photosensitivity, P04): the setting
/// plus the one screen-wide flash budget every astronaut's zaps (and the ghost's) share.
pub struct ZapLook<'a> {
    pub photo: bool,
    pub gate: &'a mut fx::FlashGate,
    pub now: f32,
}

/// Fire one volley (every behavior except the always-on Orbit, Aura, Hug and Tether).
#[allow(clippy::too_many_arguments)]
fn fire_volley(
    v: &Volley,
    stats: &crate::stats::Stats,
    assets: &WeaponAssets,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    zaps: &mut ZapLook,
    forces: &mut MessageWriter<crate::coop::FriendlyForce>,
    ax: &mut arsenal::ArsenalCx,
    rng: &mut impl Rng,
) {
    let def = v.kind.def();
    let (up, aim, origin, size, count, dmg) = (v.up, v.aim, v.origin, v.size, v.count, v.dmg);
    let start_dir = v.center.normalize_or_zero();
    match def.behavior {
        Behavior::MeleeArc { arc_deg, range } => {
            let arc_deg = if v.ring { 360.0 } else { arc_deg };
            let r = range * size;
            let cos_half = (arc_deg.to_radians() / 2.0).cos();
            for (e, tf, en) in enemies.iter() {
                let d = tf.translation - v.center;
                if d.length_squared() > r * r {
                    continue;
                }
                let vt = (d - up * d.dot(up)).normalize_or_zero();
                if arc_deg >= 360.0 || vt.dot(aim) > cos_half {
                    let (cm, crit) = roll_crit(v.crit_ch, stats.crit_damage, rng);
                    let elite = if en.elite { stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg {
                        source: Some(v.owner),
                        target: e,
                        amount: dmg * cm * elite,
                        crit,
                        knock: vt * 9.0 * stats.knockback,
                        by: HitBy::Weapon(v.kind),
                    });
                }
            }
            // §11 friendly physics: the swing boops a teammate caught in its arc (no damage)
            forces.write(crate::coop::FriendlyForce {
                from: v.owner,
                at: v.center,
                radius: r,
                cone: (arc_deg < 360.0).then_some((aim, cos_half)),
                kind: crate::coop::ForceKind::Shove(FRIENDLY_SWING_SHOVE * stats.knockback.clamp(0.5, 2.0), 0.0),
            });
            // sweep visual: a crescent over exactly the arc and reach that just hit
            let arc_mesh = assets.sweep_arcs.get(&(arc_deg.round() as u32)).unwrap_or(&assets.sweep_arcs[&360]);
            commands.spawn((
                Mesh3d(arc_mesh.clone()),
                MeshMaterial3d(assets.sweep_mats.get(&v.kind).unwrap_or(&assets.mats[&v.kind]).clone()),
                Transform::from_translation(origin + up * SWEEP_LIFT)
                    .with_rotation(sphere::frame_quat(up, aim))
                    .with_scale(Vec3::new(r, 1.0, r)),
                // a see-through streak must not print a solid shadow on the ground
                bevy::light::NotShadowCaster,
                Fader { life: 0.14, max: 0.14, swell: None },
                StageScoped,
            ));
            sfx.write(SfxMsg(Sfx::Hit));
        }
        Behavior::Shot { speed, pierce, spread_deg } => {
            let headings: Vec<Vec3> = if v.ring {
                ring_headings(up, aim, count.max(ANTIGRAV_RING_SHOTS)).collect()
            } else {
                (0..count)
                    .map(|i| {
                        let ang = if count > 1 {
                            (i as f32 / (count - 1) as f32 - 0.5) * spread_deg.to_radians()
                        } else {
                            rng.gen_range(-0.04..0.04)
                        };
                        Quat::from_axis_angle(up, ang) * aim
                    })
                    .collect()
            };
            for h in headings {
                commands.spawn((
                    Projectile {
                        owner: v.owner,
                        weapon: v.kind,
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: pierce as i32,
                        life: 2.2 * stats.duration,
                        size,
                        kind: ProjKind::Straight,
                        hit_cd: HashMap::new(),
                        bounces: Projectile::roll_bounce(stats, rng),
                        hop: 0.0,
                    },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin).with_scale(Vec3::splat(size)),
                    StageScoped,
                ));
            }
        }
        Behavior::Seek { speed, pierce } => {
            let headings: Vec<Vec3> = if v.ring {
                ring_headings(up, aim, count.max(ANTIGRAV_RING_SHOTS)).collect()
            } else {
                (0..count)
                    .map(|i| Quat::from_axis_angle(up, i as f32 * 0.35 - (count as f32 - 1.0) * 0.175) * aim)
                    .collect()
            };
            for h in headings {
                commands.spawn((
                    Projectile {
                        owner: v.owner,
                        weapon: v.kind,
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: pierce as i32,
                        life: 2.6 * stats.duration,
                        size,
                        kind: ProjKind::Seek,
                        hit_cd: HashMap::new(),
                        bounces: Projectile::roll_bounce(stats, rng),
                        hop: 0.0,
                    },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin).with_scale(Vec3::splat(size * 0.9)),
                    StageScoped,
                ));
            }
        }
        Behavior::Boomerang { speed, range } => {
            let headings: Vec<Vec3> = if v.ring {
                ring_headings(up, aim, count.max(ANTIGRAV_RING_SHOTS)).collect()
            } else {
                (0..count)
                    .map(|i| Quat::from_axis_angle(up, i as f32 * 0.5 - (count as f32 - 1.0) * 0.25) * aim)
                    .collect()
            };
            // Tome of Orbit: a return weapon flies bigger and faster; with the same out-time
            // that also carries it further before it turns for home.
            let size = size * stats.orbit;
            for h in headings {
                let out_time = range / speed;
                commands.spawn((
                    Projectile {
                        owner: v.owner,
                        weapon: v.kind,
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed * stats.orbit,
                        damage: dmg,
                        pierce: 999,
                        life: out_time * 2.4 * stats.duration,
                        size,
                        kind: ProjKind::Boomerang { age: 0.0, out_time },
                        hit_cd: HashMap::new(),
                        bounces: 0, // it comes back; it never comes down
                        hop: 0.0,
                    },
                    Mesh3d(assets.drone_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin).with_scale(Vec3::splat(size)),
                    StageScoped,
                ));
            }
        }
        Behavior::Beam { range, width } => {
            let headings: Vec<Vec3> =
                if v.ring { ring_headings(up, aim, count.max(ANTIGRAV_RING_BEAMS)).collect() } else { vec![aim] };
            for h in headings {
                commands.spawn((
                    Beam {
                        owner: v.owner,
                        weapon: v.kind,
                        heading: h,
                        range: range * size,
                        width: width * size,
                        damage: dmg,
                        ticks_left: (5.0 * stats.duration) as u32 + 1,
                        tick_cd: 0.0,
                        ghost: v.ghost,
                    },
                    Mesh3d(assets.beam_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin),
                    StageScoped,
                ));
            }
        }
        Behavior::Chain { jumps, range, link_range } => {
            let mut chain: Vec<(Entity, Vec3)> = Vec::new();
            let mut from = v.center;
            let mut max_d = range;
            for _ in 0..=(jumps + count - 1) {
                let mut best: Option<(Entity, Vec3, f32)> = None;
                for (e, tf, _) in enemies.iter() {
                    if chain.iter().any(|(ce, _)| *ce == e) || pots.get(e).is_ok() {
                        continue;
                    }
                    let d = tf.translation.distance_squared(from);
                    if d < max_d * max_d && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                        best = Some((e, tf.translation, d));
                    }
                }
                let Some((e, pos, _)) = best else { break };
                chain.push((e, pos));
                from = pos;
                max_d = link_range;
            }
            // Photosensitivity: no strobe. A zap swells in and out over PHOTO_ZAP_SECS in
            // the dimmed material, and the whole screen shows at most one zap per
            // PHOTO_MIN_FLASH_INTERVAL — one budget however many astronauts (the host draws
            // everyone's) and ghosts carry chain weapons. Damage is unaffected.
            let fader = if zaps.photo {
                Fader { life: PHOTO_ZAP_SECS, max: PHOTO_ZAP_SECS, swell: Some(0.12) }
            } else {
                Fader { life: 0.12, max: 0.12, swell: None }
            };
            let zap_w = if zaps.photo { 0.0 } else { 0.12 };
            let show_zaps = !chain.is_empty() && (!zaps.photo || zaps.gate.allow(zaps.now));
            let mut prev = origin;
            for (e, pos) in &chain {
                let (cm, crit) = roll_crit(v.crit_ch, stats.crit_damage, rng);
                let elite = enemies.get(*e).map(|(_, _, en)| if en.elite { stats.elite_damage } else { 1.0 }).unwrap_or(1.0);
                hits.write(HitMsg { source: Some(v.owner), target: *e, amount: dmg * cm * elite, crit, knock: Vec3::ZERO, by: HitBy::Weapon(v.kind) });
                // §11: the arc jolts a teammate standing next to the foe it jumps to
                forces.write(crate::coop::FriendlyForce {
                    from: v.owner,
                    at: *pos,
                    radius: FRIENDLY_JOLT_RADIUS,
                    cone: None,
                    kind: crate::coop::ForceKind::Jolt,
                });
                // zap segment visual
                if show_zaps {
                    let mid = (prev + *pos) / 2.0;
                    let len = prev.distance(*pos);
                    let dirv = (*pos - prev).normalize_or_zero();
                    commands.spawn((
                        Mesh3d(assets.beam_mesh.clone()),
                        MeshMaterial3d(assets.mats[&v.kind].clone()),
                        Transform::from_translation(mid)
                            .with_rotation(Quat::from_rotation_arc(Vec3::Z, dirv))
                            // a soft zap starts as a hairline and swells in
                            .with_scale(Vec3::new(zap_w, zap_w, len)),
                        fader,
                        StageScoped,
                    ));
                }
                prev = *pos;
            }
            if !chain.is_empty() {
                sfx.write(SfxMsg(Sfx::Hit));
            }
        }
        Behavior::Rocket { speed, aoe } => {
            let headings: Vec<Vec3> = if v.ring {
                ring_headings(up, aim, count.max(ANTIGRAV_RING_SHOTS)).collect()
            } else {
                (0..count).map(|_| Quat::from_axis_angle(up, rng.gen_range(-0.6..0.6)) * aim).collect()
            };
            for h in headings {
                commands.spawn((
                    Projectile {
                        owner: v.owner,
                        weapon: v.kind,
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: 0,
                        life: 4.0 * stats.duration,
                        size,
                        kind: ProjKind::Rocket { aoe: aoe * size },
                        hit_cd: HashMap::new(),
                        bounces: 0, // a rocket that comes down goes off
                        hop: 0.0,
                    },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin).with_scale(Vec3::splat(size * 1.3)),
                    StageScoped,
                ));
            }
        }
        // ---- the Tier-1 weapons' own behaviours (arsenal.rs) ----
        Behavior::Lob { range, flight, aoe, split } => {
            arsenal::fire_lob(v, range, flight, aoe, split, assets, enemies, pots, commands, ax, rng);
        }
        Behavior::Disc { speed, bounces, link } => {
            arsenal::fire_disc(v, speed, bounces, link, stats, assets, commands, sfx, ax);
        }
        Behavior::Cone { arc_deg, range } => {
            arsenal::fire_cone(v, arc_deg, range, stats, assets, enemies, pots, commands, hits, sfx, ax, rng);
        }
        Behavior::Repulsor { radius } => {
            arsenal::fire_repulsor(v, radius, assets, commands, sfx, ax);
        }
        Behavior::Toll { radius, wisps } => {
            arsenal::fire_toll(v, v.pid, radius, wisps, stats, assets, enemies, pots, commands, hits, sfx, ax, rng);
        }
        Behavior::Orbit { .. } | Behavior::Aura { .. } | Behavior::Hug { .. } | Behavior::Tether { .. } => {}
    }
}

/// Where a volley from `center` aims: the nearest enemy in range, flattened onto the local
/// horizontal, else straight ahead.
fn aim_from(
    center: Vec3,
    up: Vec3,
    facing: Vec3,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
) -> Vec3 {
    let aim = match nearest_enemy(center, 40.0, enemies, pots) {
        Some((_, tpos)) => {
            let v = tpos - center;
            (v - up * v.dot(up)).normalize_or_zero()
        }
        None => facing,
    };
    if aim == Vec3::ZERO { facing } else { aim }
}

/// The weapons whose visuals strobe: chain lightning re-zaps at the fire rate, and the
/// DEATH RAY is the brightest bloom source in the game (§13: "disables Storm Core strobe,
/// softens Death Ray bloom").
const STROBING_WEAPONS: [WeaponKind; 3] = [WeaponKind::Tesla, WeaponKind::StormCore, WeaponKind::DeathRay];

/// PRESENTATION: photosensitivity mode turns the strobing weapons' shared materials into a
/// dim, see-through version of themselves; flash reduction takes the evolution fanfare's
/// gold down from HDR (where it blooms, and keeps its colour through the fanfare's
/// desaturation) to a plain gold. Base color and alpha, not emissive: the weapon materials
/// are unlit, and an unlit material draws its base color and nothing else.
pub fn apply_weapon_photosensitivity(
    save: Res<crate::save::MetaSave>,
    assets: Option<Res<WeaponAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut applied: Local<Option<(bool, bool)>>,
) {
    let want = (save.accessibility.photosensitive, save.accessibility.flash_reduction);
    if *applied == Some(want) {
        return;
    }
    let Some(assets) = assets else { return };
    let (photo, reduced) = want;
    for kind in STROBING_WEAPONS {
        if let Some(m) = assets.mats.get(&kind).and_then(|h| materials.get_mut(h)) {
            let c = kind.def().color;
            m.base_color = if photo { c.with_alpha(PHOTO_WEAPON_ALPHA) } else { c };
        }
    }
    if let Some(m) = materials.get_mut(&assets.fanfare_mat) {
        m.base_color = fanfare_gold(reduced);
    }
    *applied = Some(want);
}

/// The fanfare's gold: HDR at rest, plain under flash reduction.
fn fanfare_gold(reduced: bool) -> Color {
    if reduced {
        Color::linear_rgb(0.85, 0.55, 0.08)
    } else {
        Color::linear_rgb(3.2, 2.0, 0.35)
    }
}

/// Tick weapon cooldowns and fire.
///
/// Runs on every machine for every astronaut it has a sheet for — authoritative on the
/// host, the owner's cosmetic prediction on a client — so the §7 firing rules live here
/// too: The Overheat's jam, Anti-Grav Boots' airborne 360° ring, and Second Astronaut's
/// ghost volley.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn weapon_fire(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    particles: Option<Res<ParticleAssets>>,
    (mut telemetry, mut arsenal_tm): (ResMut<crate::items::ItemTelemetry>, ResMut<arsenal::ArsenalTelemetry>),
    (save, mut flash_gate, mut recent_kills): (Res<crate::save::MetaSave>, ResMut<fx::FlashGate>, ResMut<arsenal::RecentKills>),
    mut q_player: Query<(
        Entity,
        &Player,
        &crate::player::PlayerId,
        &mut PlayerState,
        &mut crate::items::ItemProcs,
        &mut arsenal::WeaponProcs,
        &Transform,
    )>,
    enemies: Foes,
    q_pots: Query<(), With<Pot>>,
    (q_drones, q_auras, q_discs): (Query<(Entity, &Drone)>, Query<(Entity, &AuraVis)>, Query<&arsenal::Disc>),
    (mut hits, mut sfx, mut weapon_fx, mut forces): (
        MessageWriter<HitMsg>,
        MessageWriter<SfxMsg>,
        MessageWriter<arsenal::WeaponFxMsg>,
        MessageWriter<crate::coop::FriendlyForce>,
    ),
    role: Res<crate::net::NetRole>,
    (hash, planet): (Res<SpatialHash>, Res<CurrentPlanet>),
) {
    use crate::content::items::ItemKind;
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let t_now = time.elapsed_secs();
    let simulating = role.simulates();
    let mut zaps = ZapLook { photo: save.accessibility.photosensitive, gate: &mut *flash_gate, now: t_now };
    let mut live_discs: HashMap<Entity, usize> = HashMap::new();
    for d in &q_discs {
        *live_discs.entry(d.owner).or_insert(0) += 1;
    }
    let mut ax = arsenal::ArsenalCx {
        hash: &hash,
        planet: &planet,
        kills: &mut recent_kills,
        fx: &mut weapon_fx,
        tm: &mut arsenal_tm,
        simulating,
        now: t_now,
        live_discs: &live_discs,
        tolled: false,
    };

    // Reconcile drone + aura entities with owned weapons — keyed by (owner, weapon).
    let mut want_drones: HashMap<(Entity, WeaponKind), (usize, f32, f32, f32, f32)> = HashMap::new();
    let mut want_auras: Vec<(Entity, WeaponKind)> = Vec::new();
    let mut player_pos: Vec<(Entity, Vec3)> = Vec::new();

    for (pe, player, pid, mut run, mut procs, mut wprocs, ptf) in &mut q_player {
    if run.dead {
        continue; // a downed astronaut stops firing
    }
    player_pos.push((pe, ptf.translation));
    let atk_speed = run.attack_speed();
    let stats = run.live_stats();
    let crit_ch = run.crit_chance(); // captured before the &mut weapons loop (Reticle pulse)
    let aura_sc = run.aura_scale(); // Aurora's fields swell while sprinting
    let overheat = run.has_item(ItemKind::TheOverheat);
    let ring = run.airborne && run.has_item(ItemKind::AntiGravBoots);
    procs.jam = (procs.jam - dt).max(0.0);
    let jammed = procs.jam > 0.0;
    let up = player.dir;
    let origin = ptf.translation + up * 0.4;
    let numbers: Vec<(f32, u32, f32)> = run.weapons.iter().map(|wi| weapon_numbers(&run, &stats, wi)).collect();

    for (wi, &(dmg, count, size)) in run.weapons.iter_mut().zip(&numbers) {
        let def = wi.kind.def();

        match def.behavior {
            Behavior::Orbit { radius, deg_per_sec } => {
                // Tome of Orbit: a wider ring of bigger bodies, swung faster
                let orbit = stats.orbit;
                want_drones.insert((pe, wi.kind), (count as usize, dmg, radius * size * orbit, deg_per_sec * orbit, orbit));
                continue;
            }
            // the yo-yos are always out (`arsenal::tether_update` swings them)
            Behavior::Tether { .. } => continue,
            Behavior::Aura { radius, slow } => {
                // (the slow on foes rides on the hit: `apply_hits` reads it off `HitMsg::weapon`)
                let radius = radius * aura_sc;
                want_auras.push((pe, wi.kind));
                if jammed {
                    continue; // The Overheat: a jammed suit's fields stop pulsing too
                }
                wi.cd -= dt * atk_speed;
                if wi.cd <= 0.0 {
                    wi.cd = def.cooldown;
                    let r = radius * size;
                    // §11: a cryo field chills a teammate standing in it too
                    if slow > 0.0 {
                        forces.write(crate::coop::FriendlyForce {
                            from: pe,
                            at: ptf.translation,
                            radius: r,
                            cone: None,
                            kind: crate::coop::ForceKind::Chill(def.cooldown / atk_speed.max(0.1) + FRIENDLY_CHILL_SECS),
                        });
                    }
                    for (e, tf, en) in enemies.iter() {
                        if tf.translation.distance_squared(ptf.translation) < r * r {
                            let (cm, crit) = roll_crit(crit_ch, stats.crit_damage, &mut rng);
                            let elite = if en.elite { stats.elite_damage } else { 1.0 };
                            hits.write(HitMsg {
                                source: Some(pe),
                                target: e,
                                amount: dmg * cm * elite,
                                crit,
                                knock: Vec3::ZERO,
                                by: HitBy::Weapon(wi.kind),
                            });
                        }
                    }
                }
                continue;
            }
            Behavior::Hug { radius, nova } => {
                want_auras.push((pe, wi.kind));
                if jammed {
                    continue;
                }
                wi.cd -= dt * atk_speed;
                if wi.cd <= 0.0 {
                    wi.cd = def.cooldown;
                    let r = radius * aura_sc * size;
                    arsenal::hug_pulse(
                        pe, wi.kind, ptf.translation, r, dmg, crit_ch, &stats, &enemies, &q_pots, &hash, &assets,
                        &mut commands, &mut hits, &mut zaps, ax.tm, &mut rng,
                    );
                }
                // FULL DISCHARGE: the field stores charge and lets it all go at once
                if nova > 0.0 {
                    wprocs.discharge += dt * atk_speed;
                    if wprocs.discharge >= FULL_DISCHARGE_SECS {
                        wprocs.discharge = 0.0;
                        arsenal::discharge_nova(
                            pe, wi.kind, ptf.translation, nova * size, dmg, crit_ch, &stats, &enemies, &q_pots, &planet,
                            &assets, &mut commands, &mut hits, &mut sfx, ax.tm, &mut rng,
                        );
                    }
                }
                continue;
            }
            _ => {}
        }

        if jammed {
            continue;
        }
        // Sonic Whoopee is the panic button: mobbed, it recharges several times faster
        let panic = matches!(def.behavior, Behavior::Cone { .. } | Behavior::Repulsor { .. })
            && hash
                .near(ptf.translation, WHOOPEE_PANIC_RADIUS)
                .filter(|(e, p)| {
                    p.distance_squared(ptf.translation) < WHOOPEE_PANIC_RADIUS * WHOOPEE_PANIC_RADIUS
                        && enemies.get(*e).is_ok_and(|(_, _, en)| en.speed > 0.0)
                })
                .count()
                >= WHOOPEE_PANIC_CROWD;
        if panic {
            ax.tm.panic_secs += dt;
        }
        wi.cd -= dt * atk_speed * if panic { WHOOPEE_PANIC_RATE } else { 1.0 };
        if wi.cd > 0.0 {
            continue;
        }
        wi.cd = def.cooldown;

        let aim = aim_from(ptf.translation, up, player.facing, &enemies, &q_pots);
        let v = Volley {
            owner: pe,
            pid: pid.0,
            kind: wi.kind,
            dmg,
            count,
            size,
            center: ptf.translation,
            origin,
            up,
            aim,
            ring,
            ghost: false,
            crit_ch,
        };
        fire_volley(&v, &stats, &assets, &enemies, &q_pots, &mut commands, &mut hits, &mut sfx, &mut zaps, &mut forces, &mut ax, &mut rng);
        if ax.tolled {
            ax.tolled = false;
            wprocs.last_toll = t_now;
        }
        if ring {
            telemetry.ring_volleys += 1;
        }
        // The Overheat: every Nth volley jams every gun for a beat. The HOST counts; a
        // client's cosmetic copy takes the jam from the host (`net::adopt_my_item_vis`),
        // or its own count would jam its guns at moments the real ones keep firing.
        procs.volleys += 1;
        if overheat && simulating && procs.volleys % OVERHEAT_JAM_EVERY == 0 {
            procs.jam = OVERHEAT_JAM_SECS;
            telemetry.jams += 1;
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, origin, up, Pcolor::White, 10, 3.5);
            }
            break;
        }
    }

    // Second Astronaut: the ghost co-pilot fires its mirrored weapon on its own cadence,
    // from where it floats, at a share of the weapon's damage. It mirrors one of OUR guns
    // (and fires at our Overheat-boosted speed), so an Overheat jam — including one this
    // very frame — silences it too.
    let ghost_power = run.item_power(ItemKind::SecondAstronaut);
    let mirrored = run.ghost_weapon.and_then(|g| run.weapons.iter().find(|w| w.kind == g).cloned());
    if let (true, Some(wi), false) = (ghost_power > 0.0, mirrored, procs.jam > 0.0) {
        procs.ghost_cd -= dt * atk_speed;
        if procs.ghost_cd <= 0.0 {
            procs.ghost_cd = wi.kind.def().cooldown;
            let (dmg, count, size) = weapon_numbers(&run, &stats, &wi);
            let gpos = crate::items::ghost_anchor(ptf, t_now);
            let gup = gpos.normalize_or_zero();
            let v = Volley {
                owner: pe,
                pid: pid.0,
                kind: wi.kind,
                dmg: dmg * GHOST_MIRROR * ghost_power,
                count,
                size,
                center: gpos,
                origin: gpos,
                up: gup,
                aim: aim_from(gpos, gup, player.facing, &enemies, &q_pots),
                ring: false,
                ghost: true,
                crit_ch,
            };
            fire_volley(&v, &stats, &assets, &enemies, &q_pots, &mut commands, &mut hits, &mut sfx, &mut zaps, &mut forces, &mut ax, &mut rng);
            ax.tolled = false;
            telemetry.ghost_volleys += 1;
        }
    }
    }

    // ------ drone reconciliation
    let mut have: HashMap<(Entity, WeaponKind), usize> = HashMap::new();
    for (e, d) in q_drones.iter() {
        let alive = want_drones.get(&(d.owner, d.weapon));
        match alive {
            Some((want_count, dmg, radius, dps, scale)) => {
                have.entry((d.owner, d.weapon)).and_modify(|c| *c += 1).or_insert(1);
                if d.idx >= *want_count {
                    commands.entity(e).despawn();
                } else {
                    // keep tuning current
                    commands.entity(e).insert(Drone {
                        owner: d.owner,
                        weapon: d.weapon,
                        idx: d.idx,
                        count: *want_count,
                        damage: *dmg,
                        radius: *radius,
                        deg_per_sec: *dps,
                        tick: d.tick,
                        scale: *scale,
                    });
                }
            }
            None => {
                commands.entity(e).despawn();
            }
        }
    }
    for ((owner, kind), (count, dmg, radius, dps, scale)) in &want_drones {
        let existing = have.get(&(*owner, *kind)).copied().unwrap_or(0);
        let home = player_pos.iter().find(|(e, _)| e == owner).map(|(_, p)| *p).unwrap_or(Vec3::ZERO);
        for idx in existing..*count {
            commands.spawn((
                Drone {
                    owner: *owner,
                    weapon: *kind,
                    idx,
                    count: *count,
                    damage: *dmg,
                    radius: *radius,
                    deg_per_sec: *dps,
                    tick: 0.0,
                    scale: *scale,
                },
                Mesh3d(assets.drone_mesh.clone()),
                MeshMaterial3d(assets.mats[kind].clone()),
                Transform::from_translation(home),
                StageScoped,
            ));
        }
    }

    // ------ aura visuals reconciliation
    for (e, a) in q_auras.iter() {
        if !want_auras.contains(&(a.owner, a.weapon)) {
            commands.entity(e).despawn();
        }
    }
    let existing_auras: Vec<(Entity, WeaponKind)> = q_auras.iter().map(|(_, a)| (a.owner, a.weapon)).collect();
    for (owner, kind) in want_auras {
        if !existing_auras.contains(&(owner, kind)) {
            if let Some(radius) = field_radius(kind) {
                let home = player_pos.iter().find(|(e, _)| *e == owner).map(|(_, p)| *p).unwrap_or(Vec3::ZERO);
                commands.spawn((
                    AuraVis { owner, weapon: kind },
                    Mesh3d(assets.aura_mesh.clone()),
                    MeshMaterial3d(assets.aura_mats[&kind].clone()),
                    Transform::from_translation(home).with_scale(Vec3::splat(radius)),
                    StageScoped,
                ));
            }
        }
    }
}

/// Move player projectiles, collide with the swarm via the spatial hash.
pub fn projectile_move(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<(&Player, &PlayerState, &Transform), Without<Projectile>>,
    enemies: Query<&Enemy>,
    q_pots: Query<(), With<Pot>>,
    mut q: Query<(Entity, &mut Projectile, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
    mut forces: MessageWriter<crate::coop::FriendlyForce>,
    // §9 Magnetar elites (P10): a client's proxies carry their affixes too, so its own
    // shots bend exactly as the host's copy of them does
    (q_magnetars, mut affix_tm): (Query<(&Transform, &crate::affixes::Affixes), (With<Enemy>, Without<Projectile>)>, ResMut<crate::affixes::AffixTelemetry>),
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let magnetars: Vec<Vec3> = q_magnetars
        .iter()
        .filter(|(_, a)| a.0.has(crate::content::enemies::Affix::Magnetar))
        .map(|(t, _)| t.translation)
        .collect();

    for (pe, mut p, mut tf) in &mut q {
        let Ok((player, run, ptf)) = q_player.get(p.owner) else {
            commands.entity(pe).despawn();
            continue;
        };
        p.life -= dt;

        // steering per kind
        match p.kind {
            ProjKind::Seek | ProjKind::Rocket { .. } => {
                // home to nearest via hash (radius 14); pottery is not a target
                let mut best: Option<(Vec3, f32)> = None;
                for (he, pos) in hash.near(tf.translation, 14.0) {
                    if q_pots.get(he).is_ok() {
                        continue;
                    }
                    let d = pos.distance_squared(tf.translation);
                    if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                        best = Some((pos, d));
                    }
                }
                if let Some((tpos, _)) = best {
                    let v = tpos - tf.translation;
                    let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
                    if vt != Vec3::ZERO {
                        let turn = if matches!(p.kind, ProjKind::Rocket { .. }) { 6.0 } else { 10.0 };
                        p.heading = (p.heading + vt * turn * dt).normalize();
                    }
                }
            }
            ProjKind::Boomerang { ref mut age, out_time } => {
                *age += dt;
                if *age > out_time {
                    let v = ptf.translation - tf.translation;
                    let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
                    if vt != Vec3::ZERO {
                        p.heading = (p.heading + vt * 8.0 * dt).normalize();
                    }
                    if v.length() < 1.2 {
                        commands.entity(pe).despawn();
                        continue;
                    }
                }
            }
            ProjKind::Straight => {}
        }
        // Magnetar: shots passing near one curve into it (seekers home through)
        if !magnetars.is_empty() {
            let seeker = matches!(p.kind, ProjKind::Seek | ProjKind::Rocket { .. });
            let up = p.dir;
            if crate::affixes::magnetar_bend(&mut p.heading, tf.translation, up, &magnetars, seeker, dt) {
                affix_tm.bends += 1;
            }
        }

        if p.life <= 0.0 && p.bounces > 0 {
            // Tome of Ricochet: the shot comes down and skips off the ground once — a fresh
            // leg, turned toward the nearest foe it can find (pottery is not a target).
            p.bounces -= 1;
            p.life = RICOCHET_LIFE * run.stats.duration;
            p.hop = RICOCHET_HOP_SECS;
            p.hit_cd.clear();
            p.pierce = p.pierce.max(0);
            let here = tf.translation;
            let target = hash
                .near(here, RICOCHET_SEEK)
                .filter(|(he, _)| q_pots.get(*he).is_err() && enemies.get(*he).is_ok_and(|en| en.speed > 0.0))
                .min_by(|a, b| a.1.distance_squared(here).total_cmp(&b.1.distance_squared(here)));
            if let Some((_, tpos)) = target {
                let v = tpos - here;
                let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
                if vt != Vec3::ZERO {
                    p.heading = vt;
                }
            }
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, planet.surface_point(p.dir), p.dir, Pcolor::White, 5, 3.0);
            }
        }
        if p.life <= 0.0 {
            if let ProjKind::Rocket { aoe } = p.kind {
                explode(&mut commands, &hash, &enemies, &run, (p.owner, p.weapon), tf.translation, aoe, p.damage, &mut hits, &mut forces, &particles, p.dir, &mut rng);
            }
            commands.entity(pe).despawn();
            continue;
        }

        // advance along sphere
        let r = planet.surface(p.dir) + 0.9;
        let vel = p.heading * p.speed;
        let (nd, nv) = sphere::advance(p.dir, vel, r, dt);
        p.dir = nd;
        p.heading = nv.normalize_or_zero();
        // a skip arcs up off the ground and back down onto its new line
        p.hop = (p.hop - dt).max(0.0);
        let hop = if p.hop > 0.0 {
            RICOCHET_HOP * (std::f32::consts::PI * (1.0 - p.hop / RICOCHET_HOP_SECS)).sin()
        } else {
            0.0
        };
        tf.translation = planet.surface_point(p.dir) + p.dir * (0.9 + hop);
        tf.rotation = sphere::frame_quat(p.dir, p.heading) * Quat::from_rotation_x((time.elapsed_secs() * 14.0).sin() * 0.3);

        // decay per-target re-hit cooldowns
        p.hit_cd.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });

        // collisions
        let hit_r = 0.45 * p.size + 0.55;
        let mut exploded = false;
        let candidates: Vec<(Entity, Vec3)> = hash.near(tf.translation, hit_r + 1.2).collect();
        for (te, tpos) in candidates {
            if p.hit_cd.contains_key(&te) {
                continue;
            }
            let Ok(en) = enemies.get(te) else { continue };
            let reach = hit_r + en.scale * 0.5;
            if tpos.distance_squared(tf.translation) < reach * reach {
                if let ProjKind::Rocket { aoe } = p.kind {
                    explode(&mut commands, &hash, &enemies, &run, (p.owner, p.weapon), tf.translation, aoe, p.damage, &mut hits, &mut forces, &particles, p.dir, &mut rng);
                    exploded = true;
                    break;
                }
                let (cm, crit) = roll_crit(run.crit_chance(), run.crit_damage(), &mut rng);
                let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                hits.write(HitMsg {
                    source: Some(p.owner),
                    target: te,
                    amount: p.damage * cm * elite,
                    crit,
                    knock: p.heading * 4.0 * run.stats.knockback,
                    by: HitBy::Weapon(p.weapon),
                });
                p.hit_cd.insert(te, 0.5);
                p.pierce -= 1;
                if p.pierce < 0 {
                    commands.entity(pe).despawn();
                    break;
                }
            }
        }
        if exploded {
            commands.entity(pe).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn explode(
    commands: &mut Commands,
    hash: &SpatialHash,
    enemies: &Query<&Enemy>,
    run: &PlayerState,
    // the astronaut whose rocket this was, and the weapon — so AoE damage credits the
    // right player and family
    (owner, weapon): (Entity, WeaponKind),
    pos: Vec3,
    aoe: f32,
    damage: f32,
    hits: &mut MessageWriter<HitMsg>,
    forces: &mut MessageWriter<crate::coop::FriendlyForce>,
    particles: &Option<Res<ParticleAssets>>,
    up: Vec3,
    rng: &mut impl Rng,
) {
    for (te, tpos) in hash.near(pos, aoe + 1.0) {
        let Ok(en) = enemies.get(te) else { continue };
        if tpos.distance_squared(pos) < (aoe + en.scale * 0.5) * (aoe + en.scale * 0.5) {
            let (cm, crit) = roll_crit(run.crit_chance(), run.crit_damage(), rng);
            let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
            let kdir = (tpos - pos).normalize_or_zero();
            hits.write(HitMsg { source: Some(owner), target: te, amount: damage * cm * elite, crit, knock: kdir * 7.0, by: HitBy::Weapon(weapon) });
        }
    }
    // §11 friendly physics: the blast throws a teammate too — never hurts them
    forces.write(crate::coop::FriendlyForce {
        from: owner,
        at: pos,
        radius: aoe,
        cone: None,
        kind: crate::coop::ForceKind::Shove(FRIENDLY_BLAST_SHOVE, FRIENDLY_POP),
    });
    if let Some(pa) = particles {
        fx::burst(commands, pa, pos, up, Pcolor::Red, 16, 8.0);
    }
}

/// Drones orbit and bonk what they touch.
pub fn drone_update(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    q_player: Query<(&Player, &PlayerState, &Transform), Without<Drone>>,
    enemies: Query<&Enemy>,
    mut q: Query<(&mut Drone, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let t = time.elapsed_secs();

    for (mut d, mut tf) in &mut q {
        let Ok((player, run, ptf)) = q_player.get(d.owner) else { continue };
        d.tick = (d.tick - dt).max(0.0);
        let base = t * d.deg_per_sec.to_radians() + d.idx as f32 / d.count.max(1) as f32 * std::f32::consts::TAU;
        let up = player.dir;
        let (tan, bit) = sphere::tangent_frame(up);
        let offset = (tan * base.cos() + bit * base.sin()) * d.radius;
        tf.translation = ptf.translation + offset + up * 0.6;
        tf.rotation = sphere::frame_quat(up, offset.normalize_or_zero());
        tf.scale = Vec3::splat(d.scale);

        if d.tick <= 0.0 {
            let mut hit_any = false;
            for (te, tpos) in hash.near(tf.translation, 0.8 * d.scale + 0.8) {
                let Ok(en) = enemies.get(te) else { continue };
                let reach = 0.8 * d.scale + en.scale * 0.5;
                if tpos.distance_squared(tf.translation) < reach * reach {
                    let (cm, crit) = roll_crit(run.crit_chance(), run.crit_damage(), &mut rng);
                    let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg {
                        source: Some(d.owner),
                        target: te,
                        amount: d.damage * cm * elite,
                        crit,
                        knock: offset.normalize_or_zero() * 5.0,
                        by: HitBy::Weapon(d.weapon),
                    });
                    hit_any = true;
                }
            }
            if hit_any {
                d.tick = 0.3;
            }
        }
    }
}

/// Beams tick damage along a corridor, stretch their visual, then fade.
pub fn beam_update(
    mut commands: Commands,
    time: Res<Time>,
    q_player: Query<(&Player, &PlayerState, &Transform), Without<Beam>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Beam>, Without<Player>)>,
    mut q: Query<(Entity, &mut Beam, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();

    for (e, mut beam, mut tf) in &mut q {
        let Ok((player, run, ptf)) = q_player.get(beam.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        beam.tick_cd -= dt;
        let up = player.dir;
        let origin = if beam.ghost {
            crate::items::ghost_anchor(ptf, time.elapsed_secs())
        } else {
            ptf.translation + up * 0.6
        };
        tf.translation = origin + beam.heading * beam.range * 0.5;
        tf.rotation = sphere::frame_quat(up, beam.heading);
        tf.scale = Vec3::new(beam.width, beam.width, beam.range);

        if beam.tick_cd <= 0.0 {
            beam.tick_cd = 0.12;
            if beam.ticks_left == 0 {
                commands.entity(e).despawn();
                continue;
            }
            beam.ticks_left -= 1;
            for (te, ttf, en) in enemies.iter() {
                let v = ttf.translation - origin;
                let along = v.dot(beam.heading);
                if along < 0.0 || along > beam.range {
                    continue;
                }
                let perp = (v - beam.heading * along - up * v.dot(up)).length();
                if perp < beam.width + en.scale * 0.5 {
                    let (cm, crit) = roll_crit(run.crit_chance(), run.crit_damage(), &mut rng);
                    let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg { source: Some(beam.owner), target: te, amount: beam.damage * cm * elite, crit, knock: Vec3::ZERO, by: HitBy::Weapon(beam.weapon) });
                }
            }
        }
    }
}

/// Aura visuals follow the player and pulse; cryo slow applied on hit below.
pub fn aura_follow(
    time: Res<Time>,
    q_player: Query<(&PlayerState, &Transform), (With<Player>, Without<AuraVis>)>,
    mut q: Query<(&AuraVis, &mut Transform), Without<Player>>,
) {
    let t = time.elapsed_secs();
    for (a, mut tf) in &mut q {
        let Ok((run, ptf)) = q_player.get(a.owner) else { continue };
        let Some(radius) = field_radius(a.weapon) else { continue };
        // drawn at the radius it DAMAGES at: level and the Size stat grow it too
        let size = run
            .weapons
            .iter()
            .find(|w| w.kind == a.weapon)
            .map(|w| w.kind.level_scaling(w.level).2)
            .unwrap_or(1.0)
            * run.stats.size;
        // a hug field crackles; a cold one breathes
        let pulse = if matches!(a.weapon.def().behavior, Behavior::Hug { .. }) {
            1.0 + ((t * 31.0).sin() * (t * 17.0).cos()) * 0.03
        } else {
            1.0 + (t * 3.0).sin() * 0.04
        };
        tf.translation = ptf.translation;
        tf.scale = Vec3::splat(radius * run.aura_scale() * size * pulse);
    }
}

/// The radius a field weapon's bubble is drawn at (before level and size).
fn field_radius(kind: WeaponKind) -> Option<f32> {
    match kind.def().behavior {
        Behavior::Aura { radius, .. } | Behavior::Hug { radius, .. } => Some(radius),
        _ => None,
    }
}

/// Visual-only entities fade out and die.
pub fn fader_update(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Fader, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut f, mut tf) in &mut q {
        f.life -= dt;
        if f.life <= 0.0 {
            commands.entity(e).despawn();
        } else if let Some(w) = f.swell {
            let k = (std::f32::consts::PI * f.life / f.max).sin() * w;
            tf.scale.x = k;
            tf.scale.y = k;
        } else {
            let t = (f.life / f.max).max(0.0);
            tf.scale *= 0.9 + t * 0.1;
        }
    }
}

/// Resolve HitMsg against enemies / bosses / pots: damage, flash, knockback,
/// lifesteal, slows, deaths — and what a hit's weapon does beyond damage: a bell mark's
/// extra crit (anyone's hit), a Whoopee's stun, a cryo vent's own slow. A killing blow by
/// THIS machine's astronaut asks for the §13 hitstop (sparse; solo only — `fx::Hitstop`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn apply_hits(
    mut commands: Commands,
    mut reader: MessageReader<HitMsg>,
    mut run: ResMut<RunState>,
    mut q_ps: Query<(&mut PlayerState, &crate::player::PlayerId)>,
    q_local: Query<(), With<crate::player::LocalPlayer>>,
    (mut shake, mut hitstop, time, role): (ResMut<Shake>, ResMut<Hitstop>, Res<Time>, Res<crate::net::NetRole>),
    (mut ledger, mut duos): (ResMut<crate::duos::DuoLedger>, MessageWriter<crate::duos::DuoMsg>),
    mut enemies: Query<
        (
            &mut Enemy,
            &Transform,
            Option<&Boss>,
            Option<&crate::enemies::MinibossSlot>,
            Option<&mut crate::bestiary::AegisShield>,
            Option<&crate::bestiary::Mimic>,
            Has<arsenal::BellMark>,
            // §9 Glitched affixes (P10): Nightborne, the Warden's shield, Contagious buds
            (Option<&crate::affixes::Affixes>, Option<&mut crate::affixes::AffixCore>, Option<&mut crate::affixes::WardenShield>),
        ),
        Without<Pot>,
    >,
    mut pots: Query<(&mut Pot, &Transform)>,
    mut kills: MessageWriter<KillMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    // (§9) where a hit's shooter stands — the Aegis Drone's shield reads the line of it —
    // and a dead Mimic's swallowed gold, back to whoever paid it; (§6) the arsenal's tallies
    (shooters, mut refunds, mut telemetry, mut tm): (
        Query<&Player>,
        MessageWriter<crate::bestiary::RefundMsg>,
        ResMut<crate::bestiary::BestiaryTelemetry>,
        ResMut<arsenal::ArsenalTelemetry>,
    ),
    (mut splits, mut affix_fx, mut affix_tm): (
        MessageWriter<crate::affixes::AffixSplitMsg>,
        MessageWriter<crate::affixes::AffixFxMsg>,
        ResMut<crate::affixes::AffixTelemetry>,
    ),
) {
    let mut rng = rand::thread_rng();
    let now = time.elapsed_secs();
    let sun = crate::daynight::Sun::of(&run);
    // the §11 duo combos need a teammate: solo keeps no marks
    let squad = q_ps.iter().count() > 1;
    ledger.prune(now);

    for msg in reader.read() {
        let shooter = msg.source.and_then(|s| q_ps.get(s).ok()).map(|(_, pid)| pid.0);
        // Pots first: they piggyback on the enemy hash but die as pots.
        if let Ok((mut pot, tf)) = pots.get_mut(msg.target) {
            if pot.broken {
                continue;
            }
            pot.broken = true;
            kills.write(KillMsg {
                pos: tf.translation,
                dir: tf.translation.normalize_or_zero(),
                kind: None,
                elite: false,
                xp: 0.0,
                is_boss: false,
                is_miniboss: false,
                is_pot: true,
                affixes: Default::default(),
                by: msg.source,
            });
            sfx.write(SfxMsg(Sfx::Pot));
            commands.entity(msg.target).despawn();
            continue;
        }
        if let Ok((mut e, tf, boss, slot, shield, mimic, marked, (affixes, mut core, mut warden))) = enemies.get_mut(msg.target) {
            if e.hp <= 0.0 {
                continue;
            }
            let set = affixes.map(|a| a.0).unwrap_or_default();
            // Cosmonaut's Bell: a tolled foe takes crits more easily — from anyone's hit
            let (mut amount, mut crit) = (msg.amount, msg.crit);
            if marked && !crit && rng.gen_bool(BELL_MARK_CRIT as f64) {
                let crit_dmg = msg.source.and_then(|s| q_ps.get(s).ok()).map(|(ps, _)| ps.crit_damage()).unwrap_or(2.0);
                amount *= crit_dmg;
                crit = true;
                tm.mark_crits += 1;
            }
            // §11 duo combos (crowd and elites; a boss holds its own ground): a teammate's
            // setup mark on this foe, finished by this hit's family, pays off
            let combo = match (squad, shooter, boss) {
                (true, Some(pid), None) => ledger.hit(msg.target, pid, msg.by, now),
                _ => None,
            };
            amount *= combo.map_or(1.0, |(feat, _)| feat.finisher_mult());
            // The Aegis Drone's shield: a hit along its facing cone lands a sliver and does
            // not stagger it (§9 — flank it). A shape-coded BLOCK read-out, throttled per drone.
            let mut blocked = false;
            if let Some(mut sh) = shield {
                let shooter_dir = msg.source.and_then(|s| shooters.get(s).ok()).map(|p| p.dir);
                if crate::bestiary::shield_blocks(e.dir, sh.facing, msg.knock, shooter_dir) {
                    blocked = true;
                    amount *= AEGIS_BLOCK_FRACTION;
                    telemetry.aegis_blocks += 1;
                    if sh.block_fx <= 0.0 {
                        sh.block_fx = AEGIS_BLOCK_FX_SECS;
                        numbers.write(NumberMsg { pos: tf.translation, amount: 0.0, kind: NumKind::Block });
                    }
                } else {
                    telemetry.aegis_open_hits += 1;
                }
            }
            // §9 Glitched affixes (P10): a Nightborne elite in the day takes nothing at all
            // (no stagger, no stun, no lifesteal off it); a Warden's shield soaks what comes
            // at its face until it breaks — flank the curve.
            if !set.is_empty() {
                let shooter_dir = msg.source.and_then(|s| shooters.get(s).ok()).map(|p| p.dir);
                match crate::affixes::guard_hit(set, e.dir, sun.is_night(e.dir), warden.as_deref_mut(), msg.knock, shooter_dir, &mut amount) {
                    crate::affixes::Guard::Immune => {
                        affix_tm.immune += 1;
                        if core.as_deref_mut().is_some_and(|c| c.readout_ready()) {
                            numbers.write(NumberMsg { pos: tf.translation, amount: 0.0, kind: NumKind::Immune });
                        }
                        continue;
                    }
                    crate::affixes::Guard::Shield { broke, .. } => {
                        affix_tm.warden_blocks += 1;
                        if broke {
                            affix_tm.warden_breaks += 1;
                            affix_fx.write(crate::affixes::AffixFxMsg { fx: crate::affixes::AffixFx::ShieldBreak { dir: e.dir }, from_wire: false });
                        }
                        if amount <= 0.0 {
                            blocked = true;
                            if core.as_deref_mut().is_some_and(|c| c.readout_ready()) {
                                numbers.write(NumberMsg { pos: tf.translation, amount: 0.0, kind: NumKind::Block });
                            }
                        }
                    }
                    crate::affixes::Guard::Open => {}
                }
            }
            e.hp -= amount;
            if !blocked {
                e.flash = 1.0;
            }
            // Contagious buds at each HP threshold a hit it survives crosses (burst, don't chip)
            if set.has(crate::content::enemies::Affix::Contagious) && e.hp > 0.0 {
                if let Some(c) = core.as_deref_mut() {
                    for _ in 0..c.splits(e.hp / e.max_hp.max(1e-3)) {
                        splits.write(crate::affixes::AffixSplitMsg { kind: e.kind, dir: e.dir });
                    }
                }
            }
            let knock_scale = if boss.is_some() {
                0.05
            } else if blocked {
                0.2
            } else if set.has(crate::content::enemies::Affix::Leaden) {
                LEADEN_KNOCK
            } else {
                1.0
            };
            e.knock += msg.knock * knock_scale;
            if let Some(w) = msg.weapon() {
                // a cold field slows by ITS OWN amount, and only its own hits do (L14)
                if let Behavior::Aura { slow, .. } = w.def().behavior {
                    if slow > 0.0 {
                        e.slow = e.slow.max(slow);
                    }
                }
                // a Whoopee shove leaves the crowd reeling; bosses shrug it off
                if let (Some(secs), None) = (arsenal::stun_secs(w), boss) {
                    commands.entity(msg.target).try_insert(crate::enemies::Stunned { secs });
                    tm.stuns += 1;
                }
            }
            if !blocked {
                numbers.write(NumberMsg {
                    pos: tf.translation,
                    amount,
                    kind: if crit { NumKind::Crit } else { NumKind::Hit },
                });
            }
            if crit {
                sfx.write(SfxMsg(Sfx::Crit));
            }
            // lifesteal: chance to heal 1 — heals the SHOOTER, not an arbitrary player
            if let Some((mut ps, _)) = msg.source.and_then(|s| q_ps.get_mut(s).ok()) {
                // (a shot still in flight when its shooter went down heals no Beacon)
                if !ps.dead && ps.stats.lifesteal > 0.0 && rng.gen_bool((ps.stats.lifesteal.min(1.0)) as f64) {
                    ps.hp = (ps.hp + 1.0).min(ps.stats.max_hp);
                }
            }
            if e.hp <= 0.0 {
                if let (Some((feat, setup)), Some(finisher)) = (combo, shooter) {
                    duos.write(crate::duos::DuoMsg { feat, setup, finisher, dir: e.dir, max_hp: e.max_hp });
                }
                let is_boss = boss.map(|b| b.kind.def().is_stage_boss).unwrap_or(false);
                let is_mini = boss.is_some() && !is_boss;
                if let Some(m) = mimic {
                    if let (Some(payer), true) = (m.payer, m.paid > 0) {
                        refunds.write(crate::bestiary::RefundMsg { payer, gold: m.paid, pos: tf.translation });
                    }
                }
                kills.write(KillMsg {
                    pos: tf.translation,
                    dir: e.dir,
                    kind: Some(e.kind),
                    elite: e.elite || is_mini || e.kind.elite_loot(),
                    xp: e.xp,
                    is_boss,
                    is_miniboss: is_mini,
                    is_pot: false,
                    affixes: set,
                    by: msg.source,
                });
                // §3: the 7:00 spike pays out a guaranteed chest where the miniboss fell.
                if slot.map(|s| s.0) == Some(0) {
                    run.reward_chest = Some(e.dir);
                }
                if is_boss {
                    run.boss_dead = true;
                    run.boss_kills += 1;
                    shake.add(0.8);
                }
                // §13 hitstop: only on YOUR killing blow (never a teammate's, never chip)
                if fx::freezes(&role) && msg.source.is_some_and(|s| q_local.contains(s)) {
                    let weight = if is_boss {
                        fx::KillWeight::Boss
                    } else if e.elite || is_mini {
                        fx::KillWeight::Elite
                    } else {
                        fx::KillWeight::Crowd
                    };
                    hitstop.kill(weight, msg.weapon().is_some_and(|w| w.is_evolution()), now);
                }
                commands.entity(msg.target).despawn();
            }
        }
    }
}

/// Resolve hits on the player: evasion -> shield -> armor -> hp, thorns reflect — and, when
/// a hit would kill, the death-save chain: first the ONE item resolver
/// (`items::resolve_death_save`, §7 stacking rule), then, only if no item caught it, the
/// §13 "one more chance" token (an accessibility option never eats a save the build paid
/// for). The §13 enemy-damage assist scales every hit here, where it lands, so shots and
/// telegraphs already in flight when the slider moved honour it too. HOST-only: a peer's
/// saves resolve here too and reach its screen as an item event / its streamed vitals.
#[allow(clippy::too_many_arguments)]
pub fn apply_player_hits(
    mut reader: MessageReader<PlayerHitMsg>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    mut q_ps: Query<(
        &mut PlayerState,
        &mut Player,
        &mut crate::items::ItemProcs,
        &mut crate::techs::MoveTech,
        &crate::player::InputIntent,
        &crate::player::PlayerId,
        &Transform,
        Has<crate::player::LocalPlayer>,
        &mut arsenal::WeaponProcs,
    )>,
    mut q_crowd: Query<(&mut Enemy, Has<Boss>), Without<Pot>>,
    mut shake: ResMut<Shake>,
    mut hits: MessageWriter<HitMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    (mut item_fx, mut tech_fx): (MessageWriter<crate::items::ItemFxMsg>, MessageWriter<crate::techs::TechFxMsg>),
    (mut telemetry, mut tech_telemetry, mut arsenal_tm): (
        ResMut<crate::items::ItemTelemetry>,
        ResMut<crate::techs::TechTelemetry>,
        ResMut<arsenal::ArsenalTelemetry>,
    ),
    mut banners: MessageWriter<BannerMsg>,
) {
    let mut rng = rand::thread_rng();
    for msg in reader.read() {
        // Address the hit to its actual victim. `continue`, never unwrap: messages are
        // double-buffered, so a victim CAN be despawned between the write and this read
        // (stage change, disconnect).
        let Ok((mut run_ps, mut body, mut procs, mut tech, intent, pid, ptf, is_local, mut wprocs)) = q_ps.get_mut(msg.victim) else { continue };
        // Boomerang Insurance's escape is an Antipode Blink (§15: one mechanic), turned
        // the way this astronaut is looking, like a keyed one.
        let mut insured_blink = |body: &mut Player,
                                 ps: &mut PlayerState,
                                 procs: &mut crate::items::ItemProcs,
                                 tech: &mut crate::techs::MoveTech| {
            let jump = crate::techs::blink_body(body, ps, tech, procs, intent.forward);
            tech_telemetry.insured_blinks += 1;
            tech_fx.write(crate::techs::TechFxMsg {
                fx: crate::techs::TechFx::Blink { owner: pid.0, from: jump.from, to: jump.to, axis: jump.axis, insured: true },
                from_wire: false,
            });
        };
        // a Beacon takes no hits (it is not "killed again", which would reset its meters)
        if run_ps.iframes > 0.0 || run_ps.hp <= 0.0 || run_ps.dead {
            continue;
        }
        // evasion
        if rng.gen_bool(run_ps.effective_evasion_fraction() as f64) {
            numbers.write(NumberMsg { pos: ptf.translation, amount: 0.0, kind: NumKind::Dodge });
            continue;
        }
        // Cracked Helmet's price is paid before mitigation, like any other damage taken, and
        // so are the tomes' (Elite, Static) against the kind of foe that swung; the §13
        // enemy-damage assist eases the hit itself. A boss or miniboss head carries the elite
        // flag for its loot, but it is not the "elite" Tome of the Elite hunts.
        let (by_elite, by_static) = msg
            .attacker
            .and_then(|a| q_crowd.get(a).ok())
            .map(|(en, boss)| (en.elite && !boss, en.kind == crate::content::enemies::EnemyKind::Ghost))
            .unwrap_or((false, false));
        let mut amount = msg.amount
            * run.assist.enemy_damage
            * run_ps.stats.damage_taken.max(0.0)
            * crate::tomes::incoming_mult(&run_ps.stats, by_elite, by_static)
            * (1.0 - run_ps.effective_armor_fraction());
        // shield first
        if run_ps.shield > 0.0 {
            let absorbed = run_ps.shield.min(amount);
            run_ps.shield -= absorbed;
            amount -= absorbed;
        }
        run_ps.shield_cd = 5.0;
        run_ps.hp -= amount;
        run_ps.iframes = 0.4;
        // a hit that lands (shield or not — a dodge is not a hit) breaks the Yo-Yo combo
        if wprocs.combo >= 1.0 {
            arsenal_tm.combo_breaks += 1;
        }
        wprocs.combo = 0.0;
        wprocs.combo_m = 0.0;
        if is_local {
            shake.add(SHAKE_PLAYER_HIT);
            sfx.write(SfxMsg(Sfx::Hurt));
        }

        // thorns
        if run_ps.stats.thorns > 0.0 {
            if let Some(att) = msg.attacker {
                hits.write(HitMsg { source: Some(msg.victim), target: att, amount: run_ps.stats.thorns, crit: false, knock: Vec3::ZERO, by: HitBy::Thorns });
            }
        }

        let here = body.dir;
        if run_ps.hp <= 0.0 {
            match crate::items::resolve_death_save(&mut run_ps, &mut procs, here) {
                Some((crate::items::DeathSave::AntipodeEscape, _)) => {
                    telemetry.saves[crate::items::DeathSave::AntipodeEscape.code() as usize] += 1;
                    // its event (TechFx::Blink, insured) carries the banner and the snap
                    insured_blink(&mut body, &mut run_ps, &mut procs, &mut tech);
                }
                Some((save, landing)) => {
                    telemetry.saves[save.code() as usize] += 1;
                    if landing != here {
                        telemetry.rewinds.push(crate::sphere::arc_dist(here, landing, planet.radius));
                        move_astronaut(&mut body, &mut procs, &mut tech, landing);
                    }
                    item_fx.write(crate::items::ItemFxMsg {
                        fx: crate::items::ItemFx::DeathSave { owner: pid.0, save, dir: landing },
                        from_wire: false,
                    });
                }
                None => {
                    run_ps.hp = 0.0;
                    if run_ps.try_revive_token(&run.assist) {
                        revive_nova(&hash, &mut q_crowd, ptf.translation, here, planet.radius);
                        if is_local {
                            banners.write(BannerMsg("ONE MORE CHANCE!".into()));
                        }
                        sfx.write(SfxMsg(Sfx::Shrine));
                        info!("revive token spent (hp -> {:.0})", run_ps.hp);
                    } else {
                        // down: a Tumbling Beacon (§11; solo, the end of the run)
                        run_ps.go_down();
                    }
                }
            }
        } else if crate::items::boomerang_insurance(&run_ps, &mut procs, here).is_some() {
            insured_blink(&mut body, &mut run_ps, &mut procs, &mut tech);
        }
    }
}

/// "One more chance" clears a breathing ring so the second chance does not open inside the
/// same crowd that closed the first. Crowd only — bosses and pots hold their ground.
fn revive_nova(
    hash: &SpatialHash,
    q_crowd: &mut Query<(&mut Enemy, Has<Boss>), Without<Pot>>,
    at: Vec3,
    dir: Vec3,
    planet_radius: f32,
) {
    for (e, _) in hash.near(at, REVIVE_NOVA_RADIUS) {
        let Ok((mut en, false)) = q_crowd.get_mut(e) else { continue };
        let arc = sphere::arc_dist(en.dir, dir, planet_radius);
        if arc >= REVIVE_NOVA_RADIUS {
            continue;
        }
        let away = en.dir - dir * en.dir.dot(dir);
        let away = away.try_normalize().unwrap_or_else(|| sphere::tangent_frame(dir).0);
        en.knock += away * REVIVE_NOVA_KNOCK * (1.0 - arc / REVIVE_NOVA_RADIUS).max(0.3);
    }
}

/// Put an astronaut somewhere else on the planet (a tether rewind, a blink): standing,
/// still, and without the move reading as a fall to Downhill Momentum.
fn move_astronaut(body: &mut Player, procs: &mut crate::items::ItemProcs, tech: &mut crate::techs::MoveTech, to: Vec3) {
    tech.cancel_moves();
    body.dir = to;
    body.vel_t = Vec3::ZERO;
    body.vel_r = 0.0;
    body.height = 0.0;
    procs.forget_altitude();
}
