//! Planetary events — per-world hazards that make each rock feel alive. First one:
//! Mars's MIGRATING DUST STORM. A storm-cell wanders the surface; while you're inside it,
//! ranged enemies can't see you (a mobile stealth bubble to ride or flee), and the screen
//! hazes over. Ride it to break Beamer/Lobber lines, or flee it if it drifts onto a pack.
//!
//! CO-OP: the HOST owns the storm — when it forms, where, which way it drifts — and marks
//! every astronaut standing in it with `InStorm`, which is what the ranged horde checks.
//! A client only DRAWS it: the cell's shape rides `RunSnapMsg`, and `dust_storm_visuals`
//! (the one part both machines run) dead-reckons the drift between snapshots and hazes the
//! screen from the local astronaut's own position.

use crate::content::planets::PlanetKind;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::Player;
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;

#[derive(Resource, Default)]
pub struct DustStorm {
    pub active: bool,
    pub dir: Vec3,        // storm center (unit direction on the sphere)
    pub radius: f32,      // meters
    pub heading: Vec3,    // tangent drift direction
    pub timer: f32,       // HOST: time left in the current phase (active or gap)
    /// HOST: the first-storm delay has been armed for this Mars stage.
    pub primed: bool,
    /// The LOCAL astronaut is inside — drives this machine's HUD haze only. Gameplay asks
    /// the per-astronaut `InStorm` marker instead.
    pub player_inside: bool,
    pub spawned_vis: bool,
}

/// HOST: this astronaut is inside the dust storm, so ranged enemies can't see them. A
/// marker per astronaut rather than one flag, so riding the storm hides YOU, not the squad.
#[derive(Component)]
pub struct InStorm;

#[derive(Component)]
pub struct DustStormVis;

const STORM_RADIUS: f32 = 24.0;
const STORM_ACTIVE_SECS: f32 = 26.0;
const STORM_GAP_SECS: f32 = 20.0;
const STORM_DRIFT: f32 = 3.2; // m/s

/// Carry the cell along its great circle, re-projecting the heading to stay tangent.
/// The one drift step both the host's simulation and a client's dead reckoning take.
fn drift(storm: &mut DustStorm, dt: f32, radius: f32) {
    let h = (storm.heading - storm.dir * storm.heading.dot(storm.dir)).normalize_or_zero();
    if h != Vec3::ZERO {
        storm.dir = sphere::offset_dir(storm.dir, h, STORM_DRIFT * dt, radius);
        storm.heading = h;
    }
}

/// HOST: the storm's life cycle, and who is standing in it.
pub fn dust_storm_sim(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut storm: ResMut<DustStorm>,
    q_astronauts: Query<(Entity, &Player, Has<InStorm>)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    // Storms only blow on Mars — elsewhere, make sure it's off and nobody stays hidden.
    if planet.kind != PlanetKind::Mars {
        storm.active = false;
        storm.primed = false; // the next Mars stage gets the full first-storm delay again
        for (e, _, hidden) in &q_astronauts {
            if hidden {
                commands.entity(e).try_remove::<InStorm>();
            }
        }
        return;
    }

    let mut rng = rand::thread_rng();
    if !storm.primed {
        storm.primed = true;
        storm.active = false;
        storm.timer = STORM_GAP_SECS * 0.5; // first storm arrives fairly soon
    }

    storm.timer -= dt;
    if storm.active {
        drift(&mut storm, dt, planet.radius);
        if storm.timer <= 0.0 {
            storm.active = false;
            storm.timer = STORM_GAP_SECS;
        }
    } else if storm.timer <= 0.0 {
        // spawn a new storm somewhere on the planet, drifting a random way
        storm.active = true;
        storm.timer = STORM_ACTIVE_SECS;
        storm.radius = STORM_RADIUS;
        storm.dir = crate::planet::random_dir(&mut rng);
        let (t, b) = sphere::tangent_frame(storm.dir);
        let a = rng.gen_range(0.0..std::f32::consts::TAU);
        storm.heading = (t * a.cos() + b * a.sin()).normalize();
    }

    // Every astronaut, not just the local one: a joiner riding the storm must be as
    // invisible to the Beamers as the host would be.
    for (e, p, hidden) in &q_astronauts {
        let inside = storm.active && sphere::arc_dist(p.dir, storm.dir, planet.radius) < storm.radius;
        // `try_`: a stage change queued this same frame may already be sweeping this body.
        if inside && !hidden {
            commands.entity(e).try_insert(InStorm);
        } else if !inside && hidden {
            commands.entity(e).try_remove::<InStorm>();
        }
    }
}

/// Every machine: the storm dome and the local haze flag. On a client this is also where
/// the streamed cell drifts between the 4 Hz snapshots, so it glides instead of stepping.
#[allow(clippy::too_many_arguments)]
pub fn dust_storm_visuals(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    role: Res<crate::net::NetRole>,
    mut storm: ResMut<DustStorm>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_local: Query<&Player, With<crate::player::LocalPlayer>>,
    mut q_vis: Query<(&mut Transform, &mut Visibility), With<DustStormVis>>,
) {
    let dt = time.delta_secs();
    if planet.kind != PlanetKind::Mars {
        storm.player_inside = false;
        for (_, mut vis) in &mut q_vis {
            *vis = Visibility::Hidden;
        }
        return;
    }

    // Lazily create the (hidden) dome on each Mars stage. Keyed on the dome actually
    // existing: it is StageScoped, so a flag alone would never rebuild it for a second
    // Mars stage (or a second run).
    if q_vis.is_empty() {
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.75, 0.5, 0.32, 0.16),
            emissive: LinearRgba::rgb(0.22, 0.12, 0.05),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        commands.spawn((
            DustStormVis,
            Mesh3d(meshes.add(Mesh::from(Sphere::new(1.0)))),
            MeshMaterial3d(mat),
            Transform::from_scale(Vec3::splat(STORM_RADIUS)),
            Visibility::Hidden,
            StageScoped,
        ));
        storm.spawned_vis = true;
        return;
    }

    if storm.active && !role.simulates() && dt > 0.0 {
        drift(&mut storm, dt, planet.radius);
    }

    // LOCAL only — this is the HUD haze. Gameplay reads `InStorm` on the host.
    storm.player_inside = storm.active
        && q_local
            .iter()
            .next()
            .map(|p| sphere::arc_dist(p.dir, storm.dir, planet.radius) < storm.radius)
            .unwrap_or(false);

    // move/show the dome
    for (mut tf, mut vis) in &mut q_vis {
        if storm.active {
            *vis = Visibility::Visible;
            tf.translation = planet.surface_point(storm.dir) + storm.dir * 2.0;
            let pulse = 1.0 + (time.elapsed_secs() * 0.6).sin() * 0.04;
            tf.scale = Vec3::splat(storm.radius * pulse);
        } else {
            *vis = Visibility::Hidden;
        }
    }
}
