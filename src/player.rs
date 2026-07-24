//! The astronaut: spherical-gravity kinematic controller (walk / jump / slide / bunny-hop)
//! and the orbiting third-person camera rig.

use crate::config::*;
use crate::content::characters::Passive;
use crate::fx::Shake;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::run::{RunPhase, RunState};
use crate::sphere;
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

#[derive(Component)]
pub struct Player {
    pub dir: Vec3,     // unit direction from planet core
    pub height: f32,   // above terrain
    pub vel_t: Vec3,   // world-space tangent velocity
    pub vel_r: f32,    // radial velocity
    pub grounded: bool,
    pub facing: Vec3,  // tangent unit vector
    pub jumps_used: i32,
    pub slide_timer: f32,
    pub slide_cd: f32,
    pub land_timer: f32, // time since landing (for bhop window)
    pub coyote: f32,
}

#[derive(Component)]
pub struct PlayerRig; // camera

#[derive(Resource)]
pub struct CamRig {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.55 }
    }
}

pub fn spawn_player(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    run: &RunState,
) {
    let def = run.character.def();
    let dir = Vec3::Y;
    let pos = planet.surface_point(dir) + dir * PLAYER_HEIGHT;

    let suit = materials.add(StandardMaterial {
        base_color: def.suit,
        perceptual_roughness: 0.7,
        ..default()
    });
    let visor = materials.add(StandardMaterial {
        base_color: def.visor,
        emissive: def.visor.to_linear() * 1.2,
        perceptual_roughness: 0.15,
        ..default()
    });
    let pack = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.85),
        perceptual_roughness: 0.9,
        ..default()
    });

    commands
        .spawn((
            Player {
                dir,
                height: 0.0,
                vel_t: Vec3::ZERO,
                vel_r: 0.0,
                grounded: true,
                facing: sphere::tangent_frame(dir).0,
                jumps_used: 0,
                slide_timer: 0.0,
                slide_cd: 0.0,
                land_timer: 10.0,
                coyote: 0.0,
            },
            Transform::from_translation(pos),
            Visibility::default(),
            StageScoped,
        ))
        .with_children(|p| {
            // full suit (torso, chest panel, shoulders, arms, legs, boots) as one mesh
            p.spawn((
                Mesh3d(meshes.add(astronaut_suit_mesh())),
                MeshMaterial3d(suit.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
            // helmet
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.3)))),
                MeshMaterial3d(suit),
                Transform::from_xyz(0.0, 0.7, 0.0),
            ));
            // visor
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Sphere::new(0.24)))),
                MeshMaterial3d(visor),
                Transform::from_xyz(0.0, 0.72, -0.16).with_scale(Vec3::new(1.05, 0.85, 0.7)),
            ));
            // backpack (main + two life-support tanks)
            p.spawn((
                Mesh3d(meshes.add(backpack_mesh())),
                MeshMaterial3d(pack),
                Transform::from_xyz(0.0, 0.18, 0.0),
            ));
            // hand tool (whatever weapon is equipped, this is its silhouette)
            let tool_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(0.25, 0.27, 0.32),
                perceptual_roughness: 0.5,
                metallic: 0.6,
                ..default()
            });
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.14, 0.16, 0.68)))),
                MeshMaterial3d(tool_mat),
                Transform::from_xyz(0.42, 0.12, -0.28),
            ));
            // flashlight body + glowing lens, mounted on top of the tool
            let lens_mat = materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.97, 0.85),
                emissive: LinearRgba::rgb(6.0, 5.6, 4.6),
                unlit: true,
                ..default()
            });
            p.spawn((
                Mesh3d(meshes.add(Mesh::from(Cuboid::new(0.09, 0.09, 0.2)))),
                MeshMaterial3d(lens_mat),
                Transform::from_xyz(0.42, 0.24, -0.52),
            ));
            // the beam itself: a real spotlight aimed where the astronaut faces
            p.spawn((
                SpotLight {
                    color: Color::srgb(1.0, 0.96, 0.86),
                    intensity: 6_000_000.0,
                    range: 55.0,
                    radius: 0.05,
                    inner_angle: 0.22,
                    outer_angle: 0.55,
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(0.42, 0.3, -0.5)
                    .with_rotation(Quat::from_rotation_x(-0.10)),
            ));
        });
}

