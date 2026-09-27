//! World interactables: pots, chests, the Shady Guy, shrines, the cage,
//! the microwave, moai heads, and the stage teleporter.

use crate::config::*;
use crate::content::items::ItemKind;
use crate::content::Rarity;
use crate::fx::{self, Pcolor, ParticleAssets};
use crate::messages::*;
use crate::pickups::Pickup;
use crate::planet::{random_dir, CurrentPlanet, PropColliders, StageScoped};
use crate::player::Player;
use crate::run::{roll_item, ChoicePanel, PlayerState, RunPhase, RunState, UpgradeOption};
use crate::save::MetaSave;
use crate::sphere;
use crate::techs::GrindLines;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

#[derive(Component)]
pub struct Pot {
    pub broken: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum InteractKind {
    Chest,
    ShadyGuy,
    GreedShrine,
    MagnetShrine,
    Moai,
    Microwave,
    Cage,
    Teleporter,
    /// The guaranteed cache miniboss #1 drops (§3 run arc): free, pick one of three.
    RewardChest,
}

#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
    pub used: bool,
    /// Chests remember their rolled item (and its grade); shady guys their stock.
    pub chest_item: Option<(ItemKind, Rarity)>,
    pub stock: Vec<(ItemKind, Rarity, u64, bool)>, // item, grade, price, sold
}

#[derive(Component)]
pub struct ChargeShrine {
    pub progress: f32,
    pub done: bool,
}

#[derive(Component)]
pub struct ShrineRing;

/// "Press E" prompt contents for the HUD.
#[derive(Resource, Default)]
pub struct InteractPrompt(pub Option<String>);

/// Chest reveal modal.
#[derive(Resource, Default)]
pub struct ChestPanel {
    pub open: bool,
    pub item: Option<(ItemKind, Rarity)>,
    pub cost: u64,
    pub chest: Option<Entity>,
}

/// Shady Guy shop modal.
#[derive(Resource, Default)]
pub struct ShopPanel {
    pub open: bool,
    pub vendor: Option<Entity>,
    pub offers: Vec<(ItemKind, Rarity, u64, bool)>,
}

pub struct InteractDefs;
impl InteractDefs {
    pub fn color(kind: InteractKind) -> Color {
        match kind {
            InteractKind::Chest => Color::srgb(0.85, 0.6, 0.2),
            InteractKind::ShadyGuy => Color::srgb(0.4, 0.35, 0.5),
            InteractKind::GreedShrine => Color::srgb(1.0, 0.75, 0.1),
            InteractKind::MagnetShrine => Color::srgb(0.3, 0.7, 1.0),
            InteractKind::Moai => Color::srgb(0.6, 0.62, 0.66),
            InteractKind::Microwave => Color::srgb(0.9, 0.9, 0.95),
            InteractKind::Cage => Color::srgb(0.5, 0.4, 0.3),
            InteractKind::Teleporter => Color::srgb(0.3, 1.0, 0.8),
            InteractKind::RewardChest => Color::srgb(1.0, 0.78, 0.2),
        }
    }
}

/// The miniboss cache's hand: `REWARD_CACHE_CHOICES` DISTINCT items rolled at bonus luck.
/// A fork offering the same item twice is not a fork.
pub fn reward_cache_options(ps: &PlayerState, rng: &mut impl Rng) -> Vec<UpgradeOption> {
    let mut picked: Vec<(ItemKind, Rarity)> = Vec::new();
    for _ in 0..REWARD_CACHE_CHOICES * 8 {
        if picked.len() >= REWARD_CACHE_CHOICES {
            break;
        }
        let (item, grade) = roll_item(ps, ps.stats.luck + REWARD_CACHE_LUCK, rng);
        if !picked.iter().any(|(i, _)| *i == item) {
            picked.push((item, grade));
        }
    }
    picked.into_iter().map(|(i, g)| UpgradeOption::item(i, g, ps)).collect()
}

