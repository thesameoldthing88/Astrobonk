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
use std::collections::HashMap;
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
    // Refresh until the free ones are gone, then once more at a price. Bounded, because
    // Lady Fortuna's refreshes stay free forever and "once it costs Gold" never comes.
    for _ in 0..=FREE_REFRESHES {
        let (gold, price) = (ps.gold, ps.refresh_price());
        let ok = ps.spend_refresh();
        match (price, ok) {
            (RefreshPrice::Free, true) => {
                if ps.gold != gold {
                    panic!("SMOKE FAIL: a free refresh charged {}g", gold - ps.gold);
                }
                script.free_refreshes += 1;
            }
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
        (NewItem(x, _) | ItemUp(x, _), NewItem(y, _) | ItemUp(y, _)) => x == y,
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
    tech_probe: Res<TechProbe>,
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
    if matches!(*phase, RunPhase::Dead) || tech_probe.holding {
        return;
    }
    // §11: where the squad's Beacons lie (the bot answers them, as a player would)
    let beacons: Vec<Vec3> = q.iter().filter(|(_, ps, ..)| ps.dead && !ps.claimed).map(|(_, _, tf, _)| tf.translation).collect();

    for (mut p, mut run, ptf, is_local) in &mut q {
    // A Beacon rolls on its own (`coop::tumble`), and a drop-in in its grace is the
    // autopilot's (`coop::autopilot_peers`, under `--dropin`).
    if run.dead || run.grace > 0.0 || (!is_local && std::env::args().any(|a| a == "--dropin")) {
        continue;
    }

    // miniboss cache first (it is the thing under test when one exists); then
    // hurt -> kite away from the nearest threat; healthy -> chase gems; else wander
    let mut heading = None;
    let cache = if is_local { q_cache.iter().next().map(|t| t.translation) } else { None };
    let beacon = beacons
        .iter()
        .copied()
        .filter(|b| b.distance_squared(ptf.translation) > 1.0)
        .min_by(|a, b| a.distance_squared(ptf.translation).total_cmp(&b.distance_squared(ptf.translation)));
    if let Some(bpos) = beacon {
        let v = bpos - ptf.translation;
        let vt = (v - p.dir * v.dot(p.dir)).normalize_or_zero();
        // close enough: stand in the ring (stopping) while the revive fills
        if vt != Vec3::ZERO && v.length() > REVIVE_RADIUS * 0.5 {
            heading = Some(vt);
        } else {
            p.vel_t = Vec3::ZERO;
            continue;
        }
    } else if let Some(cpos) = cache {
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

/// `--items a,b,…` / `--deathsave`: the §7 item probes. Items are handed to EVERY
/// astronaut (with `--coop2` that includes the peer, so the host is seen simulating a
/// joiner's items too), and the summary asserts each one visibly did its thing.
#[derive(Resource, Default)]
struct ItemProbe {
    items: Vec<crate::content::items::ItemKind>,
    granted: bool,
    ticks: u64,
    /// PlayerIds that threw a yo-yo / opened a singularity / lit a trail (from ItemFxMsg).
    fx_owners: Vec<u8>,
    deathsave: bool,
    /// `--deathsave` stage: 0 waiting, then 1.. through `deathsave_steps`, `DS_DONE` done.
    ds_stage: u8,
    ds_victim: Option<Entity>,
    ds_log: Vec<String>,
    ds_fail: Option<String>,
}

/// Hand out the probe's items on the first frame the astronauts stand.
fn item_probe_grant(
    mut probe: ResMut<ItemProbe>,
    save: Res<MetaSave>,
    global: Res<RunState>,
    mut q: Query<(Entity, &crate::player::PlayerId, &mut PlayerState, Has<crate::player::LocalPlayer>)>,
) {
    if probe.granted || q.is_empty() {
        return;
    }
    let coop2 = std::env::args().any(|a| a == "--coop2");
    for (e, pid, mut ps, local) in &mut q {
        let mut items = probe.items.clone();
        let victim = if coop2 { pid.0 == 1 } else { local };
        if probe.deathsave && victim {
            items.extend([crate::content::items::ItemKind::DeadMansTether, crate::content::items::ItemKind::WidowsRing]);
            probe.ds_victim = Some(e);
        }
        let taken = crate::items::grant_items(&mut ps, &items, &save, global.greed_stacks);
        println!("  ITEMS player {} granted {}", pid.0, taken.iter().map(|i| i.def().name).collect::<Vec<_>>().join(", "));
    }
    probe.granted = true;
}

/// Make the bot exercise the items it holds: stand still for a beat every 8 s (Comet Tail
/// ignites on a stand-still) and hop the rest of the time, holding jump (Icarus, Anti-Grav
/// hover and its 360° ring).
fn item_probe_drive(
    mut probe: ResMut<ItemProbe>,
    mut q: Query<(&mut Player, &mut crate::player::InputIntent, &PlayerState)>,
) {
    probe.ticks += 1;
    let t = probe.ticks % 240; // 8 s at 33 ms
    for (mut p, mut intent, ps) in &mut q {
        if ps.dead {
            continue;
        }
        if t < 80 {
            p.vel_t = Vec3::ZERO;
            intent.jump_held = false;
        } else {
            intent.jump_held = true;
            if p.grounded && t % 45 == 0 {
                p.vel_r = PLAYER_JUMP_VEL * ps.stats.jump_height.sqrt();
                p.grounded = false;
            }
        }
    }
}

fn item_probe_fx(mut probe: ResMut<ItemProbe>, mut fx: MessageReader<crate::items::ItemFxMsg>) {
    use crate::items::ItemFx;
    for m in fx.read() {
        let owner = match m.fx {
            ItemFx::Orbit { owner, .. } | ItemFx::Ignite { owner } | ItemFx::DeathSave { owner, .. } => owner,
            ItemFx::Singularity { .. } => continue,
        };
        if !probe.fx_owners.contains(&owner) {
            probe.fx_owners.push(owner);
        }
    }
}

/// `--tomes all|a,b,… [--tome-rank N]`: the §7 tome probe. The save slots the listed tomes
/// at that rank for EVERY astronaut (with `--coop2` that includes the peer, so the host is
/// seen simulating a joiner's tome lines too), the bot is handed the kit the tomes act on,
/// and the summary asserts each one visibly did its thing.
#[derive(Resource, Default)]
struct TomeProbe {
    tomes: Vec<crate::content::tomes::TomeKind>,
    rank: u32,
    set_up: bool,
    ticks: u64,
    /// (player, banishes, refreshes, evolution cap, free Microwave uses) as each sheet began.
    start: Vec<(u8, u32, u32, u32, u32)>,
    /// Largest drone body / Orbital Yo-Yo swing seen, over what it is without the tome.
    drone_scale: f32,
    yoyo_ratio: f32,
    // What the tomes did, watched from outside (`tome_probe_watch`): the simulation systems
    // carry no telemetry of their own.
    max_crowd_bonus: f32,
    night_secs: f32,
    fast_fall_secs: f32,
    max_momentum_bonus: f32,
    ricochets: u32,
    horizon_calls: u32,
    horizon_arrivals: u32,
    /// Horde elites killed, and how many of those dropped Tome of the Elite's richer loot.
    elite_kills: u32,
    elite_richer: u32,
    /// Boss and miniboss kills the tome wrongly made richer (must stay 0).
    boss_richer: u32,
    /// Hits a ghost of The Static landed on an astronaut (Tome of Static bites harder).
    ghost_hits: u32,
    /// Tome of Duplication at the Microwave: (free uses, stage use spent, item copies) before
    /// the first press, after it, and after the second.
    microwave: Vec<(u32, bool, u32)>,
    /// `--coop2` Horizon: the peer's called gem whose flight the probe cut by downing the
    /// peer for a moment, the tick it did, and whether the host withdrew the call.
    withdraw: Option<(Entity, u64)>,
    withdrawn: Option<bool>,
}

/// Hand every astronaut the kit its tomes act on (a shot weapon for Ricochet, a drone ring
/// and a boomerang for Orbit, the proc and lifesteal items for Swarm and Vampirism) and lay
/// XP out over each one's horizon for the Tome of the Horizon to call home.
#[allow(clippy::too_many_arguments)]
fn tome_probe_setup(
    mut commands: Commands,
    mut probe: ResMut<TomeProbe>,
    save: Res<MetaSave>,
    global: Res<RunState>,
    planet: Res<CurrentPlanet>,
    assets: Res<crate::pickups::PickupAssets>,
    mut q: Query<(&crate::player::PlayerId, &Player, &mut PlayerState)>,
) {
    use crate::content::items::ItemKind;
    use crate::content::weapons::WeaponKind;
    if probe.set_up || q.is_empty() {
        return;
    }
    probe.set_up = true;
    for (pid, p, mut ps) in &mut q {
        probe.start.push((pid.0, ps.banishes, ps.refreshes, ps.evo_cap(), ps.free_microwave));
        for w in [WeaponKind::LaserPistol, WeaponKind::Drones, WeaponKind::Boomerang] {
            if ps.weapons.len() < WEAPON_SLOTS && !ps.weapons.iter().any(|i| i.kind == w) {
                ps.weapons.push(crate::run::WeaponInstance { kind: w, level: 1, cd: 0.0 });
            }
        }
        crate::items::grant_items(&mut ps, &[ItemKind::OrbitalYoYo, ItemKind::VampireVisor], &save, global.greed_stacks);
        // gems well past the horizon, in three directions — only for the Tome of the
        // Horizon to call home: nothing else collects them, and a run that drops no XP of
        // its own (`--staticnow`: The Static's ghosts only) would read them as a dead XP
        // pipeline
        if !probe.tomes.contains(&crate::content::tomes::TomeKind::Horizon) {
            continue;
        }
        let (t, b) = sphere::tangent_frame(p.dir);
        for (i, arc) in [HORIZON_ARC * 1.6, HORIZON_ARC * 2.5, HORIZON_ARC * 1.2].iter().enumerate() {
            let a = i as f32 * 2.1 + pid.0 as f32;
            let dir = sphere::offset_dir(p.dir, t * a.cos() + b * a.sin(), *arc, planet.radius);
            crate::pickups::spawn_pickup(&mut commands, &assets, &planet, dir, crate::pickups::PickupKind::Xp(5.0));
        }
    }
    println!(
        "  TOMES slotted {} at rank {}: {}",
        probe.tomes.len(),
        probe.rank,
        probe.tomes.iter().map(|t| t.def().name).collect::<Vec<_>>().join(", ")
    );
}

/// Walk the bot onto the night side once, hop it every couple of seconds (Tome of Gravity's
/// fall), watch the orbiting bodies the Tome of Orbit scales, and — with Tome of
/// Duplication — walk the local astronaut to the Microwave twice and press E through the
/// real `interact_system`.
#[allow(clippy::too_many_arguments)]
fn tome_probe_drive(
    mut probe: ResMut<TomeProbe>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    mut fx: MessageReader<crate::items::ItemFxMsg>,
    drones: Query<&crate::combat::Drone>,
    mut crowd: Query<(Entity, &mut Enemy), Without<crate::enemies::Boss>>,
    mut hits: MessageWriter<crate::messages::HitMsg>,
    mut q: Query<(&crate::player::PlayerId, &mut Player, &mut crate::items::ItemProcs, &PlayerState, Has<crate::player::LocalPlayer>)>,
    (interactables, mut keys, mut shots): (
        Query<(&crate::interact::Interactable, &Transform)>,
        ResMut<ButtonInput<KeyCode>>,
        Query<&mut crate::combat::Projectile>,
    ),
) {
    probe.ticks += 1;
    let t = probe.ticks;
    // Tome of Ricochet skips a shot that COMES DOWN — one whose life runs out. In a dense
    // late-game crowd (--fast-boss) every shot can spend its pierce on a body first, so the
    // natural count can be zero on a lucky-for-the-bot run: every 3 s, bring one shot the
    // real firing path rolled a skip for (`bounces > 0`) to the end of its life, and let
    // `projectile_move` do the rest.
    if probe.tomes.contains(&crate::content::tomes::TomeKind::Ricochet) && t % 90 == 45 {
        if let Some(mut shot) = shots.iter_mut().find(|s| s.bounces > 0 && s.hop <= 0.0 && s.life > 0.1) {
            shot.life = 0.001;
        }
    }
    // Tome of Encirclement counts foes within TOME_CROWD_RADIUS, and a bot carrying every
    // tome clears its surroundings before many close in: every 5 s, walk a handful of the
    // horde in to ring the local astronaut at arm's length.
    if probe.tomes.contains(&crate::content::tomes::TomeKind::Encirclement) && t % 150 == 75 {
        let me = q.iter().find(|(.., ps, local)| *local && !ps.dead).map(|(_, p, ..)| p.dir);
        if let Some(me) = me {
            let (a, b) = sphere::tangent_frame(me);
            let ring = crowd.iter_mut().filter(|(_, en)| en.speed > 0.0 && en.hp > 0.0).take(6);
            for (i, (_, mut en)) in ring.enumerate() {
                let ang = i as f32 * std::f32::consts::TAU / 6.0;
                en.dir = sphere::offset_dir(me, a * ang.cos() + b * ang.sin(), TOME_CROWD_RADIUS * 0.5, planet.radius);
            }
        }
    }
    // Tome of Duplication: two presses at the Microwave, 3 s apart — the free use must go
    // first and leave the stage's own; each must duplicate an item.
    if probe.tomes.contains(&crate::content::tomes::TomeKind::Duplication) && (90..=240).contains(&t) {
        let oven = interactables
            .iter()
            .find(|(i, _)| i.kind == crate::interact::InteractKind::Microwave)
            .map(|(_, tf)| tf.translation.normalize_or_zero());
        for (_, mut p, mut procs, ps, is_local) in &mut q {
            let Some(oven) = oven.filter(|_| is_local && !ps.dead) else { continue };
            let copies = ps.items.iter().map(|s| s.count()).sum::<u32>();
            let now = (ps.free_microwave, run.microwave_used, copies);
            if t == 90 || t == 180 || t == 240 {
                probe.microwave.push(now);
            }
            if (90..=100).contains(&t) || (180..=190).contains(&t) {
                p.dir = oven;
                p.vel_t = Vec3::ZERO;
                procs.forget_altitude();
                if t == 92 || t == 182 {
                    keys.press(KeyCode::KeyE);
                }
            }
        }
    }
    // Tome of the Elite pays out on an elite kill, and a short run may not meet one: every
    // 10 s, promote a walker and drop it through the real HitMsg → KillMsg → kill_drops path.
    if t % 300 == 0 && probe.tomes.contains(&crate::content::tomes::TomeKind::Elite) {
        if let Some((e, mut en)) = crowd.iter_mut().find(|(_, en)| en.speed > 0.0 && en.hp > 0.0) {
            en.elite = true;
            hits.write(crate::messages::HitMsg { source: None, target: e, amount: en.hp + 1.0, crit: false, knock: Vec3::ZERO, by: crate::messages::HitBy::Other });
        }
    }
    for (pid, mut p, mut procs, ps, _) in &mut q {
        if ps.dead {
            continue;
        }
        if t == 60 {
            // just past the terminator on the dark side, fanned out per player
            let night = -crate::planet::sunward();
            let (a, _) = sphere::tangent_frame(night);
            p.dir = sphere::offset_dir(night, a, 4.0 * pid.0 as f32, planet.radius);
            p.vel_t = Vec3::ZERO;
            p.height = 0.0;
            procs.forget_altitude();
        }
        if t % 70 == 0 && p.grounded {
            p.vel_r = PLAYER_JUMP_VEL * ps.stats.jump_height.sqrt();
            p.grounded = false;
        }
    }
    for d in &drones {
        probe.drone_scale = probe.drone_scale.max(d.scale);
    }
    for m in fx.read() {
        if let crate::items::ItemFx::Orbit { radius, .. } = m.fx {
            probe.yoyo_ratio = probe.yoyo_ratio.max(radius / crate::items::yoyo_radius(&planet));
        }
    }
}

/// What the tomes did, read off the world each tick — the crowd, night, fall and momentum
/// readings on every sheet, shots mid-skip, gems called home and handed to the magnet, and
/// the elite and ghost traffic — so the simulation itself carries no probe counters.
#[allow(clippy::too_many_arguments)]
fn tome_probe_watch(
    time: Res<Time>,
    run: Res<RunState>,
    mut probe: ResMut<TomeProbe>,
    sheets: Query<(&Player, &PlayerState)>,
    shots: Query<(Entity, &crate::combat::Projectile)>,
    called: Query<(), Changed<crate::pickups::HorizonBound>>,
    homing: Query<(Entity, &crate::pickups::Pickup), With<crate::pickups::HorizonBound>>,
    ghosts: Query<&Enemy>,
    (mut kills, mut player_hits): (
        MessageReader<crate::messages::KillMsg>,
        MessageReader<crate::messages::PlayerHitMsg>,
    ),
    mut seen: Local<(std::collections::HashSet<Entity>, std::collections::HashSet<Entity>)>,
) {
    let dt = time.delta_secs();
    let (skipped, handed) = &mut *seen;
    let mut night = false;
    let mut fast_fall = false;
    for (p, ps) in &sheets {
        probe.max_crowd_bonus = probe.max_crowd_bonus.max(ps.crowd_bonus());
        probe.max_momentum_bonus = probe.max_momentum_bonus.max(ps.momentum_bonus());
        night |= ps.night && ps.stats.night_damage > 0.0;
        fast_fall |= !p.grounded && p.vel_r < 0.0 && ps.stats.fall_speed > 1.0;
    }
    if night {
        probe.night_secs += dt;
    }
    if fast_fall {
        probe.fast_fall_secs += dt;
    }
    // a shot skips at most once, and hops for RICOCHET_HOP_SECS: each hopping shot is one
    for (e, shot) in &shots {
        if shot.hop > 0.0 && skipped.insert(e) {
            probe.ricochets += 1;
        }
    }
    skipped.retain(|e| shots.contains(*e));
    probe.horizon_calls += called.iter().count() as u32;
    for (e, gem) in &homing {
        if !gem.arc && handed.insert(e) {
            probe.horizon_arrivals += 1;
        }
    }
    handed.retain(|e| homing.contains(*e));
    for k in kills.read() {
        let richer = crate::pickups::elite_loot_mult(k, &run) > 1.0;
        if k.is_boss || k.is_miniboss {
            probe.boss_richer += u32::from(richer);
        } else if k.elite {
            probe.elite_kills += 1;
            probe.elite_richer += u32::from(richer);
        }
    }
    for h in player_hits.read() {
        if h.attacker.and_then(|a| ghosts.get(a).ok()).is_some_and(|en| en.kind == crate::content::enemies::EnemyKind::Ghost) {
            probe.ghost_hits += 1;
        }
    }
}

/// `--coop2` with Tome of the Horizon: once a gem is flying home to the PEER, down the peer
/// for two ticks — the host must withdraw the call (drop `HorizonBound`, which is what
/// `netenemy::stream_pickups` announces as HorizonSettle) and leave the gem lying where it
/// got to, not flying on to a body that cannot collect it.
fn tome_probe_withdraw(
    mut probe: ResMut<TomeProbe>,
    gems: Query<(Entity, &crate::pickups::Pickup, Option<&crate::pickups::HorizonBound>)>,
    mut peers: Query<(&crate::player::PlayerId, &mut PlayerState), Without<crate::player::LocalPlayer>>,
) {
    let t = probe.ticks;
    match probe.withdraw {
        None => {
            let Some((pid, mut ps)) = peers.iter_mut().find(|(_, ps)| !ps.dead) else { return };
            let flying = gems.iter().find(|(_, g, hb)| g.arc && hb.is_some_and(|hb| hb.0 == pid.0));
            if let Some((e, ..)) = flying {
                ps.dead = true;
                probe.withdraw = Some((e, t));
            }
        }
        Some((e, at)) if probe.withdrawn.is_none() && t >= at + 2 => {
            for (_, mut ps) in &mut peers {
                ps.dead = false;
            }
            // collected or merged in between proves nothing either way: try another gem
            let Ok((_, gem, hb)) = gems.get(e) else {
                probe.withdraw = None;
                return;
            };
            probe.withdrawn = Some(hb.is_none() && !gem.arc && !gem.flying);
        }
        _ => {}
    }
}

/// `--staticnow` (headless): a few seconds in, wind the clock out so The Static rises
/// through the real `run_clock` path — with `--fast-boss` the marks have already fired, so
/// nothing but The Static arrives. Tome of Static's payout and bite need it to be reached.
fn static_now(mut run: ResMut<RunState>, mut ticks: Local<u32>) {
    *ticks += 1;
    if *ticks == 150 && !run.static_active {
        run.timer = run.timer.min(0.5);
    }
}

/// What one `--deathsave` lethal hit must resolve to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DeathSaveStep {
    /// Exactly this item save (`items::DeathSave::code`) fired, leaving 1 HP.
    Item(usize),
    /// No item save was left, so the §13 "one more chance" token (P04) caught it — the LAST
    /// link of the chain, after every item save.
    Token,
    /// Nothing is left: the astronaut goes down.
    Down,
}

/// The `--deathsave` script: Tether, then Widow's Ring, then — with `--assist` — the revive
/// token, then (co-op peer only: a solo death would end the run) the unsaved death.
fn deathsave_steps(coop2: bool, token: bool) -> Vec<DeathSaveStep> {
    let mut steps = vec![
        DeathSaveStep::Item(crate::items::DeathSave::Tether.code() as usize),
        DeathSaveStep::Item(crate::items::DeathSave::WidowsRing.code() as usize),
    ];
    if coop2 && token {
        steps.push(DeathSaveStep::Token);
    }
    if coop2 {
        steps.push(DeathSaveStep::Down);
    }
    steps
}

/// `ItemProbe::ds_stage` once the `--deathsave` script has finished (or failed).
const DS_DONE: u8 = u8::MAX;

/// `--deathsave`: lethal hits on the victim, one stage at a time, re-sent every tick until
/// one lands (i-frames and evasion can eat a hit) — then check exactly the right save fired.
fn deathsave_probe(
    mut probe: ResMut<ItemProbe>,
    telemetry: Res<crate::items::ItemTelemetry>,
    run: Res<RunState>,
    q: Query<&PlayerState>,
    mut hits: MessageWriter<crate::messages::PlayerHitMsg>,
    mut last: Local<([u32; 4], u32)>,
    mut waited: Local<u32>,
) {
    let Some(victim) = probe.ds_victim else { return };
    let Ok(ps) = q.get(victim) else { return };
    let coop2 = std::env::args().any(|a| a == "--coop2");
    let steps = deathsave_steps(coop2, run.assist.revive_token);
    let saves = telemetry.saves;
    let stage = probe.ds_stage;
    if stage == 0 {
        if probe.ticks >= 150 {
            *last = (saves, ps.revives);
            probe.ds_stage = 1;
        }
        return;
    }
    let Some(&want) = steps.get(stage as usize - 1) else { return };
    let (last_saves, last_revives) = *last;
    let new: Vec<usize> = (0..4).filter(|i| saves[*i] > last_saves[*i]).collect();
    let revived = ps.revives > last_revives;
    if !new.is_empty() || ps.dead || revived {
        let got_one = new.len() == 1 && saves[new[0]] == last_saves[new[0]] + 1;
        let ok = match want {
            DeathSaveStep::Item(w) => got_one && new[0] == w && !revived && !ps.dead && (ps.hp - 1.0).abs() < 0.5,
            DeathSaveStep::Token => {
                new.is_empty()
                    && ps.revives == last_revives + 1
                    && !ps.dead
                    && (ps.hp - ps.stats.max_hp * REVIVE_TOKEN_HP_FRAC).abs() < 0.5
            }
            DeathSaveStep::Down => new.is_empty() && !revived && ps.dead,
        };
        probe.ds_log.push(format!(
            "stage {stage} ({want:?}): saves {last_saves:?} -> {saves:?}, revives {last_revives} -> {}, hp {:.1}, dead {}",
            ps.revives, ps.hp, ps.dead
        ));
        if !ok {
            probe.ds_fail = Some(format!("stage {stage} resolved wrong ({})", probe.ds_log.last().cloned().unwrap_or_default()));
            probe.ds_stage = DS_DONE;
            return;
        }
        if stage == 1 && telemetry.rewinds.last().is_none_or(|m| *m < 0.5) {
            probe.ds_fail = Some(format!("the Tether rewind did not move the astronaut ({:?})", telemetry.rewinds));
            probe.ds_stage = DS_DONE;
            return;
        }
        *last = (saves, ps.revives);
        *waited = 0;
        probe.ds_stage = if stage as usize >= steps.len() { DS_DONE } else { stage + 1 };
        return;
    }
    // Past the i-frames of the previous save (the token's grace is the longest), keep swinging.
    *waited += 1;
    if *waited as f32 * 0.033 > REVIVE_TOKEN_IFRAMES.max(2.0) {
        hits.write(crate::messages::PlayerHitMsg { victim, amount: 1.0e6, from: Vec3::ZERO, attacker: None });
    }
    if *waited > 600 {
        probe.ds_fail = Some(format!("stage {stage}: no lethal hit landed in 20 s"));
        probe.ds_stage = DS_DONE;
    }
}

/// `--assist`: the §13 "difficulty as options" end to end on the real hit path. Assists are
/// set before the run (density 50%, damage 50%, one more chance), then, on the local
/// astronaut:
///   1. a staged 20-damage hit must land as 20 × 0.5 × damage taken × (1 − armor);
///   2. a staged lethal hit must be caught by the token — back at REVIVE_TOKEN_HP_FRAC with
///      REVIVE_TOKEN_IFRAMES of grace, the nearby crowd shoved back;
///   3. once the grace is over, a second lethal hit must down it (one token per run) — the
///      probe stands it back up so the smoke keeps running.
/// Each staged hit retries until it lands clean: a horde hit in the same frame can take the
/// i-frames first.
#[derive(Resource, Default)]
struct AssistProbe {
    ticks: u64,
    stage: u8,
    /// (hp before, expected loss) of the staged damage hit in flight
    pending: Option<(f32, f32)>,
    damage_ok: Option<(f32, f32)>,
    revived: bool,
    nova_pushed: usize,
    /// enemies inside the nova radius when the lethal hit was staged
    nova_near: Vec<Entity>,
    second_downed: bool,
}

const ASSIST_PROBE_DAMAGE: f32 = 20.0;

fn assist_probe_hit(
    mut probe: ResMut<AssistProbe>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(Entity, &Player, &mut PlayerState), With<crate::player::LocalPlayer>>,
    q_enemies: Query<(Entity, &Enemy), (Without<crate::enemies::Boss>, Without<crate::interact::Pot>)>,
    mut hits: MessageWriter<crate::messages::PlayerHitMsg>,
    run: Res<RunState>,
) {
    probe.ticks += 1;
    let Ok((e, p, mut ps)) = q.single_mut() else { return };
    if ps.dead || probe.ticks < 60 || probe.ticks % 5 != 0 {
        return;
    }
    // The horde may have spent the token before the staged lethal hit: then only the
    // one-per-run half is left to prove.
    if probe.stage == 1 && ps.revives > 0 {
        probe.stage = 2;
    }
    match probe.stage {
        0 => {
            ps.iframes = 0.0;
            ps.shield = 0.0;
            ps.hp = ps.stats.max_hp;
            // the same product apply_player_hits takes (Cracked Helmet's ×2 taken included)
            let expected = ASSIST_PROBE_DAMAGE
                * run.assist.enemy_damage
                * ps.stats.damage_taken.max(0.0)
                * (1.0 - ps.effective_armor_fraction());
            probe.pending = Some((ps.hp, expected));
            hits.write(crate::messages::PlayerHitMsg { victim: e, amount: ASSIST_PROBE_DAMAGE, from: Vec3::ZERO, attacker: None });
        }
        1 => {
            probe.nova_near = q_enemies
                .iter()
                .filter(|(_, en)| en.speed > 0.0 && sphere::arc_dist(en.dir, p.dir, planet.radius) < REVIVE_NOVA_RADIUS * 0.8)
                .map(|(en, _)| en)
                .collect();
            // wait for company, so the nova has someone to push (give up after ~40 s)
            if probe.nova_near.is_empty() && probe.ticks < 1200 {
                return;
            }
            ps.iframes = 0.0;
            ps.shield = 0.0;
            hits.write(crate::messages::PlayerHitMsg { victim: e, amount: 1.0e6, from: Vec3::ZERO, attacker: None });
        }
        2 if ps.iframes <= 0.0 => {
            ps.shield = 0.0;
            hits.write(crate::messages::PlayerHitMsg { victim: e, amount: 1.0e6, from: Vec3::ZERO, attacker: None });
        }
        _ => {}
    }
}

fn assist_probe_check(
    mut probe: ResMut<AssistProbe>,
    mut q: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    q_enemies: Query<&Enemy>,
) {
    let Ok(mut ps) = q.single_mut() else { return };
    match probe.stage {
        0 => {
            let Some((before, expected)) = probe.pending.take() else { return };
            let lost = before - ps.hp;
            // anything else was a horde hit taking the i-frames first (or a dodge): retry
            if (lost - expected).abs() < 0.05 {
                probe.damage_ok = Some((ASSIST_PROBE_DAMAGE, lost));
                probe.stage = 1;
            }
        }
        1 if ps.revives > 0 => {
            if ps.dead || ps.hp <= 0.0 || ps.iframes < REVIVE_TOKEN_IFRAMES - 0.1 {
                panic!("SMOKE FAIL: revive token left hp={} dead={} iframes={}", ps.hp, ps.dead, ps.iframes);
            }
            let want = ps.stats.max_hp * REVIVE_TOKEN_HP_FRAC;
            if (ps.hp - want).abs() > 0.5 {
                panic!("SMOKE FAIL: revive token restored {} hp, expected {want}", ps.hp);
            }
            probe.revived = true;
            probe.nova_pushed = probe
                .nova_near
                .iter()
                .filter(|e| q_enemies.get(**e).map(|en| en.knock.length() > 1.0).unwrap_or(false))
                .count();
            probe.stage = 2;
        }
        2 if ps.dead => {
            if ps.revives != 1 {
                panic!("SMOKE FAIL: the revive token fired {} times", ps.revives);
            }
            probe.second_downed = true;
            probe.stage = 3;
            // stand back up (before downed_watch sees it) so the smoke keeps running
            ps.dead = false;
            ps.hp = ps.stats.max_hp;
        }
        _ => {}
    }
}

/// `--techs`: the §4 movement techs, staged on EVERY astronaut (with `--coop2` that includes
/// the peer, so the host is seen resolving a joiner's techs) and asserted, in order:
///   * Antipode Blink — the key lands on the exact antipode, spends the charge, and a second
///     press while it recharges does nothing;
///   * the Slam — a redlined hop held into a dive lands a full-power shockwave that hits
///     the ring of Shamblers staged around it;
///   * the slide's plow — a slide through a staged line of Shamblers shoves them aside;
///   * Grind-Lines — a slide onto a spine catches it, rides it at the rail's speed with the
///     footing locked to the crest, and a jump leaves it with the rail's momentum;
///   * the antipode read — a crowd staged at the far pole shows in the scan and in the
///     replicated `NetItemVis` the HUD dial reads;
///   * slope-boost — a slide down the steepest slope on the world speeds up, one up it slows;
///   * no ramp, no bomb — a plain running hop held forward (air control steering it as
///     `player_input` would) comes down at run speed, and a Slam from it is a dud;
///   * Boomerang Insurance — a hit that drops a charged, armed astronaut under its line
///     blinks it to the antipode through the same mechanic.
/// The probe keeps everyone standing between its stages, so a short run can't end it early.
#[derive(Resource, Default)]
struct TechProbe {
    on: bool,
    ticks: u64,
    /// The probe is steering the astronauts itself: `bot_drive` keeps its hands off.
    holding: bool,
    /// dir before the blink, by PlayerId
    before: HashMap<u8, Vec3>,
    /// (plowed count, slope speed) snapshots between stage ticks
    plowed_at: u32,
    slope_v0: f32,
    /// Boomerang Insurance: PlayerIds whose staged dip is in flight (with their blink count
    /// when it was staged), and those it caught
    insuring: Vec<u8>,
    insure_base: HashMap<u8, u32>,
    insured: Vec<u8>,
    /// The no-ramp hop: fastest speed seen in the air, and the speed its Slam banked, by
    /// PlayerId
    hop_top: HashMap<u8, f32>,
    hop_bank: HashMap<u8, f32>,
    /// Slam blinks/slams seen as TechFx, by owner
    fx_slams: Vec<u8>,
    fx_blinks: Vec<u8>,
    ok: Vec<String>,
    fail: Vec<String>,
}

const TECH_GRANT: u64 = 40;
const TECH_BLINK: u64 = 60;
const TECH_SLAM: u64 = 90;
const TECH_SLAM_CHECK: u64 = 135;
const TECH_PLOW: u64 = 150;
const TECH_PLOW_CHECK: u64 = 170;
const TECH_GRIND: u64 = 185;
const TECH_GRIND_JUMP: u64 = 215;
const TECH_GRIND_CHECK: u64 = 218;
const TECH_ANTIPODE: u64 = 240;
const TECH_ANTIPODE_CHECK: u64 = 262;
const TECH_SLOPE_DOWN: u64 = 280;
const TECH_SLOPE_UP: u64 = 300;
const TECH_HOP: u64 = 320;
/// Into the hop, the air slide that arms the Slam (the hop is ~22 ticks of hang).
const TECH_HOP_ARM: u64 = TECH_HOP + 6;
const TECH_HOP_CHECK: u64 = TECH_HOP + 40;
/// Past the blink's recharge (BLINK_COOLDOWN from TECH_BLINK, at 33 ms a tick).
const TECH_INSURE: u64 = TECH_BLINK + (BLINK_COOLDOWN / 0.033) as u64 + 30;
const TECH_DONE: u64 = TECH_INSURE + 120;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tech_probe(
    mut commands: Commands,
    mut probe: ResMut<TechProbe>,
    lines: Res<crate::techs::GrindLines>,
    planet: Res<CurrentPlanet>,
    assets: Res<crate::enemies::EnemyAssets>,
    save: Res<MetaSave>,
    global: Res<RunState>,
    telemetry: Res<crate::techs::TechTelemetry>,
    mut q: Query<(
        Entity,
        &crate::player::PlayerId,
        &mut Player,
        &mut PlayerState,
        &mut crate::techs::MoveTech,
        &mut crate::items::ItemProcs,
        &mut crate::player::InputIntent,
        &crate::net::NetItemVis,
    )>,
    enemies: Query<&Enemy>,
    placed: Query<&Transform, Or<(With<crate::interact::Interactable>, With<crate::interact::ChargeShrine>)>>,
    mut hits: MessageWriter<crate::messages::PlayerHitMsg>,
) {
    use crate::content::enemies::EnemyKind;
    use crate::content::items::ItemKind;
    probe.ticks += 1;
    let t = probe.ticks;
    let mut rng = rand::thread_rng();
    let r = planet.radius;
    let stage = |commands: &mut Commands, kind: EnemyKind, dir: Vec3, rng: &mut rand::rngs::ThreadRng| {
        crate::enemies::spawn_enemy(commands, &assets, &planet, kind, dir, false, 1.0, 1.0, rng);
    };
    let run_speed = PLAYER_RUN_SPEED;
    if t == TECH_GRANT {
        // no chest, shrine or vendor was placed standing on a rail
        let on_rail = placed
            .iter()
            .filter(|tf| lines.closest(tf.translation.normalize()).is_some_and(|(arc, ..)| arc <= GRIND_INTERACT_CLEARANCE))
            .count();
        if on_rail > 0 {
            probe.fail.push(format!("{on_rail} of {} interactables stand on a Grind-Line", placed.iter().count()));
        } else {
            probe.ok.push(format!("all {} interactables stand clear of the rails", placed.iter().count()));
        }
    }
    for (_, pid, mut p, mut ps, mut tech, procs, mut intent, vis) in &mut q {
        let who = format!("player {}", pid.0);
        // keep everyone standing through the stages (not while the Insurance dip is staged)
        if !probe.insuring.contains(&pid.0) && (ps.dead || ps.hp < ps.stats.max_hp * 0.4) {
            ps.dead = false;
            ps.hp = ps.stats.max_hp;
        }
        match t {
            TECH_GRANT => {
                crate::items::grant_items(&mut ps, &[ItemKind::AntipodeBlink, ItemKind::BoomerangInsurance], &save, global.greed_stacks);
            }
            TECH_BLINK => {
                probe.before.insert(pid.0, p.dir);
                intent.blink = true;
            }
            x if x == TECH_BLINK + 1 => {
                let before = probe.before[&pid.0];
                let off = sphere::arc_dist(p.dir, -before, r);
                if tech.blinks != 1 || off > 1.0 || procs.blink_cd <= 0.0 {
                    probe.fail.push(format!("{who}: the blink key landed {off:.2} m off the antipode (blinks {}, charge {:.1})", tech.blinks, procs.blink_cd));
                }
                intent.blink = true; // pressed again while it recharges
            }
            x if x == TECH_BLINK + 3 => {
                if tech.blinks != 1 {
                    probe.fail.push(format!("{who}: a blink fired while recharging"));
                } else {
                    probe.ok.push(format!("{who}: blink -> antipode, recharge {:.1}s, second press refused", procs.blink_cd));
                }
            }
            TECH_SLAM => {
                probe.holding = true;
                tech.grind = None;
                p.height = 6.0;
                p.vel_r = 0.0;
                p.grounded = false;
                let fwd = p.facing;
                p.vel_t = fwd * run_speed * SPEED_HARD_CAP;
                tech.slam_armed = true;
                tech.slam_hold = 0.0;
                intent.slide_held = true;
                // the ring it will land in, a couple of metres round the drop point
                let (tx, bx) = sphere::tangent_frame(p.dir);
                for k in 0..8 {
                    let a = k as f32 / 8.0 * std::f32::consts::TAU;
                    let d = sphere::offset_dir(p.dir, (tx * a.cos() + bx * a.sin()).normalize(), 2.2, r);
                    stage(&mut commands, EnemyKind::Shambler, d, &mut rng);
                }
            }
            x if x > TECH_SLAM && x < TECH_SLAM_CHECK => {
                intent.slide_held = tech.slam_armed || tech.slam.is_some();
                // Until the dive commits, the speed staged at TECH_SLAM is what it banks: a
                // tall prop the flight brushes (darkmoon's are dense) would strip it and turn
                // the bomb this stage is about into a dud.
                if tech.slam_armed {
                    p.vel_t = p.facing * run_speed * SPEED_HARD_CAP;
                }
            }
            TECH_SLAM_CHECK => {
                intent.slide_held = false;
                if tech.slams < 1 || tech.slam_hits < 1 {
                    probe.fail.push(format!("{who}: the Slam never landed a hit (slams {}, hits {})", tech.slams, tech.slam_hits));
                } else {
                    probe.ok.push(format!("{who}: slam landed, {} hits", tech.slam_hits));
                }
            }
            TECH_PLOW => {
                probe.plowed_at = telemetry.plowed;
                tech.grind = None;
                p.height = 0.0;
                p.vel_r = 0.0;
                p.grounded = true;
                let fwd = p.facing;
                p.vel_t = fwd * run_speed * SLIDE_BOOST;
                p.slide_timer = SLIDE_TIME;
                for k in 0..6 {
                    let d = sphere::offset_dir(p.dir, fwd, 1.5 + k as f32 * 1.1, r);
                    stage(&mut commands, EnemyKind::Shambler, d, &mut rng);
                }
            }
            x if x > TECH_PLOW && x < TECH_PLOW_CHECK => {
                // keep the slide going, as the slope or a held line would
                p.slide_timer = p.slide_timer.max(0.1);
            }
            TECH_PLOW_CHECK if pid.0 == 0 => {
                let shoved = telemetry.plowed - probe.plowed_at;
                if shoved < 3 {
                    probe.fail.push(format!("the slides through a staged line shoved {shoved} Shamblers aside"));
                } else {
                    probe.ok.push(format!("slide plow shoved {shoved} Shamblers aside"));
                }
            }
            TECH_GRIND => {
                let Some(longest) = (0..lines.spines.len()).max_by(|a, b| lines.spines[*a].length().total_cmp(&lines.spines[*b].length())) else {
                    probe.fail.push(format!("{who}: this world has no Grind-Lines"));
                    continue;
                };
                let (d, run) = lines.sample(longest, 1.0 + pid.0 as f32 * 2.0);
                p.dir = d;
                p.height = 0.0;
                p.vel_r = 0.0;
                p.grounded = true;
                p.facing = run;
                p.vel_t = run * run_speed * SLIDE_BOOST;
                p.slide_timer = SLIDE_TIME;
                // off whatever rail the earlier stages' slides found on their own
                tech.grind = None;
                tech.grind_cd = 0.0;
            }
            x if x > TECH_GRIND && x < TECH_GRIND_JUMP => {
                if x == TECH_GRIND + 2 && tech.grind.is_none() {
                    probe.fail.push(format!("{who}: a slide onto a spine did not catch it"));
                }
                if let Some(g) = tech.grind {
                    let off = lines.closest(p.dir).map_or(f32::MAX, |c| c.0);
                    if off > 0.05 || p.height != GRIND_RAIL_LIFT {
                        probe.fail.push(format!("{who}: grinding {off:.3} m off the rail at height {:.2}", p.height));
                    }
                    let target = run_speed * ps.move_speed_mult() * GRIND_SPEED_MULT;
                    if x == TECH_GRIND_JUMP - 1 && (g.speed - target).abs() > 1.0 {
                        probe.fail.push(format!("{who}: rail speed {:.1} m/s, expected ~{target:.1}", g.speed));
                    }
                }
            }
            TECH_GRIND_JUMP => {
                if tech.grind.is_some() {
                    // what player_input does with a jump press
                    p.vel_r = PLAYER_JUMP_VEL;
                    p.grounded = false;
                }
            }
            TECH_GRIND_CHECK => {
                if tech.grinds < 1 || tech.grind_m < 8.0 {
                    probe.fail.push(format!("{who}: rode only {:.1} m of rail ({} catches)", tech.grind_m, tech.grinds));
                } else if tech.grind.is_some() || p.vel_t.length() < run_speed * 1.3 {
                    probe.fail.push(format!("{who}: jumping off the rail kept it ({}), speed {:.1}", tech.grind.is_some(), p.vel_t.length()));
                } else {
                    probe.ok.push(format!("{who}: grind caught, rode {:.1} m, jumped off at {:.1} m/s", tech.grind_m, p.vel_t.length()));
                }
                probe.holding = false;
            }
            TECH_ANTIPODE => {
                // stand still, so the far pole stays where the crowd is staged
                probe.holding = true;
                p.vel_t = Vec3::ZERO;
                if pid.0 != 0 {
                    continue;
                }
                for k in 0..14 {
                    let (tx, bx) = sphere::tangent_frame(-p.dir);
                    let a = k as f32 * 0.9;
                    let d = sphere::offset_dir(-p.dir, (tx * a.cos() + bx * a.sin()).normalize(), 1.0 + k as f32 * 0.6, r);
                    stage(&mut commands, EnemyKind::Shambler, d, &mut rng);
                }
            }
            TECH_ANTIPODE_CHECK => {
                let near = (ANTIPODE_SCAN_ARC / r).cos();
                let truth = enemies.iter().filter(|e| e.speed > 0.0 && e.dir.dot(-p.dir) >= near).count() as i32;
                let got = procs.antipode as i32;
                if (got - truth).abs() > 3.max(truth / 5) || vis.antipode as i32 != got.min(255) {
                    probe.fail.push(format!("{who}: antipode read {got} (hud {}) but {truth} stand there", vis.antipode));
                } else if pid.0 == 0 && got < 10 {
                    probe.fail.push(format!("{who}: the staged far-side crowd read as {got}"));
                } else {
                    probe.ok.push(format!("{who}: antipode read {got} ({:?}), hud {}", crate::techs::AntipodeBand::of(got as u32), vis.antipode));
                }
                probe.holding = false;
            }
            TECH_SLOPE_DOWN | TECH_SLOPE_UP if pid.0 == 0 => {
                // the steepest ground on the world, slid down (then up) it from run speed
                let steep = sphere::fib_sphere(6000)
                    .map(|d| (d, planet.terrain.slope(d, r)))
                    .max_by(|a, b| a.1.length().total_cmp(&b.1.length()));
                let Some((d, up)) = steep else { continue };
                let fall = -up.normalize();
                probe.holding = true;
                p.dir = d;
                p.height = 0.0;
                p.vel_r = 0.0;
                p.grounded = true;
                tech.grind = None;
                tech.grind_cd = 10.0; // no rail may catch this slide
                p.vel_t = if t == TECH_SLOPE_DOWN { fall } else { -fall } * run_speed;
                p.slide_timer = SLIDE_TIME;
                probe.slope_v0 = run_speed;
            }
            x if pid.0 == 0 && (x == TECH_SLOPE_DOWN + 6 || x == TECH_SLOPE_UP + 6) => {
                let v = p.vel_t.length();
                let down = x == TECH_SLOPE_DOWN + 6;
                let grade = planet.terrain.slope(p.dir, r).length();
                let v0 = probe.slope_v0;
                let way = if down { "down" } else { "up" };
                if (down && v < v0 + 0.3) || (!down && v > v0 - 0.3) {
                    probe.fail.push(format!("{who}: a slide {way} a {grade:.2} grade went {v0:.2} -> {v:.2} m/s"));
                } else {
                    probe.ok.push(format!("{who}: slide {way} a {grade:.2} grade: {v0:.2} -> {v:.2} m/s"));
                }
                tech.grind_cd = 0.0;
                probe.holding = false;
            }
            TECH_HOP => {
                // a plain running hop off whatever ground this is, W held
                probe.holding = true;
                tech.grind = None;
                tech.grind_cd = 10.0; // the held slide must not catch a rail instead
                p.height = 0.0;
                p.grounded = true;
                p.slide_timer = 0.0;
                p.land_timer = 10.0;
                p.vel_t = p.facing * run_speed * ps.move_speed_mult();
                p.vel_r = PLAYER_JUMP_VEL * ps.stats.jump_height.sqrt();
                p.grounded = false;
                intent.wish = p.facing;
                probe.hop_top.insert(pid.0, 0.0);
                probe.hop_bank.remove(&pid.0);
            }
            x if x > TECH_HOP && x < TECH_HOP_CHECK => {
                // what player_input does with W held (in the air: `steer`, 60% authority)
                if !p.grounded && tech.slam.is_none() {
                    let drive = run_speed * ps.move_speed_mult();
                    p.vel_t = crate::player::steer(p.vel_t, intent.wish, false, drive, 0.033);
                    let top = probe.hop_top.entry(pid.0).or_default();
                    *top = top.max(p.vel_t.length());
                }
                // ...and with slide pressed in the air, then held
                if x == TECH_HOP_ARM && !p.grounded {
                    tech.slam_armed = true;
                    tech.slam_hold = 0.0;
                }
                intent.slide_held = x >= TECH_HOP_ARM && (tech.slam_armed || tech.slam.is_some());
                if let Some(bank) = tech.slam {
                    probe.hop_bank.insert(pid.0, bank);
                }
            }
            TECH_HOP_CHECK => {
                intent.slide_held = false;
                intent.wish = Vec3::ZERO;
                tech.grind_cd = 0.0;
                probe.holding = false;
                let run = run_speed * ps.move_speed_mult();
                let top = probe.hop_top.get(&pid.0).copied().unwrap_or(0.0);
                match probe.hop_bank.get(&pid.0).copied() {
                    None => probe.fail.push(format!("{who}: the no-ramp hop's Slam never committed")),
                    Some(bank) => {
                        let reach = crate::techs::slam_reach(crate::techs::slam_power(bank, ps.move_speed_mult()));
                        if top > run * 1.05 || reach.t > 0.0 {
                            probe.fail.push(format!(
                                "{who}: a plain hop reached {top:.2} m/s (run {run:.2}) and slammed at {bank:.2} m/s for {:.0}% of the bomb",
                                reach.t * 100.0
                            ));
                        } else {
                            probe.ok.push(format!("{who}: no ramp, no bomb — a hop held forward topped {top:.2} m/s, its Slam a dud"));
                        }
                    }
                }
            }
            x if x >= TECH_INSURE && x < TECH_DONE => {
                if probe.insured.contains(&pid.0) {
                    continue;
                }
                if !probe.insuring.contains(&pid.0) {
                    if procs.blink_cd > 0.0 || !procs.insurance_armed {
                        continue; // wait for a charged, armed policy
                    }
                    probe.insuring.push(pid.0);
                    probe.insure_base.insert(pid.0, tech.blinks);
                    probe.before.insert(pid.0, p.dir);
                }
                if tech.blinks > probe.insure_base[&pid.0] {
                    let off = sphere::arc_dist(p.dir, -probe.before[&pid.0], r);
                    probe.insuring.retain(|i| *i != pid.0);
                    probe.insured.push(pid.0);
                    if off > 1.0 {
                        probe.fail.push(format!("{who}: Boomerang Insurance landed {off:.2} m off the antipode"));
                    } else {
                        probe.ok.push(format!("{who}: Boomerang Insurance paid at {:.0}% HP -> antipode", ps.hp / ps.stats.max_hp * 100.0));
                    }
                    ps.hp = ps.stats.max_hp;
                    continue;
                }
                // just above the line, then a hit that takes it under (evasion may eat one)
                probe.before.insert(pid.0, p.dir);
                ps.iframes = 0.0;
                ps.shield = 0.0;
                ps.hp = ps.stats.max_hp * (BOOMERANG_INSURANCE_HP + 0.02);
            }
            _ => {}
        }
    }
    // the Insurance hits, addressed after the loop (the query is borrowed inside it)
    if t >= TECH_INSURE && t < TECH_DONE {
        for (e, pid, _, ps, _, _, _, _) in &q {
            if probe.insuring.contains(&pid.0) {
                let amount = ps.stats.max_hp * 0.1 / (ps.stats.damage_taken.max(0.1) * (1.0 - ps.effective_armor_fraction()).max(0.05));
                hits.write(crate::messages::PlayerHitMsg { victim: e, amount, from: Vec3::ZERO, attacker: None });
            }
        }
    }
    if t == TECH_DONE {
        let n = q.iter().count();
        if probe.insured.len() < n {
            let paid = probe.insured.clone();
            probe.fail.push(format!("Boomerang Insurance paid for {paid:?} of {n} astronauts"));
        }
        probe.holding = false;
    }
}

/// `--techs`: which astronauts' Slams and blinks came out as TechFx one-shots — the
/// messages the hazard lane carries to joiners.
fn tech_probe_fx(mut probe: ResMut<TechProbe>, mut fx: MessageReader<crate::techs::TechFxMsg>) {
    use crate::techs::TechFx;
    for m in fx.read() {
        match m.fx {
            TechFx::Slam { owner, .. } if !probe.fx_slams.contains(&owner) => probe.fx_slams.push(owner),
            TechFx::Blink { owner, .. } if !probe.fx_blinks.contains(&owner) => probe.fx_blinks.push(owner),
            _ => {}
        }
    }
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

/// `--balance`: what the §3 horde asks of a real build across a WHOLE stage and into The
/// Static. The bot is kept alive (it is too dumb to dodge past ~4 min, so survival says
/// nothing about the late stage) and every 30 s prints the horde's inflow — spawns/s and
/// crowd HP/s at the current `Scaling` — against the party's kills/s and live count. Kills
/// keeping pace with spawns means the build clears the horde; a live count climbing to the
/// cap means it is drowning. This is the number to tune `SCALE_*` against, not bot deaths.
#[derive(Default)]
struct BalanceWindow {
    secs: f32,
    spawned: u32,
    spawned_hp: f32,
    kills: u32,
}

fn balance_probe(
    time: Res<Time>,
    run: Res<RunState>,
    mut q_ps: Query<&mut PlayerState>,
    q_new: Query<&Enemy, (Added<Enemy>, Without<crate::enemies::Boss>)>,
    q_alive: Query<&Enemy>,
    mut kills: MessageReader<crate::messages::KillMsg>,
    mut win: Local<BalanceWindow>,
) {
    // Runs between apply_player_hits and downed_watch, so a lethal hit never ends the run.
    for mut ps in &mut q_ps {
        ps.hp = ps.stats.max_hp;
        ps.dead = false;
    }
    // speed 0 = pots, which share the Enemy component but are scenery
    for e in q_new.iter().filter(|e| e.speed > 0.0) {
        win.spawned += 1;
        win.spawned_hp += e.max_hp;
    }
    win.kills += kills.read().filter(|k| !k.is_pot).count() as u32;
    win.secs += time.delta_secs();
    if win.secs < 30.0 {
        return;
    }
    let w = win.secs;
    let sc = crate::run::scaling::Scaling::for_run(&run, q_ps.iter().count());
    let lead = q_ps.iter().next();
    let weapons: Vec<String> = lead
        .map(|p| p.weapons.iter().map(|w| format!("{}:{}", w.kind.def().name, w.level)).collect())
        .unwrap_or_default();
    println!(
        "  BALANCE t={:>4.0}s stage={} lvl={:<3} horde[hp x{:.2} dmg x{:.2}] spawns/s={:>5.1} crowdHP/s={:>6.0} kills/s={:>5.1} alive={:>4} weapons=[{}]",
        run.total_elapsed,
        run.stage,
        lead.map(|p| p.level).unwrap_or(1),
        sc.hp,
        sc.dmg,
        win.spawned as f32 / w,
        win.spawned_hp / w,
        win.kills as f32 / w,
        q_alive.iter().filter(|e| e.speed > 0.0).count(),
        weapons.join(", ")
    );
    *win = BalanceWindow::default();
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
        .and_then(|_| silver_self_check())
        .and_then(|_| crate::items::self_check(&MetaSave::default()))
        .and_then(|_| crate::save::settings_self_check())
        .and_then(|_| crate::fx::flash_gate_self_check())
        .and_then(|_| crate::ui::settings::ui_scale_self_check())
        .and_then(|_| crate::tomes::self_check())
        .and_then(|_| crate::techs::self_check())
        .and_then(|_| crate::net::edge_presses_self_check());
    match rules {
        Ok(()) => println!("RULES OK (scaling, choice economy, evolution cap, silver, items, settings, flash gate, ui fit, tomes, movement techs, input edges)"),
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
        for t in [crate::content::tomes::TomeKind::Damage, crate::content::tomes::TomeKind::Health] {
            save.tome_levels.insert(t, TOME_MAX_RANK);
            save.tome_loadout.push(t);
        }
    }
    let args: Vec<String> = std::env::args().collect();
    let (probe_tomes, probe_rank) = crate::tomes::tomes_from_args();
    if !probe_tomes.is_empty() {
        // on top of the fast-boss veteran loadout, so the bot lives to see the tomes act
        let slotted = crate::tomes::save_with(&probe_tomes, probe_rank);
        save.tome_levels.extend(slotted.tome_levels);
        for t in slotted.tome_loadout {
            if !save.tome_loadout.contains(&t) {
                save.tome_loadout.push(t);
            }
        }
        save.tome_slots = save.tome_slots.max(save.tome_loadout.len() as u32);
    }
    let assist = args.iter().any(|a| a == "--assist");
    if assist {
        save.assist = crate::save::AssistOptions { enemy_density: 0.5, enemy_damage: 0.5, revive_token: true };
    }
    let coop2 = args.iter().any(|a| a == "--coop2");
    if assist && !coop2 && args.iter().any(|a| a == "--deathsave") {
        // Both probes stage lethal hits on the local astronaut and would eat each other's
        // saves; with --coop2 the death-save victim is the peer and the two compose.
        println!("SMOKE FAIL: --deathsave with --assist needs --coop2");
        std::process::exit(2);
    }
    if !probe_tomes.is_empty() && args.iter().any(|a| a == "--techs") {
        // Both probes stage their scenes by moving the astronauts (the tomes' to the night
        // side and the Microwave, the techs' onto rails, ramps and the antipode) on
        // overlapping ticks, and would fail each other's checks: run them separately.
        println!("SMOKE FAIL: --techs and --tomes stage conflicting scenes; run them separately");
        std::process::exit(2);
    }
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
        // The host path: headless IS the host (or solo), so these stay at their defaults —
        // but the shared presentation systems read them.
        .init_resource::<crate::net::NetRole>()
        .init_resource::<crate::net::MyPlayerId>()
        .insert_resource(probe)
        .init_resource::<XpTally>()
        .init_resource::<crate::items::ItemTelemetry>()
        .insert_resource(ItemProbe {
            items: crate::items::items_from_args(),
            deathsave: args.iter().any(|a| a == "--deathsave"),
            ..default()
        })
        .init_resource::<AssistProbe>()
        .insert_resource(TomeProbe { tomes: probe_tomes.clone(), rank: probe_rank, ..default() })
        .insert_resource(TechProbe { on: args.iter().any(|a| a == "--techs"), ..default() })
        .init_resource::<crate::techs::GrindLines>()
        .init_resource::<crate::techs::TechTelemetry>()
        .init_resource::<crate::fx::FlashGate>()
        .init_resource::<crate::fx::ScreenFlash>()
        .init_resource::<crate::coop::CoopTelemetry>()
        .init_resource::<crate::duos::DuoLedger>()
        .init_resource::<crate::duos::CascadeState>()
        .insert_resource(CoopProbe18::from_args())
        .insert_resource(save)
        .insert_resource(run)
        .add_message::<crate::net::GrantOut>()
        .add_message::<crate::coop::FriendlyForce>()
        .add_message::<crate::coop::CoopFxMsg>()
        .add_message::<crate::duos::DuoMsg>()
        .add_message::<crate::items::ItemFxMsg>()
        .add_message::<crate::techs::TechFxMsg>()
        .add_message::<crate::messages::HitMsg>()
        .add_message::<crate::messages::PlayerHitMsg>()
        .add_message::<crate::messages::KillMsg>()
        .add_message::<crate::messages::NumberMsg>()
        .add_message::<crate::messages::BannerMsg>()
        .add_message::<crate::messages::SfxMsg>()
        .add_systems(Startup, (crate::enemies::setup_enemy_assets, crate::combat::setup_weapon_assets, crate::pickups::setup_pickup_assets, crate::items::setup_item_assets, crate::techs::setup_tech_assets, crate::coop::setup_coop_assets, headless_enter))
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
                crate::enemies::crack_decals,
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
                crate::comet::comet_presentation,
                crate::events_world::dust_storm_sim,
                storm_peer_pin.run_if(|p: Res<CoopProbe>| p.storm_peer),
                crate::events_world::dust_storm_visuals,
                storm_peer_check.run_if(|p: Res<CoopProbe>| p.storm_peer),
            )
                .chain()
                .run_if(crate::playing),
        )
        // §7 items — the same set main.rs runs (headless IS the host, so the client-only
        // trail drops simply never run)
        .add_systems(
            Update,
            (
                crate::items::item_upkeep,
                crate::items::encirclement_scan,
                crate::items::orbital_yoyo,
                crate::items::comet_tail,
                crate::items::little_black_hole,
                crate::items::orbit_chunks,
                crate::items::trail_patches,
                crate::items::trail_client_drops.run_if(crate::net::is_client),
                crate::items::singularity_update,
                crate::items::push_net_item_vis,
                crate::items::item_visuals,
                crate::items::apply_sun_shrink,
            )
                .chain()
                .run_if(crate::playing),
        )
        // §4 movement techs — the host's half as main.rs runs it (headless IS the host);
        // the moves themselves are player_physics below
        .add_systems(
            Update,
            (
                crate::techs::antipode_blink,
                crate::techs::consume_edge_intents,
            )
                .chain()
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                crate::techs::antipode_scan,
                crate::techs::slam_shockwave.after(crate::player::player_physics),
                crate::techs::slide_plow,
                crate::techs::animate_tech_fx,
                tech_probe
                    .after(bot_drive)
                    .before(crate::player::player_physics)
                    .before(crate::techs::antipode_blink)
                    .before(crate::combat::apply_player_hits)
                    .run_if(|p: Res<TechProbe>| p.on),
                tech_probe_fx.run_if(|p: Res<TechProbe>| p.on),
            )
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            crate::techs::tech_fx_presentation
                .after(crate::techs::slam_shockwave)
                .after(crate::techs::antipode_blink)
                .run_if(resource_exists::<crate::planet::CurrentPlanet>),
        )
        // as in main.rs: item one-shots are presented behind a card panel too
        .add_systems(
            Update,
            crate::items::item_fx_presentation
                .after(crate::items::apply_sun_shrink)
                .run_if(resource_exists::<crate::planet::CurrentPlanet>),
        )
        .add_systems(
            Update,
            (
                item_probe_grant,
                item_probe_drive.after(bot_drive),
                item_probe_fx,
                deathsave_probe.before(crate::combat::apply_player_hits),
            )
                .run_if(|p: Res<ItemProbe>| !p.items.is_empty() || p.deathsave)
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                tome_probe_setup,
                tome_probe_drive.after(bot_drive).before(crate::interact::interact_system),
                tome_probe_watch,
                tome_probe_withdraw
                    .after(tome_probe_drive)
                    .run_if(|p: Res<TomeProbe>| p.tomes.contains(&crate::content::tomes::TomeKind::Horizon))
                    .run_if(|| std::env::args().any(|a| a == "--coop2")),
            )
                .run_if(|p: Res<TomeProbe>| !p.tomes.is_empty())
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            static_now.run_if(crate::playing).run_if(|| std::env::args().any(|a| a == "--staticnow")),
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
                crate::interact::sync_reward_cache.before(crate::director::stage_transition),
                bot_watchdog,
                balance_probe
                    .after(crate::combat::apply_player_hits)
                    .before(crate::director::downed_watch)
                    .run_if(|| std::env::args().any(|a| a == "--balance")),
                tally_xp,
                crate::director::sync_assist_options,
                assist_probe_hit.before(crate::combat::apply_player_hits).run_if(move || assist),
                assist_probe_check
                    .after(crate::combat::apply_player_hits)
                    .before(crate::director::downed_watch)
                    .run_if(move || assist),
            ),
        )
        .add_systems(
            Update,
            crate::director::dev_miniboss_now
                .run_if(crate::playing)
                .run_if(|| std::env::args().any(|a| a == "--minibossnow")),
        )
        // §11 co-op rules (P18) — the host's half as main.rs runs it, plus the presentation
        // that carries logic worth exercising (the Beacon's pose and dressing, the belt)
        .add_systems(
            Update,
            (
                crate::coop::friendly_physics.before(crate::player::player_physics),
                crate::coop::beacon_rescue.after(crate::player::player_physics),
                crate::coop::orbital_drops.after(crate::player::player_physics),
                crate::duos::static_cascade,
                crate::duos::duo_payoffs.after(crate::combat::apply_hits),
                crate::coop::tumble_pose.after(crate::player::player_physics),
                crate::coop::hide_claimed,
                crate::coop::beacon_flares,
                crate::coop::coop_fx_presentation,
                crate::duos::animate_cascade_belts,
            )
                .run_if(crate::playing),
        )
        .add_systems(
            Update,
            (
                coop_probe_fx,
                revive_probe
                    .after(bot_drive)
                    .before(crate::player::player_physics)
                    .run_if(|p: Res<CoopProbe18>| p.revive),
                cascade_probe
                    .after(bot_drive)
                    .before(crate::player::player_physics)
                    .run_if(|p: Res<CoopProbe18>| p.cascade),
                duos_probe.run_if(|p: Res<CoopProbe18>| p.duos),
                dropin_probe.after(bot_drive).run_if(|p: Res<CoopProbe18>| p.dropin),
                party_probe.run_if(|p: Res<CoopProbe18>| p.coop4),
            )
                .run_if(crate::playing),
        )
        // `--dropin`: the drop-in is driven through its intent, as a joiner's body is on a
        // host — the host's autopilot writes that intent while its player is idle
        .add_systems(
            Update,
            (crate::coop::autopilot_peers, crate::player::player_input)
                .chain()
                .after(bot_drive)
                .before(crate::player::player_physics)
                .run_if(crate::playing)
                .run_if(|p: Res<CoopProbe18>| p.dropin),
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
        .as_ref()
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
    let silver = {
        let save = world.resource::<MetaSave>();
        let golden = save.tome_rank_equipped(crate::content::tomes::TomeKind::Golden);
        let (rocks, gain, static_silver) = ps
            .as_ref()
            .map(|p| (p.item_count(crate::content::items::ItemKind::CursedMoonRock), p.stats.silver_gain, p.stats.static_silver))
            .unwrap_or((0, 1.0, 1.0));
        crate::director::silver_payout(&run, false, golden, rocks, gain, static_silver)
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
    let xp_grants = world.resource::<XpTally>().0;
    // `--staticnow` winds the run into The Static within seconds, and while it is up no
    // kill drops XP (`pickups::kill_drops` pays its ghosts' Silver instead): the XP pipeline
    // is judged on the runs that walk a horde, not on this one.
    let xp_run = !std::env::args().any(|a| a == "--staticnow");
    if xp_run && run.kills > 50 && xp_grants == 0 && gems_left == 0 {
        println!("FAIL: XP pipeline dead (kills dropped no gems)");
        ok = false;
    }
    if xp_grants > 0 && best_level < 2 && !any_xp {
        println!("FAIL: XP pipeline dead ({xp_grants} gems collected, no XP, no levels)");
        ok = false;
    }
    // The collection link and the level curve, judged where they are certain: outside
    // fast-boss the bot walks the horde it kills, and the magnet reaches most drops. The
    // collection threshold is LOW on purpose: with no XP the bot never levels and dies
    // early, so a dead pickup path shows up as a short run with a handful of kills.
    if xp_run && !fast_boss && run.kills >= 10 && xp_grants == 0 {
        println!("FAIL: XP pipeline dead ({gems_left} gems on the ground, none ever collected)");
        ok = false;
    }
    if xp_run && !fast_boss && run.kills > 50 && best_level < 2 {
        println!("FAIL: XP pipeline dead ({xp_grants} gems collected, still level 1)");
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
    // §7 items: every item handed out must have visibly DONE its thing, on every astronaut
    // that carries it (with --coop2 the peer's items are the host simulating a joiner's).
    let item_sheets: Vec<(u8, PlayerState, crate::net::NetItemVis)> = world
        .query::<(&crate::player::PlayerId, &PlayerState, &crate::net::NetItemVis)>()
        .iter(world)
        .map(|(pid, ps, vis)| (pid.0, ps.clone(), *vis))
        .collect();
    let item_probe = world.resource::<ItemProbe>();
    if !item_probe.items.is_empty() || item_probe.deathsave {
        let tel = world.resource::<crate::items::ItemTelemetry>();
        println!(
            "ITEMS yoyo[throws={} hits={}] tail[patches={} ignites={} hits={}] jams={} ghost={} ring={} hover={:.1}s air={:.1}s hole[{} pulled={}] encircle={}/8 descent={:.2}m sun_steps={} radio_static_at={:?} saves={:?} fx_owners={:?}",
            tel.yoyo_throws, tel.yoyo_hits, tel.trail_patches, tel.ignites, tel.trail_hits, tel.jams,
            tel.ghost_volleys, tel.ring_volleys, tel.hover_secs, tel.airborne_secs, tel.singularities,
            tel.pulled, tel.max_encircle, tel.max_descent, tel.sun_steps, tel.radio_static_at, tel.saves,
            item_probe.fx_owners
        );
        use crate::content::items::ItemKind as I;
        let has = |i: I| item_probe.items.contains(&i);
        let mut fails: Vec<String> = Vec::new();
        let mut need = |cond: bool, what: &str| {
            if !cond {
                fails.push(what.to_string());
            }
        };
        if has(I::OrbitalYoYo) {
            need(tel.yoyo_throws > 0 && tel.yoyo_hits > 0, "Orbital Yo-Yo never hit anything");
        }
        if has(I::CometTail) {
            need(tel.trail_patches > 0 && tel.trail_hits > 0, "Comet Tail laid no burning trail");
            need(tel.ignites > 0, "Comet Tail never ignited on a stand-still");
        }
        if has(I::TheOverheat) {
            need(tel.jams > 0, "The Overheat never jammed");
        }
        if has(I::DownhillMomentum) {
            need(tel.max_descent > 0.0, "Downhill Momentum never read a descent");
        }
        if has(I::SecondAstronaut) {
            need(tel.ghost_volleys > 0, "Second Astronaut's ghost never fired");
        }
        if has(I::EncirclementBonus) {
            need(tel.max_encircle > 0, "Encirclement Bonus never saw the horde");
        }
        if has(I::IcarusBoots) {
            need(tel.airborne_secs > 0.0, "Icarus Boots: never airborne");
        }
        if has(I::AntiGravBoots) {
            need(tel.hover_secs > 0.0, "Anti-Grav Boots never hovered");
            need(tel.ring_volleys > 0, "Anti-Grav Boots never fired a 360 ring");
        }
        if has(I::LittleBlackHole) {
            need(tel.singularities > 0 && tel.pulled > 0, "Little Black Hole pulled nothing");
        }
        if has(I::StaticRadio) {
            need(run.static_radio, "The Static Radio did not reach RunState");
            if run.timer <= STATIC_RADIO_LEAD_SECS && fast_boss {
                need(run.static_active && tel.radio_static_at.is_some(), "The Static Radio did not bring The Static early");
            }
        }
        // held for a full period (the probe's own clock: fast-boss winds total_elapsed)
        if has(I::DevouredSunShard) && item_probe.ticks as f32 * 0.033 > SUN_SHARD_PERIOD + 1.0 {
            need(tel.sun_steps > 0 && run.sun_shrink > 0.0, "Devoured Sun Shard never shrank the sun");
        }
        for (pid, ps, vis) in &item_sheets {
            let who = format!("player {pid}");
            if has(I::CrackedHelmet) {
                need(ps.stats.damage_taken >= 2.0, &format!("{who}: Cracked Helmet does not double damage taken"));
            }
            if has(I::WidowsRing) {
                need((ps.stats.max_hp_mult - 0.8).abs() < 1e-3, &format!("{who}: Widow's Ring did not cut max HP"));
            }
            if has(I::SignalFlare) {
                need(ps.revealed() && ps.stats.xp_gain >= 1.4, &format!("{who}: Signal Flare not live"));
            }
            // the replicated look must match the simulated state (what a joiner draws)
            let ghost = ps.ghost_weapon.map(|w| w.code() + 1).unwrap_or(0);
            need(vis.ghost == ghost, &format!("{who}: NetItemVis ghost {} but sheet {ghost}", vis.ghost));
            need(
                (vis.flags & crate::net::ITEMVIS_TETHER_SPENT != 0) == ps.tether_used,
                &format!("{who}: NetItemVis tether flag disagrees with the sheet"),
            );
        }
        let coop2 = std::env::args().any(|a| a == "--coop2");
        if coop2 && has(I::OrbitalYoYo) {
            need(item_probe.fx_owners.contains(&1), "the host never simulated the peer's items (no item event owned by player 1)");
        }
        if item_probe.deathsave {
            for l in &item_probe.ds_log {
                println!("  DEATHSAVE {l}");
            }
            match &item_probe.ds_fail {
                Some(f) => need(false, &format!("death-save: {f}")),
                None => need(item_probe.ds_stage == DS_DONE, &format!("death-save probe never finished (stage {})", item_probe.ds_stage)),
            }
            if item_probe.ds_fail.is_none() && item_probe.ds_stage == DS_DONE {
                println!(
                    "DEATHSAVE OK: Tether rewound {:.1} m, then Widow's Ring held at 1 HP{}{}",
                    tel.rewinds.first().copied().unwrap_or(0.0),
                    if coop2 && run.assist.revive_token { ", then the one-more-chance token (last link)" } else { "" },
                    if coop2 { ", then the spent peer went down" } else { "" }
                );
            }
        }
        if fails.is_empty() {
            println!("ITEMS OK ({} items)", item_probe.items.len());
        } else {
            for f in fails {
                println!("FAIL: {f}");
            }
            ok = false;
        }
    }
    // §7 tomes: every slotted tome must have visibly DONE its thing, on every astronaut
    // (with --coop2 the peer's lines are the host simulating a joiner's).
    let tome_probe = world.resource::<TomeProbe>();
    if !tome_probe.tomes.is_empty() {
        use crate::content::tomes::TomeKind as T;
        let tel = tome_probe;
        let items_tel = world.resource::<crate::items::ItemTelemetry>();
        // what Tome of Static adds at banking, by the banker's own sheet
        let static_pay = |mult: f32| crate::director::silver_payout(&run, false, 0, 0, 1.0, mult).total;
        let static_mult = ps.as_ref().map(|p| p.stats.static_silver).unwrap_or(1.0);
        println!(
            "TOMES crowd=+{:.0}% night={:.1}s fast_fall={:.1}s momentum=+{:.0}% ricochets={} horizon[calls={} home={}] elite[kills={} richer={} boss_richer={}] drone_scale={:.2} yoyo_swing=x{:.2} yoyo_throws={}",
            tel.max_crowd_bonus * 100.0,
            tel.night_secs,
            tel.fast_fall_secs,
            tel.max_momentum_bonus * 100.0,
            tel.ricochets,
            tel.horizon_calls,
            tel.horizon_arrivals,
            tel.elite_kills,
            tel.elite_richer,
            tel.boss_richer,
            tome_probe.drone_scale,
            tome_probe.yoyo_ratio,
            items_tel.yoyo_throws
        );
        if run.static_secs_total > 0.0 || !tel.microwave.is_empty() {
            println!(
                "  TOMES static[{:.0}s ghost_silver={} ghost_hits={} tome_pays=+{}] microwave{:?}",
                run.static_secs_total,
                run.static_silver_found,
                tel.ghost_hits,
                static_pay(static_mult) - static_pay(1.0),
                tel.microwave
            );
        }
        let has = |t: T| tome_probe.tomes.contains(&t) && tome_probe.rank > 0;
        // what one line is worth at the probe's rank
        let at = |t: T, i: usize| t.def().effects[i].at(tome_probe.rank);
        let near = |a: f32, b: f32| (a - b).abs() < 1e-3;
        let mut fails: Vec<String> = Vec::new();
        let mut need = |cond: bool, what: String| {
            if !cond {
                fails.push(what);
            }
        };
        if has(T::Encirclement) {
            need(tel.max_crowd_bonus > 0.0, "Tome of Encirclement never counted a crowd".into());
        }
        if has(T::Nightfall) {
            need(tel.night_secs > 0.0, "Tome of Nightfall never saw the night side".into());
        }
        if has(T::Gravity) {
            need(tel.fast_fall_secs > 0.0, "Tome of Gravity never pulled a fall down faster".into());
        }
        if has(T::Momentum) {
            need(tel.max_momentum_bonus > 0.0, "Tome of Momentum never built momentum".into());
        }
        if has(T::Ricochet) {
            need(tel.ricochets > 0, "Tome of Ricochet: no shot ever skipped".into());
        }
        if has(T::Horizon) {
            need(tel.horizon_calls > 0 && tel.horizon_arrivals > 0, "Tome of the Horizon never called XP home".into());
            if std::env::args().any(|a| a == "--coop2") {
                need(tel.withdrawn == Some(true), format!("Tome of the Horizon: a downed caller's gem was not withdrawn ({:?})", tel.withdrawn));
            }
        }
        if has(T::Orbit) {
            let orbit = 1.0 + at(T::Orbit, 0);
            need(near(tome_probe.drone_scale, orbit), format!("Tome of Orbit: drones at x{:.2}, want x{orbit:.2}", tome_probe.drone_scale));
            need(items_tel.yoyo_throws == 0 || near(tome_probe.yoyo_ratio, orbit), format!("Tome of Orbit: yo-yo swing x{:.2}", tome_probe.yoyo_ratio));
        }
        if has(T::Swarm) {
            need(items_tel.yoyo_throws > 0, "Tome of the Swarm: the Orbital Yo-Yo never procced".into());
        }
        if has(T::Elite) {
            need(near(run.elite_loot, 1.0 + at(T::Elite, 0)), format!("Tome of the Elite: party elite loot x{:.2}", run.elite_loot));
            need(
                tel.elite_kills > 0 && tel.elite_richer == tel.elite_kills,
                format!("Tome of the Elite: {} of {} elite kills dropped richer", tel.elite_richer, tel.elite_kills),
            );
            need(tel.boss_richer == 0, format!("Tome of the Elite made {} boss/miniboss kills richer", tel.boss_richer));
        }
        // The Static, when the run reached it (`--staticnow`): its ghosts' coins were found
        // and the banker's tome pays on them and on the overtime, once, unrounded per coin
        if has(T::Static) && run.static_secs_total > 5.0 {
            let mult = 1.0 + at(T::Static, 0);
            let want = (run.static_secs_total * SILVER_PER_STATIC_SEC * mult) as u64 - (run.static_secs_total * SILVER_PER_STATIC_SEC) as u64
                + (run.static_silver_found as f32 * (mult - 1.0)).round() as u64;
            need(run.static_silver_found > 0, "Tome of Static: no ghost Silver was found in The Static".into());
            need(
                static_pay(static_mult) - static_pay(1.0) == want,
                format!("Tome of Static paid +{} at banking, want +{want}", static_pay(static_mult) - static_pay(1.0)),
            );
        }
        // Tome of Duplication at the Microwave: the free use first (the stage's own left), then
        // the stage's own, each a duplicate
        if has(T::Duplication) {
            let ok = match tel.microwave.as_slice() {
                [(1, false, a), (0, false, b), (0, true, c)] => b > a && c > b,
                _ => false,
            };
            need(ok, format!("Tome of Duplication at the Microwave went {:?}", tel.microwave));
        }
        for (pid, ps, _) in &item_sheets {
            let who = format!("player {pid}");
            if let Some((_, banish, refresh, cap, free)) = tome_probe.start.iter().find(|s| s.0 == *pid) {
                if has(T::Banishment) {
                    need(*banish == BANISH_CHARGES + 1 && *refresh == FREE_REFRESHES + 1, format!("{who}: Tome of Banishment began at {banish} banishes / {refresh} refreshes"));
                }
                if has(T::Ascension) {
                    need(*cap == EVOLUTION_CAP + 1, format!("{who}: Tome of Ascension began with an evolution cap of {cap}"));
                }
                if has(T::Duplication) {
                    need(*free == 1, format!("{who}: Tome of Duplication began with {free} free Microwave uses"));
                }
            } else {
                need(false, format!("{who}: never seen at the start of the run"));
            }
            if has(T::Nightfall) {
                need(near(ps.stats.flashlight, 1.0 + at(T::Nightfall, 1)), format!("{who}: flashlight x{:.1}", ps.stats.flashlight));
            }
            if has(T::Salvage) {
                need(ps.stats.chest_discount >= at(T::Salvage, 0) - 1e-3, format!("{who}: chest discount {:.2}", ps.stats.chest_discount));
            }
            if has(T::Vampirism) {
                need(ps.stats.lifesteal > at(T::Vampirism, 0) + 0.05, format!("{who}: lifesteal {:.3} (Visor not boosted?)", ps.stats.lifesteal));
            }
            if has(T::Static) {
                need(near(ps.stats.static_silver, 1.0 + at(T::Static, 0)), format!("{who}: Static Silver x{:.1}", ps.stats.static_silver));
            }
            if has(T::Horizon) {
                need(ps.stats.horizon_collect > 0.0, format!("{who}: no Horizon line on the sheet"));
            }
        }
        if fails.is_empty() {
            println!("TOMES OK ({} tomes at rank {})", tome_probe.tomes.len(), tome_probe.rank);
        } else {
            for f in fails {
                println!("FAIL: {f}");
            }
            ok = false;
        }
    }
    // §4 movement techs
    let tech_probe = world.resource::<TechProbe>();
    if tech_probe.on {
        let tel = world.resource::<crate::techs::TechTelemetry>();
        let lines = world.resource::<crate::techs::GrindLines>();
        println!(
            "TECHS rails={} ({:.0} m) slams={} (duds {}) hits={} best_power={:.2} blinks={} insured={} plowed={} scans={} fx[slams {:?} blinks {:?}]",
            lines.spines.len(), lines.total_length(), tel.slams, tel.slam_duds, tel.slam_hits, tel.best_slam_power,
            tel.blinks, tel.insured_blinks, tel.plowed, tel.scans, tech_probe.fx_slams, tech_probe.fx_blinks
        );
        for l in &tech_probe.ok {
            println!("  TECH {l}");
        }
        let mut fails = tech_probe.fail.clone();
        if tech_probe.ticks < TECH_DONE {
            fails.push(format!("the probe needs {TECH_DONE} ticks, the run gave it {}", tech_probe.ticks));
        }
        let n = peers.len() as u8;
        for id in 0..n {
            if !tech_probe.fx_slams.contains(&id) || !tech_probe.fx_blinks.contains(&id) {
                fails.push(format!("player {id}'s Slam/blink never became a TechFx (what a joiner is sent)"));
            }
        }
        if fails.is_empty() {
            println!("TECHS OK (blink, slam, plow, grind, antipode read, slope-boost, Boomerang Insurance)");
        } else {
            for f in fails {
                println!("FAIL: techs: {f}");
            }
            ok = false;
        }
    }
    // §13: a run with every assist off must never be flagged; one with them must be.
    if run.assisted != assist {
        println!("FAIL: run.assisted={} with --assist {}", run.assisted, if assist { "on" } else { "off" });
        ok = false;
    }
    if assist {
        let probe = world.resource::<AssistProbe>();
        let cap = crate::run::scaling::Scaling::for_run(&run, 1).live_cap;
        println!(
            "ASSIST damage={:?} revived={} nova_pushed={}/{} second_hit_downed={} live_cap={cap}",
            probe.damage_ok, probe.revived, probe.nova_pushed, probe.nova_near.len(), probe.second_downed
        );
        if probe.damage_ok.is_none() {
            println!("FAIL: the assisted damage hit never landed clean");
            ok = false;
        }
        if !probe.second_downed {
            println!("FAIL: the one-more-chance sequence did not complete (stage {})", probe.stage);
            ok = false;
        }
        if !probe.nova_near.is_empty() && probe.revived && probe.nova_pushed == 0 {
            println!("FAIL: the revive nova pushed none of {} nearby enemies", probe.nova_near.len());
            ok = false;
        }
        if cap != ENEMY_CAP / 2 {
            println!("FAIL: 50% density should halve the live cap, got {cap}");
            ok = false;
        }
        if ok {
            println!("ASSIST OK: density, damage, one more chance, run flagged");
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
    if !coop_probe_report(world) {
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
    let (props, rails) = crate::planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    commands.insert_resource(props);
    crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 0, run_state.character, true, None);
    // `--coop2` reproduces a 2-player HOST headlessly. Without it none of the multi-player
    // work is testable without launching two windows by hand.
    if std::env::args().any(|a| a == "--coop2") {
        crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, 1, run_state.character, false, None);
    }
    // `--coop4`: the full §11 squad — three peers, the ids a 4-player lobby seats
    if std::env::args().any(|a| a == "--coop4") {
        for id in 1..crate::net::MAX_PLAYERS as u8 {
            crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &save, id, run_state.character, false, None);
        }
    }
    crate::interact::spawn_interactables(&mut commands, &mut meshes, &mut materials, &planet, &run_state, &PlayerState::new(run_state.character, &save), &save, &rails, Vec3::Y);
    commands.insert_resource(rails);
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
    let p = crate::director::silver_payout(&run, true, 2, 2, 1.0, 1.0);
    // (100 + 250 + 150 + 100 + 30) × 1.10 × 1.30 = 900.9 → 901, + 7 found
    if p.total != 908 {
        return Err(format!("silver formula gave {} for the reference run, want 908", p.total));
    }
    let dead = crate::director::silver_payout(&run, false, 0, 0, 1.0, 1.0);
    if dead.total != 100 + 250 + 150 + 30 + 7 {
        return Err(format!("a death should bank everything but the tier bonus, got {}", dead.total));
    }
    Ok(())
}

// ─── §11 co-op rules (P18) ────────────────────────────────────────────────────

/// The §11 probes (P18), each staged on the real paths and asserted:
///   * `--revive`  (with `--coop2`/`--coop4`): the peer goes down on a slope and must ROLL
///     downhill, fill the Static Meter at its rate, ignore shots and shrine rings (L18/L19),
///     come back up after REVIVE_SECS in the host's ring at REVIVE_HP_FRAC with Hero's
///     Adrenaline for the host; then, left alone, be claimed by The Static (hidden, out of
///     the party scale) and rejoin through the next teleporter at REJOIN_HP_FRAC (M15).
///   * `--cascade` (with `--coop2`): two STORM CORE owners 60° apart never link; 150° apart
///     they charge and wrap the planet — hits, belt, tally — then wait out the cooldown.
///   * `--duos`    (with `--coop2`): each named duo through the real `apply_hits` (setup by
///     the host, finisher by the peer; the melt multiplier, the shatter burst), and neither
///     a solo "combo" nor one past the window counts.
///   * `--dropin`: a peer seated mid-run lands from orbit at half the squad's level, and
///     the autopilot fights for it through its grace — then stops.
///   * `--coop4`:  a party of four: the §11 HP scale on every crowd spawn and the boss.
#[derive(Resource, Default)]
struct CoopProbe18 {
    revive: bool,
    cascade: bool,
    duos: bool,
    dropin: bool,
    coop4: bool,
    ticks: u32,
    stage: u8,
    t0: u32,
    fails: Vec<String>,
    notes: Vec<String>,
    done: bool,
    // --revive
    down_dir: Vec3,
    down_h: f32,
    grade: f32,
    shrine: Option<Entity>,
    shot: Option<Entity>,
    // --cascade
    idle_charge: f32,
    // --duos
    staged: Vec<Entity>,
    // --dropin
    peer_peak: f32,
    peer_path: f32,
    peer_last: Vec3,
    expect_level: u32,
    // --coop4
    crowd_checked: u32,
    crowd_bad: u32,
    boss_ratio: Option<(f32, f32)>,
    /// CoopFx one-shots seen, by kind: revived, shove, cascade, duo, drop-in.
    fx: [u32; 5],
}

impl CoopProbe18 {
    fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let has = |f: &str| args.iter().any(|a| a == f);
        // exactly one peer: a squad of four has other bots who would answer the Beacon
        let peer = has("--coop2") && !has("--coop4");
        Self {
            revive: peer && has("--revive"),
            cascade: peer && has("--cascade"),
            duos: peer && has("--duos"),
            dropin: has("--dropin") && !has("--coop2") && !has("--coop4"),
            coop4: has("--coop4"),
            ..default()
        }
    }
    fn any(&self) -> bool {
        self.revive || self.cascade || self.duos || self.dropin || self.coop4
    }
    fn fail(&mut self, why: impl Into<String>) {
        let why = why.into();
        println!("  COOP PROBE FAIL: {why}");
        self.fails.push(why);
        self.done = true;
    }
    fn next(&mut self) {
        self.stage += 1;
        self.t0 = self.ticks;
    }
    fn secs_in_stage(&self) -> f32 {
        (self.ticks - self.t0) as f32 * 0.033
    }
}

fn coop_probe_fx(mut probe: ResMut<CoopProbe18>, mut fx: MessageReader<crate::coop::CoopFxMsg>) {
    use crate::coop::CoopFx;
    for m in fx.read() {
        let k = match m.fx {
            CoopFx::Revived { .. } => 0,
            CoopFx::Shove { .. } => 1,
            CoopFx::Cascade { .. } => 2,
            CoopFx::Duo { .. } => 3,
            CoopFx::DropIn { .. } => 4,
        };
        probe.fx[k] += 1;
    }
}

/// A direction `arc` metres from `from`, heading `h` radians round its tangent frame.
fn offset(from: Vec3, h: f32, arc: f32, radius: f32) -> Vec3 {
    let (t, b) = sphere::tangent_frame(from);
    sphere::offset_dir(from, (t * h.cos() + b * h.sin()).normalize(), arc, radius)
}

fn pin(p: &mut Player, dir: Vec3) {
    p.dir = dir;
    p.vel_t = Vec3::ZERO;
    p.vel_r = 0.0;
    p.height = 0.0;
    p.grounded = true;
}

/// `--revive`: see `CoopProbe18`. Runs after the bot, before the physics.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn revive_probe(
    mut commands: Commands,
    mut probe: ResMut<CoopProbe18>,
    mut run: ResMut<RunState>,
    mut pending: ResMut<crate::director::PendingStage>,
    planet: Res<CurrentPlanet>,
    telemetry: Res<crate::coop::CoopTelemetry>,
    mut q: Query<(Entity, &crate::player::PlayerId, &mut Player, &mut PlayerState, &Visibility)>,
    mut shrines: Query<(Entity, &mut crate::interact::ChargeShrine, &Transform), Without<Player>>,
    shots: Query<(), With<crate::enemies::EnemyProjectile>>,
    mut hits: MessageWriter<crate::messages::PlayerHitMsg>,
) {
    if probe.done {
        return;
    }
    probe.ticks += 1;
    let tick = probe.ticks;
    if tick == 1 {
        // a two-world chain, so the rejoin has a teleporter to go through
        let next = if planet.kind == PlanetKind::Mars { PlanetKind::Moon } else { PlanetKind::Mars };
        run.chain = vec![run.chain[0], next];
        run.tier = 2;
        return;
    }
    let find = |q: &Query<(Entity, &crate::player::PlayerId, &mut Player, &mut PlayerState, &Visibility)>, id: u8| {
        q.iter().find(|(_, pid, ..)| pid.0 == id).map(|(e, _, p, ps, v)| (e, p.dir, ps.dead, ps.claimed, *v))
    };
    // (either can be missing for the frame a stage change re-embodies them)
    let (Some((host_e, host_dir, ..)), Some((peer_e, peer_dir, peer_dead, peer_claimed, peer_vis))) = (find(&q, 0), find(&q, 1)) else {
        return;
    };
    let r = planet.radius;
    if probe.stage > 0 && probe.secs_in_stage() > 12.0 {
        let s = probe.stage;
        probe.fail(format!("revive probe stuck in stage {s}"));
        return;
    }
    match probe.stage {
        // on a slope near the host, the peer goes down
        0 if tick >= 45 => {
            let mut best = (host_dir, 0.0f32);
            for k in 0..24 {
                for arc in [10.0, 16.0, 22.0, 30.0] {
                    let d = offset(host_dir, k as f32 * 0.2618, arc, r);
                    let g = planet.terrain.slope(d, r).length();
                    if g > best.1 {
                        best = (d, g);
                    }
                }
            }
            probe.down_dir = best.0;
            probe.grade = best.1;
            probe.down_h = planet.surface(best.0);
            if let Ok((_, _, mut p, _, _)) = q.get_mut(peer_e) {
                pin(&mut p, best.0);
            }
            hits.write(crate::messages::PlayerHitMsg { victim: peer_e, amount: 1.0e6, from: Vec3::ZERO, attacker: None });
            probe.next();
        }
        // alone for two seconds: it rolls, the meter fills at its rate
        1 => {
            let far = offset(peer_dir, 0.0, 60.0, r);
            if let Ok((_, _, mut p, _, _)) = q.get_mut(host_e) {
                pin(&mut p, far);
            }
            if probe.secs_in_stage() < 2.0 {
                return;
            }
            let Ok((_, _, _, ps, _)) = q.get(peer_e) else { return };
            if !ps.dead {
                probe.fail("the peer never went down (a save caught it?)");
                return;
            }
            let rolled = sphere::arc_dist(probe.down_dir, peer_dir, r);
            let dh = planet.surface(peer_dir) - probe.down_h;
            let want_meter = probe.secs_in_stage() / STATIC_METER_SECS;
            if (ps.static_meter - want_meter).abs() > 0.02 {
                let m = ps.static_meter;
                probe.fail(format!("Static Meter at {m:.3} after 2 s, want ~{want_meter:.3}"));
                return;
            }
            if probe.grade > 0.08 && (rolled < 0.3 || dh > -0.02) {
                let g = probe.grade;
                probe.fail(format!("the Beacon did not roll downhill on a {g:.2} grade (moved {rolled:.2} m, dh {dh:+.2} m)"));
                return;
            }
            let g = probe.grade;
            probe.notes.push(format!("rolled {rolled:.1} m downhill (grade {g:.2}, dh {dh:+.2} m), meter {:.3}", ps.static_meter));
            probe.next();
        }
        // the host stands in the ring: back up in REVIVE_SECS
        2 => {
            let beside = offset(peer_dir, 1.0, 1.5, r);
            if let Ok((_, _, mut p, _, _)) = q.get_mut(host_e) {
                pin(&mut p, beside);
            }
            if peer_dead {
                return;
            }
            let secs = probe.secs_in_stage();
            let Ok((_, _, _, ps, _)) = q.get(peer_e) else { return };
            let frac = ps.hp / ps.stats.max_hp;
            let Ok((_, _, _, host, _)) = q.get(host_e) else { return };
            let mut calm = host.clone();
            calm.adrenaline = 0.0;
            let boost = host.move_speed_mult() / calm.move_speed_mult();
            if !(REVIVE_SECS - 0.1..=REVIVE_SECS + 0.4).contains(&secs)
                || (frac - REVIVE_HP_FRAC).abs() > 0.02
                || host.adrenaline < ADRENALINE_SECS - 0.5
                || (boost - (1.0 + ADRENALINE_SPEED)).abs() > 0.01
                || host.rescues != 1
                || telemetry.revives != 1
            {
                probe.fail(format!(
                    "revive: {secs:.2}s, hp {frac:.2}, adrenaline {:.1}s x{boost:.2}, rescues {}, telemetry {}",
                    host.adrenaline, host.rescues, telemetry.revives
                ));
                return;
            }
            probe.notes.push(format!("revived in {secs:.2} s at {:.0}% HP, rescuer x{boost:.2} for {:.1} s", frac * 100.0, host.adrenaline));
            probe.next();
        }
        // down again, nobody coming: shots pass over it, a ring ignores it (L18, L19)
        3 => {
            let far = offset(peer_dir, 0.0, 60.0, r);
            if let Ok((_, _, mut p, _, _)) = q.get_mut(host_e) {
                pin(&mut p, far);
            }
            if !peer_dead {
                // down again, as soon as the revive's grace (iframes) runs out
                hits.write(crate::messages::PlayerHitMsg { victim: peer_e, amount: 1.0e6, from: Vec3::ZERO, attacker: None });
                probe.t0 = probe.ticks;
                return;
            }
            if probe.shrine.is_none() {
                // the nearest unfinished ring, under the Beacon, half charged
                let Some((se, _, _)) = shrines.iter().filter(|(_, s, _)| !s.done).min_by(|a, b| {
                    let da = a.2.translation.normalize_or_zero().dot(peer_dir);
                    let db = b.2.translation.normalize_or_zero().dot(peer_dir);
                    db.total_cmp(&da)
                }) else {
                    probe.fail("no charge shrine to test L19 on");
                    return;
                };
                probe.shrine = Some(se);
                if let Ok((_, mut s, _)) = shrines.get_mut(se) {
                    s.progress = 0.5;
                }
                // a shot fired straight through the Beacon
                let from = offset(peer_dir, 2.0, 3.0, r);
                let heading = (peer_dir - from * peer_dir.dot(from)).normalize_or_zero();
                probe.shot = Some(
                    commands
                        .spawn((
                            crate::enemies::EnemyProjectile { dir: from, heading, speed: 10.0, damage: 5.0, life: 2.0, hover: 1.0 },
                            Transform::from_translation(planet.surface_point(from) + from),
                        ))
                        .id(),
                );
            }
            // keep the Beacon inside the ring (it would roll out of it)
            if let (Some(se), Ok((_, _, mut p, _, _))) = (probe.shrine, q.get_mut(peer_e)) {
                if let Ok((_, _, stf)) = shrines.get(se) {
                    pin(&mut p, stf.translation.normalize_or_zero());
                }
            }
            if probe.secs_in_stage() < 1.0 {
                return;
            }
            let prog = probe.shrine.and_then(|se| shrines.get(se).ok()).map(|(_, s, _)| s.progress).unwrap_or(1.0);
            let shot_alive = probe.shot.is_some_and(|e| shots.get(e).is_ok());
            if prog > 0.5 || !shot_alive {
                probe.fail(format!("a Beacon still charged a ring (progress {prog:.3}) or ate a shot (shot alive: {shot_alive})"));
                return;
            }
            probe.notes.push(format!("a downed body left the ring draining ({prog:.2}) and let the shot through"));
            if let Ok((_, _, _, mut ps, _)) = q.get_mut(peer_e) {
                ps.static_meter = 0.97; // the fill rate was checked in stage 1
            }
            probe.next();
        }
        // claimed: hidden, out of the party, until the teleporter
        4 => {
            let far = offset(peer_dir, 0.0, 60.0, r);
            if let Ok((_, _, mut p, _, _)) = q.get_mut(host_e) {
                pin(&mut p, far);
            }
            if !peer_claimed {
                return;
            }
            if probe.secs_in_stage() < 0.5 {
                return;
            }
            let party = crate::run::scaling::living_party(q.iter().map(|(_, _, _, ps, _)| &*ps));
            let want_party = q.iter().count() - 1;
            if peer_vis != Visibility::Hidden || party != want_party || telemetry.claims != 1 {
                probe.fail(format!("claimed body visible={peer_vis:?}, living party {party} (want {want_party}), claims {}", telemetry.claims));
                return;
            }
            probe.notes.push(format!("claimed by The Static: hidden, party scale counts {party}"));
            pending.0 = Some(run.stage + 1);
            probe.next();
        }
        // through the teleporter: back at REJOIN_HP_FRAC
        5 => {
            if run.stage != 1 {
                return;
            }
            let Ok((_, _, _, ps, _)) = q.get(peer_e) else { return };
            let frac = ps.hp / ps.stats.max_hp;
            if ps.dead || ps.claimed || (frac - REJOIN_HP_FRAC).abs() > 0.02 || telemetry.rejoins != 1 {
                probe.fail(format!("rejoin: dead={} claimed={} hp {frac:.2} rejoins {}", ps.dead, ps.claimed, telemetry.rejoins));
                return;
            }
            probe.notes.push(format!("rejoined on {} at {:.0}% HP", run.planet().def().name, frac * 100.0));
            probe.done = true;
        }
        _ => {}
    }
}

