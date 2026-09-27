//! Planet + sky construction: icosphere terrain with analytic hills, scattered props,
//! starfield, sun, and (on the Moon) an Earthrise.

use crate::content::planets::{FloraStyle, PlanetDef, PlanetKind};
use crate::sphere::{self, Terrain};
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Everything spawned for a stage carries this for cleanup.
#[derive(Component)]
pub struct StageScoped;

/// A solid prop you can't walk through: a vertical cylinder standing on the surface,
/// stored as a surface direction + radius (meters) + how tall it blocks. Populated at
/// stage spawn; collision is resolved on the tangent plane (see `prop_collision`).
#[derive(Clone, Copy)]
pub struct PropCollider {
    pub dir: Vec3,
    pub radius: f32,
    pub height: f32,
}

/// All solid props on the current planet. Rebuilt per stage.
#[derive(Resource, Default)]
pub struct PropColliders(pub Vec<PropCollider>);

impl PropColliders {
    /// Push a point out of any prop it's inside. Returns the corrected position
    /// direction, or None if it wasn't colliding. Works on the sphere's tangent plane.
    pub fn resolve(&self, dir: Vec3, height: f32, radius: f32, planet_r: f32) -> Option<Vec3> {
        let mut out = dir;
        let mut hit = false;
        for c in &self.0 {
            if height > c.height {
                continue; // jumped over it
            }
            let arc = out.angle_between(c.dir) * planet_r;
            let min = c.radius + radius;
            if arc < min && arc > 1e-4 {
                // push straight away from the prop centre along the great circle
                let away = (out - c.dir * out.dot(c.dir)).normalize_or_zero();
                if away != Vec3::ZERO {
                    let axis = c.dir.cross(away).normalize_or_zero();
                    if axis != Vec3::ZERO {
                        out = (Quat::from_axis_angle(axis, min / planet_r) * c.dir).normalize();
                        hit = true;
                    }
                }
            }
        }
        if hit { Some(out) } else { None }
    }
}

/// The active planet parameters, sampled by every system that touches the ground.
#[derive(Resource, Clone, Copy)]
pub struct CurrentPlanet {
    pub kind: PlanetKind,
    pub radius: f32,
    pub terrain: Terrain,
}

impl CurrentPlanet {
    pub fn from_kind(kind: PlanetKind) -> Self {
        let d = kind.def();
        Self {
            kind,
            radius: d.radius,
            terrain: Terrain {
                seed: d.seed,
                amp: d.hill_amp,
                rugged: d.rugged,
                craters: d.craters,
                crater_depth: d.crater_depth,
                crater_width: 0.16,
            },
        }
    }
    /// Radial distance to the terrain surface along `dir`.
    pub fn surface(&self, dir: Vec3) -> f32 {
        self.terrain.surface(dir, self.radius)
    }
    pub fn surface_point(&self, dir: Vec3) -> Vec3 {
        dir * self.surface(dir)
    }
}

pub fn despawn_stage(mut commands: Commands, q: Query<Entity, With<StageScoped>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Build the icosphere terrain mesh with per-vertex displacement + biome vertex colors.
fn planet_mesh(def: &PlanetDef, terrain: &Terrain) -> Mesh {
    // subdiv 7 gives ~4× the terrain resolution of the old mesh — crisper mountains,
    // sharper crater rims. Collision stays analytic, so this is purely visual.
    let (mut verts, faces) = icosphere(7);
    let mut positions = Vec::with_capacity(verts.len());
    let mut colors = Vec::with_capacity(verts.len());

    let low = def.ground_low.to_linear();
    let mid = def.ground.to_linear();
    let high = def.ground_high.to_linear();

    for v in verts.drain(..) {
        let dir = v.normalize();
        let h = terrain.height(dir);
        let r = def.radius * (1.0 + def.hill_amp * h);
        positions.push([dir.x * r, dir.y * r, dir.z * r]);
        // color by height band: crater floors dark, peaks bright
        let t = (h * 0.38 + 0.5).clamp(0.0, 1.0);
        let c = if t < 0.5 {
            mix(low, mid, t * 2.0)
        } else {
            mix(mid, high, (t - 0.5) * 2.0)
        };
        colors.push([c.red, c.green, c.blue, 1.0]);
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(faces));
    mesh.compute_smooth_normals();
    if !std::env::var("TOONDBG").unwrap_or_default().contains("nolean") { toon_terrain_normals(&mut mesh); }
    mesh
}

/// Cel bands quantize N·L, so every small bump in a noisy normal field turns into a speckle
/// of triangles flipping between bands. Lean the shading normals toward the planet's own
/// radial normal: the terminator becomes one clean line across the world and only the big
/// hills and crater walls step into their own bands. Visual only — collision stays analytic.
fn toon_terrain_normals(mesh: &mut Mesh) {
    let Some(pos) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).and_then(|a| a.as_float3()) else { return };
    let radial: Vec<Vec3> = pos.iter().map(|p| Vec3::from(*p).normalize_or_zero()).collect();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(nrm)) = mesh.attribute_mut(Mesh::ATTRIBUTE_NORMAL) {
        for (n, up) in nrm.iter_mut().zip(radial) {
            let lean = up.lerp(Vec3::from(*n), crate::config::TOON_TERRAIN_NORMAL_DETAIL).normalize_or_zero();
            *n = lean.to_array();
        }
    }
}

