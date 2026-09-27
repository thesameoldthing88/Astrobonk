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
            let (ridge, mask) = self.ridge_parts(dir);
            h += self.rugged * 1.7 * ridge.max(0.0) * mask;
        }
        // craters: smooth bowls with a raised rim
        for i in 0..self.craters {
            let (c, w) = self.crater(i);
            let ang = dir.angle_between(c);
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

    /// The ridged-mountain field before it is scaled into metres: the ridge profile (1 on a
    /// crease, falling off either side) and the alpine mask that gates it.
    fn ridge_parts(&self, dir: Vec3) -> (f32, f32) {
        let ridge = 1.0 - 2.0 * hills_n(dir, self.seed.wrapping_add(101), 6).abs();
        let mask = (hills_n(dir, self.seed.wrapping_add(202), 4) * 1.6 - 0.15).clamp(0.0, 1.0);
        (ridge, mask)
    }

    /// How much ridged mountain stands at `dir`, 0..1 (× rugged × 1.7 × amp × radius is its
    /// height in metres). Zero on a world without mountains.
    pub fn ridge(&self, dir: Vec3) -> f32 {
        if self.rugged <= 0.0 {
            return 0.0;
        }
        let (ridge, mask) = self.ridge_parts(dir);
        ridge.max(0.0) * mask
    }

    /// Crater `i`'s centre and angular radius.
    fn crater(&self, i: u32) -> (Vec3, f32) {
        let c = hash_dir(self.seed.wrapping_add(31), i);
        let w = self.crater_width
            * (0.7 + 0.6 * (i.wrapping_mul(2654435769).wrapping_add(7) % 100) as f32 / 100.0);
        (c, w)
    }

    /// Inside any crater's bowl (rim included).
    pub fn in_crater(&self, dir: Vec3) -> bool {
        (0..self.craters).any(|i| {
            let (c, w) = self.crater(i);
            dir.angle_between(c) < w
        })
    }

    /// The Grind-Line spines (GDD §4 "ridged-mountain crests expose a thin grindable
    /// spine"): the crest lines of the ridged-mountain field, as polylines of unit
    /// directions GRIND_STEP metres apart, on a planet of `radius`.
    ///
    /// A crest is FOLLOWED, not solved for: the ridge term is a creased profile times a
    /// smooth mask, and where the mask is high the crease itself has usually wandered off
    /// into the foothills — so the crest is the running maximum ACROSS the range: step along
    /// it, then re-centre on the highest point of a short cross-section. A trace stops where
    /// the range fades (below GRIND_CREST_MIN), flattens out across (the maximum sits at the
    /// section's edge: a slope, not a crest), turns harder than a rail can, drops into a
    /// crater bowl, closes its own loop, or meets a crest already traced. A pure function of
    /// the terrain — no RNG — so every machine traces the same rails.
    pub fn ridge_spines(&self, radius: f32) -> Vec<Vec<Vec3>> {
        use crate::config::{GRIND_CREST_MIN, GRIND_MAX_TURN_DEG, GRIND_MIN_LEN, GRIND_SEEDS, GRIND_STEP};
        use std::collections::HashSet;
        let mut out = Vec::new();
        if self.rugged <= 0.0 {
            return out;
        }
        // cells of crest already traced, so each crest is traced once
        let cell = 2.5 / radius;
        let key = |d: Vec3| (d / cell).floor().as_ivec3();
        let mut visited: HashSet<IVec3> = HashSet::new();
        let min_turn = GRIND_MAX_TURN_DEG.to_radians().cos();
        let usable = |d: Vec3, visited: &HashSet<IVec3>| {
            self.ridge(d) >= GRIND_CREST_MIN && !self.in_crater(d) && !visited.contains(&key(d))
        };
        // a crest can't be longer than a lap of the planet
        let max_steps = (std::f32::consts::TAU * radius / GRIND_STEP) as usize;
        for seed in fib_sphere(GRIND_SEEDS) {
            if self.ridge(seed) < GRIND_CREST_MIN || visited.contains(&key(seed)) {
                continue;
            }
            // Climb onto the crest up the slope; the crest then runs square to that climb.
            let Some(across) = self.ridge_gradient(seed).try_normalize() else { continue };
            let Some(start) = self.crest_across(seed, across, radius) else { continue };
            if !usable(start, &visited) {
                continue;
            }
            let run = start.cross(across).normalize();
            let mut line = vec![start];
            for sign in [1.0f32, -1.0] {
                let mut here = start;
                let mut heading = run * sign;
                let mut leg: Vec<Vec3> = Vec::new();
                for _ in 0..max_steps {
                    let ahead = offset_dir(here, heading, GRIND_STEP, radius);
                    let side = ahead.cross(heading).normalize_or_zero();
                    let Some(next) = self.crest_across(ahead, side, radius) else { break };
                    if !usable(next, &visited) {
                        break;
                    }
                    let step = next - here;
                    let Some(dir) = (step - next * step.dot(next)).try_normalize() else { break };
                    if dir.dot(heading) < min_turn {
                        break;
                    }
                    // back where it started: a closed ring, so stop before doubling it
                    if leg.len() > 10 && arc_dist(next, start, radius) < 2.0 {
                        break;
                    }
                    leg.push(next);
                    here = next;
                    heading = dir;
                }
                if sign > 0.0 {
                    line.extend(leg);
                } else {
                    leg.reverse();
                    leg.extend(line);
                    line = leg;
                }
            }
            for d in &line {
                visited.insert(key(*d));
            }
            if (line.len() - 1) as f32 * GRIND_STEP >= GRIND_MIN_LEN {
                out.push(line);
            }
        }
        out
    }

    /// Gradient of the ridge term on the tangent plane at `dir`, per radian.
    fn ridge_gradient(&self, dir: Vec3) -> Vec3 {
        let (t, b) = tangent_frame(dir);
        let e = 1e-3;
        let dt = self.ridge((dir + t * e).normalize()) - self.ridge((dir - t * e).normalize());
        let db = self.ridge((dir + b * e).normalize()) - self.ridge((dir - b * e).normalize());
        (t * dt + b * db) / (2.0 * e)
    }

    /// The highest point of the ridge term on a short cross-section through `dir` along
    /// the unit tangent `side`, or None when the section holds no crest — its maximum sits
    /// at an end, so this is a slope rather than a ridge.
    fn crest_across(&self, dir: Vec3, side: Vec3, radius: f32) -> Option<Vec3> {
        const HALF: f32 = 2.0; // metres either side
        const SAMPLES: usize = 9;
        let at = |x: f32| offset_dir(dir, side, x, radius);
        let xs = |k: usize| (k as f32 / (SAMPLES - 1) as f32 * 2.0 - 1.0) * HALF;
        let (best, _) = (0..SAMPLES)
            .map(|k| (k, self.ridge(at(xs(k)))))
            .fold((0, f32::MIN), |acc, (k, v)| if v > acc.1 { (k, v) } else { acc });
        if best == 0 || best == SAMPLES - 1 {
            return None;
        }
        // refine: step toward whichever neighbour is higher, halving the step each time
        let mut x = xs(best);
        let mut h = HALF / (SAMPLES - 1) as f32;
        for _ in 0..6 {
            let (l, c, r) = (self.ridge(at(x - h)), self.ridge(at(x)), self.ridge(at(x + h)));
            if l > c && l >= r {
                x -= h;
            } else if r > c {
                x += h;
            }
            h *= 0.5;
        }
        Some(at(x))
    }

    /// The terrain's slope at `dir` on a planet of `radius`: the tangent vector pointing
    /// straight UPHILL, with length = rise per metre (0 on flat ground). A sliding
    /// astronaut accelerates down it (§4 slope-boost).
    pub fn slope(&self, dir: Vec3, radius: f32) -> Vec3 {
        let (t, b) = tangent_frame(dir);
        let e = 0.5 / radius; // half a metre either side
        let s = |d: Vec3| self.surface(d.normalize(), radius);
        let dt = s(dir + t * e) - s(dir - t * e);
        let db = s(dir + b * e) - s(dir - b * e);
        (t * dt + b * db) / (2.0 * e * radius)
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