/// `--cascade`: see `CoopProbe18`. Runs after the bot, before the physics.
#[allow(clippy::too_many_arguments)]
fn cascade_probe(
    mut probe: ResMut<CoopProbe18>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    telemetry: Res<crate::coop::CoopTelemetry>,
    belts: Query<(), With<crate::duos::CascadeBelt>>,
    mut q: Query<(&crate::player::PlayerId, &mut Player, &mut PlayerState)>,
    mut crowd: Query<(Entity, &mut Enemy, &mut Transform), (Without<crate::enemies::Boss>, Without<crate::interact::Pot>, Without<Player>)>,
) {
    use crate::content::weapons::WeaponKind;
    if probe.done {
        return;
    }
    probe.ticks += 1;
    if probe.ticks < 30 {
        return;
    }
    let r = planet.radius;
    let Some(host_dir) = q.iter().find(|(pid, ..)| pid.0 == 0).map(|(_, p, _)| p.dir) else { return };
    let apart = |deg: f32| offset(host_dir, 0.7, deg.to_radians() * r, r);
    if probe.stage == 0 {
        for (_, _, mut ps) in &mut q {
            ps.weapons.push(crate::run::WeaponInstance { kind: WeaponKind::StormCore, level: 1, cd: 0.0 });
        }
        probe.next();
        return;
    }
    let deg = if probe.stage == 1 { 60.0 } else { 150.0 };
    let peer_at = apart(deg);
    for (pid, mut p, mut ps) in &mut q {
        // both stand through the probe: a downed owner breaks the link
        ps.iframes = ps.iframes.max(0.5);
        let at = if pid.0 == 0 { host_dir } else { peer_at };
        pin(&mut p, at);
    }
    match probe.stage {
        // too close to link
        1 => {
            probe.idle_charge = probe.idle_charge.max(run.cascade_charge);
            if probe.secs_in_stage() < 3.0 * CASCADE_CHARGE_SECS / 2.0 {
                return;
            }
            if probe.idle_charge > 0.0 || telemetry.cascades > 0 {
                let c = probe.idle_charge;
                probe.fail(format!("storm-callers 60 degrees apart charged a link ({c:.2})"));
                return;
            }
            // five foes parked on the belt's line, halfway round from either owner (and one
            // well off it) — the cascade must reach them, and only them
            let axis = host_dir.cross(peer_at).normalize();
            let mid = (host_dir + peer_at).normalize_or_zero();
            let picked: Vec<Entity> = crowd.iter().filter(|(_, en, _)| en.speed > 0.0).map(|(e, ..)| e).take(6).collect();
            for (k, e) in picked.iter().enumerate() {
                let d = if k < 5 {
                    (Quat::from_axis_angle(axis, (k as f32 - 2.0) * 0.05) * mid).normalize()
                } else {
                    (mid + axis * 0.3).normalize()
                };
                if let Ok((_, mut en, mut tf)) = crowd.get_mut(*e) {
                    en.dir = d;
                    en.speed = 0.0;
                    en.hp = 1.0e5;
                    en.max_hp = 1.0e5;
                    tf.translation = planet.surface_point(d);
                }
            }
            probe.staged = picked;
            probe.next();
        }
        // opposite hemispheres: charge, fire
        2 => {
            if telemetry.cascades == 0 || probe.fx[2] == 0 {
                if probe.secs_in_stage() > CASCADE_CHARGE_SECS + 1.5 {
                    let why = format!("no STATIC CASCADE after {:.1} s linked (charge {:.2})", probe.secs_in_stage(), run.cascade_charge);
                    probe.fail(why);
                }
                return;
            }
            let secs = probe.secs_in_stage();
            if !(CASCADE_CHARGE_SECS - 0.1..=CASCADE_CHARGE_SECS + 0.4).contains(&secs) {
                probe.fail(format!("the link took {secs:.2} s to fire, want {CASCADE_CHARGE_SECS} s"));
                return;
            }
            probe.notes.push(format!("STATIC CASCADE after {secs:.2} s linked"));
            probe.next();
        }
        // the belt stands, the foes in it were hit, the pair is tallied — and the cooldown holds
        3 => {
            if probe.secs_in_stage() < 0.2 {
                return;
            }
            if probe.secs_in_stage() < 0.25 {
                let n_belt = belts.iter().count();
                let tallied = run.feats.iter().any(|f| f.feat == crate::content::duos::CoopFeat::StaticCascade && f.count == 1);
                let struck = |e: &Entity| crowd.get(*e).map(|(_, en, _)| en.hp < en.max_hp).unwrap_or(true);
                let on_line = probe.staged.iter().take(5).filter(|e| struck(e)).count();
                let off_line = probe.staged.get(5).is_some_and(struck);
                if on_line < 5 || off_line || n_belt == 0 || !tallied || probe.fx[2] != 1 {
                    let why = format!(
                        "cascade: {on_line}/5 foes on the line struck, off-line struck {off_line}, {} hits, {n_belt} belt segments, tallied {tallied}, fx {}",
                        telemetry.cascade_hits, probe.fx[2]
                    );
                    probe.fail(why);
                    return;
                }
                probe.notes.push(format!("{} foes in the belt (all 5 staged on its line, not the one beside it), {n_belt} belt segments", telemetry.cascade_hits));
            }
            if telemetry.cascades > 1 {
                probe.fail("a second cascade inside the cooldown");
                return;
            }
            if probe.secs_in_stage() > 4.0 {
                probe.done = true;
            }
        }
        _ => {}
    }
}

