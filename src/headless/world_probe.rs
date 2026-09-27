//! `--daynight` and `--hazards`: the P07 probes. Headless IS the host, so the sun, the
//! night rules and the world gimmicks are staged on the real astronauts and ASSERTED:
//!
//! * `--daynight` — the sun turns at its rate; the crash site opens in daylight; after the
//!   squad is moved into the night, the horde there runs NIGHT_ENEMY_SPEED faster than by
//!   day and its waves land inside the night band; staged elite kills pay ~+25% Gold per
//!   coin at night and not by day; the sun is eaten iff something ate it for SUN_EAT_SECS;
//!   on the Moon an XP gem on Farside wears the brighter material and one on Earthside not.
//! * `--hazards` — Mars: every astronaut pinned in a thorn bush is snagged (speed ×
//!   THORN_SLOW) and freed once out of it. Dark Moon: a cap next to the local astronaut is
//!   primed once, bursts on the horde and the astronaut, and its cloud bites; then the clock
//!   is wound to The Crawl: sites mass before The Static, are erupting as it rises, and every
//!   ghost after that erupts from one.

use crate::config::*;
use crate::content::enemies::EnemyKind;
use crate::content::planets::{FloraStyle, PlanetKind};
use crate::daynight::Sun;
use crate::enemies::Enemy;
use crate::gimmicks::{Crawl, GimmickTelemetry, WorldFlora};
use crate::planet::CurrentPlanet;
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use std::collections::HashMap;

/// `--daynight`'s script: the crash site's morning (a day horde and its waves) until
/// NIGHT_AT, then the squad in the night; staged kills at night, then by day; the Farside
/// gems at GEMS_AT. Fits a 1200-tick run.
const NIGHT_AT: u64 = 380;
const GEMS_AT: u64 = 1000;

#[derive(Resource, Default)]
pub struct WorldProbe {
    pub daynight: bool,
    pub hazards: bool,
    ticks: u64,
    /// (phase, stage clock) when the probe first looked.
    start: Option<(f32, f32)>,
    crash_site_lit: Option<bool>,
    night_secs: f32,
    /// Per-tick moves of walking horde enemies over their own speed, by day / by night.
    day_ratio: Vec<f32>,
    night_ratio: Vec<f32>,
    /// Arc from a fresh spawn to the nearest astronaut, with the whole squad by day / night.
    day_spawns: Vec<f32>,
    night_spawns: Vec<f32>,
    /// Where staged elite kills fell (spot, at night?), and the coins seen landing there.
    kill_spots: Vec<(Vec3, bool)>,
    night_coins: Vec<u64>,
    day_coins: Vec<u64>,
    /// Seconds something (a Sun Shard or Difficulty) was eating the sun.
    eat_secs: f32,
    /// Farside / Earthside gems placed by the probe, and whether each wore the right glow.
    gems: Option<(Entity, Entity)>,
    gem_glow: Option<(bool, bool)>,
    // --hazards
    thorn_pinned: HashMap<u8, (u32, u32)>,
    thorn_slow_ok: bool,
    thorn_freed: Option<bool>,
    spore_plant: Option<usize>,
    spore_walkers: bool,
    crawl_wound: bool,
    crawl_massing_seen: bool,
    crawl_erupting_at_static: Option<usize>,
    static_ticks: u32,
    /// Every place a Crawl site has been seen erupting (a ghost spawned in a site's last
    /// frame is seen after the site has closed).
    erupted: Vec<Vec3>,
    crawl_near: u32,
    crawl_far: u32,
}

impl WorldProbe {
    pub fn from_args(args: &[String]) -> Self {
        Self { daynight: args.iter().any(|a| a == "--daynight"), hazards: args.iter().any(|a| a == "--hazards"), ..default() }
    }
    pub fn on(p: Res<WorldProbe>) -> bool {
        p.daynight || p.hazards
    }
}

/// A night point well away from every astronaut (and a day one), for the staged kills.
fn lonely_spot(sun: &Sun, night: bool, astronauts: &[Vec3], radius: f32) -> Vec3 {
    let pole = if night { -sun.toward } else { sun.toward };
    let (t, b) = sphere::tangent_frame(pole);
    (0..8)
        .map(|i| {
            let a = i as f32 / 8.0 * std::f32::consts::TAU;
            sphere::offset_dir(pole, t * a.cos() + b * a.sin(), radius * 0.5, radius)
        })
        .filter(|d| sun.is_night(*d) == night)
        .max_by(|a, b| {
            let far = |d: &Vec3| astronauts.iter().map(|x| sphere::arc_dist(*d, *x, radius)).fold(f32::MAX, f32::min);
            far(a).total_cmp(&far(b))
        })
        .unwrap_or(pole)
}

