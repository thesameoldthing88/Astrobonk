//! The toon look (locked direction #7): cel-banded light, a lit rim, hard highlights, a
//! two-step flashlight cone, and ink outlines — with zero external assets and without
//! touching any of the game's material sites.
//!
//! Two central mechanisms, so every mesh any system (or any later package) spawns is toon
//! automatically:
//!
//! 1. **Lighting.** `patch_pbr_lighting` rewrites bevy_pbr's own `bevy_pbr::lighting` shader
//!    module once it has loaded: N·L goes through `toon_band` (2–3 cel steps), specular through
//!    `toon_spec` (a crisp highlight or none), the spot cone through `toon_cone`, and a sun-side
//!    rim is added. Every `StandardMaterial` imports that module, so the terrain, the 1,200
//!    instanced crowd enemies, bosses, suits and props all shade toon with no new material,
//!    no extra draw and no batching change — the patch is pure fragment ALU. Unlit materials
//!    (glows, telegraphs, UI-ish decals) never run the lighting and are untouched, so the
//!    colorblind danger palette and the flash-reduction dimming (P04) still own them.
//!    If a Bevy upgrade moves an anchor, the patch refuses as a whole and logs it (the game
//!    keeps the plain PBR look) rather than half-applying.
//! 2. **Ink.** A fullscreen pass after tonemapping (`toon_outline.wgsl`) inks silhouettes from
//!    the depth prepass and creases from the normal prepass. Screen-space: its cost does not
//!    grow with the horde.
//!
//! Per-world grading (`PlanetKind::look`) sets the ink color, the night side's fill, the
//! space backdrop (KNOWN_ISSUES L6) and the color grading whenever the stage's planet changes.
//!
//! Presentation only: nothing here simulates, so nothing is gated on the net role and the
//! headless smoke app does not build this plugin (it has no renderer).

use bevy::{
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        prepass::ViewPrepassTextures,
        FullscreenShader,
    },
    ecs::query::QueryItem,
    prelude::*,
    render::{
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_graph::{
            NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, texture_depth_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice},
        view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, ViewTarget},
        RenderApp, RenderStartup,
    },
    shader::{Shader, ShaderImport, Source},
};

use crate::config::*;
use crate::planet::CurrentPlanet;
use crate::save::MetaSave;

pub struct ToonPlugin;

impl Plugin for ToonPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "toon_outline.wgsl");
        app.add_plugins((
            ExtractComponentPlugin::<ToonInk>::default(),
            UniformComponentPlugin::<ToonInk>::default(),
        ))
        .init_resource::<LightingPatch>()
        .add_systems(
            Update,
            (
                patch_pbr_lighting.run_if(|p: Res<LightingPatch>| !p.done),
                apply_world_look,
                apply_ink_settings,
                track_sun,
            ),
        );

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_ink_pipeline)
            .add_render_graph_node::<ViewNodeRunner<InkNode>>(Core3d, InkLabel)
            // After tonemapping (so the ink is a flat display color, not an HDR value bloom
            // would smear), before FXAA (so the lines get anti-aliased with everything else).
            .add_render_graph_edges(Core3d, (Node3d::Tonemapping, InkLabel, Node3d::Fxaa));
    }
}

// ---------------------------------------------------------------- 1. toon lighting

/// Whether bevy_pbr's lighting module has been rewritten yet (it loads asynchronously).
#[derive(Resource, Default)]
struct LightingPatch {
    done: bool,
}

/// The module every StandardMaterial's fragment shader imports for its direct light.
const PBR_LIGHTING_MODULE: &str = "bevy_pbr::lighting";
/// Marks a source we already rewrote (a re-added asset must not be patched twice).
const PATCH_MARK: &str = "// ASTROBONK toon lighting";

