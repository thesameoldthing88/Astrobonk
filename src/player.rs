//! The astronaut: spherical-gravity kinematic controller (walk / jump / slide / bunny-hop)
//! and the orbiting third-person camera rig.

use crate::config::*;
use crate::content::characters::Passive;
use crate::fx::Shake;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::run::{PlayerState, RunPhase, RunState};
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

/// Which player this astronaut belongs to. 0 = local/host; 1.. = joined peers.
/// The netcode layer replicates inputs and state keyed on this id.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct PlayerId(pub u8);

/// Marks the astronaut this machine drives (exactly one, even in co-op).
#[derive(Component)]
pub struct LocalPlayer;

/// A cheap copy of one astronaut's position, taken once per system before any mutable
/// loop. Enemy systems iterate up to ENEMY_CAP entities, and Bevy forbids reading the
/// player query inside a `&mut` loop over an overlapping archetype (B0001) — so every
/// "who do I target?" site snapshots into a Vec first.
#[derive(Clone, Copy)]
pub struct AstronautSnap {
    pub entity: Entity,
    pub dir: Vec3,
    pub pos: Vec3,
}

/// The astronaut closest to `from_dir` along the surface.
/// GREAT-CIRCLE distance, never `Vec3::distance`: on a 105-160m planet the straight-line
/// chord badly under-reads for anything past a short hop, so two players on opposite sides
/// would compare wrongly.
pub fn nearest_astronaut(from_dir: Vec3, list: &[AstronautSnap], radius: f32) -> Option<AstronautSnap> {
    list.iter().copied().min_by(|a, b| {
        crate::sphere::arc_dist(from_dir, a.dir, radius)
            .total_cmp(&crate::sphere::arc_dist(from_dir, b.dir, radius))
    })
}

/// One frame of movement intent for an astronaut. The LOCAL player's is filled from
/// keyboard/mouse; a REMOTE player's is filled from their PlayerInputMsg on the host.
/// Movement then consumes this identically either way, so there is exactly one
/// authoritative copy of the movement rules.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct InputIntent {
    pub wish: Vec3,      // desired move dir, world-space tangent, normalized
    pub forward: Vec3,   // camera forward (tangent) — drives facing/aim
    pub jump: bool,      // edge-triggered (true only on the frame pressed)
    pub slide: bool,     // edge-triggered
    pub interact: bool,  // edge-triggered
    pub jump_held: bool, // level: jump is down this frame (Anti-Grav Boots hover)
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
    /// The local body followed last frame and where it stood. A NEW body (each stage spawns
    /// one) is picked up as it is; the same body jumping metres in one frame was teleported.
    pub last_body: Option<(Entity, Vec3)>,
    /// The point the rig centred on last frame: the body, or partway along a glide.
    pub focus: Vec3,
    /// A teleport glide in flight: the focus it left from, and seconds into it.
    pub glide: Option<(Vec3, f32)>,
}

impl Default for CamRig {
    fn default() -> Self {
        Self { forward: Vec3::NEG_Z, pitch: 0.55, last_body: None, focus: Vec3::ZERO, glide: None }
    }
}

/// Where a teleport glide's focus is, `s` (0..1, eased) of the way from `from` to `to`:
/// along the great circle between them with the radius eased, so the frame sweeps over the
/// ground instead of cutting through the planet. An exact antipode has no unique great
/// circle, so that one goes forward over the top, the way the camera was looking.
fn glide_point(from: Vec3, to: Vec3, s: f32, forward: Vec3) -> Vec3 {
    let (a, b) = (from.normalize_or_zero(), to.normalize_or_zero());
    let axis = a.cross(b).try_normalize().unwrap_or_else(|| {
        let f = (forward - a * forward.dot(a)).try_normalize().unwrap_or_else(|| sphere::tangent_frame(a).0);
        a.cross(f).normalize()
    });
    let dir = Quat::from_axis_angle(axis, a.angle_between(b) * s) * a;
    dir * (from.length() + (to.length() - from.length()) * s)
}

