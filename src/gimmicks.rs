//! §8 "A world is not a skin; it is a rule that rewrites how you run": each world's standing
//! gimmick and hazard — the rules the ground itself plays by, all stage long. (The timed set
//! pieces, planetary EVENTS, are `events_world`; the Moon's Earthside/Farside divide lives
//! with the light it changes, in `daynight`.)
//!
//! * MARS — thorn flora slows on contact. The bushes are the stage's own seeded flora
//!   (`WorldFlora`, laid by `planet::spawn_stage`), so every machine holds the same ones and
//!   `player_physics` snags each body a machine moves: a joiner predicts its own snag.
//! * DARK MOON — fungus that detonates spore clouds, telegraphed. An astronaut near a ripe
//!   cap primes it (`spore_sim`, HOST): a danger disc counts down, the cap swells, it bursts
//!   (hurting astronauts AND the horde — lure them in) and leaves a spore cloud that eats
//!   HP. `SporeBurst` rides the hazard lane (`HazardEvent::Spore`), so a joiner draws the
//!   same swell, cloud and regrowth (`spore_clock` + `spore_visuals` run everywhere).
//! * DARK MOON — THE CRAWL. The planet is faintly translucent, so you see The Static massing
//!   under the crust on the far side and know where it will erupt: sites mass for
//!   CRAWL_MASS_SECS before The Static rises, then spew its ghosts (`director_spawn` asks
//!   `Crawl::ghost_dir`), and each one's successor is on show before it closes (`crawl_sim`,
//!   HOST; the sites ride `RunSnapMsg`). The markers draw through the ground with an x-ray
//!   material (`XRay`) so the far side reads.

use crate::config::*;
use crate::content::planets::{FloraStyle, PlanetKind};
use crate::enemies::{EnemyAssets, SpatialHash, Telegraph};
use crate::messages::{HitMsg, PlayerHitMsg, Sfx, SfxMsg};
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{LocalPlayer, Player};
use crate::run::scaling::Scaling;
use crate::run::{GameRng, PlayerState, RunState};
use crate::sphere;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError};
use rand::{Rng, SeedableRng};

/// One plant of the stage's flora, as `planet::spawn_stage` laid it. Indexed identically on
/// every machine (the scatter is seeded), which is what lets the hazard lane name a plant.
#[derive(Clone, Copy, Debug)]
pub struct Plant {
    pub dir: Vec3,
    pub scale: f32,
    /// The plant's root entity (its parts are children), for the swell and regrowth.
    pub entity: Entity,
}

/// The stage's flora and what kind it is. Rebuilt with the world each stage.
#[derive(Resource, Default)]
pub struct WorldFlora {
    pub style: Option<FloraStyle>,
    pub plants: Vec<Plant>,
}

impl WorldFlora {
    /// Mars: is a body of `radius` at `dir`, `height` over the ground, in a thorn bush? A
    /// jump over it clears it.
    pub fn thorn_contact(&self, dir: Vec3, height: f32, radius: f32, planet_r: f32) -> bool {
        self.style == Some(FloraStyle::Thorns)
            && self.plants.iter().any(|pl| {
                height < THORN_HEIGHT * pl.scale && sphere::arc_dist(dir, pl.dir, planet_r) < THORN_REACH * pl.scale + radius
            })
    }
}

/// What the gimmicks did — the headless probes read it rather than the systems carrying
/// probe counters of their own. Per machine.
#[derive(Resource, Default, Debug)]
pub struct GimmickTelemetry {
    pub thorn_snags: u32,
    pub thorn_secs: f32,
    pub spore_primes: u32,
    pub spore_pops: u32,
    pub spore_enemy_hits: u32,
    pub spore_player_hits: u32,
    pub crawl_sites: u32,
    pub crawl_ghosts: u32,
}

// ---------------------------------------------------------------------------------------
// Spore fungus (Dark Moon)