/// The exact bevy_pbr 0.18 lines the patch rewrites, and what each becomes. Each anchor must
/// occur exactly once or nothing is patched.
const PATCH_SITES: [(&str, &str); 3] = [
    // directional_light (the sun): banded diffuse, cel highlight
    (
        "color = (diffuse + specular_light) * derived_input.NdotL;",
        "color = (diffuse + toon_spec(specular_light)) * toon_band(derived_input.NdotL);",
    ),
    // point_light (also the body of every spot light — the flashlights)
    (
        "color_times_NdotL = diffuse * derived_input.NdotL + specular_light * specular_derived_input.NdotL;",
        "color_times_NdotL = diffuse * toon_band(derived_input.NdotL) \
         + toon_spec(specular_light) * toon_band(specular_derived_input.NdotL);",
    ),
    // spot_light: the flashlight's cone in two crisp steps instead of a soft falloff
    (
        "let spot_attenuation = attenuation * attenuation;",
        "let spot_attenuation = toon_cone(attenuation * attenuation);",
    ),
];

/// The WGSL the rewritten sites call, with this build's tuning baked in. Appended to the
/// module (WGSL resolves module-scope functions in any order).
fn toon_functions() -> String {
    let f = |x: f32| format!("{x:.4}");
    let (b0, b1) = TOON_BAND_EDGES;
    let (s0, s1) = TOON_SPEC_EDGE;
    let (c0, c1) = TOON_CONE_EDGES;
    let s = TOON_BAND_SOFT;
    format!(
        r#"
{PATCH_MARK} (src/toon.rs) — cel steps on N·L: none, mid, full.
fn toon_band(n_dot_l: f32) -> f32 {{
    return {mid} * smoothstep({b0lo}, {b0hi}, n_dot_l)
        + ({full} - {mid}) * smoothstep({b1lo}, {b1hi}, n_dot_l);
}}

// Specular as a hard cel highlight: keep the core of a glossy spot, drop every soft sheen.
fn toon_spec(spec: vec3<f32>) -> vec3<f32> {{
    return spec * smoothstep({s0}, {s1}, max(spec.r, max(spec.g, spec.b)));
}}

// The spot cone in two steps: a dim outer ring, then the full core.
fn toon_cone(falloff: f32) -> f32 {{
    return {ring} * smoothstep({c0lo}, {c0hi}, falloff)
        + (1.0 - {ring}) * smoothstep({c1lo}, {c1hi}, falloff);
}}
"#,
        mid = f(TOON_BAND_MID),
        full = f(TOON_BAND_FULL),
        b0lo = f(b0 - s),
        b0hi = f(b0 + s),
        b1lo = f(b1 - s),
        b1hi = f(b1 + s),
        s0 = f(s0),
        s1 = f(s1),
        ring = f(TOON_CONE_RING),
        c0lo = f(c0 * 0.5),
        c0hi = f(c0 * 1.5),
        c1lo = f(c1 - 0.04),
        c1hi = f(c1 + 0.04),
    )
}

/// Rewrite bevy_pbr's lighting source, or say why not.
fn toon_lighting_source(src: &str) -> Result<String, String> {
    let mut out = src.to_string();
    for (anchor, toon) in PATCH_SITES {
        match out.matches(anchor).count() {
            1 => out = out.replacen(anchor, toon, 1),
            n => return Err(format!("anchor found {n}x (want 1): {anchor}")),
        }
    }
    out.push_str(&toon_functions());
    Ok(out)
}

