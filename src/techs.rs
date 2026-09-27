//! The §4 movement techs Megabonk can't reach — the Orbital Slingshot's Slam, Grind-Lines
//! and Antipode Blink — and the slide's §4 table rows (slope-boost, plowing through the
//! horde).
//!
//! Movement itself stays in `player::player_input` / `player_physics`: one copy of the
//! rules, run on every body a machine moves (the host: all; a client: its own, predicted).
//! They keep the techs' state on each astronaut's `MoveTech`; the rails they ride are the
//! stage's `GrindLines`, traced from the terrain (`sphere::Terrain::ridge_spines`) the same
//! way on every machine. What a tech DOES to the world is here, HOST-simulated for every
//! astronaut: the Slam's shockwave, the slide's plow, the blink (and Boomerang Insurance's,
//! which routes through it — §15), the antipode read behind the HUD tell.
//!
//! CO-OP: a joiner predicts its own slides, grinds and slam dives from its own input (the
//! host runs the same physics on its copy from the same `PlayerInputMsg`); a blink is the
//! host's alone — the client snaps when its copy jumps (`net::reconcile_own_astronaut`) and
//! turns its momentum on the `TechFx::Blink` event. What others must SEE rides existing
//! lanes: grinding/tuck/flashlight on `NetTransform`, the blink charge and antipode read on
//! `NetItemVis`, Slam and Blink one-shots on the hazard event lane (as `TechFxMsg`, turned
//! back into the same local message on a client, so `tech_fx_presentation` is one path).

use crate::config::*;
use crate::content::items::ItemKind;
use crate::enemies::{Boss, Enemy, SpatialHash};
use crate::fx::{self, ParticleAssets, Pcolor, Shake};
use crate::interact::Pot;
use crate::items::ItemProcs;
use crate::messages::*;
use crate::net::{NetItemVis, NetTransform};
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{InputIntent, LocalPlayer, Player, PlayerId};
use crate::remote::RemoteAstronaut;
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use std::collections::HashMap;

// ─── per-astronaut state ─────────────────────────────────────────────────────

/// One astronaut's movement-tech state. Per STAGE, like `items::ItemProcs` (every body
/// `spawn_player` builds gets a fresh one); on every body a machine moves.
#[derive(Component, Default)]
pub struct MoveTech {
    /// Riding a Grind-Line.
    pub grind: Option<Grind>,
    /// Seconds before a rail may catch us again (after jumping or running off one).
    pub grind_cd: f32,
    /// Slide was pressed in the air: holding it SLAM_HOLD_SECS commits the Slam.
    pub slam_armed: bool,
    pub slam_hold: f32,
    /// Diving: the horizontal speed banked when the Slam committed.
    pub slam: Option<f32>,
    /// Landed a Slam, with its banked speed — `slam_shockwave` (host) takes and detonates
    /// it the same frame. (A joiner's predicted landing is never taken: the host's is the
    /// one that counts, and comes back as a `TechFx::Slam`.)
    pub slam_landed: Option<f32>,
    // Counted for the probes (and P21's "movement mastery" quest ladder to read).
    pub slams: u32,
    pub slam_hits: u32,
    pub grinds: u32,
    pub grind_m: f32,
    pub blinks: u32,
}

impl MoveTech {
    /// A teleport (a blink, a Dead Man's Tether rewind, a joiner's snap to the host's copy)
    /// leaves any rail and any Slam behind: otherwise the next `ride_rail` pulls the body
    /// straight back onto the rail it rode — undoing the rewind — and a dive that was in
    /// the air where it left detonates where it lands.
    pub fn cancel_moves(&mut self) {
        self.grind = None;
        self.grind_cd = GRIND_RECATCH_SECS;
        self.slam = None;
        self.slam_armed = false;
    }
}

/// Where on which rail an astronaut rides, which way, how fast.
#[derive(Clone, Copy, Debug)]
pub struct Grind {
    pub spine: usize,
    /// Arc metres along the spine.
    pub s: f32,
    /// +1 rides toward the spine's end, -1 toward its start.
    pub sign: f32,
    pub speed: f32,
}

// ─── Grind-Lines ─────────────────────────────────────────────────────────────

/// One rail: unit surface directions GRIND_STEP apart along a crest, and the running arc
/// length at each.
pub struct Spine {
    pub pts: Vec<Vec3>,
    pub at: Vec<f32>,
}

impl Spine {
    pub fn length(&self) -> f32 {
        self.at.last().copied().unwrap_or(0.0)
    }
}

/// Cell size of the rail lookup, metres — wider than any catch, so a 3×3×3 block of cells
/// around a body holds every segment it could catch.
const GRIND_CELL: f32 = 4.0;

/// The stage's Grind-Lines, rebuilt by `planet::spawn_stage` with the world.
#[derive(Resource, Default)]
pub struct GrindLines {
    pub spines: Vec<Spine>,
    radius: f32,
    /// segment lookup: cell of either end -> (spine, segment)
    cells: HashMap<IVec3, Vec<(u32, u32)>>,
}

impl GrindLines {
    pub fn new(lines: Vec<Vec<Vec3>>, radius: f32) -> Self {
        let mut out = GrindLines { spines: Vec::new(), radius, cells: HashMap::new() };
        for pts in lines.into_iter().filter(|p| p.len() >= 2) {
            let mut at = vec![0.0];
            for w in pts.windows(2) {
                at.push(at.last().unwrap() + sphere::arc_dist(w[0], w[1], radius));
            }
            let si = out.spines.len() as u32;
            for (i, w) in pts.windows(2).enumerate() {
                for end in [w[0], w[1]] {
                    let cell = out.cell(end);
                    let v = out.cells.entry(cell).or_default();
                    if !v.contains(&(si, i as u32)) {
                        v.push((si, i as u32));
                    }
                }
            }
            out.spines.push(Spine { pts, at });
        }
        out
    }

    fn cell(&self, dir: Vec3) -> IVec3 {
        (dir * self.radius / GRIND_CELL).floor().as_ivec3()
    }

