//! The COMET COMBO — the signature scored lap-kill. Run with a horde strung out in your
//! wake and a charge builds (mass × speed). Fill it before the tail breaks and it cashes
//! out: a screen-clearing detonation, bonus Silver, and a "COMET ×N!" payoff. This is the
//! game's marketable money-shot — running around your own apocalypse, rewarded.
//!
//! CO-OP: every astronaut runs their own combo. The HOST computes all of them — it owns the
//! horde the tail is counted from — on a per-astronaut `CometState`, detonates each
//! cash-out around whoever earned it, and mirrors a compact `net::NetComet` onto each
//! astronaut. Presentation (HUD meter, banner, audio ticks, shake, tail sparks) reads ONLY
//! `NetComet`, so the host's own HUD and a joiner's HUD run the exact same code.

use crate::config::*;
use crate::enemies::{Enemy, SpatialHash};
use crate::fx::{self, Hitstop, ParticleAssets, Pcolor, Shake};
use crate::interact::Pot;
use crate::messages::*;
use crate::net::{MyPlayerId, NetComet, NetRole};
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::remote::RemoteAstronaut;
use crate::run::{PlayerState, RunState};
use bevy::prelude::*;
use std::collections::HashMap;

/// HOST: one astronaut's live combo.
#[derive(Component, Default)]
pub struct CometState {
    pub active: bool,
    pub count: u32,       // current tail size
    pub peak: u32,        // peak tail this combo (drives the payout)
    pub charge: f32,      // progress toward cash-out
    pub break_timer: f32, // grace since the tail dipped below threshold
    pub fires: u32,       // lifetime cash-outs by this astronaut this stage
    pub last_peak: u32,   // the tail of the most recent cash-out
}

impl CometState {
    pub fn progress(&self) -> f32 {
        (self.charge / COMET_CHARGE_GOAL).clamp(0.0, 1.0)
    }
    fn to_net(&self) -> NetComet {
        NetComet {
            active: self.active,
            count: self.count.min(u16::MAX as u32) as u16,
            progress: (self.progress() * 255.0).round() as u8,
            fires: self.fires.min(u16::MAX as u32) as u16,
            peak: self.last_peak.min(u16::MAX as u32) as u16,
        }
    }
}

/// THIS machine's player's combo, as the HUD shows it. Filled by `comet_presentation`
/// from the local player's `NetComet` — on a client that is the host's copy of us.
#[derive(Resource, Default)]
pub struct Comet {
    pub active: bool,
    pub count: u32,
    pub progress: f32,
    pub flash: f32, // brief HUD flash after a cash-out
    pub fires: u32, // lifetime cash-outs this run (stats/tests)
}

