//! What the §7 tomes DO beyond their stat lines — the map, the one piece of presentation,
//! and the self-check.
//!
//! A tome is data (`content::tomes`): lines in the `Stats` vocabulary folded in by
//! `PlayerState::recompute_stats`. The plain stats (Damage, Health, Salvage's chest
//! discount, …) need nothing more. The rest act where the thing they change already lives:
//!
//! | Tome | Line | Effect site |
//! |---|---|---|
//! | Orbit | Orbit | `combat::weapon_fire` (drone ring/speed/size), `combat::fire_volley` (return weapons), `items::orbital_yoyo` |
//! | Encirclement | CrowdDamage | `items::encirclement_scan` counts → `PlayerState::crowd_bonus` |
//! | Nightfall | NightDamage, Flashlight | `player::player_physics` sets `night` (`planet::is_night`) → `night_bonus`; `apply_flashlights` |
//! | Gravity | FallSpeed (+JumpHeight) | `player::player_physics` |
//! | Swarm | ProcRate | `items::proc_period` |
//! | Salvage | ChestDiscount | chest and Shady Guy prices (`interact`); P16's Microwave price reads it too |
//! | Ricochet | Ricochet | rolled per shot in `combat::fire_volley`, skipped in `combat::projectile_move` |
//! | Momentum | MomentumDamage | `player::player_physics` times `momentum` → `momentum_bonus` |
//! | Vampirism | Lifesteal, VisorBoost | `recompute_stats` (Vampire Visor's share) |
//! | Elite | EliteLoot, EliteDamageTaken | `items::item_upkeep` → `RunState::elite_loot` → `pickups::elite_loot_mult`; `incoming_mult` (horde elites only, never a boss) |
//! | Banishment | ExtraBanishes/Refreshes, RefreshDiscount | `PlayerState::new`, `PlayerState::refresh_price` |
//! | Duplication | FreeMicrowave, DupeKeepGrade | `PlayerState::enter_stage`, the Microwave in `interact::interact_system` |
//! | Horizon | HorizonCollect | `pickups::pickup_update` → `Pickup::fly_home` (+ `PickupEvent::Horizon`/`HorizonSettle` for clients) |
//! | Static | StaticSilver, StaticDamageTaken | ghost coins tagged `pickups::StaticSilver` → `RunState::static_silver_found` and the overtime term, both in `director::silver_payout`; `incoming_mult` |
//! | Ascension | EvoSlots, EvoDamage | `PlayerState::new` → `evo_cap`; `combat::weapon_fire` |
//!
//! CO-OP: each machine folds ITS OWN save's tomes into its own sheet; a joiner's derived
//! `Stats` reach the host in `net::PlayerBuildMsg`, so every line the host simulates for a
//! peer (orbit, crowd, night, fall, procs, skips, momentum, lifesteal, elite and static
//! hits, horizon calls, evolved damage) is the peer's. The per-run allowances (banishes,
//! refreshes, evolution slots, the free Microwave) are spent on the owner's own screen —
//! though a joiner cannot work the Microwave, chests or the Shady Guy until P14's peer
//! interactables, so Duplication and Salvage pay only the host (and solo) until then.
//! What a client must SEE rides existing lanes: the horizon call (and its withdrawal) on the
//! pickup lane, the flashlight on `NetItemVis::lamp`.

use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::content::items::ItemKind;
use crate::content::tomes::TomeKind;
use crate::content::Rarity;
use crate::run::{PlayerState, RefreshPrice, RunState};
use crate::save::MetaSave;
use crate::stats::Stats;
use bevy::prelude::*;

/// Multiplier on a hit an astronaut takes from the tomes that make certain foes hit harder:
/// Tome of the Elite (from an elite) and Tome of Static (from The Static).
pub fn incoming_mult(stats: &Stats, by_elite: bool, by_static: bool) -> f32 {
    let mut m = 1.0;
    if by_elite {
        m *= stats.elite_damage_taken.max(0.0);
    }
    if by_static {
        m *= stats.static_damage_taken.max(0.0);
    }
    m
}