    /// Every rail segment near `dir`, as (spine, segment).
    fn near(&self, dir: Vec3) -> impl Iterator<Item = (usize, usize)> + '_ {
        let c = self.cell(dir);
        (-1..=1).flat_map(move |x| {
            (-1..=1).flat_map(move |y| {
                (-1..=1).flat_map(move |z| {
                    self.cells.get(&(c + IVec3::new(x, y, z))).into_iter().flatten().map(|&(s, i)| (s as usize, i as usize))
                })
            })
        })
    }

    /// The closest point of any rail to `dir`: (arc metres off it, spine, arc position,
    /// the rail's run there).
    pub fn closest(&self, dir: Vec3) -> Option<(f32, usize, f32, Vec3)> {
        let mut best: Option<(f32, usize, f32, Vec3)> = None;
        for (si, i) in self.near(dir) {
            let sp = &self.spines[si];
            let (a, b) = (sp.pts[i], sp.pts[i + 1]);
            let ab = b - a;
            let t = ((dir - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
            let q = (a + ab * t).normalize();
            // the chord, not `arc_dist`: at a few centimetres the angle is ~1e-4 rad and
            // acos(dot) quantizes it to multiples of 5 cm in f32; the chord is exact here
            let arc = (q - dir).length() * self.radius;
            if best.is_none_or(|b| arc < b.0) {
                let run = (ab - q * ab.dot(q)).normalize_or_zero();
                best = Some((arc, si, sp.at[i] + (sp.at[i + 1] - sp.at[i]) * t, run));
            }
        }
        best
    }

    /// Can a body at `dir`, heading `heading` (unit tangent), catch a rail? Returns the rail,
    /// where on it, and which way along it the heading runs.
    pub fn catch(&self, dir: Vec3, heading: Vec3) -> Option<(usize, f32, f32)> {
        let (arc, si, s, run) = self.closest(dir)?;
        let along = heading.dot(run);
        if arc > GRIND_CATCH_ARC || along.abs() < GRIND_ALIGN_MIN {
            return None;
        }
        Some((si, s, along.signum()))
    }

    /// The point `s` arc metres along rail `spine` (clamped to it), and the rail's run there.
    pub fn sample(&self, spine: usize, s: f32) -> (Vec3, Vec3) {
        let sp = &self.spines[spine];
        let s = s.clamp(0.0, sp.length());
        let i = sp.at.partition_point(|x| *x <= s).saturating_sub(1).min(sp.pts.len() - 2);
        let f = (s - sp.at[i]) / (sp.at[i + 1] - sp.at[i]).max(1e-6);
        let (a, b) = (sp.pts[i], sp.pts[i + 1]);
        let d = a.lerp(b, f).normalize();
        let ab = b - a;
        (d, (ab - d * ab.dot(d)).normalize_or_zero())
    }

    pub fn total_length(&self) -> f32 {
        self.spines.iter().map(|s| s.length()).sum()
    }
}

/// Everything the physics step needs to ride one rail for one frame. Returns false when the
/// ride ended this frame (ran off the rail's end) — the body then carries the rail's
/// momentum into normal physics.
pub fn ride_rail(p: &mut Player, g: &mut Grind, lines: &GrindLines, move_mult: f32, dt: f32) -> bool {
    let target = PLAYER_RUN_SPEED * move_mult * GRIND_SPEED_MULT;
    g.speed = if g.speed < target {
        (g.speed + GRIND_ACCEL * dt).min(target)
    } else {
        (g.speed - GRIND_OVERSPEED_DECAY * dt).max(target)
    };
    let len = lines.spines[g.spine].length();
    let s = g.s + g.sign * g.speed * dt;
    let (dir, run) = lines.sample(g.spine, s);
    p.dir = dir;
    p.vel_t = run * g.sign * g.speed;
    p.facing = run * g.sign;
    p.height = GRIND_RAIL_LIFT;
    p.vel_r = 0.0;
    p.grounded = true;
    p.jumps_used = 0;
    p.coyote = 0.12;
    // A grind wears the slide's tuck and keeps the slide's hard cap (so `player_input`
    // never clamps a rail's speed back to a jog on the frame you jump off it).
    p.slide_timer = p.slide_timer.max(0.1);
    g.s = s.clamp(0.0, len);
    s > 0.0 && s < len
}

// ─── the Slam ────────────────────────────────────────────────────────────────

/// A Slam's shockwave from its banked speed (`power`, in the slammer's own run speeds —
/// `slam_power`): how much of the full bomb it is (0 = a dud — "whiff the ramp, whiff the
/// bomb" — up to 1 at redline) and how far it reaches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlamReach {
    pub t: f32,
    pub radius: f32,
}

pub fn slam_reach(power: f32) -> SlamReach {
    let t = ((power - SLAM_MIN_POWER) / (SLAM_FULL_POWER - SLAM_MIN_POWER)).clamp(0.0, 1.0);
    SlamReach { t, radius: SLAM_RADIUS_MIN + (SLAM_RADIUS_MAX - SLAM_RADIUS_MIN) * t }
}

/// A banked speed (m/s) in units of the slammer's own run speed. Measured against their
/// OWN run so the ramp is never optional: a speed build's flat slide is still a flat slide
/// (its redline is faster, and `slam_speed_scale` pays that out instead).
pub fn slam_power(bank: f32, move_mult: f32) -> f32 {
    bank / (PLAYER_RUN_SPEED * move_mult.max(0.05))
}

/// Speed builds' bonus on a Slam's damage: their redline is faster in m/s.
pub fn slam_speed_scale(move_mult: f32) -> f32 {
    move_mult.clamp(1.0, SLAM_SPEED_SCALE_MAX)
}

// ─── Antipode Blink ──────────────────────────────────────────────────────────

/// Antipode Blink's recharge for a stack of graded power `power` (0 = the item isn't held:
/// Boomerang Insurance alone recharges at the base).
pub fn blink_cooldown_for(power: f32) -> f32 {
    if power <= 0.0 {
        BLINK_COOLDOWN
    } else {
        (BLINK_COOLDOWN / power).max(BLINK_MIN_COOLDOWN)
    }
}

pub fn blink_cooldown(ps: &PlayerState) -> f32 {
    blink_cooldown_for(ps.item_power(ItemKind::AntipodeBlink))
}

