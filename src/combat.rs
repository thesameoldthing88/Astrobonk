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
use crate::run::{RunPhase, RunState};
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
}

pub fn setup_weapon_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut mats = HashMap::new();
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
    }
    commands.insert_resource(WeaponAssets {
        proj_mesh: meshes.add(Mesh::from(Sphere::new(0.22))),
        drone_mesh: meshes.add(Mesh::from(Cuboid::new(0.4, 0.25, 0.55))),
        beam_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        sweep_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 0.12, 1.0))),
        aura_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
        mats,
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
    pub heading: Vec3,
    pub range: f32,
    pub width: f32,
    pub damage: f32,
    pub ticks_left: u32,
    pub tick_cd: f32,
}

#[derive(Component)]
pub struct Fader {
    pub life: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct AuraVis {
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

fn roll_crit(crit_chance: f32, crit_damage: f32, rng: &mut impl Rng) -> (f32, bool) {
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

/// Tick weapon cooldowns and fire.
#[allow(clippy::too_many_arguments)]
pub fn weapon_fire(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    _planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    q_player: Query<(&Player, &Transform)>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Player>)>,
    q_pots: Query<(), With<Pot>>,
    q_drones: Query<(Entity, &Drone)>,
    q_auras: Query<(Entity, &AuraVis)>,
    mut hits: MessageWriter<HitMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };
    let mut rng = rand::thread_rng();
    let atk_speed = run.attack_speed();
    let dmg_mult = run.damage_mult();
    let stats = run.stats.clone();

    // Reconcile drone + aura entities with owned weapons.
    let mut want_drones: HashMap<WeaponKind, (usize, f32, f32, f32)> = HashMap::new();
    let mut want_auras: Vec<WeaponKind> = Vec::new();

    let weapons_snapshot: Vec<(WeaponKind, u32)> = run.weapons.iter().map(|w| (w.kind, w.level)).collect();

    for wi in run.weapons.iter_mut() {
        let def = wi.kind.def();
        let (lvl_dmg, lvl_extra, lvl_size) = wi.kind.level_scaling(wi.level);
        let dmg = def.damage * lvl_dmg * dmg_mult;
        let count = (def.projectiles + lvl_extra + stats.projectiles.max(0) as u32).max(1);
        let size = lvl_size * stats.size;

        match def.behavior {
            Behavior::Orbit { radius, deg_per_sec } => {
                want_drones.insert(wi.kind, (count as usize, dmg, radius * size, deg_per_sec));
                continue;
            }
            Behavior::Aura { radius, slow } => {
                want_auras.push(wi.kind);
                wi.cd -= dt * atk_speed;
                if wi.cd <= 0.0 {
                    wi.cd = def.cooldown;
                    let r = radius * size;
                    for (e, tf, en) in enemies.iter() {
                        if tf.translation.distance_squared(ptf.translation) < r * r {
                            let (cm, crit) = roll_crit(stats.crit_chance, stats.crit_damage, &mut rng);
                            let elite = if en.elite { stats.elite_damage } else { 1.0 };
                            hits.write(HitMsg {
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

        wi.cd -= dt * atk_speed;
        if wi.cd > 0.0 {
            continue;
        }
        wi.cd = def.cooldown;

        let target = nearest_enemy(ptf.translation, 40.0, &enemies, &q_pots);
        let up = player.dir;
        let aim = match target {
            Some((_, tpos)) => {
                let v = tpos - ptf.translation;
                (v - up * v.dot(up)).normalize_or_zero()
            }
            None => player.facing,
        };
        let aim = if aim == Vec3::ZERO { player.facing } else { aim };
        let origin = ptf.translation + up * 0.4;

        match def.behavior {
            Behavior::MeleeArc { arc_deg, range } => {
                let r = range * size;
                let cos_half = (arc_deg.to_radians() / 2.0).cos();
                for (e, tf, en) in enemies.iter() {
                    let v = tf.translation - ptf.translation;
                    if v.length_squared() > r * r {
                        continue;
                    }
                    let vt = (v - up * v.dot(up)).normalize_or_zero();
                    if arc_deg >= 360.0 || vt.dot(aim) > cos_half {
                        let (cm, crit) = roll_crit(stats.crit_chance, stats.crit_damage, &mut rng);
                        let elite = if en.elite { stats.elite_damage } else { 1.0 };
                        hits.write(HitMsg {
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
                    MeshMaterial3d(assets.mats[&wi.kind].clone()),
                    Transform::from_translation(origin + aim * r * 0.5)
                        .with_rotation(sphere::frame_quat(up, aim))
                        .with_scale(Vec3::new(r * (arc_deg / 90.0).min(2.2), 0.1, r)),
                    Fader { life: 0.14, max: 0.14 },
                    StageScoped,
                ));
                sfx.write(SfxMsg(Sfx::Hit));
            }
            Behavior::Shot { speed, pierce, spread_deg } => {
                for i in 0..count {
                    let ang = if count > 1 {
                        (i as f32 / (count - 1) as f32 - 0.5) * spread_deg.to_radians()
                    } else {
                        rng.gen_range(-0.04..0.04)
                    };
                    let h = Quat::from_axis_angle(up, ang) * aim;
                    commands.spawn((
                        Projectile {
                            dir: player.dir,
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
                        MeshMaterial3d(assets.mats[&wi.kind].clone()),
                        Transform::from_translation(origin).with_scale(Vec3::splat(size)),
                        StageScoped,
                    ));
                }
            }
            Behavior::Seek { speed, pierce } => {
                for i in 0..count {
                    let ang = i as f32 * 0.35 - (count as f32 - 1.0) * 0.175;
                    let h = Quat::from_axis_angle(up, ang) * aim;
                    commands.spawn((
                        Projectile {
                            dir: player.dir,
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
                        MeshMaterial3d(assets.mats[&wi.kind].clone()),
                        Transform::from_translation(origin).with_scale(Vec3::splat(size * 0.9)),
                        StageScoped,
                    ));
                }
            }
            Behavior::Boomerang { speed, range } => {
                for i in 0..count {
                    let ang = i as f32 * 0.5 - (count as f32 - 1.0) * 0.25;
                    let h = Quat::from_axis_angle(up, ang) * aim;
                    let out_time = range / speed;
                    commands.spawn((
                        Projectile {
                            dir: player.dir,
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
                        MeshMaterial3d(assets.mats[&wi.kind].clone()),
                        Transform::from_translation(origin).with_scale(Vec3::splat(size)),
                        StageScoped,
                    ));
                }
            }
            Behavior::Beam { range, width } => {
                commands.spawn((
                    Beam {
                        heading: aim,
                        range: range * size,
                        width: width * size,
                        damage: dmg,
                        ticks_left: (5.0 * stats.duration) as u32 + 1,
                        tick_cd: 0.0,
                    },
                    Mesh3d(assets.beam_mesh.clone()),
                    MeshMaterial3d(assets.mats[&wi.kind].clone()),
                    Transform::from_translation(origin),
                    StageScoped,
                ));
            }
            Behavior::Chain { jumps, range, link_range } => {
                let mut chain: Vec<(Entity, Vec3)> = Vec::new();
                let mut from = ptf.translation;
                let mut max_d = range;
                for _ in 0..=(jumps + count - 1) {
                    let mut best: Option<(Entity, Vec3, f32)> = None;
                    for (e, tf, _) in enemies.iter() {
                        if chain.iter().any(|(ce, _)| *ce == e) || q_pots.get(e).is_ok() {
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
                    let (cm, crit) = roll_crit(stats.crit_chance, stats.crit_damage, &mut rng);
                    let elite = enemies.get(*e).map(|(_, _, en)| if en.elite { stats.elite_damage } else { 1.0 }).unwrap_or(1.0);
                    hits.write(HitMsg { target: *e, amount: dmg * cm * elite, crit, knock: Vec3::ZERO });
                    // zap segment visual
                    let mid = (prev + *pos) / 2.0;
                    let len = prev.distance(*pos);
                    let dirv = (*pos - prev).normalize_or_zero();
                    commands.spawn((
                        Mesh3d(assets.beam_mesh.clone()),
                        MeshMaterial3d(assets.mats[&wi.kind].clone()),
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
                for _ in 0..count {
                    let h = Quat::from_axis_angle(up, rng.gen_range(-0.6..0.6)) * aim;
                    commands.spawn((
                        Projectile {
                            dir: player.dir,
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
                        MeshMaterial3d(assets.mats[&wi.kind].clone()),
                        Transform::from_translation(origin).with_scale(Vec3::splat(size * 1.3)),
                        StageScoped,
                    ));
                }
            }
            Behavior::Orbit { .. } | Behavior::Aura { .. } => unreachable!(),
        }
    }

    // ------ drone reconciliation
    let mut have: HashMap<WeaponKind, usize> = HashMap::new();
    for (e, d) in q_drones.iter() {
        let alive = want_drones.get(&d.weapon);
        match alive {
            Some((want_count, dmg, radius, dps)) => {
                have.entry(d.weapon).and_modify(|c| *c += 1).or_insert(1);
                if d.idx >= *want_count {
                    commands.entity(e).despawn();
                } else {
                    // keep tuning current
                    commands.entity(e).insert(Drone {
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
    for (kind, (count, dmg, radius, dps)) in &want_drones {
        let existing = have.get(kind).copied().unwrap_or(0);
        for idx in existing..*count {
            commands.spawn((
                Drone {
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
                Transform::from_translation(ptf.translation),
                StageScoped,
            ));
        }
    }

    // ------ aura visuals reconciliation
    for (e, a) in q_auras.iter() {
        if !want_auras.contains(&a.weapon) {
            commands.entity(e).despawn();
        }
    }
    let existing_auras: Vec<WeaponKind> = q_auras.iter().map(|(_, a)| a.weapon).collect();
    for kind in want_auras {
        if !existing_auras.contains(&kind) {
            if let Behavior::Aura { radius, .. } = kind.def().behavior {
                commands.spawn((
                    AuraVis { weapon: kind },
                    Mesh3d(assets.aura_mesh.clone()),
                    MeshMaterial3d(assets.mats[&kind].clone()),
                    Transform::from_translation(ptf.translation).with_scale(Vec3::splat(radius)),
                    StageScoped,
                ));
            }
        }
    }
    let _ = weapons_snapshot;
}

/// Move player projectiles, collide with the swarm via the spatial hash.
pub fn projectile_move(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    run: Res<RunState>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<(&Player, &Transform), Without<Projectile>>,
    enemies: Query<&Enemy>,
    q_pots: Query<(), With<Pot>>,
    mut q: Query<(Entity, &mut Projectile, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };
    let mut rng = rand::thread_rng();

    for (pe, mut p, mut tf) in &mut q {
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
                explode(&mut commands, &hash, &enemies, &run, tf.translation, aoe, p.damage, &mut hits, &particles, p.dir, &mut rng);
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
                    explode(&mut commands, &hash, &enemies, &run, tf.translation, aoe, p.damage, &mut hits, &particles, p.dir, &mut rng);
                    exploded = true;
                    break;
                }
                let (cm, crit) = roll_crit(run.stats.crit_chance, run.stats.crit_damage, &mut rng);
                let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                hits.write(HitMsg {
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
    let _ = player;
}

#[allow(clippy::too_many_arguments)]
fn explode(
    commands: &mut Commands,
    hash: &SpatialHash,
    enemies: &Query<&Enemy>,
    run: &RunState,
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
            let (cm, crit) = roll_crit(run.stats.crit_chance, run.stats.crit_damage, rng);
            let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
            let kdir = (tpos - pos).normalize_or_zero();
            hits.write(HitMsg { target: te, amount: damage * cm * elite, crit, knock: kdir * 7.0 });
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
    run: Res<RunState>,
    q_player: Query<(&Player, &Transform), Without<Drone>>,
    enemies: Query<&Enemy>,
    mut q: Query<(&mut Drone, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };
    let mut rng = rand::thread_rng();
    let t = time.elapsed_secs();

    for (mut d, mut tf) in &mut q {
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
                    let (cm, crit) = roll_crit(run.stats.crit_chance, run.stats.crit_damage, &mut rng);
                    let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg {
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
    run: Res<RunState>,
    q_player: Query<(&Player, &Transform), Without<Beam>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<Buried>, Without<Beam>, Without<Player>)>,
    mut q: Query<(Entity, &mut Beam, &mut Transform), Without<Player>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, ptf)) = q_player.single() else { return };
    let mut rng = rand::thread_rng();

    for (e, mut beam, mut tf) in &mut q {
        beam.tick_cd -= dt;
        let up = player.dir;
        let origin = ptf.translation + up * 0.6;
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
                    let (cm, crit) = roll_crit(run.stats.crit_chance, run.stats.crit_damage, &mut rng);
                    let elite = if en.elite { run.stats.elite_damage } else { 1.0 };
                    hits.write(HitMsg { target: te, amount: beam.damage * cm * elite, crit, knock: Vec3::ZERO });
                }
            }
        }
    }
}

/// Aura visuals follow the player and pulse; cryo slow applied on hit below.
pub fn aura_follow(
    time: Res<Time>,
    q_player: Query<&Transform, (With<Player>, Without<AuraVis>)>,
    mut q: Query<(&AuraVis, &mut Transform), Without<Player>>,
) {
    let Ok(ptf) = q_player.single() else { return };
    let t = time.elapsed_secs();
    for (a, mut tf) in &mut q {
        if let Behavior::Aura { radius, .. } = a.weapon.def().behavior {
            tf.translation = ptf.translation;
            tf.scale = Vec3::splat(radius * (1.0 + (t * 3.0).sin() * 0.04));
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
    mut shake: ResMut<Shake>,
    mut hitstop: ResMut<Hitstop>,
    mut enemies: Query<(&mut Enemy, &Transform, Option<&Boss>), Without<Pot>>,
    mut pots: Query<(&mut Pot, &Transform)>,
    mut kills: MessageWriter<KillMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let mut rng = rand::thread_rng();
    let has_cryo = run
        .weapons
        .iter()
        .any(|w| matches!(w.kind, WeaponKind::CryoVent | WeaponKind::AbsoluteZero));

    for msg in reader.read() {
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
        if let Ok((mut e, tf, boss)) = enemies.get_mut(msg.target) {
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
            // lifesteal: chance to heal 1
            if run.stats.lifesteal > 0.0 && rng.gen_bool((run.stats.lifesteal.min(1.0)) as f64) {
                run.hp = (run.hp + 1.0).min(run.stats.max_hp);
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
                if is_boss {
                    run.boss_dead = true;
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

/// Resolve hits on the player: evasion -> shield -> armor -> hp, thorns reflect.
pub fn apply_player_hits(
    mut reader: MessageReader<PlayerHitMsg>,
    mut run: ResMut<RunState>,
    mut shake: ResMut<Shake>,
    mut phase: ResMut<RunPhase>,
    q_player: Query<&Transform, With<Player>>,
    mut hits: MessageWriter<HitMsg>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let Ok(ptf) = q_player.single() else { return };
    let mut rng = rand::thread_rng();
    for msg in reader.read() {
        if run.iframes > 0.0 || run.hp <= 0.0 {
            continue;
        }
        // evasion
        if rng.gen_bool(run.stats.evasion_fraction() as f64) {
            numbers.write(NumberMsg { pos: ptf.translation, amount: 0.0, kind: NumKind::Dodge });
            continue;
        }
        let mut amount = msg.amount * (1.0 - run.stats.armor_fraction());
        // shield first
        if run.shield > 0.0 {
            let absorbed = run.shield.min(amount);
            run.shield -= absorbed;
            amount -= absorbed;
        }
        run.shield_cd = 5.0;
        run.hp -= amount;
        run.iframes = 0.4;
        shake.add(0.12);
        sfx.write(SfxMsg(Sfx::Hurt));

        // thorns
        if run.stats.thorns > 0.0 {
            if let Some(att) = msg.attacker {
                hits.write(HitMsg { target: att, amount: run.stats.thorns, crit: false, knock: Vec3::ZERO });
            }
        }

        if run.hp <= 0.0 {
            run.hp = 0.0;
            run.result = Some(crate::run::RunResult::Death);
            *phase = RunPhase::Dead;
        }
    }
}
