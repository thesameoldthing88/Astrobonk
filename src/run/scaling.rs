//! The §3 difficulty-scaling model, in ONE place.
//!
//! ```text
//! EnemyHP(t)     = HP_base   × (1 + 0.11·t)^1.35 × (1 + 0.20·d) × T × (1 + 0.06·Δ)
//! EnemyDMG(t)    = DMG_base  × (1 + 0.08·t)      × (1 + 0.15·d) × T × (1 + 0.05·Δ)
//! SpawnRate(t)   = Rate_base × (1 + 0.14·t)      × (1 + 0.10·d)       × (1 + 0.04·Δ)
//! EliteChance(t) = min(0.35, 0.02·t + 0.05·d + 0.03·Δ)
//! ```
//!
//! `HP_base` / `DMG_base` are each `EnemyDef`'s own numbers times the `SCALE_HP_BASE` /
//! `SCALE_DMG_BASE` anchors in config.rs (which say why the anchor sits where it does).
//!
//! Every system that sizes an enemy, a boss or the spawn budget reads a [`Scaling`] built
//! here instead of doing its own arithmetic. The §13 enemy-density assist multiplies in
//! here too (spawn rate and live cap); the enemy-damage assist is applied where a hit
//! lands (`combat::apply_player_hits`) so it also covers shots already in flight. Later
//! layers (Ascension Depth — P23, weekly mutators — P23, co-op per-enemy and boss HP — P18,
//! Farside's elite bump — P07) multiply into the fields of `Scaling` inside
//! [`Scaling::new`], never at call sites — as The Static Radio's "angrier Static" (§7)
//! already does.
//!
//! Pure functions of plain numbers so the headless self-check can pin the curve shapes.

use super::RunState;
use crate::config::*;

/// The four inputs of the §3 formulas plus the party size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScalingInputs {
    /// Run minutes across the WHOLE chain. The stage clock restarts at each teleporter, and
    /// a per-stage `t` would hand an evolved build a softer horde on the next world than the
    /// one it just left; `d` is the per-world step on top.
    pub t_min: f32,
    /// Chain depth: 0 on the first world of the chain.
    pub depth: f32,
    /// Planet multiplier `T` (`PlanetDef::threat`).
    pub planet: f32,
    /// Difficulty points `Δ` (see [`difficulty_points`]).
    pub delta: f32,
    /// Astronauts in the run (1–4).
    pub party: usize,
    /// Someone carries The Static Radio: The Static comes angrier (§7).
    pub static_radio: bool,
    /// The §13 enemy-density assist (1 = canon).
    pub density: f32,
}

impl ScalingInputs {
    pub fn from_run(run: &RunState, party: usize) -> Self {
        Self {
            t_min: run.total_elapsed / 60.0,
            depth: run.stage as f32,
            planet: run.planet().def().threat,
            delta: difficulty_points(run),
            party: party.max(1),
            static_radio: run.static_radio,
            density: run.assist.enemy_density,
        }
    }
}

/// `Δ`: the sheet's Difficulty stat in points (Cursed Moon Rock, Cursed Tome, Greed
/// shrines — already folded into `run.difficulty` as the party max) plus a step per Tier.
/// The Tier comes from the chain length rather than `run.tier` because the chain is what a
/// co-op client receives from the host.
pub fn difficulty_points(run: &RunState) -> f32 {
    let tier = run.chain.len().max(1) as f32;
    run.difficulty.max(0.0) * DIFFICULTY_POINTS_PER_UNIT + (tier - 1.0) * TIER_DIFFICULTY_POINTS
}

/// How many astronauts count toward the §11 party scaling: the ones still standing. A
/// downed or claimed teammate no longer keeps the horde at full party size, nor a boss that
/// spawns meanwhile at a squad's HP (KNOWN_ISSUES L17). Never below one.
pub fn living_party<'a>(sheets: impl Iterator<Item = &'a super::PlayerState>) -> usize {
    sheets.filter(|ps| !ps.dead).count().max(1)
}

/// Multipliers for one moment of the run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scaling {
    /// Crowd enemy HP multiplier.
    pub hp: f32,
    /// Crowd enemy damage multiplier.
    pub dmg: f32,
    /// Multiplier on `Rate_base` (includes party scaling).
    pub spawn: f32,
    /// Chance that one elite roll (every `ELITE_ROLL_SECS`) promotes the next spawn.
    pub elite_chance: f32,
    /// Boss/miniboss HP multiplier. Bosses arrive at fixed stage marks and their `BossDef`
    /// numbers are authored for that moment, so the run-time term is left out — only the
    /// depth, planet and Δ terms of the §3 formula apply.
    pub boss_hp: f32,
    /// Boss/miniboss contact-damage multiplier (same reasoning as `boss_hp`).
    pub boss_dmg: f32,
    /// Party multiplier alone.
    pub party_spawn: f32,
    /// The §11 per-enemy HP multiplier alone (already in `hp`; `boss_hp` has its own).
    pub party_hp: f32,
    /// On top of everything above, for The Static's ghosts only: HP, damage and spawn-rate
    /// multipliers (The Static Radio makes it "angrier", §7).
    pub static_hp: f32,
    pub static_dmg: f32,
    pub static_rate: f32,
    /// Live-enemy ceiling: ENEMY_CAP grown with the party, shrunk with the density assist.
    pub live_cap: usize,
}

