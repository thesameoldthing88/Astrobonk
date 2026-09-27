//! GDD §9 "New enemies (behaviour-first)", batch 1 (P08): Rollo, the Trencher, the Aegis
//! Drone, the Sunskimmer, the Beacon Tick, the Mimic Chest and the Longshot Beamer Prime.
//!
//! Split like the rest of the horde. The SIMULATION systems here are host-only
//! (`net::is_simulating`) and act for every astronaut. The PRESENTATION systems run on
//! every machine from two inputs: `EnemyVis` — the little each new kind shows that a client
//! cannot derive from positions (under the crust, dive height, shield facing, sniper aim),
//! which crosses on the enemy-state lane (`netenemy::stream_enemy_states`) — and
//! `BestiaryFxMsg`, the one-shots (an uppercut, a tracker, a sprung mimic) that ride the
//! hazard lane. Everything that hurts is the host's.
//!
//! Crowd rules hold: one mesh and one material per kind (`enemies::enemy_mesh`, the accent
//! through an emissive mask), whole-transform animation, no per-enemy children. The few
//! extra visuals — ridge mounds, skimmer shadows, tracker beacons — are loose entities that
//! share one mesh and material each, so they batch too, and are counted by their caps.

use crate::config::*;
use crate::content::enemies::EnemyKind;
use crate::enemies::{self, AimLine, Beamer, Buried, Enemy, EnemyAssets, EnemyProjectile, Telegraph};
use crate::events_world::InStorm;
use crate::fx::{self, ParticleAssets, Pcolor, Shake};
use crate::messages::*;
use crate::planet::{CurrentPlanet, PropColliders, StageScoped};
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::run::scaling::Scaling;
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use rand::Rng;
use std::collections::{HashMap, HashSet};

// ─── shared state ───────────────────────────────────────────────────────────

/// What a new kind SHOWS that positions alone don't say. The host's behaviour systems write
/// it every frame; a client receives it on the enemy-state lane; the pose reads it on both.
///
/// `state` per kind — Rollo: `ROLLING`/`STUNNED`; Trencher: `SURFACED`..`ERUPT`; Sunskimmer:
/// `CRUISE`/`DIVE`; Beamer Prime: `IDLE`/`CHARGING`/`LOCKED`; Mimic: `FLEEING`/`DIGGING`.
/// `alt` is the Sunskimmer's height over the ground; `heading` the Aegis Drone's shield
/// facing or the Prime's aim (world-space tangent).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct EnemyVis {
    pub state: u8,
    pub alt: f32,
    pub heading: Vec3,
}

impl EnemyVis {
    pub const ROLLING: u8 = 0;
    pub const STUNNED: u8 = 1;
    pub const SURFACED: u8 = 0;
    pub const SINKING: u8 = 1;
    pub const TUNNEL: u8 = 2;
    pub const WINDUP: u8 = 3;
    pub const ERUPT: u8 = 4;
    pub const CRUISE: u8 = 0;
    pub const DIVE: u8 = 1;
    pub const IDLE: u8 = 0;
    pub const CHARGING: u8 = 1;
    pub const LOCKED: u8 = 2;
    pub const FLEEING: u8 = 0;
    pub const DIGGING: u8 = 1;
}

/// Presentation memory for a new-kind enemy's pose, on every machine: the heading it is
/// seen moving along (derived from motion, so host and client pose alike), how far it has
/// rolled, how long it has been in its current look, where it last raised a ridge mound.
#[derive(Component, Clone, Copy, Debug)]
pub struct PoseMemo {
    pub prev: Vec3,
    pub heading: Vec3,
    /// The drawn shield/aim facing, eased toward `EnemyVis.heading` (a 10 Hz lane on a
    /// client would otherwise step).
    pub facing: Vec3,
    pub spin: f32,
    pub state: u8,
    pub age: f32,
    pub mound_at: Vec3,
}

impl PoseMemo {
    pub fn new(dir: Vec3) -> Self {
        let t = sphere::tangent_frame(dir).0;
        Self { prev: dir, heading: t, facing: Vec3::ZERO, spin: 0.0, state: 0, age: 0.0, mound_at: dir }
    }
}

/// The kinds that carry `EnemyVis` + `PoseMemo` (the Beacon Tick walks like the crowd).
pub fn has_vis(kind: EnemyKind) -> bool {
    use EnemyKind::*;
    matches!(kind, Rollo | Trencher | AegisDrone | Sunskimmer | Mimic | BeamerPrime)
}

/// The look components a new-kind enemy is built with, host or client proxy alike.
pub fn vis_bundle(dir: Vec3) -> (EnemyVis, PoseMemo) {
    (EnemyVis::default(), PoseMemo::new(dir))
}

/// Steers itself: `enemies::enemy_move` leaves it alone.
#[derive(Component)]
pub struct SelfSteered;

/// Rollo's roll: a heading that turns slowly, a speed the slope feeds, a stun after a bonk.
#[derive(Component)]
pub struct Rollo {
    pub heading: Vec3,
    pub speed: f32,
    pub stun: f32,
    /// Its scaled contact damage at cruise (the bite grows and shrinks with its speed).
    pub bite: f32,
}

/// The Trencher's cycle (`EnemyVis::SURFACED`..`ERUPT`).
#[derive(Component)]
pub struct Trencher {
    pub phase: u8,
    pub timer: f32,
    pub cd: f32,
    pub target: Option<Entity>,
}

/// The Sunskimmer: cruising high, or diving from `from` to `to`.
#[derive(Component)]
pub struct Skimmer {
    pub diving: bool,
    pub t: f32,
    pub from: Vec3,
    pub to: Vec3,
    pub alt: f32,
}

/// The Aegis Drone's shield: where it faces (a world tangent), turned at AEGIS_TURN_RATE.
#[derive(Component)]
pub struct AegisShield {
    pub facing: Vec3,
    pub block_fx: f32,
}

/// Marks the Longshot Beamer Prime. It wears a `Beamer` for the shared aim-line visuals;
/// `prime_attack` aims it (with a lead) instead of `enemies::beamer_attack`.
#[derive(Component)]
pub struct PrimeSight;

/// A sprung Mimic: who paid the chest price it swallowed (their purse gets it back if it
/// dies), and how long it has left to escape.
#[derive(Component)]
pub struct Mimic {
    pub payer: Option<Entity>,
    pub paid: u64,
    pub left: f32,
}

/// A stage chest that is secretly a Mimic (rolled with the layout, the same on every
/// machine). `tell` counts down to its next breath.
#[derive(Component)]
pub struct MimicDisguise {
    pub tell: f32,
    pub breath: f32,
}

/// Set on a disguised chest by `interact::interact_system` when someone tries its lid and
/// pays: `mimic_spring` turns it into the monster.
#[derive(Component)]
pub struct MimicSprung {
    pub payer: Entity,
    pub paid: u64,
}

/// A railbolt flying a line of fire over the curve: radius eases from the muzzle's `r0` to
/// the aim point's `r1` over `span` metres of arc, then holds.
#[derive(Component, Clone, Copy, Debug)]
pub struct CurveShot {
    pub r0: f32,
    pub r1: f32,
    pub span: f32,
    pub flown: f32,
}

impl CurveShot {
    pub fn radius(&self) -> f32 {
        let f = if self.span > 1e-3 { (self.flown / self.span).clamp(0.0, 1.0) } else { 1.0 };
        self.r0 + (self.r1 - self.r0) * f
    }
}

/// A telegraph that belongs to one enemy (a Trencher's uppercut spot, a Sunskimmer's
/// landing): it dies with its owner instead of standing there as a lie.
#[derive(Component)]
pub struct TelegraphOwner(pub Entity);

/// On an astronaut a Beacon Tick touched: the horde knows where they are for `secs`.
#[derive(Component)]
pub struct Tracked {
    pub secs: f32,
}

// ─── presentation-only entities ─────────────────────────────────────────────

/// One mound of a Trencher's ridge.
#[derive(Component)]
pub struct RidgeMound {
    pub age: f32,
    pub dir: Vec3,
}

/// The shadow under a Sunskimmer — where it is, and where to shoot it.
#[derive(Component)]
pub struct SkimShadow {
    pub owner: Entity,
}

/// The red beacon over a tracked astronaut (by PlayerId, so it finds them on any machine).
#[derive(Component)]
pub struct TrackerBeacon {
    pub pid: u8,
    pub left: f32,
}

