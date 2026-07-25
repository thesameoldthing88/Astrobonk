//! Drops and pickups: XP gems, gold, food, powerups, silver — with magnet
//! attraction and over-cap gem merging.

use crate::config::*;
use crate::fx::{self, Pcolor, ParticleAssets};
use crate::messages::*;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::Player;
use crate::run::{PlayerState, PowerupKind, RunState};
use bevy::prelude::*;
use rand::Rng;

#[derive(Clone, Copy, PartialEq)]
pub enum PickupKind {
    Xp(f32),
    Gold(u64),
    Silver(u64),
    Food,
    Powerup(PowerupKind),
}

#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    pub dir: Vec3,
    pub flying: bool,
    pub speed: f32,
    pub bob: f32,
}

#[derive(Resource)]
pub struct PickupAssets {
    pub gem_mesh: Handle<Mesh>,
    pub gem_mat: Handle<StandardMaterial>,
    pub big_gem_mat: Handle<StandardMaterial>,
    pub coin_mesh: Handle<Mesh>,
    pub coin_mat: Handle<StandardMaterial>,
    pub silver_mat: Handle<StandardMaterial>,
    pub food_mesh: Handle<Mesh>,
    pub food_mat: Handle<StandardMaterial>,
    pub power_mesh: Handle<Mesh>,
    pub power_mat: Handle<StandardMaterial>,
}

pub fn setup_pickup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let emissive = |c: Color, s: f32| StandardMaterial {
        base_color: c,
        emissive: c.to_linear() * s,
        unlit: true,
        ..default()
    };
    commands.insert_resource(PickupAssets {
        gem_mesh: meshes.add(Mesh::from(Sphere::new(0.22))),
        gem_mat: materials.add(emissive(Color::srgb(0.25, 1.0, 0.4), 2.2)),
        big_gem_mat: materials.add(emissive(Color::srgb(0.3, 0.7, 1.0), 3.0)),
        coin_mesh: meshes.add(Mesh::from(Cylinder::new(0.22, 0.07))),
        coin_mat: materials.add(emissive(Color::srgb(1.0, 0.85, 0.2), 2.0)),
        silver_mat: materials.add(emissive(Color::srgb(0.8, 0.9, 1.0), 2.6)),
        food_mesh: meshes.add(Mesh::from(Cuboid::new(0.34, 0.34, 0.34))),
        food_mat: materials.add(emissive(Color::srgb(1.0, 0.35, 0.35), 1.6)),
        power_mesh: meshes.add(Mesh::from(Sphere::new(0.3))),
        power_mat: materials.add(emissive(Color::srgb(0.8, 0.4, 1.0), 3.0)),
    });
}

pub fn spawn_pickup(
    commands: &mut Commands,
    assets: &PickupAssets,
    planet: &CurrentPlanet,
    dir: Vec3,
    kind: PickupKind,
) {
    let mut rng = rand::thread_rng();
    // scatter a touch
    let (t, b) = crate::sphere::tangent_frame(dir);
    let a = rng.gen_range(0.0..std::f32::consts::TAU);
    let jitter = rng.gen_range(0.0..1.2);
    let dir = crate::sphere::offset_dir(dir, t * a.cos() + b * a.sin(), jitter, planet.radius);

    let (mesh, mat, scale) = match kind {
        PickupKind::Xp(v) => (
            assets.gem_mesh.clone(),
            if v >= 10.0 { assets.big_gem_mat.clone() } else { assets.gem_mat.clone() },
            if v >= 10.0 { 1.6 } else { 1.0 },
        ),
        PickupKind::Gold(_) => (assets.coin_mesh.clone(), assets.coin_mat.clone(), 1.0),
        PickupKind::Silver(_) => (assets.coin_mesh.clone(), assets.silver_mat.clone(), 1.1),
        PickupKind::Food => (assets.food_mesh.clone(), assets.food_mat.clone(), 1.0),
        PickupKind::Powerup(_) => (assets.power_mesh.clone(), assets.power_mat.clone(), 1.0),
    };
    let pos = planet.surface_point(dir) + dir * 0.35;
    commands.spawn((
        Pickup { kind, dir, flying: false, speed: 0.0, bob: rng.gen_range(0.0..6.28) },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
        StageScoped,
    ));
}

/// Attraction + collection.
#[allow(clippy::too_many_arguments)]
pub fn pickup_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut q_player: Query<(&Player, &mut PlayerState, &Transform), Without<Pickup>>,
    particles: Option<Res<ParticleAssets>>,
    mut q: Query<(Entity, &mut Pickup, &mut Transform), Without<Player>>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((player, mut ps, ptf)) = q_player.single_mut() else { return };
    let range = ps.pickup_range();
    let t_now = time.elapsed_secs();

    for (e, mut p, mut tf) in &mut q {
        let d = tf.translation.distance(ptf.translation);
        if !p.flying && d < range {
            p.flying = true;
            p.speed = 6.0;
        }
        if p.flying {
            p.speed = (p.speed + 60.0 * dt).min(PICKUP_FLY_SPEED * 1.8);
            let to = (ptf.translation - tf.translation).normalize_or_zero();
            tf.translation += to * p.speed * dt;
            if tf.translation.distance(ptf.translation) < 0.8 {
                collect(&mut run, &mut ps, p.kind, ptf.translation, &mut numbers, &mut sfx, &mut banners);
                if let Some(pa) = &particles {
                    let c = match p.kind {
                        PickupKind::Xp(_) => Pcolor::Green,
                        PickupKind::Gold(_) => Pcolor::Gold,
                        PickupKind::Silver(_) => Pcolor::Blue,
                        PickupKind::Food => Pcolor::Red,
                        PickupKind::Powerup(_) => Pcolor::Purple,
                    };
                    fx::burst(&mut commands, pa, ptf.translation, player.dir, c, 4, 3.0);
                }
                commands.entity(e).despawn();
                continue;
            }
        } else {
            // idle bob + spin on the surface
            let up = p.dir;
            tf.translation = planet.surface_point(up) + up * (0.35 + ((t_now * 2.0 + p.bob).sin() * 0.08));
            tf.rotation = Quat::from_axis_angle(up, t_now * 1.5 + p.bob);
        }
    }
}

