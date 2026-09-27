use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanetKind {
    Moon,
    Mars,
    DarkMoon,
}

/// What grows (or pretends to grow) on a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloraStyle {
    /// Pale mineral spires — lifeless worlds.
    Spires,
    /// Rust-red thorn shrubs.
    Thorns,
    /// Bioluminescent fungus trees.
    GlowShrooms,
}

pub struct PlanetDef {
    pub kind: PlanetKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub radius: f32,
    pub hill_amp: f32,
    pub rugged: f32,
    pub craters: u32,
    pub crater_depth: f32,
    pub seed: u32,
    pub ground: Color,
    pub ground_low: Color,
    pub ground_high: Color,
    pub sky: Color,
    pub sun: Color,
    pub enemy_tint: Color,
    pub rocks: usize,
    pub crystals: usize,
    pub pots: usize,
    pub flora: usize,
    pub flora_style: FloraStyle,
    pub has_earthrise: bool,
    pub meteor_showers: bool,
    /// The §3 planet multiplier `T` on enemy HP and damage — "hotter/weirder deeper" (§8),
    /// so a world's bite matches where it sits in the campaign map.
    pub threat: f32,
}

/// A world's toon grade (locked direction #7), applied by `toon::apply_world_look` whenever
/// the stage's planet changes. Its sky, sun and ground colors stay on `PlanetDef`.
pub struct ToonLook {
    /// Ink outline color: a deep shade of the world, never pure black, never danger red.
    pub ink: Color,
    /// The night side's only light (the global ambient): tint and brightness. Dark enough
    /// that the flashlight cone is THE read at night, cold enough to feel like dread.
    pub night: Color,
    pub night_brightness: f32,
    /// Color grading: post-tonemap saturation, so the flat cel colors pop...
    pub saturation: f32,
    /// ...and exposure (EV): cel bands light a face fully or not at all, so a bright world
    /// (the pale Moon) needs stopping down to keep its lit side from bleaching to white.
    pub exposure: f32,
}

impl PlanetKind {
    pub const ALL: [PlanetKind; 3] = [PlanetKind::Moon, PlanetKind::Mars, PlanetKind::DarkMoon];

    pub fn def(&self) -> PlanetDef {
        use PlanetKind::*;
        match self {
            Moon => PlanetDef {
                kind: *self,
                name: "THE MOON",
                desc: "Home turf. Well. Near-home turf.",
                radius: 140.0,
                hill_amp: 0.035,
                rugged: 0.35,
                craters: 10,
                crater_depth: 0.8,
                seed: 7,
                ground: Color::srgb(0.54, 0.55, 0.63),
                ground_low: Color::srgb(0.3, 0.31, 0.43),
                ground_high: Color::srgb(0.78, 0.79, 0.88),
                sky: Color::srgb(0.016, 0.022, 0.06),
                sun: Color::srgb(1.0, 0.98, 0.92),
                enemy_tint: Color::srgb(0.5, 0.9, 0.6),
                rocks: 200,
                crystals: 42,
                pots: 60,
                flora: 40,
                flora_style: FloraStyle::Spires,
                has_earthrise: true,
                meteor_showers: true,
                threat: 1.0,
            },
            Mars => PlanetDef {
                kind: *self,
                name: "MARS",
                desc: "Red, dead, and full of teeth.",
                radius: 160.0,
                hill_amp: 0.045,
                rugged: 0.55,
                craters: 6,
                crater_depth: 0.5,
                seed: 23,
                ground: Color::srgb(0.74, 0.46, 0.30),
                ground_low: Color::srgb(0.44, 0.27, 0.21),
                ground_high: Color::srgb(0.88, 0.66, 0.46),
                sky: Color::srgb(0.04, 0.02, 0.045),
                sun: Color::srgb(1.0, 0.85, 0.7),
                enemy_tint: Color::srgb(0.95, 0.55, 0.35),
                rocks: 250,
                crystals: 52,
                pots: 68,
                flora: 64,
                flora_style: FloraStyle::Thorns,
                has_earthrise: false,
                meteor_showers: true,
                threat: 1.1,
            },
            DarkMoon => PlanetDef {
                kind: *self,
                name: "THE DARK MOON",
                desc: "It was not on any chart.",
                radius: 105.0,
                hill_amp: 0.03,
                rugged: 0.8,
                craters: 4,
                crater_depth: 0.6,
                seed: 66,
                ground: Color::srgb(0.20, 0.16, 0.28),
                ground_low: Color::srgb(0.09, 0.07, 0.15),
                ground_high: Color::srgb(0.36, 0.28, 0.46),
                sky: Color::srgb(0.03, 0.008, 0.055),
                sun: Color::srgb(0.74, 0.6, 0.95),
                enemy_tint: Color::srgb(0.8, 0.5, 1.0),
                rocks: 120,
                crystals: 85,
                pots: 38,
                flora: 55,
                flora_style: FloraStyle::GlowShrooms,
                has_earthrise: false,
                meteor_showers: false,
                threat: 1.25,
            },
        }
    }

    /// The toon grade (see `ToonLook`). Exhaustive on purpose: a new world must pick one.
    pub fn look(&self) -> ToonLook {
        use PlanetKind::*;
        match self {
            Moon => ToonLook {
                ink: Color::srgb(0.05, 0.06, 0.15),
                night: Color::srgb(0.55, 0.64, 1.0),
                night_brightness: 70.0,
                saturation: 1.1,
                exposure: -0.7,
            },
            Mars => ToonLook {
                ink: Color::srgb(0.13, 0.05, 0.05),
                night: Color::srgb(0.72, 0.55, 0.9),
                night_brightness: 70.0,
                saturation: 1.0,
                exposure: -0.25,
            },
            DarkMoon => ToonLook {
                ink: Color::srgb(0.06, 0.02, 0.1),
                night: Color::srgb(0.5, 0.45, 1.0),
                night_brightness: 60.0,
                saturation: 1.05,
                exposure: -0.15,
            },
        }
    }

    /// Stage chain: higher tiers chain more worlds (Megabonk-style).
    /// Moon T1 = [Moon], T2 = [Moon, Mars], T3 = [Moon, Mars, DarkMoon].
    /// Mars runs standalone once unlocked.
    pub fn chain_from(start: PlanetKind, tier: u32) -> Vec<PlanetKind> {
        match (start, tier) {
            (PlanetKind::Moon, 1) => vec![PlanetKind::Moon],
            (PlanetKind::Moon, 2) => vec![PlanetKind::Moon, PlanetKind::Mars],
            (PlanetKind::Moon, _) => vec![PlanetKind::Moon, PlanetKind::Mars, PlanetKind::DarkMoon],
            (p, _) => vec![p],
        }
    }

    /// Highest selectable tier for a starting planet.
    pub fn max_tier(start: PlanetKind) -> u32 {
        match start {
            PlanetKind::Moon => 3,
            _ => 1,
        }
    }
}
