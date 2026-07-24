//! Planetary events — per-world hazards that make each rock feel alive. First one:
//! Mars's MIGRATING DUST STORM. A storm-cell wanders the surface; while you're inside it,
//! ranged enemies can't see you (a mobile stealth bubble to ride or flee), and the screen
//! hazes over. Ride it to break Beamer/Lobber lines, or flee it if it drifts onto a pack.

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
    pub timer: f32,       // time left in the current phase (active or gap)
    pub player_inside: bool,
    pub spawned_vis: bool,
}

#[derive(Component)]
pub struct DustStormVis;

const STORM_RADIUS: f32 = 24.0;
const STORM_ACTIVE_SECS: f32 = 26.0;
const STORM_GAP_SECS: f32 = 20.0;
const STORM_DRIFT: f32 = 3.2; // m/s

pub fn dust_storm_system(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut storm: ResMut<DustStorm>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q_player: Query<&Player>,
    mut q_vis: Query<(&mut Transform, &mut Visibility), With<DustStormVis>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    // Storms only blow on Mars — elsewhere, make sure it's off and hidden.
    if planet.kind != PlanetKind::Mars {
        if storm.active {
            storm.active = false;
            storm.player_inside = false;
        }
        for (_, mut vis) in &mut q_vis {
            *vis = Visibility::Hidden;
        }
        return;
    }

    let Ok(player) = q_player.single() else { return };
    let mut rng = rand::thread_rng();

    // Lazily create the (hidden) storm dome the first time we're on Mars.
    if !storm.spawned_vis && q_vis.is_empty() {
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
        storm.timer = STORM_GAP_SECS * 0.5; // first storm arrives fairly soon
        return;
    }

    storm.timer -= dt;

    if storm.active {
        // drift the storm along a great circle, re-projecting the heading to stay tangent
        let h = (storm.heading - storm.dir * storm.heading.dot(storm.dir)).normalize_or_zero();
        if h != Vec3::ZERO {
            storm.dir = sphere::offset_dir(storm.dir, h, STORM_DRIFT * dt, planet.radius);
            storm.heading = h;
        }
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

    storm.player_inside =
        storm.active && sphere::arc_dist(player.dir, storm.dir, planet.radius) < storm.radius;

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
