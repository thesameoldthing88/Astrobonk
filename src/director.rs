//! The run director: stage timer, miniboss/boss marks, THE STATIC, level-up
//! modal flow, stage transitions, and results banking.

use crate::config::*;
use crate::content::enemies::BossKind;
use crate::content::items::ItemKind;
use crate::content::planets::PlanetKind;
use crate::content::tomes::TomeKind;
use crate::enemies::{self, Director, EnemyAssets, MinibossSlot};
use crate::interact;
use crate::messages::*;
use crate::planet::{self, CurrentPlanet, StageScoped};
use crate::player::{self, Player};
use crate::run::scaling::Scaling;
use crate::run::{self, ChoicePanel, PlayerState, RunPhase, RunResult, RunState};
use crate::save::MetaSave;
use crate::AppState;
use bevy::prelude::*;

/// Set to request a stage change (teleporter). `usize::MAX`-safe: >= chain len = victory.
#[derive(Resource, Default)]
pub struct PendingStage(pub Option<usize>);

/// Results screen contents, built when the run ends.
#[derive(Resource, Default)]
pub struct ResultsData {
    pub victory: bool,
    pub kills: u64,
    pub level: u32,
    pub gold: u64,
    pub silver_earned: u64,
    /// The §10 formula term by term, (label, amount) — the results screen shows the math
    /// so "the next unlock always feels close" is something the player can read.
    pub silver_lines: Vec<(String, String)>,
    pub time: f32,
    pub quests_completed: Vec<String>,
    pub daily: Option<(String, u64, bool)>, // (world name, best score, is-new-best)
    /// The run used §13 assists: what was on when it ended. Shown on results, and the daily
    /// score went to the separate assisted board.
    pub assisted: Option<String>,
}

/// HOST/SOLO: keep the run's assist options current with this machine's settings, so a
/// slider moved in the pause menu applies mid-run (§13: every setting is reachable mid-run).
/// Any assist, even for a moment, flags the run. A client never runs this — its run is the
/// host's, adopted from `RunSnapMsg`.
pub fn sync_assist_options(save: Res<MetaSave>, mut run: ResMut<RunState>) {
    if run.assist != save.assist {
        run.assist = save.assist;
    }
    if run.assist.is_assisted() && !run.assisted {
        run.assisted = true;
        info!("run flagged ASSISTED ({})", run.assist.summary());
    }
}

