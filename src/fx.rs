//! Juice: screenshake, hitstop, and a tiny pooled particle system.

use crate::planet::StageScoped;
use bevy::prelude::*;
use rand::Rng;

#[derive(Resource, Default)]
pub struct Shake {
    pub trauma: f32,
}

impl Shake {
    pub fn add(&mut self, v: f32) {
        self.trauma = (self.trauma + v).min(1.0);
    }
}

pub fn shake_decay(time: Res<Time<Real>>, mut shake: ResMut<Shake>) {
    shake.trauma = (shake.trauma - time.delta_secs() * 1.6).max(0.0);
}

#[derive(Resource, Default)]
pub struct Hitstop {
    pub timer: f32,
}

pub fn hitstop_system(
    time: Res<Time<Real>>,
    mut hitstop: ResMut<Hitstop>,
    mut virt: ResMut<Time<Virtual>>,
    phase: Res<crate::run::RunPhase>,
) {
    use crate::run::RunPhase;
    // Modal phases pause time outright; hitstop only modulates during play.
    if *phase != RunPhase::Playing {
        return;
    }
    if hitstop.timer > 0.0 {
        hitstop.timer -= time.delta_secs();
        virt.set_relative_speed(0.06);
    } else if virt.relative_speed() != 1.0 {
        virt.set_relative_speed(1.0);
    }
}

/// Pause/unpause virtual time when the phase changes.
pub fn phase_time_control(phase: Res<crate::run::RunPhase>, mut virt: ResMut<Time<Virtual>>) {
    use crate::run::RunPhase;
    if !phase.is_changed() {
        return;
    }
    match *phase {
        RunPhase::Playing => {
            virt.unpause();
            virt.set_relative_speed(1.0);
        }
        _ => virt.pause(),
    }
}

// ---------------------------------------------------------------- particles

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pcolor {
    White,
    Gold,
    Green,
    Red,
    Blue,
    Purple,
    Cyan,
}

#[derive(Resource)]
pub struct ParticleAssets {
    pub mesh: Handle<Mesh>,
    pub mats: Vec<(Pcolor, Handle<StandardMaterial>)>,
}

impl ParticleAssets {
    pub fn mat(&self, c: Pcolor) -> Handle<StandardMaterial> {
        self.mats
            .iter()
            .find(|(k, _)| *k == c)
            .map(|(_, h)| h.clone())
            .unwrap_or_else(|| self.mats[0].1.clone())
    }
}

#[derive(Component)]
pub struct Particle {
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub gravity: Vec3,
}

pub fn setup_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let colors = [
        (Pcolor::White, Color::srgb(1.0, 1.0, 1.0)),
        (Pcolor::Gold, Color::srgb(1.0, 0.85, 0.2)),
        (Pcolor::Green, Color::srgb(0.4, 1.0, 0.5)),
        (Pcolor::Red, Color::srgb(1.0, 0.3, 0.25)),
        (Pcolor::Blue, Color::srgb(0.4, 0.6, 1.0)),
        (Pcolor::Purple, Color::srgb(0.8, 0.4, 1.0)),
        (Pcolor::Cyan, Color::srgb(0.4, 1.0, 1.0)),
    ];
    let mats = colors
        .iter()
        .map(|(k, c)| {
            (
                *k,
                materials.add(StandardMaterial {
                    base_color: *c,
                    emissive: c.to_linear() * 2.5,
                    unlit: true,
                    ..default()
                }),
            )
        })
        .collect();
    commands.insert_resource(ParticleAssets {
        mesh: meshes.add(Mesh::from(Cuboid::new(0.16, 0.16, 0.16))),
        mats,
    });
}

pub fn burst(
    commands: &mut Commands,
    assets: &ParticleAssets,
    pos: Vec3,
    up: Vec3,
    color: Pcolor,
    count: usize,
    speed: f32,
) {
    let mut rng = rand::thread_rng();
    let mat = assets.mat(color);
    for _ in 0..count {
        let v = Vec3::new(
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
        )
        .normalize_or_zero()
            * speed
            * rng.gen_range(0.5..1.3)
            + up * speed * 0.7;
        let life = rng.gen_range(0.35..0.7);
        commands.spawn((
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(pos).with_scale(Vec3::splat(rng.gen_range(0.7..1.4))),
            Particle { vel: v, life, max_life: life, gravity: -up * 18.0 },
            StageScoped,
        ));
    }
}

pub fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut p, mut tf) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let vel = p.vel;
        let grav = p.gravity;
        p.vel = vel + grav * dt;
        tf.translation += p.vel * dt;
        let s = (p.life / p.max_life).max(0.05);
        tf.scale = Vec3::splat(s * 0.9);
    }
}
