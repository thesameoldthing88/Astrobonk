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
    // --- animation state (see the code-art-animation skill) ---
    pub stride: f32,     // gait phase, advanced by DISTANCE so feet don't skate
    pub gait_amp: f32,   // smoothed 0..1 blend between idle and full walk
    pub squash: f32,     // landing-squash timer (counts up to ~0.25)
    pub squash_amt: f32, // how hard the landing was
    pub lean: f32,       // smoothed lean-into-acceleration
}

#[derive(Component)]
pub struct PlayerRig; // camera

/// Which limb a joint drives (hero-tier rig — see the code-art-animation skill).
#[derive(Clone, Copy, PartialEq)]
pub enum Limb {
    ArmL,
    ArmR,
    LegL,
    LegR,
    Head,
    Body,
}

/// An animated joint: we always compose from `rest`, never accumulate onto the
/// live transform (the golden rule — otherwise it drifts and explodes).
#[derive(Component)]
pub struct Joint {
    pub limb: Limb,
    pub rest: Transform,
    /// smoothed drag value, used by the head/body lag layer
    pub lag: f32,
}

#[derive(Resource)]
pub struct CamRig {
    /// Persistent world-space forward (tangent to the sphere). Carried along as the
    /// player moves so a fixed aim stays fixed — no per-frame tangent-frame drift/flip.
    pub forward: Vec3,
    pub pitch: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self { forward: Vec3::NEG_Z, pitch: 0.55 }
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
                stride: 0.0,
                gait_amp: 0.0,
                squash: 1.0,
                squash_amt: 0.0,
                lean: 0.0,
            },
            Transform::from_translation(pos),
            Visibility::default(),
            StageScoped,
        ))
        .with_children(|p| {
            // ---- BODY joint: torso + backpack ride here so they can bob/lean ----
            let body_rest = Transform::from_xyz(0.0, 0.0, 0.0);
            p.spawn((
                Joint { limb: Limb::Body, rest: body_rest, lag: 0.0 },
                body_rest,
                Visibility::default(),
            ))
            .with_children(|b| {
                b.spawn((
                    Mesh3d(meshes.add(astronaut_torso_mesh())),
                    MeshMaterial3d(suit.clone()),
                    Transform::IDENTITY,
                ));
                b.spawn((
                    Mesh3d(meshes.add(backpack_mesh())),
                    MeshMaterial3d(pack),
                    Transform::from_xyz(0.0, 0.18, 0.0),
                ));
            });

            // ---- HEAD joint: helmet + visor (gets the drag/scan layer) ----
            let head_rest = Transform::from_xyz(0.0, 0.7, 0.0);
            p.spawn((
                Joint { limb: Limb::Head, rest: head_rest, lag: 0.0 },
                head_rest,
                Visibility::default(),
            ))
            .with_children(|h| {
                h.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.3)))),
                    MeshMaterial3d(suit.clone()),
                    Transform::IDENTITY,
                ));
                h.spawn((
                    Mesh3d(meshes.add(Mesh::from(Sphere::new(0.24)))),
                    MeshMaterial3d(visor),
                    Transform::from_xyz(0.0, 0.02, -0.16).with_scale(Vec3::new(1.05, 0.85, 0.7)),
                ));
            });

            // ---- LIMB joints: arms pivot at shoulders, legs at hips ----
            let arm_mesh = meshes.add(astronaut_arm_mesh());
            let leg_mesh = meshes.add(astronaut_leg_mesh());
            for (limb, x, y, mesh) in [
                (Limb::ArmL, -0.34, 0.42, arm_mesh.clone()),
                (Limb::ArmR, 0.34, 0.42, arm_mesh),
                (Limb::LegL, -0.15, -0.3, leg_mesh.clone()),
                (Limb::LegR, 0.15, -0.3, leg_mesh),
            ] {
                // arms rest with a slight outward splay (asymmetry sells life)
                let rest = Transform::from_xyz(x, y, 0.0).with_rotation(
                    if matches!(limb, Limb::ArmL | Limb::ArmR) {
                        Quat::from_rotation_z(if x < 0.0 { 0.12 } else { -0.12 })
                    } else {
                        Quat::IDENTITY
                    },
                );
                p.spawn((
                    Joint { limb, rest, lag: 0.0 },
                    Mesh3d(mesh),
                    MeshMaterial3d(suit.clone()),
                    rest,
                ));
            }
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

