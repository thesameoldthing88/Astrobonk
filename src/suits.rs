//! What a suit looks like on the one astronaut (locked direction #3/#4), and the Suit
//! Wardrobe's mannequin.
//!
//! The rig (`player::build_astronaut_rig`) is Milo's body: the same proportions, backpack
//! and turtle-shell patch in every suit. A suit dresses it in three materials (shell, trim,
//! visor glow) and one of twelve helmets. Every mesh here is built once per session and
//! shared (L9); the colours live in the materials, so a helmet's three meshes serve any
//! palette, the wardrobe's locked silhouettes included.
//!
//! The mannequin is Milo in the hovered suit, turning slowly in the wardrobe: a small
//! camera renders him into the wardrobe's viewport node, from a stage far away from any
//! world (the menus have none).

use crate::content::characters::{Helmet, SuitKind, TURTLE_SCUTES, TURTLE_SHELL, TURTLE_SKIN};
use crate::meshkit::{at, MeshData};
use crate::player::{Joint, RigAnim, RigDrive};
use bevy::camera::RenderTarget;
use bevy::prelude::*;

/// Vertex tones under a suit material: the shell (full colour) and its seams/details.
const SHELL: Color = Color::WHITE;
const SEAM: Color = Color::srgb(0.55, 0.55, 0.6);
const DARK: Color = Color::srgb(0.32, 0.32, 0.38);

/// The visor every helmet but the Probe's and the Diver's wears (the rig's original one).
fn std_visor(m: &mut MeshData) {
    m.add_ellipsoid(Vec3::new(0.252, 0.204, 0.168), 2, at(Vec3::new(0.0, 0.02, -0.16)), SHELL);
}

/// A ring facing forward (-Z) around the visor.
fn visor_ring(m: &mut MeshData, major: f32, minor: f32, z: f32) {
    let tf = Transform::from_translation(Vec3::new(0.0, 0.02, z)).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
    m.add_torus(major, minor, 20, 6, tf, SHELL);
}

/// A point on the dome of radius `r`: `up` degrees from the crown toward the back
/// (negative = toward the visor), `side` degrees toward +X.
fn dome_point(r: f32, up: f32, side: f32) -> Vec3 {
    let (u, s) = (up.to_radians(), side.to_radians());
    Quat::from_rotation_z(-s) * (Quat::from_rotation_x(u) * Vec3::Y) * r
}

