//! Spherical-world math. Positions on a planet are unit direction vectors from the
//! planet core; the terrain is an analytic noise field over directions, so collision,
//! prop placement and the render mesh all sample the same function.

use bevy::prelude::*;

/// Fixed pseudo-random unit vectors + frequencies for the hill field (deterministic).
const HILL_WAVES: [(Vec3, f32, f32); 13] = [
    (Vec3::new(0.267, 0.535, 0.802), 3.0, 0.0),
    (Vec3::new(-0.577, 0.577, 0.577), 4.0, 1.3),
    (Vec3::new(0.707, -0.707, 0.0), 5.0, 2.1),
    (Vec3::new(0.0, 0.6, -0.8), 6.0, 4.2),
    (Vec3::new(-0.802, 0.267, 0.535), 7.0, 0.7),
    (Vec3::new(0.484, 0.727, -0.485), 9.0, 3.3),
    (Vec3::new(-0.371, -0.557, 0.743), 11.0, 5.1),
    (Vec3::new(0.9, 0.1, -0.424), 13.0, 2.8),
    (Vec3::new(-0.141, 0.99, 0.0), 17.0, 1.9),
    (Vec3::new(0.5, -0.5, -0.707), 21.0, 0.4),
    // finer detail octaves (small amplitude → visual crinkle, gentle on movement)
    (Vec3::new(-0.667, -0.333, 0.667), 27.0, 3.7),
    (Vec3::new(0.333, -0.667, -0.667), 34.0, 5.5),
    (Vec3::new(-0.408, 0.816, -0.408), 43.0, 1.1),
];

/// Smooth height field in [-1, 1] over the unit sphere. `seed` de-correlates planets.
pub fn hills(dir: Vec3, seed: u32) -> f32 {
    let s = seed as f32 * 1.618;
    let mut h = 0.0;
    let mut amp = 1.0;
    let mut total = 0.0;
    for (i, (v, f, p)) in HILL_WAVES.iter().enumerate() {
        h += amp * (f * dir.dot(*v) + p + s * (i as f32 + 1.0)).sin();
        total += amp;
        amp *= 0.75;
    }
    h / total
}

/// Radial distance from core to terrain surface along `dir`.
pub fn surface_radius(dir: Vec3, radius: f32, hill_amp: f32, seed: u32) -> f32 {
    radius * (1.0 + hill_amp * hills(dir, seed))
}

/// Cheaper hill variant using only the first `n` waves (for masks).
fn hills_n(dir: Vec3, seed: u32, n: usize) -> f32 {
    let s = seed as f32 * 1.618;
    let mut h = 0.0;
    let mut amp = 1.0;
    let mut total = 0.0;
    for (i, (v, f, p)) in HILL_WAVES.iter().take(n).enumerate() {
        h += amp * (f * dir.dot(*v) + p + s * (i as f32 + 1.0)).sin();
        total += amp;
        amp *= 0.75;
    }
    h / total
}

