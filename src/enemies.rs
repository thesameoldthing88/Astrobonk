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
}

#[derive(Component)]
pub struct Boss {
    pub kind: BossKind,
    pub attack_timer: f32,
    pub burst_timer: f32,
}

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
        let mesh = match kind {
            EnemyKind::Shambler => Mesh::from(Capsule3d::new(0.42, 0.5)),
            EnemyKind::Sprinter => Mesh::from(Cone::new(0.38, 1.0)),
            EnemyKind::Bruiser => Mesh::from(Cuboid::new(0.95, 1.1, 0.8)),
            EnemyKind::Spitter => Mesh::from(Sphere::new(0.5)),
            EnemyKind::Ufo => Mesh::from(Sphere::new(0.62)),
            EnemyKind::Burrower => Mesh::from(Capsule3d::new(0.38, 0.7)),
            EnemyKind::Beamer => Mesh::from(Cone::new(0.32, 1.7)),
            EnemyKind::Lobber => Mesh::from(Cylinder::new(0.62, 0.65)),
            EnemyKind::Ghost => Mesh::from(Cone::new(0.45, 1.2)),
        };
        mesh_map.insert(kind, meshes.add(mesh));
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
    let mut rng = rand::thread_rng();

    let alive = q_enemies.iter().count();
    let (hp_mult, dmg_mult) = time_scaling(run.elapsed, run.stats.difficulty);

    let rate = if run.static_active {
        10.0 + run.static_timer * 0.15
    } else {
        let t = run.elapsed / 60.0;
        (1.5 + t * 2.2) * (1.0 + run.stats.difficulty)
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
            spawn_enemy(&mut commands, &assets, &planet, EnemyKind::Ghost, dir, false, hp_mult, dmg_mult, &mut rng);
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
        spawn_enemy(&mut commands, &assets, &planet, kind, dir, elite, hp_mult, dmg_mult, &mut rng);
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
    let mesh = meshes.add(Mesh::from(Capsule3d::new(0.6, 0.9)));
    commands.spawn((
        Enemy {
            kind: EnemyKind::Bruiser,
            dir,
            hover: 0.0,
            speed: def.speed,
            damage: def.damage * (1.0 + difficulty * 0.5),
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
        },
        Boss { kind, attack_timer: 4.0, burst_timer: 7.0 },
        Mesh3d(mesh),
        MeshMaterial3d(assets.boss_mat.clone()),
        BaseMat(assets.boss_mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(def.scale)),
        StageScoped,
    ));
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
        let bob = if e.hover > 0.0 { (t_now * 2.2 + e.wobble).sin() * 0.35 } else { 0.0 };
        let pos = planet.surface_point(up) + up * (e.hover + bob + e.scale * 0.6);
        tf.translation = pos;

        let face = (player_pos - pos).normalize_or_zero();
        let mut rot = sphere::frame_quat(up, face);
        if e.hover == 0.0 {
            rot *= Quat::from_rotation_z((t_now * (3.0 + e.speed) + e.wobble).sin() * 0.09);
        }
        tf.rotation = rot;
        let flash_pulse = 1.0 + e.flash * 0.25;
        tf.scale = Vec3::splat(e.scale * flash_pulse);
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
    q_player: Query<&Player>,
    mut q: Query<(&Enemy, &mut Spitter, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
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
    q_player: Query<(&Player, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &Enemy, &mut Beamer, &Transform), Without<Buried>>,
    mut q_lines: Query<(Entity, &AimLine, &mut Transform), (Without<Enemy>, Without<Player>)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
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
    q_player: Query<&Player>,
    mut q: Query<(&Enemy, &mut Lobber, &Transform), Without<Buried>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
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
