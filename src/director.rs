//! The run director: stage timer, miniboss/boss marks, THE STATIC, level-up
//! modal flow, stage transitions, and results banking.

use crate::config::*;
use crate::content::enemies::BossKind;
use crate::content::planets::PlanetKind;
use crate::enemies::{self, Boss, Director, EnemyAssets};
use crate::interact;
use crate::messages::*;
use crate::planet::{self, CurrentPlanet, StageScoped};
use crate::player::{self, Player};
use crate::run::{self, ChoicePanel, RunPhase, RunResult, RunState};
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
    pub time: f32,
    pub quests_completed: Vec<String>,
    pub daily: Option<(String, u64, bool)>, // (world name, best score, is-new-best)
}

/// Countdown, boss marks, The Static.
pub fn run_clock(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemy_assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    q_player: Query<&Player>,
    q_boss: Query<&Boss>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok(p) = q_player.single() else { return };

    run.elapsed += dt;
    run.total_elapsed += dt;

    if run.static_active {
        run.static_timer += dt;
        // trickle of silver for surviving
        return;
    }

    run.timer -= dt;

    // miniboss marks
    for (i, mark) in MINIBOSS_MARKS.iter().enumerate() {
        if run.timer <= *mark && !run.minibosses_spawned[i] {
            run.minibosses_spawned[i] = true;
            let kind = if i == 0 { BossKind::CraterpillarJr } else { BossKind::RoverGoneWrong };
            enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, p.dir, kind, run.stats.difficulty);
            banners.write(BannerMsg(format!("{} APPROACHES", kind.def().name)));
            sfx.write(SfxMsg(Sfx::BossRoar));
        }
    }

    // stage boss
    if run.timer <= BOSS_MARK && !run.boss_spawned {
        run.boss_spawned = true;
        let kind = match planet.kind {
            PlanetKind::Moon => BossKind::Craterpillar,
            _ => BossKind::Anubot,
        };
        enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, p.dir, kind, run.stats.difficulty);
        banners.write(BannerMsg(format!("{} RISES", kind.def().name)));
        sfx.write(SfxMsg(Sfx::BossRoar));
    }

    // boss died -> teleporter (once)
    if run.boss_dead && !run.teleporter_open {
        run.teleporter_open = true;
        interact::spawn_teleporter(&mut commands, &mut meshes, &mut materials, &planet, p.dir);
        banners.write(BannerMsg("TELEPORTER ONLINE — OR STAY AND FARM".into()));
    }

    // clock ran out
    if run.timer <= 0.0 {
        run.timer = 0.0;
        if !run.static_active {
            run.static_active = true;
            banners.write(BannerMsg("THE STATIC RISES. RUN OR FARM SILVER.".into()));
            sfx.write(SfxMsg(Sfx::BossRoar));
            // the boss (if alive) stays; if it was never beaten the teleporter never opens
        }
    }
}

/// Open the level-up panel when XP crossed a threshold.
pub fn levelup_trigger(
    mut run: ResMut<RunState>,
    save: Res<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    if *phase != RunPhase::Playing || run.pending_levelups == 0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let opts = run::roll_upgrades(&run, &save, &mut rng);
    *panel = ChoicePanel {
        title: format!("LEVEL {}", run.level),
        options: opts,
        banishing: false,
        is_levelup: true,
    };
    // stat recompute so CritPerLevel-style passives track level
    let save_clone = save.clone();
    run.recompute_stats(&save_clone);
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
    *director = Director::default();
    let stage_seed = run.run_seed.wrapping_add(run.stage as u64);
    game_rng.reseed(stage_seed);

    let planet = CurrentPlanet::from_kind(run.planet());
    planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run);
    interact::spawn_interactables(&mut commands, &mut meshes, &mut materials, &planet, &run, &save, Vec3::Y);
    commands.insert_resource(planet);

    banners.write(BannerMsg(format!("STAGE {} — {}", target + 1, run.planet().def().name)));
    *phase = RunPhase::Playing;
}

/// Death -> Results after a short beat.
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
    mut run: ResMut<RunState>,
    mut save: ResMut<MetaSave>,
    mut phase: ResMut<RunPhase>,
) {
    let victory = run.result == Some(RunResult::Victory);

    // meta counters
    save.counters.kills += run.kills;
    save.counters.pots += run.pots_broken;
    save.counters.chests += run.chests_opened;
    save.counters.shrines += run.shrines_charged;
    save.counters.gold += run.gold_collected;
    save.counters.evolves += run.evolves;
    save.counters.best_level = save.counters.best_level.max(run.level);
    save.counters.static_secs_best = save.counters.static_secs_best.max(run.static_timer);
    save.counters.runs_started += 1;
    if victory {
        save.counters.runs_won += 1;
    }

    // silver payout: pickups + performance
    let performance = (run.kills / 40) as u64 + run.level as u64 + if victory { 30 * run.tier as u64 } else { 0 };
    let payout = run.silver_run + (performance as f32 * run.stats.silver_gain) as u64;
    save.silver += payout;

    // daily challenge: track today's best score
    let daily = if run.is_daily {
        let day = crate::run::today();
        if save.daily_day != day {
            save.daily_day = day;
            save.daily_best = 0;
        }
        let new_best = payout > save.daily_best;
        if new_best {
            save.daily_best = payout;
        }
        Some((crate::run::daily_name(run.run_seed), save.daily_best, new_best))
    } else {
        None
    };

    // a completed non-daily run means the player has seen the ropes
    if !run.is_daily {
        save.tutorial_done = true;
    }

    let newly = save.check_quests();
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
        level: run.level,
        gold: run.gold,
        silver_earned: payout,
        time: run.total_elapsed,
        quests_completed,
        daily,
    });

    *phase = RunPhase::Playing; // reset for next run
    run.result = None;
}