/// Deterministic pseudo-random unit vector from (seed, i) — crater centers etc.
pub fn hash_dir(seed: u32, i: u32) -> Vec3 {
    let mut x = seed.wrapping_mul(747796405).wrapping_add(i.wrapping_mul(2891336453)).wrapping_add(1);
    let mut next = || {
        x ^= x >> 16;
        x = x.wrapping_mul(2654435769);
        x ^= x >> 13;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    Vec3::new(next(), next(), next()).try_normalize().unwrap_or(Vec3::Y)
}

/// Full terrain description: smooth hills + ridged mountain chains + craters.
/// `height()` is unitless (~-1.5 .. +2); scale by `amp * radius` for meters.
#[derive(Clone, Copy)]
pub struct Terrain {
    pub seed: u32,
    pub amp: f32,
    pub rugged: f32,
    pub craters: u32,
    pub crater_depth: f32,
    pub crater_width: f32, // radians
}

impl Terrain {
    pub fn height(&self, dir: Vec3) -> f32 {
        let smooth = hills(dir, self.seed);
        // ridged mountains, gated by a smooth mask so only some regions are alpine
        let mut h = smooth;
        if self.rugged > 0.0 {
            let ridge = 1.0 - 2.0 * hills_n(dir, self.seed.wrapping_add(101), 6).abs();
            let mask = (hills_n(dir, self.seed.wrapping_add(202), 4) * 1.6 - 0.15).clamp(0.0, 1.0);
            h += self.rugged * 1.7 * ridge.max(0.0) * mask;
        }
        // craters: smooth bowls with a raised rim
        for i in 0..self.craters {
            let c = hash_dir(self.seed.wrapping_add(31), i);
            let ang = dir.angle_between(c);
            let w = self.crater_width
                * (0.7 + 0.6 * (i.wrapping_mul(2654435769).wrapping_add(7) % 100) as f32 / 100.0);
            if ang < w {
                let t = ang / w;
                let bowl = -((t * std::f32::consts::PI).cos() * 0.5 + 0.5);
                let rim = (t * std::f32::consts::PI).sin().powi(3) * 0.35;
                h += self.crater_depth * (bowl + rim);
            }
        }
        h
    }

    pub fn surface(&self, dir: Vec3, radius: f32) -> f32 {
        radius * (1.0 + self.amp * self.height(dir))
    }
}

/// A stable tangent frame at `up` (any unit vector).
pub fn tangent_frame(up: Vec3) -> (Vec3, Vec3) {
    let helper = if up.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let t = helper.cross(up).normalize();
    let b = up.cross(t);
    (t, b)
}

/// Rotation that stands an object on the sphere: local +Y = `up`, local -Z = `forward`.
pub fn frame_quat(up: Vec3, forward: Vec3) -> Quat {
    let fwd = (forward - up * forward.dot(up)).try_normalize().unwrap_or_else(|| tangent_frame(up).0);
    let right = fwd.cross(up).normalize();
    Quat::from_mat3(&Mat3::from_cols(right, up, -fwd))
}

/// Move a unit direction along the great circle toward `target` by `angle` radians.
/// Never overshoots. Returns `dir` unchanged if the two are (anti)parallel.
pub fn step_toward(dir: Vec3, target: Vec3, angle: f32) -> Vec3 {
    let full = dir.angle_between(target);
    if full < 1e-5 || full > std::f32::consts::PI - 1e-4 {
        return dir;
    }
    let t = (angle / full).min(1.0);
    dir.slerp(target, t).normalize()
}

/// Advance a unit direction by a world-space tangent velocity for `dt` seconds,
/// as if rolling over a sphere of radius `r`. Returns (new_dir, reprojected_vel).
pub fn advance(dir: Vec3, vel: Vec3, r: f32, dt: f32) -> (Vec3, Vec3) {
    let v_t = vel - dir * vel.dot(dir);
    let dist = v_t.length() * dt;
    if dist < 1e-7 {
        return (dir, v_t);
    }
    let axis = dir.cross(v_t.normalize());
    let new_dir = (Quat::from_axis_angle(axis.normalize(), dist / r) * dir).normalize();
    let new_vel = v_t - new_dir * v_t.dot(new_dir);
    (new_dir, new_vel.normalize_or_zero() * v_t.length())
}

/// Great-circle distance between two unit directions on a sphere of radius `r`.
pub fn arc_dist(a: Vec3, b: Vec3, r: f32) -> f32 {
    a.angle_between(b) * r
}

/// A point at `arc` meters from `dir` in the tangent direction `heading` (unit, tangent).
pub fn offset_dir(dir: Vec3, heading: Vec3, arc: f32, r: f32) -> Vec3 {
    let axis = dir.cross(heading).normalize_or_zero();
    if axis == Vec3::ZERO {
        return dir;
    }
    (Quat::from_axis_angle(axis, arc / r) * dir).normalize()
}

/// Deterministic scatter of `n` roughly-even directions (Fibonacci sphere).
pub fn fib_sphere(n: usize) -> impl Iterator<Item = Vec3> {
    let golden = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    (0..n).map(move |i| {
        let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
        let r = (1.0 - y * y).sqrt();
        let th = golden * i as f32;
        Vec3::new(th.cos() * r, y, th.sin() * r)
    })
}