/// What a blink did: where from, where to (the exact antipode), and the axis the move turned
/// the astronaut about — every machine turns momentum and camera the same way with it.
#[derive(Clone, Copy, Debug)]
pub struct BlinkJump {
    pub from: Vec3,
    pub to: Vec3,
    pub axis: Vec3,
}

/// The axis a blink from `dir` turns about: square to where the astronaut is LOOKING
/// (`forward`, the camera's forward on its owner's screen), so it goes "forward over the
/// top" — the same great circle the camera glides along (`player::glide_point`), and the
/// run you were on is still a run away from the camera when you land.
pub fn blink_axis(dir: Vec3, forward: Vec3, facing: Vec3) -> Vec3 {
    let tangent = |v: Vec3| (v - dir * v.dot(dir)).try_normalize();
    let f = tangent(forward).or_else(|| tangent(facing)).unwrap_or_else(|| sphere::tangent_frame(dir).0);
    dir.cross(f).normalize()
}

/// THE antipode mechanic (§4; §15: every antipode effect routes through it — a keyed
/// Antipode Blink and Boomerang Insurance's escape alike). Puts the astronaut on the exact
/// opposite point of the planet, momentum and facing turned half a lap about `blink_axis`
/// so a slide or bhop chain carries straight on over there, with BLINK_IFRAMES of grace.
/// Leaves any rail, cancels a Slam, and is not a descent (Downhill Momentum).
pub fn blink_body(p: &mut Player, ps: &mut PlayerState, tech: &mut MoveTech, procs: &mut ItemProcs, forward: Vec3) -> BlinkJump {
    let from = p.dir;
    let axis = blink_axis(from, forward, p.facing);
    let half = Quat::from_axis_angle(axis, std::f32::consts::PI);
    // exactly -from: a half turn about an axis square to it
    p.dir = -from;
    p.vel_t = half * p.vel_t;
    p.facing = (half * p.facing).normalize_or_zero();
    if p.facing == Vec3::ZERO {
        p.facing = sphere::tangent_frame(p.dir).0;
    }
    tech.cancel_moves();
    tech.blinks += 1;
    procs.forget_altitude();
    ps.iframes = ps.iframes.max(BLINK_IFRAMES);
    BlinkJump { from, to: p.dir, axis }
}

/// The §4 read on the far side, in bands the HUD dial names (colorblind-safe: a word, not a
/// colour alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntipodeBand {
    Clear,
    Thin,
    Crowded,
    Wall,
}

impl AntipodeBand {
    pub fn of(count: u32) -> Self {
        if count >= ANTIPODE_WALL {
            AntipodeBand::Wall
        } else if count >= ANTIPODE_CROWDED {
            AntipodeBand::Crowded
        } else if count >= ANTIPODE_THIN {
            AntipodeBand::Thin
        } else {
            AntipodeBand::Clear
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            AntipodeBand::Clear => "CLEAR",
            AntipodeBand::Thin => "THIN",
            AntipodeBand::Crowded => "CROWDED",
            AntipodeBand::Wall => "WALL",
        }
    }
}

// ─── presentation messages ───────────────────────────────────────────────────

