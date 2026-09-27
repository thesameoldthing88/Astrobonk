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
mod items;
mod meshkit;
mod messages;
mod music;
mod net;
mod pickups;
mod planet;
mod player;
mod netenemy;
mod playlog;
mod remote;
mod run;
mod save;
mod sphere;
mod stats;
mod techs;
mod tomes;
mod toon;
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
    /// The tome library (§7 loadout), reached from the main menu.
    Tomes,
    InRun,
    Results,
}

/// Convenience run-condition: gameplay is live.
pub fn playing(phase: Res<run::RunPhase>) -> bool {
    *phase == run::RunPhase::Playing
}

/// A test-harness flag that bends a run (winds the clock, hands out loot) counts only when
/// `--dev` is passed with it, so a shipped binary can't be talked into it (CLAUDE.md rule
/// 10). P28 routes the older harness flags (`--bossnow`, `--stagenow`, …) through here too.
pub fn dev_flag(name: &str) -> bool {
    dev_mode() && std::env::args().any(|a| a == name)
}

/// `--dev` was passed: the dev KEYS (B summons the stage boss, T replays the tutorial) and
/// the `dev_flag` harness flags are live. Read once — it is a run condition, checked every
/// frame.
pub fn dev_mode() -> bool {
    static DEV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *DEV.get_or_init(|| std::env::args().any(|a| a == "--dev"))
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
        // dim enough that the night side is dark and the flashlight earns its keep (each
        // world re-grades it from its `ToonLook`, `toon::apply_world_look`)
        .insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.65, 0.7, 0.9),
            brightness: 80.0,
            ..default()
        })
        // space, not Bevy's default mid-gray, behind the menus too (each world then sets its
        // own sky)
        .insert_resource(ClearColor(content::planets::PlanetKind::Moon.def().sky))
        .add_plugins(toon::ToonPlugin)
        .add_plugins(net::NetPlugin)
        .add_plugins(remote::RemoteVisualsPlugin)
        .add_plugins(netenemy::EnemyStreamPlugin)
        .add_plugins(playlog::PlayLogPlugin)
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
        .init_resource::<ui::settings::SettingsTab>()
        .init_resource::<ui::MenuFit>()
        .init_resource::<fx::FlashGate>()
        .init_resource::<fx::ScreenFlash>()
        .init_resource::<ui::menus::Selected>()
        .init_resource::<ui::menus::MenuTab>()
        .init_resource::<ui::hud::BannerQueue>()
        .init_resource::<audio::SfxThrottle>()
        .init_resource::<items::ItemTelemetry>()
        .init_resource::<techs::GrindLines>()
        .init_resource::<techs::TechTelemetry>()
        .init_resource::<player::FlashlightSwitch>()
        .add_message::<items::ItemFxMsg>()
        .add_message::<techs::TechFxMsg>()
        .add_message::<messages::HitMsg>()
        .add_message::<messages::SlowMsg>()
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
                items::setup_item_assets,
                techs::setup_tech_assets,
                audio::build_sfx_bank,
                music::build_music_bank,
                ui::numbers::spawn_number_pool,
                fx::spawn_screen_flash,
                boot,
            ),
        )
        // ------------- state flow
        .add_systems(OnEnter(AppState::MainMenu), ui::menus::spawn_main_menu)
        .add_systems(OnExit(AppState::MainMenu), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::Tomes), ui::menus::spawn_tome_library)
        .add_systems(OnExit(AppState::Tomes), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::CharSelect), ui::menus::spawn_char_select)
        .add_systems(OnExit(AppState::CharSelect), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::PlanetSelect), ui::menus::spawn_planet_select)
        .add_systems(OnExit(AppState::PlanetSelect), ui::menus::despawn_menu)
        .add_systems(OnEnter(AppState::InRun), (enter_run, ui::hud::spawn_hud, music::start_music))
        .add_systems(
            OnExit(AppState::InRun),
            (
                planet::despawn_stage,
                ui::hud::despawn_hud,
                ui::panels::despawn_panels,
                clear_panels,
                music::stop_music,
            ),
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
            dev_levelup_now
                .run_if(in_state(AppState::InRun))
                .run_if(|| dev_flag("--levelupnow")),
        )
        .add_systems(
            Update,
            dev_give_weapons
                .run_if(in_state(AppState::InRun))
                .run_if(|| dev_flag("--give")),
        )
        .add_systems(
            Update,
            dev_stage_now
                .run_if(in_state(AppState::InRun))
                .run_if(|| std::env::args().any(|a| a == "--stagenow")),
        )
        .add_systems(
            Update,
            director::dev_miniboss_now
                .run_if(net::is_simulating)
                .run_if(in_state(AppState::InRun).and(playing))
                .run_if(|| dev_flag("--minibossnow")),
        )
        .add_systems(
            Update,
            dev_autopick
                .run_if(in_state(AppState::InRun))
                .run_if(|| std::env::args().any(|a| a == "--autopick")),
        )
        .init_resource::<ui::menus::JoinAddr>()
        .init_resource::<ui::menus::JoinOpen>()
        .init_resource::<ui::menus::CoopNote>()
        .add_systems(Update, ui::menus::main_menu_input.run_if(in_state(AppState::MainMenu)))
        .add_systems(
            Update,
            (ui::menus::join_panel_sync, ui::menus::join_addr_input)
                .run_if(in_state(AppState::MainMenu)),
        )
        .add_systems(
            Update,
            (ui::menus::tome_library_input, ui::menus::refresh_tome_library)
                .chain()
                .run_if(in_state(AppState::Tomes)),
        )
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
                // KEPT on clients: the beam's pose and slab, drawn from the streamed boss
                // (after drive_boss_proxies has placed the proxy this frame).
                enemies::anubot_beam_visuals.after(netenemy::drive_boss_proxies),
                enemies::boss_phase_system.run_if(net::is_simulating),
                enemies::burrower_emerge.run_if(net::is_simulating),
                enemies::enemy_contact.run_if(net::is_simulating),
                enemies::spitter_attack.run_if(net::is_simulating),
                enemies::beamer_attack.run_if(net::is_simulating),
                // KEPT on clients: aim lines rebuilt from the hazard lane draw here too.
                enemies::aim_line_visuals.after(netenemy::drive_net_aim_lines),
                enemies::lobber_attack.run_if(net::is_simulating),
                // KEPT on clients: these three integrate the hazards the host streamed as
                // spawn events. Gating them would freeze every shot and telegraph mid-air.
                enemies::mortar_shells,
                enemies::crack_decals,
                enemies::enemy_projectiles,
                enemies::boss_attacks.run_if(net::is_simulating),
                enemies::telegraphs,
                // PRESENTATION (§13): the readable-without-color parts of every hazard.
                enemies::animate_hazard_decor,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (
                // Presentation only, on every machine: children for new hazards, and the
                // viewer's palette / flash settings on the shared hazard materials.
                enemies::decorate_hazards,
                enemies::apply_danger_palette,
                combat::apply_weapon_photosensitivity,
            )
                .run_if(in_state(AppState::InRun)),
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
                // M14: a dev key, not a shipped one (B is also the level-up Banish key)
                enemies::debug_spawn_boss.run_if(net::is_simulating).run_if(dev_mode),
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        .add_systems(
            Update,
            (
                // The host computes every astronaut's combo and owns the storm; what they
                // look like runs everywhere, from the replicated/streamed state.
                comet::comet_system.run_if(net::is_simulating),
                comet::comet_presentation,
                events_world::dust_storm_sim.run_if(net::is_simulating),
                events_world::dust_storm_visuals,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        // ------------- §7 items: the host simulates what they DO for every astronaut; every
        // machine draws them (from NetItemVis and the hazard lane's item events)
        .add_systems(
            Update,
            (
                items::item_upkeep.run_if(net::is_simulating),
                items::encirclement_scan,
                items::orbital_yoyo.run_if(net::is_simulating),
                items::comet_tail.run_if(net::is_simulating),
                items::little_black_hole.run_if(net::is_simulating),
                items::orbit_chunks,
                items::trail_patches,
                items::trail_client_drops.run_if(net::is_client),
                items::singularity_update,
                items::push_net_item_vis.run_if(net::is_simulating),
                items::item_visuals,
                items::apply_sun_shrink,
            )
                .chain()
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        // Item one-shots are presented even behind a card panel: a joiner picking a level-up
        // is still being hunted on the host, and a death-save that fires meanwhile must not
        // expire unread (messages live two updates) — no banner, and no snap of the
        // predicted body to where the host rewound it.
        .add_systems(
            Update,
            items::item_fx_presentation
                .after(items::apply_sun_shrink)
                .run_if(in_state(AppState::InRun)),
        )
        .add_systems(
            Update,
            dev_grant_items
                .run_if(in_state(AppState::InRun))
                .run_if(|| dev_flag("--items")),
        )
        // `--dev --warp noon|dusk|night`: the toon look's windowed checks (P36) — every run
        // lands on the night side, so the lit side needs a walk the bot takes minutes over.
        .add_systems(
            Update,
            dev_warp
                .run_if(net::is_simulating)
                .run_if(in_state(AppState::InRun).and(playing))
                .run_if(|| dev_flag("--warp")),
        )
        // ------------- §4 movement techs: the moves themselves are player_input/physics on
        // every body a machine moves; what they do to the WORLD (a blink, the Slam's
        // shockwave, the slide's plow, the antipode read) is the host's, for every
        // astronaut; every machine draws them (NetTransform, NetItemVis, the hazard lane).
        .add_systems(
            Update,
            (techs::antipode_blink.run_if(net::is_simulating), techs::blink_denied_feedback)
                .chain()
                .after(player::player_input)
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        // After the last reader of the edge intents: every edge lives one frame. Behind a
        // panel too — a joiner's press that lands while the host's run is held is dropped,
        // as its own client drops presses behind its own panels, not fired late on resume.
        .add_systems(
            Update,
            techs::consume_edge_intents
                .after(player::player_input)
                .after(techs::antipode_blink)
                .after(techs::blink_denied_feedback)
                .run_if(in_state(AppState::InRun)),
        )
        .add_systems(
            Update,
            (
                techs::antipode_scan.run_if(net::is_simulating),
                techs::slam_shockwave.run_if(net::is_simulating).after(player::player_physics),
                techs::slide_plow.run_if(net::is_simulating),
                techs::grind_sparks,
                techs::grind_hint,
                techs::animate_tech_fx,
                player::sync_flashlights,
            )
                .run_if(in_state(AppState::InRun).and(playing)),
        )
        // Presented behind a card panel too, like the item one-shots: a joiner's blink the
        // host resolves meanwhile must still turn its predicted body.
        .add_systems(
            Update,
            techs::tech_fx_presentation
                .after(techs::slam_shockwave)
                .after(techs::antipode_blink)
                .run_if(in_state(AppState::InRun)),
        )
        .add_systems(
            Update,
            techs::dev_tech_bot
                .after(player::gather_local_input)
                .after(net::bot_input)
                .before(net::send_local_input)
                .before(player::player_input)
                .run_if(in_state(AppState::InRun).and(playing))
                .run_if(|| dev_flag("--techbot")),
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
                // after this frame's jump/slide presses are applied, never before (L3)
                player::player_physics.after(player::player_input),
                player::refit_astronaut_rigs,
                player::animate_player,
                // Regen, i-frames, shield recharge and powerup decay are all host-owned
                // per-player state. A client adopts its own hp from the replicated
                // PlayerVitals (net::adopt_my_vitals) instead of regenerating locally.
                player::player_upkeep.run_if(net::is_simulating),
                fx::update_particles,
                director::stage_transition,
                // Host AND client: spawns/pops the miniboss cache from RunState, which a
                // client adopts from RunSnapMsg.
                interact::sync_reward_cache.before(director::stage_transition),
                // Run-end is the host's call. On a client `all(dead)` is a one-element
                // check over its own sheet and fires while the host plays on.
                director::downed_watch.run_if(net::is_simulating),
                director::death_watch.run_if(net::is_simulating),
                // what Results banks, kept on RunState: the astronaut is gone by then (H1)
                director::snapshot_local_sheet,
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
                ui::hud::update_item_status,
                ui::hud::update_dust_overlay,
                ui::hud::update_edge_markers,
                ui::hud::update_assist_hud,
                ui::hud::update_antipode_dial,
                ui::hud::hide_mid_hud_under_panels,
                ui::hud::keep_panels_clear_of_hud,
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
        // Tome of Nightfall's beam on every astronaut drawn, dark while its F switch is off
        // (presentation: not in headless). After the switch is read, so a toggle lands the
        // same frame.
        .add_systems(
            Update,
            tomes::apply_flashlights
                .after(player::sync_flashlights)
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
                ui::settings::apply_ui_scale,
                fx::apply_fx_settings,
                fx::update_screen_flash,
                // The host's (or solo player's) assist options are the run's; a client
                // adopts the host's from RunSnapMsg instead.
                director::sync_assist_options.run_if(net::is_simulating),
            ),
        )
        // UI layout helpers for every screen (§13 UI scale): shrink-to-fit, wheel-scrolled
        // lists and their "scroll for more" hints.
        .add_systems(
            Update,
            (
                ui::fit_panels,
                ui::fit_menus.before(ui::settings::apply_ui_scale),
                ui::wheel_scroll,
                ui::scroll_hints,
            ),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom {
            intensity: config::BLOOM_INTENSITY,
            prefilter: bevy::post_process::bloom::BloomPrefilter {
                threshold: config::BLOOM_THRESHOLD,
                threshold_softness: config::BLOOM_THRESHOLD_SOFTNESS,
            },
            ..Bloom::NATURAL
        },
        bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
        // the toon look: prepasses for the ink, cel shadows, the per-world grade
        toon::camera_bundle(),
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
    mine: Res<net::MyPlayerId>,
    state: Res<State<AppState>>,
    mut next: ResMut<NextState<AppState>>,
    mut announced: Local<bool>,
) {
    if !matches!(*role, net::NetRole::Client) {
        *announced = false;
        return;
    }
    // The player id too, not just the seed: our astronaut drops at OUR slot's spot around
    // the landing site — the same spot the host seats our server-side body — so the two
    // start in the same place instead of a few metres apart for the whole run.
    if !sync.seeded || mine.0.is_none() {
        if !*announced {
            *announced = true;
            info!("NET waiting for the host's run seed before building the world");
        }
        return;
    }
    // Only pull IN from a menu state. Without this guard the client fights death_watch and
    // results_input for NextState and ping-pongs between Results and a rebuilt world. The
    // tome library counts: a joiner re-slotting tomes while the host picks a world would
    // otherwise stay there while its server-side body stood idle in the host's run.
    if matches!(*state.get(), AppState::MainMenu | AppState::Tomes | AppState::Boot) {
        next.set(AppState::InRun);
    }
}

/// Load the save; a placeholder RunState keeps Res<RunState> alive in menus.
fn boot(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    let mut save = save::MetaSave::load();
    // --stagenow needs a MULTI-planet chain to advance into: tier 1 is Moon-only, so
    // advancing from it hits the victory branch instead. Tier 3 gives Moon -> Mars ->
    // DarkMoon, which also exercises the planet RADIUS change (140 -> 160) that would
    // otherwise decode the streamed horde at the wrong arc scale.
    let dev_tier = if std::env::args().any(|a| a == "--stagenow") { 3 } else { 1 };
    // Harness flags for the windowed/co-op tests, mirroring the headless ones: which hero
    // this instance plays (a joiner's own pick is what the host must seat), and which world
    // an --autodrop run lands on.
    let args: Vec<String> = std::env::args().collect();
    let arg = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let hero = arg("--hero")
        .and_then(|s| content::characters::AstronautKind::from_name(&s))
        .unwrap_or(content::characters::AstronautKind::Buzz);
    let planet = match arg("--planet").as_deref() {
        Some("mars") => content::planets::PlanetKind::Mars,
        Some("darkmoon") => content::planets::PlanetKind::DarkMoon,
        _ => content::planets::PlanetKind::Moon,
    };
    dev_settings(&mut save, &args);
    let run_state = run::RunState::new(hero, planet, dev_tier, &save);
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

/// Test harness (needs `--dev`, CLAUDE.md rule 10) for the §13 settings, so a windowed or
/// two-instance co-op run can exercise them without clicking through the settings panel in
/// every window:
///   `--dev --assist`       density 50%, damage 50%, one more chance
///   `--dev --a11y LIST`    comma list of deut|prot|trit, outline, flash, photo, ui=PCT,
///                          numbers=full|merged|crits|off, numsize=X
///   `--dev --tomes LIST [--tome-rank N]`  slot these tomes (or `all`) at rank N (default
///                          max), as the headless `--tomes` probe does
/// In memory only; the save on disk changes only if the settings panel saves over it (or,
/// for the dev tomes, a finished run banks its Silver).
fn dev_settings(save: &mut save::MetaSave, args: &[String]) {
    if dev_flag("--assist") {
        save.assist = save::AssistOptions { enemy_density: 0.5, enemy_damage: 0.5, revive_token: true };
    }
    if dev_flag("--tomes") {
        let (tomes, rank) = tomes::tomes_from_args();
        let slotted = tomes::save_with(&tomes, rank);
        save.tome_levels.extend(slotted.tome_levels);
        save.tome_loadout = slotted.tome_loadout;
        save.tome_slots = save.tome_slots.max(slotted.tome_slots);
        info!("DEV tomes: {:?} at rank {rank}", save.tome_loadout);
    }
    if !dev_flag("--a11y") {
        return;
    }
    let list = args.iter().position(|a| a == "--a11y").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let a = &mut save.accessibility;
    for tok in list.split(',') {
        use content::palettes::Palette;
        match tok.split_once('=') {
            Some(("ui", v)) => a.ui_scale = v.parse::<f32>().map(|p| p / 100.0).unwrap_or(1.0),
            Some(("numsize", v)) => a.number_size = v.parse().unwrap_or(1.0),
            Some(("numbers", v)) => {
                a.numbers = match v {
                    "merged" => save::NumberMode::Merged,
                    "crits" => save::NumberMode::CritsOnly,
                    "off" => save::NumberMode::Off,
                    _ => save::NumberMode::Full,
                }
            }
            _ => match tok {
                "deut" => a.palette = Palette::Deuteranopia,
                "prot" => a.palette = Palette::Protanopia,
                "trit" => a.palette = Palette::Tritanopia,
                "outline" => a.high_contrast = true,
                "flash" => a.flash_reduction = true,
                "photo" => a.photosensitive = true,
                _ => {}
            },
        }
    }
    a.ui_scale = a.ui_scale.clamp(config::UI_SCALE_MIN, config::UI_SCALE_MAX);
    a.number_size = a.number_size.clamp(config::NUMBER_SIZE_MIN, config::NUMBER_SIZE_MAX);
    info!("DEV settings: {:?} {:?}", save.accessibility, save.assist);
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
    mine: Res<net::MyPlayerId>,
    mut storm: ResMut<events_world::DustStorm>,
    mut banners: MessageWriter<messages::BannerMsg>,
) {
    // M20: the host's address, where it will be read — its own HUD, as the run opens
    if *role == net::NetRole::Host {
        banners.write(messages::BannerMsg(format!("HOSTING: TEAMMATES JOIN AT {}", net::local_ip())));
    }
    *comet_res = comet::Comet::default();
    *storm = events_world::DustStorm::default();
    // first-run onboarding, only for a brand-new player on a normal run
    *tut = tutorial::Tutorial {
        active: !save.tutorial_done && !run_state.is_daily,
        step: 0,
        timer: 0.0,
    };
    sync.world_built = true;
    sync.built_for = Some((run_state.run_seed, run_state.stage));
    // A joiner stands in its OWN slot (client_follow_host_run waits for the id), which is
    // where the host seats its server-side body.
    let my_slot = if *role == net::NetRole::Client { mine.0.unwrap_or(0) } else { 0 };
    // seed the run's deterministic RNG streams from the run seed + stage
    let stage_seed = run_state.run_seed.wrapping_add(run_state.stage as u64);
    game_rng.reseed(stage_seed);
    let planet = planet::CurrentPlanet::from_kind(run_state.planet());
    let (props, rails) = planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, my_slot, run_state.character, true, None);
    interact::spawn_interactables(
        &mut commands,
        &mut meshes,
        &mut materials,
        &planet,
        &run_state,
        &run::PlayerState::new(run_state.character, &save),
        &save,
        &rails,
        &props,
        Vec3::Y,
    );
    commands.insert_resource(props);
    commands.insert_resource(rails);
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
        // ...and the spawn mix with it: a real boss arrives ~8 minutes in, with Beamers,
        // Burrowers and UFOs in the horde. Without this the boss is fought among a
        // minute-one crowd of shamblers, which tests none of the late-game lanes.
        run.elapsed = config::STAGE_SECONDS[0] - run.timer;
        // ...and the §3 run-time scaling term (run::scaling reads total_elapsed), exactly
        // as the headless --fast-boss and --minibossnow wind it.
        run.total_elapsed = run.total_elapsed.max(run.elapsed);
        // Winding past MINIBOSS_MARKS would fire BOTH minibosses on the next tick as well,
        // burying a level-1 test player under three bosses at once. Mark them done.
        run.minibosses_spawned = [true; 2];
        info!("DEV --bossnow: clock wound to {:.0}s (minibosses skipped)", run.timer);
    }
    *done = true;
}

/// `--dev --levelupnow`: queue three level-ups and 60 Gold on the local astronaut a few
/// seconds in, so the level-up panel's Refresh (free, then paid) / Banish / Skip row can be
/// driven and screenshot in a windowed test without farming gems first.
fn dev_levelup_now(
    time: Res<Time>,
    mut q: Query<&mut run::PlayerState, With<player::LocalPlayer>>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *waited += time.delta_secs();
    if *waited < 3.0 {
        return;
    }
    let Ok(mut ps) = q.single_mut() else { return };
    let target = ps.level + 3;
    while ps.level < target {
        let need = ps.xp_needed - ps.xp;
        ps.xp += need;
        ps.gain_xp(0.0);
    }
    ps.gold += 60;
    *done = true;
    info!("DEV --levelupnow: {} level-ups queued", ps.pending_levelups);
}

/// `--dev --items a,b,…` (or `new` for the fifteen §7 additions): hand the LOCAL astronaut
/// those items at their native grade once it stands, so a windowed or two-instance test can
/// watch them work without farming cards. On a joiner they reach the host through the
/// normal build sync — the path a picked card takes.
fn dev_grant_items(
    save: Res<save::MetaSave>,
    run: Res<run::RunState>,
    mut q: Query<&mut run::PlayerState, With<player::LocalPlayer>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Ok(mut ps) = q.single_mut() else { return };
    let granted = items::grant_items(&mut ps, &items::items_from_args(), &save, run.greed_stacks);
    info!("DEV --items: granted {granted:?}");
    *done = true;
}

/// `--dev --warp noon|dusk|night`: a second in, set the local astronaut down where the sun
/// stands high, just on the lit side of the terminator, or deep in the night, facing away
/// from the sun (lit faces toward the camera; at dusk, the terminator ahead). Host/solo only
/// — a joiner's body is the host's to move.
fn dev_warp(
    time: Res<Time>,
    mut q: Query<(&mut player::Player, &mut techs::MoveTech), With<player::LocalPlayer>>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *waited += time.delta_secs();
    if *waited < 1.0 {
        return;
    }
    let Ok((mut p, mut tech)) = q.single_mut() else { return };
    let args: Vec<String> = std::env::args().collect();
    let want = args.iter().position(|a| a == "--warp").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    // how high the sun stands over the spot (sun · up)
    let sun_up = match want.as_str() {
        "noon" => 0.85,
        "dusk" => 0.12,
        _ => -0.8,
    };
    let sun = planet::sunward();
    let across = sphere::tangent_frame(sun).0;
    let dir = (sun * sun_up + across * (1.0 - sun_up * sun_up).sqrt()).normalize();
    tech.cancel_moves();
    p.dir = dir;
    p.vel_t = Vec3::ZERO;
    p.vel_r = 0.0;
    p.height = 0.0;
    p.facing = (dir * sun.dot(dir) - sun).normalize_or_zero();
    *done = true;
    info!("DEV --warp {want}: sun·up {sun_up:.2}");
}

/// `--dev --give deathray,stormcore`: hand the local astronaut these weapons (names as the
/// game prints them, lower case, no spaces) a couple of seconds in, so a windowed test can
/// look at a late-game weapon's visuals — e.g. under photosensitivity mode — without
/// playing to its evolution.
fn dev_give_weapons(
    time: Res<Time>,
    assets: Res<combat::WeaponAssets>,
    mut q: Query<&mut run::PlayerState, With<player::LocalPlayer>>,
    mut waited: Local<f32>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    *waited += time.delta_secs();
    if *waited < 2.0 {
        return;
    }
    let Ok(mut ps) = q.single_mut() else { return };
    let args: Vec<String> = std::env::args().collect();
    let list = args.iter().position(|a| a == "--give").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    for want in list.split(',') {
        let key = |n: &str| n.to_lowercase().replace([' ', '-', '\''], "");
        let Some(kind) = assets.mats.keys().copied().find(|k| key(k.def().name) == key(want)) else {
            warn!("DEV --give: no weapon called {want:?}");
            continue;
        };
        if !ps.weapons.iter().any(|w| w.kind == kind) {
            ps.weapons.push(run::WeaponInstance { kind, level: 1, cd: 0.0 });
            info!("DEV --give: {}", kind.def().name);
        }
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
