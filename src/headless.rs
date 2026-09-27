//! Headless smoke test: `astrobonk --headless [ticks]` runs the full simulation
//! with no window, a movement bot, and auto-picked upgrades, then prints a
//! summary and exits non-zero on failure. chadkit:verify-headless compatible.

use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::content::planets::PlanetKind;
use crate::enemies::Enemy;
use crate::planet::CurrentPlanet;
use crate::player::Player;
use crate::run::{ChoicePanel, PlayerState, RefreshPrice, RunPhase, RunState, UpgradeOption};
use crate::save::MetaSave;
use crate::sphere;
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

/// `--choices`: what the bot's scripted level-up economy has exercised so far.
#[derive(Default)]
struct ChoiceScript {
    levelups: u32,
    free_refreshes: u32,
    paid_refreshes: u32,
    refused_refreshes: u32,
    banishes: u32,
    skips: u32,
    banned: Vec<UpgradeOption>,
}

/// `--choices`: drive one level-up panel through Refresh / Banish / Skip the way a player
/// would (the same `PlayerState` calls `ui::panels::choice_input` makes) and fail the smoke
/// the moment a §3 rule breaks. Returns true when the panel was consumed by a Skip.
fn scripted_choice(
    ps: &mut PlayerState,
    panel: &mut ChoicePanel,
    save: &MetaSave,
    script: &mut ChoiceScript,
) -> bool {
    let mut rng = rand::thread_rng();
    let dealt_banned = |panel: &ChoicePanel, banned: &[UpgradeOption]| {
        panel.options.iter().any(|o| banned.iter().any(|b| same_pool_entry(o, b)))
    };
    if dealt_banned(panel, &script.banned) {
        panic!("SMOKE FAIL: a banished card was dealt again ({:?})", panel.options);
    }
    script.levelups += 1;
    // Refresh until the free ones are gone, then once more at a price.
    loop {
        let (gold, price) = (ps.gold, ps.refresh_price());
        let ok = ps.spend_refresh();
        match (price, ok) {
            (RefreshPrice::Free, true) => script.free_refreshes += 1,
            (RefreshPrice::Gold(c), true) => {
                if ps.gold != gold - c {
                    panic!("SMOKE FAIL: paid refresh charged {} not {c}", gold - ps.gold);
                }
                script.paid_refreshes += 1;
            }
            (RefreshPrice::Gold(c), false) => {
                if gold >= c || ps.gold != gold {
                    panic!("SMOKE FAIL: refresh refused with {gold}g for a {c}g price");
                }
                script.refused_refreshes += 1;
            }
            (RefreshPrice::Free, false) => panic!("SMOKE FAIL: a free refresh was refused"),
        }
        if !ok {
            break;
        }
        panel.options = crate::run::roll_upgrades(ps, save, &mut rng);
        if dealt_banned(panel, &script.banned) {
            panic!("SMOKE FAIL: a banished card came back on refresh");
        }
        if !matches!(price, RefreshPrice::Free) {
            break;
        }
    }
    // Banish the first poolable card while charges last.
    if let Some(idx) = panel.options.iter().position(|o| !matches!(o, UpgradeOption::GoldPile(_))) {
        let charges = ps.banishes;
        let opt = panel.options[idx].clone();
        let ok = ps.banish(&opt);
        if ok != (charges > 0) || ps.banishes != charges.saturating_sub(1) {
            panic!("SMOKE FAIL: banish with {charges} charges returned {ok}");
        }
        if ok {
            panel.options.remove(idx);
            script.banned.push(opt);
            script.banishes += 1;
        }
    }
    // Skip every other level-up; pick otherwise.
    if script.levelups % 2 == 1 {
        let (gold, xp) = ps.skip_reward();
        let before = ps.gold;
        ps.take_skip();
        if ps.gold != before + gold || xp <= 0.0 {
            panic!("SMOKE FAIL: skip paid {}g (expected {gold}g) / {xp} xp", ps.gold - before);
        }
        script.skips += 1;
        return true;
    }
    false
}

