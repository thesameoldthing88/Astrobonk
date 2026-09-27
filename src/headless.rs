//! Headless smoke test: `astrobonk --headless [ticks]` runs the full simulation
//! with no window, a movement bot, and auto-picked upgrades, then prints a
//! summary and exits non-zero on failure. chadkit:verify-headless compatible.

use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::content::planets::PlanetKind;
use crate::enemies::Enemy;
use crate::planet::CurrentPlanet;
use crate::player::Player;
use crate::run::{ChoicePanel, PlayerState, RunPhase, RunState};
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
    global: Res<RunState>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    save: Res<MetaSave>,
    mut chest: ResMut<crate::interact::ChestPanel>,
    mut shop: ResMut<crate::interact::ShopPanel>,
    mut q: Query<(&mut Player, &mut PlayerState, &Transform, Has<crate::player::LocalPlayer>)>,
    q_pickups: Query<(&crate::pickups::Pickup, &Transform), Without<Player>>,
    q_enemies: Query<(&Enemy, &Transform), (Without<Player>, Without<crate::pickups::Pickup>)>,
    mut heading_angle: Local<f32>,
) {
    let dt = time.delta_secs();

    // Panels are per-MACHINE, so only the local astronaut resolves them — mirroring the
    // real game, where a peer levelling up must not spend the host human's cards.
    if matches!(*phase, RunPhase::LevelUp | RunPhase::Modal) {
        for (_, mut run, _, is_local) in &mut q {
            if !is_local {
                continue;
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

    for (mut p, mut run, ptf, _is_local) in &mut q {

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
    }
    let _ = &planet;
}

/// Co-op probes. Headless IS a host, so the per-astronaut paths a joiner depends on can be
/// staged around `--coop2`'s peer (player 1) and ASSERTED, instead of only eyeballed in two
/// windows:
///   * `--comet-peer`  — the peer strings a tail and cashes out a Comet Combo: the host must
///     run the combo for EVERY astronaut and pay it out around the one who earned it.
///   * `--storm-peer`  — (Mars) a tight dust storm is pinned on the peer: only the peer may
///     wear `InStorm`, and no Beamer may keep an aim line on it.
///   * `--peer-hero X` — the peer's sheet switches hero (what a joiner's first build
///     heartbeat does): its rig and replicated NetHero must follow.
#[derive(Resource, Default)]
struct CoopProbe {
    comet_peer: bool,
    storm_peer: bool,
    peer_hero: Option<AstronautKind>,
    ticks: u64,
    comet_staged: bool,
    /// (peer fires, run silver) when the tail was staged
    comet_base: (u32, u64),
    /// run silver gained on the cash-out frame, once it happened
    comet_paid: Option<u64>,
    hero_swapped: bool,
    storm_ticks_hidden: u32,
    storm_marker_errors: u32,
    storm_hidden_locks: u32,
    storm_host_locks: u32,
}

/// `--comet-peer`: a couple of seconds in, put a 12-strong tail right in the peer's wake
/// and its charge one step short of the goal. The next frames must cash out FOR THE PEER.
fn comet_peer_stage(
    mut commands: Commands,
    mut probe: ResMut<CoopProbe>,
    run: Res<RunState>,
    assets: Res<crate::enemies::EnemyAssets>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(&crate::player::PlayerId, &Player, &PlayerState, &mut crate::comet::CometState)>,
) {
    probe.ticks += 1;
    if probe.comet_staged {
        if probe.comet_paid.is_none() {
            let fired = q.iter().any(|(pid, _, _, c)| pid.0 == 1 && c.fires > probe.comet_base.0);
            if fired {
                probe.comet_paid = Some(run.silver_run.saturating_sub(probe.comet_base.1));
            }
        }
        return;
    }
    if probe.ticks < 60 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (pid, p, ps, mut c) in &mut q {
        if pid.0 != 1 || ps.dead {
            continue;
        }
        let back = (-p.vel_t).try_normalize().unwrap_or(sphere::tangent_frame(p.dir).0);
        let side = back.cross(p.dir).normalize_or_zero();
        for i in 0..12 {
            let along = 3.0 + (i / 3) as f32 * 1.5;
            let lateral = ((i % 3) as f32 - 1.0) * 1.2;
            let v = back * along + side * lateral;
            let dir = sphere::offset_dir(p.dir, v.normalize(), v.length(), planet.radius);
            crate::enemies::spawn_enemy(
                &mut commands,
                &assets,
                &planet,
                crate::content::enemies::EnemyKind::Shambler,
                dir,
                false,
                1.0,
                1.0,
                &mut rng,
            );
        }
        c.active = true;
        c.charge = COMET_CHARGE_GOAL - 1.0;
        c.peak = c.peak.max(12);
        probe.comet_base = (c.fires, run.silver_run);
        probe.comet_staged = true;
        println!("  COMETPEER staged a 12-tail behind player 1 at tick {}", probe.ticks);
    }
}

/// `--peer-hero X`: swap the peer's hero on its sheet, as apply_player_build does when a
/// joiner's first heartbeat names a different hero than the host seated it with.
fn peer_hero_swap(
    mut probe: ResMut<CoopProbe>,
    mut q: Query<(&crate::player::PlayerId, &mut PlayerState)>,
    mut ticks: Local<u32>,
) {
    *ticks += 1;
    if probe.hero_swapped || *ticks < 30 {
        return;
    }
    let Some(hero) = probe.peer_hero else { return };
    for (pid, mut ps) in &mut q {
        if pid.0 == 1 {
            ps.character = hero;
            probe.hero_swapped = true;
        }
    }
}

/// Size of the storm `--storm-peer` pins on the peer: tight, so the host astronaut is
/// usually OUTSIDE it and the test proves the hiding is per-astronaut.
const STORM_PROBE_RADIUS: f32 = 2.5;

/// `--storm-peer`: keep a tight storm centred on the peer.
fn storm_peer_pin(mut storm: ResMut<crate::events_world::DustStorm>, q: Query<(&crate::player::PlayerId, &Player)>) {
    let Some(dir) = q.iter().find(|(pid, _)| pid.0 == 1).map(|(_, p)| p.dir) else { return };
    storm.primed = true;
    storm.active = true;
    storm.timer = 999.0;
    storm.radius = STORM_PROBE_RADIUS;
    storm.dir = dir;
}

/// `--storm-peer`: the markers must match who is actually inside, and no Beamer may hold a
/// line on the hidden peer for more than the frame it takes to notice.
fn storm_peer_check(
    mut probe: ResMut<CoopProbe>,
    storm: Res<crate::events_world::DustStorm>,
    planet: Res<CurrentPlanet>,
    q: Query<(Entity, &crate::player::PlayerId, &Player, Has<crate::events_world::InStorm>)>,
    beamers: Query<(Entity, &crate::enemies::Beamer)>,
    mut offenders: Local<Vec<Entity>>,
    mut host_charging: Local<Vec<Entity>>,
    mut armed: Local<bool>,
) {
    // The very first check follows the very first pin, before the sim has marked anyone.
    let first = !*armed;
    *armed = true;
    let mut peer = None;
    for (e, pid, p, hidden) in &q {
        let arc = sphere::arc_dist(p.dir, storm.dir, planet.radius);
        // a little slack: the pin and the sim see positions a frame apart
        if !first && ((hidden && arc > storm.radius + 0.6) || (!hidden && arc < storm.radius - 0.6)) {
            probe.storm_marker_errors += 1;
        }
        if pid.0 == 1 && hidden {
            peer = Some(e);
            probe.storm_ticks_hidden += 1;
        }
    }
    let mut now_offending = Vec::new();
    let mut now_host = Vec::new();
    for (be, b) in &beamers {
        if b.charging <= 0.0 {
            continue;
        }
        if b.target.is_some() && b.target == peer {
            // one frame of grace: beamer_attack may run before the marker lands
            if offenders.contains(&be) {
                probe.storm_hidden_locks += 1;
            }
            now_offending.push(be);
        } else if b.target.is_some() {
            if !host_charging.contains(&be) {
                probe.storm_host_locks += 1; // a fresh charge on someone visible
            }
            now_host.push(be);
        }
    }
    *offenders = now_offending;
    *host_charging = now_host;
}

/// Gems collected over the run (every collection writes one `GrantOut::Xp`).
#[derive(Resource, Default)]
struct XpTally(u64);

fn tally_xp(mut grants: MessageReader<crate::net::GrantOut>, mut tally: ResMut<XpTally>) {
    for g in grants.read() {
        if matches!(g, crate::net::GrantOut::Xp(_)) {
            tally.0 += 1;
        }
    }
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
        println!(
            "  t={:>4.0}s timer={:>5.1} lvl={} kills={} hp={:.0} enemies={} gold={} static={}",
            run.total_elapsed, run.timer, q_ps.single().map(|p| p.level).unwrap_or(1), run.kills, hp, alive, q_ps.single().map(|p| p.gold).unwrap_or(0), run.static_active
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
    let args: Vec<String> = std::env::args().collect();
    let coop2 = args.iter().any(|a| a == "--coop2");
    let probe = CoopProbe {
        // all three stage a scene around the PEER, so they need `--coop2`'s second astronaut
        comet_peer: coop2 && args.iter().any(|a| a == "--comet-peer"),
        storm_peer: coop2 && planet_kind == PlanetKind::Mars && args.iter().any(|a| a == "--storm-peer"),
        peer_hero: args
            .iter()
            .position(|a| a == "--peer-hero")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| AstronautKind::from_name(s))
            .filter(|_| coop2),
        ..default()
    };
    let mut run = RunState::new(hero, planet_kind, 1, &save);
    if let Some(s) = seed {
        run.run_seed = s;
    }
    if fast_boss {
        run.timer = 95.0; // just above the boss mark: boss arrives ~5s in
        run.elapsed = 570.0; // late-game spawn mix: beamers, lobbers, UFOs, burrowers
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
        // The host path: headless IS the host (or solo), so these stay at their defaults —
        // but the shared presentation systems read them.
        .init_resource::<crate::net::NetRole>()
        .init_resource::<crate::net::MyPlayerId>()
        .insert_resource(probe)
        .init_resource::<XpTally>()
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
                crate::enemies::anubot_beam_visuals,
                crate::enemies::boss_phase_system,
                crate::enemies::burrower_emerge,
                crate::enemies::enemy_contact,
                crate::enemies::spitter_attack,
                crate::enemies::beamer_attack,
                crate::enemies::aim_line_visuals,
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
                crate::comet::comet_system,
                crate::comet::comet_presentation,
                crate::events_world::dust_storm_sim,
                storm_peer_pin.run_if(|p: Res<CoopProbe>| p.storm_peer),
                crate::events_world::dust_storm_visuals,
                storm_peer_check.run_if(|p: Res<CoopProbe>| p.storm_peer),
            )
                .chain()
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                comet_peer_stage.run_if(|p: Res<CoopProbe>| p.comet_peer),
                peer_hero_swap.run_if(|p: Res<CoopProbe>| p.peer_hero.is_some()),
            )
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
                crate::player::refit_astronaut_rigs,
                crate::net::push_net_hero,
                crate::player::player_upkeep,
                crate::fx::update_particles,
                crate::director::stage_transition,
                bot_watchdog,
                tally_xp,
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
    // The LOCAL astronaut's sheet: with `--coop2` an arbitrary one could be the peer.
    let ps = world
        .query_filtered::<&PlayerState, With<crate::player::LocalPlayer>>()
        .iter(world)
        .next()
        .cloned();
    let (p_level, p_gold, p_hp, p_maxhp) = ps
        .map(|p| (p.level, p.gold, p.hp, p.stats.max_hp))
        .unwrap_or((1, 0, 0.0, 100.0));
    // The XP pipeline is kills -> gems -> collection -> XP. Judge each link on its own:
    // "kills but level 1" alone flaked on the base build, because a fast-boss squad can
    // die with a hundred kills' worth of gems still on the ground (one Comet cash-out
    // clears ~100) — a short run, not a dead pipeline. XP is also granted only to
    // astronauts still standing, so look at every sheet, not just the local one.
    let best_level = world.query::<&PlayerState>().iter(world).map(|p| p.level).max().unwrap_or(1);
    let any_xp = world.query::<&PlayerState>().iter(world).any(|p| p.xp > 0.0);
    let gems_left = world
        .query::<&crate::pickups::Pickup>()
        .iter(world)
        .filter(|p| matches!(p.kind, crate::pickups::PickupKind::Xp(_)))
        .count();
    let peers: Vec<(u8, crate::content::characters::AstronautKind, crate::player::RigHero, crate::net::NetHero, u32, crate::net::NetComet)> = world
        .query::<(&crate::player::PlayerId, &PlayerState, &crate::player::RigHero, &crate::net::NetHero, &crate::comet::CometState, &crate::net::NetComet)>()
        .iter(world)
        .map(|(id, ps, rig, nh, c, nc)| (id.0, ps.character, *rig, *nh, c.fires, *nc))
        .collect();
    let enemies = world.query_filtered::<(), With<Enemy>>().iter(world).count();
    let phase = *world.resource::<RunPhase>();
    let comet_fires = world.resource::<crate::comet::Comet>().fires;
    let storm = world.resource::<crate::events_world::DustStorm>();
    let storm_state = format!("spawned={} active={}", storm.spawned_vis, storm.active);

    println!("--- SMOKE SUMMARY ---");
    println!(
        "phase={phase:?} level={} kills={} gold={} hp={:.0}/{:.0} timer={:.0} enemies={} comets={comet_fires} storm[{storm_state}] boss_spawned={} boss_dead={}",
        p_level, run.kills, p_gold, p_hp, p_maxhp, run.timer, enemies, run.boss_spawned, run.boss_dead
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
    let xp_grants = world.resource::<XpTally>().0;
    if run.kills > 50 && xp_grants == 0 && gems_left == 0 {
        println!("FAIL: XP pipeline dead (kills dropped no gems)");
        ok = false;
    }
    if xp_grants > 0 && best_level < 2 && !any_xp {
        println!("FAIL: XP pipeline dead ({xp_grants} gems collected, no XP, no levels)");
        ok = false;
    }
    // Every astronaut's replicated mirrors must agree with its simulated state — this is
    // exactly what a joiner's HUD and rigs are drawn from.
    for (id, hero, rig, net_hero, fires, nc) in &peers {
        if rig.0 != *hero || net_hero.0 != crate::net::hero_code(*hero) {
            println!("FAIL: player {id} is {} but its rig/NetHero disagree", hero.def().name);
            ok = false;
        }
        if nc.fires as u32 != *fires {
            println!("FAIL: player {id} NetComet.fires={} but CometState.fires={fires}", nc.fires);
            ok = false;
        }
    }
    let probe = world.resource::<CoopProbe>();
    if let Some(hero) = probe.peer_hero {
        let peer = peers.iter().find(|p| p.0 == 1);
        if !probe.hero_swapped || peer.map(|p| p.1 != hero || p.2.0 != hero).unwrap_or(true) {
            println!("FAIL: peer never re-suited as {}", hero.def().name);
            ok = false;
        } else {
            println!("PEERHERO OK: player 1 re-suited as {} (NetHero {})", hero.def().name, crate::net::hero_code(hero));
        }
    }
    if probe.comet_peer {
        match probe.comet_paid {
            Some(silver) if silver >= 12 => {
                println!("COMETPEER OK: player 1 cashed out its own combo (+{silver} silver)")
            }
            other => {
                println!("FAIL: peer's staged comet never cashed out for it (paid={other:?}, staged={})", probe.comet_staged);
                ok = false;
            }
        }
    }
    if probe.storm_peer {
        println!(
            "STORMPEER hidden_ticks={} marker_errors={} beamer_locks_on_hidden_peer={} beamer_locks_on_visible={}",
            probe.storm_ticks_hidden, probe.storm_marker_errors, probe.storm_hidden_locks, probe.storm_host_locks
        );
        if probe.storm_ticks_hidden == 0 || probe.storm_marker_errors > 0 || probe.storm_hidden_locks > 0 {
            println!("FAIL: dust storm hiding is not per-astronaut");
            ok = false;
        }
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