/// A shrine/Moai hand: `n` rolled items, one card per item (two grades of one item is not a
/// choice).
fn item_hand(ps: &PlayerState, luck: f32, n: usize, rng: &mut impl Rng) -> Vec<UpgradeOption> {
    let mut picked: Vec<(ItemKind, Rarity)> = Vec::new();
    for _ in 0..n * 8 {
        if picked.len() >= n {
            break;
        }
        let (item, grade) = roll_item(ps, luck, rng);
        if !picked.iter().any(|(i, _)| *i == item) {
            picked.push((item, grade));
        }
    }
    picked.into_iter().map(|(i, g)| UpgradeOption::item(i, g, ps)).collect()
}

/// Marks the one miniboss cache entity so `sync_reward_cache` can find it; remembers the
/// stage it dropped on.
#[derive(Component)]
pub struct RewardCache {
    pub stage: usize,
}

/// Keep the miniboss cache entity in step with `RunState::reward_chest`, on host AND client:
/// the host sets/clears the field (miniboss #1 dies / cache opened) and a client adopts it
/// from `RunSnapMsg`, so this one reconcile both spawns the chest where the corpse fell and
/// pops it when anyone opens it — no separate wire event to lose.
#[allow(clippy::too_many_arguments)]
pub fn sync_reward_cache(
    mut commands: Commands,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<(Entity, &Transform, &RewardCache)>,
    particles: Option<Res<ParticleAssets>>,
    role: Option<Res<crate::net::NetRole>>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let existing = q.iter().next();
    match (run.reward_chest, existing) {
        (Some(dir), None) => {
            spawn_reward_cache(&mut commands, &mut meshes, &mut materials, &planet, dir, run.stage);
            info!("REWARD miniboss cache up at {:.2?}", dir);
            // Only the host's astronaut can open it until peers get interact requests (P14),
            // so a client's banner must not promise a pick it can't take.
            let client = role.is_some_and(|r| matches!(*r, crate::net::NetRole::Client));
            let banner = if client { "MINIBOSS CACHE DROPPED" } else { "MINIBOSS CACHE DROPPED: FREE PICK" };
            banners.write(BannerMsg(banner.into()));
        }
        (None, Some((e, tf, cache))) => {
            // A client learns of a stage change in the same snapshot that clears the field;
            // then the stage sweep takes the chest, and a gold burst would celebrate nothing.
            let opened = cache.stage == run.stage;
            if let (true, Some(pa)) = (opened, &particles) {
                fx::burst(&mut commands, pa, tf.translation, tf.translation.normalize_or_zero(), Pcolor::Gold, 28, 9.0);
            }
            // try_: the stage sweep (StageScoped) can take it in the same frame
            commands.entity(e).try_despawn();
            if opened {
                info!("REWARD miniboss cache opened");
            }
        }
        _ => {}
    }
}

/// A gold chest on a pedestal under a tall light column — the column is what makes it
/// readable from over the horizon, where it usually lands after a running miniboss fight.
fn spawn_reward_cache(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    dir: Vec3,
    stage: usize,
) {
    let c = InteractDefs::color(InteractKind::RewardChest);
    let gold = materials.add(StandardMaterial {
        base_color: c,
        emissive: c.to_linear() * 2.2,
        metallic: 0.6,
        perceptual_roughness: 0.35,
        ..default()
    });
    let base_mat = materials.add(StandardMaterial {
        base_color: c.darker(0.35),
        perceptual_roughness: 0.9,
        ..default()
    });
    let beam = materials.add(StandardMaterial {
        base_color: c.with_alpha(0.35),
        emissive: c.to_linear() * 2.5,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let pos = planet.surface_point(dir);
    let rot = sphere::frame_quat(dir, sphere::tangent_frame(dir).0);
    commands
        .spawn((
            Interactable { kind: InteractKind::RewardChest, used: false, chest_item: None, stock: Vec::new() },
            RewardCache { stage },
            Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.95, 0.5)))),
            MeshMaterial3d(base_mat),
            Transform::from_translation(pos + dir * 0.25).with_rotation(rot),
            StageScoped,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.3, 0.8, 0.85)))),
                MeshMaterial3d(gold.clone()),
                Transform::from_xyz(0.0, 0.8, 0.0),
            ));
            // domed lid, slightly proud of the box so the silhouette reads "chest"
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.43, 1.34)))),
                MeshMaterial3d(gold),
                Transform::from_xyz(0.0, 1.2, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
            ));
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.35, 40.0)))),
                MeshMaterial3d(beam),
                Transform::from_xyz(0.0, 20.0, 0.0),
            ));
        });
}