/// Rewrite `bevy_pbr::lighting` in place once it has loaded. Changing the asset is Bevy's own
/// hot-reload path: the pipeline cache drops every pipeline that imports the module and
/// recompiles it, so this works whether or not a material was drawn before it landed (the
/// boot/menu frames).
fn patch_pbr_lighting(mut shaders: ResMut<Assets<Shader>>, mut patch: ResMut<LightingPatch>) {
    let module = ShaderImport::Custom(PBR_LIGHTING_MODULE.into());
    let Some(id) = shaders
        .iter()
        .find_map(|(id, s)| (s.import_path == module).then_some(id))
    else {
        return; // not loaded yet
    };
    patch.done = true;
    if std::env::var("TOONDBG").unwrap_or_default().contains("nopatch") { return; }
    let Some(Source::Wgsl(src)) = shaders.get(id).map(|s| &s.source) else {
        warn!("TOON: {PBR_LIGHTING_MODULE} is not WGSL — keeping the PBR look");
        return;
    };
    if src.contains(PATCH_MARK) {
        return;
    }
    match toon_lighting_source(src) {
        Ok(toon) => {
            if let Some(shader) = shaders.get_mut(id) {
                shader.source = Source::Wgsl(toon.into());
                info!("TOON lighting: {PBR_LIGHTING_MODULE} patched ({} sites)", PATCH_SITES.len());
            }
        }
        Err(why) => warn!("TOON lighting NOT applied, keeping the PBR look — {why}"),
    }
}

// ---------------------------------------------------------------- 2. ink outlines

/// The ink pass's settings, on the camera; extracted and uploaded as one uniform per view.
/// Only cameras carrying it are inked.
#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct ToonInk {
    /// rgb = ink color, a = silhouette opacity
    pub color: Vec4,
    /// x = depth edge, y = normal edge, z = line width (px at 720p), w = camera near plane
    pub params: Vec4,
    /// x/y = fade distances (m), z = crease opacity
    pub fade: Vec4,
    /// xyz = world direction toward the sun, w = rim strength (0 while there is no sun)
    pub rim: Vec4,
    /// rgb = the sun's color
    pub rim_color: Vec4,
}

