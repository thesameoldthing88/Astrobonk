//! Headless smoke test: `astrobonk --headless [ticks]` runs the full simulation
//! with no window, a movement bot, and auto-picked upgrades, then prints a
//! summary and exits non-zero on failure. chadkit:verify-headless compatible.

use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::content::planets::PlanetKind;
use crate::enemies::Enemy;
use crate::planet::CurrentPlanet;
use crate::player::Player;
use crate::run::{ChoicePanel, RunPhase, RunState};
use crate::save::MetaSave;
use crate::sphere;
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

/// Simple bot: run in a slowly-rotating direction, hop sometimes, take option 1
/// of every choice panel, and E every prompt it happens to stand on.
fn bot_drive(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    save: Res<MetaSave>,
    mut chest: ResMut<crate::interact::ChestPanel>,
    mut shop: ResMut<crate::interact::ShopPanel>,
    mut q: Query<(&mut Player, &Transform)>,
    q_pickups: Query<(&crate::pickups::Pickup, &Transform), Without<Player>>,
    q_enemies: Query<(&Enemy, &Transform), (Without<Player>, Without<crate::pickups::Pickup>)>,
    mut heading_angle: Local<f32>,
) {
    let Ok((mut p, ptf)) = q.single_mut() else { return };
    let dt = time.delta_secs();

    // resolve any open panel instantly
    match *phase {
        RunPhase::LevelUp | RunPhase::Modal => {
            if !panel.options.is_empty() {
                let opt = panel.options[0].clone();
                run.apply_upgrade(&opt, &save);
                if panel.is_levelup {
                    run.pending_levelups = run.pending_levelups.saturating_sub(1);
                }
                panel.options.clear();
            }
            chest.open = false;
            shop.open = false;
            *phase = RunPhase::Playing;
            return;
        }
        RunPhase::Dead => return,
        _ => {}
    }

    // hurt -> kite away from the nearest threat; healthy -> chase gems; else wander
    let mut heading = None;
    if run.hp < run.stats.max_hp * 0.45 {
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
    let _ = &planet;
}

/// Fail-fast sanity checks each tick.
fn bot_watchdog(run: Res<RunState>, q_enemies: Query<(), With<Enemy>>, mut ticks: Local<u64>) {
    *ticks += 1;
    let alive = q_enemies.iter().count();
    if alive > ENEMY_CAP + 400 {
        panic!("SMOKE FAIL: enemy cap breached ({alive})");
    }
    if !run.hp.is_finite() || !run.timer.is_finite() {
        panic!("SMOKE FAIL: non-finite run state");
    }
    if *ticks % 300 == 0 {
        println!(
            "  t={:>4.0}s timer={:>5.1} lvl={} kills={} hp={:.0} enemies={} gold={} static={}",
            run.total_elapsed, run.timer, run.level, run.kills, run.hp, alive, run.gold, run.static_active
        );
    }
}

pub fn run_headless(ticks: u64, fast_boss: bool, hero: AstronautKind) {
    println!(
        "ASTROBONK headless smoke: {ticks} ticks @33ms{} hero={}",
        if fast_boss { " (fast-boss)" } else { "" },
        hero.def().name
    );
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
    let mut run = RunState::new(hero, PlanetKind::Moon, 1, &save);
    if fast_boss {
        run.timer = 95.0; // just above the boss mark: boss arrives ~5s in
        run.elapsed = 570.0; // late-game spawn mix: beamers, lobbers, UFOs, burrowers
        if let Some(w) = run.weapons.first_mut() {
            w.level = 7;
        }
        run.hp = run.stats.max_hp;
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
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(save)
        .insert_resource(run)
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
                crate::pickups::pickup_update,
                crate::director::run_clock,
                crate::director::levelup_trigger,
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
                crate::pickups::kill_drops,
                crate::combat::fader_update,
                crate::player::player_physics,
                crate::player::player_upkeep,
                crate::fx::update_particles,
                crate::director::stage_transition,
                bot_watchdog,
            ),
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
    let enemies = world.query_filtered::<(), With<Enemy>>().iter(world).count();
    let phase = *world.resource::<RunPhase>();

    println!("--- SMOKE SUMMARY ---");
    println!(
        "phase={phase:?} level={} kills={} gold={} hp={:.0}/{:.0} timer={:.0} enemies={} boss_spawned={} boss_dead={}",
        run.level, run.kills, run.gold, run.hp, run.stats.max_hp, run.timer, enemies, run.boss_spawned, run.boss_dead
    );

    let mut ok = true;
    if matches!(phase, RunPhase::LevelUp | RunPhase::Modal) {
        println!("FAIL: run ended stuck in a panel phase");
        ok = false;
    }
    if run.kills == 0 && !fast_boss {
        println!("FAIL: bot killed nothing");
        ok = false;
    }
    if run.level < 2 && run.kills > 50 {
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
) {
    let planet = CurrentPlanet::from_kind(run_state.planet());
    crate::planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet);
    crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state);
    crate::interact::spawn_interactables(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, Vec3::Y);
    commands.insert_resource(planet);
}