/// One cap's detonation cycle: fuse (the telegraph) → burst → spore cloud → regrowth. The
/// cap is spent for as long as this lives. The HOST's copy (`live`) deals the damage; a
/// joiner's, built from the hazard lane, only draws it.
#[derive(Component)]
pub struct SporeBurst {
    /// Index into `WorldFlora::plants`.
    pub plant: u16,
    pub dir: Vec3,
    pub age: f32,
    pub live: bool,
    /// HOST: the burst has gone off (its one hit on the horde is dealt).
    pub popped: bool,
    /// HOST: seconds to the cloud's next bite.
    pub tick: f32,
}

impl SporeBurst {
    pub fn new(plant: u16, dir: Vec3, live: bool) -> Self {
        Self { plant, dir, age: 0.0, live, popped: false, tick: 0.0 }
    }
    /// Seconds since the cap burst, if it has.
    pub fn since_pop(&self) -> Option<f32> {
        (self.age >= SPORE_FUSE).then(|| self.age - SPORE_FUSE)
    }
}

/// The burst's entity: where the cap stands, carrying the cloud's parts as children.
pub fn spore_bundle(planet: &CurrentPlanet, burst: SporeBurst) -> impl Bundle {
    let tf = Transform::from_translation(planet.surface_point(burst.dir))
        .with_rotation(sphere::frame_quat(burst.dir, sphere::tangent_frame(burst.dir).0));
    (burst, tf, Visibility::default(), StageScoped)
}

/// Every machine: age each cycle; a cap that has regrown is ripe again.
pub fn spore_clock(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut SporeBurst)>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut b) in &mut q {
        b.age += dt;
        if b.age >= SPORE_FUSE + SPORE_REGROW {
            commands.entity(e).despawn();
        }
    }
}

/// HOST: prime the ripe caps astronauts walk up to, and let the bursts and clouds bite.
#[allow(clippy::too_many_arguments)]
pub fn spore_sim(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    flora: Res<WorldFlora>,
    enemy_assets: Res<EnemyAssets>,
    hash: Res<SpatialHash>,
    astronauts: Query<(Entity, &Player, &PlayerState, &Transform)>,
    mut bursts: Query<(&mut SporeBurst, &Transform)>,
    (mut hits, mut player_hits, mut telemetry): (MessageWriter<HitMsg>, MessageWriter<PlayerHitMsg>, ResMut<GimmickTelemetry>),
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || flora.style != Some(FloraStyle::GlowShrooms) {
        return;
    }
    let standing: Vec<(Entity, Vec3)> = astronauts.iter().filter(|(_, _, ps, _)| !ps.dead).map(|(e, p, _, _)| (e, p.dir)).collect();
    let sc = Scaling::for_run(&run, astronauts.iter().count());

    // bursts and clouds
    let mut spent = vec![false; flora.plants.len()];
    for (mut b, tf) in &mut bursts {
        if let Some(s) = spent.get_mut(b.plant as usize) {
            *s = true;
        }
        if !b.live {
            continue;
        }
        let Some(since) = b.since_pop() else { continue };
        if !b.popped {
            // the burst: the telegraph's own detonation hurts the astronauts; this is the
            // horde's share — a primed cap is a trap you can lead them into
            b.popped = true;
            telemetry.spore_pops += 1;
            let at = tf.translation;
            for (e, pos) in hash.near(at, SPORE_RADIUS) {
                let off = pos - at;
                if off.length() < SPORE_RADIUS {
                    let out = (off - b.dir * off.dot(b.dir)).normalize_or_zero();
                    hits.write(HitMsg { source: None, target: e, amount: SPORE_ENEMY_DAMAGE * sc.hp, crit: false, knock: out * 14.0, weapon: None });
                    telemetry.spore_enemy_hits += 1;
                }
            }
        }
        if since < SPORE_CLOUD_SECS {
            b.tick -= dt;
            if b.tick <= 0.0 {
                b.tick += SPORE_TICK;
                for (e, d) in &standing {
                    if sphere::arc_dist(*d, b.dir, planet.radius) < SPORE_CLOUD_RADIUS {
                        // chip damage: no knock (camera law 9)
                        player_hits.write(PlayerHitMsg { victim: *e, amount: SPORE_CLOUD_DPS * SPORE_TICK * sc.dmg, from: tf.translation, attacker: None });
                        telemetry.spore_player_hits += 1;
                    }
                }
            }
        }
    }

    // prime the ripe caps somebody is standing near
    for (i, pl) in flora.plants.iter().enumerate() {
        if spent[i] || !standing.iter().any(|(_, d)| sphere::arc_dist(*d, pl.dir, planet.radius) < SPORE_TRIGGER) {
            continue;
        }
        telemetry.spore_primes += 1;
        commands.spawn(spore_bundle(&planet, SporeBurst::new(i as u16, pl.dir, true)));
        // the danger disc IS the fuse (and streams to joiners on its own, like every telegraph)
        commands.spawn(crate::enemies::telegraph_bundle(
            &enemy_assets,
            &planet,
            Telegraph { timer: SPORE_FUSE, max: SPORE_FUSE, radius: SPORE_RADIUS, damage: SPORE_BURST_DAMAGE * sc.dmg, dir: pl.dir, ring: false },
        ));
    }
}