/// Do two cards strike the same entry from the pool?
fn same_pool_entry(a: &UpgradeOption, b: &UpgradeOption) -> bool {
    use UpgradeOption::*;
    match (a, b) {
        (NewItem(x) | ItemUp(x), NewItem(y) | ItemUp(y)) => x == y,
        (NewWeapon(x) | WeaponUp(x), NewWeapon(y) | WeaponUp(y)) => x == y,
        (Evolve(x), Evolve(y)) => x == y,
        _ => false,
    }
}

/// Simple bot: run in a slowly-rotating direction, hop sometimes, take option 1
/// of every choice panel, and walk to + open the miniboss cache when one drops.
#[allow(clippy::too_many_arguments)]
fn bot_drive(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    global: Res<RunState>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    save: Res<MetaSave>,
    mut chest: ResMut<crate::interact::ChestPanel>,
    mut shop: ResMut<crate::interact::ShopPanel>,
    mut q: Query<(&mut Player, &mut PlayerState, &Transform, Has<crate::player::LocalPlayer>)>,
    q_pickups: Query<(&crate::pickups::Pickup, &Transform), Without<Player>>,
    q_enemies: Query<(&Enemy, &Transform), (Without<Player>, Without<crate::pickups::Pickup>)>,
    q_cache: Query<&Transform, (With<crate::interact::RewardCache>, Without<Player>)>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut script: Local<ChoiceScript>,
    mut heading_angle: Local<f32>,
) {
    let dt = time.delta_secs();
    // E is "tapped" fresh each frame the bot wants it, so interact_system sees just_pressed.
    keys.release(KeyCode::KeyE);
    keys.clear();

    // Panels are per-MACHINE, so only the local astronaut resolves them — mirroring the
    // real game, where a peer levelling up must not spend the host human's cards.
    if matches!(*phase, RunPhase::LevelUp | RunPhase::Modal) {
        let scripted = std::env::args().any(|a| a == "--choices");
        for (_, mut run, _, is_local) in &mut q {
            if !is_local {
                continue;
            }
            if scripted && panel.is_levelup && !panel.options.is_empty() {
                if scripted_choice(&mut run, &mut panel, &save, &mut script) {
                    run.pending_levelups = run.pending_levelups.saturating_sub(1);
                    panel.options.clear();
                }
                if script.levelups % 5 == 0 {
                    println!(
                        "  CHOICES levelups={} refresh free={} paid={} refused={} banish={} skip={} gold={}",
                        script.levelups, script.free_refreshes, script.paid_refreshes,
                        script.refused_refreshes, script.banishes, script.skips, run.gold
                    );
                }
            }
            if !panel.options.is_empty() {
                let opt = panel.options[0].clone();
                run.apply_upgrade(&opt, &save, global.greed_stacks);
                if panel.is_levelup {
                    run.pending_levelups = run.pending_levelups.saturating_sub(1);
                }
                panel.options.clear();
            }
        }
        chest.open = false;
        shop.open = false;
        *phase = RunPhase::Playing;
        return;
    }
    if matches!(*phase, RunPhase::Dead) {
        return;
    }

    for (mut p, mut run, ptf, is_local) in &mut q {

    // miniboss cache first (it is the thing under test when one exists); then
    // hurt -> kite away from the nearest threat; healthy -> chase gems; else wander
    let mut heading = None;
    let cache = if is_local { q_cache.iter().next().map(|t| t.translation) } else { None };
    if let Some(cpos) = cache {
        if cpos.distance(ptf.translation) < INTERACT_RANGE {
            keys.press(KeyCode::KeyE);
        }
        let v = cpos - ptf.translation;
        let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
        if vt != Vec3::ZERO {
            heading = Some(vt);
        }
    } else if run.hp < run.stats.max_hp * 0.45 {
        let mut best = f32::MAX;
        for (en, tf) in q_enemies.iter() {
            if en.speed == 0.0 {
                continue; // pots
            }
            let d = tf.translation.distance_squared(ptf.translation);
            if d < best {
                best = d;
                let v = ptf.translation - tf.translation;
                let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
                if vt != Vec3::ZERO {
                    heading = Some(vt);
                }
            }
        }
    } else {
        let mut best = f32::MAX;
        for (pu, tf) in q_pickups.iter() {
            if !matches!(pu.kind, crate::pickups::PickupKind::Xp(_)) {
                continue;
            }
            let d = tf.translation.distance_squared(ptf.translation);
            if d < 30.0 * 30.0 && d < best {
                best = d;
                let v = tf.translation - ptf.translation;
                let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
                if vt != Vec3::ZERO {
                    heading = Some(vt);
                }
            }
        }
    }
    let heading = heading.unwrap_or_else(|| {
        *heading_angle += dt * 0.25;
        let (t, b) = sphere::tangent_frame(p.dir);
        t * heading_angle.cos() + b * heading_angle.sin()
    });
    let speed = PLAYER_RUN_SPEED * run.move_speed_mult() * 0.85;
    p.vel_t = heading * speed;
    p.facing = heading;
    }
    let _ = &planet;
}

