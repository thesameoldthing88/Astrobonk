//! §4 day/night: the planet turns, and the terminator sweeps the surface over the stage.
//! "Night is a biome, not a filter" — the horde runs faster there and crests the horizon
//! closer, and every kill in it pays more Gold.
//!
//! ONE sun. [`Sun::of`] turns the run's `sun_phase` into the sunward direction and carries
//! how much of the day side has been eaten (`sun_shrink`, §3 diegetic difficulty: the
//! Devoured Sun Shard and the party's Difficulty shrink the day every 60 s toward
//! night-lock). Every gameplay question about night — Tome of Nightfall, the horde's night
//! speed (`enemies::enemy_move`; any custom mover must multiply `Sun::enemy_speed` in too),
//! closer night spawns (`enemies::director_spawn`), night Gold (`pickups::kill_drops`) —
//! and the lighting itself (`apply_sky`) ask this one sun.
//!
//! CO-OP: the host turns the sun and eats it (`advance_sun`); `sun_phase` and `sun_shrink`
//! ride `RunSnapMsg`, and a client dead-reckons the phase between snapshots (`drift_sun`),
//! so both machines light the same sky and agree on where night is. The lighting is each
//! machine's own presentation from those two numbers and its own astronaut.
//!
//! The Moon's EARTHSIDE/FARSIDE (§8) is a second, FIXED divide — the hemisphere facing the
//! Earth — and lives here too, next to the light it changes: Earthside gets the Earth's
//! cyan fill, Farside loses it (ambient drops), its gems glow brighter and its elite roll is
//! likelier (`farside_elite_mult`).

use crate::config::*;
use crate::content::items::ItemKind;
use crate::content::planets::PlanetKind;
use crate::messages::BannerMsg;
use crate::pickups::{Pickup, PickupAssets, PickupKind};
use crate::planet::CurrentPlanet;
use crate::player::{LocalPlayer, Player};
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;

/// The sun at one moment of the run.
#[derive(Clone, Copy, Debug)]
pub struct Sun {
    /// Unit direction from the planet's core toward the sun: the day side faces it.
    pub toward: Vec3,
    /// How much of the day side is eaten, 0..1 (§3): the terminator sits where
    /// `dot(dir, toward) == shrink`, so at 0 it is the great circle and at 1 it is gone.
    pub shrink: f32,
}

impl Sun {
    pub fn at(kind: PlanetKind, phase: f32, shrink: f32) -> Self {
        Self { toward: sunward(kind, phase), shrink: shrink.clamp(0.0, 1.0) }
    }

    /// The run's sun right now — the same on the host and (to within a snapshot's
    /// dead-reckoning) on every joiner.
    pub fn of(run: &RunState) -> Self {
        Self::at(run.planet(), run.sun_phase, run.sun_shrink)
    }

    /// Night-lock: the sun is gone and the whole world is the night side.
    pub fn locked(&self) -> bool {
        self.shrink >= 1.0
    }

    /// Is the surface point `dir` on the night side? THE test.
    pub fn is_night(&self, dir: Vec3) -> bool {
        self.locked() || dir.dot(self.toward) < self.shrink
    }

    /// How much day `dir` stands in, 0 (night) ..1 (day), ramped across the twilight band
    /// either side of the terminator — for things that should ease, not flip.
    pub fn daylight(&self, dir: Vec3) -> f32 {
        if self.locked() {
            return 0.0;
        }
        smoothstep(self.shrink - TWILIGHT_BAND, self.shrink + TWILIGHT_BAND, dir.dot(self.toward))
    }

    /// §4 night risk: the horde's move-speed multiplier where it stands.
    pub fn enemy_speed(&self, dir: Vec3) -> f32 {
        1.0 + NIGHT_ENEMY_SPEED * (1.0 - self.daylight(dir))
    }

    /// §4 night risk: how far out a wave aimed at an astronaut at `anchor` lands, as a share
    /// of the day's spawn band.
    pub fn spawn_arc(&self, anchor: Vec3) -> f32 {
        if self.is_night(anchor) {
            NIGHT_SPAWN_ARC_MULT
        } else {
            1.0
        }
    }

    /// §4 night reward: a kill's `amount` of Gold where it fell. The +25% is paid in whole
    /// coins, rounded up with the leftover as the odds, so even a 1-Gold drop pays it on
    /// average.
    pub fn kill_gold(&self, dir: Vec3, amount: u64, rng: &mut impl Rng) -> u64 {
        if !self.is_night(dir) {
            return amount;
        }
        night_gold(amount, rng.gen_range(0.0..1.0))
    }
}