/// Shady Guy price by the grade the item was ROLLED at — a Legendary-grade Borgar costs
/// what a Legendary does.
fn price(grade: Rarity, discount: f32) -> u64 {
    let base = match grade {
        Rarity::Common => 30.0,
        Rarity::Rare => 60.0,
        Rarity::Epic => 120.0,
        Rarity::Legendary => 240.0,
        Rarity::Cursed => CURSED_ITEM_PRICE,
    };
    (base * (1.0 - discount)).round().max(1.0) as u64
}

/// Where interactables may not stand: the Grind-Lines — a chest or a shrine on a rail would
/// be ridden straight through — and the solid props (L7: one inside a big boulder could be
/// neither reached nor jumped onto). Both are the terrain's and the stage seed's, the same on
/// every machine, so the retries they cost keep the layout stream machine-independent.
pub struct Keepout<'a> {
    pub rails: &'a GrindLines,
    pub props: &'a PropColliders,
}

impl Keepout<'_> {
    fn clear(&self, d: Vec3, planet: &CurrentPlanet) -> bool {
        self.rails.closest(d).is_none_or(|(arc, ..)| arc > GRIND_INTERACT_CLEARANCE)
            && self.props.0.iter().all(|c| {
                // compare cosines: no acos per prop per try
                let reach = (c.radius + INTERACT_PROP_CLEARANCE) / planet.radius;
                reach >= std::f32::consts::PI || d.dot(c.dir) < reach.cos()
            })
    }
}

/// A random direction at least `min_arc` meters (great-circle) from `avoid` and clear of
/// the `Keepout`.
fn place_dir(rng: &mut impl Rng, planet: &CurrentPlanet, keep: &Keepout, avoid: Vec3, min_arc: f32) -> Vec3 {
    for _ in 0..40 {
        let d = random_dir(rng);
        if sphere::arc_dist(d, avoid, planet.radius) > min_arc && keep.clear(d, planet) {
            return d;
        }
    }
    random_dir(rng)
}

/// Scatter all interactables for the current stage.
/// Separates the vendor-stock stream from the layout stream of the same stage seed (M1).
const SHOP_STOCK_SEED_SALT: u64 = 0x5709_C4A5_E5EE_D001;