fn collect(
    run: &mut RunState,
    ps: &mut PlayerState,
    kind: PickupKind,
    pos: Vec3,
    numbers: &mut MessageWriter<NumberMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    banners: &mut MessageWriter<BannerMsg>,
) {
    match kind {
        PickupKind::Xp(v) => {
            ps.gain_xp(v);
            sfx.write(SfxMsg(Sfx::Pickup));
        }
        PickupKind::Gold(g) => {
            let g = (g as f32 * ps.stats.gold_gain).round() as u64;
            ps.gold += g;
            run.gold_collected += g;
            sfx.write(SfxMsg(Sfx::Coin));
        }
        PickupKind::Silver(s) => {
            let s = (s as f32 * ps.stats.silver_gain).round() as u64;
            run.silver_run += s;
            sfx.write(SfxMsg(Sfx::Coin));
        }
        PickupKind::Food => {
            let heal = ps.stats.max_hp * 0.2;
            ps.hp = (ps.hp + heal).min(ps.stats.max_hp);
            numbers.write(NumberMsg { pos, amount: heal, kind: NumKind::Heal });
            sfx.write(SfxMsg(Sfx::Pickup));
        }
        PickupKind::Powerup(k) => {
            let (name, secs) = match k {
                PowerupKind::Damage2x => ("2x DAMAGE!", 20.0),
                PowerupKind::Magnet => ("MEGA MAGNET!", 12.0),
                PowerupKind::Speed => ("SPEED BOOST!", 15.0),
            };
            ps.powerups.retain(|(pk, _)| *pk != k);
            ps.powerups.push((k, secs));
            banners.write(BannerMsg(name.into()));
            sfx.write(SfxMsg(Sfx::LevelUp));
        }
    }
}

/// Deaths drop loot.
pub fn kill_drops(
    mut commands: Commands,
    mut reader: MessageReader<KillMsg>,
    assets: Res<PickupAssets>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    q_ps: Query<&PlayerState>,
    particles: Option<Res<ParticleAssets>>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let mut rng = rand::thread_rng();
    for msg in reader.read() {
        if msg.is_pot {
            run.pots_broken += 1;
            // pots: gold, sometimes silver or food
            let roll = rng.gen_range(0.0..1.0);
            if roll < 0.12 {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Silver(rng.gen_range(1..3)));
            } else if roll < 0.24 {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Food);
            } else {
                for _ in 0..rng.gen_range(1..4) {
                    spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Gold(rng.gen_range(2..7)));
                }
            }
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, msg.pos, msg.dir, Pcolor::Gold, 8, 5.0);
            }
            continue;
        }

        run.kills += 1;

        if run.static_active {
            // ghosts pay silver
            if rng.gen_bool(0.5) {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Silver(1));
            }
        } else if msg.xp > 0.0 {
            spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Xp(msg.xp));
        }

        if msg.elite {
            for _ in 0..rng.gen_range(4..8) {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Gold(rng.gen_range(4..10)));
            }
            if rng.gen_bool(0.35) {
                let kinds = [PowerupKind::Damage2x, PowerupKind::Magnet, PowerupKind::Speed];
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Powerup(kinds[rng.gen_range(0..3)]));
            }
        } else {
            if rng.gen_bool(0.07) {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Gold(rng.gen_range(1..4)));
            }
            if rng.gen_bool(0.012) {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Food);
            }
            if rng.gen_bool(0.006) {
                let kinds = [PowerupKind::Damage2x, PowerupKind::Magnet, PowerupKind::Speed];
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Powerup(kinds[rng.gen_range(0..3)]));
            }
        }

        if msg.is_boss {
            sfx.write(SfxMsg(Sfx::BossRoar));
            for _ in 0..14 {
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Gold(rng.gen_range(8..20)));
            }
        }

        if let Some(pa) = &particles {
            let color = if msg.elite { Pcolor::Gold } else { Pcolor::Green };
            let n = if msg.is_boss { 40 } else if msg.elite { 16 } else { 6 };
            fx::burst(&mut commands, pa, msg.pos, msg.dir, color, n, if msg.is_boss { 12.0 } else { 6.0 });
        }
    }
}

/// Keep the gem population under control: merge the oldest into big gems.
pub fn gem_merge(
    mut commands: Commands,
    q: Query<(Entity, &Pickup, &Transform)>,
    assets: Res<PickupAssets>,
    planet: Res<CurrentPlanet>,
) {
    let gems: Vec<(Entity, f32, Vec3)> = q
        .iter()
        .filter_map(|(e, p, tf)| match p.kind {
            PickupKind::Xp(v) => Some((e, v, tf.translation)),
            _ => None,
        })
        .collect();
    if gems.len() <= GEM_CAP {
        return;
    }
    let excess = gems.len() - GEM_CAP + 40;
    let mut total = 0.0;
    let mut center = Vec3::ZERO;
    for (e, v, pos) in gems.iter().take(excess) {
        total += v;
        center += *pos;
        commands.entity(*e).despawn();
    }
    center /= excess as f32;
    let dir = center.normalize_or_zero();
    if dir != Vec3::ZERO && total > 0.0 {
        spawn_pickup(&mut commands, &assets, &planet, dir, PickupKind::Xp(total));
    }
}