/// `--duos`: see `CoopProbe18`. Hits are written straight into the real `apply_hits`.
#[allow(clippy::too_many_arguments)]
fn duos_probe(
    mut probe: ResMut<CoopProbe18>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    telemetry: Res<crate::coop::CoopTelemetry>,
    q: Query<(Entity, &crate::player::PlayerId, &Player)>,
    mut crowd: Query<(Entity, &mut Enemy, &mut Transform), (Without<crate::enemies::Boss>, Without<crate::interact::Pot>, Without<Player>)>,
    mut hits: MessageWriter<crate::messages::HitMsg>,
) {
    use crate::content::duos::CoopFeat;
    use crate::content::weapons::WeaponKind as W;
    use crate::messages::{HitBy, HitMsg};
    if probe.done {
        return;
    }
    probe.ticks += 1;
    let (Some(host), Some(peer)) = (
        q.iter().find(|(_, id, _)| id.0 == 0).map(|(e, _, p)| (e, p.dir)),
        q.iter().find(|(_, id, _)| id.0 == 1).map(|(e, ..)| e),
    ) else {
        return;
    };
    let hit = |hits: &mut MessageWriter<HitMsg>, src: Entity, target: Entity, amount: f32, by: HitBy| {
        hits.write(HitMsg { source: Some(src), target, amount, crit: false, knock: Vec3::ZERO, by });
    };
    let t = probe.secs_in_stage();
    match probe.stage {
        // six foes, parked at the far side where no weapon reaches them
        0 if probe.ticks >= 60 => {
            let far = -host.1;
            let spots: Vec<Vec3> = (0..6).map(|i| offset(far, i as f32 * 1.0472, if i == 1 { 1.5 } else { 6.0 }, planet.radius)).collect();
            let picked: Vec<Entity> = crowd.iter().filter(|(_, en, _)| en.speed > 0.0).map(|(e, ..)| e).take(6).collect();
            if picked.len() < 6 {
                return; // wait for a horde
            }
            for (k, e) in picked.iter().enumerate() {
                if let Ok((_, mut en, mut tf)) = crowd.get_mut(*e) {
                    // E0 is Deep Freeze's victim; E1 stands beside it to be shattered
                    let d = if k == 1 { offset(spots[0], 0.0, 1.5, planet.radius) } else { spots[k] };
                    en.dir = d;
                    en.speed = 0.0;
                    en.hp = if k == 1 { 1000.0 } else { 100.0 };
                    en.max_hp = en.hp;
                    en.elite = false;
                    tf.translation = planet.surface_point(d);
                }
            }
            probe.staged = picked;
            probe.next();
        }
        // setups by the host
        1 => {
            let s = probe.staged.clone();
            hit(&mut hits, host.0, s[0], 1.0, HitBy::Weapon(W::CryoVent)); // Deep Freeze
            hit(&mut hits, host.0, s[2], 1.0, HitBy::Weapon(W::Boomerang)); // Magnet Circus
            hit(&mut hits, host.0, s[3], 1.0, HitBy::Weapon(W::RivetGun)); // Rivet & Rescue
            hit(&mut hits, host.0, s[4], 1.0, HitBy::Weapon(W::CryoVent)); // self-finish (no)
            hit(&mut hits, host.0, s[5], 1.0, HitBy::Weapon(W::CryoVent)); // too late (no)
            probe.next();
        }
        // the melt, measured
        2 if t >= 0.1 => {
            let s = probe.staged.clone();
            hit(&mut hits, peer, s[2], 10.0, HitBy::Weapon(W::LaserPistol));
            probe.next();
        }
        // the finishers
        3 if t >= 0.1 => {
            let s = probe.staged.clone();
            let melted = crowd.get(s[2]).map(|(_, en, _)| en.hp).unwrap_or(0.0);
            let want = 100.0 - 1.0 - 10.0 * DUO_MELT_MULT;
            if (melted - want).abs() > 0.05 {
                probe.fail(format!("Magnet Circus melt left {melted:.2} HP, want {want:.2}"));
                return;
            }
            hit(&mut hits, peer, s[0], 1.0e4, HitBy::Weapon(W::DeathRay));
            hit(&mut hits, peer, s[2], 1.0e4, HitBy::Weapon(W::GatlingLaser));
            hit(&mut hits, peer, s[3], 1.0e4, HitBy::Thorns);
            hit(&mut hits, host.0, s[4], 1.0e4, HitBy::Weapon(W::DeathRay));
            probe.next();
        }
        // past the window
        4 if t >= DUO_WINDOW + 0.4 => {
            let s = probe.staged.clone();
            hit(&mut hits, peer, s[5], 1.0e4, HitBy::Weapon(W::MiningLaser));
            probe.next();
        }
        5 if t >= 0.3 => {
            let shattered = crowd.get(probe.staged[1]).map(|(_, en, _)| 1000.0 - en.hp).unwrap_or(0.0);
            let got = |f: CoopFeat| telemetry.feats[f.code() as usize];
            let pair_ok = |f: CoopFeat| run.feats.iter().any(|x| x.feat == f && x.a.0 == 0 && x.b.0 == 1 && x.count == 1);
            let want_shatter = 100.0 * DUO_SHATTER_FRAC;
            if got(CoopFeat::DeepFreeze) != 1
                || got(CoopFeat::MagnetCircus) != 1
                || got(CoopFeat::RivetRescue) != 1
                || !CoopFeat::DUOS.iter().all(|f| pair_ok(*f))
                || (shattered - want_shatter).abs() > 0.5
                || probe.fx[3] != 3
            {
                let why = format!(
                    "duos: tally {:?}, squad feats {}, shatter took {shattered:.1} (want {want_shatter:.1}), fx {}",
                    telemetry.feats,
                    run.feats.len(),
                    probe.fx[3]
                );
                probe.fail(why);
                return;
            }
            probe.notes.push(format!(
                "Deep Freeze (shatter {shattered:.0}), Magnet Circus (melt x{DUO_MELT_MULT}), Rivet & Rescue (thorns) each counted once for P1+P2; a solo pair and a late finisher did not"
            ));
            probe.done = true;
        }
        _ => {}
    }
}