/// `amount` × NIGHT_GOLD_MULT in whole coins: the fraction becomes one more coin when the
/// roll `u` (0..1) falls under it. Split out so the self-check can pin the expectation.
pub fn night_gold(amount: u64, u: f32) -> u64 {
    let v = amount as f32 * NIGHT_GOLD_MULT;
    let whole = v.floor();
    whole as u64 + u64::from(u < v - whole)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Unit direction from the core toward the sun at `phase`. The spin axis lies in the crash
/// site's horizon (the crash site is +Y), so the sun passes over it; at phase 0 it stands
/// SUN_START_BEFORE_NOON short of noon there — every stage opens in the morning.
pub fn sunward(kind: PlanetKind, phase: f32) -> Vec3 {
    let light = kind.def().light;
    let axis = Quat::from_rotation_y(light.axis_yaw) * Vec3::new(1.0, 0.0, 0.35).normalize();
    let noon = Vec3::Y * light.declination.cos() + axis * light.declination.sin();
    (Quat::from_axis_angle(axis, phase - SUN_START_BEFORE_NOON) * noon).normalize()
}

/// Radians the sun turns per second on stage `stage` (one full turn per stage clock).
pub fn sun_rate(stage: usize) -> f32 {
    std::f32::consts::TAU * SUN_TURNS_PER_STAGE / STAGE_SECONDS[stage.min(STAGE_SECONDS.len() - 1)]
}

// ---------------------------------------------------------------------------------------
// Earthside / Farside (the Moon)

/// Where the Earth hangs over the Moon, fixed (a soft "north", §8).
pub fn earth_dir() -> Vec3 {
    Vec3::new(0.5, 0.62, 0.35).normalize()
}

/// Does this world have an Earthside/Farside divide (the Moon, under its Earthrise)?
pub fn has_farside(kind: PlanetKind) -> bool {
    kind.def().has_earthrise
}

/// `dir` is on the hemisphere facing away from the Earth. Pass `has_farside` in, so a
/// per-gem or per-anchor loop does not rebuild the planet table each time.
pub fn is_farside(has_farside: bool, dir: Vec3) -> bool {
    has_farside && dir.dot(earth_dir()) < 0.0
}

/// §8 Farside: the elite roll's multiplier, for the share of the spawn anchors (astronauts)
/// standing on Farside — +20% with the whole party there.
pub fn farside_elite_mult(kind: PlanetKind, anchors: impl Iterator<Item = Vec3>) -> f32 {
    let far = has_farside(kind);
    if !far {
        return 1.0;
    }
    let (mut n, mut on) = (0usize, 0usize);
    for d in anchors {
        n += 1;
        on += usize::from(is_farside(far, d));
    }
    if n == 0 {
        1.0
    } else {
        1.0 + FARSIDE_ELITE_BONUS * on as f32 / n as f32
    }
}

// ---------------------------------------------------------------------------------------
// Simulation

/// HOST: turn the sun, and let the invited danger eat it (§3 diegetic difficulty): every
/// SUN_EAT_SECS the day side shrinks by SUN_SHARD_STEP while anyone carries a Devoured Sun
/// Shard (a world-level lever, like The Static Radio), plus SUN_CURSED_STEP per unit of the
/// party's Difficulty. The clock only runs while something is eating.
pub fn advance_sun(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut telemetry: ResMut<crate::items::ItemTelemetry>,
    q: Query<&PlayerState>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    run.sun_phase = (run.sun_phase + sun_rate(run.stage) * dt).rem_euclid(std::f32::consts::TAU);
    if run.sun_shrink >= 1.0 {
        return;
    }
    let shard = q.iter().any(|ps| ps.has_item(ItemKind::DevouredSunShard));
    let step = if shard { SUN_SHARD_STEP } else { 0.0 } + SUN_CURSED_STEP * run.difficulty.max(0.0);
    if step <= 0.0 {
        return;
    }
    run.sun_eat_secs += dt;
    if run.sun_eat_secs >= SUN_EAT_SECS {
        run.sun_eat_secs -= SUN_EAT_SECS;
        run.sun_shrink = (run.sun_shrink + step).min(1.0);
        if shard {
            telemetry.sun_steps += 1;
        }
    }
}

/// CLIENT: keep the sun turning between the host's 4 Hz snapshots, so the terminator and
/// the shadows glide instead of stepping. The next snapshot corrects any drift.
pub fn drift_sun(time: Res<Time>, mut run: ResMut<RunState>) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        run.sun_phase = (run.sun_phase + sun_rate(run.stage) * dt).rem_euclid(std::f32::consts::TAU);
    }
}