pub fn spawn_interactables(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    run: &RunState,
    ps: &PlayerState,
    save: &MetaSave,
    rails: &GrindLines,
    props: &PropColliders,
    player_dir: Vec3,
) {
    let def = planet.kind.def();
    let keep = Keepout { rails, props };
    // deterministic interactable layout from the run seed — every draw of it the same on
    // every machine (CLAUDE.md rule 5)
    let layout_seed = run.run_seed.wrapping_add(run.stage as u64).wrapping_mul(0x9e37);
    let mut rng = StdRng::seed_from_u64(layout_seed);
    // Vendor stock gets its OWN stream (M1). It is rolled from a sheet (luck, items owned at
    // max, bans) that differs per machine and per carried build, and `roll_item` consumes a
    // sheet-dependent number of draws — on the layout stream that shifted every shrine, the
    // Moai, the microwave, the cage slot and all five charge rings between host and joiner.
    let mut stock_rng = StdRng::seed_from_u64(layout_seed ^ SHOP_STOCK_SEED_SALT);

    // Pots
    let pot_mesh = meshes.add(Mesh::from(Cylinder::new(0.32, 0.55)));
    let pot_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.55, 0.35),
        perceptual_roughness: 0.9,
        ..default()
    });
    let silver_pot_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.85, 1.0),
        emissive: LinearRgba::rgb(0.4, 0.6, 1.2),
        ..default()
    });
    for _ in 0..def.pots {
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 8.0);
        let silverish = rng.gen_bool(0.1);
        commands.spawn((
            Pot { broken: false },
            crate::enemies::Enemy {
                // Pots piggyback on the enemy hash for weapon collisions? No —
                // they are their own query; this marker keeps them hittable
                // by projectiles via the hash.
                kind: crate::content::enemies::EnemyKind::Shambler,
                dir,
                hover: 0.0,
                speed: 0.0,
                damage: 0.0,
                xp: 0.0,
                hp: 1.0,
                max_hp: 1.0,
                elite: false,
                contact_cd: f32::INFINITY,
                slow: 0.0,
                knock: Vec3::ZERO,
                flash: 0.0,
                scale: 1.0,
                wobble: 0.0,
                stride: 0.0,
            },
            Mesh3d(pot_mesh.clone()),
            MeshMaterial3d(if silverish { silver_pot_mat.clone() } else { pot_mat.clone() }),
            Transform::from_translation(planet.surface_point(dir) + dir * 0.3)
                .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0)),
            StageScoped,
        ));
    }

    let pedestal = meshes.add(Mesh::from(Cylinder::new(0.8, 0.5)));
    let icon = meshes.add(Mesh::from(Sphere::new(0.4)));

    let mut spawn_simple = |commands: &mut Commands, kind: InteractKind, dir: Vec3, extra_stock: Vec<(ItemKind, Rarity, u64, bool)>, chest_item: Option<(ItemKind, Rarity)>| -> Entity {
        let c = InteractDefs::color(kind);
        let mat = materials.add(StandardMaterial {
            base_color: c,
            emissive: c.to_linear() * 1.4,
            perceptual_roughness: 0.5,
            ..default()
        });
        let base_mat = materials.add(StandardMaterial {
            base_color: c.darker(0.15),
            perceptual_roughness: 0.9,
            ..default()
        });
        let pos = planet.surface_point(dir);
        let rot = sphere::frame_quat(dir, sphere::tangent_frame(dir).0);
        commands
            .spawn((
                Interactable { kind, used: false, chest_item, stock: extra_stock },
                Mesh3d(pedestal.clone()),
                MeshMaterial3d(base_mat),
                Transform::from_translation(pos + dir * 0.25).with_rotation(rot),
                StageScoped,
            ))
            .with_children(|p| {
                let shape = match kind {
                    InteractKind::Chest => Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.0, 0.7, 0.7)))),
                    InteractKind::ShadyGuy => Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.4, 0.8)))),
                    InteractKind::Cage => Mesh3d(meshes.add(Mesh::from(Cuboid::new(1.2, 1.4, 1.2)))),
                    InteractKind::Microwave => Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.9, 0.6, 0.6)))),
                    InteractKind::Moai => Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.7, 1.6, 0.6)))),
                    InteractKind::Teleporter => Mesh3d(meshes.add(Mesh::from(Torus::new(1.2, 1.5)))),
                    _ => Mesh3d(icon.clone()),
                };
                p.spawn((shape, MeshMaterial3d(mat), Transform::from_xyz(0.0, 1.0, 0.0)));
            })
            .id()
    };

    // Chests — some of them Mimics (§9). The roll is drawn for every chest from the layout
    // stream, so both machines agree which ones bite; its first breath is hashed off where
    // it stands, so the tell starts out of step from chest to chest.
    for _ in 0..7 {
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 12.0);
        let mimic = rng.gen_bool(MIMIC_CHEST_CHANCE);
        let chest = spawn_simple(commands, InteractKind::Chest, dir, Vec::new(), None);
        if mimic {
            let first = MIMIC_TELL_SECS.0 + (dir.x * 43.7 + dir.y * 9.1).fract().abs() * (MIMIC_TELL_SECS.1 - MIMIC_TELL_SECS.0);
            commands.entity(chest).insert(crate::bestiary::MimicDisguise { tell: first, breath: 0.0 });
        }
    }
    // Shady guys with pre-rolled stock (fixed at stage entry, luck applies now)
    for _ in 0..2 {
        let mut stock = Vec::new();
        for _ in 0..3 {
            let (item, grade) = roll_item(ps, ps.stats.luck, &mut stock_rng);
            stock.push((item, grade, price(grade, ps.stats.chest_discount), false));
        }
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 15.0);
        spawn_simple(commands, InteractKind::ShadyGuy, dir, stock, None);
    }
    // Shrines
    for _ in 0..2 {
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 15.0);
        spawn_simple(commands, InteractKind::GreedShrine, dir, Vec::new(), None);
    }
    for _ in 0..2 {
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 15.0);
        spawn_simple(commands, InteractKind::MagnetShrine, dir, Vec::new(), None);
    }
    let dir = place_dir(&mut rng, planet, &keep, player_dir, 20.0);
    spawn_simple(commands, InteractKind::Moai, dir, Vec::new(), None);
    let dir = place_dir(&mut rng, planet, &keep, player_dir, 20.0);
    spawn_simple(commands, InteractKind::Microwave, dir, Vec::new(), None);
    // The draw happens UNCONDITIONALLY even though the cage itself is conditional.
    // `place_dir` consumes a variable number of rng draws (it retries up to 40 times), so
    // skipping it shifts every later draw — which moved all five charge shrines. Two players
    // whose saves disagree about `chimp_freed` would then stand in DIFFERENT rings, and
    // "converge on the shrine together" silently cannot work. The cage stays per-machine
    // (it is a per-machine unlock); only the rng stream is made machine-independent.
    let cage_dir = place_dir(&mut rng, planet, &keep, player_dir, 25.0);
    if planet.kind == crate::content::planets::PlanetKind::Moon && !save.counters.chimp_freed {
        spawn_simple(commands, InteractKind::Cage, cage_dir, Vec::new(), None);
    }

    // Charge shrines (stand in the ring). Bevy's Torus lies in its local XZ plane and
    // frame_quat maps local Y to the surface normal, so the ring lies flat on the ground and
    // outlines the 4.2 m charge zone. (It used to take an extra quarter-turn about X, which
    // stood it on its edge like an arch — the zone you stand in was never drawn.)
    let ring_mesh = meshes.add(Mesh::from(Torus::new(3.6, 3.9)));
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 1.0, 0.9),
        emissive: LinearRgba::rgb(0.4, 1.6, 1.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for _ in 0..5 {
        let dir = place_dir(&mut rng, planet, &keep, player_dir, 18.0);
        let pos = planet.surface_point(dir);
        commands.spawn((
            ChargeShrine { progress: 0.0, done: false },
            ShrineRing,
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_translation(pos + dir * 0.2)
                .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0)),
            StageScoped,
        ));
    }
}

