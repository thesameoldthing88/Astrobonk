mod audio;
mod combat;
mod comet;
mod config;
mod content;
mod director;
mod enemies;
mod events_world;
mod fx;
mod headless;
mod interact;
mod meshkit;
mod messages;
mod music;
mod net;
mod pickups;
mod planet;
mod player;
mod netenemy;
mod remote;
mod run;
mod save;
mod sphere;
mod stats;
mod tutorial;
mod ui;

use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::time::common_conditions::on_timer;
use std::time::Duration;

#[derive(States, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    CharSelect,
    PlanetSelect,
    InRun,
    Results,
}

/// Convenience run-condition: gameplay is live.
pub fn playing(phase: Res<run::RunPhase>) -> bool {
    *phase == run::RunPhase::Playing
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--headless") {
        let ticks: u64 = args.get(pos + 1).and_then(|s| s.parse().ok()).unwrap_or(1500);
        let fast_boss = args.iter().any(|a| a == "--fast-boss");
        let hero = args
            .iter()
            .position(|a| a == "--hero")
            .and_then(|p| args.get(p + 1))
            .and_then(|s| content::characters::AstronautKind::from_name(s))
            .unwrap_or(content::characters::AstronautKind::Buzz);
        let planet = match args.iter().position(|a| a == "--planet").and_then(|p| args.get(p + 1)).map(|s| s.as_str()) {
            Some("mars") => content::planets::PlanetKind::Mars,
            Some("darkmoon") => content::planets::PlanetKind::DarkMoon,
            _ => content::planets::PlanetKind::Moon,
        };
        let seed = args.iter().position(|a| a == "--seed").and_then(|p| args.get(p + 1)).and_then(|s| s.parse().ok());
        headless::run_headless(ticks, fast_boss, hero, planet, seed);
        return;
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ASTROBONK".into(),
                ..default()
            }),
            ..default()
        }))
        // dim enough that the night side is dark and the flashlight earns its keep
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.65, 0.7, 0.9),
            brightness: 80.0,
            ..default()
        })
        .add_plugins(net::NetPlugin)
        .add_plugins(remote::RemoteVisualsPlugin)
        .add_plugins(netenemy::EnemyStreamPlugin)
        .init_state::<AppState>()
        .init_resource::<run::RunPhase>()
        .init_resource::<run::ChoicePanel>()
        .init_resource::<player::CamRig>()
        .init_resource::<fx::Shake>()
        .init_resource::<fx::Hitstop>()
        .init_resource::<enemies::SpatialHash>()
        .init_resource::<enemies::Director>()
        .init_resource::<director::PendingStage>()
        .init_resource::<director::ResultsData>()
        .init_resource::<interact::InteractPrompt>()
        .init_resource::<interact::ChestPanel>()
        .init_resource::<interact::ShopPanel>()
        .init_resource::<comet::Comet>()
        .init_resource::<run::GameRng>()
        .init_resource::<planet::PropColliders>()
        .init_resource::<tutorial::Tutorial>()
        .init_resource::<events_world::DustStorm>()
        .init_resource::<ui::settings::SettingsOpen>()
        .init_resource::<ui::menus::Selected>()
        .init_resource::<ui::menus::MenuTab>()
        .init_resource::<ui::hud::BannerQueue>()
        .init_resource::<audio::SfxThrottle>()
        .add_message::<messages::HitMsg>()
        .add_message::<messages::PlayerHitMsg>()
        .add_message::<messages::KillMsg>()
        .add_message::<messages::NumberMsg>()
        .add_message::<messages::BannerMsg>()
        .add_message::<messages::SfxMsg>()
        .add_systems(
            Startup,
            (
                setup_camera,
                fx::setup_particles,
                enemies::setup_enemy_assets,
                combat::setup_weapon_assets,
                pickups::setup_pickup_assets,
                audio::build_sfx_bank,
                music::build_music_bank,
                ui::numbers::spawn_number_pool,
                boot,
            ),
        )
        // ------------- state flow
        .add_systems(OnEnter(AppState::MainMenu), ui::menus::spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::CharSelect), ui::menus::spawn_char_select)
        .add_systems(OnExit(AppState::CharSelect), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::PlanetSelect), ui::menus::spawn_planet_select)
        .add_systems(OnExit(AppState::PlanetSelect), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::InRun), (enter_run, ui::hud::spawn_hud, music::start_music))
        .add_systems(
            OnExit(AppState::InRun),
            (planet::despawn_stage, ui::hud::despawn_hud, clear_panels, music::stop_music),
        )
        .add_systems(
            OnEnter(AppState::Results),
            (
                // A client must never bank a run it did not simulate: its RunState is
                // adopted from the host, so this would write the HOST's kills and silver
                // into the joiner's own save file.
                director::bank_results.run_if(net::is_simulating),
                ui::menus::spawn_results,
            )
                .chain(),
        )
        .add_systems(OnExit(AppState::Results), ui::menus::despawn_menu)
        // ------------- menu inputs
        .add_systems(Update, client_follow_host_run)
        .add_systems(Update, dev_fast_boss.run_if(in_state(AppState::InRun)))
        .add_systems(
            Update,
            dev_stage_now
                .run_if(in_state(AppState::InRun))
                .run_if(|| std::env::args().any(|a| a == "--stagenow")),
        )
        .add_systems(
            Update,
            dev_autopick
                .run_if(in_state(AppState::InRun))
                .run_if(|| std::env::args().any(|a| a == "--autopick")),
        )
        .add_systems(Update, ui::menus::main_menu_input.run_if(in_state(AppState::MainMenu)))
        .add_systems(Update, ui::menus::char_select_input.run_if(in_state(AppState::CharSelect)))
        .add_systems(Update, ui::menus::planet_select_input.run_if(in_state(AppState::PlanetSelect)))
        .add_systems(Update, ui::menus::results_input.run_if(in_state(AppState::Results)))
        // ------------- live simulation (only while actually playing)
        .add_systems(
            Update,
            (
                enemies::rebuild_hash,
                enemies::director_spawn.run_if(net::is_simulating),
                // enemy_move MUST be gated: streamed proxies carry a real Enemy, so on a
                // client this would steer them with local AI and fight drive_proxies for
                // the transform.
                enemies::enemy_move.run_if(net::is_simulating),
                // KEPT on clients: this places the streamed worm's 12 segments from the
                // head's trail. It writes PlayerHitMsg, which is inert now that
                // apply_player_hits is host-only.
                enemies::craterpillar_update,
                enemies::anubot_beam_system.run_if(net::is_simulating),
                enemies::boss_phase_system.run_if(net::is_simulating),
                enemies::burrower_emerge.run_if(net::is_simulating),
                enemies::enemy_contact.run_if(net::is_simulating),
                enemies::spitter_attack.run_if(net::is_simulating),
                enemies::beamer_attack.run_if(net::is_simulating),
                enemies::lobber_attack.run_if(net::is_simulating),
                // KEPT on clients: these three integrate the hazards the host streamed as
                // spawn events. Gating them would freeze every shot and telegraph mid-air.
                enemies::mortar_shells,
                enemies::enemy_projectiles,
                enemies::boss_attacks.run_if(net::is_simulating),
                enemies::telegraphs,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (
                player::gather_local_input,
                player::player_input,
                combat::weapon_fire,
                combat::projectile_move,
                combat::drone_update,
                combat::beam_update,
                combat::aura_follow,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (
                interact::charge_shrines.run_if(net::is_simulating),
                // Interactables are host-resolved. A joiner pressing E is a no-op today —
                // the same documented gap as a peer on the host.
                interact::interact_system.run_if(net::is_simulating),
                // Collection and the XP grant are the host's; a client animates its
                // streamed loot with netenemy::animate_net_pickups instead. Left ungated
                // this would collect locally and DOUBLE the XP a joiner receives.
                pickups::pickup_update.run_if(net::is_simulating),
                // The clock, boss marks and teleporter belong to the host — a client
                // adopts them from RunSnapMsg instead of running a second, drifting copy.
                director::run_clock.run_if(net::is_simulating),
                director::levelup_trigger,
                enemies::debug_spawn_boss,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (comet::comet_system, events_world::dust_storm_system).run_if(net::is_simulating)
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            pickups::gem_merge.run_if(net::is_simulating).run_if(
                in_state(AppState::InRun)
                    .and(playing)
                    .and(on_timer(Duration::from_secs(1))),
            ),
        )
        // ------------- consumers + always-on-in-run
        .add_systems(
            Update,
            (
                combat::apply_hits.run_if(net::is_simulating),
                combat::apply_player_hits.run_if(net::is_simulating),
                // Loot is rolled by the host (it owns run.kills and the luck roll) and
                // reaches clients as PickupEvent spawns.
                pickups::kill_drops.run_if(net::is_simulating),
                combat::fader_update,
                enemies::enemy_flash,
                player::player_physics,
                player::animate_player,
                // Regen, i-frames, shield recharge and powerup decay are all host-owned
                // per-player state. A client adopts its own hp from the replicated
                // PlayerVitals (net::adopt_my_vitals) instead of regenerating locally.
                player::player_upkeep.run_if(net::is_simulating),
                fx::update_particles,
                director::stage_transition,
                // Run-end is the host's call. On a client `all(dead)` is a one-element
                // check over its own sheet and fires while the host plays on.
                director::downed_watch.run_if(net::is_simulating),
                director::death_watch.run_if(net::is_simulating),
            )
                .run_if(in_state(AppState::InRun)),
        )
        // ------------- in-run UI
        .add_systems(
            Update,
            (
                ui::hud::update_hud,
                ui::hud::update_weapon_row,
                ui::hud::update_boss_bar,
                ui::hud::update_comet_hud,
                ui::hud::update_dust_overlay,
                ui::hud::update_edge_markers,
                tutorial::tutorial_system.run_if(playing),
                ui::hud::update_banners,
                ui::panels::sync_choice_panel,
                ui::panels::choice_input,
                ui::panels::chest_panel,
                ui::panels::shop_panel,
                ui::panels::pause_panel,
            )
                .run_if(in_state(AppState::InRun)),
        )
        .add_systems(
            Update,
            (ui::numbers::claim_numbers, ui::numbers::update_numbers).chain(),
        )
        .add_systems(
            PostUpdate,
            player::camera_rig
                .run_if(in_state(AppState::InRun))
                .before(TransformSystems::Propagate),
        )
        // ------------- global
        .add_systems(
            Update,
            (
                player::cursor_control,
                fx::shake_decay,
                fx::hitstop_system,
                fx::phase_time_control,
                audio::play_sfx,
                music::update_music.run_if(in_state(AppState::InRun)),
                // runs after pause_panel so a single ESC closes settings without also resuming
                ui::settings::settings_panel.after(ui::panels::pause_panel),
                ui::button_hover,
            ),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::NATURAL,
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        Transform::from_xyz(0.0, 140.0, 220.0).looking_at(Vec3::ZERO, Vec3::Y),
        player::PlayerRig,
    ));
}

/// CLIENT: don't enter the run until the host's seed has arrived.
///
/// The joiner's own RunState still holds a random local seed, so building the world early
/// would place every rock, pot, chest and shrine somewhere the host never put them — and
/// the streamed horde would walk through scenery that isn't there. Waiting is also what
/// keeps `enter_run` a single code path: by the time it runs, the seed is already correct.
fn client_follow_host_run(
    role: Res<net::NetRole>,
    sync: Res<net::RunSync>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut announced: Local<bool>,
) {
    if !matches!(*role, net::NetRole::Client) {
        return;
    }
    if !sync.seeded {
        if !*announced {
            *announced = true;
            info!("NET waiting for the host's run seed before building the world");
        }
        return;
    }
    // Only pull IN from a menu state. Without this guard the client fights death_watch and
    // results_input for NextState and ping-pongs between Results and a rebuilt world.
    if matches!(*state.get(), AppState::MainMenu | AppState::Boot) {
        next.set(AppState::InRun);
    }
}

/// Load the save; a placeholder RunState keeps Res<RunState> alive in menus.
fn boot(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    let save = save::MetaSave::load();
    // --stagenow needs a MULTI-planet chain to advance into: tier 1 is Moon-only, so
    // advancing from it hits the victory branch instead. Tier 3 gives Moon -> Mars ->
    // DarkMoon, which also exercises the planet RADIUS change (140 -> 160) that would
    // otherwise decode the streamed horde at the wrong arc scale.
    let dev_tier = if std::env::args().any(|a| a == "--stagenow") { 3 } else { 1 };
    let run_state = run::RunState::new(
        content::characters::AstronautKind::Buzz,
        content::planets::PlanetKind::Moon,
        dev_tier,
        &save,
    );
    commands.insert_resource(save);
    commands.insert_resource(run_state);
    // Dev/co-op harness: drop straight into a run so two instances can be tested
    // without a human clicking through menus in each window.
    // A client never self-starts: `client_follow_host_run` enters the run once the host's
    // seed lands, so both machines build the same planet.
    let joining = std::env::args().any(|a| a == "--join");
    if std::env::args().any(|a| a == "--autodrop") && !joining {
        next.set(AppState::InRun);
    } else {
        next.set(AppState::MainMenu);
    }
}

fn enter_run(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    run_state: Res<run::RunState>,
    save: Res<save::MetaSave>,
    mut director_res: ResMut<enemies::Director>,
    mut phase: ResMut<run::RunPhase>,
    mut comet_res: ResMut<comet::Comet>,
    mut game_rng: ResMut<run::GameRng>,
    mut tut: ResMut<tutorial::Tutorial>,
    role: Res<net::NetRole>,
    mut sync: ResMut<net::RunSync>,
) {
    *comet_res = comet::Comet::default();
    // first-run onboarding, only for a brand-new player on a normal run
    *tut = tutorial::Tutorial {
        active: !save.tutorial_done && !run_state.is_daily,
        step: 0,
        timer: 0.0,
    };
    sync.world_built = true;
    let _ = &role;
    // seed the run's deterministic RNG streams from the run seed + stage
    let stage_seed = run_state.run_seed.wrapping_add(run_state.stage as u64);
    game_rng.reseed(stage_seed);
    let planet = planet::CurrentPlanet::from_kind(run_state.planet());
    let props = planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    commands.insert_resource(props);
    player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 0, run_state.character, true, None);
    interact::spawn_interactables(
        &mut commands,
        &mut meshes,
        &mut materials,
        &planet,
        &run_state,
        &run::PlayerState::new(run_state.character, &save),
        &save,
        Vec3::Y,
    );
    commands.insert_resource(planet);
    *director_res = enemies::Director::default();
    *phase = run::RunPhase::Playing;
}