/// Fail-fast sanity checks each tick.
/// `--enemydist`: histogram how far the horde actually is from each astronaut, in
/// great-circle metres. This is the number the co-op streaming bandwidth budget rests on —
/// interest management is only a win if most of the horde is genuinely out of view.
fn enemy_distance_probe(
    planet: Res<CurrentPlanet>,
    q_players: Query<&Player>,
    q_enemies: Query<&Enemy>,
    mut ticks: Local<u64>,
) {
    *ticks += 1;
    if *ticks % 450 != 0 {
        return;
    }
    let players: Vec<Vec3> = q_players.iter().map(|p| p.dir).collect();
    if players.is_empty() {
        return;
    }
    // bucket by distance to the NEAREST astronaut (that is what interest management asks)
    let mut buckets = [0usize; 7];
    let edges = [20.0f32, 40.0, 60.0, 80.0, 120.0, 200.0];
    let mut total = 0usize;
    let mut statics = 0usize;
    for e in q_enemies.iter() {
        // Pots piggyback on Enemy with speed == 0 and never move — they are scenery, not
        // horde, and must not be counted against a streaming budget.
        if e.speed == 0.0 {
            statics += 1;
            continue;
        }
        total += 1;
        let d = players
            .iter()
            .map(|p| crate::sphere::arc_dist(e.dir, *p, planet.radius))
            .fold(f32::MAX, f32::min);
        let mut i = edges.len();
        for (k, edge) in edges.iter().enumerate() {
            if d < *edge {
                i = k;
                break;
            }
        }
        buckets[i] += 1;
    }
    let pct = |n: usize| if total == 0 { 0.0 } else { n as f32 * 100.0 / total as f32 };
    println!(
        "  ENEMYDIST mobile={total:<4} static_pots={statics:<3} <20m:{:>4} ({:>4.1}%) <40m:{:>4} ({:>4.1}%) <60m:{:>4} ({:>4.1}%) <80m:{:>4} ({:>4.1}%) <120m:{:>4} ({:>4.1}%) <200m:{:>4} 200m+:{:>4}",
        buckets[0], pct(buckets[0]), buckets[1], pct(buckets[1]), buckets[2], pct(buckets[2]),
        buckets[3], pct(buckets[3]), buckets[4], pct(buckets[4]), buckets[5], buckets[6]
    );
}