/// A helmet's three meshes: the shell (suit material), its trim (trim material) and what
/// glows (visor material: the visor itself and any lamp, gem or halo).
pub(crate) fn helmet_meshes(h: Helmet) -> [Mesh; 3] {
    let (mut shell, mut trim, mut glow) = (MeshData::new(), MeshData::new(), MeshData::new());
    let dome = |m: &mut MeshData| m.add_sphere(0.3, 2, Transform::IDENTITY, SHELL);
    match h {
        Helmet::Classic => {
            dome(&mut shell);
            std_visor(&mut glow);
            visor_ring(&mut trim, 0.215, 0.028, -0.2);
            // the comm box on the left temple
            trim.add_box(Vec3::new(0.08, 0.12, 0.14), at(Vec3::new(-0.3, 0.02, 0.02)), SHELL);
            shell.add_cylinder(0.012, 0.16, 5, at(Vec3::new(-0.3, 0.14, 0.05)), DARK);
        }
        Helmet::Sokol => {
            dome(&mut shell);
            std_visor(&mut glow);
            // a crest stripe over the crown, front to back
            for i in 0..8 {
                let up = -35.0 + i as f32 * 20.0;
                trim.add_box(
                    Vec3::new(0.07, 0.05, 0.09),
                    Transform::from_translation(dome_point(0.3, up, 0.0)).with_rotation(Quat::from_rotation_x(up.to_radians())),
                    SHELL,
                );
            }
            // pressure-hood cuff ring low on the neck
            trim.add_torus(0.24, 0.035, 20, 6, at(Vec3::new(0.0, -0.2, 0.0)), SHELL);
        }
        Helmet::Probe => {
            shell.add_box(Vec3::new(0.52, 0.46, 0.5), at(Vec3::new(0.0, 0.02, 0.0)), SHELL);
            shell.add_box(Vec3::new(0.4, 0.1, 0.06), at(Vec3::new(0.0, -0.15, -0.26)), DARK);
            // a slit visor and a blinking antenna tip
            glow.add_box(Vec3::new(0.42, 0.08, 0.04), at(Vec3::new(0.0, 0.06, -0.26)), SHELL);
            glow.add_sphere(0.045, 1, at(Vec3::new(0.14, 0.52, 0.04)), SHELL);
            trim.add_cylinder(0.014, 0.26, 5, at(Vec3::new(0.14, 0.37, 0.04)), SHELL);
            for s in [-1.0, 1.0] {
                trim.add_cylinder(0.09, 0.04, 10, Transform::from_xyz(0.27 * s, 0.02, 0.0).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)), SHELL);
                trim.add_sphere(0.022, 0, at(Vec3::new(0.18 * s, 0.2, -0.255)), SHELL);
            }
        }
        Helmet::Shinobi => {
            dome(&mut shell);
            glow.add_ellipsoid(Vec3::new(0.25, 0.085, 0.15), 2, at(Vec3::new(0.0, 0.04, -0.18)), SHELL);
            // a headband round the brow, its two tails streaming behind
            trim.add_torus(0.3, 0.03, 22, 6, Transform::from_xyz(0.0, 0.12, 0.0).with_rotation(Quat::from_rotation_x(-0.18)), SHELL);
            for s in [-1.0, 1.0] {
                trim.add_box(
                    Vec3::new(0.05, 0.02, 0.26),
                    Transform::from_xyz(0.05 * s, 0.1, 0.4).with_rotation(Quat::from_euler(EulerRot::YXZ, 0.35 * s, 0.35, 0.0)),
                    SHELL,
                );
            }
        }
        Helmet::Mercury => {
            dome(&mut shell);
            std_visor(&mut glow);
            // round ear cups and a breather vent on the crown
            for s in [-1.0, 1.0] {
                trim.add_cylinder(0.11, 0.08, 12, Transform::from_xyz(0.29 * s, 0.0, 0.02).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)), SHELL);
                shell.add_cylinder(0.05, 0.1, 8, Transform::from_xyz(0.33 * s, 0.0, 0.02).with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)), SEAM);
            }
            trim.add_cylinder(0.06, 0.07, 10, at(Vec3::new(0.0, 0.3, 0.04)), SHELL);
        }
        Helmet::HardHat => {
            dome(&mut shell);
            std_visor(&mut glow);
            // the hat: a cap over the crown and a brim, a headlamp on the front
            trim.add_ellipsoid(Vec3::new(0.32, 0.2, 0.33), 2, at(Vec3::new(0.0, 0.12, 0.0)), SHELL);
            trim.add_cylinder(0.39, 0.03, 18, Transform::from_xyz(0.0, 0.12, -0.02).with_rotation(Quat::from_rotation_x(-0.08)), SHELL);
            shell.add_cylinder(0.065, 0.08, 10, Transform::from_xyz(0.0, 0.2, -0.31).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), DARK);
            glow.add_cylinder(0.05, 0.02, 10, Transform::from_xyz(0.0, 0.2, -0.355).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)), SHELL);
        }
        Helmet::Monocle => {
            dome(&mut shell);
            std_visor(&mut glow);
            // a scope over the right eye, its lens glowing, and the strap holding it
            let tube = Transform::from_xyz(0.11, 0.07, -0.33).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
            trim.add_cylinder(0.065, 0.2, 12, tube, SHELL);
            glow.add_cylinder(0.05, 0.02, 12, tube.with_translation(Vec3::new(0.11, 0.07, -0.44)), SHELL);
            trim.add_torus(0.3, 0.018, 22, 5, Transform::from_xyz(0.0, 0.07, 0.0).with_rotation(Quat::from_rotation_z(-0.1)), SHELL);
        }
        Helmet::Racer => {
            shell.add_ellipsoid(Vec3::new(0.3, 0.3, 0.33), 2, at(Vec3::new(0.0, 0.0, 0.02)), SHELL);
            glow.add_ellipsoid(Vec3::new(0.27, 0.17, 0.17), 2, at(Vec3::new(0.0, 0.03, -0.17)), SHELL);
            // two fins swept back from the temples, and a centre stripe
            for s in [-1.0, 1.0] {
                trim.add_box(
                    Vec3::new(0.03, 0.12, 0.3),
                    Transform::from_xyz(0.24 * s, 0.14, 0.2).with_rotation(Quat::from_euler(EulerRot::YXZ, 0.25 * s, -0.4, -0.5 * s)),
                    SHELL,
                );
            }
            for i in 0..6 {
                let up = -20.0 + i as f32 * 22.0;
                trim.add_box(
                    Vec3::new(0.05, 0.02, 0.1),
                    Transform::from_translation(dome_point(0.305, up, 0.0)).with_rotation(Quat::from_rotation_x(up.to_radians())),
                    SHELL,
                );
            }
        }
        Helmet::Diver => {
            shell.add_sphere(0.34, 2, at(Vec3::new(0.0, 0.02, 0.0)), SHELL);
            shell.add_cylinder(0.26, 0.1, 16, at(Vec3::new(0.0, -0.26, 0.0)), SEAM);
            // a round porthole, its bolted frame, and bolts round the neck
            glow.add_ellipsoid(Vec3::new(0.15, 0.15, 0.08), 2, at(Vec3::new(0.0, 0.04, -0.3)), SHELL);
            visor_ring(&mut trim, 0.17, 0.035, -0.31);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                trim.add_sphere(0.025, 0, at(Vec3::new(a.cos() * 0.17, 0.04 + a.sin() * 0.17, -0.35)), SHELL);
                trim.add_sphere(0.03, 0, at(Vec3::new(a.cos() * 0.27, -0.24, a.sin() * 0.27)), SHELL);
            }
        }
        Helmet::Crown => {
            dome(&mut shell);
            std_visor(&mut glow);
            // a little crown, a gem at its front
            trim.add_cylinder(0.16, 0.07, 14, at(Vec3::new(0.0, 0.29, 0.0)), SHELL);
            for i in 0..5 {
                let a = i as f32 / 5.0 * std::f32::consts::TAU + std::f32::consts::FRAC_PI_2;
                trim.add_cone(0.04, 0.1, 6, at(Vec3::new(a.cos() * 0.14, 0.37, -a.sin() * 0.14)), SHELL);
            }
            glow.add_sphere(0.035, 1, at(Vec3::new(0.0, 0.3, -0.165)), SHELL);
        }
        Helmet::Halo => {
            dome(&mut shell);
            std_visor(&mut glow);
            // a halo floating over the crown, and a jewel on the brow
            glow.add_torus(0.22, 0.022, 24, 6, Transform::from_xyz(0.0, 0.45, 0.04).with_rotation(Quat::from_rotation_x(0.18)), SHELL);
            trim.add_ellipsoid(Vec3::new(0.05, 0.07, 0.03), 1, at(Vec3::new(0.0, 0.27, -0.14)), SHELL);
            visor_ring(&mut trim, 0.215, 0.02, -0.2);
        }
        Helmet::Combat => {
            dome(&mut shell);
            std_visor(&mut glow);
            // a combat helmet's brim tipped over the brow, and a boom mic to the chin
            trim.add_ellipsoid(Vec3::new(0.33, 0.17, 0.34), 2, Transform::from_xyz(0.0, 0.1, 0.0).with_rotation(Quat::from_rotation_x(0.12)), SHELL);
            trim.add_cylinder(0.36, 0.03, 18, Transform::from_xyz(0.0, 0.07, 0.0).with_rotation(Quat::from_rotation_x(0.12)), SHELL);
            shell.add_cylinder(0.014, 0.24, 5, Transform::from_xyz(0.2, -0.12, -0.2).with_rotation(Quat::from_euler(EulerRot::YXZ, -0.6, 1.3, 0.0)), DARK);
            glow.add_sphere(0.03, 1, at(Vec3::new(0.1, -0.15, -0.3)), SHELL);
        }
    }
    [shell.build(), trim.build(), glow.build()]
}