/// Marks the exit teleporter, so `sync_teleporter` can tell whether it stands.
#[derive(Component)]
pub struct Teleporter;

/// Where the exit teleporter opens once the boss falls: 18 m from the squad's centroid, at a
/// random bearing. HOST-rolled — the spot then rides `RunState::teleporter_dir`.
pub fn teleporter_spot(planet: &CurrentPlanet, anchor: Vec3, rng: &mut impl Rng) -> Vec3 {
    let (t, b) = sphere::tangent_frame(anchor);
    let a = rng.gen_range(0.0..std::f32::consts::TAU);
    sphere::offset_dir(anchor, t * a.cos() + b * a.sin(), 18.0, planet.radius)
}

/// Keep the exit teleporter in step with `RunState::teleporter_dir`, on host AND client —
/// the `sync_reward_cache` pattern. It used to be spawned only inside the host's clock at a
/// `thread_rng` bearing, so a joiner never had one: no ring, no edge marker, no way home (M3).
#[allow(clippy::too_many_arguments)]
pub fn sync_teleporter(
    mut commands: Commands,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<Entity, With<Teleporter>>,
    role: Option<Res<crate::net::NetRole>>,
    mut banners: MessageWriter<BannerMsg>,
) {
    match (run.teleporter_dir, q.iter().next()) {
        (Some(dir), None) => {
            spawn_teleporter(&mut commands, &mut meshes, &mut materials, &planet, dir);
            // the host's clock announced it; a joiner hears it here
            if role.is_some_and(|r| matches!(*r, crate::net::NetRole::Client)) {
                banners.write(BannerMsg(crate::director::TELEPORTER_BANNER.into()));
            }
        }
        // a client learns of a stage change in the snapshot that clears the field; the
        // stage sweep may take it this same frame
        (None, Some(e)) => {
            commands.entity(e).try_despawn();
        }
        _ => {}
    }
}

