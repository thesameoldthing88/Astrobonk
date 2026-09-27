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

#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// Who this pickup is flying to. LATCHED: with two attractors a gem released between
    /// them would re-pick the nearest every frame and stall in the middle.
    pub target: Option<Entity>,
    /// Seconds it has lain untouched (Tome of the Horizon waits this out). HOST only.
    pub idle: f32,
    /// Flying home from over the horizon (Tome of the Horizon): it travels ALONG the
    /// surface — `dir` moves with it — since a straight line would cut through the planet.
    pub arc: bool,
}

impl Pickup {
    pub fn new(kind: PickupKind, dir: Vec3, bob: f32) -> Self {
        Self { kind, dir, flying: false, speed: 0.0, bob, target: None, idle: 0.0, arc: false }
    }

    /// Tome of the Horizon calls this gem home: it lifts off along the surface.
    pub fn call_home(&mut self) {
        self.flying = true;
        self.arc = true;
        self.speed = HORIZON_FLY_START;
    }

    /// Its caller went down or left before it arrived: it lies where it got to, anybody's
    /// again, and waits out the horizon from scratch.
    pub fn settle(&mut self, dir: Vec3) {
        self.dir = dir;
        self.flying = false;
        self.arc = false;
        self.speed = 0.0;
        self.idle = 0.0;
        self.target = None;
    }

    /// One frame of the flight home to the astronaut over surface point `to`, returning
    /// where the gem is drawn. The host and a client both fly it with this, so they draw one
    /// flight; within HORIZON_HANDOFF_ARC `arc` clears and `magnet_step` takes it in.
    pub fn fly_home(&mut self, to: Vec3, dt: f32, planet: &CurrentPlanet) -> Vec3 {
        self.speed = (self.speed + HORIZON_FLY_ACCEL * dt).min(HORIZON_FLY_SPEED);
        let mut dir = self.dir;
        let at = horizon_flight(&mut dir, to, self.speed * dt, planet.radius, planet);
        self.dir = dir;
        if crate::sphere::arc_dist(self.dir, to, planet.radius) < HORIZON_HANDOFF_ARC {
            self.arc = false;
        }
        at
    }

    /// One frame of the ordinary magnet flight from `at` straight at `pos`.
    pub fn magnet_step(&mut self, at: Vec3, pos: Vec3, dt: f32) -> Vec3 {
        self.speed = (self.speed + 60.0 * dt).min(PICKUP_FLY_SPEED * 1.8);
        at + (pos - at).normalize_or_zero() * self.speed * dt
    }
}

/// HOST: this gem was called home from over the horizon by player `.0` (Tome of the
/// Horizon). `netenemy::stream_pickups` announces it, so a client draws the same flight, and
/// announces where it settled if the host takes it off again (its caller went down).
#[derive(Component, Clone, Copy)]
pub struct HorizonBound(pub u8);

/// HOST: Silver one of The Static's ghosts dropped — what Tome of Static pays on
/// (`RunState::static_silver_found`), tagged where it drops so neither a pot broken during
/// The Static nor a ghost coin picked up after it ends is misread.
#[derive(Component)]
pub struct StaticSilver;

/// One frame of a horizon flight: move `dir` along the surface toward `to` by `step` metres
/// on a planet of `radius`, and return where the gem is drawn — lifted off the ground in the
/// middle of a long flight, settling as it arrives. An exact antipode has no unique great
/// circle, so that one sets off along any tangent.
pub fn horizon_flight(dir: &mut Vec3, to: Vec3, step: f32, radius: f32, planet: &CurrentPlanet) -> Vec3 {
    let angle = step / radius.max(1.0);
    let next = crate::sphere::step_toward(*dir, to, angle);
    *dir = if next == *dir && dir.dot(to) < 0.0 {
        crate::sphere::offset_dir(*dir, crate::sphere::tangent_frame(*dir).0, step, radius)
    } else {
        next
    };
    let left = crate::sphere::arc_dist(*dir, to, radius);
    let lift = (left * 0.15).min(HORIZON_FLY_LIFT);
    planet.surface_point(*dir) + *dir * (0.35 + lift)
}