/// `--autopick`: take option 1 of every card panel automatically. Without this a bot-driven
/// client stalls forever on its first level-up (the panel opens and waits for a keypress
/// that never comes), which also freezes the run — so co-op builds can never diverge and
/// build sync is untestable.
fn dev_autopick(
    mut phase: ResMut<run::RunPhase>,
    mut panel: ResMut<run::ChoicePanel>,
    save: Res<save::MetaSave>,
    global: Res<run::RunState>,
    mut chest: ResMut<interact::ChestPanel>,
    mut shop: ResMut<interact::ShopPanel>,
    mut q: Query<&mut run::PlayerState, With<player::LocalPlayer>>,
) {
    if !matches!(*phase, run::RunPhase::LevelUp | run::RunPhase::Modal) {
        return;
    }
    if let Ok(mut ps) = q.single_mut() {
        if !panel.options.is_empty() {
            let opt = panel.options[0].clone();
            ps.apply_upgrade(&opt, &save, global.greed_stacks);
            if panel.is_levelup {
                ps.pending_levelups = ps.pending_levelups.saturating_sub(1);
            }
            panel.options.clear();
        }
    }
    chest.open = false;
    shop.open = false;
    *phase = run::RunPhase::Playing;
}

/// `--stagenow`: host-side dev trigger that advances a stage ~20s in, so the client's
/// world rebuild can be tested without killing a boss and walking to a teleporter first.
fn dev_stage_now(
    time: Res<Time>,
    role: Res<net::NetRole>,
    run: Res<run::RunState>,
    mut pending: ResMut<director::PendingStage>,
    mut fired: Local<f32>,
) {
    if matches!(*role, net::NetRole::Client) || pending.0.is_some() {
        return;
    }
    *fired += time.delta_secs();
    if *fired > 20.0 && run.stage == 0 {
        *fired = -1.0e9; // once
        pending.0 = Some(1);
        info!("DEV --stagenow: advancing to stage 2");
    }
}

