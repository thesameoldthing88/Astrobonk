//! Co-op rules (GDD §11, P18): the Tumbling Beacon (down, revive, the Static Meter, Hero's
//! Adrenaline), drop-in with its autopilot grace, and friendly physics.
//!
//! The model, in one paragraph: `PlayerState::dead` is the DOWN state. At 0 HP with no save
//! left (`combat::apply_player_hits`) an astronaut goes down and becomes a Tumbling Beacon —
//! the body rolls down the terrain's fall line (`tumble`, from `player_physics`, so a joiner
//! predicts its own roll like it predicts its run) under a planet-high distress flare
//! (`beacon_flares`). A teammate standing in its ring for REVIVE_SECS brings it back up
//! (`beacon_rescue`), and earns Hero's Adrenaline; nobody can revive themselves. The Static
//! Meter fills meanwhile: full, The Static CLAIMS the astronaut — hidden, out of every
//! target list — until the next teleporter (`director::stage_transition` → `rejoin`). Solo,
//! going down is still the end of the run (`director::downed_watch`, after P04's "one more
//! chance" token), and none of the Beacon's co-op dressing is drawn.
//!
//! Everything that SIMULATES here is host-only; what a joiner must see rides
//! `net::PlayerVitals` (down / claimed / meter / revive / adrenaline / chill / grace / drop
//! level) and one-shots on the hazard lane (`CoopFx`: a revive, a shove of the joiner's own
//! predicted body, a drop-in's landing, and `duos`' set-pieces).

use crate::config::*;
use crate::content::duos::CoopFeat;
use crate::enemies::{Boss, Enemy, SpatialHash};
use crate::fx::{self, Pcolor, ParticleAssets};
use crate::interact::Pot;
use crate::messages::{BannerMsg, Sfx, SfxMsg};
use crate::net::PlayerVitals;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{InputIntent, LocalPlayer, Player, PlayerId};
use crate::remote::RemoteAstronaut;
use crate::run::{PlayerState, RunState};
use crate::save::MetaSave;
use crate::sphere;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

// ─── shared assets ────────────────────────────────────────────────────────────

/// Meshes (and the materials that never change colour) for the Beacon's flare, the revive
/// ring and STATIC CASCADE's belt. Per-Beacon materials are made per Beacon — there are at
/// most three — because their colour follows that Beacon's own meters.
#[derive(Resource)]
pub struct CoopAssets {
    pub column_mesh: Handle<Mesh>,
    pub ball_mesh: Handle<Mesh>,
    pub ring_mesh: Handle<Mesh>,
    pub belt_mesh: Handle<Mesh>,
    pub belt_mat: Handle<StandardMaterial>,
}

pub fn setup_coop_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(CoopAssets {
        // unit column: 1 m tall, 1 m across, scaled per flare
        column_mesh: meshes.add(Mesh::from(Cylinder::new(0.5, 1.0))),
        ball_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
        // a flat ring of radius 1 (tube 0.06), scaled to REVIVE_RADIUS
        ring_mesh: meshes.add(Mesh::from(Torus::new(0.94, 1.06))),
        // one belt segment: a unit bar along Z, scaled to its chord
        belt_mesh: meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0))),
        belt_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.9, 1.0, 0.9),
            emissive: LinearRgba::rgb(4.0, 6.0, 9.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        }),
    });
}

// ─── one-shots on the hazard lane ─────────────────────────────────────────────

