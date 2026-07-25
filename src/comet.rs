//! The COMET COMBO — the signature scored lap-kill. Run with a horde strung out in your
//! wake and a charge builds (mass × speed). Fill it before the tail breaks and it cashes
//! out: a screen-clearing detonation, bonus Silver, and a "COMET ×N!" payoff. This is the
//! game's marketable money-shot — running around your own apocalypse, rewarded.

use crate::config::*;
use crate::enemies::{Enemy, SpatialHash};
use crate::fx::{self, Hitstop, ParticleAssets, Pcolor, Shake};
use crate::interact::Pot;
use crate::messages::*;
use crate::player::Player;
use crate::run::{PlayerState, RunState};
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct Comet {
    pub active: bool,
    pub count: u32,       // current tail size (for the HUD)
    pub peak: u32,        // peak tail this combo (drives the payout)
    pub charge: f32,      // progress toward cash-out
    pub break_timer: f32, // grace since the tail dipped below threshold
    pub flash: f32,       // brief HUD flash after a cash-out
    pub fires: u32,       // lifetime cash-outs this run (stats/tests)
}

impl Comet {
    pub fn progress(&self) -> f32 {
        (self.charge / COMET_CHARGE_GOAL).clamp(0.0, 1.0)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn comet_system(
    mut commands: Commands,
    time: Res<Time>,
    hash: Res<SpatialHash>,
    mut comet: ResMut<Comet>,
    mut run: ResMut<RunState>,
    mut shake: ResMut<Shake>,
    mut hitstop: ResMut<Hitstop>,
    particles: Option<Res<ParticleAssets>>,
    q_player: Query<(&Player, &PlayerState, &Transform), With<crate::player::LocalPlayer>>,
    enemies: Query<&Enemy, Without<Pot>>,
    mut hits: MessageWriter<HitMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    comet.flash = (comet.flash - dt).max(0.0);
    let prev_progress = comet.progress();
    let Ok((p, ps, ptf)) = q_player.single() else { return };
    let ppos = ptf.translation;
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

        // rising audio ticks at each quarter of the charge
        for q in [0.25, 0.5, 0.75] {
            if prev_progress < q && comet.progress() >= q {
                sfx.write(SfxMsg(Sfx::Coin));
            }
        }
        // the comet's glowing tail while the combo is alive
        if let Some(pa) = &particles {
            if (time.elapsed_secs() * 14.0).fract() < 14.0 * dt {
                fx::burst(&mut commands, pa, ppos - move_dir * 0.8, p.dir, Pcolor::Gold, 1, 1.6);
            }
        }

        if comet.charge >= COMET_CHARGE_GOAL {
            // ---- CASH OUT ----
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
                    });
                }
            }
            run.silver_run += peak as u64;
            shake.add(0.8);
            hitstop.timer = 0.22;
            banners.write(BannerMsg(format!("\u{2604} COMET x{peak}!")));
            sfx.write(SfxMsg(Sfx::Comet));
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, ppos, p.dir, Pcolor::Gold, 44, 13.0);
            }
            let fires = comet.fires + 1;
            *comet = Comet { flash: 0.8, fires, ..Default::default() };
        }
    } else if comet.active {
        comet.break_timer += dt;
        if comet.break_timer > COMET_GRACE {
            let fires = comet.fires;
            *comet = Comet { fires, ..Default::default() }; // combo dropped — no payout
        }
    }
}