/// Countdown, boss marks, The Static.
#[allow(clippy::too_many_arguments)]
pub fn run_clock(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemy_assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<&Player>,
    mut telemetry: ResMut<crate::items::ItemTelemetry>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    run.elapsed += dt;
    run.total_elapsed += dt;

    if run.static_active {
        run.static_timer += dt;
        // every overtime second is paid at banking (§10 Static_overtime_seconds)
        run.static_secs_total += dt;
    } else {
        run.timer -= dt;
    }

    // Spawn anchor: the party CENTROID, so a boss doesn't erupt in one player's lap and
    // half a planet away from the other. Only the spawn calls below need it, so the clock
    // above keeps running even with nobody alive.
    let dirs: Vec<Vec3> = q_player.iter().map(|p| p.dir).collect();
    let anchor = dirs
        .iter()
        .copied()
        .sum::<Vec3>()
        .try_normalize()
        .or_else(|| dirs.first().copied());

    let scaling = Scaling::for_run(&run, dirs.len());

    // The marks belong to the clock, and the clock stops when The Static rises.
    if !run.static_active {
        // miniboss marks
        for (i, mark) in MINIBOSS_MARKS.iter().enumerate() {
            let Some(anchor) = anchor else { break };
            if run.timer <= *mark && !run.minibosses_spawned[i] {
                run.minibosses_spawned[i] = true;
                let kind = if i == 0 { BossKind::CraterpillarJr } else { BossKind::RoverGoneWrong };
                let e = enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, anchor, kind, &scaling);
                // Tag WHICH mark this is: the §3 guaranteed chest follows miniboss #1, whatever
                // kind a world's miniboss table (P20) puts there.
                commands.entity(e).insert(MinibossSlot(i as u8));
                banners.write(BannerMsg(format!("{} APPROACHES", kind.def().name)));
                sfx.write(SfxMsg(Sfx::BossRoar));
            }
        }

        // stage boss
        if run.timer <= BOSS_MARK && !run.boss_spawned && anchor.is_some() {
            run.boss_spawned = true;
            let kind = match planet.kind {
                PlanetKind::Moon => BossKind::Craterpillar,
                _ => BossKind::Anubot,
            };
            enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, anchor.unwrap_or(Vec3::Y), kind, &scaling);
            banners.write(BannerMsg(format!("{} RISES", kind.def().name)));
            sfx.write(SfxMsg(Sfx::BossRoar));
        }
    }

    // boss died -> teleporter (once). Checked in The Static too: a boss the squad finally
    // drops in overtime still opens the way out — and with The Static Radio (§7) the boss
    // and The Static arrive together, so otherwise the stage could never be left.
    if run.boss_dead && !run.teleporter_open && anchor.is_some() {
        run.teleporter_open = true;
        interact::spawn_teleporter(&mut commands, &mut meshes, &mut materials, &planet, anchor.unwrap_or(Vec3::Y));
        banners.write(BannerMsg("TELEPORTER ONLINE — OR STAY AND FARM".into()));
    }

    // clock ran out — STATIC_RADIO_LEAD_SECS early if someone carries The Static Radio
    let lead = if run.static_radio { STATIC_RADIO_LEAD_SECS } else { 0.0 };
    if !run.static_active && run.timer <= lead {
        run.timer = run.timer.max(0.0);
        run.static_active = true;
        if lead > 0.0 {
            telemetry.radio_static_at.get_or_insert(run.timer);
            banners.write(BannerMsg("THE RADIO CALLED IT IN. THE STATIC RISES EARLY.".into()));
        } else {
            banners.write(BannerMsg("THE STATIC RISES. RUN OR FARM SILVER.".into()));
        }
        sfx.write(SfxMsg(Sfx::BossRoar));
        // the boss (if alive) stays; if it was never beaten the teleporter never opens
    }
}

/// Open the level-up panel when XP crossed a threshold.
pub fn levelup_trigger(
    run: Res<RunState>,
    mut q_ps: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let Ok(mut ps) = q_ps.single_mut() else { return };
    if *phase != RunPhase::Playing || ps.pending_levelups == 0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let opts = run::roll_upgrades(&ps, &save, &mut rng);
    *panel = ChoicePanel {
        title: format!("LEVEL {}", ps.level),
        options: opts,
        banishing: false,
        is_levelup: true,
    };
    // stat recompute so CritPerLevel-style passives track level
    let save_clone = save.clone();
    ps.recompute_stats(&save_clone, run.greed_stacks);
    *phase = RunPhase::LevelUp;
    sfx.write(SfxMsg(Sfx::LevelUp));
}

