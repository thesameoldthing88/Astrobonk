//! Procedural mesh compositor. Bakes several primitive shapes (transformed) into a
//! single `Mesh` so a detailed model still renders as ONE instanced draw call — the
//! trick that lets 1,200 enemies each have a real silhouette without 1,200×N entities.
//!
//! Vertex colors are a *multiplier* over the material's base color: pass `Color::WHITE`
//! for a body part (shows the material color fully) and a darker shade for accents
//! (visors, mouths, undersides) so each kind keeps one readable signal color + detail.
//!
//! Every shape is wound counter-clockwise seen from outside — Bevy's front face — so under
//! default back-face culling the NEAR walls draw, lit by their own normals, and an inverted
//! hull (front faces culled) shows the far ones. `winding_self_check` pins it.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Default)]
pub struct MeshData {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    /// Emissive mask coordinate per vertex: `GLOW_UV` for accent parts, `BODY_UV` otherwise.
    uv: Vec<[f32; 2]>,
    idx: Vec<u32>,
    glow: bool,
}

/// UVs into `glow_mask_image`, a 2×1 texture: left texel black, right texel white. A
/// material that uses it as its `emissive_texture` lights only the accent parts, so one
/// material (one draw call per kind) can carry one glowing detail.
pub const BODY_UV: [f32; 2] = [0.25, 0.5];
pub const GLOW_UV: [f32; 2] = [0.75, 0.5];