/// A movement-tech one-shot worth showing. The host writes these; `netenemy` forwards them
/// on the hazard event lane and a client writes the same message back with `from_wire`
/// set, so `tech_fx_presentation` is one system on every machine.
#[derive(Clone, Copy, Debug)]
pub enum TechFx {
    /// `owner` (a PlayerId) landed a Slam at `dir` with `power` of their own run speeds
    /// banked (`slam_power`).
    Slam { owner: u8, dir: Vec3, power: f32 },
    /// `owner` blinked; `insured` = Boomerang Insurance paid out.
    Blink { owner: u8, from: Vec3, to: Vec3, axis: Vec3, insured: bool },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct TechFxMsg {
    pub fx: TechFx,
    /// Rebuilt from the hazard lane on a client.
    pub from_wire: bool,
}

/// What the techs did this run — for the headless `--techs` probe and nothing else.
#[derive(Resource, Default, Debug)]
pub struct TechTelemetry {
    pub slams: u32,
    pub slam_duds: u32,
    pub slam_hits: u32,
    pub best_slam_power: f32,
    pub blinks: u32,
    pub insured_blinks: u32,
    /// Crowd enemies shoved aside by a slide or a grind.
    pub plowed: u32,
    pub scans: u32,
}

// ─── HOST simulation ─────────────────────────────────────────────────────────

/// HOST: a keyed Antipode Blink, for every astronaut holding the item whose blink is
/// charged. A press while recharging does nothing (the owner's dial says why).
#[allow(clippy::type_complexity)]
pub fn antipode_blink(
    mut q: Query<(&PlayerId, &mut Player, &mut PlayerState, &mut ItemProcs, &mut MoveTech, &InputIntent)>,
    mut fx: MessageWriter<TechFxMsg>,
    mut telemetry: ResMut<TechTelemetry>,
) {
    for (pid, mut p, mut ps, mut procs, mut tech, intent) in &mut q {
        if !intent.blink || ps.dead || procs.blink_cd > 0.0 || !ps.has_item(ItemKind::AntipodeBlink) {
            continue;
        }
        procs.blink_cd = blink_cooldown(&ps);
        let jump = blink_body(&mut p, &mut ps, &mut tech, &mut procs, intent.forward);
        telemetry.blinks += 1;
        fx.write(TechFxMsg {
            fx: TechFx::Blink { owner: pid.0, from: jump.from, to: jump.to, axis: jump.axis, insured: false },
            from_wire: false,
        });
    }
}

/// HOST: the antipode read (§4 "the off-screen threat ring shows antipode density") for
/// every astronaut carrying a blink — Antipode Blink or Boomerang Insurance. A recount every
/// ANTIPODE_SCAN_SECS is ONE pass over the horde for all carriers together (the far pole is
/// 300+ m away, beyond any spatial-hash neighbourhood); a joiner reads its own in
/// `NetItemVis`, since its enemy stream never covers the far side.
pub fn antipode_scan(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    enemies: Query<(&Enemy, Has<Boss>)>,
    mut q: Query<(Entity, &Player, &PlayerState, &mut ItemProcs)>,
    mut telemetry: ResMut<TechTelemetry>,
) {
    let dt = time.delta_secs();
    let mut due: Vec<(Entity, Vec3)> = Vec::new();
    for (e, p, ps, mut procs) in &mut q {
        let carries = ps.has_item(ItemKind::AntipodeBlink) || ps.has_item(ItemKind::BoomerangInsurance);
        if !carries {
            if procs.antipode != 0 || procs.antipode_static || procs.antipode_boss {
                procs.antipode = 0;
                procs.antipode_static = false;
                procs.antipode_boss = false;
            }
            continue;
        }
        procs.antipode_scan -= dt;
        if procs.antipode_scan <= 0.0 {
            procs.antipode_scan = ANTIPODE_SCAN_SECS;
            due.push((e, -p.dir));
        }
    }
    if due.is_empty() {
        return;
    }
    let near = (ANTIPODE_SCAN_ARC / planet.radius).cos();
    let mut counts = vec![(0u16, false, false); due.len()];
    for (en, boss) in &enemies {
        // pots share Enemy (speed 0): scenery, not a crowd
        if en.speed <= 0.0 {
            continue;
        }
        for (k, (_, anti)) in due.iter().enumerate() {
            if en.dir.dot(*anti) >= near {
                let c = &mut counts[k];
                c.0 = c.0.saturating_add(1);
                c.1 |= en.kind == crate::content::enemies::EnemyKind::Ghost;
                c.2 |= boss;
            }
        }
    }
    for ((e, _), (n, stat, boss)) in due.into_iter().zip(counts) {
        if let Ok((_, _, _, mut procs)) = q.get_mut(e) {
            procs.antipode = n;
            procs.antipode_static = stat;
            procs.antipode_boss = boss;
        }
    }
    telemetry.scans += 1;
}

/// HOST: detonate every Slam that landed this frame — a radial shockwave around the crater,
/// damage and knock scaling with the speed the dive banked (`slam_reach`). Runs after the
/// physics step that landed it.
#[allow(clippy::type_complexity)]
pub fn slam_shockwave(
    hash: Res<SpatialHash>,
    planet: Res<CurrentPlanet>,
    enemies: Query<&Enemy>,
    mut q: Query<(Entity, &PlayerId, &Player, &PlayerState, &mut MoveTech, &Transform)>,
    mut hits: MessageWriter<HitMsg>,
    mut fx: MessageWriter<TechFxMsg>,
    mut telemetry: ResMut<TechTelemetry>,
) {
    let mut rng = rand::thread_rng();
    for (e, pid, p, ps, mut tech, tf) in &mut q {
        // taken, so a frame the physics skipped (hitstop, dt 0) can't detonate it twice
        let Some(bank) = tech.slam_landed.take() else { continue };
        let power = slam_power(bank, ps.move_speed_mult());
        let reach = slam_reach(power);
        let scale = slam_speed_scale(ps.move_speed_mult());
        telemetry.slams += 1;
        telemetry.best_slam_power = telemetry.best_slam_power.max(power);
        fx.write(TechFxMsg { fx: TechFx::Slam { owner: pid.0, dir: p.dir, power }, from_wire: false });
        if reach.t <= 0.0 {
            telemetry.slam_duds += 1;
            continue;
        }
        let damage = SLAM_DAMAGE * reach.t * scale * ps.damage_mult();
        for (te, _) in hash.near(tf.translation, reach.radius + 1.5) {
            let Ok(en) = enemies.get(te) else { continue };
            let arc = sphere::arc_dist(en.dir, p.dir, planet.radius);
            if arc > reach.radius + en.scale * 0.4 {
                continue;
            }
            // the rim takes 60% of the centre: stand ON them for the full bomb
            let falloff = 1.0 - 0.4 * (arc / reach.radius).min(1.0);
            let away = en.dir - p.dir * en.dir.dot(p.dir);
            let away = away.try_normalize().unwrap_or_else(|| sphere::tangent_frame(p.dir).0);
            let (cm, crit) = crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng);
            let elite = if en.elite { ps.stats.elite_damage } else { 1.0 };
            hits.write(HitMsg {
                source: Some(e),
                target: te,
                amount: damage * falloff * cm * elite,
                crit,
                knock: away * SLAM_KNOCK * reach.t * falloff,
                weapon: None,
            });
            tech.slam_hits += 1;
            telemetry.slam_hits += 1;
        }
    }
}

/// HOST: slide knockback (§4 "knocks back enemies you plow through") — a sliding or
/// grinding astronaut shoves the crowd it meets aside and along, carving the "breathing-
/// hole in a wall of Shamblers". Physics only (no damage), crowd only (bosses and pots hold
/// their ground), and the astronaut never slows for it. A spatial-hash query per body.
pub fn slide_plow(
    hash: Res<SpatialHash>,
    planet: Res<CurrentPlanet>,
    bodies: Query<(&Player, &PlayerState, &MoveTech, &Transform)>,
    mut crowd: Query<&mut Enemy, (Without<Boss>, Without<Pot>)>,
    mut telemetry: ResMut<TechTelemetry>,
) {
    for (p, ps, tech, tf) in &bodies {
        let plowing = (p.slide_timer > 0.0 && p.grounded) || tech.grind.is_some();
        let speed = p.vel_t.length();
        if !plowing || ps.dead || speed < PLAYER_RUN_SPEED * 0.8 {
            continue;
        }
        let fwd = p.vel_t / speed;
        for (te, _) in hash.near(tf.translation, SLIDE_PLOW_REACH + 1.5) {
            let Ok(mut en) = crowd.get_mut(te) else { continue };
            if en.speed <= 0.0 {
                continue;
            }
            let arc = sphere::arc_dist(en.dir, p.dir, planet.radius);
            if arc > SLIDE_PLOW_REACH + en.scale * 0.5 {
                continue;
            }
            // aside from the plow's line (which side they already stand on) and along it
            let off = en.dir - p.dir * en.dir.dot(p.dir);
            let side = off - fwd * off.dot(fwd);
            let side = side.try_normalize().unwrap_or_else(|| fwd.cross(p.dir).normalize_or_zero());
            let push = (side * 0.8 + fwd * 0.6).normalize_or_zero();
            // top the shove up to the plow's speed rather than stacking it every frame
            let have = en.knock.dot(push);
            if have < SLIDE_PLOW_KNOCK {
                if have < SLIDE_PLOW_KNOCK * 0.5 {
                    telemetry.plowed += 1;
                }
                en.knock += push * (SLIDE_PLOW_KNOCK - have);
            }
        }
    }
}