impl Scaling {
    pub fn new(i: ScalingInputs) -> Self {
        let t = i.t_min.max(0.0);
        let d = i.depth.max(0.0);
        let delta = i.delta.max(0.0);
        let depth_hp = 1.0 + SCALE_HP_D * d;
        let depth_dmg = 1.0 + SCALE_DMG_D * d;
        let delta_hp = 1.0 + SCALE_HP_DELTA * delta;
        let delta_dmg = 1.0 + SCALE_DMG_DELTA * delta;
        let seat = i.party.clamp(1, PARTY_SPAWN_SCALE.len()) - 1;
        let party_spawn = PARTY_SPAWN_SCALE[seat];
        // §11: a fuller ring of slightly tougher foes; bosses grow sub-linearly
        let party_hp = PARTY_HP_SCALE[seat];
        let party_boss = PARTY_BOSS_HP_SCALE[seat];
        let density = i.density.clamp(ASSIST_DENSITY_MIN, 1.0);
        Self {
            hp: SCALE_HP_BASE * (1.0 + SCALE_HP_T * t).powf(SCALE_HP_EXP) * depth_hp * i.planet * delta_hp * party_hp,
            dmg: SCALE_DMG_BASE * (1.0 + SCALE_DMG_T * t) * depth_dmg * i.planet * delta_dmg,
            spawn: (1.0 + SCALE_RATE_T * t)
                * (1.0 + SCALE_RATE_D * d)
                * (1.0 + SCALE_RATE_DELTA * delta)
                * party_spawn
                * density,
            elite_chance: (ELITE_CHANCE_T * t + ELITE_CHANCE_D * d + ELITE_CHANCE_DELTA * delta)
                .clamp(0.0, ELITE_CHANCE_CAP),
            boss_hp: depth_hp * i.planet * delta_hp * party_boss,
            boss_dmg: depth_dmg * i.planet * delta_dmg,
            party_spawn,
            party_hp,
            static_hp: if i.static_radio { STATIC_RADIO_HP } else { 1.0 },
            static_dmg: if i.static_radio { STATIC_RADIO_DMG } else { 1.0 },
            static_rate: if i.static_radio { STATIC_RADIO_RATE } else { 1.0 },
            live_cap: (ENEMY_CAP as f32 * party_spawn * density) as usize,
        }
    }

    pub fn for_run(run: &RunState, party: usize) -> Self {
        Self::new(ScalingInputs::from_run(run, party))
    }
}

/// `Rate_base` in spawns per second: the §3 run-arc beats, interpolated on the stage
/// countdown, or The Static's ever-growing base once the clock has run out.
pub fn spawn_rate_base(timer_left: f32, static_active: bool, static_secs: f32) -> f32 {
    if static_active {
        return STATIC_RATE_BASE + STATIC_RATE_GROWTH * static_secs.max(0.0);
    }
    let beats = &SPAWN_RATE_BEATS;
    if timer_left >= beats[0].0 {
        return beats[0].1;
    }
    for w in beats.windows(2) {
        let ((t0, r0), (t1, r1)) = (w[0], w[1]);
        if timer_left >= t1 {
            let f = (t0 - timer_left) / (t0 - t1).max(1e-3);
            return r0 + (r1 - r0) * f;
        }
    }
    beats[beats.len() - 1].1
}

/// The "tension breathes" beat modifier on top of `Rate_base`: exhale after a boss falls,
/// hold while a miniboss is up, otherwise full inhale.
pub fn beat_modifier(miniboss_alive: bool, exhale_left: f32) -> f32 {
    if exhale_left > 0.0 {
        SPAWN_EXHALE_MULT
    } else if miniboss_alive {
        SPAWN_HOLD_MULT
    } else {
        1.0
    }
}

fn p_inputs(base: ScalingInputs) -> ScalingInputs {
    ScalingInputs { t_min: 6.0, depth: 1.0, ..base }
}