/// The spore cloud's shared parts (presentation; not built headless).
#[derive(Resource)]
pub struct GimmickAssets {
    pub cloud_mesh: Handle<Mesh>,
    pub cloud_mat: Handle<StandardMaterial>,
    pub crawl_mesh: Handle<Mesh>,
    pub crawl_mat: Handle<XRayMaterial>,
}

/// A part of a spore cloud, a child of its `SporeBurst`: the billowing dome, or its lethal
/// edge lying on the ground (the danger palette's ring).
#[derive(Component)]
pub struct SporeCloudPart {
    pub dome: bool,
}

pub fn setup_gimmick_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut xray: ResMut<Assets<XRayMaterial>>,
) {
    use crate::meshkit::{at, MeshData};
    // The Crawl: a knot of static "pixels" around a thin column — The Static as a glitch in
    // the ground rather than a creature (§12: film-grain static, scanline flicker).
    let mut m = MeshData::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xC4A71);
    for _ in 0..14 {
        let p = Vec3::new(rng.gen_range(-0.9..0.9), rng.gen_range(-0.2..1.8), rng.gen_range(-0.9..0.9));
        let s = rng.gen_range(0.18..0.5);
        m.add_box(Vec3::splat(s), at(p), Color::WHITE);
    }
    m.add_cylinder(0.12, 6.0, 6, at(Vec3::new(0.0, 3.0, 0.0)), Color::WHITE);
    commands.insert_resource(GimmickAssets {
        cloud_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
        cloud_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.62, 0.95, 0.55, 0.22),
            emissive: LinearRgba::rgb(0.5, 0.9, 0.45),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        crawl_mesh: meshes.add(m.build()),
        crawl_mat: xray.add(ExtendedMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(0.85, 0.35, 1.0, 0.55),
                emissive: LinearRgba::rgb(2.2, 0.8, 2.8),
                alpha_mode: AlphaMode::Add,
                unlit: true,
                ..default()
            },
            extension: XRay {},
        }),
    });
}