#[derive(Resource)]
pub struct BestiaryAssets {
    pub mound_mesh: Handle<Mesh>,
    pub mound_mat: Handle<StandardMaterial>,
    pub shadow_mesh: Handle<Mesh>,
    pub shadow_mat: Handle<StandardMaterial>,
    pub beacon_mesh: Handle<Mesh>,
    pub beacon_mat: Handle<StandardMaterial>,
}

pub fn setup_bestiary_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    use crate::meshkit::{at, MeshData};
    // a lumpy dirt mound, flat side down
    let mut m = MeshData::new();
    m.add_ellipsoid(Vec3::new(0.55, 0.3, 0.45), 1, at(Vec3::ZERO), Color::WHITE);
    m.add_sphere(0.22, 0, at(Vec3::new(0.22, 0.12, 0.1)), Color::srgb(0.8, 0.8, 0.8));
    m.add_sphere(0.18, 0, at(Vec3::new(-0.2, 0.1, -0.12)), Color::srgb(0.8, 0.8, 0.8));
    let mound_mesh = meshes.add(m.build());
    // the beacon: a bulb and a thin column of light above it
    let mut b = MeshData::new();
    b.add_sphere(0.16, 1, at(Vec3::ZERO), Color::WHITE);
    b.add_cylinder(0.05, 6.0, 6, at(Vec3::new(0.0, 3.1, 0.0)), Color::WHITE);
    let beacon_mesh = meshes.add(b.build());
    commands.insert_resource(BestiaryAssets {
        mound_mesh,
        mound_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.42, 0.34, 0.27),
            perceptual_roughness: 0.95,
            ..default()
        }),
        shadow_mesh: meshes.add(Mesh::from(Cylinder::new(1.0, 0.01))),
        shadow_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.45),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: 10.0,
            ..default()
        }),
        beacon_mesh,
        beacon_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.15, 0.1, 0.8),
            emissive: LinearRgba::rgb(6.0, 0.6, 0.4),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

/// Host-side telemetry: what the new kinds actually did (the headless `--bestiary` probe
/// asserts on it; nothing in the game reads it).
#[derive(Resource, Default, Debug)]
pub struct BestiaryTelemetry {
    pub rollo_bonks: u32,
    pub rollo_max_speed: f32,
    /// Largest heading change Rollo made in one second (it must stay under its turn rate).
    pub rollo_max_turn: f32,
    pub trench_dives: u32,
    pub trench_uppercuts: u32,
    pub trench_launched: u32,
    pub trench_airborne_spared: u32,
    pub aegis_blocks: u32,
    pub aegis_open_hits: u32,
    pub skim_dives: u32,
    pub skim_blasts: u32,
    pub skim_blast_hits: u32,
    pub ticks_latched: u32,
    pub mimic_springs: u32,
    pub mimic_refunds: u64,
    pub mimic_escapes: u32,
    pub prime_shots: u32,
    pub prime_bolts_shadowed: u32,
    /// Largest angle (rad) between the Prime's aim and the straight line to its mark.
    pub prime_max_lead: f32,
    pub rings_reaped: u32,
}

/// The one-shots of the new kinds, said locally on the host and rebuilt from the hazard
/// lane on a client (`from_wire`), so `bestiary_fx_presentation` draws them the same.
#[derive(Clone, Copy, Debug)]
pub enum BestiaryFx {
    /// A Trencher erupted at `dir`; bit `i` of `launched` = PlayerId `i` was thrown up.
    Uppercut { dir: Vec3, launched: u8 },
    /// A Beacon Tick tagged astronaut `owner` (PlayerId) for `secs`.
    Tracked { owner: u8, secs: f32 },
    /// The disguised chest at `dir` was a Mimic.
    MimicSprung { dir: Vec3 },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct BestiaryFxMsg {
    pub fx: BestiaryFx,
    pub from_wire: bool,
}

/// A killed Mimic gives back what it swallowed — to the purse that paid, never to whoever
/// landed the last hit. Written by `combat::apply_hits`, paid by `pay_refunds`.
#[derive(Message, Clone, Copy, Debug)]
pub struct RefundMsg {
    pub payer: Entity,
    pub gold: u64,
    pub pos: Vec3,
}

// ─── spawning ───────────────────────────────────────────────────────────────

/// Give a freshly spawned enemy of `kind` what its behaviour needs (called by
/// `enemies::spawn_enemy`, so every spawn path gets it).
pub fn attach(cmd: &mut EntityCommands, kind: EnemyKind, dir: Vec3, rng: &mut impl Rng) {
    use EnemyKind::*;
    let heading = {
        let (t, b) = sphere::tangent_frame(dir);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        t * a.cos() + b * a.sin()
    };
    match kind {
        Rollo => {
            cmd.insert((self::Rollo { heading, speed: ROLLO_MIN_SPEED, stun: 0.0, bite: 0.0 }, SelfSteered, vis_bundle(dir)));
        }
        Trencher => {
            cmd.insert((
                self::Trencher { phase: EnemyVis::SURFACED, timer: 0.0, cd: rng.gen_range(1.0..3.0), target: None },
                vis_bundle(dir),
            ));
        }
        AegisDrone => {
            cmd.insert((AegisShield { facing: heading, block_fx: 0.0 }, vis_bundle(dir)));
        }
        Sunskimmer => {
            cmd.insert((Skimmer { diving: false, t: 0.0, from: dir, to: dir, alt: SKIM_ALTITUDE }, SelfSteered, vis_bundle(dir)));
        }
        Mimic => {
            cmd.insert((self::Mimic { payer: None, paid: 0, left: MIMIC_FLEE_SECS }, SelfSteered, vis_bundle(dir)));
        }
        BeamerPrime => {
            cmd.insert((
                enemies::Beamer { cd: rng.gen_range(2.0..4.0), charging: 0.0, aim: Vec3::ZERO, target: None },
                PrimeSight,
                vis_bundle(dir),
            ));
        }
        _ => {}
    }
}

// ─── helpers ────────────────────────────────────────────────────────────────

/// `v` flattened onto the tangent plane at `up`, unit (zero if it was radial).
fn tangent(v: Vec3, up: Vec3) -> Vec3 {
    (v - up * v.dot(up)).normalize_or_zero()
}

/// Turn tangent `heading` toward `want` about `up` by at most `max` radians.
pub fn turn_toward(heading: Vec3, want: Vec3, up: Vec3, max: f32) -> Vec3 {
    let h = tangent(heading, up);
    let w = tangent(want, up);
    if h == Vec3::ZERO {
        return w;
    }
    if w == Vec3::ZERO {
        return h;
    }
    let ang = h.angle_between(w);
    if ang <= max {
        return w;
    }
    // rotating h about +up moves it along up × h
    let axis = if up.cross(h).dot(w) >= 0.0 { up } else { -up };
    (Quat::from_axis_angle(axis, max) * h).normalize()
}

/// An astronaut as the new kinds see one: where, how high, how fast, whether it is theirs
/// to hunt.
#[derive(Clone, Copy)]
struct Mark {
    entity: Entity,
    pid: u8,
    dir: Vec3,
    pos: Vec3,
    vel: Vec3,
    height: f32,
}

fn nearest_mark(from: Vec3, marks: &[Mark], radius: f32) -> Option<Mark> {
    marks.iter().copied().min_by(|a, b| sphere::arc_dist(from, a.dir, radius).total_cmp(&sphere::arc_dist(from, b.dir, radius)))
}

/// A telegraph owned by `owner` (it dies with it).
fn owned_telegraph(commands: &mut Commands, assets: &EnemyAssets, planet: &CurrentPlanet, owner: Entity, dir: Vec3, radius: f32, secs: f32) {
    commands.spawn((
        enemies::telegraph_bundle(assets, planet, Telegraph { timer: secs, max: secs, radius, damage: 0.0, dir, ring: false }),
        TelegraphOwner(owner),
    ));
}

/// The small outward bloom a telegraph of radius 0 draws (the Burrower's eruption pop).
fn pop(commands: &mut Commands, assets: &EnemyAssets, planet: &CurrentPlanet, dir: Vec3) {
    commands.spawn(enemies::telegraph_bundle(
        assets,
        planet,
        Telegraph { timer: 0.25, max: 0.25, radius: 0.0, damage: 0.0, dir, ring: false },
    ));
}

// ─── simulation (host) ──────────────────────────────────────────────────────

/// Rollo rolls the great circle toward the nearest astronaut: it eases to cruise on the
/// flat, runs away with itself downhill, bogs down uphill, turns wide — and a crater wall
/// or a rock taken at speed stuns it and hurts it. It bowls THROUGH you (its bite scales
/// with its speed) and has to come round again.
#[allow(clippy::too_many_arguments)]
pub fn rollo_roll(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    props: Res<PropColliders>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<(Entity, &Player, &PlayerState), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &mut self::Rollo, &mut EnemyVis), Without<Buried>>,
    mut hits: MessageWriter<HitMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let marks: Vec<Vec3> = q_player.iter().filter(|(_, _, ps)| !ps.dead).map(|(_, p, _)| p.dir).collect();
    for (entity, mut e, mut r, mut vis) in &mut q {
        enemies::tick_enemy(&mut e, dt);
        if r.bite <= 0.0 {
            r.bite = e.damage.max(0.01);
        }
        let up = e.dir;
        if r.stun > 0.0 {
            r.stun -= dt;
            // dizzy, uncurled: harmless until it rolls again
            e.contact_cd = e.contact_cd.max(0.3);
            vis.state = EnemyVis::STUNNED;
            vis.heading = r.heading;
            continue;
        }
        let target = marks
            .iter()
            .copied()
            .min_by(|a, b| sphere::arc_dist(up, *a, planet.radius).total_cmp(&sphere::arc_dist(up, *b, planet.radius)));
        let before = r.heading;
        if let Some(t) = target {
            r.heading = turn_toward(r.heading, t - up * t.dot(up), up, ROLLO_TURN_RATE * dt);
        }
        r.heading = tangent(r.heading, up);
        if r.heading == Vec3::ZERO {
            r.heading = sphere::tangent_frame(up).0;
        }
        telemetry.rollo_max_turn = telemetry.rollo_max_turn.max(before.angle_between(r.heading) / dt);
        // downhill feeds it, uphill starves it
        let grade = planet.terrain.slope(up, planet.radius).dot(r.heading);
        r.speed = (r.speed + ((ROLLO_CRUISE_SPEED - r.speed) * ROLLO_EASE - grade * ROLLO_SLOPE_ACCEL) * dt)
            .clamp(ROLLO_MIN_SPEED, ROLLO_MAX_SPEED);
        telemetry.rollo_max_speed = telemetry.rollo_max_speed.max(r.speed);
        let rad = planet.surface(up);
        let (next, vel) = sphere::advance(up, r.heading * r.speed * (1.0 - e.slow), rad, dt);
        let rock = props.resolve(next, 0.0, e.scale * 0.5, planet.radius);
        let wall = grade > ROLLO_WALL_GRADE;
        if (wall || rock.is_some()) && r.speed > ROLLO_BONK_SPEED {
            // BONK — baited into a crater wall or a boulder
            r.stun = ROLLO_STUN_SECS;
            r.speed = ROLLO_MIN_SPEED;
            r.heading = -r.heading;
            telemetry.rollo_bonks += 1;
            hits.write(HitMsg { source: None, target: entity, amount: e.max_hp * ROLLO_BONK_SELF, crit: false, knock: Vec3::ZERO });
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, planet.surface_point(up) + up * e.scale * 0.5, up, Pcolor::White, 10, 5.0);
            }
            vis.state = EnemyVis::STUNNED;
            vis.heading = r.heading;
            continue;
        }
        let mut dir = rock.unwrap_or(next);
        let fresh = tangent(vel, dir);
        if fresh != Vec3::ZERO {
            r.heading = fresh;
        }
        if e.knock.length_squared() > 0.001 {
            dir = sphere::advance(dir, e.knock, rad, dt).0;
        }
        e.dir = dir;
        e.damage = r.bite * (0.5 + 0.5 * r.speed / ROLLO_CRUISE_SPEED);
        vis.state = EnemyVis::ROLLING;
        vis.heading = r.heading;
    }
}