fn bot_watchdog(run: Res<RunState>, q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>, q_enemies: Query<(), With<Enemy>>, mut ticks: Local<u64>) {
    *ticks += 1;
    let alive = q_enemies.iter().count();
    if alive > ENEMY_CAP + 400 {
        panic!("SMOKE FAIL: enemy cap breached ({alive})");
    }
    let hp = q_ps.single().map(|p| p.hp).unwrap_or(1.0);
    if !hp.is_finite() || !run.timer.is_finite() {
        panic!("SMOKE FAIL: non-finite run state");
    }
    if *ticks % 300 == 0 {
        let sc = crate::run::scaling::Scaling::for_run(&run, 1);
        println!(
            "  t={:>4.0}s timer={:>5.1} lvl={} kills={} hp={:.0} enemies={} gold={} static={} scale[hp x{:.2} dmg x{:.2} spawn x{:.2} elite {:.0}%]",
            run.total_elapsed, run.timer, q_ps.single().map(|p| p.level).unwrap_or(1), run.kills, hp, alive, q_ps.single().map(|p| p.gold).unwrap_or(0), run.static_active,
            sc.hp, sc.dmg, sc.spawn, sc.elite_chance * 100.0
        );
    }
}

pub fn run_headless(ticks: u64, fast_boss: bool, hero: AstronautKind, planet_kind: PlanetKind, seed: Option<u64>) {
    println!(
        "ASTROBONK headless smoke: {ticks} ticks @33ms{} hero={} planet={:?} seed={:?}",
        if fast_boss { " (fast-boss)" } else { "" },
        hero.def().name,
        planet_kind,
        seed
    );
    // The rules are pure data: pin them before simulating anything.
    let rules = crate::run::scaling::self_check()
        .and_then(|_| crate::run::rules_self_check(&MetaSave::default()))
        .and_then(|_| silver_self_check());
    match rules {
        Ok(()) => println!("RULES OK (scaling, choice economy, evolution cap, silver)"),
        Err(e) => {
            println!("SMOKE FAIL: rules self-check: {e}");
            std::process::exit(1);
        }
    }

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_once()),
        TransformPlugin,
        StatesPlugin,
        AssetPlugin::default(),
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(33)))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_resource::<Assets<Mesh>>()
    .init_resource::<Assets<StandardMaterial>>();

    let mut save = MetaSave::default();
    if fast_boss {
        // a veteran loadout so the bot survives long enough to meet the late-game kinds
        save.tome_levels.insert(crate::content::tomes::TomeKind::Damage, 20);
        save.tome_levels.insert(crate::content::tomes::TomeKind::Health, 20);
    }
    let mut run = RunState::new(hero, planet_kind, 1, &save);
    if let Some(s) = seed {
        run.run_seed = s;
    }
    if fast_boss {
        run.timer = 95.0; // just above the boss mark: boss arrives ~5s in
        run.elapsed = 570.0; // late-game spawn mix: beamers, lobbers, UFOs, burrowers
        run.total_elapsed = 570.0; // and the §3 run-time scaling that goes with it
    }

    app.init_state::<crate::AppState>()
        .init_resource::<RunPhase>()
        .init_resource::<ChoicePanel>()
        .init_resource::<crate::player::CamRig>()
        .init_resource::<crate::fx::Shake>()
        .init_resource::<crate::fx::Hitstop>()
        .init_resource::<crate::enemies::SpatialHash>()
        .init_resource::<crate::enemies::Director>()
        .init_resource::<crate::director::PendingStage>()
        .init_resource::<crate::director::ResultsData>()
        .init_resource::<crate::interact::InteractPrompt>()
        .init_resource::<crate::interact::ChestPanel>()
        .init_resource::<crate::interact::ShopPanel>()
        .init_resource::<crate::comet::Comet>()
        .init_resource::<crate::run::GameRng>()
        .init_resource::<crate::planet::PropColliders>()
        .init_resource::<crate::events_world::DustStorm>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(save)
        .insert_resource(run)
        .add_message::<crate::net::GrantOut>()
        .add_message::<crate::messages::HitMsg>()
        .add_message::<crate::messages::PlayerHitMsg>()
        .add_message::<crate::messages::KillMsg>()
        .add_message::<crate::messages::NumberMsg>()
        .add_message::<crate::messages::BannerMsg>()
        .add_message::<crate::messages::SfxMsg>()
        .add_systems(Startup, (crate::enemies::setup_enemy_assets, crate::combat::setup_weapon_assets, crate::pickups::setup_pickup_assets, headless_enter))
        .add_systems(
            Update,
            (
                crate::enemies::rebuild_hash,
                crate::enemies::director_spawn,
                crate::enemies::enemy_move,
                crate::enemies::craterpillar_update,
                crate::enemies::anubot_beam_system,
                crate::enemies::boss_phase_system,
                crate::enemies::burrower_emerge,
                crate::enemies::enemy_contact,
                crate::enemies::spitter_attack,
                crate::enemies::beamer_attack,
                crate::enemies::lobber_attack,
                crate::enemies::mortar_shells,
                crate::enemies::enemy_projectiles,
                crate::enemies::boss_attacks,
                crate::enemies::telegraphs,
            )
                .chain()
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                crate::combat::weapon_fire,
                crate::combat::projectile_move,
                crate::combat::drone_update,
                crate::combat::beam_update,
                crate::interact::charge_shrines,
                crate::interact::interact_system,
                crate::pickups::pickup_update,
                crate::director::run_clock,
                crate::director::levelup_trigger,
                crate::comet::comet_system,
                crate::events_world::dust_storm_system,
            )
                .chain()
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                bot_drive,
                crate::combat::apply_hits,
                crate::combat::apply_player_hits,
                crate::director::downed_watch,
                enemy_distance_probe.run_if(|| std::env::args().any(|a| a == "--enemydist")),
                crate::pickups::kill_drops,
                crate::combat::fader_update,
                crate::player::player_physics,
                crate::player::player_upkeep,
                crate::fx::update_particles,
                crate::director::stage_transition,
                crate::interact::sync_reward_cache.before(crate::director::stage_transition),
                bot_watchdog,
            ),
        )
        .add_systems(
            Update,
            crate::director::dev_miniboss_now
                .run_if(crate::playing)
                .run_if(|| std::env::args().any(|a| a == "--minibossnow")),
        );

    // enter InRun immediately
    app.insert_state(crate::AppState::InRun);

    for _ in 0..ticks {
        app.update();
        // stop early on death
        let phase = *app.world().resource::<RunPhase>();
        if phase == RunPhase::Dead {
            break;
        }
    }

    let world = app.world_mut();
    let run = world.resource::<RunState>().clone();
    let ps = world.query::<&PlayerState>().iter(world).next().cloned();
    let (p_level, p_gold, p_hp, p_maxhp) = ps
        .as_ref()
        .map(|p| (p.level, p.gold, p.hp, p.stats.max_hp))
        .unwrap_or((1, 0, 0.0, 100.0));
    let enemies = world.query_filtered::<(), With<Enemy>>().iter(world).count();
    let phase = *world.resource::<RunPhase>();
    let comet_fires = world.resource::<crate::comet::Comet>().fires;
    let silver = {
        let save = world.resource::<MetaSave>();
        let golden = if save.tome_loadout.contains(&crate::content::tomes::TomeKind::Golden) {
            save.tome_level(crate::content::tomes::TomeKind::Golden)
        } else {
            0
        };
        let (rocks, gain) = ps
            .as_ref()
            .map(|p| (p.item_count(crate::content::items::ItemKind::CursedMoonRock), p.stats.silver_gain))
            .unwrap_or((0, 1.0));
        crate::director::silver_payout(&run, false, golden, rocks, gain)
    };
    let storm = world.resource::<crate::events_world::DustStorm>();
    let storm_state = format!("spawned={} active={}", storm.spawned_vis, storm.active);

    println!("--- SMOKE SUMMARY ---");
    println!(
        "phase={phase:?} level={} kills={} gold={} hp={:.0}/{:.0} timer={:.0} enemies={} comets={comet_fires} storm[{storm_state}] boss_spawned={} boss_dead={}",
        p_level, run.kills, p_gold, p_hp, p_maxhp, run.timer, enemies, run.boss_spawned, run.boss_dead
    );

    let lines: Vec<String> = silver.lines.iter().map(|(l, a)| format!("{l} {a}")).collect();
    println!("silver={} [{}] chests_opened={} boss_kills={}", silver.total, lines.join(", "), run.chests_opened, run.boss_kills);

    let mut ok = true;
    if silver.total == 0 && run.total_elapsed > SILVER_SURVIVAL_SECS_PER {
        println!("FAIL: the run banked zero Silver (§10: no run ever pays out zero)");
        ok = false;
    }
    if std::env::args().any(|a| a == "--minibossnow") && run.chests_opened == 0 {
        println!("FAIL: miniboss #1 cache never dropped/opened (reward_chest={:?})", run.reward_chest);
        ok = false;
    }
    if matches!(phase, RunPhase::LevelUp | RunPhase::Modal) {
        println!("FAIL: run ended stuck in a panel phase");
        ok = false;
    }
    if run.kills == 0 && !fast_boss {
        println!("FAIL: bot killed nothing");
        ok = false;
    }
    if p_level < 2 && run.kills > 50 {
        println!("FAIL: XP pipeline dead (kills but no levels)");
        ok = false;
    }
    if enemies == 0 && !run.boss_dead {
        println!("FAIL: spawner produced no live enemies");
        ok = false;
    }
    if fast_boss && !run.boss_spawned {
        println!("FAIL: boss never spawned in fast-boss mode");
        ok = false;
    }
    if ok {
        println!("SMOKE OK");
    } else {
        std::process::exit(1);
    }
}

