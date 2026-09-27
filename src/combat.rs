//! Player weapons: firing, projectiles, beams, orbitals, chains, auras —
//! plus damage resolution for both sides.

use crate::config::*;
use crate::content::weapons::{Behavior, WeaponKind};
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
    pub sweep_mesh: Handle<Mesh>,
    pub aura_mesh: Handle<Mesh>,
    pub mats: HashMap<WeaponKind, Handle<StandardMaterial>>,
    /// Faint see-through variants for the aura sphere so it doesn't blind the player.
    pub aura_mats: HashMap<WeaponKind, Handle<StandardMaterial>>,
}

pub fn setup_weapon_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mats = HashMap::new();
    let mut aura_mats = HashMap::new();
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
        // A faint, see-through version for the aura bubble that surrounds the player.
        aura_mats.insert(
            kind,
            materials.add(StandardMaterial {
                base_color: c.with_alpha(0.12),
                emissive: c.to_linear() * 0.6,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            }),
        );
    }
    commands.insert_resource(WeaponAssets {
        proj_mesh: meshes.add(Mesh::from(Sphere::new(0.22))),
        drone_mesh: meshes.add(Mesh::from(Cuboid::new(0.4, 0.25, 0.55))),
        beam_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        sweep_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 0.12, 1.0))),
        aura_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
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
    pub dir: Vec3,     // unit direction from core
    pub heading: Vec3, // tangent unit
    pub speed: f32,
    pub damage: f32,
    pub pierce: i32,
    pub life: f32,
    pub size: f32,
    pub kind: ProjKind,
    pub hit_cd: HashMap<Entity, f32>,
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
}

#[derive(Component)]
pub struct Beam {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    pub heading: Vec3,
    pub range: f32,
    pub width: f32,
    pub damage: f32,
    pub ticks_left: u32,
    pub tick_cd: f32,
    /// Fired by Second Astronaut's ghost: it streams from where the ghost floats.
    pub ghost: bool,
}

#[derive(Component)]
pub struct Fader {
    pub life: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct AuraVis {
    /// The astronaut that created this. Without it, two players with the same orbital
    /// weapon annihilate each other's drones in the reconciler, and a peer's beam emits
    /// from the host's shoulder.
    pub owner: Entity,
    pub weapon: WeaponKind,
}

/// Nearest live enemy to `pos` within `max_d` meters — pottery is scenery, not a target.
fn nearest_enemy(
    pos: Vec3,
    max_d: f32,
    enemies: &Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Player>)>,
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
struct Volley {
    owner: Entity,
    kind: WeaponKind,
    dmg: f32,
    count: u32,
    size: f32,
    /// Where the shooter stands (melee reach and chain hops measure from here) and where
    /// its shots leave from (a little above that).
    center: Vec3,
    origin: Vec3,
    up: Vec3,
    aim: Vec3,
    /// Anti-Grav Boots, airborne: every directional weapon fires a full 360° ring.
    ring: bool,
    /// Second Astronaut's ghost fired this (its beams track the ghost, not the owner).
    ghost: bool,
    crit_ch: f32,
}

/// Evenly spaced headings all the way round `up`, starting at `aim` (a 360° ring).
fn ring_headings(up: Vec3, aim: Vec3, n: u32) -> impl Iterator<Item = Vec3> {
    (0..n).map(move |i| Quat::from_axis_angle(up, i as f32 / n as f32 * std::f32::consts::TAU) * aim)
}

/// Fire one volley (every behavior except the always-on Orbit and Aura).
#[allow(clippy::too_many_arguments)]
fn fire_volley(
    v: &Volley,
    stats: &crate::stats::Stats,
    assets: &WeaponAssets,
    enemies: &Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Player>)>,
    pots: &Query<(), With<Pot>>,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
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
                    });
                }
            }
            // sweep visual
            commands.spawn((
                Mesh3d(assets.sweep_mesh.clone()),
                MeshMaterial3d(assets.mats[&v.kind].clone()),
                Transform::from_translation(if v.ring { origin } else { origin + aim * r * 0.5 })
                    .with_rotation(sphere::frame_quat(up, aim))
                    .with_scale(if v.ring {
                        Vec3::new(r * 2.0, 0.1, r * 2.0)
                    } else {
                        Vec3::new(r * (arc_deg / 90.0).min(2.2), 0.1, r)
                    }),
                Fader { life: 0.14, max: 0.14 },
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
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: pierce as i32,
                        life: 2.2 * stats.duration,
                        size,
                        kind: ProjKind::Straight,
                        hit_cd: HashMap::new(),
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
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: pierce as i32,
                        life: 2.6 * stats.duration,
                        size,
                        kind: ProjKind::Seek,
                        hit_cd: HashMap::new(),
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
            for h in headings {
                let out_time = range / speed;
                commands.spawn((
                    Projectile {
                        owner: v.owner,
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: 999,
                        life: out_time * 2.4 * stats.duration,
                        size,
                        kind: ProjKind::Boomerang { age: 0.0, out_time },
                        hit_cd: HashMap::new(),
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
            let mut prev = origin;
            for (e, pos) in &chain {
                let (cm, crit) = roll_crit(v.crit_ch, stats.crit_damage, rng);
                let elite = enemies.get(*e).map(|(_, _, en)| if en.elite { stats.elite_damage } else { 1.0 }).unwrap_or(1.0);
                hits.write(HitMsg { source: Some(v.owner), target: *e, amount: dmg * cm * elite, crit, knock: Vec3::ZERO });
                // zap segment visual
                let mid = (prev + *pos) / 2.0;
                let len = prev.distance(*pos);
                let dirv = (*pos - prev).normalize_or_zero();
                commands.spawn((
                    Mesh3d(assets.beam_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(mid)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Z, dirv))
                        .with_scale(Vec3::new(0.12, 0.12, len)),
                    Fader { life: 0.12, max: 0.12 },
                    StageScoped,
                ));
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
                        dir: start_dir,
                        heading: h,
                        speed: speed * stats.proj_speed,
                        damage: dmg,
                        pierce: 0,
                        life: 4.0 * stats.duration,
                        size,
                        kind: ProjKind::Rocket { aoe: aoe * size },
                        hit_cd: HashMap::new(),
                    },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.mats[&v.kind].clone()),
                    Transform::from_translation(origin).with_scale(Vec3::splat(size * 1.3)),
                    StageScoped,
                ));
            }
        }
        Behavior::Orbit { .. } | Behavior::Aura { .. } => {}
    }
}