/// The trim on Milo's torso: collar ring, belt and buckle, two chest pipings.
pub(crate) fn body_trim_mesh() -> Mesh {
    let mut m = MeshData::new();
    m.add_torus(0.15, 0.035, 18, 6, at(Vec3::new(0.0, 0.47, 0.0)), SHELL);
    m.add_box(Vec3::new(0.46, 0.07, 0.33), at(Vec3::new(0.0, -0.05, 0.0)), SHELL);
    m.add_box(Vec3::new(0.11, 0.09, 0.03), at(Vec3::new(0.0, -0.05, -0.175)), SEAM);
    for s in [-1.0, 1.0] {
        m.add_box(Vec3::new(0.045, 0.4, 0.02), at(Vec3::new(0.19 * s, 0.22, -0.175)), SHELL);
    }
    m.build()
}

/// A cuff at the wrist (arm joints hang down -Y from the shoulder).
pub(crate) fn arm_cuff_mesh() -> Mesh {
    let mut m = MeshData::new();
    m.add_cylinder(0.105, 0.07, 10, at(Vec3::new(0.0, -0.665, 0.0)), SHELL);
    m.build()
}

/// A cuff over the boot top (leg joints hang down -Y from the hip).
pub(crate) fn leg_cuff_mesh() -> Mesh {
    let mut m = MeshData::new();
    m.add_cylinder(0.125, 0.07, 10, at(Vec3::new(0.0, -0.625, 0.0)), SHELL);
    m.build()
}

