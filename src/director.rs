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
    run.elapsed += dt;
    run.total_elapsed += dt;

    if run.static_active {
        run.static_timer += dt;
        // trickle of silver for surviving
        return;
    }

    run.timer -= dt;

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

    // miniboss marks
    for (i, mark) in MINIBOSS_MARKS.iter().enumerate() {
        let Some(anchor) = anchor else { break };
        if run.timer <= *mark && !run.minibosses_spawned[i] {
            run.minibosses_spawned[i] = true;
            let kind = if i == 0 { BossKind::CraterpillarJr } else { BossKind::RoverGoneWrong };
            enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, anchor, kind, run.difficulty);
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
        enemies::spawn_boss(&mut commands, &mut meshes, &enemy_assets, &planet, anchor.unwrap_or(Vec3::Y), kind, run.difficulty);
        banners.write(BannerMsg(format!("{} RISES", kind.def().name)));
        sfx.write(SfxMsg(Sfx::BossRoar));
    }

    // boss died -> teleporter (once)
    if run.boss_dead && !run.teleporter_open && anchor.is_some() {
        run.teleporter_open = true;
        interact::spawn_teleporter(&mut commands, &mut meshes, &mut materials, &planet, anchor.unwrap_or(Vec3::Y));
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

    // silver payout: pickups + performance
    let performance = (run.kills / 40) as u64 + p_level as u64 + if victory { 30 * run.tier as u64 } else { 0 };
    let payout = run.silver_run + (performance as f32 * q_ps.single().map(|p| p.stats.silver_gain).unwrap_or(1.0)) as u64;
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
        level: p_level,
        gold: p_gold,
        silver_earned: payout,
        time: run.total_elapsed,
        quests_completed,
        daily,
    });

    *phase = RunPhase::Playing; // reset for next run
    run.result = None;
}