pub fn spawn_player(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    planet: &CurrentPlanet,
    run: &RunState,
    save: &crate::save::MetaSave,
    id: u8,
    character: crate::content::characters::AstronautKind,
    is_local: bool,
    // Carried progression, when this astronaut already existed (a stage change). `None`
    // starts a fresh sheet. Without this, every player's build is wiped on teleport.
    carried: Option<PlayerState>,
) {
    let def = character.def();
    // fan players out around the drop point so they don't spawn inside each other
    let dir = if id == 0 {
        Vec3::Y
    } else {
        let (t, b) = sphere::tangent_frame(Vec3::Y);
        let a = id as f32 * 1.7;
        crate::sphere::offset_dir(Vec3::Y, (t * a.cos() + b * a.sin()).normalize(), 3.5, planet.radius)
    };
    let pos = planet.surface_point(dir) + dir * PLAYER_HEIGHT;

    let root = commands
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
            // a carried sheet arrives through a teleporter: refill its per-stage grants
            carried
                .map(|mut ps| {
                    ps.enter_stage();
                    ps
                })
                .unwrap_or_else(|| PlayerState::new(character, save)),
            PlayerId(id),
            InputIntent::default(),
            RigHero(character),
            crate::comet::CometState::default(),
            crate::items::ItemProcs::default(),
            // what crosses the wire (bundled: a flat tuple would pass Bevy's 15-element cap)
            (
                crate::net::NetTransform { dir, height: 0.0, facing: sphere::tangent_frame(dir).0, sliding: false },
                crate::net::PlayerVitals { hp: 0.0, max_hp: 0.0, level: 1, down: false, revives: 0 },
                crate::net::NetHero(crate::net::hero_code(character)),
                crate::net::NetComet::default(),
                crate::net::NetItemVis::default(),
                bevy_replicon::prelude::Replicated,
            ),
            Transform::from_translation(pos),
            Visibility::default(),
            StageScoped,
        ))
        .insert_if(LocalPlayer, || is_local)
        .id();
    build_astronaut_rig(commands, root, meshes, materials, def.suit, def.visor);
}

/// Which hero's suit an astronaut's rig was built in. Compared against the sheet by
/// `refit_astronaut_rigs`, because on a co-op host a peer is seated before its first build
/// heartbeat says which hero it plays.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct RigHero(pub crate::content::characters::AstronautKind);

/// Rebuild an astronaut's rig in its hero's suit whenever the sheet's hero and the rig
/// disagree. Every child of an astronaut is rig, so the swap is clear-and-rebuild.
pub fn refit_astronaut_rigs(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &PlayerState, &mut RigHero)>,
) {
    for (e, ps, mut rig) in &mut q {
        if rig.0 == ps.character {
            continue;
        }
        rig.0 = ps.character;
        let def = ps.character.def();
        commands.entity(e).despawn_related::<Children>();
        build_astronaut_rig(&mut commands, e, &mut meshes, &mut materials, def.suit, def.visor);
    }
}