#[derive(Resource)]
pub struct PickupAssets {
    pub gem_mesh: Handle<Mesh>,
    pub gem_mat: Handle<StandardMaterial>,
    pub big_gem_mat: Handle<StandardMaterial>,
    /// §8 Farside: the same gems burning brighter (`daynight::farside_gems` swaps them in).
    pub gem_far_mat: Handle<StandardMaterial>,
    pub big_gem_far_mat: Handle<StandardMaterial>,
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
        // unlit draws base colour only, so "brighter" is an over-white base the bloom catches
        gem_far_mat: materials.add(emissive(Color::LinearRgba(Color::srgb(0.25, 1.0, 0.4).to_linear() * FARSIDE_GEM_GLOW), 2.2)),
        big_gem_far_mat: materials.add(emissive(Color::LinearRgba(Color::srgb(0.3, 0.7, 1.0).to_linear() * FARSIDE_GEM_GLOW), 3.0)),
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
) -> Entity {
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
    commands
        .spawn((
            Pickup::new(kind, dir, rng.gen_range(0.0..6.28)),
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
            StageScoped,
        ))
        .id()
}

/// Attraction + collection — and Tome of the Horizon, which calls XP that has lain over its
/// holder's horizon long enough home across the planet.
#[allow(clippy::too_many_arguments)]
pub fn pickup_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut q_player: Query<(Entity, &Player, &mut PlayerState, &Transform, Has<crate::player::LocalPlayer>), Without<Pickup>>,
    particles: Option<Res<ParticleAssets>>,
    mut q: Query<(Entity, &mut Pickup, &mut Transform, Has<HorizonBound>, Has<StaticSilver>), Without<Player>>,
    mut numbers: MessageWriter<NumberMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
    mut grants: MessageWriter<crate::net::GrantOut>,
    q_ids: Query<&crate::player::PlayerId>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t_now = time.elapsed_secs();

    // (1) Snapshot every attractor BEFORE touching the pickup query (B0001), with a
    // PER-PLAYER range — the Magnet powerup multiplies it 40x, so a shared range would
    // let one player's powerup vacuum the planet into someone else's pocket.
    struct Attractor {
        entity: Entity,
        pid: u8,
        pos: Vec3,
        dir: Vec3,
        range: f32,
        /// Tome of the Horizon: seconds XP must lie over this astronaut's horizon before it
        /// flies home. None without the tome.
        horizon_wait: Option<f32>,
        is_local: bool,
    }
    let attractors: Vec<Attractor> = q_player
        .iter()
        .filter(|(_, _, ps, _, _)| !ps.dead)
        .map(|(e, pl, ps, tf, is_local)| Attractor {
            entity: e,
            pid: q_ids.get(e).map(|id| id.0).unwrap_or(0),
            pos: tf.translation,
            dir: pl.dir,
            range: ps.pickup_range(),
            horizon_wait: (ps.stats.horizon_collect > 0.0).then(|| 1.0 / ps.stats.horizon_collect),
            is_local,
        })
        .collect();
    let horizon = attractors.iter().any(|a| a.horizon_wait.is_some());

    // (2) Move pickups and record what got collected — resolve ownership PER PICKUP so a
    // gem inside 0.8m of two astronauts is granted (and despawned) exactly once.
    let mut collected: Vec<(Entity, PickupKind, Vec3, bool, bool)> = Vec::new();

    for (e, mut p, mut tf, called_home, from_static) in &mut q {
        let cur = p
            .target
            .and_then(|t| attractors.iter().find(|a| a.entity == t));
        if cur.is_none() && called_home {
            // its caller went down or left before it arrived: it settles where it got to,
            // and the call is withdrawn so the clients stop flying it too
            let dir = p.dir;
            p.settle(dir);
            commands.entity(e).remove::<HorizonBound>();
        }
        let mut target = match cur {
            Some(a) => Some(a),
            None => attractors
                .iter()
                .filter(|a| tf.translation.distance(a.pos) < a.range)
                .min_by(|a, b| {
                    tf.translation
                        .distance(a.pos)
                        .total_cmp(&tf.translation.distance(b.pos))
                }),
        };
        // Tome of the Horizon: XP nobody is reaching for, lying over the horizon of a holder
        // for long enough, is called home to the nearest such holder.
        if target.is_none() && horizon && matches!(p.kind, PickupKind::Xp(_)) {
            p.idle += dt;
            let called = attractors
                .iter()
                .filter(|a| a.horizon_wait.is_some_and(|w| p.idle >= w))
                .map(|a| (a, crate::sphere::arc_dist(p.dir, a.dir, planet.radius)))
                .filter(|(_, arc)| *arc > HORIZON_ARC)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(a, _)| a);
            if let Some(a) = called {
                p.call_home();
                commands.entity(e).try_insert(HorizonBound(a.pid));
                target = Some(a);
            }
        }
        if let Some(a) = target {
            if !p.flying {
                p.flying = true;
                p.speed = 6.0;
            }
            p.target = Some(a.entity);
            if p.arc {
                // home along the surface; the last stretch is the ordinary magnet flight
                tf.translation = p.fly_home(a.dir, dt, &planet);
                continue;
            }
            tf.translation = p.magnet_step(tf.translation, a.pos, dt);
            if tf.translation.distance(a.pos) < 0.8 {
                collected.push((a.entity, p.kind, a.pos, a.is_local, from_static));
                if let Some(pa) = &particles {
                    let c = match p.kind {
                        PickupKind::Xp(_) => Pcolor::Green,
                        PickupKind::Gold(_) => Pcolor::Gold,
                        PickupKind::Silver(_) => Pcolor::Blue,
                        PickupKind::Food => Pcolor::Red,
                        PickupKind::Powerup(_) => Pcolor::Purple,
                    };
                    // unconditional: everyone should see a teammate's pop
                    fx::burst(&mut commands, pa, a.pos, a.dir, c, 4, 3.0);
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

    // (3) Grant. XP is a SHARED pool on an individual curve (each player's own xp_gain and
    // level thresholds still apply); gold, food and powerups belong to the collector.
    for (collector, kind, pos, is_local, from_static) in collected {
        if let PickupKind::Xp(v) = kind {
            for (_, _, mut ps, _, _) in &mut q_player {
                if !ps.dead {
                    ps.gain_xp(v);
                }
            }
            // Every client applies the same grant to its own PlayerState, which is what
            // keeps card picks local while the pool stays shared.
            grants.write(crate::net::GrantOut::Xp(v));
            if is_local {
                sfx.write(SfxMsg(Sfx::Pickup));
            }
            continue;
        }
        // Gold goes over the wire AFTER the collector's Gold gain: the joiner adds what it
        // is told, so a raw amount would silently drop its Golden Antennas and Signal Flare.
        let mut granted = kind;
        if let Ok((_, _, mut ps, _, _)) = q_player.get_mut(collector) {
            if let PickupKind::Gold(g) = kind {
                granted = PickupKind::Gold((g as f32 * ps.stats.gold_gain).round() as u64);
            }
            collect(&mut run, &mut ps, kind, from_static, pos, is_local, &mut numbers, &mut sfx, &mut banners);
        }
        if !is_local {
            // loot picked up by a REMOTE astronaut has to reach that player's machine
            if let Ok(pid) = q_ids.get(collector) {
                grants.write(crate::net::GrantOut::Loot(pid.0, granted));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect(
    run: &mut RunState,
    ps: &mut PlayerState,
    kind: PickupKind,
    from_static: bool,
    pos: Vec3,
    is_local: bool,
    numbers: &mut MessageWriter<NumberMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    banners: &mut MessageWriter<BannerMsg>,
) {
    match kind {
        PickupKind::Xp(v) => {
            // shared-pool XP is granted by the caller across all players
            ps.gain_xp(v);
            if is_local {
                sfx.write(SfxMsg(Sfx::Pickup));
            }
        }
        PickupKind::Gold(g) => {
            let g = (g as f32 * ps.stats.gold_gain).round() as u64;
            ps.gold += g;
            run.gold_collected += g;
            if is_local {
                sfx.write(SfxMsg(Sfx::Coin));
            }
        }
        PickupKind::Silver(s) => {
            let s = (s as f32 * ps.stats.silver_gain).round() as u64;
            run.silver_run += s;
            // Tome of Static multiplies this total once, at banking (`director::silver_payout`):
            // per coin, a x1.3 on a 1-Silver ghost drop would round away to nothing
            if from_static {
                run.static_silver_found += s;
            }
            if is_local {
                sfx.write(SfxMsg(Sfx::Coin));
            }
        }
        PickupKind::Food => {
            let heal = ps.stats.max_hp * 0.2;
            ps.hp = (ps.hp + heal).min(ps.stats.max_hp);
            numbers.write(NumberMsg { pos, amount: heal, kind: NumKind::Heal });
            if is_local {
                sfx.write(SfxMsg(Sfx::Pickup));
            }
        }
        PickupKind::Powerup(k) => {
            let (name, secs) = match k {
                PowerupKind::Damage2x => ("2x DAMAGE!", 20.0),
                PowerupKind::Magnet => ("MEGA MAGNET!", 12.0),
                PowerupKind::Speed => ("SPEED BOOST!", 15.0),
            };
            ps.powerups.retain(|(pk, _)| *pk != k);
            ps.powerups.push((k, secs));
            if is_local {
                banners.write(BannerMsg(name.into()));
            }
            if is_local {
                sfx.write(SfxMsg(Sfx::LevelUp));
            }
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
    particles: Option<Res<ParticleAssets>>,
    mut sfx: MessageWriter<SfxMsg>,
    (save, mut flash_gate, time): (Res<crate::save::MetaSave>, ResMut<fx::FlashGate>, Res<Time>),
) {
    let mut rng = rand::thread_rng();
    // §4 night reward: kills on the night side drop +25% Gold (pots are not kills)
    let sun = crate::daynight::Sun::of(&run);
    for msg in reader.read() {
        let gold = |n: u64, rng: &mut rand::rngs::ThreadRng| PickupKind::Gold(sun.kill_gold(msg.dir, n, rng));
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

        if run.static_active || msg.kind == Some(crate::content::enemies::EnemyKind::Ghost) {
            // ghosts pay silver (tagged: Tome of Static pays on exactly these) — a Cursed-
            // Touched elite's mini-Static before the clock runs out too (§9)
            if rng.gen_bool(0.5) {
                let coin = spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Silver(1));
                commands.entity(coin).insert(StaticSilver);
            }
        } else if msg.xp > 0.0 {
            spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Xp(msg.xp));
        }

        if msg.elite {
            let more = elite_loot_mult(msg, &run);
            // §9: the loot grows with a Glitched elite's stack (two affixes and up: a
            // powerup for certain)
            let extra = msg.affixes.len().saturating_sub(1) as f32;
            let coins = (rng.gen_range(4..8) as f32 * more * (1.0 + AFFIX_LOOT_PER_EXTRA * extra)).round() as u32;
            for _ in 0..coins {
                let n = rng.gen_range(4..10);
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, gold(n, &mut rng));
            }
            if rng.gen_bool(((0.35 + AFFIX_POWERUP_PER_EXTRA * extra) * more).min(if extra > 0.0 { 1.0 } else { 0.95 }) as f64) {
                let kinds = [PowerupKind::Damage2x, PowerupKind::Magnet, PowerupKind::Speed];
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, PickupKind::Powerup(kinds[rng.gen_range(0..3)]));
            }
        } else {
            if rng.gen_bool(0.07) {
                let n = rng.gen_range(1..4);
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, gold(n, &mut rng));
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
                let n = rng.gen_range(8..20);
                spawn_pickup(&mut commands, &assets, &planet, msg.dir, gold(n, &mut rng));
            }
        }

        // Photosensitivity: death bursts draw on the screen's one under-3/s flash budget —
        // The Static dying in waves is otherwise a strobe. A boss always gets its burst (one
        // a stage, and the kill needs its confirmation), and it spends the budget too.
        let now = time.elapsed_secs();
        let show_burst = if !save.accessibility.photosensitive {
            true
        } else if msg.is_boss {
            flash_gate.mark(now);
            true
        } else {
            flash_gate.allow(now)
        };
        if let (Some(pa), true) = (&particles, show_burst) {
            let color = if msg.elite { Pcolor::Gold } else { Pcolor::Green };
            let n = if msg.is_boss { 40 } else if msg.elite { 16 } else { 6 };
            fx::burst(&mut commands, pa, msg.pos, msg.dir, color, n, if msg.is_boss { 12.0 } else { 6.0 });
        }
    }
}

/// Tome of the Elite's multiplier on an elite kill's coins and powerup odds: the party's
/// best (`RunState::elite_loot`) for an elite of the horde, 1 for the boss and minibosses —
/// they share the elite loot table, but §7's tome is about elites ("big game"), and a boss's
/// payout is §3's to set.
pub fn elite_loot_mult(msg: &KillMsg, run: &RunState) -> f32 {
    if msg.elite && !msg.is_boss && !msg.is_miniboss {
        run.elite_loot.max(1.0)
    } else {
        1.0
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
