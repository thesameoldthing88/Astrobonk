//! Drawing OTHER players on a co-op client.
//!
//! The host simulates every astronaut and replicates a compact `NetTransform` per player.
//! A client receives those as bare entities — `PlayerId` + `NetTransform` + `PlayerVitals`
//! and nothing else — so by default nothing is drawn. This module gives each one the
//! astronaut rig and drives it from the replicated pose.
//!
//! THE CENTRAL RULE: a remote astronaut never gets a `Player` or `PlayerState` component.
//! Two reasons, both load-bearing:
//!   1. `player_physics` iterates every `&mut Player` and overwrites the Transform from
//!      local physics, which would stomp the network-driven pose every frame.
//!   2. Roughly thirty systems locate the player with `.single()` on `Player`/`PlayerState`
//!      (HUD, camera, enemy targeting, pickups, panels). A second match makes those return
//!      `Err(MultipleEntities)` and silently early-return — no panic, no log. Two of them
//!      fail even quieter: the Anubot verdict beam and Craterpillar contact damage just
//!      stop hurting the player while everything else keeps running.
//! So remotes are VISUAL ONLY: a rig, a Transform, and enough finite-differenced motion to
//! animate. That keeps the client's `Player` count at exactly one and needs zero changes to
//! the systems above.

use crate::config::{PLAYER_HEIGHT, REMOTE_SMOOTH_RATE, REMOTE_SNAP_ARC};
use crate::content::characters::AstronautKind;
use crate::planet::CurrentPlanet;
use crate::player::{Joint, Player, RigAnim, RigDrive};
use crate::sphere;
use bevy::prelude::*;

/// A replicated astronaut we are DRAWING (never simulating). Holds the smoothed pose plus
/// the motion we derive for the animator.
#[derive(Component)]
pub struct RemoteAstronaut {
    /// Smoothed surface direction — chases the replicated `NetTransform.dir`.
    pub dir: Vec3,
    pub height: f32,
    pub facing: Vec3,
    /// Tangential speed in m/s, finite-differenced from the SMOOTHED dir (see below).
    pub speed: f32,
    pub vel_r: f32,
    pub grounded: bool,
    pub anim: RigAnim,
}

pub struct RemoteVisualsPlugin;

impl Plugin for RemoteVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (spawn_remote_rigs, drive_remote_transforms, animate_remote_rigs)
                .chain()
                .run_if(in_state(crate::AppState::InRun))
                .run_if(crate::net::is_client),
        )
        // NOT gated on is_client: a client that drops mid-run still has to clean up.
        .add_systems(OnExit(crate::AppState::InRun), despawn_remote_rigs);
    }
}

/// Give every remote astronaut a rig. This POLLS rather than using `Added<>` or an observer
/// on purpose: the replicated entity, our own `MyPlayerId`, and `CurrentPlanet` can arrive
/// in any order, and a one-shot trigger fired before the other two exist would be lost.
/// Polling self-heals.
fn spawn_remote_rigs(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    planet: Res<CurrentPlanet>,
    mine: Res<crate::net::MyPlayerId>,
    q_new: Query<
        (Entity, &crate::player::PlayerId, &crate::net::NetTransform),
        (Without<Player>, Without<RemoteAstronaut>),
    >,
) {
    // Until the host tells us who we are we cannot distinguish teammates from the server's
    // copy of ourselves, so we draw NOBODY rather than risk a ghost twin.
    let Some(my_id) = mine.0 else { return };

    for (e, pid, nt) in &q_new {
        if pid.0 == my_id {
            continue; // the server's copy of us — we already draw our own predicted body
        }
        // A remote's real AstronautKind isn't on the wire yet, so pick a stable palette by
        // slot. Teammates are visually distinct and consistent; correct suits need a
        // character handshake (see NETCODE NOTES).
        let def = AstronautKind::ALL[(pid.0 as usize) % AstronautKind::ALL.len()].def();

        let up = nt.dir;
        let tf = Transform {
            translation: planet.surface_point(up) + up * (nt.height + PLAYER_HEIGHT * 0.5),
            rotation: sphere::frame_quat(up, nt.facing),
            ..default()
        };

        commands.entity(e).insert((
            RemoteAstronaut {
                dir: nt.dir,
                height: nt.height,
                facing: nt.facing,
                speed: 0.0,
                vel_r: 0.0,
                grounded: true,
                anim: RigAnim::default(),
            },
            tf,
            Visibility::default(),
        ));
        // No StageScoped: this entity belongs to the server. Letting despawn_stage reap it
        // would pull a replicated entity out from under replicon.
        crate::player::build_astronaut_rig(&mut commands, e, &mut meshes, &mut materials, def.suit, def.visor);
        info!("NET remote visual: built rig for player {}", pid.0);
    }
}