/// The astronaut's TORSO only — limbs are separate joint entities so they can be
/// animated (hero-tier rig, per the code-art-animation skill). Suit material
/// multiplies the vertex colors: WHITE = suit tint, darker = joints/panel.
fn astronaut_torso_mesh() -> Mesh {
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
    // shoulder caps (the arm joints pivot inside these)
    for s in [-1.0, 1.0] {
        m.add_sphere(0.16, 1, at(Vec3::new(0.31 * s, 0.42, 0.0)), joint);
    }
    m.build()
}

/// One arm, built pivoting at the SHOULDER (origin) and hanging down -Y so a
/// rotation about X swings it fore-aft correctly.
fn astronaut_arm_mesh() -> Mesh {
    use crate::meshkit::at;
    let body = Color::WHITE;
    let joint = Color::srgb(0.5, 0.5, 0.56);
    let mut m = crate::meshkit::MeshData::new();
    m.add_cylinder(0.1, 0.36, 8, at(Vec3::new(0.0, -0.19, 0.0)), body); // upper arm
    m.add_sphere(0.095, 1, at(Vec3::new(0.0, -0.38, 0.0)), joint); // elbow
    m.add_cylinder(0.09, 0.34, 8, at(Vec3::new(0.0, -0.55, 0.0)), body); // forearm
    m.add_sphere(0.11, 1, at(Vec3::new(0.0, -0.75, 0.0)), joint); // glove
    m.build()
}