/// The 2×1 emissive mask the `GLOW_UV`/`BODY_UV` coordinates sample (nearest filtering, so
/// the two texels never bleed into each other).
pub fn glow_mask_image() -> Image {
    use bevy::image::ImageSampler;
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut img = Image::new(
        Extent3d { width: 2, height: 1, depth_or_array_layers: 1 },
        TextureDimension::D2,
        vec![0, 0, 0, 255, 255, 255, 255, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.sampler = ImageSampler::nearest();
    img
}

impl MeshData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shapes added while this is on are the model's emissive accent (see `GLOW_UV`).
    pub fn set_glow(&mut self, on: bool) {
        self.glow = on;
    }

    fn push(&mut self, verts: &[Vec3], normals: &[Vec3], tris: &[u32], tf: Transform, color: Color) {
        let base = self.pos.len() as u32;
        let c = color.to_linear();
        let ca = [c.red, c.green, c.blue, c.alpha];
        for (v, n) in verts.iter().zip(normals.iter()) {
            let p = tf.transform_point(*v);
            let nn = (tf.rotation * *n).normalize_or_zero();
            self.pos.push([p.x, p.y, p.z]);
            self.nrm.push([nn.x, nn.y, nn.z]);
            self.col.push(ca);
            self.uv.push(if self.glow { GLOW_UV } else { BODY_UV });
        }
        self.idx.extend(tris.iter().map(|i| base + i));
    }

    /// Axis-aligned box of full extents `size`, faceted (per-face normals).
    pub fn add_box(&mut self, size: Vec3, tf: Transform, color: Color) {
        let (hx, hy, hz) = (size.x * 0.5, size.y * 0.5, size.z * 0.5);
        // 6 faces × 4 verts, each with its own normal for the chunky look.
        let faces: [(Vec3, [Vec3; 4]); 6] = [
            (Vec3::X, [Vec3::new(hx, -hy, -hz), Vec3::new(hx, -hy, hz), Vec3::new(hx, hy, hz), Vec3::new(hx, hy, -hz)]),
            (Vec3::NEG_X, [Vec3::new(-hx, -hy, hz), Vec3::new(-hx, -hy, -hz), Vec3::new(-hx, hy, -hz), Vec3::new(-hx, hy, hz)]),
            (Vec3::Y, [Vec3::new(-hx, hy, -hz), Vec3::new(hx, hy, -hz), Vec3::new(hx, hy, hz), Vec3::new(-hx, hy, hz)]),
            (Vec3::NEG_Y, [Vec3::new(-hx, -hy, hz), Vec3::new(hx, -hy, hz), Vec3::new(hx, -hy, -hz), Vec3::new(-hx, -hy, -hz)]),
            (Vec3::Z, [Vec3::new(-hx, -hy, hz), Vec3::new(hx, -hy, hz), Vec3::new(hx, hy, hz), Vec3::new(-hx, hy, hz)]),
            (Vec3::NEG_Z, [Vec3::new(hx, -hy, -hz), Vec3::new(-hx, -hy, -hz), Vec3::new(-hx, hy, -hz), Vec3::new(hx, hy, -hz)]),
        ];
        for (n, v) in faces {
            // wind each quad so it faces out along its normal, whichever way the table
            // lists its corners (the ±X/±Y rows run clockwise seen from outside)
            let ccw = (v[1] - v[0]).cross(v[2] - v[0]).dot(n) > 0.0;
            let tris = if ccw { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
            self.push(&v, &[n; 4], &tris, tf, color);
        }
    }

    /// Smooth icosphere of radius `r` (normals = direction from center).
    pub fn add_sphere(&mut self, r: f32, subdiv: u32, tf: Transform, color: Color) {
        let (verts, tris) = icosphere(subdiv);
        let scaled: Vec<Vec3> = verts.iter().map(|v| *v * r).collect();
        self.push(&scaled, &verts, &tris, tf, color);
    }

    /// A squashed/stretched sphere — good for domes, bellies, saucers.
    pub fn add_ellipsoid(&mut self, radii: Vec3, subdiv: u32, tf: Transform, color: Color) {
        let (verts, _) = icosphere(subdiv);
        let tris: Vec<u32> = icosphere(subdiv).1;
        let scaled: Vec<Vec3> = verts.iter().map(|v| Vec3::new(v.x * radii.x, v.y * radii.y, v.z * radii.z)).collect();
        let normals: Vec<Vec3> = verts
            .iter()
            .map(|v| Vec3::new(v.x / radii.x.max(1e-3), v.y / radii.y.max(1e-3), v.z / radii.z.max(1e-3)).normalize_or_zero())
            .collect();
        self.push(&scaled, &normals, &tris, tf, color);
    }

    /// Cylinder along +Y, centered, radius `r`, height `h`.
    pub fn add_cylinder(&mut self, r: f32, h: f32, seg: usize, tf: Transform, color: Color) {
        let (hy, seg) = (h * 0.5, seg.max(3));
        let mut verts = Vec::new();
        let mut norms = Vec::new();
        let mut tris = Vec::new();
        // side
        for i in 0..seg {
            let a = i as f32 / seg as f32 * std::f32::consts::TAU;
            let (c, s) = (a.cos(), a.sin());
            let n = Vec3::new(c, 0.0, s);
            let b = verts.len() as u32;
            verts.push(Vec3::new(c * r, -hy, s * r));
            verts.push(Vec3::new(c * r, hy, s * r));
            norms.push(n);
            norms.push(n);
            let nb = ((i + 1) % seg) as u32 * 2;
            let b0 = i as u32 * 2;
            tris.extend([b0, b0 + 1, nb, nb, b0 + 1, nb + 1]);
            let _ = b;
        }
        // caps
        // the ring runs counter-clockwise seen from +Y, so the top cap keeps its order and
        // the bottom one (seen from -Y) reverses it
        for (yy, ny, flip) in [(hy, Vec3::Y, false), (-hy, Vec3::NEG_Y, true)] {
            let center = verts.len() as u32;
            verts.push(Vec3::new(0.0, yy, 0.0));
            norms.push(ny);
            let ring = verts.len() as u32;
            for i in 0..seg {
                let a = i as f32 / seg as f32 * std::f32::consts::TAU;
                verts.push(Vec3::new(a.cos() * r, yy, a.sin() * r));
                norms.push(ny);
            }
            for i in 0..seg as u32 {
                let n = (i + 1) % seg as u32;
                if flip {
                    tris.extend([center, ring + i, ring + n]);
                } else {
                    tris.extend([center, ring + n, ring + i]);
                }
            }
        }
        self.push(&verts, &norms, &tris, tf, color);
    }

    /// Cone along +Y: base radius `r` at bottom, apex at top, height `h`.
    pub fn add_cone(&mut self, r: f32, h: f32, seg: usize, tf: Transform, color: Color) {
        let (hy, seg) = (h * 0.5, seg.max(3));
        let mut verts = Vec::new();
        let mut norms = Vec::new();
        let mut tris = Vec::new();
        let apex = Vec3::new(0.0, hy, 0.0);
        let slope = (r / h).atan();
        for i in 0..seg {
            let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
            let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
            let p0 = Vec3::new(a0.cos() * r, -hy, a0.sin() * r);
            let p1 = Vec3::new(a1.cos() * r, -hy, a1.sin() * r);
            let am = (a0 + a1) * 0.5;
            let n = Vec3::new(am.cos() * slope.cos(), slope.sin(), am.sin() * slope.cos());
            let b = verts.len() as u32;
            verts.extend([apex, p0, p1]);
            norms.extend([n, n, n]);
            tris.extend([b, b + 2, b + 1]);
        }
        // base cap
        let center = verts.len() as u32;
        verts.push(Vec3::new(0.0, -hy, 0.0));
        norms.push(Vec3::NEG_Y);
        let ring = verts.len() as u32;
        for i in 0..seg {
            let a = i as f32 / seg as f32 * std::f32::consts::TAU;
            verts.push(Vec3::new(a.cos() * r, -hy, a.sin() * r));
            norms.push(Vec3::NEG_Y);
        }
        for i in 0..seg as u32 {
            let n = (i + 1) % seg as u32;
            tris.extend([center, ring + i, ring + n]);
        }
        self.push(&verts, &norms, &tris, tf, color);
    }

    /// Torus lying in the XZ plane round +Y: ring radius `major`, tube radius `minor`.
    pub fn add_torus(&mut self, major: f32, minor: f32, seg: usize, tube_seg: usize, tf: Transform, color: Color) {
        let (seg, tube_seg) = (seg.max(3), tube_seg.max(3));
        let mut verts = Vec::with_capacity(seg * tube_seg);
        let mut norms = Vec::with_capacity(seg * tube_seg);
        for i in 0..seg {
            let u = i as f32 / seg as f32 * std::f32::consts::TAU;
            let (cu, su) = (u.cos(), u.sin());
            for j in 0..tube_seg {
                let v = j as f32 / tube_seg as f32 * std::f32::consts::TAU;
                let n = Vec3::new(v.cos() * cu, v.sin(), v.cos() * su);
                verts.push(Vec3::new(cu * major, 0.0, su * major) + n * minor);
                norms.push(n);
            }
        }
        let mut tris = Vec::with_capacity(seg * tube_seg * 6);
        let id = |i: usize, j: usize| ((i % seg) * tube_seg + (j % tube_seg)) as u32;
        for i in 0..seg {
            for j in 0..tube_seg {
                // round the ring (u) runs toward +Z, round the tube (v) toward +Y: (a, d, b)
                // is counter-clockwise seen from outside
                let (a, b, c, d) = (id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1));
                tris.extend([a, d, b, b, d, c]);
            }
        }
        self.push(&verts, &norms, &tris, tf, color);
    }

    /// Capsule along +Y (cylinder body + two hemispherical-ish caps).
    pub fn add_capsule(&mut self, r: f32, body_h: f32, tf: Transform, color: Color) {
        self.add_cylinder(r, body_h, 10, tf, color);
        let up = tf.rotation * Vec3::Y * (body_h * 0.5);
        self.add_sphere(r, 1, tf.with_translation(tf.translation + up), color);
        self.add_sphere(r, 1, tf.with_translation(tf.translation - up), color);
    }

    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uv);
        mesh.insert_indices(Indices::U32(self.idx));
        mesh
    }
}