// ---------------------------------------------------------------------------------------
// Presentation (every machine, from the run's sun and its own astronaut)

/// The sun's key light, as `planet::spawn_stage` builds it.
#[derive(Component)]
pub struct SunLight;
/// The visible sun.
#[derive(Component)]
pub struct SunDisc;
/// The Moon's Earthlight: a cyan fill from the fixed Earth, lighting Earthside only.
#[derive(Component)]
pub struct EarthFill;

/// Where the key light is drawn from, for an observer at `at`. With the day side whole this
/// IS the sun. Eaten, the lit cap is smaller than the hemisphere a directional light can
/// light, so the light is tipped away from the observer by the eaten angle: the terminator
/// they can see then lies exactly on `Sun::is_night`'s edge (the far side of the cap is
/// over their horizon). Faded to nothing near the sub-solar point, where "away" has no
/// direction.
pub fn drawn_sun(sun: &Sun, at: Vec3) -> Vec3 {
    if sun.shrink <= 0.0 || sun.locked() {
        return sun.toward;
    }
    let eaten = sun.shrink.asin();
    let w = smoothstep(0.0, 0.35, at.angle_between(sun.toward));
    let axis = sun.toward.cross(at).try_normalize().unwrap_or_else(|| sphere::tangent_frame(sun.toward).0);
    (Quat::from_axis_angle(axis, -eaten * w) * sun.toward).normalize()
}

/// The eased sky state of this machine's view.
#[derive(Default)]
pub struct SkyEase {
    /// Which world (seed, stage) the ease belongs to — a new one snaps instead of fading.
    key: Option<(u64, usize)>,
    day: f32,
    far: f32,
}