fn mix(a: LinearRgba, b: LinearRgba, t: f32) -> LinearRgba {
    LinearRgba::rgb(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
    )
}

use crate::meshkit::icosphere;

/// Spawn terrain, props, sky, lights for the current stage. Returns the solid props and the
/// Grind-Lines for the caller to insert as resources — after `interact::spawn_interactables`
/// has placed the stage's chests and shrines clear of the rails.
pub fn spawn_stage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    seed: u64,
) -> (PropColliders, crate::techs::GrindLines) {
    let def = planet.kind.def();
    // deterministic prop scatter from the run seed (terrain was already seed-driven)
    let mut rng = StdRng::seed_from_u64(seed ^ 0xA11CE ^ planet.terrain.seed as u64);
    // solid props collected as we place them
    let mut colliders: Vec<PropCollider> = Vec::new();

    // Terrain
    let mesh = meshes.add(planet_mesh(&def, &planet.terrain));
    let mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::IDENTITY,
        StageScoped,
    ));

    // Rocks: six angular variants (faceted, chunkier displacement), scattered.
    let rock_mat = materials.add(StandardMaterial {
        base_color: def.ground_low,
        perceptual_roughness: 1.0,
        ..default()
    });
    let rock_dark = materials.add(StandardMaterial {
        base_color: def.ground_low.darker(0.08),
        perceptual_roughness: 1.0,
        ..default()
    });
    let make_rock = |meshes: &mut Assets<Mesh>, seed: u32, amp: f32, subdiv: u32| -> Handle<Mesh> {
        let (mut v, f) = icosphere(subdiv);
        for p in v.iter_mut() {
            let n = sphere::hills(p.normalize(), seed);
            let n2 = sphere::hills(p.normalize() * 2.3, seed + 7);
            *p *= 1.0 + amp * n + amp * 0.5 * n2;
        }
        let positions: Vec<[f32; 3]> = v.iter().map(|p| [p.x, p.y, p.z]).collect();
        let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        m.insert_indices(Indices::U32(f));
        m.compute_smooth_normals();
        meshes.add(m)
    };
    let rock_meshes: Vec<Handle<Mesh>> = (0..6)
        .map(|i| make_rock(meshes, 100 + i * 13 + planet.terrain.seed, 0.45, if i % 2 == 0 { 1 } else { 0 }))
        .collect();
    for dir in sphere::fib_sphere(def.rocks) {
        let jitter = Vec3::new(
            rng.gen_range(-0.3..0.3),
            rng.gen_range(-0.3..0.3),
            rng.gen_range(-0.3..0.3),
        );
        let dir = (dir + jitter).normalize();
        let scale = rng.gen_range(0.4..2.4) * Vec3::new(rng.gen_range(0.8..1.3), rng.gen_range(0.6..1.1), rng.gen_range(0.8..1.3));
        let pos = planet.surface_point(dir) - dir * scale.y * 0.25;
        let fwd = sphere::tangent_frame(dir).0;
        commands.spawn((
            Mesh3d(rock_meshes[rng.gen_range(0..6)].clone()),
            MeshMaterial3d(if rng.gen_bool(0.3) { rock_dark.clone() } else { rock_mat.clone() }),
            Transform::from_translation(pos)
                .with_rotation(sphere::frame_quat(dir, fwd) * Quat::from_rotation_y(rng.gen_range(0.0..6.28)))
                .with_scale(scale),
            StageScoped,
        ));
        // only chunky rocks block movement — pebbles you just step over
        if scale.x > 1.1 {
            colliders.push(PropCollider { dir, radius: scale.x * 0.75, height: scale.y * 1.2 });
        }
    }

    // Boulders: a few big angular landmarks that break up the horizon.
    let boulder_meshes: Vec<Handle<Mesh>> = (0..3)
        .map(|i| make_rock(meshes, 400 + i * 17 + planet.terrain.seed, 0.6, 1))
        .collect();
    for _ in 0..(def.rocks / 20).max(4) {
        let dir = random_dir(&mut rng);
        let scale = rng.gen_range(3.0..6.0);
        let pos = planet.surface_point(dir) - dir * scale * 0.3;
        let fwd = sphere::tangent_frame(dir).0;
        commands.spawn((
            Mesh3d(boulder_meshes[rng.gen_range(0..3)].clone()),
            MeshMaterial3d(rock_dark.clone()),
            Transform::from_translation(pos)
                .with_rotation(sphere::frame_quat(dir, fwd) * Quat::from_rotation_y(rng.gen_range(0.0..6.28)))
                .with_scale(Vec3::splat(scale)),
            StageScoped,
        ));
        // big landmarks are always solid
        colliders.push(PropCollider { dir, radius: scale * 0.7, height: scale * 1.4 });
    }

    // Crystals: emissive spikes (accent + light the night side a bit).
    let crystal_mat = materials.add(StandardMaterial {
        base_color: def.enemy_tint,
        emissive: def.enemy_tint.to_linear() * 2.0,
        perceptual_roughness: 0.2,
        ..default()
    });
    let crystal_mesh = meshes.add(Mesh::from(Cone::new(0.35, 1.6)));
    for _ in 0..def.crystals {
        let dir = random_dir(&mut rng);
        let pos = planet.surface_point(dir);
        let fwd = sphere::tangent_frame(dir).0;
        let scale = rng.gen_range(0.6..1.5);
        commands.spawn((
            Mesh3d(crystal_mesh.clone()),
            MeshMaterial3d(crystal_mat.clone()),
            Transform::from_translation(pos)
                .with_rotation(sphere::frame_quat(dir, fwd) * Quat::from_rotation_x(rng.gen_range(-0.25..0.25)))
                .with_scale(Vec3::splat(scale)),
            StageScoped,
        ));
        colliders.push(PropCollider { dir, radius: scale * 0.4, height: scale * 1.6 });
    }

    // Wrecks: crashed landers half-sunk in the surface — some are your own dead prints.
    {
        use crate::meshkit::{at, MeshData};
        let wreck_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.55, 0.55, 0.60),
            perceptual_roughness: 0.6,
            metallic: 0.5,
            ..default()
        });
        let mut w = MeshData::new();
        w.add_box(Vec3::new(1.3, 0.8, 1.1), at(Vec3::new(0.0, 0.35, 0.0)), Color::WHITE);
        w.add_cylinder(0.42, 0.5, 8, at(Vec3::new(0.0, -0.05, 0.0)), Color::srgb(0.7, 0.7, 0.75)); // engine bell
        for s in [-1.0, 1.0] {
            w.add_cylinder(0.08, 1.0, 5, Transform::from_translation(Vec3::new(0.7 * s, 0.1, 0.5)).with_rotation(Quat::from_rotation_z(0.5 * s)), Color::srgb(0.5, 0.5, 0.55)); // bent leg
        }
        w.add_box(Vec3::new(0.9, 0.05, 0.7), at(Vec3::new(0.3, 0.75, -0.2)), Color::srgb(0.4, 0.4, 0.45)); // torn panel
        let wreck_mesh = meshes.add(w.build());
        for _ in 0..def.rocks / 45 + 3 {
            let dir = random_dir(&mut rng);
            let scale = rng.gen_range(1.4..2.4);
            let pos = planet.surface_point(dir) - dir * scale * 0.4; // half-sunk
            let fwd = sphere::tangent_frame(dir).0;
            commands.spawn((
                Mesh3d(wreck_mesh.clone()),
                MeshMaterial3d(wreck_mat.clone()),
                Transform::from_translation(pos)
                    .with_rotation(sphere::frame_quat(dir, fwd) * Quat::from_rotation_x(rng.gen_range(-0.5..0.5)) * Quat::from_rotation_z(rng.gen_range(-0.4..0.4)))
                    .with_scale(Vec3::splat(scale)),
                StageScoped,
            ));
            colliders.push(PropCollider { dir, radius: scale * 0.85, height: scale * 1.1 });
        }
    }

    // Radio beacons: mast + dish + a blinking light — lore landmarks, night guides.
    {
        use crate::meshkit::{at, MeshData};
        let beacon_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.62, 0.68),
            perceptual_roughness: 0.5,
            metallic: 0.6,
            ..default()
        });
        let light_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.35, 0.3),
            emissive: LinearRgba::rgb(3.0, 0.4, 0.3),
            unlit: true,
            ..default()
        });
        let mut b = MeshData::new();
        b.add_box(Vec3::new(0.5, 0.2, 0.5), at(Vec3::new(0.0, 0.1, 0.0)), Color::WHITE); // base
        b.add_cylinder(0.06, 1.8, 6, at(Vec3::new(0.0, 1.0, 0.0)), Color::WHITE); // mast
        b.add_ellipsoid(Vec3::new(0.35, 0.1, 0.35), 1, Transform::from_translation(Vec3::new(0.0, 1.9, 0.0)).with_rotation(Quat::from_rotation_x(0.5)), Color::srgb(0.8, 0.8, 0.85)); // dish
        let beacon_mesh = meshes.add(b.build());
        let light_mesh = meshes.add(Mesh::from(Sphere::new(0.09)));
        for _ in 0..def.crystals / 8 + 4 {
            let dir = random_dir(&mut rng);
            let pos = planet.surface_point(dir);
            let fwd = sphere::tangent_frame(dir).0;
            let rot = sphere::frame_quat(dir, fwd);
            commands.spawn((
                Mesh3d(beacon_mesh.clone()),
                MeshMaterial3d(beacon_mat.clone()),
                Transform::from_translation(pos).with_rotation(rot),
                StageScoped,
            ));
            commands.spawn((
                Mesh3d(light_mesh.clone()),
                MeshMaterial3d(light_mat.clone()),
                Transform::from_translation(pos + dir * 2.0),
                StageScoped,
            ));
            // the mast is a thin pole — small radius, but you can't walk through it
            colliders.push(PropCollider { dir, radius: 0.55, height: 2.4 });
        }
    }

    // Flora: per-world plant archetypes built from shared primitive parts.
    if def.flora > 0 {
        let (part_a, part_b, mat_a, mat_b): (Handle<Mesh>, Handle<Mesh>, Handle<StandardMaterial>, Handle<StandardMaterial>) =
            match def.flora_style {
                FloraStyle::Spires => (
                    meshes.add(Mesh::from(Cone::new(0.22, 1.6))),
                    meshes.add(Mesh::from(Cone::new(0.13, 1.0))),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.85, 0.84, 0.80),
                        perceptual_roughness: 1.0,
                        ..default()
                    }),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.72, 0.70, 0.66),
                        perceptual_roughness: 1.0,
                        ..default()
                    }),
                ),
                FloraStyle::Thorns => (
                    meshes.add(Mesh::from(Cone::new(0.09, 1.3))),
                    meshes.add(Mesh::from(Cone::new(0.06, 0.9))),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.55, 0.22, 0.12),
                        perceptual_roughness: 0.95,
                        ..default()
                    }),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.70, 0.32, 0.14),
                        perceptual_roughness: 0.95,
                        ..default()
                    }),
                ),
                FloraStyle::GlowShrooms => (
                    meshes.add(Mesh::from(Cylinder::new(0.10, 1.1))),
                    meshes.add(Mesh::from(Sphere::new(0.42))),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.35, 0.30, 0.45),
                        perceptual_roughness: 0.9,
                        ..default()
                    }),
                    materials.add(StandardMaterial {
                        base_color: Color::srgb(0.75, 0.45, 1.0),
                        emissive: LinearRgba::rgb(1.4, 0.7, 2.2),
                        perceptual_roughness: 0.4,
                        ..default()
                    }),
                ),
            };
        for _ in 0..def.flora {
            let dir = random_dir(&mut rng);
            let pos = planet.surface_point(dir);
            let fwd = sphere::tangent_frame(dir).0;
            let scale = rng.gen_range(0.7..1.6);
            let rot = sphere::frame_quat(dir, fwd) * Quat::from_rotation_y(rng.gen_range(0.0..6.28));
            commands
                .spawn((
                    Transform::from_translation(pos).with_rotation(rot).with_scale(Vec3::splat(scale)),
                    Visibility::default(),
                    StageScoped,
                ))
                .with_children(|plant| {
                    match def.flora_style {
                        FloraStyle::Spires => {
                            plant.spawn((Mesh3d(part_a.clone()), MeshMaterial3d(mat_a.clone()), Transform::from_xyz(0.0, 0.8, 0.0)));
                            plant.spawn((
                                Mesh3d(part_b.clone()),
                                MeshMaterial3d(mat_b.clone()),
                                Transform::from_xyz(0.25, 0.5, 0.1).with_rotation(Quat::from_rotation_z(-0.35)),
                            ));
                        }
                        FloraStyle::Thorns => {
                            for k in 0..5 {
                                let a = k as f32 / 5.0 * std::f32::consts::TAU;
                                let lean = Quat::from_rotation_y(a) * Quat::from_rotation_x(0.55);
                                let m = if k % 2 == 0 { &part_a } else { &part_b };
                                let mm = if k % 2 == 0 { &mat_a } else { &mat_b };
                                plant.spawn((
                                    Mesh3d(m.clone()),
                                    MeshMaterial3d(mm.clone()),
                                    Transform::from_xyz(0.0, 0.45, 0.0).with_rotation(lean),
                                ));
                            }
                        }
                        FloraStyle::GlowShrooms => {
                            plant.spawn((Mesh3d(part_a.clone()), MeshMaterial3d(mat_a.clone()), Transform::from_xyz(0.0, 0.55, 0.0)));
                            plant.spawn((
                                Mesh3d(part_b.clone()),
                                MeshMaterial3d(mat_b.clone()),
                                Transform::from_xyz(0.0, 1.15, 0.0).with_scale(Vec3::new(1.0, 0.7, 1.0)),
                            ));
                        }
                    }
                });
        }
    }

    // Grind-Lines (§4): the ridged crests' exposed spines, and the rail that shows them.
    // Traced from the terrain alone — no draw from `rng` — so the layout stream above is
    // untouched and every machine lays the same rails (CLAUDE.md rule 5).
    let lines = grind_lines(planet, &colliders);
    if !lines.spines.is_empty() {
        commands.spawn((
            Mesh3d(meshes.add(rail_mesh(planet, &lines))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: crate::techs::RAIL_COLOR,
                unlit: true,
                ..default()
            })),
            Transform::IDENTITY,
            StageScoped,
        ));
    }

    // Starfield
    let star_mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));
    let star_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::rgb(2.2, 2.2, 2.4),
        unlit: true,
        ..default()
    });
    for _ in 0..420 {
        let dir = random_dir(&mut rng);
        let d = rng.gen_range(1400.0..1900.0);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_mat.clone()),
            Transform::from_translation(dir * d).with_scale(Vec3::splat(rng.gen_range(0.8..2.6))),
            StageScoped,
        ));
    }

    // Earthrise
    if def.has_earthrise {
        let earth_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.5, 0.95),
            emissive: LinearRgba::rgb(0.12, 0.3, 0.7),
            perceptual_roughness: 0.7,
            ..default()
        });
        let dir = Vec3::new(0.5, 0.62, 0.35).normalize();
        commands.spawn((
            Mesh3d(meshes.add(Mesh::from(Sphere::new(90.0)))),
            MeshMaterial3d(earth_mat),
            Transform::from_translation(dir * 1500.0),
            StageScoped,
        ));
    }

    // Sun: directional light + visible disc. `sun_dir` is the way the light TRAVELS.
    let sun_dir = -sunward();
    commands.spawn((
        DirectionalLight {
            color: def.sun,
            illuminance: 9_000.0,
            shadows_enabled: true,
            ..default()
        },
        // the Devoured Sun Shard dims it (`items::apply_sun_shrink`)
        crate::items::SunLight { base: 9_000.0 },
        Transform::from_translation(-sun_dir * 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        StageScoped,
    ));
    let sun_mat = materials.add(StandardMaterial {
        base_color: def.sun,
        emissive: def.sun.to_linear() * 18.0,
        unlit: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Mesh::from(Sphere::new(45.0)))),
        MeshMaterial3d(sun_mat),
        crate::items::SunDisc,
        Transform::from_translation(-sun_dir * 1600.0),
        StageScoped,
    ));

    (PropColliders(colliders), lines)
}