/// Teleporter fired: advance stage or bank a victory.
#[allow(clippy::too_many_arguments)]
pub fn stage_transition(
    mut commands: Commands,
    mut pending: ResMut<PendingStage>,
    mut run: ResMut<RunState>,
    mut save: ResMut<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut director: ResMut<Director>,
    mut game_rng: ResMut<crate::run::GameRng>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut next: ResMut<NextState<AppState>>,
    q_ps: Query<(&PlayerState, &crate::player::PlayerId, Has<crate::player::LocalPlayer>)>,
    scoped: Query<Entity, With<StageScoped>>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let Some(target) = pending.0 else { return };
    pending.0 = None;

    if target >= run.chain.len() {
        // VICTORY
        run.result = Some(RunResult::Victory);
        save.counters.cleared.insert((run.chain[0], run.tier));
        // also mark per-planet tier-1 clears for later planets in the chain
        if run.chain.len() > 1 {
            for p in run.chain.iter().skip(1) {
                save.counters.cleared.insert((*p, 1));
            }
        }
        next.set(AppState::Results);
        return;
    }

    // Snapshot every astronaut's sheet BEFORE the sweep: astronauts are StageScoped, so
    // this despawns them all. Previously only player 0 was respawned (peers vanished for
    // good) and its sheet was read back AFTER the wipe, i.e. a blank level-1 sheet — which
    // is why the shop rolled at zero luck and results always printed level 1.
    let mut carried: Vec<(u8, PlayerState, bool)> = q_ps
        .iter()
        .map(|(ps, id, local)| (id.0, ps.clone(), local))
        .collect();
    carried.sort_by_key(|(id, _, _)| *id);

    // tear down the old stage
    for e in &scoped {
        commands.entity(e).despawn();
    }

    // set up the new one
    run.stage = target;
    run.timer = STAGE_SECONDS[target.min(STAGE_SECONDS.len() - 1)];
    run.elapsed = 0.0;
    run.boss_spawned = false;
    run.boss_dead = false;
    run.minibosses_spawned = [false; 2];
    run.static_active = false;
    run.static_timer = 0.0;
    run.teleporter_open = false;
    run.microwave_used = false;
    run.reward_chest = None;
    *director = Director::default();
    let stage_seed = run.run_seed.wrapping_add(run.stage as u64);
    game_rng.reseed(stage_seed);

    let planet = CurrentPlanet::from_kind(run.planet());
    let props = planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    commands.insert_resource(props);
    let ps_snapshot = carried
        .iter()
        .find(|(_, _, local)| *local)
        .map(|(_, ps, _)| ps.clone())
        .unwrap_or_else(|| PlayerState::new(run.character, &save));
    if carried.is_empty() {
        player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run, &save, 0, run.character, true, None);
    } else {
        for (id, ps, is_local) in carried {
            player::spawn_player(
                &mut commands, &mut meshes, &mut materials, &planet, &run, &save,
                id, ps.character, is_local, Some(ps),
            );
        }
    }
    interact::spawn_interactables(&mut commands, &mut meshes, &mut materials, &planet, &run, &ps_snapshot, &save, Vec3::Y);
    commands.insert_resource(planet);

    banners.write(BannerMsg(format!("STAGE {} — {}", target + 1, run.planet().def().name)));
    *phase = RunPhase::Playing;
}

/// Death -> Results after a short beat.
/// The run ends only when EVERY astronaut is down — one player's mistake must not kick the
/// whole lobby to the results screen. With a single player this is bit-identical to the old
/// behaviour (the one player being down IS all of them being down).
///
/// This is also the exact hook a revive plugs into: reviving is just clearing `dead`.
pub fn downed_watch(
    mut run: ResMut<RunState>,
    mut phase: ResMut<RunPhase>,
    q: Query<&crate::run::PlayerState, With<crate::player::Player>>,
) {
    if *phase == RunPhase::Dead || run.result.is_some() {
        return;
    }
    if !q.is_empty() && q.iter().all(|ps| ps.dead) {
        run.result = Some(crate::run::RunResult::Death);
        *phase = RunPhase::Dead;
    }
}

pub fn death_watch(
    phase: Res<RunPhase>,
    mut timer: Local<f32>,
    time: Res<Time<Real>>,
    mut next: ResMut<NextState<AppState>>,
) {
    if *phase == RunPhase::Dead {
        *timer += time.delta_secs();
        if *timer > 1.6 {
            *timer = 0.0;
            next.set(AppState::Results);
        }
    } else {
        *timer = 0.0;
    }
}