/// The turtle-shell patch strapped over the backpack's tanks (in the backpack's frame):
/// Milo's mark in every suit. A domed shell with its scutes, the head up top, four
/// flippers and a stub of a tail. Its colours are baked in (a white material carries it).
pub(crate) fn turtle_patch_mesh() -> Mesh {
    let mut m = MeshData::new();
    let c = Vec3::new(0.0, 0.03, 0.55);
    m.add_ellipsoid(Vec3::new(0.15, 0.17, 0.06), 2, at(c), TURTLE_SHELL);
    // scutes: one in the middle, six around it, sitting proud of the dome
    m.add_ellipsoid(Vec3::new(0.05, 0.055, 0.02), 1, at(c + Vec3::new(0.0, 0.0, 0.052)), TURTLE_SCUTES);
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        let (x, y) = (a.cos() * 0.095, a.sin() * 0.105);
        m.add_ellipsoid(Vec3::new(0.035, 0.035, 0.015), 1, at(c + Vec3::new(x, y, 0.038)), TURTLE_SCUTES);
    }
    // head, flippers, tail
    m.add_sphere(0.05, 1, at(c + Vec3::new(0.0, 0.2, 0.0)), TURTLE_SKIN);
    for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        m.add_ellipsoid(Vec3::new(0.055, 0.03, 0.02), 1, at(c + Vec3::new(0.14 * sx, 0.1 * sy, -0.01)), TURTLE_SKIN);
    }
    m.add_cone(0.025, 0.06, 6, Transform::from_translation(c + Vec3::new(0.0, -0.19, 0.0)).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)), TURTLE_SKIN);
    m.build()
}

// ------------------------------------------------------------------ the wardrobe mannequin

/// Where the wardrobe stages Milo: far from any world, so the mannequin camera sees him
/// alone (and nothing else ever wanders into its shot).
const STAGE: Vec3 = Vec3::new(0.0, 5000.0, 0.0);

/// Everything spawned for the wardrobe preview, torn down when the wardrobe closes.
#[derive(Component)]
pub struct WardrobeScene;

/// Milo on the wardrobe stage, and what he has on.
#[derive(Component, Default)]
pub struct Mannequin {
    anim: RigAnim,
    wearing: Option<(SuitKind, bool)>,
}

/// The suit the wardrobe is showing: the card under the pointer, else the one picked last.
#[derive(Resource, Clone, Copy, PartialEq)]
pub struct WardrobeFocus {
    pub suit: SuitKind,
    pub unlocked: bool,
}

impl Default for WardrobeFocus {
    fn default() -> Self {
        Self { suit: SuitKind::Buzz, unlocked: true }
    }
}

/// Stage the mannequin, its lights and the camera that renders him; returns the camera for
/// the wardrobe's `ViewportNode` (which sizes the camera's image to the node).
pub fn spawn_wardrobe_scene(commands: &mut Commands, images: &mut Assets<Image>) -> Entity {
    use bevy::asset::RenderAssetUsages;
    use bevy::render::render_resource::{TextureDimension, TextureFormat, TextureUsages};
    let mut image = Image::new_uninit(default(), TextureDimension::D2, TextureFormat::Bgra8UnormSrgb, RenderAssetUsages::all());
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let look_at = STAGE + Vec3::new(0.0, crate::config::WARDROBE_CAM_AIM_Y, 0.0);
    let cam = commands
        .spawn((
            WardrobeScene,
            Camera3d::default(),
            // before the game camera, which draws the UI this image lands in
            Camera { order: -1, clear_color: ClearColorConfig::Custom(Color::srgb(0.05, 0.06, 0.1)), ..default() },
            RenderTarget::Image(image.into()),
            bevy::render::view::Hdr,
            bevy::core_pipeline::tonemapping::Tonemapping::AcesFitted,
            // the same toon look as the game camera (ink, cel bands, grade)
            crate::toon::camera_bundle(),
            Transform::from_translation(STAGE + crate::config::WARDROBE_CAM_OFFSET).looking_at(look_at, Vec3::Y),
        ))
        .id();
    // a warm key light from the front, a cool rim from behind (the turtle patch reads too)
    for (offset, color, lumens) in [
        (Vec3::new(1.6, 1.8, -2.4), Color::srgb(1.0, 0.95, 0.88), crate::config::WARDROBE_KEY_LUMENS),
        (Vec3::new(-1.8, 1.4, 2.2), Color::srgb(0.6, 0.75, 1.0), crate::config::WARDROBE_RIM_LUMENS),
    ] {
        commands.spawn((
            WardrobeScene,
            PointLight { color, intensity: lumens, range: 12.0, shadows_enabled: false, ..default() },
            Transform::from_translation(STAGE + offset),
        ));
    }
    commands.spawn((WardrobeScene, Mannequin::default(), Transform::from_translation(STAGE), Visibility::default()));
    cam
}