/// HOST: advance every astronaut's combo and detonate the ones that fill.
pub fn comet_system(
    time: Res<Time>,
    hash: Res<SpatialHash>,
    mut run: ResMut<RunState>,
    mut q: Query<(&Player, &PlayerState, &Transform, &mut CometState, &mut NetComet)>,
    enemies: Query<&Enemy, Without<Pot>>,
    mut hits: MessageWriter<HitMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (p, ps, ptf, mut comet, mut net) in &mut q {
        if ps.dead {
            // a downed astronaut drags no tail; their combo is simply dropped
            let (fires, last_peak) = (comet.fires, comet.last_peak);
            *comet = CometState { fires, last_peak, ..Default::default() };
        } else {
            advance(&mut comet, p, ps, ptf.translation, dt, &hash, &enemies, &mut run, &mut hits);
        }
        let n = comet.to_net();
        if *net != n {
            *net = n; // only on change: replicon sends whatever was touched
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn advance(
    comet: &mut CometState,
    p: &Player,
    ps: &PlayerState,
    ppos: Vec3,
    dt: f32,
    hash: &SpatialHash,
    enemies: &Query<&Enemy, Without<Pot>>,
    run: &mut RunState,
    hits: &mut MessageWriter<HitMsg>,
) {
    let move_dir = p.vel_t.normalize_or_zero();
    let speed = p.vel_t.length();

    // Count the tail: living enemies close by AND behind your heading (not the ones
    // you're running into) — that's the comet's stringing-out shape.
    let mut count = 0u32;
    for (e, epos) in hash.near(ppos, WAKE_RADIUS) {
        let Ok(en) = enemies.get(e) else { continue };
        if en.speed <= 0.0 {
            continue; // pots / inert
        }
        let to = epos - ppos;
        let d = to.length();
        if d < WAKE_RADIUS && d > 0.15 && (move_dir == Vec3::ZERO || (to / d).dot(move_dir) < 0.35) {
            count += 1;
        }
    }
    comet.count = count;

    if count >= COMET_MIN_TAIL && speed > 2.0 {
        comet.active = true;
        comet.break_timer = 0.0;
        comet.peak = comet.peak.max(count);
        comet.charge += count as f32 * speed * dt;

        if comet.charge >= COMET_CHARGE_GOAL {
            // ---- CASH OUT — around the astronaut who earned it ----
            let peak = comet.peak;
            for (e, epos) in hash.near(ppos, COMET_RADIUS) {
                if enemies.get(e).map(|en| en.speed > 0.0).unwrap_or(false) {
                    let kdir = (epos - ppos).normalize_or_zero();
                    hits.write(HitMsg {
                        source: None, // world event, not an astronaut
                        target: e,
                        amount: 300.0 + peak as f32 * 18.0,
                        crit: true,
                        knock: kdir * 12.0 * ps.stats.knockback,
                        by: HitBy::Other,
                    });
                }
            }
            // Silver goes into the squad's shared run pot, whoever cashed out — the same
            // pot every Silver pickup feeds (pickups.rs), banked at the run's end by the
            // host. A joiner's own meta save is never written (a client must not bank a
            // run it did not simulate), so there is no per-player Silver to credit yet.
            run.silver_run += peak as u64;
            let fires = comet.fires + 1;
            *comet = CometState { fires, last_peak: peak, ..Default::default() };
        }
    } else if comet.active {
        comet.break_timer += dt;
        if comet.break_timer > COMET_GRACE {
            // combo dropped — no payout
            let (fires, last_peak) = (comet.fires, comet.last_peak);
            *comet = CometState { fires, last_peak, ..Default::default() };
        }
    }
}

/// Every machine: turn the replicated combos into what players see and hear.
///   * THIS player's combo drives the HUD meter, the quarter-charge ticks, and the
///     cash-out payoff (banner, sting, shake).
///   * EVERY astronaut's live combo trails gold sparks, and every cash-out bursts where it
///     happened — you watch a teammate's comet go off across the curve.
///
/// A cash-out is spotted as `fires` going up, which survives any number of lost updates.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn comet_presentation(
    mut commands: Commands,
    time: Res<Time>,
    role: Res<NetRole>,
    mine: Res<MyPlayerId>,
    mut comet: ResMut<Comet>,
    mut fx_res: (ResMut<Shake>, ResMut<Hitstop>),
    particles: Option<Res<ParticleAssets>>,
    q: Query<(
        Entity,
        &PlayerId,
        &NetComet,
        Option<&Transform>,
        Option<&Player>,
        Option<&RemoteAstronaut>,
        Has<LocalPlayer>,
    )>,
    q_local: Query<(&Player, &Transform), With<LocalPlayer>>,
    mut seen: Local<HashMap<Entity, u16>>,
    mut last_progress: Local<f32>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let (shake, hitstop) = &mut fx_res;
    let dt = time.delta_secs();
    comet.flash = (comet.flash - dt).max(0.0);
    let client = *role == NetRole::Client;
    let mut mine_found = false;
    let mut live: Vec<Entity> = Vec::new();

    for (e, pid, nc, tf, player, remote, is_local) in &q {
        // Where this astronaut is drawn on THIS machine, and which way is "behind" it.
        let (is_me, pos, up, back) = if !client {
            // host/solo: every astronaut is a simulated Player carrying its own NetComet
            let (Some(p), Some(tf)) = (player, tf) else { continue };
            (is_local, tf.translation, p.dir, -p.vel_t.normalize_or_zero())
        } else if player.is_some() {
            // our predicted body: its NetComet is never written on a client — the host's
            // copy of us (below) is the real one
            continue;
        } else if mine.0 == Some(pid.0) {
            let Ok((p, tf)) = q_local.single() else { continue };
            (true, tf.translation, p.dir, -p.vel_t.normalize_or_zero())
        } else if let (Some(r), Some(tf)) = (remote, tf) {
            (false, tf.translation, r.dir, -r.facing)
        } else {
            continue; // a teammate whose rig hasn't been built yet
        };
        live.push(e);

        let cashed = match seen.insert(e, nc.fires) {
            Some(prev) => nc.fires > prev,
            None => false, // first sighting (join, stage change): nothing to celebrate
        };

        if is_me {
            mine_found = true;
            let progress = nc.progress as f32 / 255.0;
            // rising audio ticks at each quarter of the charge
            if nc.active {
                for q in [0.25, 0.5, 0.75] {
                    if *last_progress < q && progress >= q {
                        sfx.write(SfxMsg(Sfx::Coin));
                    }
                }
            }
            *last_progress = progress;
            comet.active = nc.active;
            comet.count = nc.count as u32;
            comet.progress = progress;
            if cashed {
                // counted here, not copied: `NetComet.fires` restarts with each stage's
                // fresh astronaut, while this is the whole run's tally
                comet.fires += 1;
                comet.flash = 0.8;
                shake.add(0.8);
                // Hitstop slows the SIMULATION clock, so only a solo game takes it: on a
                // client it would stall our own prediction and leave us out of step with
                // the host, and on a host it would stall every teammate (`fx::Hitstop`).
                if fx::freezes(&role) {
                    hitstop.stop(0.22);
                }
                // ASCII only: the game font has no comet glyph (see update_comet_hud)
                banners.write(BannerMsg(format!("COMET x{}!", nc.peak)));
                sfx.write(SfxMsg(Sfx::Comet));
            }
        } else if cashed {
            banners.write(BannerMsg(format!("PLAYER {}: COMET x{}!", pid.0 + 1, nc.peak)));
        }

        let Some(pa) = &particles else { continue };
        if cashed {
            fx::burst(&mut commands, pa, pos, up, Pcolor::Gold, 44, 13.0);
        }
        // the comet's glowing tail while the combo is alive
        if nc.active && dt > 0.0 && (time.elapsed_secs() * 14.0).fract() < 14.0 * dt {
            fx::burst(&mut commands, pa, pos + back * 0.8, up, Pcolor::Gold, 1, 1.6);
        }
    }

    if !mine_found {
        comet.active = false;
        comet.count = 0;
        comet.progress = 0.0;
    }
    seen.retain(|e, _| live.contains(e));
}