/// Edge-triggered intents (jump, slide, interact, blink) live ONE frame. The local body's
/// are rewritten every frame from the keyboard, but a joiner's are OR-ed in on the host as
/// its packets arrive (`net::apply_remote_input`) and nothing cleared them: one press kept
/// firing, so a joiner who jumped once bounced forever on the host (and a blink would have
/// re-fired on every recharge). Runs after everything that reads an edge.
pub fn consume_edge_intents(mut q: Query<&mut InputIntent>) {
    for mut i in &mut q {
        if i.jump || i.slide || i.interact || i.blink {
            i.jump = false;
            i.slide = false;
            i.interact = false;
            i.blink = false;
        }
    }
}

// ─── presentation (every machine) ────────────────────────────────────────────

#[derive(Resource)]
pub struct TechAssets {
    pub wave_mesh: Handle<Mesh>,
    pub wave_mat: Handle<StandardMaterial>,
    pub dud_mat: Handle<StandardMaterial>,
    pub flare_mesh: Handle<Mesh>,
    pub flare_mat: Handle<StandardMaterial>,
}

/// The Grind-Line colour: cyan, the game's "movement toy" colour (the bhop sparks, the
/// hover pad) — never red, which §12 keeps for danger.
pub const RAIL_COLOR: Color = Color::srgb(0.55, 1.0, 1.0);

pub fn setup_tech_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let unlit = |c: Color, alpha: f32| StandardMaterial {
        base_color: c.with_alpha(alpha),
        emissive: c.to_linear() * 2.0,
        unlit: true,
        alpha_mode: if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    };
    commands.insert_resource(TechAssets {
        // a flat ring of unit radius, scaled out to the shockwave's reach
        wave_mesh: meshes.add(Mesh::from(Torus::new(0.86, 1.0))),
        // YOUR bomb: warm white-gold, not the danger red of an enemy slam's telegraph
        wave_mat: materials.add(unlit(Color::srgb(1.0, 0.9, 0.55), 0.85)),
        dud_mat: materials.add(unlit(Color::srgb(0.7, 0.7, 0.72), 0.5)),
        flare_mesh: meshes.add(Mesh::from(Cylinder::new(0.35, 1.0))),
        flare_mat: materials.add(unlit(Color::srgb(0.5, 0.95, 1.0), 0.7)),
    });
}

/// A Slam's ring, growing out to its reach and thinning away.
#[derive(Component)]
pub struct SlamWave {
    pub dir: Vec3,
    pub radius: f32,
    pub age: f32,
}

/// A blink's column of light where someone left or arrived — tall enough to read from the
/// far side of a teammate's horizon, gone in half a second.
#[derive(Component)]
pub struct BlinkFlare {
    pub age: f32,
}

const BLINK_FLARE_SECS: f32 = 0.6;
const BLINK_FLARE_HEIGHT: f32 = 14.0;

/// Every machine: turn tech one-shots into something visible. The host's own bodies were
/// already moved by the systems that wrote the message; a joiner's predicted body is
/// snapped and turned here when the host blinked it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tech_fx_presentation(
    mut commands: Commands,
    mut msgs: MessageReader<TechFxMsg>,
    planet: Res<CurrentPlanet>,
    assets: Res<TechAssets>,
    particles: Option<Res<ParticleAssets>>,
    mut local: Query<(&PlayerId, &mut Player, &mut MoveTech, Option<&mut ItemProcs>), With<LocalPlayer>>,
    mut shake: ResMut<Shake>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let local_pid = local.single().ok().map(|(pid, _, _, _)| pid.0);
    for m in msgs.read() {
        match m.fx {
            TechFx::Slam { owner, dir, power } => {
                let reach = slam_reach(power);
                let dud = reach.t <= 0.0;
                let radius = if dud { SLAM_RADIUS_MIN * 0.6 } else { reach.radius };
                commands.spawn((
                    SlamWave { dir, radius, age: 0.0 },
                    Mesh3d(assets.wave_mesh.clone()),
                    MeshMaterial3d(if dud { assets.dud_mat.clone() } else { assets.wave_mat.clone() }),
                    slam_wave_pose(&planet, dir, radius, 0.0),
                    StageScoped,
                ));
                if let Some(pa) = &particles {
                    let at = planet.surface_point(dir) + dir * 0.3;
                    fx::burst(&mut commands, pa, at, dir, Pcolor::White, 10, 5.0);
                    if !dud {
                        fx::burst(&mut commands, pa, at, dir, Pcolor::Gold, (10.0 + 20.0 * reach.t) as usize, 7.0 + 5.0 * reach.t);
                    }
                }
                if local_pid == Some(owner) {
                    shake.add(SLAM_SHAKE_MIN + (SLAM_SHAKE_MAX - SLAM_SHAKE_MIN) * reach.t);
                    sfx.write(SfxMsg(Sfx::Slam));
                }
            }
            TechFx::Blink { owner, from, to, axis, insured } => {
                // A column where they left and where they arrived, for everyone else. Not on
                // the owner's own screen: their camera glides from one spot to the other, and
                // a pillar of light there would hide exactly the crowd they came to read.
                let columns: &[Vec3] = if local_pid == Some(owner) { &[] } else { &[from, to] };
                for &d in columns {
                    commands.spawn((
                        BlinkFlare { age: 0.0 },
                        Mesh3d(assets.flare_mesh.clone()),
                        MeshMaterial3d(assets.flare_mat.clone()),
                        blink_flare_pose(&planet, d, 0.0),
                        StageScoped,
                    ));
                }
                if let Some(pa) = &particles {
                    for d in [from, to] {
                        fx::burst(&mut commands, pa, planet.surface_point(d) + d, d, Pcolor::Cyan, 18, 8.0);
                    }
                }
                if local_pid != Some(owner) {
                    continue;
                }
                sfx.write(SfxMsg(Sfx::Blink));
                if insured {
                    banners.write(BannerMsg(crate::items::DeathSave::AntipodeEscape.banner().into()));
                }
                // A joiner's body is predicted here, and the host just moved its copy: land
                // on the host's spot with our own momentum turned the host's way (the
                // reconcile snap may already have moved us — turning is still owed).
                if m.from_wire {
                    if let Ok((_, mut p, mut tech, procs)) = local.single_mut() {
                        let half = Quat::from_axis_angle(axis, std::f32::consts::PI);
                        p.dir = to;
                        p.vel_t = half * p.vel_t;
                        p.facing = (half * p.facing).try_normalize().unwrap_or_else(|| sphere::tangent_frame(to).0);
                        tech.grind = None;
                        tech.grind_cd = GRIND_RECATCH_SECS;
                        tech.slam = None;
                        tech.slam_armed = false;
                        tech.blinks += 1;
                        if let Some(mut procs) = procs {
                            procs.forget_altitude();
                        }
                    }
                }
            }
        }
    }
}