/// Every machine: light the planet from the run's sun — the key light's direction and
/// strength, the sun disc, and an ambient that sinks as the local astronaut crosses into
/// the night (and, on the Moon, onto Farside, away from the Earthlight).
#[allow(clippy::type_complexity)]
pub fn apply_sky(
    time: Res<Time>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut ease: Local<SkyEase>,
    local: Query<&Player, With<LocalPlayer>>,
    mut lights: Query<(&mut DirectionalLight, &mut Transform), (With<SunLight>, Without<SunDisc>)>,
    mut discs: Query<(&mut Transform, &mut Visibility), (With<SunDisc>, Without<SunLight>)>,
) {
    let light = planet.kind.def().light;
    let sun = Sun::of(&run);
    let at = local.iter().next().map(|p| p.dir).unwrap_or(Vec3::Y);
    let drawn = drawn_sun(&sun, at);

    let lux = if sun.locked() { 0.0 } else { light.sun_lux * (1.0 - SUN_DIM_AT_FULL_SHRINK * sun.shrink) };
    for (mut l, mut tf) in &mut lights {
        if (l.illuminance - lux).abs() > 1.0 {
            l.illuminance = lux;
        }
        let up = sphere::tangent_frame(drawn).0;
        *tf = Transform::from_translation(drawn * 10.0).looking_at(Vec3::ZERO, up);
    }
    let scale = 1.0 - 0.85 * sun.shrink;
    for (mut tf, mut vis) in &mut discs {
        tf.translation = drawn * 1600.0;
        tf.scale = Vec3::splat(scale.max(0.05));
        let want = if sun.locked() { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
    }

    // ambient: eased, so crossing the terminator is a dusk rather than a switch
    let day = sun.daylight(at);
    let far = if is_farside(has_farside(planet.kind), at) { 1.0 } else { 0.0 };
    let key = (run.run_seed, run.stage);
    if ease.key != Some(key) {
        *ease = SkyEase { key: Some(key), day, far };
    }
    let k = (AMBIENT_EASE * time.delta_secs()).min(1.0);
    ease.day += (day - ease.day) * k;
    ease.far += (far - ease.far) * k;
    let bright = (light.ambient_night + (light.ambient_day - light.ambient_night) * ease.day)
        * (1.0 - FARSIDE_AMBIENT_DROP * ease.far);
    let (n, d) = (light.ambient_night_color.to_linear(), light.ambient_day_color.to_linear());
    let color = Color::LinearRgba(n.mix(&d, ease.day));
    if (ambient.brightness - bright).abs() > 0.05 {
        ambient.brightness = bright;
    }
    ambient.color = color;
}

/// Every machine: the Mission-Control line the first time this astronaut walks into the
/// night (and, on the Moon, onto Farside) each stage, so the rules of the biome are said
/// once where they start to apply — and the sun being eaten, which a joiner sees from the
/// streamed `sun_shrink` (host banners do not cross the wire).
pub fn sky_notices(
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    local: Query<&Player, With<LocalPlayer>>,
    mut banners: MessageWriter<BannerMsg>,
    mut seen: Local<(Option<(u64, usize)>, bool, bool, f32)>,
) {
    let Some(p) = local.iter().next() else { return };
    let key = (run.run_seed, run.stage);
    if seen.0 != Some(key) {
        *seen = (Some(key), false, false, run.sun_shrink);
    }
    let sun = Sun::of(&run);
    if run.sun_shrink > seen.3 + 1e-4 {
        seen.3 = run.sun_shrink;
        banners.write(BannerMsg(if sun.locked() {
            "THE SUN IS GONE. NIGHT-LOCK.".into()
        } else {
            format!("THE SUN SHRINKS ({:.0}% EATEN)", run.sun_shrink * 100.0)
        }));
    }
    if !seen.1 && !sun.locked() && sun.is_night(p.dir) {
        seen.1 = true;
        banners.write(BannerMsg("NIGHTFALL: THEY RUN FASTER. KILLS PAY MORE GOLD.".into()));
    }
    if !seen.2 && is_farside(has_farside(planet.kind), p.dir) {
        seen.2 = true;
        banners.write(BannerMsg("FARSIDE: NO EARTHLIGHT. ELITES PROWL. GEMS BURN BRIGHT.".into()));
    }
}

/// Every machine: §8 "gems glow brighter" on Farside — an XP gem there wears the hotter
/// material. Two shared materials per gem size, swapped by handle, so it costs nothing per
/// gem on the GPU.
pub fn farside_gems(
    planet: Res<CurrentPlanet>,
    assets: Res<PickupAssets>,
    mut q: Query<(&Pickup, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let far_world = has_farside(planet.kind);
    for (p, mut mat) in &mut q {
        let PickupKind::Xp(v) = p.kind else { continue };
        let far = is_farside(far_world, p.dir);
        let want = match (v >= 10.0, far) {
            (false, false) => &assets.gem_mat,
            (true, false) => &assets.big_gem_mat,
            (false, true) => &assets.gem_far_mat,
            (true, true) => &assets.big_gem_far_mat,
        };
        if mat.0 != *want {
            mat.0 = want.clone();
        }
    }
}

// ---------------------------------------------------------------------------------------

/// Headless self-check: the sun turns once a stage and opens every stage on a lit crash
/// site; night is the side facing away and the eaten rim; night-lock is total; the night
/// modifiers have the §4 sizes; Farside is the Earth's far hemisphere.
pub fn self_check() -> Result<(), String> {
    use std::f32::consts::{PI, TAU};
    for kind in PlanetKind::ALL {
        let dawn = Sun::at(kind, 0.0, 0.0);
        if dawn.is_night(Vec3::Y) || dawn.daylight(Vec3::Y) < 0.99 {
            return Err(format!("{kind:?}: a stage does not open in daylight at the crash site"));
        }
        if !dawn.is_night(-dawn.toward) || dawn.is_night(dawn.toward) {
            return Err(format!("{kind:?}: is_night disagrees with the sun"));
        }
        // half a turn later the crash site is deep in the night
        let dusk = Sun::at(kind, PI + SUN_START_BEFORE_NOON, 0.0);
        if !dusk.is_night(Vec3::Y) {
            return Err(format!("{kind:?}: the crash site never sees night"));
        }
        let turn = sun_rate(0) * STAGE_SECONDS[0];
        if (turn - TAU * SUN_TURNS_PER_STAGE).abs() > 1e-3 {
            return Err("the sun does not turn once per stage".into());
        }
        // the eaten sun: its rim is night, its noon is not, until night-lock takes all
        let eaten = Sun::at(kind, 0.0, 0.5);
        let rim = sphere::offset_dir(eaten.toward, sphere::tangent_frame(eaten.toward).0, 1.2, 1.0);
        if !eaten.is_night(rim) || eaten.is_night(eaten.toward) || !Sun::at(kind, 0.0, 1.0).is_night(eaten.toward) {
            return Err(format!("{kind:?}: the Sun Shard does not eat the day from its rim"));
        }
        // the drawn light's terminator lies on the eaten edge for an observer near it
        let edge = sphere::offset_dir(eaten.toward, sphere::tangent_frame(eaten.toward).0, 0.5f32.acos(), 1.0);
        let lit = edge.dot(drawn_sun(&eaten, edge));
        if lit.abs() > 0.02 {
            return Err(format!("{kind:?}: the drawn terminator misses the eaten edge ({lit:.3})"));
        }
    }
    let sun = Sun::at(PlanetKind::Moon, 0.0, 0.0);
    if (sun.enemy_speed(-sun.toward) - (1.0 + NIGHT_ENEMY_SPEED)).abs() > 1e-4 || (sun.enemy_speed(sun.toward) - 1.0).abs() > 1e-4 {
        return Err("night speed is not +15% on the night side and nothing by day".into());
    }
    if sun.spawn_arc(-sun.toward) != NIGHT_SPAWN_ARC_MULT || sun.spawn_arc(sun.toward) != 1.0 {
        return Err("night spawns do not land closer".into());
    }
    // the night Gold pays +25% on average, even on 1-coin drops
    for amount in [1u64, 2, 3, 5, 8] {
        let mean: f32 = (0..1000).map(|i| night_gold(amount, (i as f32 + 0.5) / 1000.0) as f32).sum::<f32>() / 1000.0;
        if (mean - amount as f32 * NIGHT_GOLD_MULT).abs() > 0.01 {
            return Err(format!("night Gold on {amount} averages {mean:.3}"));
        }
    }
    if !is_farside(true, -earth_dir()) || is_farside(true, earth_dir()) || is_farside(false, -earth_dir()) {
        return Err("Farside is not the Earth's far hemisphere".into());
    }
    if is_farside(has_farside(PlanetKind::Moon), Vec3::Y) {
        return Err("the Moon's crash site is not on Earthside".into());
    }
    let all_far = farside_elite_mult(PlanetKind::Moon, [-earth_dir(), -earth_dir()].into_iter());
    let half = farside_elite_mult(PlanetKind::Moon, [-earth_dir(), earth_dir()].into_iter());
    if (all_far - (1.0 + FARSIDE_ELITE_BONUS)).abs() > 1e-5 || (half - (1.0 + FARSIDE_ELITE_BONUS * 0.5)).abs() > 1e-5 {
        return Err("Farside's elite bump is not +20%".into());
    }
    if farside_elite_mult(PlanetKind::Mars, [-earth_dir()].into_iter()) != 1.0 {
        return Err("Mars has a Farside".into());
    }
    Ok(())
}

/// `--netlog`: every 5 s, this machine's sun and world gimmicks — the proof a joiner lights
/// from the host's sun (compare the two sides' `phase`/`shrink`), draws the host's Crawl and
/// spore cycles (`spores` counts cycles standing here, `wire` those built from the hazard
/// lane), and predicts its own thorn snags.
#[allow(clippy::too_many_arguments)]
pub fn log_sky(
    time: Res<Time>,
    role: Res<crate::net::NetRole>,
    run: Res<RunState>,
    crawl: Res<crate::gimmicks::Crawl>,
    telemetry: Res<crate::gimmicks::GimmickTelemetry>,
    local: Query<(&Player, &PlayerState), With<LocalPlayer>>,
    spores: Query<(), With<crate::gimmicks::SporeBurst>>,
    wired: Query<(), (With<crate::gimmicks::SporeBurst>, With<crate::netenemy::NetHazard>)>,
    mut next: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 5.0;
    let sun = Sun::of(&run);
    let (night, thorned) = local.iter().next().map(|(p, ps)| (sun.is_night(p.dir), ps.thorned > 0.0)).unwrap_or((false, false));
    let erupting = crawl.sites.iter().filter(|s| s.erupting()).count();
    info!(
        "SKY[{:?}] phase={:.3} shrink={:.2} night={} thorned={} snags={} | crawl={}/{} erupting | spores={} wire={} primes={} pops={}",
        *role,
        run.sun_phase,
        run.sun_shrink,
        night,
        thorned,
        telemetry.thorn_snags,
        erupting,
        crawl.sites.len(),
        spores.iter().count(),
        wired.iter().count(),
        telemetry.spore_primes,
        telemetry.spore_pops
    );
}