/// Convenience: transform helper.
pub fn at(pos: Vec3) -> Transform {
    Transform::from_translation(pos)
}

/// Icosphere on the unit sphere: (vertices, triangle indices). Shared by terrain,
/// props, and the compositor.
pub fn icosphere(subdiv: u32) -> (Vec<Vec3>, Vec<u32>) {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut verts: Vec<Vec3> = [
        (-1.0, t, 0.0), (1.0, t, 0.0), (-1.0, -t, 0.0), (1.0, -t, 0.0),
        (0.0, -1.0, t), (0.0, 1.0, t), (0.0, -1.0, -t), (0.0, 1.0, -t),
        (t, 0.0, -1.0), (t, 0.0, 1.0), (-t, 0.0, -1.0), (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalize())
    .collect();

    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];

    use std::collections::HashMap;
    for _ in 0..subdiv {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut midpoint = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
            let key = (a.min(b), a.max(b));
            *cache.entry(key).or_insert_with(|| {
                let m = ((verts[a as usize] + verts[b as usize]) / 2.0).normalize();
                verts.push(m);
                (verts.len() - 1) as u32
            })
        };
        let mut next = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces {
            let ab = midpoint(a, b, &mut verts);
            let bc = midpoint(b, c, &mut verts);
            let ca = midpoint(c, a, &mut verts);
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        faces = next;
    }
    (verts, faces.into_iter().flatten().collect())
}

/// Headless self-check (M12): every triangle of every shape faces the way its normals say,
/// under an arbitrary rotation. The ±X/±Y box faces, cylinders and cones used to be wound
/// clockwise, so back-face culling drew their far walls (lit from behind) and an inverted-
/// hull outline built from them culled the wrong side.
pub fn winding_self_check() -> Result<(), String> {
    let tf = Transform::from_translation(Vec3::new(0.3, -1.2, 2.0))
        .with_rotation(Quat::from_euler(EulerRot::XYZ, 0.7, -1.1, 0.4));
    let shapes: [(&str, fn(&mut MeshData, Transform)); 7] = [
        ("box", |m, tf| m.add_box(Vec3::new(0.8, 1.3, 0.5), tf, Color::WHITE)),
        ("cylinder", |m, tf| m.add_cylinder(0.4, 1.1, 9, tf, Color::WHITE)),
        ("cone", |m, tf| m.add_cone(0.5, 0.9, 7, tf, Color::WHITE)),
        ("sphere", |m, tf| m.add_sphere(0.6, 1, tf, Color::WHITE)),
        ("ellipsoid", |m, tf| m.add_ellipsoid(Vec3::new(0.7, 0.3, 0.5), 1, tf, Color::WHITE)),
        ("capsule", |m, tf| m.add_capsule(0.3, 0.8, tf, Color::WHITE)),
        ("torus", |m, tf| m.add_torus(0.5, 0.12, 16, 6, tf, Color::WHITE)),
    ];
    for (name, add) in shapes {
        let mut m = MeshData::new();
        add(&mut m, tf);
        for tri in m.idx.chunks(3) {
            let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(m.pos[i as usize]));
            let face = (b - a).cross(c - a);
            let normal: Vec3 = [tri[0], tri[1], tri[2]].iter().map(|i| Vec3::from_array(m.nrm[*i as usize])).sum();
            if face.length_squared() > 1e-12 && face.dot(normal) <= 0.0 {
                return Err(format!("meshkit {name}: a triangle is wound clockwise seen from outside"));
            }
        }
    }
    Ok(())
}