/// Where a volley from `center` aims: the nearest enemy in range, flattened onto the local
/// horizontal, else straight ahead.
fn aim_from(
    center: Vec3,
    up: Vec3,
    facing: Vec3,
    enemies: &Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Player>)>,
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

/// Tick weapon cooldowns and fire.
///
/// Runs on every machine for every astronaut it has a sheet for — authoritative on the
/// host, the owner's cosmetic prediction on a client — so the §7 firing rules live here
/// too: The Overheat's jam, Anti-Grav Boots' airborne 360° ring, and Second Astronaut's
/// ghost volley.
#[allow(clippy::too_many_arguments)]
pub fn weapon_fire(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    particles: Option<Res<ParticleAssets>>,
    mut telemetry: ResMut<crate::items::ItemTelemetry>,
    mut q_player: Query<(Entity, &Player, &mut PlayerState, &mut crate::items::ItemProcs, &Transform)>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Player>)>,
    q_pots: Query<(), With<Pot>>,
    q_drones: Query<(Entity, &Drone)>,
    q_auras: Query<(Entity, &AuraVis)>,
    mut hits: MessageWriter<HitMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    role: Res<crate::net::NetRole>,
) {
    use crate::content::items::ItemKind;
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let t_now = time.elapsed_secs();
    let simulating = role.simulates();

    // Reconcile drone + aura entities with owned weapons — keyed by (owner, weapon).
    let mut want_drones: HashMap<(Entity, WeaponKind), (usize, f32, f32, f32)> = HashMap::new();
    let mut want_auras: Vec<(Entity, WeaponKind)> = Vec::new();
    let mut player_pos: Vec<(Entity, Vec3)> = Vec::new();

    for (pe, player, mut run, mut procs, ptf) in &mut q_player {
    if run.dead {
        continue; // a downed astronaut stops firing
    }
    player_pos.push((pe, ptf.translation));
    let atk_speed = run.attack_speed();
    let dmg_mult = run.damage_mult();
    let stats = run.live_stats();
    let crit_ch = run.crit_chance(); // captured before the &mut weapons loop (Reticle pulse)
    let aura_sc = run.aura_scale(); // Aurora's fields swell while sprinting
    let overheat = run.has_item(ItemKind::TheOverheat);
    let ring = run.airborne && run.has_item(ItemKind::AntiGravBoots);
    procs.jam = (procs.jam - dt).max(0.0);
    let jammed = procs.jam > 0.0;
    let up = player.dir;
    let origin = ptf.translation + up * 0.4;

    for wi in run.weapons.iter_mut() {
        let def = wi.kind.def();
        let (lvl_dmg, lvl_extra, lvl_size) = wi.kind.level_scaling(wi.level);
        let dmg = def.damage * lvl_dmg * dmg_mult;
        let count = (def.projectiles + lvl_extra + stats.projectiles.max(0) as u32).max(1);
        let size = lvl_size * stats.size;

        match def.behavior {
            Behavior::Orbit { radius, deg_per_sec } => {
                want_drones.insert((pe, wi.kind), (count as usize, dmg, radius * size, deg_per_sec));
                continue;
            }
            Behavior::Aura { radius, slow } => {
                let radius = radius * aura_sc;
                want_auras.push((pe, wi.kind));
                if jammed {
                    continue; // The Overheat: a jammed suit's fields stop pulsing too
                }
                wi.cd -= dt * atk_speed;
                if wi.cd <= 0.0 {
                    wi.cd = def.cooldown;
                    let r = radius * size;
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
                            });
                            let _ = slow; // applied in apply_hits via kind check
                        }
                    }
                }
                continue;
            }
            _ => {}
        }

        if jammed {
            continue;
        }
        wi.cd -= dt * atk_speed;
        if wi.cd > 0.0 {
            continue;
        }
        wi.cd = def.cooldown;

        let aim = aim_from(ptf.translation, up, player.facing, &enemies, &q_pots);
        let v = Volley {
            owner: pe,
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
        fire_volley(&v, &stats, &assets, &enemies, &q_pots, &mut commands, &mut hits, &mut sfx, &mut rng);
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
    let mirrored = run.ghost_weapon.and_then(|g| run.weapons.iter().find(|w| w.kind == g).map(|w| (w.kind, w.level)));
    if let (true, Some((kind, level)), false) = (ghost_power > 0.0, mirrored, procs.jam > 0.0) {
        procs.ghost_cd -= dt * atk_speed;
        if procs.ghost_cd <= 0.0 {
            let def = kind.def();
            procs.ghost_cd = def.cooldown;
            let (lvl_dmg, lvl_extra, lvl_size) = kind.level_scaling(level);
            let gpos = crate::items::ghost_anchor(ptf, t_now);
            let gup = gpos.normalize_or_zero();
            let v = Volley {
                owner: pe,
                kind,
                dmg: def.damage * lvl_dmg * dmg_mult * GHOST_MIRROR * ghost_power,
                count: (def.projectiles + lvl_extra + stats.projectiles.max(0) as u32).max(1),
                size: lvl_size * stats.size,
                center: gpos,
                origin: gpos,
                up: gup,
                aim: aim_from(gpos, gup, player.facing, &enemies, &q_pots),
                ring: false,
                ghost: true,
                crit_ch,
            };
            fire_volley(&v, &stats, &assets, &enemies, &q_pots, &mut commands, &mut hits, &mut sfx, &mut rng);
            telemetry.ghost_volleys += 1;
        }
    }
    }

    // ------ drone reconciliation
    let mut have: HashMap<(Entity, WeaponKind), usize> = HashMap::new();
    for (e, d) in q_drones.iter() {
        let alive = want_drones.get(&(d.owner, d.weapon));
        match alive {
            Some((want_count, dmg, radius, dps)) => {
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
                    });
                }
            }
            None => {
                commands.entity(e).despawn();
            }
        }
    }
    for ((owner, kind), (count, dmg, radius, dps)) in &want_drones {
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
            if let Behavior::Aura { radius, .. } = kind.def().behavior {
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
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();

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

        if p.life <= 0.0 {
            if let ProjKind::Rocket { aoe } = p.kind {
                explode(&mut commands, &hash, &enemies, &run, p.owner, tf.translation, aoe, p.damage, &mut hits, &particles, p.dir, &mut rng);
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
        tf.translation = planet.surface_point(p.dir) + p.dir * 0.9;
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
                    explode(&mut commands, &hash, &enemies, &run, p.owner, tf.translation, aoe, p.damage, &mut hits, &particles, p.dir, &mut rng);
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
    // the astronaut whose rocket this was — carried so AoE damage credits the right player
    owner: Entity,
    pos: Vec3,
    aoe: f32,
    damage: f32,
    hits: &mut MessageWriter<HitMsg>,
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
            hits.write(HitMsg { source: Some(owner), target: te, amount: damage * cm * elite, crit, knock: kdir * 7.0 });
        }
    }
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

        if d.tick <= 0.0 {
            let mut hit_any = false;
            for (te, tpos) in hash.near(tf.translation, 1.6) {
                let Ok(en) = enemies.get(te) else { continue };
                let reach = 0.8 + en.scale * 0.5;
                if tpos.distance_squared(tf.translation) < reach * reach {
                    let (cm, crit) = roll_crit(run.crit_chance(), run.crit_damage(), &mut rng);
                    let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg {
                        source: Some(d.owner),
                        target: te,
                        amount: d.damage * cm * elite,
                        crit,
                        knock: offset.normalize_or_zero() * 5.0,
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
                    hits.write(HitMsg { source: Some(beam.owner), target: te, amount: beam.damage * cm * elite, crit, knock: Vec3::ZERO });
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
        if let Behavior::Aura { radius, .. } = a.weapon.def().behavior {
            tf.translation = ptf.translation;
            tf.scale = Vec3::splat(radius * run.aura_scale() * (1.0 + (t * 3.0).sin() * 0.04));
        }
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
        } else {
            let t = (f.life / f.max).max(0.0);
            tf.scale *= 0.9 + t * 0.1;
        }
    }
}

/// Resolve HitMsg against enemies / bosses / pots: damage, flash, knockback,
/// lifesteal, slows, deaths.
#[allow(clippy::too_many_arguments)]
pub fn apply_hits(
    mut commands: Commands,
    mut reader: MessageReader<HitMsg>,
    mut run: ResMut<RunState>,
    mut q_ps: Query<&mut PlayerState>,
    mut shake: ResMut<Shake>,
    mut hitstop: ResMut<Hitstop>,
    mut enemies: Query<(&mut Enemy, &Transform, Option<&Boss>, Option<&crate::enemies::MinibossSlot>), Without<Pot>>,
    mut pots: Query<(&mut Pot, &Transform)>,
    mut kills: MessageWriter<KillMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let mut rng = rand::thread_rng();

    for msg in reader.read() {
        // Resolve cryo against THIS hit's shooter, not an arbitrary player.
        let has_cryo = msg
            .source
            .and_then(|s| q_ps.get(s).ok())
            .map(|ps| {
                ps.weapons
                    .iter()
                    .any(|w| matches!(w.kind, WeaponKind::CryoVent | WeaponKind::AbsoluteZero))
            })
            .unwrap_or(false);
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
                is_pot: true,
            });
            sfx.write(SfxMsg(Sfx::Pot));
            commands.entity(msg.target).despawn();
            continue;
        }
        if let Ok((mut e, tf, boss, slot)) = enemies.get_mut(msg.target) {
            if e.hp <= 0.0 {
                continue;
            }
            e.hp -= msg.amount;
            e.flash = 1.0;
            let knock_scale = if boss.is_some() { 0.05 } else { 1.0 };
            e.knock += msg.knock * knock_scale;
            if has_cryo {
                e.slow = (e.slow + 0.25).min(0.65);
            }
            numbers.write(NumberMsg {
                pos: tf.translation,
                amount: msg.amount,
                kind: if msg.crit { NumKind::Crit } else { NumKind::Hit },
            });
            if msg.crit {
                sfx.write(SfxMsg(Sfx::Crit));
            }
            // lifesteal: chance to heal 1 — heals the SHOOTER, not an arbitrary player
            if let Some(mut ps) = msg.source.and_then(|s| q_ps.get_mut(s).ok()) {
                if ps.stats.lifesteal > 0.0 && rng.gen_bool((ps.stats.lifesteal.min(1.0)) as f64) {
                    ps.hp = (ps.hp + 1.0).min(ps.stats.max_hp);
                }
            }
            if e.hp <= 0.0 {
                let is_boss = boss.map(|b| b.kind.def().is_stage_boss).unwrap_or(false);
                let is_mini = boss.is_some() && !is_boss;
                kills.write(KillMsg {
                    pos: tf.translation,
                    dir: e.dir,
                    kind: Some(e.kind),
                    elite: e.elite || is_mini,
                    xp: e.xp,
                    is_boss,
                    is_pot: false,
                });
                // §3: the 7:00 spike pays out a guaranteed chest where the miniboss fell.
                if slot.map(|s| s.0) == Some(0) {
                    run.reward_chest = Some(e.dir);
                }
                if is_boss {
                    run.boss_dead = true;
                    run.boss_kills += 1;
                    shake.add(0.8);
                    hitstop.timer = 0.25;
                } else if msg.crit {
                    shake.add(0.06);
                }
                commands.entity(msg.target).despawn();
            }
        }
    }
}