/// A chunky astronaut suit baked into one mesh (suit material multiplies the vertex
/// colors: WHITE = suit tint, darker = joints/boots/panel).
fn astronaut_suit_mesh() -> Mesh {
    use crate::meshkit::at;
    let body = Color::WHITE;
    let joint = Color::srgb(0.5, 0.5, 0.56);
    let dark = Color::srgb(0.30, 0.30, 0.36);
    let mut m = crate::meshkit::MeshData::new();
    // torso + lower torso
    m.add_box(Vec3::new(0.5, 0.55, 0.34), at(Vec3::new(0.0, 0.2, 0.0)), body);
    m.add_box(Vec3::new(0.42, 0.32, 0.3), at(Vec3::new(0.0, -0.12, 0.0)), body);
    // chest control panel
    m.add_box(Vec3::new(0.26, 0.2, 0.05), at(Vec3::new(0.0, 0.24, -0.19)), dark);
    // neck
    m.add_cylinder(0.11, 0.14, 8, at(Vec3::new(0.0, 0.52, 0.0)), joint);
    for s in [-1.0, 1.0] {
        // shoulder
        m.add_sphere(0.16, 1, at(Vec3::new(0.31 * s, 0.42, 0.0)), joint);
        // upper arm + forearm + glove
        m.add_cylinder(0.1, 0.36, 8, Transform::from_translation(Vec3::new(0.34 * s, 0.14, 0.0)).with_rotation(Quat::from_rotation_z(0.15 * s)), body);
        m.add_cylinder(0.09, 0.34, 8, at(Vec3::new(0.38 * s, -0.18, 0.0)), body);
        m.add_sphere(0.11, 1, at(Vec3::new(0.4 * s, -0.4, 0.0)), joint);
        // thigh + shin + boot
        m.add_cylinder(0.13, 0.36, 8, at(Vec3::new(0.15 * s, -0.45, 0.0)), body);
        m.add_cylinder(0.11, 0.32, 8, at(Vec3::new(0.15 * s, -0.76, 0.0)), body);
        m.add_box(Vec3::new(0.2, 0.13, 0.32), at(Vec3::new(0.15 * s, -0.9, -0.06)), dark);
    }
    m.build()
}

/// Backpack: main box + two life-support tanks.
fn backpack_mesh() -> Mesh {
    use crate::meshkit::at;
    let shell = Color::WHITE;
    let tank = Color::srgb(0.7, 0.72, 0.78);
    let mut m = crate::meshkit::MeshData::new();
    m.add_box(Vec3::new(0.42, 0.52, 0.24), at(Vec3::new(0.0, 0.0, 0.34)), shell);
    for s in [-1.0, 1.0] {
        m.add_cylinder(0.09, 0.5, 8, at(Vec3::new(0.13 * s, 0.0, 0.44)), tank);
    }
    m.build()
}