/// The Trencher's cycle: walk up (the ordinary steering), dive once its mark is close,
/// tunnel after them under a visible ridge, mark the spot, and uppercut everyone standing
/// on the ground there into the air. Anyone airborne at the eruption is spared.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn trencher_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    particles: Option<Res<ParticleAssets>>,
    mut q_astro: Query<(Entity, &mut Player, &PlayerState, &PlayerId, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &mut self::Trencher, &mut EnemyVis), Without<Player>>,
    mut hits: MessageWriter<PlayerHitMsg>,
    mut fx_out: MessageWriter<BestiaryFxMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let marks: Vec<Mark> = q_astro
        .iter()
        .filter(|(_, _, ps, ..)| !ps.dead)
        .map(|(en, p, _, pid, tf)| Mark { entity: en, pid: pid.0, dir: p.dir, pos: tf.translation, vel: p.vel_t, height: p.height })
        .collect();
    let mut launches: Vec<Entity> = Vec::new();
    for (entity, mut e, mut t, mut vis) in &mut q {
        match t.phase {
            EnemyVis::SINKING => {
                t.timer -= dt;
                if t.timer <= 0.0 {
                    t.phase = EnemyVis::TUNNEL;
                    t.timer = TRENCH_TUNNEL_SECS;
                }
            }
            EnemyVis::TUNNEL => {
                enemies::tick_enemy(&mut e, dt);
                // stay on the mark it dived for; a downed or departed one hands it the nearest
                let mark = t
                    .target
                    .and_then(|m| marks.iter().copied().find(|k| k.entity == m))
                    .or_else(|| nearest_mark(e.dir, &marks, planet.radius));
                t.timer -= dt;
                let close = match mark {
                    Some(m) => {
                        t.target = Some(m.entity);
                        let step = TRENCH_TUNNEL_SPEED * (1.0 - e.slow) * dt / planet.surface(e.dir);
                        e.dir = sphere::step_toward(e.dir, m.dir, step);
                        sphere::arc_dist(e.dir, m.dir, planet.radius) < 1.0
                    }
                    None => false,
                };
                if close || t.timer <= 0.0 {
                    t.phase = EnemyVis::WINDUP;
                    t.timer = TRENCH_WINDUP_SECS;
                    owned_telegraph(&mut commands, &assets, &planet, entity, e.dir, TRENCH_UPPERCUT_RADIUS, TRENCH_WINDUP_SECS);
                }
            }
            EnemyVis::WINDUP => {
                t.timer -= dt;
                if t.timer <= 0.0 {
                    // THE UPPERCUT: it breaks the crust under whoever stayed on the ground
                    commands.entity(entity).remove::<Buried>();
                    let mut launched = 0u8;
                    for m in &marks {
                        if sphere::arc_dist(e.dir, m.dir, planet.radius) > TRENCH_UPPERCUT_RADIUS {
                            continue;
                        }
                        if m.height > TRENCH_AIRBORNE_HEIGHT {
                            telemetry.trench_airborne_spared += 1;
                            continue;
                        }
                        hits.write(PlayerHitMsg {
                            victim: m.entity,
                            amount: e.damage * TRENCH_DAMAGE_MULT,
                            from: planet.surface_point(e.dir),
                            attacker: Some(entity),
                        });
                        launches.push(m.entity);
                        launched |= 1 << m.pid.min(7);
                        telemetry.trench_launched += 1;
                    }
                    telemetry.trench_uppercuts += 1;
                    fx_out.write(BestiaryFxMsg { fx: BestiaryFx::Uppercut { dir: e.dir, launched }, from_wire: false });
                    if let Some(pa) = &particles {
                        fx::burst(&mut commands, pa, planet.surface_point(e.dir), e.dir, Pcolor::White, 16, 8.0);
                    }
                    e.contact_cd = CONTACT_TICK; // the uppercut was its bite
                    t.phase = EnemyVis::ERUPT;
                    t.timer = TRENCH_ERUPT_SECS;
                }
            }
            EnemyVis::ERUPT => {
                t.timer -= dt;
                if t.timer <= 0.0 {
                    t.phase = EnemyVis::SURFACED;
                    t.cd = TRENCH_COOLDOWN;
                    t.target = None;
                }
            }
            _ => {
                // surfaced: `enemy_move` walks it in; dive once a mark is in reach
                t.cd -= dt;
                if t.cd <= 0.0 {
                    if let Some(m) = nearest_mark(e.dir, &marks, planet.radius) {
                        let arc = sphere::arc_dist(e.dir, m.dir, planet.radius);
                        if (TRENCH_MIN_ARC..=TRENCH_DIVE_ARC).contains(&arc) {
                            t.phase = EnemyVis::SINKING;
                            t.timer = TRENCH_SINK_SECS;
                            t.target = Some(m.entity);
                            // `Buried`: every weapon, the steering and the contact bite skip it
                            commands.entity(entity).insert(Buried { timer: f32::INFINITY });
                            telemetry.trench_dives += 1;
                        }
                    }
                }
            }
        }
        vis.state = t.phase;
    }
    // the launch, on the host's copy of every astronaut it caught (a joiner's own
    // predicted body takes it from the Uppercut event)
    for victim in launches {
        if let Ok((_, mut p, ..)) = q_astro.get_mut(victim) {
            p.vel_r = p.vel_r.max(TRENCH_LAUNCH_VEL);
            p.grounded = false;
            p.height = p.height.max(0.05);
        }
    }
}