/// The exit teleporter at `dir`: a ring on a pedestal under a planet-high column.
fn spawn_teleporter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    dir: Vec3,
) {
    let c = InteractDefs::color(InteractKind::Teleporter);
    let mat = materials.add(StandardMaterial {
        base_color: c,
        emissive: c.to_linear() * 3.0,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let pos = planet.surface_point(dir);
    let rot = sphere::frame_quat(dir, sphere::tangent_frame(dir).0);
    commands
        .spawn((
            Interactable { kind: InteractKind::Teleporter, used: false, chest_item: None, stock: Vec::new() },
            Teleporter,
            Mesh3d(meshes.add(Mesh::from(Torus::new(1.4, 1.7)))),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos + dir * 2.0).with_rotation(rot),
            StageScoped,
        ))
        .with_children(|p| {
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cylinder::new(0.5, 60.0)))),
                MeshMaterial3d(mat),
                Transform::from_xyz(0.0, 20.0, 0.0),
            ));
        });
}

/// Charge shrines fill while the player stands inside; done => loot choice.
pub fn charge_shrines(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    q_ps: Query<&PlayerState, With<crate::player::LocalPlayer>>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    save: Res<MetaSave>,
    q_player: Query<(&Transform, &PlayerState), With<Player>>,
    mut q: Query<(&mut ChargeShrine, &mut Transform), Without<Player>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // only astronauts standing charge a ring: not a Beacon rolling through it (L19)
    let ppos: Vec<Vec3> = q_player.iter().filter(|(_, ps)| !ps.dead).map(|(t, _)| t.translation).collect();
    let mut rng = rand::thread_rng();
    for (mut s, mut tf) in &mut q {
        if s.done {
            continue;
        }
        // Charging scales with how many astronauts are standing in it — a stand-still
        // objective is exactly where co-op should reward converging.
        let inside_n = ppos.iter().filter(|p| tf.translation.distance(**p) < 4.2).count();
        if inside_n > 0 {
            s.progress += dt / 8.0 * inside_n as f32;
        } else {
            s.progress = (s.progress - dt / 16.0).max(0.0);
        }
        let pulse = 1.0 + s.progress * 0.35;
        tf.scale = Vec3::splat(pulse);
        // In co-op the rings keep charging behind the host's panel (`crate::world_live`);
        // a full one waits there for its blessing panel until the host's current one closes.
        // (Whose blessing it is — the chargers', not the host's — is P14's peer loot.)
        if s.progress >= 1.0 && *phase != RunPhase::Playing {
            s.progress = 1.0;
            continue;
        }
        if s.progress >= 1.0 {
            s.done = true;
            run.shrines_charged += 1;
            sfx.write(SfxMsg(Sfx::Shrine));
            banners.write(BannerMsg("SHRINE CHARGED".into()));
            let Ok(ps) = q_ps.single() else { continue };
            let opts = item_hand(ps, ps.stats.luck + 0.3, 3, &mut rng);
            *panel = ChoicePanel { title: "SHRINE BLESSING".into(), options: opts, banishing: false, is_levelup: false };
            *phase = RunPhase::Modal;
            let _ = &save;
        }
    }
}