/// Headless self-check: the curve SHAPES the GDD asks for. Returns the first violated rule.
pub fn self_check() -> Result<(), String> {
    let base = ScalingInputs { t_min: 0.0, depth: 0.0, planet: 1.0, delta: 0.0, party: 1, static_radio: false, density: 1.0 };
    let s0 = Scaling::new(base);
    if (s0.hp - SCALE_HP_BASE).abs() > 1e-4
        || (s0.dmg - SCALE_DMG_BASE).abs() > 1e-4
        || (s0.spawn - 1.0).abs() > 1e-4
    {
        return Err(format!("scaling at t=0 must be the HP/DMG_base anchors, got {s0:?}"));
    }
    if s0.elite_chance != 0.0 {
        return Err("elite chance must start at 0".into());
    }
    // exact formula at a probe point: t=10, d=2, T=1.25, Δ=5
    let p = Scaling::new(ScalingInputs { t_min: 10.0, depth: 2.0, planet: 1.25, delta: 5.0, ..base });
    let want_hp = SCALE_HP_BASE * 2.1f32.powf(1.35) * 1.4 * 1.25 * 1.3;
    let want_dmg = SCALE_DMG_BASE * 1.8 * 1.3 * 1.25 * 1.25;
    let want_spawn = 2.4 * 1.2 * 1.2;
    let want_elite = (0.2f32 + 0.1 + 0.15).min(0.35);
    for (name, got, want) in [("hp", p.hp, want_hp), ("dmg", p.dmg, want_dmg), ("spawn", p.spawn, want_spawn), ("elite", p.elite_chance, want_elite)] {
        if (got - want).abs() > want * 1e-3 {
            return Err(format!("{name}: got {got}, want {want}"));
        }
    }
    // HP must be super-linear in t (evolutions mandatory), damage linear, elite capped.
    let hp_at = |t: f32| Scaling::new(ScalingInputs { t_min: t, ..base }).hp;
    if hp_at(20.0) - hp_at(10.0) <= hp_at(10.0) - hp_at(0.0) {
        return Err("enemy HP is not super-linear in run time".into());
    }
    let mut prev = Scaling::new(base);
    for step in 1..=120 {
        let s = Scaling::new(ScalingInputs { t_min: step as f32 * 0.5, ..base });
        if s.hp < prev.hp || s.dmg < prev.dmg || s.spawn < prev.spawn || s.elite_chance < prev.elite_chance {
            return Err(format!("scaling decreased at t={}", step as f32 * 0.5));
        }
        prev = s;
    }
    if Scaling::new(ScalingInputs { t_min: 500.0, depth: 9.0, delta: 99.0, ..base }).elite_chance > ELITE_CHANCE_CAP {
        return Err("elite chance exceeded its cap".into());
    }
    // The Static Radio angers The Static only — the living horde is untouched.
    let radio = Scaling::new(ScalingInputs { static_radio: true, ..base });
    if radio.static_hp <= 1.0 || radio.static_dmg <= 1.0 || radio.static_rate <= 1.0 || radio.hp != s0.hp || radio.spawn != s0.spawn {
        return Err("The Static Radio must anger The Static and nothing else".into());
    }
    // The §13 density assist thins the horde (rate and cap) and touches nothing else; it
    // can never raise the canon numbers.
    let half = Scaling::new(ScalingInputs { density: 0.5, ..p_inputs(base) });
    let full = Scaling::new(p_inputs(base));
    if (half.spawn - full.spawn * 0.5).abs() > 1e-4
        || half.live_cap != full.live_cap / 2
        || half.hp != full.hp
        || half.dmg != full.dmg
        || half.elite_chance != full.elite_chance
        || Scaling::new(ScalingInputs { density: 7.0, ..base }).spawn > s0.spawn
        || s0.live_cap != ENEMY_CAP
    {
        return Err("the density assist must scale spawn rate and live cap only".into());
    }
    // §11 party table: spawn 100/175/240/300 %, per-enemy HP 100/110/120/130 %, boss HP
    // 100/165/225/285 % — and nothing else moves with the party.
    for (n, (spawn, hp, boss)) in [(1.0, 1.0, 1.0), (1.75, 1.1, 1.65), (2.4, 1.2, 2.25), (3.0, 1.3, 2.85)].into_iter().enumerate() {
        let solo = Scaling::new(p_inputs(base));
        let squad = Scaling::new(ScalingInputs { party: n + 1, ..p_inputs(base) });
        let close = |a: f32, b: f32| (a - b).abs() <= b.abs() * 1e-4;
        if !close(squad.spawn, solo.spawn * spawn)
            || !close(squad.hp, solo.hp * hp)
            || !close(squad.boss_hp, solo.boss_hp * boss)
            || squad.dmg != solo.dmg
            || squad.elite_chance != solo.elite_chance
            || squad.live_cap != (ENEMY_CAP as f32 * spawn) as usize
        {
            return Err(format!("party of {}: {squad:?} does not follow the §11 table", n + 1));
        }
    }
    if Scaling::new(ScalingInputs { party: 9, ..base }).boss_hp != Scaling::new(ScalingInputs { party: 4, ..base }).boss_hp {
        return Err("a party past four must scale as four".into());
    }
    // Rate_base walks the arc beats and never dips outside the breathing modifiers.
    if (spawn_rate_base(600.0, false, 0.0) - SPAWN_RATE_BEATS[0].1).abs() > 1e-4
        || spawn_rate_base(300.0, false, 0.0) <= spawn_rate_base(500.0, false, 0.0)
        || spawn_rate_base(10.0, false, 0.0) < spawn_rate_base(300.0, false, 0.0)
        || spawn_rate_base(0.0, true, 120.0) <= spawn_rate_base(0.0, true, 0.0)
    {
        return Err("Rate_base does not follow the run arc".into());
    }
    Ok(())
}