/// The Sunskimmer cruises in high and fast toward the nearest astronaut; within its dive
/// arc it commits — marks the landing (a little ahead of where its mark is running) and
/// dives onto it, exploding on impact. Kill it first and the mark dies with it.
#[allow(clippy::too_many_arguments)]
pub fn skimmer_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    particles: Option<Res<ParticleAssets>>,
    mut shake: ResMut<Shake>,
    q_astro: Query<(Entity, &Player, &PlayerState, &PlayerId, &Transform), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &mut Skimmer, &mut EnemyVis)>,
    mut hits: MessageWriter<PlayerHitMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t_now = time.elapsed_secs();
    let marks: Vec<Mark> = q_astro
        .iter()
        .filter(|(_, _, ps, ..)| !ps.dead)
        .map(|(en, p, _, pid, tf)| Mark { entity: en, pid: pid.0, dir: p.dir, pos: tf.translation, vel: p.vel_t, height: p.height })
        .collect();
    for (entity, mut e, mut s, mut vis) in &mut q {
        enemies::tick_enemy(&mut e, dt);
        // no contact bite: its blast is the attack
        e.contact_cd = e.contact_cd.max(0.5);
        let up = e.dir;
        if !s.diving {
            let Some(m) = nearest_mark(up, &marks, planet.radius) else { continue };
            let step = e.speed * (1.0 - e.slow) * dt / (planet.surface(up) + s.alt);
            let mut dir = sphere::step_toward(up, m.dir, step);
            if e.knock.length_squared() > 0.001 {
                dir = sphere::advance(dir, e.knock, planet.surface(dir), dt).0;
            }
            e.dir = dir;
            s.alt = SKIM_ALTITUDE + (t_now * 1.7 + e.wobble).sin() * 0.6;
            vis.heading = tangent(m.dir - dir, dir);
            if sphere::arc_dist(dir, m.dir, planet.radius) <= SKIM_DIVE_ARC {
                // commit: lead the mark by half the dive
                let speed = m.vel.length();
                let to = if speed > 0.5 {
                    sphere::offset_dir(m.dir, m.vel / speed, speed * SKIM_DIVE_SECS * 0.5, planet.radius)
                } else {
                    m.dir
                };
                s.diving = true;
                s.t = 0.0;
                s.from = dir;
                s.to = to;
                owned_telegraph(&mut commands, &assets, &planet, entity, to, SKIM_BLAST_RADIUS, SKIM_DIVE_SECS);
                telemetry.skim_dives += 1;
            }
        } else {
            s.t += dt;
            let f = (s.t / SKIM_DIVE_SECS).min(1.0);
            // over the mark first, then down on it: it covers the ground early and drops late,
            // so the last stretch is a plunge from above — not a skim along the ground through
            // the camera behind its mark
            e.dir = s.from.slerp(s.to, 1.0 - (1.0 - f) * (1.0 - f)).normalize();
            s.alt = SKIM_ALTITUDE * (1.0 - f * f);
            vis.heading = tangent(s.to - s.from, e.dir);
            if f >= 1.0 {
                for m in &marks {
                    if sphere::arc_dist(s.to, m.dir, planet.radius) <= SKIM_BLAST_RADIUS && m.height < SKIM_BLAST_CLEAR_HEIGHT {
                        hits.write(PlayerHitMsg { victim: m.entity, amount: e.damage, from: planet.surface_point(s.to), attacker: Some(entity) });
                        telemetry.skim_blast_hits += 1;
                    }
                }
                pop(&mut commands, &assets, &planet, s.to);
                if let Some(pa) = &particles {
                    fx::burst(&mut commands, pa, planet.surface_point(s.to), s.to, Pcolor::Danger, 18, 9.0);
                }
                shake.add(0.12);
                telemetry.skim_blasts += 1;
                commands.entity(entity).despawn();
                continue;
            }
        }
        e.hover = s.alt;
        vis.state = if s.diving { EnemyVis::DIVE } else { EnemyVis::CRUISE };
        vis.alt = s.alt;
    }
}

/// The Aegis Drone swings its shield toward the nearest astronaut — slower than you can
/// run around it. The block itself is in `combat::apply_hits` (`shield_blocks`).
pub fn aegis_turn(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(&Player, &PlayerState), Without<Enemy>>,
    mut q: Query<(&Enemy, &mut AegisShield, &mut EnemyVis)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let marks: Vec<Vec3> = q_player.iter().filter(|(_, ps)| !ps.dead).map(|(p, _)| p.dir).collect();
    for (e, mut sh, mut vis) in &mut q {
        let up = e.dir;
        sh.block_fx = (sh.block_fx - dt).max(0.0);
        let mut facing = tangent(sh.facing, up);
        if facing == Vec3::ZERO {
            facing = sphere::tangent_frame(up).0;
        }
        if let Some(t) = marks
            .iter()
            .copied()
            .min_by(|a, b| sphere::arc_dist(up, *a, planet.radius).total_cmp(&sphere::arc_dist(up, *b, planet.radius)))
        {
            facing = turn_toward(facing, t - up * t.dot(up), up, AEGIS_TURN_RATE * dt);
        }
        sh.facing = facing;
        vis.heading = facing;
    }
}

/// Does an Aegis Drone at `at` facing `facing` block a hit? The hit's line comes from its
/// knockback when it has one (a shot's travel), else from where its shooter stands (an
/// aura, a beam). Environment hits (no shooter, no push) always land.
pub fn shield_blocks(at: Vec3, facing: Vec3, knock: Vec3, shooter: Option<Vec3>) -> bool {
    let from = if knock.length_squared() > 1e-6 {
        tangent(-knock, at)
    } else if let Some(s) = shooter {
        tangent(s - at, at)
    } else {
        return false;
    };
    from != Vec3::ZERO && from.dot(tangent(facing, at)) > AEGIS_SHIELD_HALF.cos()
}

/// A Beacon Tick that reaches an astronaut plants itself: the astronaut is tracked (the
/// horde paths to them, `enemies::enemy_move`) and the tick is spent. No damage — the
/// tracker is the attack.
#[allow(clippy::too_many_arguments)]
pub fn tick_latch(
    mut commands: Commands,
    particles: Option<Res<ParticleAssets>>,
    q_ticks: Query<(Entity, &Enemy, &Transform), Without<Buried>>,
    mut q_astro: Query<(Entity, &PlayerState, &PlayerId, &Transform, Option<&mut Tracked>), Without<Enemy>>,
    mut fx_out: MessageWriter<BestiaryFxMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let bodies: Vec<(Entity, u8, Vec3)> = q_astro
        .iter()
        .filter(|(_, ps, ..)| !ps.dead)
        .map(|(e, _, pid, tf, _)| (e, pid.0, tf.translation))
        .collect();
    if bodies.is_empty() {
        return;
    }
    let mut tagged: Vec<(Entity, u8)> = Vec::new();
    for (te, e, tf) in &q_ticks {
        if e.kind != EnemyKind::BeaconTick || e.hp <= 0.0 {
            continue;
        }
        let reach = e.scale * 0.55 + PLAYER_RADIUS + 0.25;
        let Some(&(victim, pid, _)) = bodies
            .iter()
            .filter(|(_, _, p)| p.distance_squared(tf.translation) < reach * reach)
            .min_by(|a, b| a.2.distance_squared(tf.translation).total_cmp(&b.2.distance_squared(tf.translation)))
        else {
            continue;
        };
        tagged.push((victim, pid));
        commands.entity(te).despawn();
        if let Some(pa) = &particles {
            fx::burst(&mut commands, pa, tf.translation, e.dir, Pcolor::Red, 8, 4.0);
        }
        telemetry.ticks_latched += 1;
    }
    for (victim, pid) in tagged {
        let Ok((.., tracked)) = q_astro.get_mut(victim) else { continue };
        match tracked {
            Some(mut t) => t.secs = TRACKER_SECS,
            None => {
                commands.entity(victim).insert(Tracked { secs: TRACKER_SECS });
            }
        }
        fx_out.write(BestiaryFxMsg { fx: BestiaryFx::Tracked { owner: pid, secs: TRACKER_SECS }, from_wire: false });
        sfx.write(SfxMsg(Sfx::Tag));
    }
}