/// Every machine: the cap swells and throbs through its fuse, bursts flat, and regrows; the
/// cloud billows over the burst and thins away. Children are built for each new cycle, so a
/// joiner's cycle from the hazard lane gets them too.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn spore_visuals(
    mut commands: Commands,
    time: Res<Time>,
    save: Res<crate::save::MetaSave>,
    planet: Res<CurrentPlanet>,
    flora: Res<WorldFlora>,
    (assets, enemy_assets): (Res<GimmickAssets>, Res<EnemyAssets>),
    local: Query<&Player, With<LocalPlayer>>,
    added: Query<(Entity, &SporeBurst), Added<SporeBurst>>,
    bursts: Query<(Entity, &SporeBurst, &Children)>,
    mut plants: Query<&mut Transform, (Without<SporeCloudPart>, Without<SporeBurst>)>,
    mut parts: Query<(&SporeCloudPart, &mut Transform, &mut Visibility), Without<SporeBurst>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut popped: Local<Vec<Entity>>,
) {
    let near_me = |dir: Vec3| local.iter().any(|p| sphere::arc_dist(p.dir, dir, planet.radius) < SPORE_TRIGGER * 4.0);
    for (e, b) in &added {
        commands.entity(e).with_children(|c| {
            c.spawn((
                SporeCloudPart { dome: true },
                Mesh3d(assets.cloud_mesh.clone()),
                MeshMaterial3d(assets.cloud_mat.clone()),
                Transform::from_scale(Vec3::ZERO),
                Visibility::Hidden,
                bevy::light::NotShadowCaster,
            ));
            c.spawn((
                SporeCloudPart { dome: false },
                Mesh3d(enemy_assets.ring_mesh.clone()),
                MeshMaterial3d(enemy_assets.ring_mat.clone()),
                Transform::from_xyz(0.0, TELEGRAPH_LIFT, 0.0).with_scale(Vec3::ZERO),
                Visibility::Hidden,
            ));
        });
        if near_me(b.dir) {
            sfx.write(SfxMsg(Sfx::Spore));
        }
    }
    let t = time.elapsed_secs();
    // under photosensitivity the throb stays well below 3/s
    let throb_hz = if save.accessibility.photosensitive { 1.2 } else { 4.0 };
    popped.retain(|e| bursts.contains(*e));
    for (e, b, children) in &bursts {
        let Some(pl) = flora.plants.get(b.plant as usize) else { continue };
        // the cap: swells and throbs as the fuse burns, is blown flat, then regrows
        let since = b.since_pop();
        let k = match since {
            None => {
                let f = (b.age / SPORE_FUSE).clamp(0.0, 1.0);
                1.0 + 0.35 * f + 0.06 * f * (t * std::f32::consts::TAU * throb_hz).sin()
            }
            Some(s) => {
                let g = (s / SPORE_REGROW).clamp(0.0, 1.0);
                0.35 + 0.65 * g * g * (3.0 - 2.0 * g)
            }
        };
        if let Ok(mut tf) = plants.get_mut(pl.entity) {
            tf.scale = Vec3::splat(pl.scale * k);
        }
        if since.is_some() && !popped.contains(&e) {
            popped.push(e);
            if near_me(b.dir) {
                sfx.write(SfxMsg(Sfx::SporePop));
            }
        }
        // the cloud: billows out fast, holds, thins away
        let cloud = since.filter(|s| *s < SPORE_CLOUD_SECS).map(|s| {
            let grow = (s / 0.35).min(1.0);
            let fade = ((SPORE_CLOUD_SECS - s) / 0.6).min(1.0);
            grow * fade
        });
        for child in children.iter() {
            let Ok((part, mut tf, mut vis)) = parts.get_mut(child) else { continue };
            let want = if cloud.is_some() { Visibility::Inherited } else { Visibility::Hidden };
            if *vis != want {
                *vis = want;
            }
            let Some(c) = cloud else { continue };
            if part.dome {
                let billow = 1.0 + 0.05 * (t * 1.7 + b.plant as f32).sin();
                tf.scale = Vec3::new(1.0, 0.62, 1.0) * SPORE_CLOUD_RADIUS * c * billow;
                tf.translation = Vec3::Y * 0.6;
            } else {
                tf.scale = Vec3::splat(SPORE_CLOUD_RADIUS * c.max(0.05));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// The Crawl (Dark Moon)

/// One place The Static is massing under the crust. `mass` climbs 0→1 while it masses (one
/// unit per CRAWL_MASS_SECS) and on past 1 while it erupts, until the eruption is spent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrawlSite {
    pub dir: Vec3,
    pub mass: f32,
}

/// `mass` at which an erupting site closes.
const CRAWL_END: f32 = 1.0 + CRAWL_ERUPT_SECS / CRAWL_MASS_SECS;
/// Seconds a successor's eruption overlaps the close of the site it replaces.
const CRAWL_OVERLAP: f32 = 1.0;

impl CrawlSite {
    pub fn erupting(&self) -> bool {
        // a hair early: massing in frame steps must not miss the frame The Static rises on
        self.mass >= 1.0 - 1e-3
    }
    /// Seconds of its life (massing and erupting) left.
    pub fn secs_left(&self) -> f32 {
        (CRAWL_END - self.mass) * CRAWL_MASS_SECS
    }
}

/// The Dark Moon's Crawl sites: the HOST's to place, a joiner's from `RunSnapMsg`.
#[derive(Resource, Default, Debug)]
pub struct Crawl {
    pub sites: Vec<CrawlSite>,
    /// HOST: which astronaut the next site is placed around (round-robin).
    next: usize,
}

impl Crawl {
    /// Mass every site on by `dt` and close the spent ones — the host's step and a joiner's
    /// dead reckoning between snapshots alike.
    pub fn tick(&mut self, dt: f32) {
        for s in &mut self.sites {
            s.mass += dt / CRAWL_MASS_SECS;
        }
        self.sites.retain(|s| s.mass < CRAWL_END);
    }

    /// Where the next ghost of The Static erupts: near a random erupting site — or, in the
    /// frame The Static rises before its sites have quite massed, the heaviest one (it breaks
    /// through where it was seen massing) — and `None` with no sites at all (the ring spawns
    /// as on any world).
    pub fn ghost_dir(&self, rng: &mut impl Rng, planet_r: f32) -> Option<Vec3> {
        let n = self.sites.iter().filter(|s| s.erupting()).count();
        let site = if n == 0 {
            self.sites.iter().max_by(|a, b| a.mass.total_cmp(&b.mass))?
        } else {
            self.sites.iter().filter(|s| s.erupting()).nth(rng.gen_range(0..n))?
        };
        let (t, b) = sphere::tangent_frame(site.dir);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        Some(sphere::offset_dir(site.dir, t * a.cos() + b * a.sin(), rng.gen_range(0.0..CRAWL_SPREAD), planet_r))
    }

    /// The wire form: direction + mass per site.
    pub fn to_wire(&self) -> Vec<[f32; 4]> {
        self.sites.iter().map(|s| [s.dir.x, s.dir.y, s.dir.z, s.mass]).collect()
    }
    pub fn adopt(&mut self, wire: &[[f32; 4]]) {
        self.sites = wire.iter().map(|w| CrawlSite { dir: Vec3::new(w[0], w[1], w[2]).normalize_or(Vec3::Y), mass: w[3] }).collect();
    }
}

/// HOST: The Crawl. From CRAWL_MASS_SECS before The Static rises (early with The Static
/// Radio), keep CRAWL_SITES (+1 per extra astronaut) sites massing or erupting, each over an
/// astronaut's horizon — a new one massing as soon as a site has no more than a massing's
/// worth of eruption left, so the next place it will come from is always on show.
pub fn crawl_sim(
    time: Res<Time>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    mut crawl: ResMut<Crawl>,
    mut rng: ResMut<GameRng>,
    mut telemetry: ResMut<GimmickTelemetry>,
    astronauts: Query<(&Player, &PlayerState)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let lead = if run.static_radio { STATIC_RADIO_LEAD_SECS } else { 0.0 };
    let until_static = (run.timer - lead).max(0.0);
    if planet.kind != PlanetKind::DarkMoon || (!run.static_active && until_static > CRAWL_MASS_SECS) {
        crawl.sites.clear();
        return;
    }
    crawl.tick(dt);
    if !run.static_active {
        // Before The Static every site masses toward the same moment — the clock running out —
        // so slave them to the clock instead of integrating on their own: a frame where the
        // clock ticked and this system did not (a level-up panel opening mid-frame stops the
        // `playing` systems after run_clock) used to leave them ~0.03 s short per level-up,
        // not yet erupting when The Static rose.
        let m = (1.0 - until_static / CRAWL_MASS_SECS).clamp(0.0, 1.0);
        for site in &mut crawl.sites {
            site.mass = m;
        }
    }
    let anchors: Vec<Vec3> = astronauts.iter().filter(|(_, ps)| !ps.dead).map(|(p, _)| p.dir).collect();
    if anchors.is_empty() {
        return;
    }
    let want = CRAWL_SITES + anchors.len() - 1;
    // a successor a beat early, so the eruptions overlap rather than gap for a frame
    let showing = crawl.sites.iter().filter(|s| s.secs_left() > CRAWL_MASS_SECS + CRAWL_OVERLAP).count();
    for _ in showing..want {
        let anchor = anchors[crawl.next % anchors.len()];
        crawl.next = crawl.next.wrapping_add(1);
        let rng = &mut rng.0;
        let (t, b) = sphere::tangent_frame(anchor);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        let arc = rng.gen_range(CRAWL_ARC_MIN..CRAWL_ARC_MAX);
        let dir = sphere::offset_dir(anchor, t * a.cos() + b * a.sin(), arc, planet.radius);
        // Before The Static, a site masses exactly until it rises; once it is up, from zero.
        let mass = if run.static_active { 0.0 } else { (1.0 - until_static / CRAWL_MASS_SECS).clamp(0.0, 1.0) };
        crawl.sites.push(CrawlSite { dir, mass });
        telemetry.crawl_sites += 1;
    }
}

/// CLIENT: mass the streamed sites on between snapshots.
pub fn crawl_drift(time: Res<Time>, mut crawl: ResMut<Crawl>) {
    let dt = time.delta_secs();
    if dt > 0.0 {
        crawl.tick(dt);
    }
}

/// A Crawl marker (pooled, one per site slot).
#[derive(Component)]
pub struct CrawlMarker(pub usize);

/// Every machine: draw each site as a knot of static under the crust — seen THROUGH the
/// planet (the x-ray material), sunk deep and small while it masses, rising and swelling to
/// the surface as it comes, and churning while it erupts.
#[allow(clippy::too_many_arguments)]
pub fn crawl_visuals(
    mut commands: Commands,
    time: Res<Time>,
    save: Res<crate::save::MetaSave>,
    planet: Res<CurrentPlanet>,
    crawl: Res<Crawl>,
    assets: Res<GimmickAssets>,
    mut markers: Query<(&CrawlMarker, &mut Transform, &mut Visibility)>,
) {
    let have = markers.iter().count();
    for i in have..crawl.sites.len() {
        commands.spawn((
            CrawlMarker(i),
            Mesh3d(assets.crawl_mesh.clone()),
            MeshMaterial3d(assets.crawl_mat.clone()),
            Transform::from_scale(Vec3::ZERO),
            Visibility::Hidden,
            bevy::light::NotShadowCaster,
            StageScoped,
        ));
    }
    let t = time.elapsed_secs();
    let churn_hz = if save.accessibility.photosensitive { 0.8 } else { 2.2 };
    for (m, mut tf, mut vis) in &mut markers {
        let Some(site) = crawl.sites.get(m.0) else {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
            }
            continue;
        };
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        let rise = site.mass.min(1.0);
        let (sink, size) = if site.erupting() {
            let churn = 1.0 + 0.12 * (t * std::f32::consts::TAU * churn_hz + m.0 as f32).sin();
            (0.0, 1.9 * churn)
        } else {
            ((1.0 - rise) * 3.5, 0.5 + 1.2 * rise)
        };
        let spin = t * (0.6 + 1.8 * rise) + m.0 as f32 * 2.1;
        *tf = Transform::from_translation(planet.surface_point(site.dir) - site.dir * sink)
            .with_rotation(sphere::frame_quat(site.dir, sphere::tangent_frame(site.dir).0) * Quat::from_rotation_y(spin))
            .with_scale(Vec3::splat(size));
    }
}

/// A StandardMaterial that ignores the depth buffer: whatever wears it is drawn over the
/// ground in front of it — The Crawl seen through the planet's crust. Additive and unlit, so
/// it reads as a glow bleeding through rather than an object pasted on top.
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct XRay {}

impl MaterialExtension for XRay {
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_compare = CompareFunction::Always;
            depth.depth_write_enabled = false;
        }
        Ok(())
    }
}