/// Nearest usable interactable within range -> prompt + E to use.
#[allow(clippy::too_many_arguments)]
pub fn interact_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut prompt: ResMut<InteractPrompt>,
    mut run: ResMut<RunState>,
    mut q_ps: Query<&mut PlayerState, With<crate::player::LocalPlayer>>,
    mut save: ResMut<MetaSave>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    mut panels: (ResMut<ChestPanel>, ResMut<ShopPanel>),
    mut pending: ResMut<crate::director::PendingStage>,
    q_player: Query<(Entity, &Transform), (With<Player>, With<crate::player::LocalPlayer>)>,
    mut q: Query<(Entity, &mut Interactable, &Transform, Has<crate::bestiary::MimicDisguise>), Without<Player>>,
    mut pickups: Query<&mut Pickup>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
    mut commands: Commands,
    particles: Option<Res<ParticleAssets>>,
) {
    if *phase != RunPhase::Playing {
        prompt.0 = None;
        return;
    }
    let Ok((actor_entity, ptf)) = q_player.single() else { return };
    let Ok(mut ps) = q_ps.single_mut() else { return };
    // a Beacon can't open, buy or teleport (§11: the downed wait for a teammate)
    if ps.dead {
        prompt.0 = None;
        return;
    }
    let mut rng = rand::thread_rng();

    let mut nearest: Option<(Entity, f32)> = None;
    for (e, i, tf, _) in q.iter() {
        if i.used {
            continue;
        }
        let d = tf.translation.distance(ptf.translation);
        if d < INTERACT_RANGE + 1.5 && nearest.map(|(_, bd)| d < bd).unwrap_or(true) {
            nearest = Some((e, d));
        }
    }

    let Some((entity, _)) = nearest else {
        prompt.0 = None;
        return;
    };
    let Ok((_, mut inter, tf, is_mimic)) = q.get_mut(entity) else {
        prompt.0 = None;
        return;
    };

    let cost_now = ((CHEST_BASE_COST as f32) * CHEST_COST_GROWTH.powi(run.chest_opens as i32)
        * (1.0 - ps.stats.chest_discount))
        .round() as u64;

    prompt.0 = Some(match inter.kind {
        InteractKind::Chest => format!("[E] Open chest ({cost_now} gold)"),
        InteractKind::ShadyGuy => "[E] Talk to the Shady Guy".into(),
        InteractKind::GreedShrine => "[E] Greed Shrine (+difficulty, +luck)".into(),
        InteractKind::MagnetShrine => "[E] Magnet Shrine (vacuum the planet)".into(),
        InteractKind::Moai => "[E] Consult the Moai".into(),
        InteractKind::Microwave => {
            if ps.free_microwave > 0 {
                "[E] Microwave (duplicate an item: FREE, Tome of Duplication)".into()
            } else if !run.microwave_used {
                "[E] Microwave (duplicate an item)".into()
            } else {
                "The microwave hums, spent".into()
            }
        }
        InteractKind::Cage => "[E] Open the cage".into(),
        InteractKind::Teleporter => "[E] TELEPORT OUT".into(),
        InteractKind::RewardChest => "[E] Open the miniboss cache (free)".into(),
    });

    if !keys.just_pressed(KeyCode::KeyE) {
        return;
    }

    match inter.kind {
        InteractKind::Chest => {
            if ps.gold < cost_now {
                banners.write(BannerMsg("NOT ENOUGH GOLD".into()));
                return;
            }
            if is_mimic {
                // Greed, punished (§9): trying the lid pays the price into its mouth. It
                // springs (`bestiary::mimic_spring`); kill it and the payer gets it back.
                ps.gold -= cost_now;
                inter.used = true;
                commands.entity(entity).insert(crate::bestiary::MimicSprung { payer: actor_entity, paid: cost_now });
                return;
            }
            let item = *inter.chest_item.get_or_insert_with(|| roll_item(&ps, ps.stats.luck, &mut rng));
            // A chest that rolled an item it can no longer deal (capped since, or banished)
            // rolls again rather than offering a dead card.
            let item = if crate::run::item_available(&ps, item.0) {
                item
            } else {
                let fresh = roll_item(&ps, ps.stats.luck, &mut rng);
                inter.chest_item = Some(fresh);
                fresh
            };
            *panels.0 = ChestPanel { open: true, item: Some(item), cost: cost_now, chest: Some(entity) };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Chest));
        }
        InteractKind::ShadyGuy => {
            *panels.1 = ShopPanel { open: true, vendor: Some(entity), offers: inter.stock.clone() };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Click));
        }
        InteractKind::GreedShrine => {
            inter.used = true;
            run.greed_stacks += 1;
            let save_clone = save.clone();
            ps.recompute_stats(&save_clone, run.greed_stacks);
            run.difficulty = ps.stats.difficulty;
            banners.write(BannerMsg("GREED: +12% DIFFICULTY, +8% LUCK".into()));
            sfx.write(SfxMsg(Sfx::Shrine));
        }
        InteractKind::MagnetShrine => {
            inter.used = true;
            for mut p in pickups.iter_mut() {
                p.flying = true;
                p.target = Some(actor_entity);
                p.speed = 14.0;
            }
            banners.write(BannerMsg("THE PLANET GIVES".into()));
            sfx.write(SfxMsg(Sfx::Shrine));
        }
        InteractKind::Moai => {
            inter.used = true;
            let opts = item_hand(&ps, ps.stats.luck + 0.15, 3, &mut rng);
            *panel = ChoicePanel { title: "THE MOAI SPEAKS".into(), options: opts, banishing: false, is_levelup: false };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Shrine));
        }
        InteractKind::Microwave => {
            // The stage's own use, or a free one from Tome of Duplication (§7 "one free use
            // per stage"). The free one goes first: P16 puts a Gold price (Salvage-discounted
            // like chests) on the stage's own, and a free use is the one worth spending.
            let free = ps.free_microwave > 0;
            if (run.microwave_used && !free) || ps.items.is_empty() {
                return;
            }
            // §7: a duplicate comes out one grade below the best copy held (never under the
            // item's native grade) — unless Tome of Duplication's later ranks keep it whole.
            // P16 adds the gamble and the Gold price.
            let keep = ps.stats.dupe_keep_grade.clamp(0.0, 1.0) as f64;
            let mut owned: Vec<(ItemKind, Rarity)> = ps
                .items
                .iter()
                .filter(|s| crate::run::item_available(&ps, s.kind))
                .map(|s| {
                    let steps = u32::from(!rng.gen_bool(keep));
                    (s.kind, s.best().step_down(steps, s.kind.def().rarity))
                })
                .collect();
            owned.shuffle(&mut rng);
            let opts: Vec<UpgradeOption> =
                owned.into_iter().take(3).map(|(k, g)| UpgradeOption::ItemUp(k, g)).collect();
            if opts.is_empty() {
                // nothing to put in: the use is not spent
                banners.write(BannerMsg("NOTHING FITS IN THE MICROWAVE".into()));
                return;
            }
            if free {
                ps.free_microwave -= 1;
            } else {
                run.microwave_used = true;
            }
            *panel = ChoicePanel { title: "MICROWAVE: DUPLICATE".into(), options: opts, banishing: false, is_levelup: false };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Click));
        }
        InteractKind::Cage => {
            inter.used = true;
            save.counters.chimp_freed = true;
            banners.write(BannerMsg("THE CAGE IS OPEN. HE REMEMBERS.".into()));
            sfx.write(SfxMsg(Sfx::LevelUp));
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, tf.translation, tf.translation.normalize_or_zero(), Pcolor::Gold, 24, 8.0);
            }
        }
        InteractKind::Teleporter => {
            inter.used = true;
            sfx.write(SfxMsg(Sfx::Teleport));
            pending.0 = Some(run.stage + 1);
        }
        InteractKind::RewardChest => {
            // Clearing the field is what despawns the chest (sync_reward_cache), here and
            // on every client.
            inter.used = true;
            run.reward_chest = None;
            run.chests_opened += 1;
            let options = reward_cache_options(&ps, &mut rng);
            *panel = ChoicePanel { title: "MINIBOSS CACHE".into(), options, banishing: false, is_levelup: false };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Chest));
        }
    }
}