/// The stage's Grind-Lines: the terrain's crest spines, smoothed, and cut short of every
/// solid prop — a rail never runs through a boulder, it stops before it.
fn grind_lines(planet: &CurrentPlanet, colliders: &[PropCollider]) -> crate::techs::GrindLines {
    use crate::config::{GRIND_MIN_LEN, GRIND_PROP_CLEARANCE, GRIND_STEP};
    let min_pts = (GRIND_MIN_LEN / GRIND_STEP) as usize + 1;
    let blocked = |d: Vec3| {
        colliders.iter().any(|c| d.dot(c.dir) > ((c.radius + GRIND_PROP_CLEARANCE) / planet.radius).cos())
    };
    let mut out: Vec<Vec<Vec3>> = Vec::new();
    for mut line in planet.terrain.ridge_spines(planet.radius) {
        // two passes of a 3-point average: the crest trace's cross-section search leaves a
        // few centimetres of zig-zag that a rail (and a rider's camera) would feel
        for _ in 0..2 {
            let prev = line.clone();
            for i in 1..prev.len().saturating_sub(1) {
                line[i] = (prev[i - 1] + prev[i] * 2.0 + prev[i + 1]).normalize();
            }
        }
        let mut run: Vec<Vec3> = Vec::new();
        for d in line {
            if blocked(d) {
                if run.len() >= min_pts {
                    out.push(std::mem::take(&mut run));
                }
                run.clear();
            } else {
                run.push(d);
            }
        }
        if run.len() >= min_pts {
            out.push(run);
        }
    }
    crate::techs::GrindLines::new(out, planet.radius)
}