/// Build the astronaut's VISUAL rig as children of `root`: torso, helmet, four animated
/// limbs, tool and flashlight. Deliberately separate from `spawn_player` so a networked
/// remote player can wear the same look WITHOUT inheriting `Player`, `PlayerState` or
/// `StageScoped` — a replicated entity is owned by the server, and giving it a `Player`
/// would both stomp its network-driven transform in `player_physics` and break the many
/// `.single()` player queries across the codebase.
pub fn build_astronaut_rig(
    commands: &mut Commands,
    root: Entity,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    suit_color: Color,
    visor_color: Color,
) {
    let suit = materials.add(StandardMaterial {
        base_color: suit_color,
        perceptual_roughness: 0.7,
        ..default()
    });
    let visor = materials.add(StandardMaterial {
        base_color: visor_color,
        emissive: visor_color.to_linear() * 1.2,
        perceptual_roughness: 0.15,
        ..default()
    });
    let pack = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.85),
        perceptual_roughness: 0.9,
        ..default()
    });

    commands.entity(root).with_children(|p| {
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
                    // Tome of Nightfall scales these (`tomes::apply_flashlights`)
                    intensity: FLASHLIGHT_INTENSITY,
                    range: FLASHLIGHT_RANGE,
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
/// Read keyboard/mouse into the LOCAL astronaut's intent. This is the only place
/// hardware input is read; everything downstream consumes `InputIntent`.
pub fn gather_local_input(
    keys: Res<ButtonInput<KeyCode>>,
    rig: Res<CamRig>,
    mut q: Query<(&Player, &mut InputIntent), With<LocalPlayer>>,
) {
    let Ok((p, mut intent)) = q.single_mut() else { return };
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
    intent.wish = wish.normalize_or_zero();
    intent.forward = fwd;
    intent.jump = keys.just_pressed(KeyCode::Space);
    intent.jump_held = keys.pressed(KeyCode::Space);
    intent.slide = keys.just_pressed(KeyCode::ControlLeft) || keys.just_pressed(KeyCode::KeyC);
    intent.interact = keys.just_pressed(KeyCode::KeyE);
}

/// Apply intent -> motion for EVERY astronaut we simulate (all of them on the host;
/// just the local one on a client, as prediction).
pub fn player_input(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    particles: Option<Res<crate::fx::ParticleAssets>>,
    mut sfx: MessageWriter<crate::messages::SfxMsg>,
    mut q: Query<(&mut Player, &mut PlayerState, &Transform, &InputIntent)>,
) {
    for (mut p, mut run, ptf, intent) in &mut q {
    let dt = time.delta_secs();
    let wish = intent.wish;

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
    if intent.jump {
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
    if intent.slide && p.slide_cd <= 0.0 && p.grounded {
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
    }
    let _ = planet; // used by physics
}

/// Integrate motion over the sphere, snap to terrain, drive the transform.
///
/// Also where the movement-side items and tomes live, because this runs on every body a
/// machine moves (the host: all; a client: its own, predicted) and so the owner's feel and
/// the host's truth come out of the same code: Anti-Grav Boots' hover, Tome of Gravity's
/// fall, and the `airborne` / `descent_m` / `night` / `momentum` readings Icarus Boots,
/// Downhill Momentum and the Nightfall and Momentum tomes deal damage from.
#[allow(clippy::too_many_arguments)]
pub fn player_physics(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    props: Res<crate::planet::PropColliders>,
    global: Res<RunState>,
    mut telemetry: ResMut<crate::items::ItemTelemetry>,
    mut tome_tel: ResMut<crate::tomes::TomeTelemetry>,
    mut q: Query<(&mut Player, &mut PlayerState, &mut crate::items::ItemProcs, &InputIntent, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (mut p, mut run, mut procs, intent, mut tf) in &mut q {
    p.slide_timer = (p.slide_timer - dt).max(0.0);
    p.slide_cd = (p.slide_cd - dt).max(0.0);
    p.coyote = (p.coyote - dt).max(0.0);
    p.land_timer += dt;
    run.frenzy_timer = (run.frenzy_timer - dt).max(0.0);
    // above base run speed? (Nova's "no cooldown while sprinting")
    run.fast_move = p.vel_t.length() > PLAYER_RUN_SPEED * run.move_speed_mult() * 1.08;

    // Anti-Grav Boots: holding jump once the rise is spent holds the altitude — gravity
    // simply stops for as long as the airtime's budget lasts.
    procs.hovering = !p.grounded
        && intent.jump_held
        && procs.hover_left > 0.0
        && p.vel_r <= 0.0
        && !run.dead
        && run.has_item(crate::content::items::ItemKind::AntiGravBoots);
    if procs.hovering {
        procs.hover_left -= dt;
        p.vel_r = 0.0;
        telemetry.hover_secs += dt;
    } else {
        // gravity — Tome of Gravity pulls harder on the way down only, so the jump's apex
        // (the air builds' firing window) is kept and the landing comes sooner
        let fall = if p.vel_r < 0.0 { run.stats.fall_speed.max(0.1) } else { 1.0 };
        p.vel_r -= PLAYER_GRAVITY * fall * dt;
        if fall > 1.0 {
            tome_tel.fast_fall_secs += dt;
        }
    }

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
        procs.hover_left = ANTIGRAV_HOVER_SECS;
    } else if p.height > 0.02 {
        p.grounded = false;
    }
    run.airborne = !p.grounded;
    if run.airborne {
        telemetry.airborne_secs += dt;
    }
    // altitude above the core, so a crater slope and a fall both count as descent
    run.descent_m = procs.track_descent(planet.surface(p.dir) + p.height, dt);
    telemetry.max_descent = telemetry.max_descent.max(run.descent_m);
    // Tome of Nightfall reads the side of the planet we stand on…
    run.night = crate::planet::is_night(p.dir, global.sun_shrink);
    if run.night && run.stats.night_damage > 0.0 {
        tome_tel.night_secs += dt;
    }
    // …and Tome of Momentum how long we have kept moving (airborne counts: a bunny-hop
    // chain is the purest momentum there is)
    run.momentum = if p.vel_t.length() >= MOMENTUM_MIN_SPEED && !run.dead {
        (run.momentum + dt).min(MOMENTUM_RAMP_SECS)
    } else {
        (run.momentum - dt * MOMENTUM_DRAIN).max(0.0)
    };
    tome_tel.max_momentum_bonus = tome_tel.max_momentum_bonus.max(run.momentum_bonus());

    let up = p.dir;
    let pos = planet.surface_point(p.dir) + up * (p.height + PLAYER_HEIGHT * 0.5);
    tf.translation = pos;
    let lean = if p.slide_timer > 0.0 { 0.9 } else { 0.0 };
    tf.rotation = sphere::frame_quat(up, p.facing) * Quat::from_rotation_x(-lean);
    }
}

/// Animation STATE for one astronaut rig. Held by `Player` for the local astronaut and by
/// `RemoteAstronaut` for networked ones, so both wear the identical walk cycle.
#[derive(Default, Clone, Copy)]
pub struct RigAnim {
    pub stride: f32,
    pub gait_amp: f32,
    pub squash: f32,
    pub squash_amt: f32,
    pub lean: f32,
}

/// What the rig is DOING this frame. Locally this comes from physics; for a remote player
/// it is finite-differenced from replicated motion.
#[derive(Clone, Copy)]
pub struct RigDrive {
    pub speed: f32,
    pub grounded: bool,
    pub sliding: bool,
    pub vel_r: f32,
}

/// The astronaut animator, independent of where the motion came from.
/// Layered per the rig rule: start from each joint's REST pose, then add
/// (1) breathing idle [R1], (2) the distance-driven walk cycle [R2],
/// (3) lean-into-acceleration [R3], (4) landing squash [R5], (5) head drag.
/// Never accumulates onto live transforms.
pub fn animate_rig(
    a: &mut RigAnim,
    d: RigDrive,
    children: &Children,
    q_joints: &mut Query<(&mut Joint, &mut Transform)>,
    dt: f32,
    t: f32,
) {

    // --- drive the gait by DISTANCE travelled (feet don't skate) ---
    let speed = d.speed;
    let stride_len = 2.1;
    a.stride = (a.stride + speed * dt / stride_len * std::f32::consts::TAU) % std::f32::consts::TAU;

    // blend gait in/out smoothly (framerate-independent smoothing)
    let target_amp = if d.grounded { (speed / PLAYER_RUN_SPEED).clamp(0.0, 1.15) } else { 0.0 };
    let k = 1.0 - (-9.0 * dt).exp();
    a.gait_amp += (target_amp - a.gait_amp) * k;
    let amp = a.gait_amp;

    // sliding: tuck the limbs instead of walking
    let slide = if d.sliding { 1.0 } else { 0.0 };
    let slide_k = 1.0 - (-14.0 * dt).exp();
    a.lean += (slide - a.lean) * slide_k;
    let tuck = a.lean;

    // landing squash timer [R5]
    a.squash += dt;
    let squash_scale = if a.squash < 0.25 && a.squash_amt > 0.0 {
        let s = a.squash;
        if s < 0.07 {
            1.0 - a.squash_amt * (s / 0.07)
        } else {
            let e = (s - 0.07) / 0.18;
            // ease-out-back: overshoot slightly past 1.0 then settle
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            let eb = 1.0 + c3 * (e - 1.0).powi(3) + c1 * (e - 1.0).powi(2);
            1.0 - a.squash_amt * (1.0 - eb)
        }
    } else {
        1.0
    };

    // airborne stretch: lengthen along the fall/rise axis
    let air_stretch = if !d.grounded { 1.0 + (d.vel_r.abs() * 0.012).min(0.14) } else { 1.0 };

    let lp = a.stride; // left phase
    let rp = a.stride + std::f32::consts::PI; // right (anti-phase)
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
                tf.rotation *= Quat::from_rotation_z((a.stride * 0.5).sin() * 0.075 * amp);
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
    let t = time.elapsed_secs();
    for (mut p, children) in &mut q_player {
        let mut a = RigAnim {
            stride: p.stride,
            gait_amp: p.gait_amp,
            squash: p.squash,
            squash_amt: p.squash_amt,
            lean: p.lean,
        };
        let d = RigDrive {
            speed: p.vel_t.length(),
            grounded: p.grounded,
            sliding: p.slide_timer > 0.0,
            vel_r: p.vel_r,
        };
        animate_rig(&mut a, d, children, &mut q_joints, dt, t);
        // squash_amt is read-only to the animator; the impact systems own it.
        p.stride = a.stride;
        p.gait_amp = a.gait_amp;
        p.squash = a.squash;
        p.lean = a.lean;
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
    q_player: Query<(Entity, &Player, &Transform), (With<LocalPlayer>, Without<PlayerRig>)>,
    mut q_cam: Query<&mut Transform, With<PlayerRig>>,
    mut q_proj: Query<&mut Projection, With<PlayerRig>>,
) {
    let Ok((pe, p, ptf)) = q_player.single() else { return };
    let Ok(mut cam) = q_cam.single_mut() else { return };
    let dt = time.delta_secs();

    // FOV punch while sliding — speed you can feel
    if let Ok(mut proj) = q_proj.single_mut() {
        if let Projection::Perspective(pp) = &mut *proj {
            let base = std::f32::consts::FRAC_PI_4;
            let target = if p.slide_timer > 0.0 { base * 1.09 } else { base };
            let k = 1.0 - (-10.0 * time.delta_secs()).exp();
            pp.fov += (target - pp.fov) * k;
        }
    }

    // The point the rig centres on. Normally the body; after a teleport (Dead Man's
    // Tether's rewind, P06's blink, a joiner snapped by the host) it GLIDES there over
    // CAM_TELEPORT_GLIDE_SECS — re-aiming at the new spot in one frame would whip the view
    // round (camera law: never snap). Walking can't trip it: the threshold rides on speed.
    let body = ptf.translation;
    match rig.last_body {
        Some((e, last)) if e == pe => {
            let jump = sphere::arc_dist(last.normalize_or_zero(), body.normalize_or_zero(), planet.radius);
            if jump > CAM_TELEPORT_ARC + p.vel_t.length() * dt * 2.0 {
                rig.glide = Some((rig.focus, 0.0));
            }
        }
        _ => {
            // a new body (stage start): take it where it stands
            rig.focus = body;
            rig.glide = None;
        }
    }
    rig.last_body = Some((pe, body));
    let focus = match rig.glide {
        Some((from, t)) if t + dt < CAM_TELEPORT_GLIDE_SECS => {
            let t = t + dt;
            rig.glide = Some((from, t));
            let s = t / CAM_TELEPORT_GLIDE_SECS;
            glide_point(from, body, s * s * (3.0 - 2.0 * s), rig.forward)
        }
        _ => {
            rig.glide = None;
            body
        }
    };
    let up = focus.normalize_or_zero();
    // Parallel-transport the persistent forward along the focus's move since last frame
    // (a glide can swing it far), then back onto the current tangent plane.
    let prev_up = rig.focus.normalize_or_zero();
    if prev_up != Vec3::ZERO && up != Vec3::ZERO {
        rig.forward = Quat::from_rotation_arc(prev_up, up) * rig.forward;
    }
    rig.focus = focus;

    // Keep the persistent forward on the current tangent plane
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
    let mut target_pos = focus + back + lift;
    let tdir = target_pos.normalize_or_zero();
    if tdir != Vec3::ZERO {
        let min_r = planet.surface(tdir) + 1.2;
        if target_pos.length() < min_r {
            target_pos = tdir * min_r;
        }
    }

    let k = 1.0 - (-CAM_STIFFNESS * dt).exp();
    let pos = cam.translation.lerp(target_pos, k);

    // Aim from the UNSHAKEN position so shake never becomes rotational jitter,
    // and never re-aim across a degenerate (near-zero) look vector.
    cam.translation = pos;
    let look_at = focus + up * 1.2 + fwd * 2.0;
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
pub fn player_upkeep(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut q: Query<&mut PlayerState>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // world difficulty is the max across players (co-op: one Cursed build raises it for all)
    run.difficulty = q.iter().map(|p| p.stats.difficulty).fold(0.0f32, f32::max);
    for mut run in &mut q {
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
}