/// WASD + jump + slide, in the camera's tangent frame.
pub fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    rig: Res<CamRig>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut q: Query<&mut Player>,
) {
    let Ok(mut p) = q.single_mut() else { return };
    let dt = time.delta_secs();

    let (t, b) = sphere::tangent_frame(p.dir);
    let fwd = (t * rig.yaw.cos() + b * rig.yaw.sin()).normalize();
    let right = fwd.cross(p.dir).normalize();

    let mut wish = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        wish += fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        wish -= fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        wish += right;
    }
    if keys.pressed(KeyCode::KeyA) {
        wish -= right;
    }
    let wish = wish.normalize_or_zero();

    let speed_mult = run.move_speed_mult();
    let sliding = p.slide_timer > 0.0;
    let max_speed = PLAYER_RUN_SPEED * speed_mult * if sliding { SLIDE_BOOST } else { 1.0 };

    let control = if p.grounded { 1.0 } else { PLAYER_AIR_CONTROL };
    if wish != Vec3::ZERO {
        p.vel_t += wish * PLAYER_ACCEL * control * dt;
    } else if p.grounded && !sliding && p.land_timer > BHOP_WINDOW {
        // friction
        let v = p.vel_t;
        let drop = PLAYER_FRICTION * dt;
        let l = v.length();
        p.vel_t = if l <= drop { Vec3::ZERO } else { v * ((l - drop) / l) };
    }

    // speed clamp (bhop chains keep slide speed but respect the hard cap)
    let cap = if p.grounded && !sliding && p.land_timer > BHOP_WINDOW {
        max_speed
    } else {
        PLAYER_RUN_SPEED * speed_mult * SPEED_HARD_CAP
    };
    if p.vel_t.length() > cap {
        p.vel_t = p.vel_t.normalize() * cap;
    }

    // jump
    let max_jumps = 1 + run.stats.extra_jumps;
    if keys.just_pressed(KeyCode::Space) {
        let can_ground = p.grounded || p.coyote > 0.0;
        if can_ground || p.jumps_used < max_jumps {
            p.vel_r = PLAYER_JUMP_VEL * run.stats.jump_height.sqrt();
            if !can_ground {
                p.jumps_used += 1;
            } else {
                p.jumps_used = 1;
            }
            p.grounded = false;
            p.coyote = 0.0;
        }
    }

    // slide
    if (keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::KeyC))
        && p.slide_cd <= 0.0
        && p.grounded
    {
        p.slide_timer = SLIDE_TIME;
        p.slide_cd = SLIDE_COOLDOWN;
        let boost_dir = if wish != Vec3::ZERO { wish } else { p.facing };
        p.vel_t = boost_dir * max_speed.max(p.vel_t.length()).max(PLAYER_RUN_SPEED * speed_mult * SLIDE_BOOST);
        if let Passive::SlideFrenzy { secs, .. } = run.character.def().passive {
            run.frenzy_timer = secs;
        }
    }

    if wish != Vec3::ZERO {
        p.facing = wish;
    } else if p.vel_t.length() > 0.5 {
        p.facing = p.vel_t.normalize();
    }

    let _ = planet; // used by physics
}

/// Integrate motion over the sphere, snap to terrain, drive the transform.
pub fn player_physics(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut q: Query<(&mut Player, &mut Transform)>,
) {
    let Ok((mut p, mut tf)) = q.single_mut() else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }

    p.slide_timer = (p.slide_timer - dt).max(0.0);
    p.slide_cd = (p.slide_cd - dt).max(0.0);
    p.coyote = (p.coyote - dt).max(0.0);
    p.land_timer += dt;
    run.frenzy_timer = (run.frenzy_timer - dt).max(0.0);

    // gravity
    p.vel_r -= PLAYER_GRAVITY * dt;

    // advance over the sphere at current radius
    let r = planet.surface(p.dir) + p.height;
    let (new_dir, new_vel) = sphere::advance(p.dir, p.vel_t, r, dt);
    p.dir = new_dir;
    p.vel_t = new_vel;
    p.height += p.vel_r * dt;

    // terrain contact
    if p.height <= 0.0 {
        if !p.grounded {
            p.land_timer = 0.0;
        }
        p.height = 0.0;
        p.vel_r = 0.0;
        p.grounded = true;
        p.jumps_used = 0;
        p.coyote = 0.12;
    } else if p.height > 0.02 {
        p.grounded = false;
    }

    let up = p.dir;
    let pos = planet.surface_point(p.dir) + up * (p.height + PLAYER_HEIGHT * 0.5);
    tf.translation = pos;
    let lean = if p.slide_timer > 0.0 { 0.9 } else { 0.0 };
    tf.rotation = sphere::frame_quat(up, p.facing) * Quat::from_rotation_x(-lean);
}