/// Stage the scenes: move the squad into the night, drop the elite kills and the gems; pin
/// astronauts in thorns / beside a cap; wind the Dark Moon's clock to The Crawl.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn world_probe_stage(
    mut commands: Commands,
    mut probe: ResMut<WorldProbe>,
    mut run: ResMut<RunState>,
    planet: Res<CurrentPlanet>,
    flora: Res<WorldFlora>,
    (pickup_assets, enemy_assets): (Res<crate::pickups::PickupAssets>, Res<crate::enemies::EnemyAssets>),
    mut kills: MessageWriter<crate::messages::KillMsg>,
    mut q: Query<(&PlayerId, &mut Player, &mut crate::items::ItemProcs, &PlayerState, Has<LocalPlayer>)>,
) {
    probe.ticks += 1;
    let t = probe.ticks;
    let sun = Sun::of(&run);
    let r = planet.radius;
    let dirs: Vec<Vec3> = q.iter().map(|(_, p, _, _, _)| p.dir).collect();

    if probe.daynight {
        if t == NIGHT_AT {
            // just past deep night's centre, fanned out per player
            let night = -sun.toward;
            let (a, _) = sphere::tangent_frame(night);
            for (pid, mut p, mut procs, _, _) in &mut q {
                p.dir = sphere::offset_dir(night, a, 4.0 * pid.0 as f32, r);
                p.vel_t = Vec3::ZERO;
                p.height = 0.0;
                procs.forget_altitude();
            }
        }
        // staged elite kills: night spots, then day spots, far from the fight
        if (NIGHT_AT + 40..=NIGHT_AT + 580).contains(&t) && t % 20 == 0 {
            let night = t <= NIGHT_AT + 300;
            let dir = lonely_spot(&sun, night, &dirs, r);
            probe.kill_spots.push((dir, night));
            kills.write(crate::messages::KillMsg {
                pos: planet.surface_point(dir),
                dir,
                kind: Some(EnemyKind::Shambler),
                elite: true,
                xp: 0.0,
                is_boss: false,
                is_miniboss: false,
                is_pot: false,
            });
            // a staged drop, not a kill: `kill_drops` (ordered after this) counts it back in
            run.kills = run.kills.saturating_sub(1);
        }
        if t == GEMS_AT && crate::daynight::has_farside(planet.kind) {
            let earth = crate::daynight::earth_dir();
            let far = crate::pickups::spawn_pickup(&mut commands, &pickup_assets, &planet, -earth, crate::pickups::PickupKind::Xp(1.0));
            let near = crate::pickups::spawn_pickup(&mut commands, &pickup_assets, &planet, earth, crate::pickups::PickupKind::Xp(1.0));
            probe.gems = Some((far, near));
        }
    }

    if probe.hazards {
        match flora.style {
            Some(FloraStyle::Thorns) => {
                // every astronaut into its own bush, then out beside it
                for (pid, mut p, mut procs, _, _) in &mut q {
                    let Some(bush) = flora.plants.get(pid.0 as usize * 7) else { continue };
                    if (60..=100).contains(&t) {
                        p.dir = bush.dir;
                        p.height = 0.0;
                        procs.forget_altitude();
                    } else if (101..=125).contains(&t) {
                        let (a, _) = sphere::tangent_frame(bush.dir);
                        let clear = (1..12)
                            .map(|k| sphere::offset_dir(bush.dir, a, 2.5 * k as f32, r))
                            .find(|d| !flora.thorn_contact(*d, 0.0, PLAYER_RADIUS, r))
                            .unwrap_or(bush.dir);
                        p.dir = clear;
                        p.vel_t = Vec3::ZERO;
                        p.height = 0.0;
                        procs.forget_altitude();
                    }
                }
            }
            Some(FloraStyle::GlowShrooms) => {
                // the local astronaut beside a cap until its cloud has bitten
                if (60..=130).contains(&t) {
                    let plant = *probe.spore_plant.get_or_insert(0);
                    if let Some(cap) = flora.plants.get(plant) {
                        let (a, _) = sphere::tangent_frame(cap.dir);
                        let beside = sphere::offset_dir(cap.dir, a, 1.5, r);
                        for (_, mut p, mut procs, _, local) in &mut q {
                            if local {
                                p.dir = beside;
                                p.vel_t = Vec3::ZERO;
                                p.height = 0.0;
                                procs.forget_altitude();
                            }
                        }
                        // a few walkers on the cap just before it goes: the burst is theirs too
                        if t == 90 && !probe.spore_walkers {
                            probe.spore_walkers = true;
                            let mut rng = rand::thread_rng();
                            for k in 0..3 {
                                let d = sphere::offset_dir(cap.dir, a, -1.0 - k as f32, r);
                                crate::enemies::spawn_enemy(&mut commands, &enemy_assets, &planet, EnemyKind::Shambler, d, false, 1.0, 1.0, &mut rng);
                            }
                        }
                    }
                }
                // then wind the clock to The Crawl (bosses marked done: the probe is the Crawl)
                if t == 150 && !run.static_active {
                    run.timer = run.timer.min(CRAWL_MASS_SECS + 3.0);
                    run.boss_spawned = true;
                    run.minibosses_spawned = [true; 2];
                    probe.crawl_wound = true;
                }
            }
            _ => {}
        }
    }
}