fn slam_wave_pose(planet: &CurrentPlanet, dir: Vec3, radius: f32, k: f32) -> Transform {
    let grow = 1.0 - (1.0 - k).powi(3); // fast out, settling
    let r = (radius * grow).max(0.2);
    Transform::from_translation(planet.surface_point(dir) + dir * TELEGRAPH_LIFT)
        .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
        .with_scale(Vec3::new(r, 0.6 * (1.0 - k) + 0.1, r))
}

fn blink_flare_pose(planet: &CurrentPlanet, dir: Vec3, k: f32) -> Transform {
    Transform::from_translation(planet.surface_point(dir) + dir * BLINK_FLARE_HEIGHT * 0.5)
        .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
        .with_scale(Vec3::new(1.0 - k, BLINK_FLARE_HEIGHT, 1.0 - k))
}

/// Every machine: grow each Slam ring out and retire it; pinch each blink flare away.
pub fn animate_tech_fx(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut waves: Query<(Entity, &mut SlamWave, &mut Transform), Without<BlinkFlare>>,
    mut flares: Query<(Entity, &mut BlinkFlare, &mut Transform), Without<SlamWave>>,
) {
    let dt = time.delta_secs();
    for (e, mut w, mut tf) in &mut waves {
        w.age += dt;
        let k = w.age / SLAM_RING_SECS;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        *tf = slam_wave_pose(&planet, w.dir, w.radius, k);
    }
    for (e, mut f, mut tf) in &mut flares {
        f.age += dt;
        let k = f.age / BLINK_FLARE_SECS;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let dir = tf.translation.normalize_or_zero();
        *tf = blink_flare_pose(&planet, dir, k);
    }
}

/// Every machine: rail sparks under every drawn body that is grinding — our own and the
/// host's bodies from their `MoveTech`, a joiner's teammates from `NetTransform`.
#[allow(clippy::type_complexity)]
pub fn grind_sparks(
    mut commands: Commands,
    time: Res<Time>,
    particles: Option<Res<ParticleAssets>>,
    bodies: Query<(&Transform, Option<&MoveTech>, Option<&NetTransform>), Or<(With<Player>, With<RemoteAstronaut>)>>,
    mut acc: Local<f32>,
) {
    let Some(pa) = particles else { return };
    *acc += time.delta_secs();
    if *acc < 0.06 {
        return;
    }
    *acc = 0.0;
    for (tf, tech, net) in &bodies {
        let grinding = match (tech, net) {
            (Some(t), _) => t.grind.is_some(),
            (None, Some(n)) => n.grinding,
            _ => false,
        };
        if grinding {
            let up = tf.translation.normalize_or_zero();
            let feet = tf.translation - up * (PLAYER_HEIGHT * 0.5 - GRIND_RAIL_LIFT);
            fx::burst(&mut commands, &pa, feet, up, Pcolor::Gold, 3, 3.5);
        }
    }
}

/// The local player's first brush with a Grind-Line each run gets one line of how to use
/// it — the rails are the only §4 tech a player can't find by pressing buttons.
pub fn grind_hint(
    run: Res<RunState>,
    lines: Res<GrindLines>,
    q: Query<(&Player, &MoveTech), With<LocalPlayer>>,
    mut banners: MessageWriter<BannerMsg>,
    mut shown_for: Local<Option<u64>>,
) {
    if *shown_for == Some(run.run_seed) {
        return;
    }
    let Ok((p, tech)) = q.single() else { return };
    if tech.grinds > 0 {
        *shown_for = Some(run.run_seed);
        return;
    }
    if lines.closest(p.dir).is_some_and(|(arc, ..)| arc < GRIND_HINT_ARC) {
        banners.write(BannerMsg("RIDGE SPINE: SLIDE ONTO IT TO GRIND".into()));
        *shown_for = Some(run.run_seed);
    }
}

/// A beep for a blink key pressed while the blink recharges (or with nothing to blink
/// with) — the dial says how long; this says "not yet" where your eyes are.
///
/// The charge it reads is `NetItemVis` (the host's truth on every machine), which on the
/// host is written by `items::push_net_item_vis` in ANOTHER chain — so on the frame a
/// press blinks, it may or may not already show the new recharge. A press that blinked
/// this frame (`antipode_blink` runs first; the body's blink count moved) is never "not
/// yet". On a joiner the count moves only when the host's blink comes back, so a press
/// the host will refuse still beeps.
pub fn blink_denied_feedback(
    q: Query<(&InputIntent, &PlayerState, &NetItemVis, &MoveTech), With<LocalPlayer>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut seen: Local<u32>,
) {
    let Ok((intent, ps, vis, tech)) = q.single() else { return };
    let blinked = tech.blinks != *seen;
    *seen = tech.blinks;
    if intent.blink && !blinked && ps.has_item(ItemKind::AntipodeBlink) && vis.blink_cd > 0 {
        sfx.write(SfxMsg(Sfx::Click));
    }
}

