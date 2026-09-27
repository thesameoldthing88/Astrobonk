//! Juice: screenshake, hitstop, a tiny pooled particle system — and the §13 guards on all
//! of it: the evolution screen flash (gone under flash reduction), the bloom clamp, and the
//! photosensitivity gate that keeps anything strobing under three flashes a second.

use crate::config::*;
use crate::planet::StageScoped;
use crate::save::MetaSave;
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

/// How heavy a killing blow was, for the §13 hitstop table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillWeight {
    Crowd,
    /// A horde elite or a miniboss.
    Elite,
    /// The stage boss.
    Boss,
}

/// §13 hitstop: "only on YOUR kills, never chip damage". A kill asks for a stop through
/// `kill`, which keeps them sparse; set pieces (an evolution, a comet cash-out) take a
/// flat one through `stop`. `hitstop_system` turns it into a slowed virtual clock.
///
/// Only a SOLO game freezes (`freezes`): the clock it slows is the simulation's, so on a
/// co-op host it would stall every teammate's world for the host's kill (KNOWN_ISSUES M4),
/// and on a client it would put its own prediction out of step with the host.
#[derive(Resource, Default)]
pub struct Hitstop {
    /// Real seconds of freeze left.
    pub timer: f32,
    /// Real seconds of the boss kill's slow-motion tail left (after the freeze).
    pub dilate: f32,
    /// When the last stop began, on the VIRTUAL clock (which a freeze barely moves, so the
    /// sparseness gap is measured in play time).
    last: Option<f32>,
    /// Telemetry: stops granted per `KillWeight` (crowd, elite, boss), kills refused for
    /// the gap, and the seconds of freeze granted in all.
    pub granted: [u32; 3],
    pub refused: u32,
    pub frozen_secs: f32,
    pub evolved_stops: u32,
}

impl Hitstop {
    /// A set-piece stop of `secs` (never shortens one already running).
    pub fn stop(&mut self, secs: f32) {
        self.timer = self.timer.max(secs);
    }

    /// Your killing blow at virtual time `now`: the §13 stop for its weight (+20 ms from an
    /// evolved weapon), unless a stop began too recently for that weight. Returns whether it
    /// was granted.
    pub fn kill(&mut self, weight: KillWeight, evolved: bool, now: f32) -> bool {
        let (base, gap, i) = match weight {
            KillWeight::Crowd => (HITSTOP_KILL_SECS, HITSTOP_KILL_GAP, 0),
            KillWeight::Elite => (HITSTOP_ELITE_SECS, HITSTOP_ELITE_GAP, 1),
            KillWeight::Boss => (HITSTOP_BOSS_SECS, 0.0, 2),
        };
        // (`now < last`: the clock restarted under us — a new run — so never lock out)
        if let Some(last) = self.last {
            if now >= last && now - last < gap {
                self.refused += 1;
                return false;
            }
        }
        let secs = base + if evolved { HITSTOP_EVOLVED_BONUS } else { 0.0 };
        self.stop(secs);
        if weight == KillWeight::Boss {
            self.dilate = HITSTOP_DILATE_SECS;
        }
        self.last = Some(now);
        self.granted[i] += 1;
        self.frozen_secs += secs;
        if evolved {
            self.evolved_stops += 1;
        }
        true
    }
}

/// Does this machine's hitstop freeze anything? Solo only — see `Hitstop`.
pub fn freezes(role: &crate::net::NetRole) -> bool {
    *role == crate::net::NetRole::Solo
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
    let dt = time.delta_secs();
    if hitstop.timer > 0.0 {
        hitstop.timer -= dt;
        virt.set_relative_speed(HITSTOP_FREEZE_SPEED);
    } else if hitstop.dilate > 0.0 {
        // the boss kill's tail: the world eases back in instead of snapping to full speed
        hitstop.dilate -= dt;
        virt.set_relative_speed(HITSTOP_DILATE_SPEED);
    } else if virt.relative_speed() != 1.0 {
        virt.set_relative_speed(1.0);
    }
}