/// `--bossnow`: wind the clock to just before the boss mark so the boss lane can be tested
/// live without waiting out a full stage. Host/solo only — a client adopts the host's clock.
fn dev_fast_boss(
    mut run: ResMut<run::RunState>,
    role: Res<net::NetRole>,
    mut done: Local<bool>,
) {
    if *done || matches!(*role, net::NetRole::Client) {
        return;
    }
    if std::env::args().any(|a| a == "--bossnow") {
        run.timer = config::BOSS_MARK + 4.0;
        // Winding past MINIBOSS_MARKS would fire BOTH minibosses on the next tick as well,
        // burying a level-1 test player under three bosses at once. Mark them done.
        run.minibosses_spawned = [true; 2];
        info!("DEV --bossnow: clock wound to {:.0}s (minibosses skipped)", run.timer);
    }
    *done = true;
}

/// Close any leftover modal state when leaving a run.
fn clear_panels(
    mut panel: ResMut<run::ChoicePanel>,
    mut chest: ResMut<interact::ChestPanel>,
    mut shop: ResMut<interact::ShopPanel>,
    mut prompt: ResMut<interact::InteractPrompt>,
    mut banners: ResMut<ui::hud::BannerQueue>,
) {
    panel.options.clear();
    chest.open = false;
    shop.open = false;
    prompt.0 = None;
    banners.current = None;
    banners.queue.clear();
}