/// `--dropin`: see `CoopProbe18`. Seats the peer the way `net::seat_joining_players` does.
#[allow(clippy::too_many_arguments)]
fn dropin_probe(
    mut commands: Commands,
    mut probe: ResMut<CoopProbe18>,
    mut run: ResMut<RunState>,
    save: Res<MetaSave>,
    planet: Res<CurrentPlanet>,
    telemetry: Res<crate::coop::CoopTelemetry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&crate::player::PlayerId, &Player, &mut PlayerState, &crate::player::InputIntent)>,
) {
    if probe.done {
        return;
    }
    probe.ticks += 1;
    let tick = probe.ticks;
    // the host earns a squad level worth dropping in to
    if tick == 30 {
        for (pid, _, mut ps, _) in &mut q {
            if pid.0 == 0 {
                while ps.level < 10 {
                    let need = (ps.xp_needed - ps.xp).max(0.0);
                    ps.xp += need;
                    ps.gain_xp(0.0);
                }
            }
        }
        return;
    }
    if tick == 60 {
        // mid-run: the seat is a drop-in only past DROPIN_MIN_ELAPSED
        let at_start = crate::coop::drop_in_level(&run, &[10]);
        run.total_elapsed = run.total_elapsed.max(DROPIN_MIN_ELAPSED + 40.0);
        let levels: Vec<u32> = q.iter().map(|(_, _, ps, _)| ps.level).collect();
        let Some(level) = crate::coop::drop_in_level(&run, &levels) else {
            probe.fail("a seat 60 s into the run was not a drop-in");
            return;
        };
        if at_start.is_some() && tick as f32 * 0.033 < DROPIN_MIN_ELAPSED {
            probe.fail("a seat at the start of the run counted as a drop-in");
            return;
        }
        probe.expect_level = level;
        let mut ps = PlayerState::new(run.character, &save);
        crate::coop::drop_in_sheet(&mut ps, level, &save);
        let body = crate::player::spawn_player(&mut commands, &mut meshes, &mut materials, &planet, &run, &save, 1, run.character, false, Some(ps));
        commands.entity(body).insert(crate::coop::OrbitalDrop::default());
        probe.next();
        return;
    }
    if probe.stage == 0 {
        return;
    }
    let Some((_, p, ps, intent)) = q.iter().find(|(pid, ..)| pid.0 == 1) else { return };
    probe.peer_peak = probe.peer_peak.max(p.height);
    let t = probe.secs_in_stage();
    match probe.stage {
        // falling from orbit, then the landing
        1 => {
            if telemetry.dropin_landings == 0 || probe.fx[4] == 0 {
                if t > 6.0 {
                    let why = format!("the drop-in never landed (peak {:.1} m)", probe.peer_peak);
                    probe.fail(why);
                }
                return;
            }
            if probe.peer_peak < DROPIN_HEIGHT * 0.9 || ps.level != probe.expect_level || probe.expect_level < 5 || probe.fx[4] != 1 {
                let why = format!(
                    "drop-in: peak {:.1} m, level {} (want {} = half the squad's), fx {}",
                    probe.peer_peak, ps.level, probe.expect_level, probe.fx[4]
                );
                probe.fail(why);
                return;
            }
            probe.peer_last = p.dir;
            probe.next();
        }
        // the grace: idle, the autopilot fights for it
        2 => {
            if !ps.dead {
                probe.peer_path += sphere::arc_dist(probe.peer_last, p.dir, planet.radius);
            }
            probe.peer_last = p.dir;
            if ps.grace > 0.0 {
                return;
            }
            if probe.peer_path < 25.0 || telemetry.autopilot_secs < DROPIN_GRACE_SECS * 0.5 {
                let why = format!(
                    "autopilot moved the drop-in {:.1} m over {:.1} s of grace",
                    probe.peer_path, telemetry.autopilot_secs
                );
                probe.fail(why);
                return;
            }
            probe.next();
        }
        // grace over: an idle player's astronaut stands still again
        3 => {
            if t < 1.5 {
                return;
            }
            if intent.wish != Vec3::ZERO || (!ps.dead && p.vel_t.length() > 1.0) {
                probe.fail(format!("the autopilot kept driving after the grace (wish {:?}, speed {:.1})", intent.wish, p.vel_t.length()));
                return;
            }
            let (lvl, path, secs, peak) = (ps.level, probe.peer_path, telemetry.autopilot_secs, probe.peer_peak);
            probe.notes.push(format!("dropped from {peak:.0} m at LV {lvl} (half the squad's); autopilot ran {path:.0} m over {secs:.1} s, then let go"));
            probe.done = true;
        }
        _ => {}
    }
}