/// Ease the drawn pose toward the last replicated snapshot and reconstruct the Transform
/// with exactly the same formula `player_physics` uses, so a remote stands on the terrain
/// the same way the local astronaut does.
fn drive_remote_transforms(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(&crate::net::NetTransform, &mut RemoteAstronaut, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let k = 1.0 - (-REMOTE_SMOOTH_RATE * dt).exp();
    let ka = 1.0 - (-9.0 * dt).exp();

    for (nt, mut r, mut tf) in &mut q {
        let prev = r.dir;
        let prev_h = r.height;

        // Great-circle chase toward the snapshot, with a hard snap for big jumps. The snap
        // is not optional: sphere::step_toward returns its input unchanged when the angle is
        // near-antipodal, so a teleport or stage change would freeze the rig forever.
        let ang = r.dir.angle_between(nt.dir);
        r.dir = if ang * planet.radius > REMOTE_SNAP_ARC || ang > std::f32::consts::PI - 1e-3 {
            nt.dir
        } else {
            sphere::step_toward(r.dir, nt.dir, ang * k)
        };
        r.height += (nt.height - r.height) * k;
        // Renormalizing is mandatory — translation is derived from this direction, so a
        // shortened vector sinks the rig into the crust.
        r.facing = r.facing.lerp(nt.facing, k).try_normalize().unwrap_or(nt.facing);

        // Derive motion for the animator from the SMOOTHED pose, not the raw snapshots:
        // snapshots only change on frames a packet arrived, so differencing them reads zero
        // on every other frame and the gait strobes between walking and idle.
        //
        // Two subtleties, both of which silently halve the reported speed if got wrong —
        // and speed drives gait amplitude, so wrong speed means skating feet:
        //   * use the LOCAL surface radius, the same one `sphere::advance` integrates with
        //     (planet.surface(dir) + height), not the nominal planet.radius;
        //   * measure the CHORD, not `angle_between`. The per-frame angle is ~1e-3 rad, so
        //     `acos(dot)` is evaluated where dot ≈ 1 - 5e-7; at f32 precision that
        //     quantizes toward 1.0 and biases every sample low. Chord length is stable at
        //     small angles and equals r·θ to within (θ/2)²/6.
        let local_r = planet.surface(r.dir) + r.height;
        r.speed += ((r.dir - prev).length() * local_r / dt - r.speed) * ka;
        r.vel_r += ((r.height - prev_h) / dt - r.vel_r) * ka;
        r.grounded = nt.height <= 0.02;

        let up = r.dir;
        tf.translation = planet.surface_point(up) + up * (r.height + PLAYER_HEIGHT * 0.5);
        tf.rotation = sphere::frame_quat(up, r.facing);
    }
}

/// Run remote rigs through the SAME animator the local astronaut uses, so teammates have
/// the game's signature walk instead of sliding around frozen.
fn animate_remote_rigs(
    time: Res<Time>,
    mut q: Query<(&mut RemoteAstronaut, &Children)>,
    mut q_joints: Query<(&mut Joint, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t = time.elapsed_secs();
    for (mut r, children) in &mut q {
        // Build the drive from immutable reads BEFORE borrowing anim mutably.
        let d = RigDrive {
            speed: r.speed,
            grounded: r.grounded,
            // slide_timer isn't replicated, so remotes stay upright through a slide.
            sliding: false,
            vel_r: r.vel_r,
        };
        crate::player::animate_rig(&mut r.anim, d, children, &mut q_joints, dt, t);
    }
}

/// Strip the rigs when the run ends. Replicated entities carry no `StageScoped`, so neither
/// `despawn_stage` nor the stage-transition sweep touches them and the rigs would otherwise
/// follow us back to the main menu. Remove our components and children only — the root
/// entity is the server's to despawn.
fn despawn_remote_rigs(mut commands: Commands, q: Query<(Entity, &Children), With<RemoteAstronaut>>) {
    for (root, children) in &q {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
        commands
            .entity(root)
            .remove::<RemoteAstronaut>()
            .remove::<Children>();
    }
}
