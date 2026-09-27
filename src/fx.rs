//! Juice: screenshake, hitstop, a tiny pooled particle system — and the §13 guards on all
//! of it: the evolution screen flash (gone under flash reduction), the bloom clamp, and the
//! photosensitivity gate that keeps anything strobing under three flashes a second.

use crate::config::*;
use crate::planet::StageScoped;
use crate::save::MetaSave;
use bevy::platform::collections::HashMap;
use bevy::post_process::bloom::Bloom;
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

// ---------------------------------------------------------------- flash guards

/// Photosensitivity mode's rate limiter (§13: "throttles ... flicker to <3 flashes/sec").
/// Each strobing source asks before it lights up, under its own key — one astronaut's chain
/// zaps, the horde's death bursts — and is refused until PHOTO_MIN_FLASH_INTERVAL has passed
/// since that source last flashed. Only consulted in photosensitivity mode.
#[derive(Resource, Default)]
pub struct FlashGate {
    last: HashMap<u64, f32>,
}

/// FlashGate key for the horde's death bursts (one shared budget: a wave dying at once is
/// exactly the strobe the mode exists to stop).
pub const GATE_KILL_BURSTS: u64 = u64::MAX;

impl FlashGate {
    pub fn allow(&mut self, key: u64, now: f32) -> bool {
        match self.last.get(&key) {
            Some(t) if now - *t < PHOTO_MIN_FLASH_INTERVAL => false,
            _ => {
                self.last.insert(key, now);
                // keys are per-astronaut and a handful of globals, but never let it grow
                if self.last.len() > 64 {
                    self.last.retain(|_, t| now - *t < PHOTO_MIN_FLASH_INTERVAL);
                }
                true
            }
        }
    }
}

/// A full-screen color flash (the evolution white-flash). Flash reduction suppresses it at
/// the source, so nothing downstream has to know the setting.
#[derive(Resource, Default)]
pub struct ScreenFlash {
    pub color: Color,
    pub alpha: f32,
}

impl ScreenFlash {
    pub fn fire(&mut self, color: Color, save: &MetaSave) {
        if save.accessibility.flash_reduction {
            return;
        }
        self.color = color;
        self.alpha = EVOLVE_FLASH_ALPHA;
    }
}

#[derive(Component)]
pub struct ScreenFlashOverlay;

pub fn spawn_screen_flash(mut commands: Commands) {
    commands.spawn((
        ScreenFlashOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::NONE),
        // over the HUD, under the modal panels
        GlobalZIndex(5),
        Pickable::IGNORE,
    ));
}

pub fn update_screen_flash(
    time: Res<Time<Real>>,
    mut flash: ResMut<ScreenFlash>,
    mut q: Query<&mut BackgroundColor, With<ScreenFlashOverlay>>,
) {
    if flash.alpha <= 0.0 && !flash.is_changed() {
        return;
    }
    flash.alpha = (flash.alpha - time.delta_secs() * EVOLVE_FLASH_ALPHA / EVOLVE_FLASH_SECS).max(0.0);
    for mut bg in &mut q {
        bg.0 = flash.color.with_alpha(flash.alpha);
    }
}

/// Flash reduction clamps the camera's bloom; it and the palette also retune the particle
/// glow (and the danger-colored sparks). Shared material handles and one camera, so a
/// settings change costs a handful of writes.
pub fn apply_fx_settings(
    save: Res<MetaSave>,
    particles: Option<Res<ParticleAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blooms: Query<&mut Bloom>,
    mut applied: Local<Option<(bool, crate::content::palettes::Palette)>>,
) {
    let want = (save.accessibility.flash_reduction, save.accessibility.palette);
    if *applied == Some(want) && blooms.iter().all(|b| b.intensity == bloom_for(want.0)) {
        return;
    }
    for mut b in &mut blooms {
        b.intensity = bloom_for(want.0);
    }
    let Some(pa) = particles else { return };
    let k = if want.0 { PARTICLE_EMISSIVE_REDUCED } else { PARTICLE_EMISSIVE };
    for (kind, handle) in &pa.mats {
        let base = if *kind == Pcolor::Danger { want.1.danger() } else { kind.color() };
        if let Some(m) = materials.get_mut(handle) {
            m.base_color = base;
            m.emissive = base.to_linear() * k;
        }
    }
    *applied = Some(want);
}

fn bloom_for(flash_reduction: bool) -> f32 {
    if flash_reduction { BLOOM_INTENSITY_REDUCED } else { BLOOM_INTENSITY }
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
    /// Telegraph detonations: follows the colorblind palette's danger color.
    Danger,
}

impl Pcolor {
    const ALL: [Pcolor; 8] = [
        Pcolor::White,
        Pcolor::Gold,
        Pcolor::Green,
        Pcolor::Red,
        Pcolor::Blue,
        Pcolor::Purple,
        Pcolor::Cyan,
        Pcolor::Danger,
    ];

    /// Canon color (Danger's is the Standard palette's; `apply_fx_settings` retints it).
    fn color(&self) -> Color {
        match self {
            Pcolor::White => Color::srgb(1.0, 1.0, 1.0),
            Pcolor::Gold => Color::srgb(1.0, 0.85, 0.2),
            Pcolor::Green => Color::srgb(0.4, 1.0, 0.5),
            Pcolor::Red => Color::srgb(1.0, 0.3, 0.25),
            Pcolor::Blue => Color::srgb(0.4, 0.6, 1.0),
            Pcolor::Purple => Color::srgb(0.8, 0.4, 1.0),
            Pcolor::Cyan => Color::srgb(0.4, 1.0, 1.0),
            Pcolor::Danger => crate::content::palettes::Palette::Standard.danger(),
        }
    }
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
    let mats = Pcolor::ALL
        .iter()
        .map(|k| {
            let c = k.color();
            (
                *k,
                materials.add(StandardMaterial {
                    base_color: c,
                    emissive: c.to_linear() * PARTICLE_EMISSIVE,
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
