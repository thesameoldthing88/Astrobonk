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
            (director::bank_results, ui::menus::spawn_results).chain(),
        )
        .add_systems(OnExit(AppState::Results), ui::menus::despawn_menu)
        // ------------- menu inputs
        .add_systems(Update, ui::menus::main_menu_input.run_if(in_state(AppState::MainMenu)))
        .add_systems(Update, ui::menus::char_select_input.run_if(in_state(AppState::CharSelect)))
        .add_systems(Update, ui::menus::planet_select_input.run_if(in_state(AppState::PlanetSelect)))
        .add_systems(Update, ui::menus::results_input.run_if(in_state(AppState::Results)))
        // ------------- live simulation (only while actually playing)
        .add_systems(
            Update,
            (
                enemies::rebuild_hash,
                enemies::director_spawn,
                enemies::enemy_move,
                enemies::craterpillar_update,
                enemies::anubot_beam_system,
                enemies::boss_phase_system,
                enemies::burrower_emerge,
                enemies::enemy_contact,
                enemies::spitter_attack,
                enemies::beamer_attack,
                enemies::lobber_attack,
                enemies::mortar_shells,
                enemies::enemy_projectiles,
                enemies::boss_attacks,
                enemies::telegraphs,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (
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
                interact::charge_shrines,
                interact::interact_system,
                pickups::pickup_update,
                director::run_clock,
                director::levelup_trigger,
                enemies::debug_spawn_boss,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (comet::comet_system, events_world::dust_storm_system)
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            pickups::gem_merge.run_if(
                in_state(AppState::InRun)
                    .and(playing)
                    .and(on_timer(Duration::from_secs(1))),
            ),
        )
        // ------------- consumers + always-on-in-run
        .add_systems(
            Update,
            (
                combat::apply_hits,
                combat::apply_player_hits,
                pickups::kill_drops,
                combat::fader_update,
                enemies::enemy_flash,
                player::player_physics,
                player::animate_player,
                player::player_upkeep,
                fx::update_particles,
                director::stage_transition,
                director::death_watch,
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

/// Load the save; a placeholder RunState keeps Res<RunState> alive in menus.
fn boot(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    let save = save::MetaSave::load();
    let run_state = run::RunState::new(
        content::characters::AstronautKind::Buzz,
        content::planets::PlanetKind::Moon,
        1,
        &save,
    );
    commands.insert_resource(save);
    commands.insert_resource(run_state);
    next.set(AppState::MainMenu);
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
) {
    *comet_res = comet::Comet::default();
    // first-run onboarding, only for a brand-new player on a normal run
    *tut = tutorial::Tutorial {
        active: !save.tutorial_done && !run_state.is_daily,
        step: 0,
        timer: 0.0,
    };
    // seed the run's deterministic RNG streams from the run seed + stage
    let stage_seed = run_state.run_seed.wrapping_add(run_state.stage as u64);
    game_rng.reseed(stage_seed);
    let planet = planet::CurrentPlanet::from_kind(run_state.planet());
    let props = planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    commands.insert_resource(props);
    player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 0, run_state.character, true);
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