pub type XRayMaterial = ExtendedMaterial<StandardMaterial, XRay>;

/// Headless self-check: a site masses for exactly CRAWL_MASS_SECS, erupts for
/// CRAWL_ERUPT_SECS, spawns only once erupting and within CRAWL_SPREAD of itself; a successor
/// is due exactly a massing before a site closes; a thorn bush snags below its height only.
pub fn self_check() -> Result<(), String> {
    let mut crawl = Crawl { sites: vec![CrawlSite { dir: Vec3::Y, mass: 0.0 }], next: 0 };
    let mut rng = rand::rngs::StdRng::seed_from_u64(7);
    let steps = (CRAWL_MASS_SECS / 0.1).round() as usize;
    for _ in 0..steps - 1 {
        crawl.tick(0.1);
    }
    if crawl.sites[0].erupting() {
        return Err("a Crawl site erupted before it had massed".into());
    }
    if Crawl::default().ghost_dir(&mut rng, 100.0).is_some() {
        return Err("The Static erupted from a Crawl with no sites".into());
    }
    crawl.tick(0.2);
    let Some(g) = crawl.ghost_dir(&mut rng, 100.0) else { return Err("a massed Crawl site never erupted".into()) };
    if sphere::arc_dist(g, Vec3::Y, 100.0) > CRAWL_SPREAD + 1e-3 {
        return Err("a ghost erupted away from its Crawl site".into());
    }
    if (crawl.sites[0].secs_left() - CRAWL_ERUPT_SECS).abs() > 0.2 {
        return Err("a Crawl site does not erupt for CRAWL_ERUPT_SECS".into());
    }
    crawl.tick(CRAWL_ERUPT_SECS + 0.1);
    if !crawl.sites.is_empty() {
        return Err("a spent Crawl site never closed".into());
    }
    let wire = Crawl { sites: vec![CrawlSite { dir: Vec3::X, mass: 0.4 }], next: 0 }.to_wire();
    let mut back = Crawl::default();
    back.adopt(&wire);
    if back.sites != vec![CrawlSite { dir: Vec3::X, mass: 0.4 }] {
        return Err("Crawl sites do not survive the wire".into());
    }
    let flora = WorldFlora {
        style: Some(FloraStyle::Thorns),
        plants: vec![Plant { dir: Vec3::Y, scale: 1.0, entity: Entity::PLACEHOLDER }],
    };
    if !flora.thorn_contact(Vec3::Y, 0.0, PLAYER_RADIUS, 100.0) || flora.thorn_contact(Vec3::Y, THORN_HEIGHT + 0.1, PLAYER_RADIUS, 100.0) {
        return Err("a thorn bush does not snag below its height only".into());
    }
    let away = sphere::offset_dir(Vec3::Y, Vec3::X, THORN_REACH + PLAYER_RADIUS + 0.2, 100.0);
    if flora.thorn_contact(away, 0.0, PLAYER_RADIUS, 100.0) {
        return Err("a thorn bush snags from out of reach".into());
    }
    let shrooms = WorldFlora { style: Some(FloraStyle::GlowShrooms), plants: flora.plants.clone() };
    if shrooms.thorn_contact(Vec3::Y, 0.0, PLAYER_RADIUS, 100.0) {
        return Err("fungus snags like thorns".into());
    }
    Ok(())
}