pub fn despawn_wardrobe_scene(mut commands: Commands, q: Query<Entity, With<WardrobeScene>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Dress the mannequin in the focused suit (a locked one as its silhouette), turn him
/// slowly, and let him breathe with the game's own idle.
pub fn dress_mannequin(
    mut commands: Commands,
    time: Res<Time>,
    focus: Res<WardrobeFocus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(Entity, &mut Mannequin, &mut Transform, Option<&Children>), Without<Joint>>,
    mut joints: Query<(&mut Joint, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (e, mut m, mut tf, children) in &mut q {
        let want = (focus.suit, focus.unlocked);
        if m.wearing != Some(want) {
            m.wearing = Some(want);
            let look = if focus.unlocked { focus.suit.look() } else { focus.suit.look().silhouette() };
            commands.entity(e).despawn_related::<Children>();
            crate::player::build_astronaut_rig(&mut commands, e, &mut meshes, &mut materials, look, false);
            // show the helmet first after a change of suit: face the camera again
            tf.rotation = Quat::IDENTITY;
            continue;
        }
        tf.rotate_y(crate::config::WARDROBE_SPIN * dt);
        if let Some(children) = children {
            let drive = RigDrive { speed: 0.0, grounded: true, sliding: false, vel_r: 0.0 };
            crate::player::animate_rig(&mut m.anim, drive, children, &mut joints, dt, time.elapsed_secs());
        }
    }
}

// ------------------------------------------------------------------ headless `--suits`

/// `--suits`: every astronaut is re-suited through all twelve suits, one every
/// `PROBE_EVERY` ticks, and each rig must come back in the right helmet and trim with the
/// turtle patch still on its back (the peer walks the list half a wardrobe out of step).
#[derive(Resource, Default)]
pub struct SuitsProbe {
    pub on: bool,
    ticks: u32,
    step: usize,
    /// (player, suit) rigs checked and found right
    fitted: Vec<(u8, SuitKind)>,
    errors: Vec<String>,
}

impl SuitsProbe {
    pub fn new(on: bool) -> Self {
        Self { on, ..default() }
    }
}

const PROBE_EVERY: u32 = 20;

/// Every rig part under `root` (the rig is two levels deep: joint, then part).
fn rig_parts(root: Entity, children: &Query<&Children>, parts: &Query<(&crate::player::RigPart, &MeshMaterial3d<StandardMaterial>)>, out: &mut Vec<(crate::player::RigPart, Handle<StandardMaterial>)>) {
    let Ok(kids) = children.get(root) else { return };
    for c in kids.iter() {
        if let Ok((part, mat)) = parts.get(c) {
            out.push((*part, mat.0.clone()));
        }
        rig_parts(c, children, parts, out);
    }
}

pub fn suits_probe(
    mut probe: ResMut<SuitsProbe>,
    mut q: Query<(Entity, &crate::player::PlayerId, &mut crate::run::PlayerState, &crate::player::RigHero)>,
    children: Query<&Children>,
    parts: Query<(&crate::player::RigPart, &MeshMaterial3d<StandardMaterial>)>,
    materials: Res<Assets<StandardMaterial>>,
) {
    probe.ticks += 1;
    if probe.ticks % PROBE_EVERY != 0 || probe.step > SuitKind::ALL.len() {
        return;
    }
    // check the rigs the last step asked for (refit has had PROBE_EVERY ticks to run)
    if probe.step > 0 {
        for (e, pid, ps, rig) in &q {
            let want = ps.character;
            let mut found = Vec::new();
            rig_parts(e, &children, &parts, &mut found);
            let look = want.look();
            let helmet_ok = found.iter().any(|(p, _)| *p == crate::player::RigPart::Helmet(look.helmet));
            let trim_ok = found.iter().any(|(p, m)| {
                *p == crate::player::RigPart::Trim && materials.get(m).is_some_and(|m| m.base_color.to_srgba().to_u8_array() == look.trim.to_srgba().to_u8_array())
            });
            let patch_ok = found.iter().any(|(p, _)| *p == crate::player::RigPart::TurtlePatch);
            let helmets = found.iter().filter(|(p, _)| matches!(p, crate::player::RigPart::Helmet(_))).count();
            if rig.0 != want || !helmet_ok || !trim_ok || !patch_ok || helmets != 1 {
                probe.errors.push(format!(
                    "player {} in the {} suit: rig={:?} helmet={helmet_ok} ({helmets} helmets) trim={trim_ok} patch={patch_ok}",
                    pid.0,
                    want.def().name,
                    rig.0
                ));
            } else {
                probe.fitted.push((pid.0, want));
            }
        }
    }
    let step = probe.step;
    probe.step += 1;
    if step >= SuitKind::ALL.len() {
        return;
    }
    for (_, pid, mut ps, _) in &mut q {
        ps.character = SuitKind::ALL[(step + pid.0 as usize * 6) % SuitKind::ALL.len()];
    }
}

/// The `--suits` verdict for the smoke summary: `None` when the probe was off.
pub fn probe_report(probe: &SuitsProbe) -> Option<Result<String, String>> {
    if !probe.on {
        return None;
    }
    if let Some(e) = probe.errors.first() {
        return Some(Err(format!("{} rig(s) wore the wrong suit, e.g. {e}", probe.errors.len())));
    }
    let suits: std::collections::HashSet<SuitKind> = probe.fitted.iter().map(|f| f.1).collect();
    let players: std::collections::HashSet<u8> = probe.fitted.iter().map(|f| f.0).collect();
    if suits.len() != SuitKind::ALL.len() {
        return Some(Err(format!("only {} of {} suits were fitted (run longer)", suits.len(), SuitKind::ALL.len())));
    }
    Some(Ok(format!("{} rigs checked: all {} suits fitted on {} astronaut(s), right helmet + trim, turtle patch on every one", probe.fitted.len(), suits.len(), players.len())))
}

/// Headless self-check: the twelve helmets are twelve different shapes, each with trim and
/// something that glows, and the parts every suit shares build.
pub fn self_check() -> Result<(), String> {
    let mut seen: Vec<(Helmet, usize)> = Vec::new();
    for h in Helmet::ALL {
        let [shell, trim, glow] = helmet_meshes(h);
        let n = |m: &Mesh| m.count_vertices();
        if n(&shell) == 0 || n(&trim) == 0 || n(&glow) == 0 {
            return Err(format!("the {h:?} helmet is missing its shell, trim or glow"));
        }
        let sig = n(&shell) * 1_000_000 + n(&trim) * 1000 + n(&glow);
        if let Some((other, _)) = seen.iter().find(|(_, s)| *s == sig) {
            return Err(format!("the {h:?} and {other:?} helmets are the same shape"));
        }
        seen.push((h, sig));
    }
    // a fresh sheet in each suit starts with that suit's weapon and passive
    let save = crate::save::MetaSave::default();
    let base = crate::run::PlayerState::new(SuitKind::Buzz, &save);
    for k in SuitKind::ALL {
        let ps = crate::run::PlayerState::new(k, &save);
        if ps.weapons.first().map(|w| w.kind) != Some(k.def().weapon) {
            return Err(format!("a fresh sheet in the {} suit does not carry its weapon", k.def().name));
        }
    }
    let iron = crate::run::PlayerState::new(SuitKind::Ironclad, &save);
    let chimp = crate::run::PlayerState::new(SuitKind::ChimpO, &save);
    if (iron.stats.max_hp - base.stats.max_hp - 90.0).abs() > 0.01 || chimp.stats.extra_jumps != base.stats.extra_jumps + 1 {
        return Err("the suits' passives no longer reach the sheet".into());
    }
    for (name, mesh) in [("body trim", body_trim_mesh()), ("arm cuff", arm_cuff_mesh()), ("leg cuff", leg_cuff_mesh()), ("turtle patch", turtle_patch_mesh())] {
        if mesh.count_vertices() == 0 {
            return Err(format!("the rig's {name} is empty"));
        }
    }
    Ok(())
}