/// Read what the sun and the gimmicks did, every tick.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn world_probe_watch(
    time: Res<Time>,
    mut probe: ResMut<WorldProbe>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    crawl: Res<Crawl>,
    (assets, hash): (Res<crate::pickups::PickupAssets>, Res<crate::enemies::SpatialHash>),
    sheets: Query<(&PlayerId, &Player, &PlayerState, Has<LocalPlayer>)>,
    horde: Query<(Entity, &Enemy, &Transform), Without<crate::enemies::Boss>>,
    fresh: Query<&Enemy, (Added<Enemy>, Without<crate::enemies::Boss>, Without<crate::interact::Pot>)>,
    coins: Query<&crate::pickups::Pickup, Added<crate::pickups::Pickup>>,
    gems: Query<&MeshMaterial3d<StandardMaterial>>,
    mut last: Local<HashMap<Entity, Vec3>>,
) {
    let dt = time.delta_secs();
    let sun = Sun::of(&run);
    let r = planet.radius;
    if probe.start.is_none() {
        probe.start = Some((run.sun_phase, run.elapsed));
        probe.crash_site_lit = Some(!sun.is_night(Vec3::Y));
    }
    let astronauts: Vec<Vec3> = sheets.iter().filter(|(_, _, ps, _)| !ps.dead).map(|(_, p, _, _)| p.dir).collect();
    if let Some((_, p, _, _)) = sheets.iter().find(|s| s.3) {
        if sun.is_night(p.dir) {
            probe.night_secs += dt;
        }
    }
    if sheets.iter().any(|(_, _, ps, _)| ps.has_item(crate::content::items::ItemKind::DevouredSunShard)) || run.difficulty > 0.0 {
        probe.eat_secs += dt;
    }

    if probe.daynight && dt > 0.0 {
        // how far each walker moved this tick over how far its own speed takes it — alone, so
        // the crowd's separation shoves are not in the number: that is its pace
        let mut seen = HashMap::with_capacity(last.len());
        for (e, en, tf) in &horde {
            seen.insert(e, en.dir);
            let Some(prev) = last.get(&e) else { continue };
            let walker = en.kind.def().standoff <= 0.0
                && en.kind != EnemyKind::Burrower
                && en.speed > 0.0
                && en.slow <= 0.0
                && en.knock.length() < 0.05;
            let alone = hash.near(tf.translation, 3.0).all(|(o, p)| o == e || p.distance(tf.translation) > 2.5);
            let clear = astronauts.iter().all(|a| sphere::arc_dist(en.dir, *a, r) > 4.0);
            if !walker || !alone || !clear {
                continue;
            }
            let moved = prev.angle_between(en.dir) * planet.surface(en.dir);
            let ratio = moved / (en.speed * dt);
            let day = sun.daylight(en.dir);
            if day > 0.95 {
                probe.day_ratio.push(ratio);
            } else if day < 0.05 {
                probe.night_ratio.push(ratio);
            }
        }
        *last = seen;
        // fresh waves: how close to the squad they landed
        let all_night = !astronauts.is_empty() && astronauts.iter().all(|a| sun.is_night(*a));
        let all_day = !astronauts.is_empty() && astronauts.iter().all(|a| !sun.is_night(*a));
        for en in &fresh {
            if en.speed <= 0.0 || matches!(en.kind, EnemyKind::Burrower | EnemyKind::Ghost) {
                continue;
            }
            let arc = astronauts.iter().map(|a| sphere::arc_dist(en.dir, *a, r)).fold(f32::MAX, f32::min);
            if all_night {
                probe.night_spawns.push(arc);
            } else if all_day {
                probe.day_spawns.push(arc);
            }
        }
        // the staged kills' coins
        for c in &coins {
            let crate::pickups::PickupKind::Gold(n) = c.kind else { continue };
            if let Some((_, night)) = probe.kill_spots.iter().find(|(d, _)| sphere::arc_dist(*d, c.dir, r) < 2.5).copied() {
                if night {
                    probe.night_coins.push(n);
                } else {
                    probe.day_coins.push(n);
                }
            }
        }
        if probe.gem_glow.is_none() && probe.ticks >= GEMS_AT + 6 {
            if let Some((far, near)) = probe.gems {
                if let (Ok(f), Ok(n)) = (gems.get(far), gems.get(near)) {
                    probe.gem_glow = Some((f.0 == assets.gem_far_mat, n.0 == assets.gem_mat));
                }
            }
        }
    }

    if probe.hazards {
        // thorns: pinned in a bush = snagged and slowed; set clear of it = freed
        let t = probe.ticks;
        if (62..=100).contains(&t) {
            for (pid, _, ps, _) in &sheets {
                let e = probe.thorn_pinned.entry(pid.0).or_default();
                e.0 += 1;
                if ps.thorned > 0.0 {
                    e.1 += 1;
                    let mut free = ps.clone();
                    free.thorned = 0.0;
                    let want = free.move_speed_mult() * THORN_SLOW;
                    probe.thorn_slow_ok = (ps.move_speed_mult() - want).abs() < 1e-4;
                }
            }
        }
        if t == 125 && planet.kind == PlanetKind::Mars {
            probe.thorn_freed = Some(sheets.iter().all(|(_, _, ps, _)| ps.thorned <= 0.0));
        }
        // The Crawl
        if probe.crawl_wound {
            if !run.static_active && crawl.sites.len() >= CRAWL_SITES && crawl.sites.iter().all(|s| !s.erupting()) {
                probe.crawl_massing_seen = true;
            }
            // the first ghosts come the frame after the clock runs out: read the sites then
            if run.static_active {
                probe.static_ticks += 1;
                if probe.static_ticks <= 3 {
                    let n = crawl.sites.iter().filter(|s| s.erupting()).count();
                    probe.crawl_erupting_at_static = Some(probe.crawl_erupting_at_static.unwrap_or(0).max(n));
                }
            }
            for s in crawl.sites.iter().filter(|s| s.erupting()) {
                if !probe.erupted.iter().any(|d| d.dot(s.dir) > 0.99999) {
                    probe.erupted.push(s.dir);
                }
            }
            for en in &fresh {
                if en.kind != EnemyKind::Ghost {
                    continue;
                }
                let near = probe.erupted.iter().any(|d| sphere::arc_dist(*d, en.dir, r) <= CRAWL_SPREAD + 0.5);
                if near {
                    probe.crawl_near += 1;
                } else {
                    probe.crawl_far += 1;
                }
            }
        }
    }
}

