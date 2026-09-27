//! Procedural mesh compositor. Bakes several primitive shapes (transformed) into a
//! single `Mesh` so a detailed model still renders as ONE instanced draw call — the
//! trick that lets 1,200 enemies each have a real silhouette without 1,200×N entities.
//!
//! Vertex colors are a *multiplier* over the material's base color: pass `Color::WHITE`
//! for a body part (shows the material color fully) and a darker shade for accents
//! (visors, mouths, undersides) so each kind keeps one readable signal color + detail.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

#[derive(Default)]
pub struct MeshData {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self::default()
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
            self.push(&v, &[n; 4], &[0, 1, 2, 0, 2, 3], tf, color);
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
            tris.extend([b0, nb, b0 + 1, nb, nb + 1, b0 + 1]);
            let _ = b;
        }
        // caps
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
                    tris.extend([center, ring + n, ring + i]);
                } else {
                    tris.extend([center, ring + i, ring + n]);
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
            tris.extend([b, b + 1, b + 2]);
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
            tris.extend([center, ring + n, ring + i]);
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

    /// `build`, with every triangle wound counter-clockwise seen from outside, which is what
    /// Bevy treats as the front face. The shape helpers above wind clockwise; under default
    /// back-face culling a model built with `build` shows its far walls instead of its near
    /// ones — the silhouette is identical, which is why the crowd has always looked right.
    /// Anything that relies on WHICH faces get culled (the §13 inverted-hull outlines) must
    /// build with this.
    pub fn build_ccw(mut self) -> Mesh {
        for tri in self.idx.chunks_mut(3) {
            tri.swap(1, 2);
        }
        self.build()
    }

    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.col);
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