/// Headless self-check of the §13 hitstop canon: the table's durations, the evolved bonus,
/// the boss's dilation, and above all SPARSENESS — a horde dying at 30 kills a second must
/// not freeze the game on every one of them (an earlier build froze ~3 times a second).
pub fn hitstop_self_check() -> Result<(), String> {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let mut h = Hitstop::default();
    if !h.kill(KillWeight::Crowd, false, 10.0) || !close(h.timer, 0.05) || h.dilate != 0.0 {
        return Err(format!("a crowd kill stopped {:.3}s (dilate {:.2}), not 50 ms", h.timer, h.dilate));
    }
    let mut h = Hitstop::default();
    if !h.kill(KillWeight::Elite, true, 10.0) || !close(h.timer, 0.11) {
        return Err(format!("an evolved weapon's elite kill stopped {:.3}s, not 90 + 20 ms", h.timer));
    }
    let mut h = Hitstop::default();
    h.kill(KillWeight::Crowd, false, 10.0);
    if !h.kill(KillWeight::Boss, false, 10.01) || !close(h.timer, 0.13) || !close(h.dilate, HITSTOP_DILATE_SECS) {
        return Err(format!("the boss kill stopped {:.3}s / dilated {:.2}s right after a crowd stop", h.timer, h.dilate));
    }
    // a horde at 30 kills/s for 60 s, with an elite every 7 s: the crowd stops must mark
    // the rhythm, never every body
    let mut h = Hitstop::default();
    let mut elites = 0;
    for i in 0..1800 {
        let now = 100.0 + i as f32 / 30.0;
        let weight = if i % 210 == 0 { KillWeight::Elite } else { KillWeight::Crowd };
        elites += u32::from(weight == KillWeight::Elite);
        h.kill(weight, false, now);
    }
    let stops = h.granted[0] + h.granted[1];
    let budget = (60.0 / HITSTOP_KILL_GAP) as u32 + elites + 1;
    if stops > budget || h.frozen_secs / 60.0 > 0.04 {
        return Err(format!("{stops} stops in a minute of 1800 kills (budget {budget}; {:.1}% of play frozen)", h.frozen_secs / 60.0 * 100.0));
    }
    if h.granted[0] == 0 {
        return Err("a minute of crowd kills never stopped time once".into());
    }
    Ok(())
}

/// The camera's shake this frame, as a positional offset along its own `right` and `up`:
/// the §13 budget `trauma² × SHAKE_MAX_DEG × slider`, split into yaw and pitch by
/// incommensurate sines, then HARD-CLAMPED so the whole offset subtends at most
/// SHAKE_MAX_DEG about the point the camera looks at, `dist` metres ahead. The offset is
/// perpendicular to the view, so that angle is exactly `atan(|offset| / dist)`.
pub fn shake_offset(trauma: f32, slider: f32, t: f32, right: Vec3, up: Vec3, dist: f32) -> Vec3 {
    let tr = trauma.clamp(0.0, 1.0);
    let max = SHAKE_MAX_DEG.to_radians();
    let amp = tr * tr * slider.clamp(0.0, 1.0) * max;
    if amp <= 1e-6 || dist <= 0.0 {
        return Vec3::ZERO;
    }
    let yaw = (t.sin() * 0.62 + (t * 1.7).cos() * 0.38) * amp;
    let pitch = ((t * 1.3).cos() * 0.7 + (t * 2.3).sin() * 0.3) * amp;
    let total = (yaw * yaw + pitch * pitch).sqrt().min(max);
    let Some(way) = (right * yaw + up * pitch).try_normalize() else { return Vec3::ZERO };
    way * dist * total.tan()
}