/// Trackers run out.
pub fn tracker_upkeep(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Tracked)>) {
    let dt = time.delta_secs();
    for (e, mut t) in &mut q {
        t.secs -= dt;
        if t.secs <= 0.0 {
            commands.entity(e).remove::<Tracked>();
        }
    }
}

/// Someone tried the lid of a disguised chest: it becomes a Mimic where it stood, lets off
/// its 360° shockwave (already in the air — killing it doesn't stop it) and runs.
#[allow(clippy::too_many_arguments)]
pub fn mimic_spring(
    mut commands: Commands,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    run: Res<RunState>,
    particles: Option<Res<ParticleAssets>>,
    q: Query<(Entity, &MimicSprung, &Transform)>,
    q_party: Query<(), With<Player>>,
    mut fx_out: MessageWriter<BestiaryFxMsg>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let mut rng = rand::thread_rng();
    for (chest, sprung, tf) in &q {
        let dir = tf.translation.normalize_or_zero();
        commands.entity(chest).despawn();
        let sc = Scaling::for_run(&run, q_party.iter().count());
        let m = enemies::spawn_enemy_at(&mut commands, &assets, &planet, EnemyKind::Mimic, dir, false, &sc, &mut rng);
        commands.entity(m).insert(Mimic { payer: Some(sprung.payer), paid: sprung.paid, left: MIMIC_FLEE_SECS });
        let damage = EnemyKind::Mimic.def().damage * sc.dmg * MIMIC_SHOCK_DAMAGE_MULT;
        commands.spawn(enemies::telegraph_bundle(
            &assets,
            &planet,
            Telegraph { timer: MIMIC_SHOCK_SECS, max: MIMIC_SHOCK_SECS, radius: MIMIC_SHOCK_RADIUS, damage, dir, ring: false },
        ));
        if let Some(pa) = &particles {
            fx::burst(&mut commands, pa, tf.translation, dir, Pcolor::Gold, 16, 7.0);
        }
        fx_out.write(BestiaryFxMsg { fx: BestiaryFx::MimicSprung { dir }, from_wire: false });
        banners.write(BannerMsg(format!("IT'S A MIMIC! IT ATE {} GOLD — CATCH IT!", sprung.paid)));
        sfx.write(SfxMsg(Sfx::Chomp));
        telemetry.mimic_springs += 1;
    }
}

/// A sprung Mimic runs from the nearest astronaut, zig-zagging, and digs out when its time
/// is up — with whatever it swallowed.
#[allow(clippy::too_many_arguments)]
pub fn mimic_flee(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    q_player: Query<(&Player, &PlayerState), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &mut Mimic, &mut EnemyVis)>,
    mut banners: MessageWriter<BannerMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t_now = time.elapsed_secs();
    let marks: Vec<Vec3> = q_player.iter().filter(|(_, ps)| !ps.dead).map(|(p, _)| p.dir).collect();
    for (entity, mut e, mut m, mut vis) in &mut q {
        enemies::tick_enemy(&mut e, dt);
        m.left -= dt;
        let up = e.dir;
        if m.left <= 0.0 {
            pop(&mut commands, &assets, &planet, up);
            if m.paid > 0 {
                banners.write(BannerMsg(format!("THE MIMIC DUG OUT WITH {} GOLD", m.paid)));
            }
            telemetry.mimic_escapes += 1;
            commands.entity(entity).despawn();
            continue;
        }
        let away = marks
            .iter()
            .copied()
            .min_by(|a, b| sphere::arc_dist(up, *a, planet.radius).total_cmp(&sphere::arc_dist(up, *b, planet.radius)))
            .map(|t| tangent(up - t, up))
            .filter(|v| *v != Vec3::ZERO)
            .unwrap_or(vis.heading);
        // a panicked zig-zag, not a straight line to shoot down
        let zig = Quat::from_axis_angle(up, (t_now * 2.3 + e.wobble).sin() * 0.55) * away;
        let rad = planet.surface(up);
        let (mut dir, _) = sphere::advance(up, zig * e.speed * (1.0 - e.slow), rad, dt);
        if e.knock.length_squared() > 0.001 {
            dir = sphere::advance(dir, e.knock, rad, dt).0;
        }
        e.dir = dir;
        vis.heading = tangent(zig, dir);
        vis.state = if m.left < 1.0 { EnemyVis::DIGGING } else { EnemyVis::FLEEING };
    }
}