fn headless_enter(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    run_state: Res<RunState>,
    save: Res<MetaSave>,
    mut game_rng: ResMut<crate::run::GameRng>,
) {
    let stage_seed = run_state.run_seed.wrapping_add(run_state.stage as u64);
    game_rng.reseed(stage_seed);
    let planet = CurrentPlanet::from_kind(run_state.planet());
    let props = crate::planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    commands.insert_resource(props);
    crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 0, run_state.character, true, None);
    // `--coop2` reproduces a 2-player HOST headlessly. Without it none of the multi-player
    // work is testable without launching two windows by hand.
    if std::env::args().any(|a| a == "--coop2") {
        crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 1, run_state.character, false, None);
    }
    crate::interact::spawn_interactables(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &PlayerState::new(run_state.character, &save), &save, Vec3::Y);
    commands.insert_resource(planet);
}

/// The §10 formula on a known run: 10:00 survived, 1,000 kills, one boss, a Tier-2 clear,
/// 30 s of Static, 7 Silver picked up, Golden Tome 2 and two Cursed Moon Rocks.
fn silver_self_check() -> Result<(), String> {
    let mut run = RunState::new(AstronautKind::Buzz, PlanetKind::Moon, 2, &MetaSave::default());
    run.total_elapsed = 600.0;
    run.kills = 1000;
    run.boss_kills = 1;
    run.static_secs_total = 30.0;
    run.silver_run = 7;
    let p = crate::director::silver_payout(&run, true, 2, 2, 1.0);
    // (100 + 250 + 150 + 100 + 30) × 1.10 × 1.30 = 900.9 → 901, + 7 found
    if p.total != 908 {
        return Err(format!("silver formula gave {} for the reference run, want 908", p.total));
    }
    let dead = crate::director::silver_payout(&run, false, 0, 0, 1.0);
    if dead.total != 100 + 250 + 150 + 30 + 7 {
        return Err(format!("a death should bank everything but the tier bonus, got {}", dead.total));
    }
    Ok(())
}