/// The rails, as ONE mesh for the whole stage: a thin glowing bar along every spine at
/// GRIND_RAIL_LIFT over the crest, on a "vertebra" fin every few metres so it reads as the
/// ridge's own exposed spine rather than a pipe laid on the ground, and a knob at each end
/// (where a rider comes off).
fn rail_mesh(planet: &CurrentPlanet, lines: &crate::techs::GrindLines) -> Mesh {
    use crate::config::GRIND_RAIL_LIFT;
    use crate::meshkit::MeshData;
    let fin = Color::srgb(0.32, 0.5, 0.56);
    let rail = |d: Vec3| planet.surface_point(d) + d * GRIND_RAIL_LIFT;
    let mut m = MeshData::new();
    for sp in &lines.spines {
        for (i, w) in sp.pts.windows(2).enumerate() {
            let (a, b) = (rail(w[0]), rail(w[1]));
            let len = (b - a).length();
            if len < 1e-4 {
                continue;
            }
            let run = (b - a) / len;
            m.add_cylinder(
                0.075,
                len + 0.03,
                6,
                Transform::from_translation((a + b) * 0.5).with_rotation(Quat::from_rotation_arc(Vec3::Y, run)),
                Color::WHITE,
            );
            if i % 2 == 0 {
                let up = w[0];
                let h = GRIND_RAIL_LIFT + 0.35;
                m.add_box(
                    Vec3::new(0.07, h, 0.34),
                    Transform::from_translation(planet.surface_point(up) + up * (h * 0.5 - 0.3))
                        .with_rotation(sphere::frame_quat(up, run)),
                    fin,
                );
            }
        }
        for end in [sp.pts[0], sp.pts[sp.pts.len() - 1]] {
            m.add_sphere(0.16, 1, Transform::from_translation(rail(end)), Color::WHITE);
        }
    }
    m.build()
}

/// Unit direction from the planet's core toward the sun — the lit hemisphere faces it.
/// Fixed for now; P07's day/night cycle turns it, and `is_night` turns with it.
pub fn sunward() -> Vec3 {
    Vec3::new(0.55, -0.35, 0.75).normalize()
}

/// Is the surface point `dir` on the night side? Gameplay's one test (Tome of Nightfall),
/// against the same sun the lighting uses. `sun_shrink` (Devoured Sun Shard, §3 diegetic
/// difficulty) eats the day side from its rim: at 0 the terminator is the great circle, at
/// 1 the whole world is night.
pub fn is_night(dir: Vec3, sun_shrink: f32) -> bool {
    sun_shrink >= 1.0 || dir.dot(sunward()) < sun_shrink.max(0.0)
}

pub fn random_dir(rng: &mut impl Rng) -> Vec3 {
    loop {
        let v = Vec3::new(
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
        );
        let l = v.length_squared();
        if l > 0.001 && l < 1.0 {
            return v / l.sqrt();
        }
    }
}
