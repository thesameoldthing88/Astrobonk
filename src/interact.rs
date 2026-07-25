//! World interactables: pots, chests, the Shady Guy, shrines, the cage,
//! the microwave, moai heads, and the stage teleporter.

use crate::config::*;
use crate::content::items::ItemKind;
use crate::content::Rarity;
use crate::fx::{self, Pcolor, ParticleAssets};
use crate::messages::*;
use crate::pickups::Pickup;
use crate::planet::{random_dir, CurrentPlanet, StageScoped};
use crate::player::Player;
use crate::run::{ChoicePanel, PlayerState, RunPhase, RunState, UpgradeOption};
use crate::save::MetaSave;
use crate::sphere;
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
}

#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
    pub used: bool,
    /// Chests remember their rolled item; shady guys their stock.
    pub chest_item: Option<ItemKind>,
    pub stock: Vec<(ItemKind, u64, bool)>, // item, price, sold
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
    pub item: Option<ItemKind>,
    pub cost: u64,
    pub chest: Option<Entity>,
}

/// Shady Guy shop modal.
#[derive(Resource, Default)]
pub struct ShopPanel {
    pub open: bool,
    pub vendor: Option<Entity>,
    pub offers: Vec<(ItemKind, u64, bool)>,
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
        }
    }
}

fn roll_item(run: &PlayerState, luck: f32, rng: &mut impl Rng) -> ItemKind {
    let rarity = Rarity::roll(luck, rng);
    let candidates: Vec<ItemKind> = ItemKind::ALL
        .iter()
        .copied()
        .filter(|i| !run.banned_items.contains(i))
        .filter(|i| run.item_count(*i) < i.def().max_stacks)
        .filter(|i| i.def().rarity == rarity)
        .collect();
    if let Some(i) = candidates.choose(rng) {
        return *i;
    }
    // fall back to anything available
    let any: Vec<ItemKind> = ItemKind::ALL
        .iter()
        .copied()
        .filter(|i| run.item_count(*i) < i.def().max_stacks)
        .collect();
    *any.choose(rng).unwrap_or(&ItemKind::SpaceBorgar)
}

fn price(rarity: Rarity, discount: f32) -> u64 {
    let base = match rarity {
        Rarity::Common => 30.0,
        Rarity::Rare => 60.0,
        Rarity::Epic => 120.0,
        Rarity::Legendary => 240.0,
    };
    (base * (1.0 - discount)).round().max(1.0) as u64
}

/// A random direction at least `min_arc` meters (great-circle) from `avoid`.
fn place_dir(rng: &mut impl Rng, planet: &CurrentPlanet, avoid: Vec3, min_arc: f32) -> Vec3 {
    for _ in 0..40 {
        let d = random_dir(rng);
        if sphere::arc_dist(d, avoid, planet.radius) > min_arc {
            return d;
        }
    }
    random_dir(rng)
}

/// Scatter all interactables for the current stage.
pub fn spawn_interactables(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    run: &RunState,
    ps: &PlayerState,
    save: &MetaSave,
    player_dir: Vec3,
) {
    let def = planet.kind.def();
    // deterministic interactable layout + vendor stock from the run seed
    let mut rng = StdRng::seed_from_u64(run.run_seed.wrapping_add(run.stage as u64).wrapping_mul(0x9e37));

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
        let dir = place_dir(&mut rng, planet, player_dir, 8.0);
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

    let mut spawn_simple = |commands: &mut Commands, kind: InteractKind, dir: Vec3, extra_stock: Vec<(ItemKind, u64, bool)>, chest_item: Option<ItemKind>| {
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
            });
    };

    // Chests
    for _ in 0..7 {
        let dir = place_dir(&mut rng, planet, player_dir, 12.0);
        spawn_simple(commands, InteractKind::Chest, dir, Vec::new(), None);
    }
    // Shady guys with pre-rolled stock (fixed at stage entry, luck applies now)
    for _ in 0..2 {
        let mut stock = Vec::new();
        for _ in 0..3 {
            let item = roll_item(ps, ps.stats.luck, &mut rng);
            stock.push((item, price(item.def().rarity, ps.stats.chest_discount), false));
        }
        let dir = place_dir(&mut rng, planet, player_dir, 15.0);
        spawn_simple(commands, InteractKind::ShadyGuy, dir, stock, None);
    }
    // Shrines
    for _ in 0..2 {
        let dir = place_dir(&mut rng, planet, player_dir, 15.0);
        spawn_simple(commands, InteractKind::GreedShrine, dir, Vec::new(), None);
    }
    for _ in 0..2 {
        let dir = place_dir(&mut rng, planet, player_dir, 15.0);
        spawn_simple(commands, InteractKind::MagnetShrine, dir, Vec::new(), None);
    }
    let dir = place_dir(&mut rng, planet, player_dir, 20.0);
    spawn_simple(commands, InteractKind::Moai, dir, Vec::new(), None);
    let dir = place_dir(&mut rng, planet, player_dir, 20.0);
    spawn_simple(commands, InteractKind::Microwave, dir, Vec::new(), None);
    if planet.kind == crate::content::planets::PlanetKind::Moon && !save.counters.chimp_freed {
        let dir = place_dir(&mut rng, planet, player_dir, 25.0);
        spawn_simple(commands, InteractKind::Cage, dir, Vec::new(), None);
    }

    // Charge shrines (stand in the ring)
    let ring_mesh = meshes.add(Mesh::from(Torus::new(3.6, 3.9)));
    let ring_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 1.0, 0.9),
        emissive: LinearRgba::rgb(0.4, 1.6, 1.4),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    for _ in 0..5 {
        let dir = place_dir(&mut rng, planet, player_dir, 18.0);
        let pos = planet.surface_point(dir);
        commands.spawn((
            ChargeShrine { progress: 0.0, done: false },
            ShrineRing,
            Mesh3d(ring_mesh.clone()),
            MeshMaterial3d(ring_mat.clone()),
            Transform::from_translation(pos + dir * 0.2)
                .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            StageScoped,
        ));
    }
}