/// PRESENTATION (every machine): each astronaut's flashlight at its owner's Tome of
/// Nightfall strength — from the sheet where this machine has one (its own astronaut; every
/// astronaut on the host), else from the replicated `NetItemVis::lamp` (a teammate drawn on
/// a client). Touches a light only when its number moves.
pub fn apply_flashlights(
    sheets: Query<&PlayerState>,
    vis: Query<&crate::net::NetItemVis>,
    mut lights: Query<(&ChildOf, &mut SpotLight)>,
) {
    for (child_of, mut light) in &mut lights {
        let owner = child_of.parent();
        let k = match (sheets.get(owner), vis.get(owner)) {
            (Ok(ps), _) => ps.stats.flashlight,
            (Err(_), Ok(v)) => crate::net::lamp_from_code(v.lamp),
            _ => continue, // not an astronaut's lamp
        }
        .max(0.1);
        let intensity = FLASHLIGHT_INTENSITY * k;
        let range = FLASHLIGHT_RANGE * (1.0 + (k - 1.0) * 0.5);
        if (light.intensity - intensity).abs() > 1.0 || (light.range - range).abs() > 0.01 {
            light.intensity = intensity;
            light.range = range;
        }
    }
}

/// `--netlog`: every 5 s, the tome lines of every sheet this machine holds (on a host that
/// includes each joiner's, as its `PlayerBuildMsg` delivered them — the proof a peer's own
/// tomes reached the simulation), and the horizon flights drawn here (on a joiner: called and
/// withdrawn by the host over the pickup lane). The probe counts by watching components, so
/// the simulation systems carry no telemetry of their own.
#[allow(clippy::too_many_arguments)]
pub fn log_tomes(
    time: Res<Time>,
    role: Res<crate::net::NetRole>,
    sheets: Query<(&crate::player::PlayerId, &PlayerState)>,
    called: Query<(), Changed<crate::pickups::HorizonBound>>,
    mut withdrawn: RemovedComponents<crate::pickups::HorizonBound>,
    still: Query<(), With<crate::pickups::Pickup>>,
    flying: Query<(), With<crate::pickups::HorizonBound>>,
    shots: Query<&crate::combat::Projectile>,
    mut counts: Local<(u32, u32)>,
    mut next: Local<f32>,
) {
    counts.0 += called.iter().count() as u32;
    // a removal whose gem still exists is a withdrawn call (a collected one is despawned)
    counts.1 += withdrawn.read().filter(|e| still.contains(*e)).count() as u32;
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 5.0;
    let lines: Vec<String> = sheets
        .iter()
        .map(|(pid, ps)| {
            format!(
                "p{} orbit x{:.2} horizon {:.2}/s ricochet {:.0}% night {} +{:.0}% momentum +{:.0}% crowd {} lamp x{:.1} evo_cap {}",
                pid.0,
                ps.stats.orbit,
                ps.stats.horizon_collect,
                ps.stats.ricochet * 100.0,
                ps.night,
                ps.night_bonus() * 100.0,
                ps.momentum_bonus() * 100.0,
                ps.crowd,
                ps.stats.flashlight,
                ps.evo_cap()
            )
        })
        .collect();
    info!(
        "TOMES[{:?}] {} | horizon calls={} withdrawn={} in_flight={} | shots skipping now={}",
        *role,
        lines.join(" ; "),
        counts.0,
        counts.1,
        flying.iter().count(),
        shots.iter().filter(|p| p.hop > 0.0).count()
    );
}

/// `--tomes all|a,b,…` (with `--tome-rank N`, default the max): the tomes the headless probe
/// slots, by variant name.
pub fn tomes_from_args() -> (Vec<TomeKind>, u32) {
    let args: Vec<String> = std::env::args().collect();
    let Some(list) = args.iter().position(|a| a == "--tomes").and_then(|i| args.get(i + 1)) else {
        return (Vec::new(), 0);
    };
    let tomes = if list == "all" {
        TomeKind::ALL.to_vec()
    } else {
        list.split(',').filter_map(TomeKind::from_name).collect()
    };
    let rank = args
        .iter()
        .position(|a| a == "--tome-rank")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(TOME_MAX_RANK)
        .min(TOME_MAX_RANK);
    (tomes, rank)
}

/// A save with exactly `tomes` slotted at `rank` (room made for them all).
pub fn save_with(tomes: &[TomeKind], rank: u32) -> MetaSave {
    let mut save = MetaSave { tome_loadout: tomes.to_vec(), ..MetaSave::default() };
    save.tome_slots = save.tome_slots.max(tomes.len() as u32);
    for t in tomes {
        save.tome_levels.insert(*t, rank);
    }
    save
}