/// Headless self-check: however much trauma piles up, at any slider setting and any
/// moment, the camera never swings more than SHAKE_MAX_DEG off its unshaken aim.
pub fn shake_self_check() -> Result<(), String> {
    let (right, up) = (Vec3::X, Vec3::Y);
    let dist = 9.0;
    let mut worst = 0.0f32;
    for trauma in [0.3, 0.7, 1.0, 1.6, 5.0] {
        for slider in [0.5, 1.0, 3.0] {
            for i in 0..2000 {
                let o = shake_offset(trauma, slider, i as f32 * 0.137, right, up, dist);
                worst = worst.max((o.length() / dist).atan().to_degrees());
            }
        }
    }
    if worst > SHAKE_MAX_DEG + 1e-3 {
        return Err(format!("camera shake reached {worst:.3}° (clamp {SHAKE_MAX_DEG}°)"));
    }
    if worst < SHAKE_MAX_DEG * 0.5 {
        return Err(format!("full trauma only shook {worst:.3}° — the budget is unreachable"));
    }
    let chip = (0..2000)
        .map(|i| shake_offset(SHAKE_PLAYER_HIT, 1.0, i as f32 * 0.137, right, up, dist).length())
        .fold(0.0f32, f32::max);
    if (chip / dist).atan().to_degrees() > SHAKE_MAX_DEG * 0.05 {
        return Err("a single chip hit shakes more than 5% of the budget (trauma² should keep it faint)".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- flash guards

/// Photosensitivity mode's rate limiter (§13: "throttles ... flicker to <3 flashes/sec").
/// What counts is the VIEWER's screen, so every strobing source — any astronaut's chain
/// zaps, the horde's death bursts (The Static dying in waves) — draws from ONE budget: at
/// most one flash per PHOTO_MIN_FLASH_INTERVAL, whoever caused it. Only consulted in
/// photosensitivity mode; a refused flash is simply not drawn (damage is never gated).
#[derive(Resource)]
pub struct FlashGate {
    last: f32,
}

impl Default for FlashGate {
    fn default() -> Self {
        Self { last: f32::NEG_INFINITY }
    }
}

impl FlashGate {
    pub fn allow(&mut self, now: f32) -> bool {
        // `now < last`: the clock restarted under us (a new run) — never lock out forever
        if now - self.last >= PHOTO_MIN_FLASH_INTERVAL || now < self.last {
            self.last = now;
            true
        } else {
            false
        }
    }

    /// A flash that must show regardless (a boss's death) still spends the budget.
    pub fn mark(&mut self, now: f32) {
        self.last = now;
    }
}

/// Headless self-check: the photosensitivity budget is ONE per screen — a second source (a
/// teammate's zaps, a death burst) inside the interval is refused — and it reopens after it.
pub fn flash_gate_self_check() -> Result<(), String> {
    let mut g = FlashGate::default();
    let dt = PHOTO_MIN_FLASH_INTERVAL;
    if !g.allow(10.0) || g.allow(10.0 + dt * 0.5) || g.allow(10.0 + dt * 0.99) {
        return Err("flash gate let two flashes through one interval".into());
    }
    if !g.allow(10.0 + dt * 1.01) {
        return Err("flash gate stayed shut past its interval".into());
    }
    g.mark(20.0);
    if g.allow(20.0 + dt * 0.5) {
        return Err("a forced flash (boss death) did not spend the budget".into());
    }
    // the budget is what WCAG 2.3.1 asks: under three flashes in any second
    if 1.0 / dt >= 3.0 {
        return Err(format!("PHOTO_MIN_FLASH_INTERVAL {dt} allows {:.1} flashes/s", 1.0 / dt));
    }
    Ok(())
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

/// Flash reduction (and photosensitivity mode, whose "softens Death Ray bloom" is the same
/// clamp) turns the camera's bloom down; flash reduction also dims the particles, and the
/// palette retints the danger-colored sparks. The particle materials are unlit — they draw
/// their base color only — so dimming means darkening that color. Shared material handles
/// and one camera, so a settings change costs a handful of writes.
pub fn apply_fx_settings(
    save: Res<MetaSave>,
    particles: Option<Res<ParticleAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blooms: Query<&mut Bloom>,
    mut applied: Local<Option<(bool, crate::content::palettes::Palette)>>,
) {
    let a = &save.accessibility;
    let bloom = if a.flash_reduction || a.photosensitive { BLOOM_INTENSITY_REDUCED } else { BLOOM_INTENSITY };
    // (the camera is respawned with every run, so its bloom is checked, not remembered)
    for mut b in &mut blooms {
        if b.intensity != bloom {
            b.intensity = bloom;
        }
    }
    let want = (a.flash_reduction, a.palette);
    if *applied == Some(want) {
        return;
    }
    let Some(pa) = particles else { return };
    let k = if want.0 { PARTICLE_BRIGHTNESS_REDUCED } else { 1.0 };
    for (kind, handle) in &pa.mats {
        let base = if *kind == Pcolor::Danger { want.1.danger() } else { kind.color() };
        if let Some(m) = materials.get_mut(handle) {
            let l = base.to_linear();
            m.base_color = LinearRgba::rgb(l.red * k, l.green * k, l.blue * k).into();
        }
    }
    *applied = Some(want);
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