/// Mouse-orbit chase camera aligned to the local vertical.
pub fn camera_rig(
    time: Res<Time<Real>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut rig: ResMut<CamRig>,
    shake: Res<Shake>,
    planet: Res<CurrentPlanet>,
    phase: Res<RunPhase>,
    q_player: Query<(&Player, &Transform), Without<PlayerRig>>,
    mut q_cam: Query<&mut Transform, With<PlayerRig>>,
) {
    let Ok((p, ptf)) = q_player.single() else { return };
    let Ok(mut cam) = q_cam.single_mut() else { return };

    if *phase == RunPhase::Playing {
        // positive yaw turns left on the sphere frame, so mouse-right must subtract
        rig.yaw -= mouse.delta.x * CAM_SENS;
        rig.pitch = (rig.pitch + mouse.delta.y * CAM_SENS).clamp(0.12, 1.25);
    }

    let up = p.dir;
    let (t, b) = sphere::tangent_frame(up);
    let flat = (t * rig.yaw.cos() + b * rig.yaw.sin()).normalize();

    let dist = CAM_DISTANCE;
    let back = -flat * (dist * rig.pitch.cos());
    let lift = up * (dist * rig.pitch.sin() + CAM_HEIGHT * 0.4);

    let target_pos = ptf.translation + back + lift;
    let look_at = ptf.translation + up * 1.2 + flat * 2.0;

    let k = 1.0 - (-CAM_STIFFNESS * time.delta_secs()).exp();
    let mut pos = cam.translation.lerp(target_pos, k);

    // keep the camera out of the ground (mountains included)
    let cam_dir = pos.normalize_or_zero();
    let min_r = planet.surface(cam_dir) + 1.2;
    if pos.length() < min_r {
        pos = cam_dir * min_r;
    }

    // Aim from the UNSHAKEN position so shake never becomes rotational jitter,
    // and never re-aim across a degenerate (near-zero) look vector.
    cam.translation = pos;
    if (look_at - pos).length_squared() > 0.25 {
        cam.look_at(look_at, up);
    }

    // positional-only screenshake, applied after aiming
    let tr = shake.trauma * shake.trauma;
    if tr > 0.001 {
        let t = (time.elapsed_secs() % 60.0) * 33.0;
        cam.translation += (t.sin() * 0.12 + (t * 1.7).cos() * 0.09) * tr * flat.cross(up)
            + ((t * 1.3).cos() * 0.10) * tr * up;
    }
}

/// Lock the cursor while playing, free it for menus/panels.
pub fn cursor_control(
    phase: Res<RunPhase>,
    state: Res<State<crate::AppState>>,
    mut cursors: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok(mut c) = cursors.single_mut() else { return };
    let lock = *state.get() == crate::AppState::InRun && *phase == RunPhase::Playing;
    let desired = if lock { CursorGrabMode::Locked } else { CursorGrabMode::None };
    if c.grab_mode != desired {
        c.grab_mode = desired;
        c.visible = !lock;
    }
}

/// HP regen, shield recharge, iframe + powerup decay.
pub fn player_upkeep(time: Res<Time>, mut run: ResMut<RunState>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    if run.hp > 0.0 {
        let regen = run.stats.regen / 60.0;
        run.hp = (run.hp + regen * dt).min(run.stats.max_hp);
    }
    run.iframes = (run.iframes - dt).max(0.0);
    run.shield_cd = (run.shield_cd - dt).max(0.0);
    if run.shield_cd <= 0.0 && run.shield < run.stats.shield {
        run.shield = (run.shield + run.stats.shield * 0.35 * dt).min(run.stats.shield);
    }
    let mut expired = false;
    for pu in run.powerups.iter_mut() {
        pu.1 -= dt;
        if pu.1 <= 0.0 {
            expired = true;
        }
    }
    if expired {
        run.powerups.retain(|(_, t)| *t > 0.0);
    }
}