/// Bank the run into the save when Results opens.
pub fn bank_results(
    mut commands: Commands,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    mut run: ResMut<RunState>,
    mut save: ResMut<MetaSave>,
    mut phase: ResMut<RunPhase>,
) {
    let victory = run.result == Some(RunResult::Victory);
    let (p_level, p_gold) = q_ps.single().map(|p| (p.level, p.gold)).unwrap_or((1, 0));

    // meta counters
    save.counters.kills += run.kills;
    save.counters.pots += run.pots_broken;
    save.counters.chests += run.chests_opened;
    save.counters.shrines += run.shrines_charged;
    save.counters.gold += run.gold_collected;
    save.counters.evolves += run.evolves;
    save.counters.best_level = save.counters.best_level.max(p_level);
    save.counters.static_secs_best = save.counters.static_secs_best.max(run.static_timer);
    save.counters.runs_started += 1;
    if victory {
        save.counters.runs_won += 1;
    }

    // silver payout: the §10 formula plus the Silver physically picked up this run
    let golden_tome = save.tome_rank_equipped(TomeKind::Golden);
    let (cursed_rocks, silver_gain, static_silver) = q_ps
        .single()
        .map(|p| (p.item_count(ItemKind::CursedMoonRock), p.stats.silver_gain, p.stats.static_silver))
        .unwrap_or((0, 1.0, 1.0));
    let silver = silver_payout(&run, victory, golden_tome, cursed_rocks, silver_gain, static_silver);
    let payout = silver.total;
    save.silver += payout;

    // daily challenge: track today's best score. An assisted run keeps its own board (§13:
    // assists never touch the ranking, and still earn Silver).
    let daily = if run.is_daily {
        let day = crate::run::today();
        if save.daily_day != day {
            save.daily_day = day;
            save.daily_best = 0;
            save.daily_best_assisted = 0;
        }
        let best = if run.assisted { &mut save.daily_best_assisted } else { &mut save.daily_best };
        let new_best = payout > *best;
        if new_best {
            *best = payout;
        }
        Some((crate::run::daily_name(run.run_seed), *best, new_best))
    } else {
        None
    };

    // a completed non-daily run means the player has seen the ropes
    if !run.is_daily {
        save.tutorial_done = true;
    }

    let newly = save.check_quests();
    // Quest Silver is paid into the save by check_quests; list it under the formula so the
    // screen accounts for every Silver the run moved (kept out of `payout`, which is also
    // the daily score and must not reward first-time quest clears).
    let quest_silver: u64 = newly
        .iter()
        .flat_map(|q| q.def().rewards.iter())
        .map(|r| match r {
            crate::content::quests::Reward::Silver(s) => *s,
            _ => 0,
        })
        .sum();
    let mut silver_lines = silver.lines;
    if quest_silver > 0 {
        silver_lines.push(("Quest rewards".into(), format!("+{quest_silver}")));
    }
    let quests_completed: Vec<String> = newly
        .iter()
        .map(|q| {
            let d = q.def();
            format!("{} — {}", d.name, d.desc)
        })
        .collect();

    save.save();

    commands.insert_resource(ResultsData {
        victory,
        kills: run.kills,
        level: p_level,
        gold: p_gold,
        silver_earned: payout,
        silver_lines,
        time: run.total_elapsed,
        quests_completed,
        daily,
        assisted: run.assisted.then(|| {
            let now = run.assist.summary();
            if now.is_empty() { "assists used earlier in the run".to_string() } else { now }
        }),
    });

    *phase = RunPhase::Playing; // reset for next run
    run.result = None;
}

/// The §10 Silver payout, itemised.
pub struct SilverPayout {
    /// (label, amount) rows for the results screen, in formula order.
    pub lines: Vec<(String, String)>,
    pub total: u64,
}