/// `--netlog`: what of the techs this machine has seen, every 5 s — on a joiner the proof
/// the hazard lane carries them ("TECHFX[Client] … wire=N") and that its own body rides
/// rails and keeps its light.
#[allow(clippy::type_complexity)]
pub fn log_tech_fx(
    time: Res<Time>,
    role: Res<crate::net::NetRole>,
    lines: Option<Res<GrindLines>>,
    mut msgs: MessageReader<TechFxMsg>,
    mine: Query<(&MoveTech, &InputIntent, &NetItemVis), With<LocalPlayer>>,
    remotes: Query<&NetTransform, With<RemoteAstronaut>>,
    mut tally: Local<([u32; 3], u32)>,
    mut next: Local<f32>,
) {
    for m in msgs.read() {
        match m.fx {
            TechFx::Slam { .. } => tally.0[0] += 1,
            TechFx::Blink { insured, .. } => tally.0[if insured { 2 } else { 1 }] += 1,
        }
        tally.1 += u32::from(m.from_wire);
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 5.0;
    let own = mine.single().ok();
    info!(
        "TECHFX[{:?}] slams={} blinks={} insured={} wire={} | rails={} ({:.0} m) | me: grinds={} grind_m={:.0} slams={} blinks={} light={} blink_cd={} antipode={} | teammates grinding={} lit={}",
        *role,
        tally.0[0],
        tally.0[1],
        tally.0[2],
        tally.1,
        lines.as_ref().map_or(0, |l| l.spines.len()),
        lines.as_ref().map_or(0.0, |l| l.total_length()),
        own.map_or(0, |o| o.0.grinds),
        own.map_or(0.0, |o| o.0.grind_m),
        own.map_or(0, |o| o.0.slams),
        own.map_or(0, |o| o.0.blinks),
        own.is_some_and(|o| o.1.light),
        own.map_or(0, |o| o.2.blink_cd),
        own.map_or(0, |o| o.2.antipode),
        remotes.iter().filter(|n| n.grinding).count(),
        remotes.iter().filter(|n| n.light).count(),
    );
}

// ─── dev harness ─────────────────────────────────────────────────────────────

/// `--dev --techbot`: a windowed / two-instance harness that works the §4 techs through the
/// REAL input path (it writes the local `InputIntent`, so on a joiner every press goes up
/// the wire exactly as keys would): every 8 s it hops and slams with a held slide, blinks
/// (with `--items antipodeblink`), then heads for the nearest Grind-Line and slides onto
/// it. Runs after the keyboard gather and `--botinput`, before the intent is sent/used.
pub fn dev_tech_bot(
    time: Res<Time>,
    lines: Res<GrindLines>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(&Player, &MoveTech, &mut InputIntent), With<LocalPlayer>>,
    mut clock: Local<f32>,
) {
    let Ok((p, tech, mut intent)) = q.single_mut() else { return };
    let before = *clock;
    *clock = (*clock + time.delta_secs()) % 8.0;
    // did the clock pass `t` this frame (a slow frame can wrap it past the cycle's end)?
    let now = *clock;
    let crossed = |t: f32| if now >= before { before < t && t <= now } else { t > before || t <= now };
    // 0.5: hop; 0.75-1.4: hold slide in the air -> Slam
    if crossed(0.5) {
        intent.jump = true;
    }
    if crossed(0.75) {
        intent.slide = true;
    }
    if (0.75..1.4).contains(&now) {
        intent.slide_held = true;
    }
    if crossed(3.0) {
        intent.blink = true;
    }
    // 4.0-7.8: make for the nearest rail and slide onto it along its run
    if (4.0..7.8).contains(&now) && tech.grind.is_none() {
        match lines.closest(p.dir) {
            Some((arc, _, _, run)) if arc <= 2.5 => {
                intent.wish = if p.vel_t.dot(run) >= 0.0 { run } else { -run };
                if p.grounded && p.slide_timer <= 0.0 {
                    intent.slide = true;
                }
            }
            _ => {
                // the nearest rail point anywhere (a dev harness can afford the scan)
                let nearest = lines.spines.iter().flat_map(|s| s.pts.iter()).max_by(|a, b| a.dot(p.dir).total_cmp(&b.dot(p.dir)));
                if let Some(target) = nearest.filter(|t| sphere::arc_dist(**t, p.dir, planet.radius) < 40.0) {
                    intent.wish = (*target - p.dir * target.dot(p.dir)).normalize_or_zero();
                }
            }
        }
    }
}

// ─── rules self-check ────────────────────────────────────────────────────────

/// Headless self-check of the §4 techs on synthetic state: the blink's geometry (exact
/// antipode, momentum turned with the camera's glide), the Slam's scale, air control's
/// no-180 and no-free-speed rules, the blink recharge, and a Grind-Line network on every
/// world whose rails sit on crests and can be caught where they run. Returns the first
/// violated rule.
pub fn self_check() -> Result<(), String> {
    use crate::content::planets::PlanetKind;
    // Blink: exact antipode; momentum and facing turned about the view axis, still tangent;
    // and the camera's glide ends where the body landed, looking the way the run now runs.
    for (i, dir) in sphere::fib_sphere(23).enumerate() {
        let (t, b) = sphere::tangent_frame(dir);
        let a = i as f32 * 0.7;
        let fwd = t * a.cos() + b * a.sin();
        let mut p = test_body(dir, fwd * 12.0);
        let mut ps = PlayerState::new(crate::content::characters::AstronautKind::Buzz, &crate::save::MetaSave::default());
        let jump = blink_body(&mut p, &mut ps, &mut MoveTech::default(), &mut ItemProcs::default(), fwd);
        if p.dir.dot(-dir) < 1.0 - 1e-6 || jump.to != p.dir {
            return Err(format!("a blink from {dir:?} landed at {:?}, not the antipode", p.dir));
        }
        if p.vel_t.dot(p.dir).abs() > 1e-3 || (p.vel_t.length() - 12.0).abs() > 1e-3 {
            return Err("a blink bent momentum off the ground or changed its speed".into());
        }
        // the glide's path over the top meets the landing, and its tangent there is the run
        let axis = crate::player::glide_axis(dir, -dir, fwd);
        let end = crate::player::glide_point(dir, -dir, 1.0, axis);
        let mid = crate::player::glide_point(dir, -dir, 0.5, axis);
        if end.normalize().dot(p.dir) < 1.0 - 1e-4 || mid.normalize().dot(fwd) < 0.999 {
            return Err("the camera's glide to the antipode does not go forward over the top".into());
        }
        let late = crate::player::glide_point(dir, -dir, 0.99, axis).normalize();
        let arrive = (p.dir - late).normalize();
        if arrive.dot(p.vel_t.normalize()) < 0.99 {
            return Err("a blink's momentum does not run the way the camera arrives".into());
        }
    }
    // Recharge: the base at the native grade, divided by grade/stacks, floored.
    if blink_cooldown_for(1.0) != BLINK_COOLDOWN
        || blink_cooldown_for(0.0) != BLINK_COOLDOWN
        || blink_cooldown_for(2.0) >= BLINK_COOLDOWN
        || blink_cooldown_for(99.0) != BLINK_MIN_COOLDOWN
    {
        return Err("the blink recharge ladder is wrong".into());
    }
    // Slam: a flat-ground slide-jump is a dud ("whiff the ramp, whiff the bomb") — for a
    // speed build too, measured against its own run; a redline is the full bomb; more
    // banked speed is a wider one.
    let (dud, full) = (slam_reach(SLIDE_BOOST), slam_reach(SPEED_HARD_CAP));
    if dud.t != 0.0
        || (full.t - 1.0).abs() > 1e-4
        || (full.radius - SLAM_RADIUS_MAX).abs() > 1e-4
        || slam_reach(1.95).radius <= dud.radius
        || slam_reach(slam_power(PLAYER_RUN_SPEED * 1.5 * SLIDE_BOOST, 1.5)).t != 0.0
    {
        return Err(format!("slam scaling wrong: dud {dud:?} full {full:?}"));
    }
    // Air control (`player::steer`, the frames `player_input` runs): a run-speed hop steered
    // straight back must not reverse before landing, and one steered sideways must curve.
    // And no wish ever ADDS speed past a run: a hop held forward, a bhop chain landing
    // after landing held forward, a hop from standstill all come down at run speed — so a
    // Slam from any of them is a dud — while a slide's hop keeps its speed (§4 "preserve
    // momentum").
    let hang = 2.0 * PLAYER_JUMP_VEL / PLAYER_GRAVITY;
    let steps = 60;
    let dt = hang / steps as f32;
    let run = PLAYER_RUN_SPEED;
    let slide = run * SLIDE_BOOST;
    let (mut back, mut side, mut fwd, mut still) = (Vec3::X * run, Vec3::X * run, Vec3::X * run, Vec3::ZERO);
    let mut slid = Vec3::X * slide;
    for _ in 0..steps {
        back = crate::player::steer(back, -Vec3::X, false, run, dt);
        side = crate::player::steer(side, Vec3::Z, false, run, dt);
        fwd = crate::player::steer(fwd, Vec3::X, false, run, dt);
        still = crate::player::steer(still, Vec3::X, false, run, dt);
        slid = crate::player::steer(slid, (Vec3::X + Vec3::Z * 0.3).normalize(), false, slide, dt);
    }
    if back.x <= 0.0 || side.z < run * 0.5 {
        return Err(format!("air control: braked to {:.2} m/s (must stay > 0), curved to {:.2} m/s sideways", back.x, side.z));
    }
    // six bhops, each jumped 0.1 s into its landing window (grounded, under the hard cap)
    let mut chain = Vec3::X * run;
    for _ in 0..6 {
        for _ in 0..steps {
            chain = crate::player::steer(chain, Vec3::X, false, run, dt);
        }
        for _ in 0..6 {
            chain = crate::player::steer(chain, Vec3::X, true, run, 0.1 / 6.0);
        }
    }
    let worst = fwd.length().max(still.length()).max(chain.length()).max(side.length());
    if worst > run * 1.001 || slam_reach(slam_power(worst, 1.0)).t > 0.0 {
        return Err(format!("air control added speed: a plain hop came down at {worst:.2} m/s (run {run})"));
    }
    if (slid.length() - slide).abs() > 1e-3 {
        return Err(format!("a slide's hop came down at {:.2} m/s, not its {slide:.2}", slid.length()));
    }
    // Grind-Lines: every world has rails, on crests, catchable along their run, not across.
    for kind in PlanetKind::ALL {
        let planet = CurrentPlanet::from_kind(kind);
        let lines = GrindLines::new(planet.terrain.ridge_spines(planet.radius), planet.radius);
        let total = lines.total_length();
        if lines.spines.len() < 3 || total < 150.0 {
            return Err(format!("{:?} has {} rails ({total:.0} m) — too few to be a highway", kind, lines.spines.len()));
        }
        for (si, sp) in lines.spines.iter().enumerate() {
            if sp.pts.iter().any(|d| planet.terrain.ridge(*d) < GRIND_CREST_MIN * 0.9 || planet.terrain.in_crater(*d)) {
                return Err(format!("{kind:?} rail {si} leaves its crest"));
            }
            let s = sp.length() * 0.5;
            let (d, run) = lines.sample(si, s);
            let side = d.cross(run);
            match lines.catch(d, run) {
                Some((sj, sc, sign)) if sj == si && (sc - s).abs() < 0.5 && sign > 0.0 => {}
                other => return Err(format!("{kind:?} rail {si}: sliding along it at {s:.1} m caught {other:?}")),
            }
            if lines.catch(d, side).is_some_and(|(sj, ..)| sj == si) {
                return Err(format!("{kind:?} rail {si} caught a slide crossing it square"));
            }
        }
    }
    Ok(())
}

fn test_body(dir: Vec3, vel: Vec3) -> Player {
    Player {
        dir,
        height: 0.0,
        vel_t: vel,
        vel_r: 0.0,
        grounded: true,
        facing: vel.normalize_or_zero(),
        jumps_used: 0,
        slide_timer: 0.0,
        slide_cd: 0.0,
        land_timer: 10.0,
        coyote: 0.0,
        stride: 0.0,
        gait_amp: 0.0,
        squash: 1.0,
        squash_amt: 0.0,
        lean: 0.0,
    }
}