/// The Longshot Beamer Prime: from well back — over the horizon — it paints the nearest
/// astronaut it can see, LEADING them by its railbolt's flight, locks, and fires a bolt
/// along a line of fire over the curve (`CurveShot`). A hill between you is the only cover.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn prime_attack(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(Entity, &Player, &PlayerState, &Transform, Has<InStorm>), Without<Enemy>>,
    mut q: Query<(Entity, &Enemy, &mut Beamer, &Transform, &mut EnemyVis), (With<PrimeSight>, Without<Buried>)>,
    q_lines: Query<(Entity, &AimLine)>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // (mark, hidden in the dust storm) — a Signal Flare carrier is never hidden
    let all: Vec<(Mark, bool)> = q_player
        .iter()
        .filter(|(_, _, ps, ..)| !ps.dead)
        .map(|(en, p, ps, tf, hidden)| {
            (Mark { entity: en, pid: 0, dir: p.dir, pos: tf.translation, vel: p.vel_t, height: p.height }, hidden && !ps.revealed())
        })
        .collect();
    let visible: Vec<Mark> = all.iter().filter(|(_, h)| !h).map(|(m, _)| *m).collect();
    let drop_line = |commands: &mut Commands, owner: Entity| {
        for (le, line) in q_lines.iter() {
            if line.owner == owner {
                commands.entity(le).try_despawn();
            }
        }
    };
    for (entity, e, mut b, tf, mut vis) in &mut q {
        let latched = b.target.filter(|_| b.charging > 0.0).and_then(|t| all.iter().copied().find(|(m, _)| m.entity == t));
        if let Some((_, true)) = latched {
            // the mark ducked into the dust: the telegraph was for them, hold fire
            b.charging = 0.0;
            b.target = None;
            drop_line(&mut commands, entity);
            vis.state = EnemyVis::IDLE;
            continue;
        }
        let Some(mark) = latched.map(|(m, _)| m).or_else(|| nearest_mark(e.dir, &visible, planet.radius)) else {
            if b.charging > 0.0 {
                b.charging = 0.0;
                b.target = None;
                drop_line(&mut commands, entity);
            }
            vis.state = EnemyVis::IDLE;
            continue;
        };
        // where the mark WILL be when a bolt fired now arrives
        let arc = sphere::arc_dist(e.dir, mark.dir, planet.radius);
        let speed = mark.vel.length();
        let lead_dir = if speed > 0.3 {
            sphere::offset_dir(mark.dir, mark.vel / speed, speed * arc / PRIME_BOLT_SPEED, planet.radius)
        } else {
            mark.dir
        };
        if b.charging > 0.0 {
            b.charging -= dt;
            if b.charging > PRIME_LOCK_SECS {
                let aim = tangent(lead_dir - e.dir, e.dir);
                if aim != Vec3::ZERO {
                    let straight = tangent(mark.dir - e.dir, e.dir);
                    telemetry.prime_max_lead = telemetry.prime_max_lead.max(aim.angle_between(straight));
                    b.aim = aim;
                }
            }
            vis.state = if b.charging > PRIME_LOCK_SECS { EnemyVis::CHARGING } else { EnemyVis::LOCKED };
            vis.heading = b.aim;
            if b.charging <= 0.0 {
                // FIRE: from the muzzle down (or up) to the led mark's chest, over the curve
                b.cd = PRIME_COOLDOWN;
                let span = sphere::arc_dist(e.dir, lead_dir, planet.radius);
                let above = (mark.pos.length() - planet.surface(mark.dir)).max(0.6);
                let shot = CurveShot {
                    r0: planet.surface(e.dir) + 1.2 * e.scale,
                    r1: planet.surface(lead_dir) + above,
                    span,
                    flown: 0.0,
                };
                commands.spawn((
                    EnemyProjectile {
                        dir: e.dir,
                        heading: b.aim,
                        speed: PRIME_BOLT_SPEED,
                        damage: e.damage,
                        life: (span + 12.0) / PRIME_BOLT_SPEED,
                        hover: 1.0,
                    },
                    shot,
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.ring_mat.clone()),
                    Transform::from_translation(e.dir * shot.r0).with_scale(Vec3::new(0.5, 0.5, 3.0)),
                    StageScoped,
                ));
                drop_line(&mut commands, entity);
                telemetry.prime_shots += 1;
                vis.state = EnemyVis::IDLE;
            }
            continue;
        }
        vis.state = EnemyVis::IDLE;
        b.cd -= dt;
        if b.cd <= 0.0 && arc < PRIME_RANGE {
            b.charging = PRIME_CHARGE_SECS;
            b.target = Some(mark.entity);
            b.aim = tangent(lead_dir - e.dir, e.dir);
            vis.heading = b.aim;
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

/// Pay a dead Mimic's swallowed gold back to the purse that fed it: the payer's own sheet on
/// the host, and — when the payer is a joiner — the same grant to their machine.
#[allow(clippy::too_many_arguments)]
pub fn pay_refunds(
    mut commands: Commands,
    particles: Option<Res<ParticleAssets>>,
    mut reader: MessageReader<RefundMsg>,
    mut q: Query<(&mut PlayerState, &PlayerId, Has<LocalPlayer>)>,
    mut grants: MessageWriter<crate::net::GrantOut>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    for m in reader.read() {
        // a payer who left the session took their purse with them
        let Ok((mut ps, pid, local)) = q.get_mut(m.payer) else { continue };
        ps.gold += m.gold;
        if let Some(pa) = &particles {
            fx::burst(&mut commands, pa, m.pos, m.pos.normalize_or_zero(), Pcolor::Gold, 20, 7.0);
        }
        telemetry.mimic_refunds += m.gold;
        if local {
            banners.write(BannerMsg(format!("THE MIMIC COUGHS UP YOUR {} GOLD", m.gold)));
            sfx.write(SfxMsg(Sfx::Coin));
        } else {
            grants.write(crate::net::GrantOut::Loot(pid.0, crate::pickups::PickupKind::Gold(m.gold)));
        }
    }
}

/// Owned telegraphs die with their owner — a Trencher killed mid-windup, a Sunskimmer shot
/// out of its dive (host, and a client's streamed copies against their proxies).
pub fn reap_owned_telegraphs(
    mut commands: Commands,
    q: Query<(Entity, &TelegraphOwner)>,
    alive: Query<(), With<Enemy>>,
    mut telemetry: ResMut<BestiaryTelemetry>,
) {
    for (e, owner) in &q {
        if alive.get(owner.0).is_err() {
            commands.entity(e).try_despawn();
            telemetry.rings_reaped += 1;
        }
    }
}

// ─── presentation (every machine) ───────────────────────────────────────────

/// Pose the new kinds from `EnemyVis` and their motion — the same on the host (after the
/// movers) and on a client (after `netenemy::drive_proxies`). Whole-transform only.
pub fn bestiary_pose(
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    mut q: Query<(&Enemy, &EnemyVis, &mut PoseMemo, &mut Transform)>,
) {
    let Some(planet) = planet else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t_now = time.elapsed_secs();
    for (e, vis, mut memo, mut tf) in &mut q {
        let up = e.dir;
        let moved = (up - memo.prev).length() * planet.radius;
        if moved > 1e-3 {
            let mv = tangent(up - memo.prev, up);
            if mv != Vec3::ZERO {
                memo.heading = mv;
            }
        }
        memo.heading = tangent(memo.heading, up);
        if memo.heading == Vec3::ZERO {
            memo.heading = sphere::tangent_frame(up).0;
        }
        memo.prev = up;
        if vis.state != memo.state {
            memo.state = vis.state;
            memo.age = 0.0;
        } else {
            memo.age += dt;
        }
        // the shield/aim facing eases toward what the simulation says
        let want = tangent(vis.heading, up);
        memo.facing = if want == Vec3::ZERO {
            tangent(memo.facing, up)
        } else if memo.facing == Vec3::ZERO {
            want
        } else {
            turn_toward(memo.facing, want, up, 12.0 * dt)
        };
        let pulse = 1.0 + e.flash * 0.25;
        let ground = planet.surface_point(up);
        match e.kind {
            EnemyKind::Rollo => {
                // a ball rolls by the distance it covers
                memo.spin = (memo.spin + moved / (0.52 * e.scale)) % std::f32::consts::TAU;
                let mut rot = sphere::frame_quat(up, memo.heading) * Quat::from_rotation_x(-memo.spin);
                if vis.state == EnemyVis::STUNNED {
                    // dizzy: a decaying wobble on the spot
                    let k = (1.0 - memo.age / ROLLO_STUN_SECS).max(0.0);
                    rot = sphere::frame_quat(up, memo.heading)
                        * Quat::from_rotation_z((t_now * 11.0).sin() * 0.35 * k)
                        * Quat::from_rotation_x(-memo.spin);
                }
                tf.translation = ground + up * (0.52 * e.scale);
                tf.rotation = rot;
                tf.scale = Vec3::splat(e.scale * pulse);
            }
            EnemyKind::Trencher => {
                let face = sphere::frame_quat(up, memo.heading);
                match vis.state {
                    EnemyVis::SINKING => {
                        let f = (memo.age / TRENCH_SINK_SECS).clamp(0.0, 1.0);
                        tf.translation = ground + up * (e.scale * (0.6 - 1.02 * f));
                        tf.rotation = face * Quat::from_rotation_x(-0.5 * f);
                    }
                    EnemyVis::TUNNEL | EnemyVis::WINDUP => {
                        // only the finned spine cuts the crust; it shudders as it winds up
                        let shake = if vis.state == EnemyVis::WINDUP { (t_now * 43.0).sin() * 0.09 } else { (t_now * 9.0).sin() * 0.03 };
                        tf.translation = ground - up * (0.42 * e.scale);
                        tf.rotation = face * Quat::from_rotation_z(shake);
                    }
                    EnemyVis::ERUPT => {
                        // the uppercut: bursting up, claws first, and dropping back
                        let f = (memo.age / TRENCH_ERUPT_SECS).clamp(0.0, 1.0);
                        let lift = (std::f32::consts::PI * f).sin() * 1.8;
                        tf.translation = ground + up * (e.scale * 0.6 + lift);
                        tf.rotation = face * Quat::from_rotation_x(1.1 * (1.0 - f));
                    }
                    // surfaced: the crowd gait from enemy_move / drive_proxies stands
                    _ => continue,
                }
                tf.scale = Vec3::splat(e.scale * pulse);
            }
            EnemyKind::Sunskimmer => {
                let dive = vis.state == EnemyVis::DIVE;
                let head = if tangent(vis.heading, up) != Vec3::ZERO { tangent(vis.heading, up) } else { memo.heading };
                let mut rot = sphere::frame_quat(up, head);
                rot *= if dive {
                    // nose down into the dive
                    Quat::from_rotation_x(-0.25 - 0.9 * (1.0 - vis.alt / SKIM_ALTITUDE).clamp(0.0, 1.0))
                } else {
                    // banking, gliding
                    Quat::from_rotation_z((t_now * 1.7 + e.wobble).sin() * 0.3)
                };
                tf.translation = ground + up * (vis.alt + 0.3 * e.scale);
                tf.rotation = rot;
                tf.scale = Vec3::splat(e.scale * pulse);
            }
            EnemyKind::AegisDrone => {
                // the shield faces where the drone says, whatever it walks toward
                if memo.facing != Vec3::ZERO {
                    tf.rotation = sphere::frame_quat(up, memo.facing) * Quat::from_rotation_z((t_now * 1.9 + e.wobble).sin() * 0.12);
                }
            }
            EnemyKind::BeamerPrime => {
                // while painting, the barrel is the aim; the recoil kicks it back when it fires
                if vis.state != EnemyVis::IDLE && memo.facing != Vec3::ZERO {
                    let brace = if vis.state == EnemyVis::LOCKED { 0.08 } else { 0.0 };
                    tf.rotation = sphere::frame_quat(up, memo.facing) * Quat::from_rotation_x(brace);
                } else if memo.state == EnemyVis::IDLE && memo.age < 0.25 && memo.facing != Vec3::ZERO {
                    tf.rotation = sphere::frame_quat(up, memo.facing) * Quat::from_rotation_x(0.35 * (1.0 - memo.age / 0.25));
                }
            }
            EnemyKind::Mimic => {
                // hopping on stubby legs, lid flapping; it sinks as it digs out
                memo.spin = (memo.spin + moved * 2.2) % std::f32::consts::TAU;
                let hop = memo.spin.sin().abs() * 0.35;
                let dig = if vis.state == EnemyVis::DIGGING { (memo.age / 1.0).clamp(0.0, 1.0) } else { 0.0 };
                tf.translation = ground + up * (e.scale * (0.5 - 1.1 * dig) + hop * (1.0 - dig));
                tf.rotation = sphere::frame_quat(up, memo.heading) * Quat::from_rotation_x((memo.spin * 2.0).sin() * 0.16);
                tf.scale = Vec3::splat(e.scale * pulse);
            }
            _ => {}
        }
    }
}

/// A tunnelling Trencher throws up its ridge: a mound every TRENCH_MOUND_STEP metres,
/// rising and slumping back over TRENCH_MOUND_SECS. Runs wherever the Trencher is drawn.
pub fn trencher_ridges(
    mut commands: Commands,
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    assets: Option<Res<BestiaryAssets>>,
    mut q: Query<(&Enemy, &EnemyVis, &mut PoseMemo)>,
    mut mounds: Query<(Entity, &mut RidgeMound, &mut Transform)>,
) {
    let (Some(planet), Some(assets)) = (planet, assets) else { return };
    let dt = time.delta_secs();
    for (e, vis, mut memo) in &mut q {
        if e.kind != EnemyKind::Trencher {
            continue;
        }
        let under = matches!(vis.state, EnemyVis::TUNNEL | EnemyVis::WINDUP);
        if !under {
            memo.mound_at = e.dir;
            continue;
        }
        if sphere::arc_dist(memo.mound_at, e.dir, planet.radius) >= TRENCH_MOUND_STEP {
            memo.mound_at = e.dir;
            let spin = (e.dir.x * 71.3 + e.dir.z * 17.9).fract() * std::f32::consts::TAU;
            let mut tf = enemies::ground_decal(&planet, e.dir, 0.0);
            tf.rotation *= Quat::from_rotation_y(spin);
            commands.spawn((
                RidgeMound { age: 0.0, dir: e.dir },
                Mesh3d(assets.mound_mesh.clone()),
                MeshMaterial3d(assets.mound_mat.clone()),
                tf.with_scale(Vec3::splat(0.01)),
                StageScoped,
            ));
        }
    }
    if dt <= 0.0 {
        return;
    }
    for (me, mut m, mut tf) in &mut mounds {
        m.age += dt;
        let f = m.age / TRENCH_MOUND_SECS;
        if f >= 1.0 {
            commands.entity(me).despawn();
            continue;
        }
        // heave up fast, slump back slowly
        let s = if f < 0.15 { f / 0.15 } else { 1.0 - (f - 0.15) / 0.85 };
        tf.scale = Vec3::new(1.5, 1.2 * s.max(0.02), 1.5);
        tf.translation = planet.surface_point(m.dir) - m.dir * 0.1;
    }
}

/// A shadow under every Sunskimmer, shrinking as it climbs: where it is, and — its
/// hitbox being the column under it — where to shoot.
pub fn skimmer_shadows(
    mut commands: Commands,
    planet: Option<Res<CurrentPlanet>>,
    assets: Option<Res<BestiaryAssets>>,
    q: Query<(Entity, &Enemy, &EnemyVis)>,
    mut shadows: Query<(Entity, &SkimShadow, &mut Transform)>,
    mut have: Local<HashMap<Entity, Entity>>,
) {
    let (Some(planet), Some(assets)) = (planet, assets) else { return };
    let mut live: HashMap<Entity, (Vec3, f32)> = HashMap::new();
    for (e, en, vis) in &q {
        if en.kind == EnemyKind::Sunskimmer {
            live.insert(e, (en.dir, vis.alt));
        }
    }
    for (se, sh, mut tf) in &mut shadows {
        let Some(&(dir, alt)) = live.get(&sh.owner) else {
            commands.entity(se).try_despawn();
            have.remove(&sh.owner);
            continue;
        };
        let k = 0.55 + 0.45 * (1.0 - alt / SKIM_ALTITUDE).clamp(0.0, 1.0);
        let mut t = enemies::ground_decal(&planet, dir, 0.08);
        t.scale = Vec3::new(1.1 * k, 1.0, 0.8 * k);
        *tf = t;
    }
    have.retain(|owner, _| live.contains_key(owner));
    for (owner, (dir, _)) in live {
        if have.contains_key(&owner) {
            continue;
        }
        let s = commands
            .spawn((
                SkimShadow { owner },
                Mesh3d(assets.shadow_mesh.clone()),
                MeshMaterial3d(assets.shadow_mat.clone()),
                enemies::ground_decal(&planet, dir, 0.08).with_scale(Vec3::splat(0.6)),
                StageScoped,
            ))
            .id();
        have.insert(owner, s);
    }
}

/// The Sunskimmer's whine as it commits to a dive near THIS machine's astronaut — at night
/// you hear it before you see it (§9).
pub fn skimmer_whine(
    planet: Option<Res<CurrentPlanet>>,
    q: Query<(Entity, &Enemy, &EnemyVis)>,
    me: Query<&Player, With<LocalPlayer>>,
    mut diving: Local<HashSet<Entity>>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let Some(planet) = planet else { return };
    let mine = me.iter().next().map(|p| p.dir);
    let mut now: HashSet<Entity> = HashSet::new();
    for (e, en, vis) in &q {
        if en.kind != EnemyKind::Sunskimmer || vis.state != EnemyVis::DIVE {
            continue;
        }
        now.insert(e);
        if !diving.contains(&e) && mine.is_some_and(|d| sphere::arc_dist(d, en.dir, planet.radius) < SKIM_WHINE_ARC) {
            sfx.write(SfxMsg(Sfx::Whine));
        }
    }
    *diving = now;
}

/// A disguised Mimic breathes now and then — its lid lifts a hair. The one tell.
pub fn mimic_tells(time: Res<Time>, mut q: Query<(&mut MimicDisguise, &Children)>, mut parts: Query<&mut Transform>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (mut d, children) in &mut q {
        d.tell -= dt;
        if d.tell <= 0.0 && d.breath <= 0.0 {
            d.breath = 0.6;
            d.tell = rng.gen_range(MIMIC_TELL_SECS.0..MIMIC_TELL_SECS.1);
        }
        let s = if d.breath > 0.0 {
            d.breath -= dt;
            1.0 + 0.14 * (std::f32::consts::PI * (1.0 - d.breath / 0.6).clamp(0.0, 1.0)).sin()
        } else {
            1.0
        };
        for c in children.iter() {
            if let Ok(mut tf) = parts.get_mut(c) {
                tf.scale = Vec3::new(1.0, s, 1.0);
            }
        }
    }
}

/// The new kinds' one-shots, drawn on every machine (the host's local messages, a client's
/// from the hazard lane): the uppercut's burst and — for a joiner it caught — the launch
/// of its own predicted body; the tracker beacon; a disguised chest a client must drop.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn bestiary_fx_presentation(
    mut commands: Commands,
    mut reader: MessageReader<BestiaryFxMsg>,
    planet: Option<Res<CurrentPlanet>>,
    assets: Option<Res<BestiaryAssets>>,
    particles: Option<Res<ParticleAssets>>,
    mut shake: ResMut<Shake>,
    mut me: Query<(&mut Player, &PlayerId), With<LocalPlayer>>,
    disguises: Query<(Entity, &Transform), With<MimicDisguise>>,
    mut beacons: Query<&mut TrackerBeacon>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let (Some(planet), Some(assets)) = (planet, assets) else {
        reader.clear();
        return;
    };
    for m in reader.read() {
        match m.fx {
            BestiaryFx::Uppercut { dir, launched } => {
                if m.from_wire {
                    if let Some(pa) = &particles {
                        fx::burst(&mut commands, pa, planet.surface_point(dir), dir, Pcolor::White, 16, 8.0);
                    }
                }
                if let Ok((mut p, pid)) = me.single_mut() {
                    if launched & (1 << pid.0.min(7)) != 0 {
                        shake.add(0.3);
                        if m.from_wire {
                            // the host threw its copy of us; our own body goes up with it
                            p.vel_r = p.vel_r.max(TRENCH_LAUNCH_VEL);
                            p.grounded = false;
                            p.height = p.height.max(0.05);
                        }
                    }
                }
            }
            BestiaryFx::Tracked { owner, secs } => {
                if let Some(mut b) = beacons.iter_mut().find(|b| b.pid == owner) {
                    b.left = secs;
                } else {
                    commands.spawn((
                        TrackerBeacon { pid: owner, left: secs },
                        Mesh3d(assets.beacon_mesh.clone()),
                        MeshMaterial3d(assets.beacon_mat.clone()),
                        Transform::from_scale(Vec3::ZERO),
                        StageScoped,
                    ));
                }
                if me.single().is_ok_and(|(_, pid)| pid.0 == owner) {
                    banners.write(BannerMsg(format!("A BEACON TICK TAGGED YOU — THE HORDE KNOWS WHERE YOU ARE ({secs:.0}s)")));
                }
            }
            BestiaryFx::MimicSprung { dir } => {
                if !m.from_wire {
                    continue; // the host despawned its own chest when it sprang
                }
                if let Some((chest, _)) = disguises
                    .iter()
                    .map(|(e, tf)| (e, sphere::arc_dist(tf.translation.normalize_or_zero(), dir, planet.radius)))
                    .filter(|(_, arc)| *arc < 3.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                {
                    commands.entity(chest).try_despawn();
                }
                if let Some(pa) = &particles {
                    fx::burst(&mut commands, pa, planet.surface_point(dir), dir, Pcolor::Gold, 16, 7.0);
                }
            }
        }
    }
}

/// Keep each tracker beacon over its astronaut (ours, or a teammate's rig on a client),
/// blinking at 2 Hz (under the photosensitivity ceiling) until it runs out.
#[allow(clippy::type_complexity)]
pub fn tracker_beacons(
    mut commands: Commands,
    time: Res<Time>,
    bodies: Query<(&PlayerId, &Transform), (Or<(With<Player>, With<crate::remote::RemoteAstronaut>)>, Without<TrackerBeacon>)>,
    mut q: Query<(Entity, &mut TrackerBeacon, &mut Transform, &mut Visibility)>,
) {
    let dt = time.delta_secs();
    let t_now = time.elapsed_secs();
    for (e, mut b, mut tf, mut vis) in &mut q {
        b.left -= dt;
        let body = bodies.iter().find(|(pid, _)| pid.0 == b.pid).map(|(_, t)| t.translation);
        let Some(pos) = body.filter(|_| b.left > 0.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let up = pos.normalize_or_zero();
        tf.translation = pos + up * 1.9;
        tf.rotation = sphere::frame_quat(up, sphere::tangent_frame(up).0);
        tf.scale = Vec3::ONE;
        *vis = if (t_now * 2.0).fract() < 0.6 { Visibility::Visible } else { Visibility::Hidden };
    }
}

// ─── self-check ─────────────────────────────────────────────────────────────

/// Headless RULES: the pure pieces of the new kinds' behaviour, pinned.
pub fn self_check() -> Result<(), String> {
    crate::content::enemies::table_self_check()?;
    let up = Vec3::Y;
    // Rollo's turn is capped
    let h = turn_toward(Vec3::X, Vec3::NEG_X + Vec3::Z * 0.01, up, 0.1);
    if (h.angle_between(Vec3::X) - 0.1).abs() > 1e-3 {
        return Err(format!("turn_toward turned {} rad, cap 0.1", h.angle_between(Vec3::X)));
    }
    if turn_toward(Vec3::X, Vec3::Z, up, 0.1).dot(Vec3::Z) <= 0.0 {
        return Err("turn_toward turned away from its target".into());
    }
    // the shield: a shot travelling INTO its face is blocked, one from behind or the side lands
    let face = Vec3::X;
    if !shield_blocks(up, face, Vec3::NEG_X * 4.0, None) {
        return Err("a frontal shot got through the Aegis shield".into());
    }
    if shield_blocks(up, face, Vec3::X * 4.0, None) || shield_blocks(up, face, Vec3::Z * 4.0, None) {
        return Err("the Aegis shield blocked a flank or rear shot".into());
    }
    if !shield_blocks(up, face, Vec3::ZERO, Some(up + Vec3::X)) || shield_blocks(up, face, Vec3::ZERO, Some(up - Vec3::X)) {
        return Err("the Aegis shield misread an aura's shooter".into());
    }
    if shield_blocks(up, face, Vec3::ZERO, None) {
        return Err("the Aegis shield blocked an environment hit".into());
    }
    // the tracker: a tagged astronaut 90 m off beats an untagged one 50 m off for the horde
    let r = 140.0;
    let a = crate::player::AstronautSnap { entity: Entity::PLACEHOLDER, dir: sphere::offset_dir(up, Vec3::X, 50.0, r), pos: Vec3::ZERO };
    let b = crate::player::AstronautSnap { entity: Entity::PLACEHOLDER, dir: sphere::offset_dir(up, Vec3::NEG_X, 90.0, r), pos: Vec3::ZERO };
    let lure = [enemies::horde_lure(false, false), enemies::horde_lure(false, true)];
    if enemies::chase_target(up, &[a, b], &lure, r) != 1 {
        return Err("a Beacon Tick's tracker did not pull the far horde".into());
    }
    if enemies::chase_target(up, &[a, b], &[1.0, 1.0], r) != 0 {
        return Err("the horde's untracked chase is not the nearest astronaut".into());
    }
    // the railbolt's line of fire eases from muzzle to mark, then holds
    let c = CurveShot { r0: 100.0, r1: 104.0, span: 40.0, flown: 20.0 };
    if (c.radius() - 102.0).abs() > 1e-3 || (CurveShot { flown: 90.0, ..c }).radius() != 104.0 {
        return Err("CurveShot's line of fire is not muzzle-to-mark".into());
    }
    Ok(())
}

// ─── dev harness ────────────────────────────────────────────────────────────

/// The new kinds named on the command line after `flag` (`all`, or a comma list:
/// rollo, trencher, aegis, skimmer, tick, mimic, prime — or their full names).
pub fn kinds_from_args(flag: &str) -> Vec<EnemyKind> {
    use EnemyKind::*;
    const NEW: [EnemyKind; 7] = [Rollo, Trencher, AegisDrone, Sunskimmer, BeaconTick, Mimic, BeamerPrime];
    let args: Vec<String> = std::env::args().collect();
    let Some(i) = args.iter().position(|a| a == flag) else { return Vec::new() };
    let list = args.get(i + 1).filter(|s| !s.starts_with("--")).map(|s| s.to_lowercase()).unwrap_or_else(|| "all".into());
    if list == "all" {
        return NEW.to_vec();
    }
    list.split(',')
        .filter_map(|name| {
            let n: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
            NEW.iter().copied().find(|k| {
                let full: String = k.def().name.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                full == n || full.contains(&n) || (n == "prime" && *k == BeamerPrime) || (n == "tick" && *k == BeaconTick)
            })
        })
        .collect()
}

/// `--dev --enemies all|a,b,…`: every 12 s, walk the named kinds in on a ring 16–20 m
/// around this machine's astronaut — the windowed look/feel check for the new kinds. Host
/// or solo only (a client's spawn would be one its host never simulates).
pub fn dev_spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    mut me: Query<&mut Player, With<LocalPlayer>>,
    q_party: Query<(), With<Player>>,
    mut next: Local<f32>,
    mut kinds: Local<Option<Vec<EnemyKind>>>,
) {
    let kinds = kinds.get_or_insert_with(|| kinds_from_args("--enemies"));
    let now = time.elapsed_secs();
    if kinds.is_empty() || now < *next {
        return;
    }
    let Ok(mut p) = me.single_mut() else { return };
    if *next == 0.0 { p.dir = (crate::planet::sunward() + Vec3::Y * 0.3).normalize(); } // TEMP daylight
    *next = now + 12.0;
    let sc = Scaling::for_run(&run, q_party.iter().count());
    let mut rng = rand::thread_rng();
    let (t, b) = sphere::tangent_frame(p.dir);
    let n = kinds.len();
    for (i, kind) in kinds.iter().enumerate() {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let dir = sphere::offset_dir(p.dir, t * a.cos() + b * a.sin(), rng.gen_range(16.0..20.0), planet.radius);
        enemies::spawn_enemy_at(&mut commands, &assets, &planet, *kind, dir, false, &sc, &mut rng);
    }
    info!("DEV --enemies: {} walked in", kinds.iter().map(|k| k.def().name).collect::<Vec<_>>().join(", "));
}