/// One leg, pivoting at the HIP (origin), hanging down -Y.
fn astronaut_leg_mesh() -> Mesh {
    use crate::meshkit::at;
    let body = Color::WHITE;
    let joint = Color::srgb(0.5, 0.5, 0.56);
    let dark = Color::srgb(0.30, 0.30, 0.36);
    let mut m = crate::meshkit::MeshData::new();
    m.add_cylinder(0.13, 0.36, 8, at(Vec3::new(0.0, -0.18, 0.0)), body); // thigh
    m.add_sphere(0.12, 1, at(Vec3::new(0.0, -0.37, 0.0)), joint); // knee
    m.add_cylinder(0.11, 0.32, 8, at(Vec3::new(0.0, -0.54, 0.0)), body); // shin
    m.add_box(Vec3::new(0.2, 0.13, 0.32), at(Vec3::new(0.0, -0.72, -0.06)), dark); // boot
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
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    rig: Res<CamRig>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    particles: Option<Res<crate::fx::ParticleAssets>>,
    mut sfx: MessageWriter<crate::messages::SfxMsg>,
    mut q: Query<(&mut Player, &Transform)>,
) {
    let Ok((mut p, ptf)) = q.single_mut() else { return };
    let dt = time.delta_secs();

    // Use the camera's persistent forward (reprojected onto the current tangent plane)
    // so movement always matches where the camera looks.
    let up = p.dir;
    let mut fwd = rig.forward - up * rig.forward.dot(up);
    if fwd.length_squared() < 1e-6 {
        fwd = sphere::tangent_frame(up).0;
    }
    let fwd = fwd.normalize();
    let right = fwd.cross(up).normalize();

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
            // successful bunny-hop: jumped inside the landing window with speed kept
            if can_ground
                && p.land_timer <= BHOP_WINDOW
                && p.vel_t.length() > PLAYER_RUN_SPEED * speed_mult * 1.05
            {
                sfx.write(crate::messages::SfxMsg(crate::messages::Sfx::Bhop));
                if let Some(pa) = &particles {
                    crate::fx::burst(
                        &mut commands,
                        pa,
                        ptf.translation - p.dir * 0.7,
                        p.dir,
                        crate::fx::Pcolor::Cyan,
                        4,
                        2.5,
                    );
                }
            }
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
        // feel: whoosh + a kick of dust at the feet
        sfx.write(crate::messages::SfxMsg(crate::messages::Sfx::Slide));
        if let Some(pa) = &particles {
            crate::fx::burst(
                &mut commands,
                pa,
                ptf.translation - p.dir * 0.7,
                p.dir,
                crate::fx::Pcolor::White,
                6,
                3.0,
            );
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
    props: Res<crate::planet::PropColliders>,
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
    // above base run speed? (Nova's "no cooldown while sprinting")
    run.fast_move = p.vel_t.length() > PLAYER_RUN_SPEED * run.move_speed_mult() * 1.08;

    // gravity
    p.vel_r -= PLAYER_GRAVITY * dt;

    // advance over the sphere at current radius
    let r = planet.surface(p.dir) + p.height;
    let (new_dir, new_vel) = sphere::advance(p.dir, p.vel_t, r, dt);
    p.dir = new_dir;
    p.vel_t = new_vel;
    p.height += p.vel_r * dt;

    // solid props: push out of rocks/boulders/wrecks/beacons and kill the
    // velocity component heading into them (so you slide along, not stick).
    if let Some(fixed) = props.resolve(p.dir, p.height, PLAYER_RADIUS, planet.radius) {
        let push = (fixed - p.dir).normalize_or_zero();
        p.dir = fixed;
        if push != Vec3::ZERO {
            let push_t = (push - p.dir * push.dot(p.dir)).normalize_or_zero();
            let into = p.vel_t.dot(push_t);
            if into < 0.0 {
                p.vel_t -= push_t * into; // remove only the inward component
            }
        }
    }

    // terrain contact
    if p.height <= 0.0 {
        if !p.grounded {
            p.land_timer = 0.0;
            // landing squash scaled by impact speed [recipe R5]
            p.squash_amt = (p.vel_r.abs() * 0.022).clamp(0.05, 0.28);
            p.squash = 0.0;
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

/// The astronaut animator — the code-art-animation skill made real.
/// Layered per the rig rule: start from each joint's REST pose, then add
/// (1) breathing idle [R1], (2) the distance-driven walk cycle [R2],
/// (3) lean-into-acceleration [R3], (4) landing squash [R5], (5) head drag.
/// Never accumulates onto live transforms.
pub fn animate_player(
    time: Res<Time>,
    mut q_player: Query<(&mut Player, &Children)>,
    mut q_joints: Query<(&mut Joint, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let Ok((mut p, children)) = q_player.single_mut() else { return };
    let t = time.elapsed_secs();

    // --- drive the gait by DISTANCE travelled (feet don't skate) ---
    let speed = p.vel_t.length();
    let stride_len = 2.1;
    p.stride = (p.stride + speed * dt / stride_len * std::f32::consts::TAU) % std::f32::consts::TAU;

    // blend gait in/out smoothly (framerate-independent smoothing)
    let target_amp = if p.grounded { (speed / PLAYER_RUN_SPEED).clamp(0.0, 1.15) } else { 0.0 };
    let k = 1.0 - (-9.0 * dt).exp();
    p.gait_amp += (target_amp - p.gait_amp) * k;
    let amp = p.gait_amp;

    // sliding: tuck the limbs instead of walking
    let slide = if p.slide_timer > 0.0 { 1.0 } else { 0.0 };
    let slide_k = 1.0 - (-14.0 * dt).exp();
    p.lean += (slide - p.lean) * slide_k;
    let tuck = p.lean;

    // landing squash timer [R5]
    p.squash += dt;
    let squash_scale = if p.squash < 0.25 && p.squash_amt > 0.0 {
        let s = p.squash;
        if s < 0.07 {
            1.0 - p.squash_amt * (s / 0.07)
        } else {
            let e = (s - 0.07) / 0.18;
            // ease-out-back: overshoot slightly past 1.0 then settle
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            let eb = 1.0 + c3 * (e - 1.0).powi(3) + c1 * (e - 1.0).powi(2);
            1.0 - p.squash_amt * (1.0 - eb)
        }
    } else {
        1.0
    };

    // airborne stretch: lengthen along the fall/rise axis
    let air_stretch = if !p.grounded { 1.0 + (p.vel_r.abs() * 0.012).min(0.14) } else { 1.0 };

    let lp = p.stride; // left phase
    let rp = p.stride + std::f32::consts::PI; // right (anti-phase)
    let breath = (t * 2.1).sin();

    for child in children.iter() {
        let Ok((joint, mut tf)) = q_joints.get_mut(child) else { continue };
        // ALWAYS start from rest
        *tf = joint.rest;
        match joint.limb {
            Limb::LegL | Limb::LegR => {
                let ph = if joint.limb == Limb::LegL { lp } else { rp };
                // swing fore-aft; tuck up when sliding
                tf.rotation *= Quat::from_rotation_x(ph.sin() * 0.85 * amp - tuck * 0.9);
                // lift only during the swing half of the cycle
                tf.translation.y += ph.sin().max(0.0) * 0.15 * amp;
            }
            Limb::ArmL | Limb::ArmR => {
                // arms swing ANTI-phase to the leg on the same side
                let ph = if joint.limb == Limb::ArmL { rp } else { lp };
                tf.rotation *= Quat::from_rotation_x(ph.sin() * 0.62 * amp + tuck * 0.5);
                // subtle idle sway when standing still
                tf.rotation *= Quat::from_rotation_z((t * 1.3).sin() * 0.03 * (1.0 - amp));
            }
            Limb::Body => {
                // bob twice per cycle (lowest at each footfall) + waddle roll
                tf.translation.y += (1.0 - lp.sin().abs()) * 0.09 * amp;
                tf.rotation *= Quat::from_rotation_z((p.stride * 0.5).sin() * 0.075 * amp);
                // lean forward into the run, deeper while sliding
                tf.rotation *= Quat::from_rotation_x(-0.12 * amp - tuck * 0.5);
                // breathing [R1] + landing squash [R5] + air stretch, volume-ish preserved
                let sy = squash_scale * air_stretch * (1.0 + breath * 0.02 * (1.0 - amp));
                tf.scale = Vec3::new(1.0 / sy.sqrt(), sy, 1.0 / sy.sqrt());
            }
            Limb::Head => {
                // counter-bob so the helmet stays steadier than the body (drag layer)
                tf.translation.y += (1.0 - lp.sin().abs()) * 0.02 * amp;
                tf.rotation *= Quat::from_rotation_x(0.06 * amp + tuck * 0.35);
                // slow idle scan when standing still — a model at rest is a statue
                tf.rotation *= Quat::from_rotation_y((t * 0.55).sin() * 0.22 * (1.0 - amp));
            }
        }
    }
}

/// Mouse-orbit chase camera aligned to the local vertical.
pub fn camera_rig(
    time: Res<Time<Real>>,
    mouse: Res<AccumulatedMouseMotion>,
    mut rig: ResMut<CamRig>,
    shake: Res<Shake>,
    planet: Res<CurrentPlanet>,
    phase: Res<RunPhase>,
    save: Res<crate::save::MetaSave>,
    q_player: Query<(&Player, &Transform), Without<PlayerRig>>,
    mut q_cam: Query<&mut Transform, With<PlayerRig>>,
    mut q_proj: Query<&mut Projection, With<PlayerRig>>,
) {
    let Ok((p, ptf)) = q_player.single() else { return };
    let Ok(mut cam) = q_cam.single_mut() else { return };

    // FOV punch while sliding — speed you can feel
    if let Ok(mut proj) = q_proj.single_mut() {
        if let Projection::Perspective(pp) = &mut *proj {
            let base = std::f32::consts::FRAC_PI_4;
            let target = if p.slide_timer > 0.0 { base * 1.09 } else { base };
            let k = 1.0 - (-10.0 * time.delta_secs()).exp();
            pp.fov += (target - pp.fov) * k;
        }
    }

    let up = p.dir;

    // Parallel-transport the persistent forward onto the current tangent plane
    // (as the player moves, `up` changes; keep `forward` tangent without twisting it).
    let mut fwd = rig.forward - up * rig.forward.dot(up);
    if fwd.length_squared() < 1e-6 {
        fwd = sphere::tangent_frame(up).0;
    }
    fwd = fwd.normalize();

    // Mouse turns the persistent forward directly — no per-frame basis, no pole flip.
    let sens = CAM_SENS * save.sensitivity;
    if *phase == RunPhase::Playing {
        fwd = Quat::from_axis_angle(up, -mouse.delta.x * sens) * fwd;
        fwd = (fwd - up * fwd.dot(up)).normalize();
        rig.pitch = (rig.pitch + mouse.delta.y * sens).clamp(0.12, 1.25);
    }
    rig.forward = fwd;

    let dist = CAM_DISTANCE;
    let back = -fwd * (dist * rig.pitch.cos());
    let lift = up * (dist * rig.pitch.sin() + CAM_HEIGHT * 0.4);

    // Clamp the TARGET above terrain (pre-lerp) so ground clearance is smoothed by the
    // easing instead of popping the final position.
    let mut target_pos = ptf.translation + back + lift;
    let tdir = target_pos.normalize_or_zero();
    if tdir != Vec3::ZERO {
        let min_r = planet.surface(tdir) + 1.2;
        if target_pos.length() < min_r {
            target_pos = tdir * min_r;
        }
    }

    let k = 1.0 - (-CAM_STIFFNESS * time.delta_secs()).exp();
    let pos = cam.translation.lerp(target_pos, k);

    // Aim from the UNSHAKEN position so shake never becomes rotational jitter,
    // and never re-aim across a degenerate (near-zero) look vector.
    cam.translation = pos;
    let look_at = ptf.translation + up * 1.2 + fwd * 2.0;
    if (look_at - pos).length_squared() > 0.25 {
        cam.look_at(look_at, up);
    }

    // positional-only screenshake, applied after aiming (scaled by the settings slider)
    let tr = shake.trauma * shake.trauma * save.shake_scale;
    if tr > 0.001 {
        let t = (time.elapsed_secs() % 60.0) * 33.0;
        cam.translation += (t.sin() * 0.12 + (t * 1.7).cos() * 0.09) * tr * fwd.cross(up)
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
    run.reticle_timer = (run.reticle_timer + dt) % 1.5;
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