/// Spawn the exit teleporter (after the boss falls).
pub fn spawn_teleporter(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    player_dir: Vec3,
) {
    let mut rng = rand::thread_rng();
    let (t, b) = sphere::tangent_frame(player_dir);
    let a = rng.gen_range(0.0..std::f32::consts::TAU);
    let dir = sphere::offset_dir(player_dir, t * a.cos() + b * a.sin(), 18.0, planet.radius);
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
    q_ps: Query<&PlayerState>,
    mut phase: ResMut<RunPhase>,
    mut panel: ResMut<ChoicePanel>,
    save: Res<MetaSave>,
    q_player: Query<&Transform, With<Player>>,
    mut q: Query<(&mut ChargeShrine, &mut Transform), Without<Player>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut banners: MessageWriter<BannerMsg>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 || *phase != RunPhase::Playing {
        return;
    }
    let ppos: Vec<Vec3> = q_player.iter().map(|t| t.translation).collect();
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
        if s.progress >= 1.0 {
            s.done = true;
            run.shrines_charged += 1;
            sfx.write(SfxMsg(Sfx::Shrine));
            banners.write(BannerMsg("SHRINE CHARGED".into()));
            let Ok(ps) = q_ps.single() else { continue };
            let mut opts = Vec::new();
            for _ in 0..3 {
                let item = roll_item(ps, ps.stats.luck + 0.3, &mut rng);
                opts.push(if ps.item_count(item) == 0 {
                    UpgradeOption::NewItem(item)
                } else {
                    UpgradeOption::ItemUp(item)
                });
            }
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
    mut q: Query<(Entity, &mut Interactable, &Transform), Without<Player>>,
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
    let mut rng = rand::thread_rng();

    let mut nearest: Option<(Entity, f32)> = None;
    for (e, i, tf) in q.iter() {
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
    let Ok((_, mut inter, tf)) = q.get_mut(entity) else {
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
            if run.microwave_used {
                "The microwave hums, spent".into()
            } else {
                "[E] Microwave (duplicate an item)".into()
            }
        }
        InteractKind::Cage => "[E] Open the cage".into(),
        InteractKind::Teleporter => "[E] TELEPORT OUT".into(),
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
            let item = *inter.chest_item.get_or_insert_with(|| roll_item(&ps, ps.stats.luck, &mut rng));
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
            let mut opts = Vec::new();
            for _ in 0..3 {
                let item = roll_item(&ps, ps.stats.luck + 0.15, &mut rng);
                opts.push(if ps.item_count(item) == 0 {
                    UpgradeOption::NewItem(item)
                } else {
                    UpgradeOption::ItemUp(item)
                });
            }
            *panel = ChoicePanel { title: "THE MOAI SPEAKS".into(), options: opts, banishing: false, is_levelup: false };
            *phase = RunPhase::Modal;
            sfx.write(SfxMsg(Sfx::Shrine));
        }
        InteractKind::Microwave => {
            if run.microwave_used || ps.items.is_empty() {
                return;
            }
            run.microwave_used = true;
            let mut owned: Vec<ItemKind> = ps
                .items
                .iter()
                .filter(|(k, c)| *c < k.def().max_stacks)
                .map(|(k, _)| *k)
                .collect();
            owned.shuffle(&mut rng);
            let opts: Vec<UpgradeOption> = owned.into_iter().take(3).map(UpgradeOption::ItemUp).collect();
            if opts.is_empty() {
                banners.write(BannerMsg("NOTHING FITS IN THE MICROWAVE".into()));
                return;
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
    }
}