/// ```text
/// Silver = (survival_seconds / 6) + (kills / 4) + (boss_kills × 150)
///        + (tier_bonus × 50) + Static_overtime_seconds
///        × (1 + GoldenTome×0.05) × (1 + CursedMoonRock×0.15)
/// ```
/// The two multipliers scale the WHOLE sum: read literally they would bind to the Static
/// term alone, which would make a Golden Tome worthless to anyone who takes the teleporter.
/// Added on top: the Silver picked up in the run (pots, The Static's ghosts), already
/// multiplied by Silver gain when it was collected. `tier_bonus` is the Tier on a chain
/// clear and 0 otherwise — a death still banks every other term (§3: death is never a zero).
/// `static_silver` is Tome of Static ("The Static pays double Silver"): it multiplies both
/// things The Static pays — the overtime term and the ghosts' Silver — the latter as one
/// total here, since a 1-Silver coin cannot carry a x1.1.
pub fn silver_payout(
    run: &RunState,
    victory: bool,
    golden_tome: u32,
    cursed_rocks: u32,
    silver_gain: f32,
    static_silver: f32,
) -> SilverPayout {
    let clock = |secs: f32| format!("{}:{:02}", (secs / 60.0) as u32, (secs % 60.0) as u32);
    let survival = (run.total_elapsed.max(0.0) / SILVER_SURVIVAL_SECS_PER) as u64;
    let kills = (run.kills as f32 / SILVER_KILLS_PER) as u64;
    let bosses = run.boss_kills * SILVER_PER_BOSS;
    let tier = if victory { run.tier as u64 * SILVER_PER_TIER } else { 0 };
    let static_tome = static_silver.max(0.0);
    let overtime = (run.static_secs_total.max(0.0) * SILVER_PER_STATIC_SEC * static_tome) as u64;
    let base = survival + kills + bosses + tier + overtime;

    let mut lines = vec![
        (format!("Survived {}", clock(run.total_elapsed)), format!("+{survival}")),
        (format!("Bonks {}", run.kills), format!("+{kills}")),
    ];
    if run.boss_kills > 0 {
        lines.push((format!("Bosses {} x {SILVER_PER_BOSS}", run.boss_kills), format!("+{bosses}")));
    }
    if tier > 0 {
        lines.push((format!("Tier {} clear", run.tier), format!("+{tier}")));
    }
    if overtime > 0 {
        let tome = if (static_tome - 1.0).abs() > 1e-3 { format!(" (Tome of Static x{static_tome:.1})") } else { String::new() };
        lines.push((format!("Static overtime {}{tome}", clock(run.static_secs_total)), format!("+{overtime}")));
    }
    let golden = 1.0 + SILVER_GOLDEN_TOME_PER_LEVEL * golden_tome as f32;
    let cursed = 1.0 + SILVER_CURSED_ROCK_EACH * cursed_rocks as f32;
    if golden_tome > 0 {
        lines.push((format!("Golden Tome Lv{golden_tome}"), format!("x{golden:.2}")));
    }
    if cursed_rocks > 0 {
        lines.push((format!("Cursed Moon Rock x{cursed_rocks}"), format!("x{cursed:.2}")));
    }
    if (silver_gain - 1.0).abs() > 1e-3 {
        lines.push(("Silver gain".into(), format!("x{silver_gain:.2}")));
    }
    let performance = (base as f32 * golden * cursed * silver_gain.max(0.0)).round() as u64;
    if run.silver_run > 0 {
        lines.push(("Silver found".into(), format!("+{}", run.silver_run)));
    }
    let ghost_bonus = (run.static_silver_found as f32 * (static_tome - 1.0).max(0.0)).round() as u64;
    if ghost_bonus > 0 {
        lines.push((format!("Static ghost Silver (Tome of Static x{static_tome:.1})"), format!("+{ghost_bonus}")));
    }
    SilverPayout { lines, total: performance + run.silver_run + ghost_bonus }
}

/// `--minibossnow` (test harness; the windowed game also needs `--dev`): wind the clock to
/// just before the 7:00 mark, then — once miniboss #1 has stood a few seconds — finish it
/// through the REAL HitMsg → apply_hits path, so the guaranteed-cache pipeline (kill →
/// RunState → sync_reward_cache → RunSnapMsg) runs end to end without a 3-minute fight.
/// Host/solo only.
pub fn dev_miniboss_now(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut wound: Local<bool>,
    mut alive_for: Local<f32>,
    q: Query<(Entity, &enemies::Enemy, &MinibossSlot)>,
    mut hits: MessageWriter<HitMsg>,
) {
    if !*wound {
        *wound = true;
        run.timer = MINIBOSS_MARKS[0] + 3.0;
        // keep the spawn mix and the §3 run-time term consistent with the wound clock
        run.elapsed = STAGE_SECONDS[0] - run.timer;
        run.total_elapsed = run.total_elapsed.max(run.elapsed);
        info!("DEV --minibossnow: clock wound to {:.0}s", run.timer);
        return;
    }
    for (e, enemy, slot) in &q {
        if slot.0 != 0 || enemy.hp <= 0.0 {
            continue;
        }
        *alive_for += time.delta_secs();
        if *alive_for > 6.0 {
            hits.write(HitMsg { source: None, target: e, amount: enemy.hp + 1.0, crit: false, knock: Vec3::ZERO });
        }
    }
}