/// Resolve hits on the player: evasion -> shield -> armor -> hp, thorns reflect — and, when
/// a hit would kill, the ONE death-save resolver (`items::resolve_death_save`, §7 stacking
/// rule). HOST-only: a peer's saves resolve here too and reach its screen as an item event.
#[allow(clippy::too_many_arguments)]
pub fn apply_player_hits(
    mut reader: MessageReader<PlayerHitMsg>,
    planet: Res<CurrentPlanet>,
    mut q_ps: Query<(
        &mut PlayerState,
        &mut Player,
        &mut crate::items::ItemProcs,
        &crate::player::PlayerId,
        &Transform,
        Has<crate::player::LocalPlayer>,
    )>,
    mut shake: ResMut<Shake>,
    mut hits: MessageWriter<HitMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut item_fx: MessageWriter<crate::items::ItemFxMsg>,
    mut telemetry: ResMut<crate::items::ItemTelemetry>,
) {
    let mut rng = rand::thread_rng();
    for msg in reader.read() {
        // Address the hit to its actual victim. `continue`, never unwrap: messages are
        // double-buffered, so a victim CAN be despawned between the write and this read
        // (stage change, disconnect).
        let Ok((mut run_ps, mut body, mut procs, pid, ptf, is_local)) = q_ps.get_mut(msg.victim) else { continue };
        if run_ps.iframes > 0.0 || run_ps.hp <= 0.0 {
            continue;
        }
        // evasion
        if rng.gen_bool(run_ps.effective_evasion_fraction() as f64) {
            numbers.write(NumberMsg { pos: ptf.translation, amount: 0.0, kind: NumKind::Dodge });
            continue;
        }
        // Cracked Helmet's price is paid before mitigation, like any other damage taken.
        let mut amount = msg.amount * run_ps.stats.damage_taken.max(0.0) * (1.0 - run_ps.effective_armor_fraction());
        // shield first
        if run_ps.shield > 0.0 {
            let absorbed = run_ps.shield.min(amount);
            run_ps.shield -= absorbed;
            amount -= absorbed;
        }
        run_ps.shield_cd = 5.0;
        run_ps.hp -= amount;
        run_ps.iframes = 0.4;
        if is_local {
            shake.add(0.12);
            sfx.write(SfxMsg(Sfx::Hurt));
        }

        // thorns
        if run_ps.stats.thorns > 0.0 {
            if let Some(att) = msg.attacker {
                hits.write(HitMsg { source: Some(msg.victim), target: att, amount: run_ps.stats.thorns, crit: false, knock: Vec3::ZERO });
            }
        }

        let here = body.dir;
        if run_ps.hp <= 0.0 {
            match crate::items::resolve_death_save(&mut run_ps, &mut procs, here) {
                Some((save, landing)) => {
                    telemetry.saves[save.code() as usize] += 1;
                    if landing != here {
                        telemetry.rewinds.push(crate::sphere::arc_dist(here, landing, planet.radius));
                        move_astronaut(&mut body, &mut procs, landing);
                    }
                    item_fx.write(crate::items::ItemFxMsg {
                        fx: crate::items::ItemFx::DeathSave { owner: pid.0, save, dir: landing },
                        from_wire: false,
                    });
                }
                None => {
                    run_ps.hp = 0.0;
                    run_ps.dead = true;
                }
            }
        } else if let Some(landing) = crate::items::boomerang_insurance(&run_ps, &mut procs, here) {
            move_astronaut(&mut body, &mut procs, landing);
            item_fx.write(crate::items::ItemFxMsg {
                fx: crate::items::ItemFx::DeathSave {
                    owner: pid.0,
                    save: crate::items::DeathSave::AntipodeEscape,
                    dir: landing,
                },
                from_wire: false,
            });
        }
    }
}

/// Put an astronaut somewhere else on the planet (a tether rewind, a blink): standing,
/// still, and without the move reading as a fall to Downhill Momentum.
fn move_astronaut(body: &mut Player, procs: &mut crate::items::ItemProcs, to: Vec3) {
    body.dir = to;
    body.vel_t = Vec3::ZERO;
    body.vel_r = 0.0;
    body.height = 0.0;
    procs.forget_altitude();
}