/// `--coop4`: every crowd spawn and the boss at the §11 party HP scale of the squad standing.
#[allow(clippy::type_complexity)]
fn party_probe(
    mut probe: ResMut<CoopProbe18>,
    run: Res<RunState>,
    squad: Query<&PlayerState>,
    fresh: Query<(&Enemy, Option<&crate::enemies::Boss>), (Added<Enemy>, Without<crate::interact::Pot>)>,
) {
    let living = crate::run::scaling::living_party(squad.iter());
    let sc = crate::run::scaling::Scaling::for_run(&run, living);
    for (en, boss) in &fresh {
        if let Some(b) = boss {
            if b.kind.def().is_stage_boss && probe.boss_ratio.is_none() {
                probe.boss_ratio = Some((en.max_hp / b.kind.def().hp, sc.boss_hp));
            }
            continue;
        }
        if en.elite || en.kind == crate::content::enemies::EnemyKind::Ghost || run.static_active {
            continue;
        }
        probe.crowd_checked += 1;
        let ratio = en.max_hp / en.kind.def().hp;
        if (ratio - sc.hp).abs() > sc.hp * 0.01 {
            probe.crowd_bad += 1;
        }
    }
}

/// The summary lines and verdict of the §11 probes. False on any failure.
fn coop_probe_report(world: &mut World) -> bool {
    let tel = world.resource::<crate::coop::CoopTelemetry>();
    println!(
        "COOP downs={} revives={} claims={} rejoins={} roll={:.1}m shoves={} chills={} jolts={} dropins={} landed={} autopilot={:.1}s cascades={} cascade_hits={} feats={:?} shatter_hits={}",
        tel.downs, tel.revives, tel.claims, tel.rejoins, tel.beacon_roll_m, tel.shoves, tel.chills, tel.jolts,
        tel.dropins, tel.dropin_landings, tel.autopilot_secs, tel.cascades, tel.cascade_hits, tel.feats, tel.shatter_hits
    );
    let probe = world.resource::<CoopProbe18>();
    if !probe.any() {
        return true;
    }
    let mut ok = probe.fails.is_empty();
    for n in &probe.notes {
        println!("  {n}");
    }
    let staged = [
        (probe.revive, "REVIVE"),
        (probe.cascade, "CASCADE"),
        (probe.duos, "DUOS"),
        (probe.dropin, "DROPIN"),
    ];
    for (on, name) in staged {
        if !on {
            continue;
        }
        if !probe.done && probe.fails.is_empty() {
            println!("FAIL: {name} probe never finished (stage {})", probe.stage);
            ok = false;
        } else if probe.fails.is_empty() {
            println!("{name} OK");
        } else {
            println!("FAIL: {name}: {}", probe.fails.join("; "));
        }
    }
    if probe.coop4 {
        let run = world.resource::<RunState>();
        let four = crate::run::scaling::Scaling::for_run(run, 4);
        println!(
            "PARTY of 4: crowd HP x{:.2} (checked {} spawns, {} off), boss {:?}, live cap {}",
            crate::config::PARTY_HP_SCALE[3],
            probe.crowd_checked,
            probe.crowd_bad,
            probe.boss_ratio,
            four.live_cap
        );
        let boss_ok = probe.boss_ratio.is_none_or(|(got, want)| (got - want).abs() <= want * 0.01);
        if probe.crowd_checked == 0 || probe.crowd_bad > 0 || !boss_ok || four.live_cap != (ENEMY_CAP as f32 * PARTY_SPAWN_SCALE[3]) as usize {
            println!("FAIL: the party of four is not scaled per §11");
            ok = false;
        } else {
            println!("PARTY4 OK");
        }
    }
    ok
}