impl Default for ToonInk {
    fn default() -> Self {
        let ink = crate::content::planets::PlanetKind::Moon.look().ink.to_linear();
        Self {
            color: Vec4::new(ink.red, ink.green, ink.blue, 1.0),
            params: Vec4::new(TOON_INK_DEPTH_EDGE, TOON_INK_NORMAL_EDGE, TOON_INK_PX, 0.1),
            fade: Vec4::new(TOON_INK_FADE.0, TOON_INK_FADE.1, TOON_INK_CREASE_ALPHA, 0.0),
            rim: Vec4::ZERO,
            rim_color: Vec4::ONE,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
struct InkLabel;

#[derive(Default)]
struct InkNode;

impl ViewNode for InkNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViewPrepassTextures,
        &'static ToonInk,
        &'static DynamicUniformIndex<ToonInk>,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (target, prepass, _ink, index): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline = world.resource::<InkPipeline>();
        let cache = world.resource::<PipelineCache>();
        // the pipeline is built for the HDR target the game camera always has
        if !target.is_hdr() {
            return Ok(());
        }
        let (Some(render_pipeline), Some(depth), Some(normal), Some(uniform)) = (
            cache.get_render_pipeline(pipeline.id),
            prepass.depth_view(),
            prepass.normal_view(),
            world.resource::<ComponentUniforms<ToonInk>>().uniforms().binding(),
        ) else {
            return Ok(());
        };
        let post = target.post_process_write();
        let bind_group = render_context.render_device().create_bind_group(
            "toon_ink_bind_group",
            &cache.get_bind_group_layout(&pipeline.layout),
            &BindGroupEntries::sequential((post.source, &pipeline.sampler, depth, normal, uniform)),
        );
        let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("toon_ink_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_render_pipeline(render_pipeline);
        pass.set_bind_group(0, &bind_group, &[index.index()]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

#[derive(Resource)]
struct InkPipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    id: CachedRenderPipelineId,
}

fn init_ink_pipeline(
    mut commands: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "toon_ink_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                texture_depth_2d(),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer::<ToonInk>(true),
            ),
        ),
    );
    let sampler = device.create_sampler(&SamplerDescriptor::default());
    let shader = bevy::asset::load_embedded_asset!(assets.as_ref(), "toon_outline.wgsl");
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("toon_ink_pipeline".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader,
            targets: vec![Some(ColorTargetState {
                format: ViewTarget::TEXTURE_FORMAT_HDR,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(InkPipeline { layout, sampler, id });
}

// ---------------------------------------------------------------- 3. per-world grade

/// A toon color grade: a little extra saturation overall, and less in the shadows so the
/// night side reads as cold dark rather than muddy color.
fn grading(saturation: f32, exposure: f32) -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal { post_saturation: saturation, exposure, ..default() },
        shadows: ColorGradingSection { saturation: 0.85, ..default() },
        midtones: ColorGradingSection { saturation: 1.05, ..default() },
        highlights: ColorGradingSection::default(),
    }
}

/// Whenever the stage's planet changes (every `enter_run` and stage transition inserts a
/// fresh `CurrentPlanet`, on the host and on every client): its ink, its night-side fill,
/// its space backdrop and its grade. Also when the camera is (re)spawned.
fn apply_world_look(
    planet: Option<Res<CurrentPlanet>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut cams: Query<(&mut ToonInk, &mut ColorGrading, &Projection)>,
) {
    let Some(planet) = planet else { return };
    // (iter_mut alone does not mark anything changed)
    let fresh_cam = cams.iter_mut().any(|(toon, ..)| toon.is_added());
    if !planet.is_changed() && !fresh_cam {
        return;
    }
    let def = planet.kind.def();
    let look = planet.kind.look();
    clear.0 = def.sky;
    ambient.color = look.night;
    ambient.brightness = look.night_brightness;
    let ink = look.ink.to_linear();
    for (mut toon, mut grade, proj) in &mut cams {
        toon.color = Vec4::new(ink.red, ink.green, ink.blue, 1.0);
        if let Projection::Perspective(p) = proj {
            toon.params.w = p.near;
        }
        *grade = grading(look.saturation, look.exposure);
    }
}

/// §13 high-contrast mode draws the ink thicker (the "minimum enemy-outline thickness" read).
fn apply_ink_settings(save: Option<Res<MetaSave>>, mut cams: Query<&mut ToonInk>) {
    let Some(save) = save else { return };
    let px = if save.accessibility.high_contrast { TOON_INK_PX * TOON_INK_HIGH_CONTRAST } else { TOON_INK_PX };
    for mut toon in &mut cams {
        if toon.params.z != px {
            toon.params.z = px;
        }
    }
}

/// The rim follows the stage's sun wherever it points (P07's day/night cycle turns it) and
/// dims with it (the Devoured Sun Shard); no sun (menus, between stages) means no rim.
fn track_sun(
    suns: Query<(&DirectionalLight, &GlobalTransform, &crate::items::SunLight)>,
    mut cams: Query<&mut ToonInk>,
) {
    let (rim, color) = match suns.iter().next() {
        Some((light, tf, sun)) => {
            let toward = -tf.forward().as_vec3();
            let share = (light.illuminance / sun.base.max(1.0)).clamp(0.0, 1.0);
            let c = light.color.to_linear();
            (toward.extend(TOON_RIM_STRENGTH * share), Vec4::new(c.red, c.green, c.blue, 1.0))
        }
        None => (Vec4::ZERO, Vec4::ONE),
    };
    for mut toon in &mut cams {
        if toon.rim != rim || toon.rim_color != color {
            toon.rim = rim;
            toon.rim_color = color;
        }
    }
}

/// Everything the game camera needs for the toon look (`setup_camera` spawns it).
pub fn camera_bundle() -> impl Bundle {
    use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
    (
        // the ink pass reads both prepasses; MSAA off keeps them single-sampled (and cheap
        // with 1,200 enemies), FXAA after the ink smooths every edge, lines included
        DepthPrepass,
        NormalPrepass,
        Msaa::Off,
        bevy::anti_alias::fxaa::Fxaa::default(),
        // hardware 2x2 PCF: crisp cel shadows instead of the default soft Gaussian
        if std::env::var("TOONDBG").unwrap_or_default().contains("gauss") { bevy::light::ShadowFilteringMethod::Gaussian } else { bevy::light::ShadowFilteringMethod::Hardware2x2 },
        grading(1.0, 0.0),
        ToonInk::default(),
    )
}