/// Headless self-check of the §7 tome rules on synthetic saves and sheets: prices, ranks,
/// save migration, the loadout, and every tome's lines landing where they act. Returns the
/// first violated rule. Pure data — no world needed.
pub fn self_check() -> Result<(), String> {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-3;

    // ---- the catalogue ----
    if TomeKind::ALL.len() != 23 {
        return Err(format!("{} tomes, §7 wants 23", TomeKind::ALL.len()));
    }
    for t in TomeKind::ALL {
        let d = t.def();
        if d.kind != t || d.effects.is_empty() || t.lines(1).is_empty() || t.lines(TOME_MAX_RANK).is_empty() {
            return Err(format!("{} has no effect at rank 1 or at max", d.name));
        }
        if !t.lines(0).is_empty() {
            return Err(format!("{} does something at rank 0", d.name));
        }
        if d.effects.iter().any(|e| e.at(TOME_MAX_RANK).abs() < e.at(1).abs()) {
            return Err(format!("{} weakens with rank", d.name));
        }
        if TomeKind::from_name(&format!("{t:?}").to_lowercase()) != Some(t) {
            return Err(format!("{t:?} is not reachable by name"));
        }
    }
    // §7: cost 100 × 1.6^level
    let costs: Vec<u64> = [0, 1, 2, 9].iter().map(|l| TomeKind::Orbit.cost(*l)).collect();
    if costs != vec![100, 160, 256, 6872] {
        return Err(format!("tome prices {costs:?}, want 100·1.6^rank"));
    }

    // ---- buying and slotting ----
    let mut s = MetaSave { silver: 259, ..MetaSave::default() };
    if s.tome_slots != TOME_BASE_SLOTS || !s.tome_loadout.is_empty() {
        return Err(format!("a fresh save has {} slots holding {:?}, want {TOME_BASE_SLOTS} empty", s.tome_slots, s.tome_loadout));
    }
    if !s.buy_tome(TomeKind::Horizon) || s.silver != 159 || s.tome_level(TomeKind::Horizon) != 1 {
        return Err("buying rank 1 did not charge 100".into());
    }
    if s.tome_loadout != vec![TomeKind::Horizon] {
        return Err("a first rank bought with a slot free was not slotted".into());
    }
    if s.buy_tome(TomeKind::Horizon) != (s.silver >= 160) {
        return Err("a rank was sold it could not afford (or refused one it could)".into());
    }
    s.silver = 1_000_000;
    s.tome_levels.insert(TomeKind::Horizon, TOME_MAX_RANK);
    if s.buy_tome(TomeKind::Horizon) || s.tome_level(TomeKind::Horizon) != TOME_MAX_RANK {
        return Err("a tome was bought past its max rank".into());
    }
    s.tome_loadout.clear();
    for t in TomeKind::ALL.iter().take(TOME_BASE_SLOTS as usize) {
        if !s.toggle_tome(*t) {
            return Err("an empty slot refused a tome".into());
        }
    }
    if s.toggle_tome(TomeKind::Ascension) || s.tome_loadout.len() != TOME_BASE_SLOTS as usize {
        return Err("the loadout took a tome past its slots".into());
    }
    if !s.toggle_tome(TomeKind::ALL[0]) || s.tome_loadout.contains(&TomeKind::ALL[0]) {
        return Err("a slotted tome would not come out".into());
    }

    // ---- save migration: 20 levels → 10 ranks, 3 base slots → 4, never twice ----
    // (Precision slotted but never bought: the old default loadout did that)
    let legacy = r#"{"silver": 5, "tome_levels": {"Damage": 15, "Health": 20, "Xp": 1},
                     "tome_loadout": ["Damage", "Precision", "Health", "Xp", "Xp"], "tome_slots": 4,
                     "quests_done": ["Kill10000"]}"#;
    let mut old: MetaSave = serde_json::from_str(legacy).map_err(|e| format!("legacy tome save rejected: {e}"))?;
    old.migrate();
    let ranks = (old.tome_level(TomeKind::Damage), old.tome_level(TomeKind::Health), old.tome_level(TomeKind::Xp));
    if ranks != (8, 10, 1) {
        return Err(format!("legacy levels 15/20/1 migrated to {ranks:?}, want 8/10/1 (rounded up)"));
    }
    if old.tome_slots != TOME_BASE_SLOTS + 1 || old.tome_loadout != vec![TomeKind::Damage, TomeKind::Health, TomeKind::Xp] {
        return Err(format!("legacy loadout/slots migrated to {:?}/{}", old.tome_loadout, old.tome_slots));
    }
    let again: MetaSave = serde_json::from_str(&serde_json::to_string(&old).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut again = again;
    again.migrate();
    if again.tome_level(TomeKind::Damage) != 8 || again.tome_level(TomeKind::Health) != 10 {
        return Err("a migrated save halved its ranks again on the next load".into());
    }

    // ---- nothing slotted, nothing gained ----
    let bare = PlayerState::new(AstronautKind::Buzz, &MetaSave::default());
    let mut unslotted = save_with(&[TomeKind::Orbit], TOME_MAX_RANK);
    unslotted.tome_loadout.clear();
    if PlayerState::new(AstronautKind::Buzz, &unslotted).stats.orbit != bare.stats.orbit {
        return Err("an unslotted tome still took effect".into());
    }

    // ---- every tome's lines, landing where they act (at max rank) ----
    let sheet = |t: TomeKind| PlayerState::new(AstronautKind::Buzz, &save_with(&[t], TOME_MAX_RANK));
    let fail = |t: TomeKind, what: &str| Err(format!("{}: {what}", t.def().name));
    for t in TomeKind::ALL {
        let mut ps = sheet(t);
        let st = ps.stats.clone();
        let ok = match t {
            TomeKind::Damage => close(st.damage, bare.stats.damage + 0.4),
            TomeKind::Health => close(st.max_hp, bare.stats.max_hp + 120.0) && close(ps.hp, st.max_hp),
            TomeKind::Agility => close(st.move_speed, bare.stats.move_speed + 0.24),
            TomeKind::Cooldown => close(st.attack_speed, bare.stats.attack_speed + 0.24),
            TomeKind::Precision => close(st.crit_chance, bare.stats.crit_chance + 0.12),
            TomeKind::Golden => close(st.gold_gain, bare.stats.gold_gain + 0.4),
            TomeKind::Xp => close(st.xp_gain, bare.stats.xp_gain + 0.3),
            TomeKind::Cursed => close(st.difficulty, bare.stats.difficulty + 0.4),
            TomeKind::Orbit => close(st.orbit, 1.4),
            TomeKind::Encirclement => {
                ps.crowd = TOME_CROWD_CAP;
                let full = ps.crowd_bonus();
                ps.crowd = TOME_CROWD_CAP * 3;
                close(full, 0.3) && close(ps.crowd_bonus(), 0.3) && ps.damage_mult() > bare.damage_mult() + 0.29
            }
            TomeKind::Nightfall => {
                let day = ps.damage_mult();
                ps.night = true;
                close(ps.damage_mult() - day, 0.3) && close(st.flashlight, 2.0)
            }
            TomeKind::Gravity => close(st.fall_speed, 1.6) && close(st.jump_height, bare.stats.jump_height + 0.3),
            TomeKind::Swarm => close(crate::items::proc_period(YOYO_PERIOD, &ps), YOYO_PERIOD / 1.4),
            TomeKind::Salvage => close(st.chest_discount, bare.stats.chest_discount + 0.3),
            TomeKind::Ricochet => close(st.ricochet, 1.0),
            TomeKind::Momentum => {
                ps.momentum = MOMENTUM_RAMP_SECS / 2.0;
                let half = ps.momentum_bonus();
                ps.momentum = MOMENTUM_RAMP_SECS;
                close(half, 0.15) && close(ps.momentum_bonus(), 0.3)
            }
            TomeKind::Vampirism => {
                // the baseline, and Vampire Visor's own 5% doubled on top
                ps.add_item(ItemKind::VampireVisor, Rarity::Epic);
                ps.recompute_stats(&save_with(&[t], TOME_MAX_RANK), 0);
                close(st.lifesteal, 0.04) && close(ps.stats.lifesteal, 0.04 + 0.05 * 2.0)
            }
            TomeKind::Elite => {
                // richer horde elites; the boss and minibosses share the elite table, not the tome
                let mut run = RunState::new(AstronautKind::Buzz, crate::content::planets::PlanetKind::Moon, 1, &MetaSave::default());
                run.elite_loot = st.elite_loot;
                let kill = |is_boss, is_miniboss| crate::messages::KillMsg {
                    pos: Vec3::ZERO,
                    dir: Vec3::Y,
                    kind: None,
                    elite: true,
                    xp: 0.0,
                    is_boss,
                    is_miniboss,
                    is_pot: false,
                };
                close(st.elite_loot, 2.5)
                    && close(crate::pickups::elite_loot_mult(&kill(false, false), &run), 2.5)
                    && close(crate::pickups::elite_loot_mult(&kill(true, false), &run), 1.0)
                    && close(crate::pickups::elite_loot_mult(&kill(false, true), &run), 1.0)
                    && close(incoming_mult(&st, true, false), 1.4)
                    && close(incoming_mult(&st, false, false), 1.0)
            }
            TomeKind::Banishment => {
                let mut broke = ps.clone();
                broke.refreshes = 0;
                ps.banishes == BANISH_CHARGES + 1
                    && ps.refreshes == FREE_REFRESHES + 1
                    && broke.refresh_price() == RefreshPrice::Gold((REFRESH_BASE_COST as f32 * 0.55).round() as u64)
            }
            TomeKind::Duplication => {
                let fresh = ps.free_microwave == 1;
                ps.free_microwave = 0;
                ps.enter_stage();
                fresh && ps.free_microwave == 1 && close(st.dupe_keep_grade, 0.54)
            }
            TomeKind::Horizon => close(1.0 / st.horizon_collect, 2.0),
            TomeKind::Static => {
                // both things The Static pays: 60 s of overtime and 20 ghost coins found
                let mut run = RunState::new(AstronautKind::Buzz, crate::content::planets::PlanetKind::Moon, 1, &MetaSave::default());
                run.static_secs_total = 60.0;
                run.silver_run = 20;
                run.static_silver_found = 20;
                let pay = |mult: f32| crate::director::silver_payout(&run, false, 0, 0, 1.0, mult).total;
                // rank 1's x1.1 on 20 one-Silver coins is +2 — not rounded away coin by coin
                let rank1 = 1.0 + TomeKind::Static.def().effects[0].at(1);
                close(st.static_silver, 2.0)
                    && pay(st.static_silver) == pay(1.0) + 60 + 20
                    && pay(rank1) == pay(1.0) + 6 + 2
                    && close(incoming_mult(&st, false, true), 1.5)
            }
            TomeKind::Ascension => ps.evo_cap() == EVOLUTION_CAP + 1 && close(st.evo_damage, 1.27),
        };
        if !ok {
            return fail(t, "its lines did not land on the sheet");
        }
    }
    // the discrete tomes' headline arrives whole at rank 1
    let r1 = |t: TomeKind| PlayerState::new(AstronautKind::Buzz, &save_with(&[t], 1));
    if r1(TomeKind::Ascension).evo_cap() != EVOLUTION_CAP + 1 || r1(TomeKind::Banishment).banishes != BANISH_CHARGES + 1 || r1(TomeKind::Duplication).free_microwave != 1 {
        return Err("a discrete tome's headline did not arrive at rank 1".into());
    }

    // ---- the night side is the lit side's opposite, and the Sun Shard eats the day ----
    let sun = crate::planet::sunward();
    if crate::planet::is_night(sun, 0.0) || !crate::planet::is_night(-sun, 0.0) || !crate::planet::is_night(sun, 1.0) {
        return Err("is_night disagrees with the sun".into());
    }

    // ---- the lines reach the host: a peer's Stats survive the wire's serde round trip ----
    let all = PlayerState::new(AstronautKind::Buzz, &save_with(&TomeKind::ALL, TOME_MAX_RANK));
    let wire: Stats = serde_json::from_str(&serde_json::to_string(&all.stats).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if !close(wire.horizon_collect, all.stats.horizon_collect) || wire.evo_slots != all.stats.evo_slots || !close(wire.orbit, all.stats.orbit) {
        return Err("tome lines were lost on a Stats round trip".into());
    }
    Ok(())
}