fn median(v: &[f32]) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[s.len() / 2]
}

fn mean(v: &[u64]) -> f32 {
    v.iter().sum::<u64>() as f32 / v.len().max(1) as f32
}

/// The verdict, after the run: prints a WORLD line and a FAIL line per broken rule.
pub fn report(world: &mut World) -> bool {
    let probe = world.resource::<WorldProbe>();
    if !probe.daynight && !probe.hazards {
        return true;
    }
    let run = world.resource::<RunState>().clone();
    let tel = format!("{:?}", world.resource::<GimmickTelemetry>());
    let g = world.resource::<GimmickTelemetry>();
    let planet = world.resource::<CurrentPlanet>().kind;
    let mut ok = true;
    let mut need = |cond: bool, what: String| {
        if !cond {
            println!("FAIL: {what}");
            ok = false;
        }
    };
    if probe.daynight {
        let (phase0, clock0) = probe.start.unwrap_or((0.0, 0.0));
        let want = crate::daynight::sun_rate(run.stage) * (run.elapsed - clock0);
        let turned = (run.sun_phase - phase0).rem_euclid(std::f32::consts::TAU);
        let (dr, nr) = (median(&probe.day_ratio), median(&probe.night_ratio));
        let (dmax, nmax) = (
            probe.day_spawns.iter().copied().fold(0.0f32, f32::max),
            probe.night_spawns.iter().copied().fold(0.0f32, f32::max),
        );
        let (nc, dc) = (mean(&probe.night_coins), mean(&probe.day_coins));
        println!(
            "  WORLD sun turned={turned:.3} (want {want:.3}) crash_lit={:?} night={:.1}s speed[day x{dr:.3} ({}), night x{nr:.3} ({})] spawns[day max {dmax:.1} ({}), night max {nmax:.1} ({})] coins[night {nc:.2} ({}), day {dc:.2} ({})] eaten={:.2} after {:.0}s gems={:?}",
            probe.crash_site_lit,
            probe.night_secs,
            probe.day_ratio.len(),
            probe.night_ratio.len(),
            probe.day_spawns.len(),
            probe.night_spawns.len(),
            probe.night_coins.len(),
            probe.day_coins.len(),
            run.sun_shrink,
            probe.eat_secs,
            probe.gem_glow,
        );
        need((turned - want).abs() < 0.02, format!("the sun turned {turned:.3} rad, not {want:.3}"));
        need(probe.crash_site_lit == Some(true), "the stage did not open in daylight at the crash site".into());
        need(probe.night_secs > 5.0, "the squad never stood in the night".into());
        need(probe.day_ratio.len() > 200 && (0.97..1.03).contains(&dr), format!("the horde by day moves x{dr:.3} its speed"));
        need(
            probe.night_ratio.len() > 200 && (nr / dr - (1.0 + NIGHT_ENEMY_SPEED)).abs() < 0.03,
            format!("the horde at night moves x{:.3} its daytime pace, not x{:.2}", nr / dr, 1.0 + NIGHT_ENEMY_SPEED),
        );
        let night_band = SPAWN_ARC_MAX * NIGHT_SPAWN_ARC_MULT + 1.5;
        need(probe.night_spawns.len() > 20 && nmax <= night_band, format!("night waves landed out to {nmax:.1} m (band ends {night_band:.1})"));
        need(probe.day_spawns.len() >= 8 && dmax > night_band, format!("day waves never used the full band (max {dmax:.1} m)"));
        // elite coins are 4..9 Gold (mean 6.5): +25% at night is ~8.1
        need(probe.night_coins.len() >= 50 && nc > 7.3, format!("night kills paid {nc:.2} Gold a coin (+25% of 6.5 is 8.1)"));
        need(probe.day_coins.len() >= 50 && dc < 7.3, format!("day kills paid {dc:.2} Gold a coin (want 6.5)"));
        let ate = probe.eat_secs >= SUN_EAT_SECS + 0.5;
        need(ate == (run.sun_shrink > 0.0) || (probe.eat_secs - SUN_EAT_SECS).abs() < 0.5, format!("the sun is {:.2} eaten after {:.0}s of eating", run.sun_shrink, probe.eat_secs));
        if crate::daynight::has_farside(planet) {
            need(probe.gem_glow == Some((true, true)), format!("Farside gems do not burn brighter (far, near) = {:?}", probe.gem_glow));
        }
    }
    if probe.hazards {
        println!(
            "  WORLD hazards thorns={:?} slow_ok={} freed={:?} spore_plant={:?} crawl[massing={} erupting_at_static={:?} near={} far={}] {tel}",
            probe.thorn_pinned, probe.thorn_slow_ok, probe.thorn_freed, probe.spore_plant, probe.crawl_massing_seen, probe.crawl_erupting_at_static, probe.crawl_near, probe.crawl_far
        );
        match planet {
            PlanetKind::Mars => {
                for (id, (pinned, snagged)) in &probe.thorn_pinned {
                    need(*pinned > 0 && *snagged + 2 >= *pinned, format!("player {id} in a thorn bush was snagged {snagged}/{pinned} ticks"));
                }
                need(!probe.thorn_pinned.is_empty() && probe.thorn_slow_ok, "a snag does not slow to THORN_SLOW".into());
                need(probe.thorn_freed == Some(true), "an astronaut stayed snagged out of the thorns".into());
                need(g.thorn_snags as usize >= probe.thorn_pinned.len(), "thorn snags were not counted".into());
            }
            PlanetKind::DarkMoon => {
                need(g.spore_primes >= 1 && g.spore_pops >= 1, "no spore cap primed and burst beside the astronaut".into());
                need(g.spore_enemy_hits >= 1, "a spore burst never hit the horde".into());
                need(g.spore_player_hits >= 1, "a spore cloud never bit the astronaut in it".into());
                need(probe.crawl_massing_seen, "The Crawl never showed massing before The Static".into());
                need(probe.crawl_erupting_at_static.unwrap_or(0) >= CRAWL_SITES, format!("The Crawl was not erupting as The Static rose ({:?})", probe.crawl_erupting_at_static));
                need(probe.crawl_near > 10 && probe.crawl_far == 0, format!("ghosts did not all erupt from The Crawl ({} near, {} elsewhere)", probe.crawl_near, probe.crawl_far));
            }
            PlanetKind::Moon => {}
        }
    }
    ok
}