/// A co-op moment every machine presents. The host's systems write these as local
/// messages; `netenemy::stream_hazards` sends them as `HazardEvent`s and a client turns them
/// back into the same message (`from_wire`), so `coop_fx_presentation` is one path.
#[derive(Clone, Copy, Debug)]
pub enum CoopFx {
    /// `rescuer` stood in `downed`'s ring long enough; `dir` is where the Beacon lay.
    Revived { rescuer: u8, downed: u8, dir: Vec3 },
    /// Friendly physics moved astronaut `target` (a boop along the ground, a hop). A joiner
    /// applies it to its own predicted body, which the host's copy would otherwise drag.
    Shove { target: u8, vel: Vec3, pop: f32 },
    /// STATIC CASCADE fired between `a` and `b` round the great circle about `axis`.
    Cascade { a: u8, b: u8, axis: Vec3 },
    /// A named duo landed: `a` set it up, `b` finished it, at `dir`. `first`: the squad's
    /// first of this duo for this pair this run (the banner; later ones are a burst only).
    Duo { feat: CoopFeat, a: u8, b: u8, dir: Vec3, first: bool },
    /// Drop-in `owner` landed from orbit at `dir`, at `level`.
    DropIn { owner: u8, level: u8, dir: Vec3 },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct CoopFxMsg {
    pub fx: CoopFx,
    /// Rebuilt from the hazard lane on a client.
    pub from_wire: bool,
}

/// What the co-op systems did this run — for the headless probes and the net log.
#[derive(Resource, Default, Debug)]
pub struct CoopTelemetry {
    pub downs: u32,
    pub revives: u32,
    pub claims: u32,
    pub rejoins: u32,
    /// Metres the Beacons rolled, all told.
    pub beacon_roll_m: f32,
    pub shoves: u32,
    pub chills: u32,
    pub jolts: u32,
    pub dropins: u32,
    pub dropin_landings: u32,
    pub autopilot_secs: f32,
    pub cascades: u32,
    pub cascade_hits: u32,
    /// Per `CoopFeat::code`.
    pub feats: [u32; 5],
    pub shatter_hits: u32,
    /// CoopFx one-shots presented (a joiner's count arrived over the wire).
    pub fx_seen: u32,
}

/// Is this a squad? The Beacon's co-op dressing (flare, ring, HUD) is only drawn when there
/// is someone to answer it: solo, going down is the end of the run, as it always was.
fn squad(n_bodies: usize) -> bool {
    n_bodies > 1
}

// ─── the Tumbling Beacon ──────────────────────────────────────────────────────

/// Physics of a downed body, from `player_physics` (every body a machine moves — so a
/// joiner predicts its own roll): it rolls down the terrain's fall line — the same noise
/// field the mesh and collision use, so "downhill" is exactly what the ground shows — and
/// settles in a crater's bowl. A claimed body is parked (and unseen).
pub fn tumble(p: &mut Player, planet: &CurrentPlanet, claimed: bool, dt: f32) {
    p.slide_timer = 0.0;
    if claimed {
        p.vel_t = Vec3::ZERO;
        return;
    }
    if p.grounded {
        let uphill = planet.terrain.slope(p.dir, planet.radius);
        let grade = uphill.length();
        if grade > BEACON_MIN_GRADE {
            let fall = -uphill / grade;
            let sin = grade / (1.0 + grade * grade).sqrt();
            p.vel_t += fall * PLAYER_GRAVITY * sin * BEACON_ROLL_GAIN * dt;
        }
        p.vel_t *= (-BEACON_ROLL_DRAG * dt).exp();
        if p.vel_t.length() > BEACON_ROLL_MAX {
            p.vel_t = p.vel_t.normalize() * BEACON_ROLL_MAX;
        }
        if p.vel_t.length() < 0.05 {
            p.vel_t = Vec3::ZERO;
        }
    }
    if let Some(h) = p.vel_t.try_normalize() {
        p.facing = h;
    }
}

/// Is this drawn body down right now (and not claimed)? A body this machine simulates
/// reads its sheet; a teammate drawn on a joiner reads its replicated vitals.
fn beacon_state(ps: Option<&PlayerState>, v: Option<&PlayerVitals>) -> BeaconState {
    match (ps, v) {
        (Some(ps), _) => BeaconState {
            down: ps.dead && !ps.claimed,
            claimed: ps.claimed,
            meter: ps.static_meter,
            revive: ps.revive,
        },
        (None, Some(v)) => BeaconState {
            down: v.down && v.status & crate::net::VITALS_CLAIMED == 0,
            claimed: v.status & crate::net::VITALS_CLAIMED != 0,
            meter: v.static_meter as f32 / 255.0,
            revive: v.revive as f32 / 255.0,
        },
        _ => BeaconState::default(),
    }
}

#[derive(Default, Clone, Copy)]
struct BeaconState {
    down: bool,
    claimed: bool,
    meter: f32,
    revive: f32,
}

/// HOST: the Beacons' clocks. Revive progress while a standing teammate is in the ring
/// (drained, not reset, when they step out), the Static Meter always, and the timers every
/// astronaut carries (Hero's Adrenaline, a friendly chill, drop-in grace). A revive beats a
/// claim on the same frame.
#[allow(clippy::too_many_arguments)]
pub fn beacon_rescue(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut q: Query<(&PlayerId, &Player, &mut PlayerState)>,
    mut fx: MessageWriter<CoopFxMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
    mut downed: Local<HashSet<u8>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let bodies: Vec<(u8, Vec3, bool, crate::content::characters::AstronautKind)> =
        q.iter().map(|(id, p, ps)| (id.0, p.dir, ps.dead, ps.character)).collect();
    let fill = dt / STATIC_METER_SECS * if run.static_active { STATIC_METER_OVERTIME } else { 1.0 };
    let mut rescued: Vec<(u8, u8)> = Vec::new();
    for (pid, p, mut ps) in &mut q {
        ps.adrenaline = (ps.adrenaline - dt).max(0.0);
        ps.chill = (ps.chill - dt).max(0.0);
        ps.grace = (ps.grace - dt).max(0.0);
        if !ps.dead {
            downed.remove(&pid.0);
            continue;
        }
        if downed.insert(pid.0) {
            telemetry.downs += 1;
        }
        if ps.claimed {
            continue;
        }
        // the nearest STANDING teammate in the ring — never the Beacon itself (§11: "you
        // cannot self-revive")
        let rescuer = bodies
            .iter()
            .filter(|(id, _, dead, _)| *id != pid.0 && !dead)
            .map(|(id, d, _, _)| (*id, sphere::arc_dist(p.dir, *d, planet.radius)))
            .filter(|(_, arc)| *arc <= REVIVE_RADIUS)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        ps.revive = match rescuer {
            Some(_) => ps.revive + dt / REVIVE_SECS,
            None => (ps.revive - dt / REVIVE_DECAY_SECS).max(0.0),
        };
        ps.static_meter = (ps.static_meter + fill).min(1.0);
        telemetry.beacon_roll_m += p.vel_t.length() * dt;
        if let (Some((by, _)), true) = (rescuer, ps.revive >= 1.0) {
            ps.revive_up();
            downed.remove(&pid.0);
            rescued.push((by, pid.0));
            telemetry.revives += 1;
            fx.write(CoopFxMsg { fx: CoopFx::Revived { rescuer: by, downed: pid.0, dir: p.dir }, from_wire: false });
            sfx.write(SfxMsg(Sfx::Shrine));
            info!("COOP player {} revived player {}", by, pid.0);
        } else if ps.static_meter >= 1.0 {
            ps.claimed = true;
            ps.revive = 0.0;
            telemetry.claims += 1;
            sfx.write(SfxMsg(Sfx::BossRoar));
            info!("COOP The Static claimed player {}", pid.0);
        }
    }
    // Hero's Adrenaline for each rescuer, and the results screen's rescue tally
    let fallback = run.character;
    for (by, down) in rescued {
        for (pid, _, mut ps) in &mut q {
            if pid.0 == by {
                ps.adrenaline = ADRENALINE_SECS;
                ps.rescues += 1;
            }
        }
        let hero = |id: u8| bodies.iter().find(|b| b.0 == id).map(|b| b.3).unwrap_or(fallback);
        crate::duos::tally(&mut run.feats, CoopFeat::Rescue, (by, hero(by)), (down, hero(down)));
        telemetry.feats[CoopFeat::Rescue.code() as usize] += 1;
    }
}

/// The rolling body's pose: a log on its side, turning over as it rolls. Kept per body so
/// the roll is continuous; `lying` eases in and out so going down and standing back up are
/// never a snap.
#[derive(Component, Default)]
pub struct Tumble {
    roll: f32,
    heading: Vec3,
    last: Vec3,
    lying: f32,
}

/// EVERY machine, presentation: lay a downed body down and roll it with its motion, on
/// whatever moved it this frame (`player_physics` for a body this machine moves, the
/// replicated pose for a teammate a joiner draws).
#[allow(clippy::type_complexity)]
pub fn tumble_pose(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut q: Query<
        (Entity, &mut Transform, Option<&PlayerState>, Option<&PlayerVitals>, Option<&mut Tumble>),
        Or<(With<Player>, With<RemoteAstronaut>)>,
    >,
) {
    let dt = time.delta_secs();
    for (e, mut tf, ps, v, tumble) in &mut q {
        let Some(mut t) = tumble else {
            commands.entity(e).insert(Tumble::default());
            continue;
        };
        let st = beacon_state(ps, v);
        let want = if st.down { 1.0 } else { 0.0 };
        t.lying += (want - t.lying) * (1.0 - (-10.0 * dt).exp());
        let up = tf.translation.normalize_or_zero();
        if up == Vec3::ZERO {
            continue;
        }
        let moved = if t.last == Vec3::ZERO { 0.0 } else { sphere::arc_dist(t.last, up, planet.radius) };
        if moved > 1e-3 && moved < 5.0 {
            let d = up - t.last;
            if let Some(h) = (d - up * d.dot(up)).try_normalize() {
                t.heading = h;
            }
            t.roll = (t.roll + moved / BEACON_BODY_RADIUS) % std::f32::consts::TAU;
        }
        t.last = up;
        if t.lying < 1e-3 {
            continue;
        }
        let heading = if t.heading == Vec3::ZERO { sphere::tangent_frame(up).0 } else { t.heading };
        // on its side (local +Y laid along the right-hand tangent), turning about that axis
        let lying = sphere::frame_quat(up, heading)
            * Quat::from_rotation_x(-t.roll)
            * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        tf.rotation = tf.rotation.slerp(lying, t.lying);
        tf.translation -= up * (PLAYER_HEIGHT * 0.5 - BEACON_BODY_RADIUS) * t.lying;
    }
}

/// EVERY machine, presentation: a body The Static claimed is not there to see.
#[allow(clippy::type_complexity)]
pub fn hide_claimed(
    mut q: Query<(&mut Visibility, Option<&PlayerState>, Option<&PlayerVitals>), Or<(With<Player>, With<RemoteAstronaut>)>>,
) {
    for (mut vis, ps, v) in &mut q {
        let want = if beacon_state(ps, v).claimed { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
    }
}

/// One piece of a Beacon's dressing, owned by the body it marks.
#[derive(Component)]
pub struct BeaconFlare {
    pub owner: Entity,
    part: FlarePart,
    mat: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlarePart {
    /// The planet-high distress column.
    Column,
    /// The flare shell that climbs it, over and over.
    Shell,
    /// The revive ring on the ground: stand in it.
    Ring,
}

const FLARE_AMBER: Color = Color::srgb(1.0, 0.62, 0.15);
const FLARE_STATIC: Color = Color::srgb(0.62, 0.32, 0.95);
const RING_READY: Color = Color::srgb(0.35, 1.0, 0.6);

/// EVERY machine, presentation: each Beacon's distress flare (a column of light a planet
/// radius high, a shell climbing it — seen over the horizon from most of the world; the
/// HUD marks the rest), and the revive ring at its foot. The flare burns amber and sours
/// toward The Static's violet as the meter fills, flickering near the end (never faster
/// than 2.5 Hz, and not at all in photosensitivity mode); the ring brightens toward green
/// as a teammate revives. Drawn only in a squad — solo there is nobody to call.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn beacon_flares(
    mut commands: Commands,
    time: Res<Time<Real>>,
    planet: Res<CurrentPlanet>,
    assets: Res<CoopAssets>,
    save: Res<MetaSave>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bodies: Query<
        (Entity, &Transform, Option<&PlayerState>, Option<&PlayerVitals>),
        (Or<(With<Player>, With<RemoteAstronaut>)>, Without<BeaconFlare>),
    >,
    mut flares: Query<(Entity, &BeaconFlare, &mut Transform)>,
) {
    let t = time.elapsed_secs();
    let in_squad = squad(bodies.iter().count());
    let beacons: HashMap<Entity, (Vec3, BeaconState)> = bodies
        .iter()
        .filter_map(|(e, tf, ps, v)| {
            let st = beacon_state(ps, v);
            (in_squad && st.down).then(|| (e, (tf.translation.normalize_or_zero(), st)))
        })
        .collect();
    let mut dressed: HashSet<Entity> = HashSet::new();
    let height = planet.radius * BEACON_FLARE_RADII;
    let photo = save.accessibility.photosensitive;
    for (fe, flare, mut tf) in &mut flares {
        let Some((dir, st)) = beacons.get(&flare.owner).copied() else {
            commands.entity(fe).despawn();
            continue;
        };
        dressed.insert(flare.owner);
        let ground = planet.surface_point(dir);
        let stand = Quat::from_rotation_arc(Vec3::Y, dir);
        // amber → violet as The Static closes in; the last fifth flickers
        let sour = FLARE_AMBER.mix(&FLARE_STATIC, st.meter);
        let flicker = if !photo && st.meter > 0.8 && (t * 2.5).fract() < 0.3 { 0.35 } else { 1.0 };
        match flare.part {
            FlarePart::Column => {
                let w = 0.55 + 0.15 * (t * std::f32::consts::TAU).sin();
                tf.translation = ground + dir * (height * 0.5);
                tf.rotation = stand;
                tf.scale = Vec3::new(w, height, w);
                if let Some(m) = materials.get_mut(&flare.mat) {
                    m.base_color = sour.with_alpha(0.35 * flicker);
                    m.emissive = sour.to_linear() * (3.0 * flicker);
                }
            }
            FlarePart::Shell => {
                let climb = (t / 1.6).fract();
                tf.translation = ground + dir * (2.0 + (height - 2.0) * climb);
                tf.scale = Vec3::splat(0.9 + 0.8 * (1.0 - climb));
                if let Some(m) = materials.get_mut(&flare.mat) {
                    m.base_color = sour.with_alpha(0.9 * flicker);
                    m.emissive = sour.to_linear() * (8.0 * flicker);
                }
            }
            FlarePart::Ring => {
                tf.translation = ground + dir * 0.15;
                tf.rotation = stand;
                tf.scale = Vec3::new(REVIVE_RADIUS, 1.0, REVIVE_RADIUS);
                let c = FLARE_AMBER.mix(&RING_READY, st.revive);
                // a revive in progress breathes (1.5 Hz: well under the §13 limit)
                let breathe = if st.revive > 0.0 { 0.75 + 0.25 * (t * 1.5 * std::f32::consts::TAU).sin() } else { 1.0 };
                if let Some(m) = materials.get_mut(&flare.mat) {
                    m.base_color = c.with_alpha(0.8);
                    m.emissive = c.to_linear() * (2.0 + 4.0 * st.revive) * breathe;
                }
            }
        }
    }
    for (owner, (dir, _)) in &beacons {
        if dressed.contains(owner) {
            continue;
        }
        for (part, mesh) in [
            (FlarePart::Column, &assets.column_mesh),
            (FlarePart::Shell, &assets.ball_mesh),
            (FlarePart::Ring, &assets.ring_mesh),
        ] {
            let mat = materials.add(StandardMaterial {
                base_color: FLARE_AMBER.with_alpha(0.4),
                emissive: FLARE_AMBER.to_linear() * 3.0,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                BeaconFlare { owner: *owner, part, mat: mat.clone() },
                Mesh3d(mesh.clone()),
                MeshMaterial3d(mat),
                // placed on its first update; parked at the core until then
                Transform::from_translation(*dir).with_scale(Vec3::ZERO),
                Visibility::default(),
                bevy::light::NotShadowCaster,
                StageScoped,
            ));
        }
    }
}

// ─── friendly physics ─────────────────────────────────────────────────────────

/// What a teammate's weapon does to YOU (§11: friendly fire off for damage, ON for physics).
#[derive(Clone, Copy, Debug)]
pub enum ForceKind {
    /// Knocked along the ground at up to this speed (m/s, fading to half at the edge), with a
    /// hop — a Wrench arc, a rocket's blast, a Slam, a drop-in's landing.
    Shove(f32),
    /// A cryo field: slowed for this long (refreshed while you stand in it).
    Chill(f32),
    /// A Tesla arc passing close: a jolt into a hop.
    Jolt,
}

/// Written wherever a weapon pushes, chills or zaps the world (on a client too, by its
/// cosmetic fire — inert there: only the host reads it).
#[derive(Message, Clone, Copy, Debug)]
pub struct FriendlyForce {
    /// The astronaut it came from — never pushed by its own swing.
    pub from: Entity,
    /// World position of its centre.
    pub at: Vec3,
    /// Reach, in metres over the ground.
    pub radius: f32,
    /// A melee arc's aim and the cosine of its half-angle; `None` = all round.
    pub cone: Option<(Vec3, f32)>,
    pub kind: ForceKind,
}

/// HOST: apply friendly forces to every OTHER astronaut caught in them. A shove moves a
/// Beacon too (booping a downed friend downhill is exactly the chaos §11 wants); a chill
/// or a jolt only reaches someone standing. Each astronaut takes one shove or jolt per
/// FRIENDLY_SHOVE_CD at most, so standing beside a Wrench is a nudge, not a juggle. A
/// joiner's own predicted body gets the same impulse from the `CoopFx::Shove` it causes.
#[allow(clippy::too_many_arguments)]
pub fn friendly_physics(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut forces: MessageReader<FriendlyForce>,
    mut q: Query<(Entity, &PlayerId, &mut Player, &mut PlayerState)>,
    mut fx: MessageWriter<CoopFxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
    mut cooldown: Local<HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    cooldown.retain(|_, t| {
        *t -= dt;
        *t > 0.0
    });
    for f in forces.read() {
        let at = f.at.normalize_or_zero();
        if at == Vec3::ZERO {
            continue;
        }
        for (e, pid, mut p, mut ps) in &mut q {
            if e == f.from || ps.claimed {
                continue;
            }
            let arc = sphere::arc_dist(p.dir, at, planet.radius);
            if arc > f.radius + PLAYER_RADIUS {
                continue;
            }
            // away from the centre, along the ground
            let away = (p.dir - at * p.dir.dot(at)).try_normalize();
            if let (Some((aim, cos_half)), Some(away)) = (f.cone, away) {
                if arc > PLAYER_RADIUS && away.dot(aim) < cos_half {
                    continue;
                }
            }
            let away = away.unwrap_or_else(|| sphere::tangent_frame(p.dir).0);
            match f.kind {
                ForceKind::Shove(speed) => {
                    if cooldown.contains_key(&e) {
                        continue;
                    }
                    let falloff = 1.0 - 0.5 * (arc / f.radius.max(0.1)).min(1.0);
                    let dv = away * (speed * falloff).min(FRIENDLY_SHOVE_MAX);
                    p.vel_t += dv;
                    let pop = if p.grounded && !ps.dead { FRIENDLY_POP } else { 0.0 };
                    if pop > 0.0 {
                        p.vel_r = p.vel_r.max(pop);
                        p.grounded = false;
                    }
                    cooldown.insert(e, FRIENDLY_SHOVE_CD);
                    telemetry.shoves += 1;
                    fx.write(CoopFxMsg { fx: CoopFx::Shove { target: pid.0, vel: dv, pop }, from_wire: false });
                }
                ForceKind::Chill(secs) => {
                    if ps.dead {
                        continue;
                    }
                    if ps.chill <= 0.0 {
                        telemetry.chills += 1;
                    }
                    ps.chill = ps.chill.max(secs);
                }
                ForceKind::Jolt => {
                    if ps.dead || !p.grounded || cooldown.contains_key(&e) {
                        continue;
                    }
                    p.vel_r = p.vel_r.max(FRIENDLY_JOLT_POP);
                    p.grounded = false;
                    cooldown.insert(e, FRIENDLY_SHOVE_CD);
                    telemetry.jolts += 1;
                    fx.write(CoopFxMsg { fx: CoopFx::Shove { target: pid.0, vel: Vec3::ZERO, pop: FRIENDLY_JOLT_POP }, from_wire: false });
                }
            }
        }
    }
}

// ─── drop-in ──────────────────────────────────────────────────────────────────

/// HOST: seating a peer now — is it a drop-in, and at what level? At the start of a run
/// the squad drops together at level 1; later (§11 onboarding), a joiner lands at half the
/// squad's average level, so it neither walks in at level 1 into a late horde nor skips
/// the build the others earned.
pub fn drop_in_level(run: &RunState, squad_levels: &[u32]) -> Option<u32> {
    if run.stage == 0 && run.total_elapsed < DROPIN_MIN_ELAPSED {
        return None;
    }
    if squad_levels.is_empty() {
        return None;
    }
    let avg = squad_levels.iter().sum::<u32>() as f32 / squad_levels.len() as f32;
    Some(((avg / 2.0).round() as u32).max(1))
}

/// HOST: the sheet a drop-in is seated with — at its level (the host's placeholder until
/// the joiner's own build arrives; the joiner deals ITS level-up cards itself), full HP,
/// and the autopilot grace.
pub fn drop_in_sheet(ps: &mut PlayerState, level: u32, save: &MetaSave) {
    ps.level = level.max(1);
    ps.xp = 0.0;
    ps.xp_needed = crate::run::xp_needed(ps.level);
    ps.recompute_stats(save, 0);
    ps.hp = ps.stats.max_hp;
    ps.grace = DROPIN_GRACE_SECS;
    ps.drop_level = ps.level;
}

/// A drop-in's body on its way down from orbit. Inserted by the seat; the first frame
/// lifts the body to DROPIN_HEIGHT (so the spawn code stays the one path), the landing ends
/// it with a shove to the crowd and the `CoopFx::DropIn` every machine announces.
#[derive(Component, Default)]
pub struct OrbitalDrop {
    lifted: bool,
}

/// HOST: lift each new drop-in into orbit, and land it: the crowd around the crater is
/// knocked back (no damage), teammates are booped, and every machine hears about it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn orbital_drops(
    mut commands: Commands,
    hash: Res<SpatialHash>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(Entity, &PlayerId, &mut Player, &PlayerState, &Transform, &mut OrbitalDrop)>,
    mut crowd: Query<&mut Enemy, (Without<Boss>, Without<Pot>)>,
    mut forces: MessageWriter<FriendlyForce>,
    mut fx: MessageWriter<CoopFxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
) {
    for (e, pid, mut p, ps, tf, mut drop) in &mut q {
        if !drop.lifted {
            drop.lifted = true;
            p.height = DROPIN_HEIGHT;
            p.vel_r = 0.0;
            p.grounded = false;
            telemetry.dropins += 1;
            continue;
        }
        if !p.grounded {
            continue;
        }
        commands.entity(e).remove::<OrbitalDrop>();
        telemetry.dropin_landings += 1;
        for (te, _) in hash.near(tf.translation, DROPIN_LAND_RADIUS + 1.5) {
            let Ok(mut en) = crowd.get_mut(te) else { continue };
            let arc = sphere::arc_dist(en.dir, p.dir, planet.radius);
            if arc > DROPIN_LAND_RADIUS {
                continue;
            }
            let away = (en.dir - p.dir * en.dir.dot(p.dir)).try_normalize().unwrap_or_else(|| sphere::tangent_frame(p.dir).0);
            en.knock += away * DROPIN_LAND_KNOCK * (1.0 - arc / DROPIN_LAND_RADIUS).max(0.3);
        }
        forces.write(FriendlyForce { from: e, at: tf.translation, radius: DROPIN_LAND_RADIUS, cone: None, kind: ForceKind::Shove(FRIENDLY_BLAST_SHOVE) });
        let level = ps.drop_level.clamp(1, u8::MAX as u32) as u8;
        fx.write(CoopFxMsg { fx: CoopFx::DropIn { owner: pid.0, level, dir: p.dir }, from_wire: false });
        info!("COOP player {} dropped in at level {}", pid.0, level);
    }
}

/// CLIENT: we dropped in (the host's copy of us carries the level it seated us at): level
/// our own sheet up to it — the cards are ours to pick, one panel at a time — and, if the
/// host's copy is still falling from orbit, fall with it. Once per run.
pub fn adopt_drop_in(
    mine: Res<crate::net::MyPlayerId>,
    run: Res<RunState>,
    server: Query<(&PlayerId, &PlayerVitals, &crate::net::NetTransform), Without<Player>>,
    mut q: Query<(&mut Player, &mut PlayerState), With<LocalPlayer>>,
    mut done: Local<Option<u64>>,
) {
    let Some(my_id) = mine.0 else { return };
    if *done == Some(run.run_seed) {
        return;
    }
    let Some((_, v, nt)) = server.iter().find(|(pid, ..)| pid.0 == my_id) else { return };
    let Ok((mut p, mut ps)) = q.single_mut() else { return };
    *done = Some(run.run_seed);
    if v.drop_level == 0 {
        return; // there from the start
    }
    let target = v.drop_level as u32;
    while ps.level < target {
        let need = (ps.xp_needed - ps.xp).max(0.0);
        ps.xp += need;
        ps.gain_xp(0.0);
    }
    ps.drop_level = target;
    if nt.height > 2.0 {
        p.height = nt.height;
        p.vel_r = 0.0;
        p.grounded = false;
    }
    info!("COOP dropped in: level {} ({} cards to pick)", ps.level, ps.pending_levelups);
}

/// Where the autopilot wants to go: away from the crowd within AUTOPILOT_SENSE (weighted
/// by closeness), round in a slow circle so a ring of foes is kited instead of stalled in,
/// and toward the nearest teammate when it has drifted further than AUTOPILOT_REGROUP.
/// Pure, so the host (for a joiner behind a panel or away from the keys) and the joiner's
/// own machine steer identically.
pub fn autopilot_wish(dir: Vec3, pos: Vec3, threats: &[Vec3], mates: &[Vec3], t: f32, planet_radius: f32) -> Vec3 {
    let flat = |v: Vec3| v - dir * v.dot(dir);
    let mut push = Vec3::ZERO;
    for tp in threats {
        let d = pos - *tp;
        let len = d.length();
        if len < 1e-3 || len > AUTOPILOT_SENSE {
            continue;
        }
        push += flat(d).normalize_or_zero() * (1.0 - len / AUTOPILOT_SENSE).powi(2) * 3.0;
    }
    let (tan, bit) = sphere::tangent_frame(dir);
    let a = t * 0.35;
    let mut wish = push + (tan * a.cos() + bit * a.sin()) * 0.45;
    if let Some(mate) = mates
        .iter()
        .copied()
        .min_by(|x, y| sphere::arc_dist(dir, *x, planet_radius).total_cmp(&sphere::arc_dist(dir, *y, planet_radius)))
    {
        if sphere::arc_dist(dir, mate, planet_radius) > AUTOPILOT_REGROUP {
            wish += flat(mate).normalize_or_zero() * 1.2;
        }
    }
    flat(wish).normalize_or_zero()
}

/// The threats an autopilot at `pos` should kite: live crowd within AUTOPILOT_SENSE (pots
/// are scenery).
fn threats_near(pos: Vec3, hash: &SpatialHash, enemies: &Query<&Enemy>) -> Vec<Vec3> {
    hash.near(pos, AUTOPILOT_SENSE)
        .filter(|(e, p)| enemies.get(*e).is_ok_and(|en| en.speed > 0.0) && p.distance_squared(pos) < AUTOPILOT_SENSE * AUTOPILOT_SENSE)
        .map(|(_, p)| p)
        .collect()
}

/// HOST: a drop-in whose player is idle (away from the keys, or picking the cards it
/// dropped in with) fights on autopilot for its grace. Idle = no wish of its own — the
/// last wish the autopilot wrote does not count as the player's.
#[allow(clippy::type_complexity)]
pub fn autopilot_peers(
    time: Res<Time>,
    hash: Res<SpatialHash>,
    planet: Res<CurrentPlanet>,
    enemies: Query<&Enemy>,
    mates: Query<(&Player, &PlayerState)>,
    mut q: Query<(Entity, &Player, &PlayerState, &mut InputIntent, &Transform), Without<LocalPlayer>>,
    mut telemetry: ResMut<CoopTelemetry>,
    mut steered: Local<HashMap<Entity, Vec3>>,
) {
    let t = time.elapsed_secs();
    for (e, p, ps, mut intent, tf) in &mut q {
        let ours = steered.get(&e).copied();
        let idle = intent.wish == Vec3::ZERO || ours == Some(intent.wish);
        if ps.grace <= 0.0 || ps.dead || !idle {
            // hand the controls back: a wish the autopilot left behind is not the player's
            if ours.is_some() && ours == Some(intent.wish) {
                intent.wish = Vec3::ZERO;
            }
            steered.remove(&e);
            continue;
        }
        let others: Vec<Vec3> = mates.iter().filter(|(m, s)| !s.dead && m.dir != p.dir).map(|(m, _)| m.dir).collect();
        let wish = autopilot_wish(p.dir, tf.translation, &threats_near(tf.translation, &hash, &enemies), &others, t, planet.radius);
        intent.wish = wish;
        if intent.forward == Vec3::ZERO {
            intent.forward = wish;
        }
        steered.insert(e, wish);
        telemetry.autopilot_secs += time.delta_secs();
    }
}

/// CLIENT: our own half of the autopilot — while the grace lasts and we are idle, the
/// autopilot's wish is our input (so our prediction runs it, and the host gets it as the
/// input it already knows how to apply). Behind a card panel our input goes up neutral,
/// and the host's `autopilot_peers` keeps our body fighting.
#[allow(clippy::type_complexity)]
pub fn autopilot_local(
    time: Res<Time>,
    hash: Res<SpatialHash>,
    planet: Res<CurrentPlanet>,
    enemies: Query<&Enemy>,
    mates: Query<&Transform, (With<RemoteAstronaut>, Without<LocalPlayer>)>,
    mut q: Query<(&Player, &PlayerState, &mut InputIntent, &Transform), With<LocalPlayer>>,
) {
    let Ok((p, ps, mut intent, tf)) = q.single_mut() else { return };
    let idle = intent.wish == Vec3::ZERO && !intent.jump && !intent.slide && !intent.blink;
    if ps.grace <= 0.0 || ps.dead || !idle {
        return;
    }
    let others: Vec<Vec3> = mates.iter().map(|m| m.translation.normalize_or_zero()).collect();
    intent.wish = autopilot_wish(p.dir, tf.translation, &threats_near(tf.translation, &hash, &enemies), &others, time.elapsed_secs(), planet.radius);
}

// ─── presentation of the one-shots ────────────────────────────────────────────

/// EVERY machine: present the co-op one-shots — bursts, banners, the joiner's own shove,
/// and STATIC CASCADE's belt (`duos::spawn_cascade_belt`). Runs behind a card panel too, like
/// the item and tech one-shots: a shove the host applied meanwhile must still reach the
/// joiner's predicted body.
#[allow(clippy::too_many_arguments)]
pub fn coop_fx_presentation(
    mut commands: Commands,
    mut msgs: MessageReader<CoopFxMsg>,
    planet: Res<CurrentPlanet>,
    assets: Res<CoopAssets>,
    particles: Option<Res<ParticleAssets>>,
    (save, mut flash, mut gate, time): (Res<MetaSave>, ResMut<fx::ScreenFlash>, ResMut<fx::FlashGate>, Res<Time<Real>>),
    mut local: Query<(&PlayerId, &mut Player), With<LocalPlayer>>,
    mine: Res<crate::net::MyPlayerId>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
) {
    // our PlayerId: a joiner's comes from the host (its body is PlayerId(0) locally)
    let me = mine.0.or_else(|| local.single().ok().map(|(pid, _)| pid.0));
    for m in msgs.read() {
        telemetry.fx_seen += u32::from(m.from_wire);
        let burst = |commands: &mut Commands, dir: Vec3, c: Pcolor, n: usize, speed: f32| {
            if let Some(pa) = &particles {
                fx::burst(commands, pa, planet.surface_point(dir) + dir * 0.8, dir, c, n, speed);
            }
        };
        match m.fx {
            CoopFx::Revived { rescuer, downed, dir } => {
                burst(&mut commands, dir, Pcolor::Green, 30, 9.0);
                let line = if me == Some(downed) {
                    format!("BACK ON YOUR FEET! P{} HAULED YOU UP", rescuer + 1)
                } else if me == Some(rescuer) {
                    "HERO'S ADRENALINE! +20% SPEED".to_string()
                } else {
                    format!("P{} REVIVED P{}", rescuer + 1, downed + 1)
                };
                banners.write(BannerMsg(line));
                if m.from_wire {
                    sfx.write(SfxMsg(Sfx::Shrine));
                }
            }
            CoopFx::Shove { target, vel, pop } => {
                // the host moved its copy of us: move our prediction the same way
                if m.from_wire && me == Some(target) {
                    if let Ok((_, mut p)) = local.single_mut() {
                        p.vel_t += vel;
                        if pop > 0.0 {
                            p.vel_r = p.vel_r.max(pop);
                            p.grounded = false;
                        }
                    }
                }
            }
            CoopFx::Cascade { a, b, axis } => {
                crate::duos::spawn_cascade_belt(&mut commands, &assets, &planet, axis, save.accessibility.photosensitive);
                if !save.accessibility.photosensitive || gate.allow(time.elapsed_secs()) {
                    flash.fire(Color::srgb(0.7, 0.85, 1.0), &save);
                }
                banners.write(BannerMsg(format!("STATIC CASCADE! P{} + P{} WRAPPED THE PLANET", a + 1, b + 1)));
                sfx.write(SfxMsg(Sfx::Evolve));
            }
            CoopFx::Duo { feat, a, b, dir, first } => {
                let c = match feat {
                    CoopFeat::DeepFreeze => Pcolor::Cyan,
                    CoopFeat::MagnetCircus => Pcolor::Gold,
                    _ => Pcolor::White,
                };
                burst(&mut commands, dir, c, 18, 8.0);
                if first {
                    banners.write(BannerMsg(format!("DUO: {}  (P{} + P{})", feat.def().name, a + 1, b + 1)));
                    sfx.write(SfxMsg(Sfx::Comet));
                }
            }
            CoopFx::DropIn { owner, level, dir } => {
                burst(&mut commands, dir, Pcolor::White, 40, 12.0);
                banners.write(BannerMsg(if me == Some(owner) {
                    format!("YOU DROPPED IN AT LV {level}: AUTOPILOT FIGHTS WHILE YOU'RE IDLE")
                } else {
                    format!("P{} DROPS IN FROM ORBIT (LV {level})", owner + 1)
                }));
                sfx.write(SfxMsg(Sfx::Teleport));
            }
        }
    }
}

/// `--netlog`: once a second, what the co-op rules have done (both sides; a joiner's
/// `fx_seen` counts one-shots that crossed the wire).
pub fn log_coop(time: Res<Time>, telemetry: Res<CoopTelemetry>, run: Res<RunState>, mut next: Local<f32>) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 1.0;
    info!(
        "COOP downs={} revives={} claims={} rejoins={} shoves={} chills={} jolts={} dropins={} landed={} autopilot={:.1}s cascades={} cascade_hits={} feats={:?} fx_seen={} cascade_charge={:.2} squad_feats={}",
        telemetry.downs, telemetry.revives, telemetry.claims, telemetry.rejoins, telemetry.shoves, telemetry.chills,
        telemetry.jolts, telemetry.dropins, telemetry.dropin_landings, telemetry.autopilot_secs, telemetry.cascades,
        telemetry.cascade_hits, telemetry.feats, telemetry.fx_seen, run.cascade_charge, run.feats.len()
    );
}
