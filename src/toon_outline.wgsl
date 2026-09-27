// ASTROBONK toon ink (locked direction #7) — see src/toon.rs.
//
// One fullscreen pass after tonemapping: ink where the depth prepass jumps (silhouettes) and
// where the normal prepass turns (creases), and a lit rim just inside every silhouette that
// faces the sun. Screen-space, so its cost is the same for one enemy or twelve hundred, and
// every mesh any system spawns gets it with no per-material work. (A rim in the lighting
// shader would light every grazing stretch of ground too — here it only finds real edges.)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct ToonInk {
    // rgb = ink color, a = silhouette opacity
    color: vec4<f32>,
    // x = depth edge threshold, y = normal edge threshold, z = line width (px), w = camera near
    params: vec4<f32>,
    // x/y = fade start/end distance (m), z = crease opacity, w = unused
    fade: vec4<f32>,
    // xyz = world direction toward the sun, w = rim strength (0 = no sun)
    rim: vec4<f32>,
    // rgb = the sun's color
    rim_color: vec4<f32>,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var normal_texture: texture_2d<f32>;
@group(0) @binding(4) var<uniform> ink: ToonInk;

fn clamp_px(p: vec2<i32>, dims: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0), dims - vec2<i32>(1));
}

fn depth_at(p: vec2<i32>, dims: vec2<i32>) -> f32 {
    return textureLoad(depth_texture, clamp_px(p, dims), 0);
}

fn normal_at(p: vec2<i32>, dims: vec2<i32>) -> vec3<f32> {
    return textureLoad(normal_texture, clamp_px(p, dims), 0).xyz * 2.0 - vec3<f32>(1.0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, screen_sampler, in.uv);
    let dims = vec2<i32>(textureDimensions(depth_texture));
    let p = vec2<i32>(in.position.xy);

    // Reverse-Z: 0 is infinitely far. The sky never gets ink; a silhouette's line lies on the
    // near object's side of the edge, so it hugs the shape instead of haloing it.
    let dc = depth_at(p, dims);
    if dc <= 0.0 {
        return color;
    }
    // Infinite reverse-Z perspective: view distance = near / depth.
    let dist = ink.params.w / dc;
    let fade = 1.0 - smoothstep(ink.fade.x, ink.fade.y, dist);
    if fade <= 0.0 {
        return color;
    }

    let w = max(1, i32(round(ink.params.z * f32(dims.y) / 720.0)));
    let l = depth_at(p - vec2<i32>(w, 0), dims);
    let r = depth_at(p + vec2<i32>(w, 0), dims);
    let u = depth_at(p - vec2<i32>(0, w), dims);
    let d = depth_at(p + vec2<i32>(0, w), dims);

    // Silhouette: the second difference of reverse-Z depth is ~0 on any plane however
    // steeply it is seen (depth is linear in screen space there), so grazing ground never
    // inks; it goes strongly negative on the near side of a depth jump or a convex ridge.
    let lap = max(-(l + r - 2.0 * dc), -(u + d - 2.0 * dc)) / dc;
    let t = ink.params.x;
    let silhouette = smoothstep(t, t * 2.0, lap) * ink.color.a;

    // Crease: normals turning between this pixel and its right/lower neighbour (one-sided,
    // so a crease is one line wide, not two).
    let nc = normal_at(p, dims);
    let nr = normal_at(p + vec2<i32>(w, 0), dims);
    let nd = normal_at(p + vec2<i32>(0, w), dims);
    // a neighbour that is sky (depth 0) has no normal to compare — the silhouette covers it
    let turn_r = select(0.0, 1.0 - dot(nc, nr), r > 0.0);
    let turn_d = select(0.0, 1.0 - dot(nc, nd), d > 0.0);
    let n = ink.params.y;
    let crease = smoothstep(n, n + 0.2, max(turn_r, turn_d)) * ink.fade.z;

    // Rim: within a few line widths of a silhouette (a much farther pixel nearby), on
    // surfaces turned toward the sun. The night side's normals face away — no rim there.
    var rim = 0.0;
    if ink.rim.w > 0.0 {
        var jump = 0.0;
        for (var k = 2; k <= 3; k++) {
            let o = w * k;
            jump = max(jump, dc - depth_at(p - vec2<i32>(o, 0), dims));
            jump = max(jump, dc - depth_at(p + vec2<i32>(o, 0), dims));
            jump = max(jump, dc - depth_at(p - vec2<i32>(0, o), dims));
            jump = max(jump, dc - depth_at(p + vec2<i32>(0, o), dims));
        }
        let edge = smoothstep(0.08, 0.16, jump / dc);
        let sunward = smoothstep(0.1, 0.35, dot(nc, ink.rim.xyz));
        rim = edge * sunward * ink.rim.w * fade;
    }
    let lit = color.rgb * (1.0 + rim) + ink.rim_color.rgb * (rim * 0.25);

    let a = max(silhouette, crease) * fade;
    return vec4<f32>(mix(lit, ink.color.rgb, a), color.a);
}
