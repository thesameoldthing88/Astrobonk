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

/// How a world is lit (§4 day/night, §12 "Color & lighting per world"). The sun's key light
/// sweeps the surface over a stage (`daynight`); ambient is deliberately dim so the night
/// side is dark and the flashlight earns its keep.
#[derive(Clone, Copy, Debug)]
pub struct SkyDef {
    /// Illuminance of the sun's key light (lux) on the day side.
    pub sun_lux: f32,
    /// Ambient brightness with the local astronaut in full day / deep night.
    pub ambient_day: f32,
    pub ambient_night: f32,
    /// Ambient tint by day and by night (§12: the Moon's night is blue-black under
    /// Earthlight, Mars's a muddy brown, the Dark Moon's near-black).
    pub ambient_day_color: Color,
    pub ambient_night_color: Color,
    /// Where the spin axis points: a turn about the core's Y axis (radians). The axis itself
    /// always lies in the crash site's horizon, so the sun passes over it.
    pub axis_yaw: f32,
    /// How far (radians) the sun's path leans off the equator — a pole in long twilight.
    pub declination: f32,
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
    pub light: SkyDef,
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
                ground: Color::srgb(0.62, 0.62, 0.66),
                ground_low: Color::srgb(0.40, 0.40, 0.46),
                ground_high: Color::srgb(0.82, 0.82, 0.86),
                sky: Color::srgb(0.008, 0.008, 0.015),
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
                // hard white sun, crisp shadows; the night is blue-black under Earthlight
                light: SkyDef {
                    sun_lux: 9_000.0,
                    ambient_day: 80.0,
                    ambient_night: 34.0,
                    ambient_day_color: Color::srgb(0.65, 0.7, 0.9),
                    ambient_night_color: Color::srgb(0.42, 0.55, 1.0),
                    axis_yaw: 0.0,
                    declination: 0.2,
                },
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
                ground: Color::srgb(0.72, 0.42, 0.26),
                ground_low: Color::srgb(0.48, 0.24, 0.15),
                ground_high: Color::srgb(0.88, 0.60, 0.40),
                sky: Color::srgb(0.03, 0.012, 0.008),
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
                // a dim amber haze by day, muddy brown by night
                light: SkyDef {
                    sun_lux: 7_000.0,
                    ambient_day: 90.0,
                    ambient_night: 30.0,
                    ambient_day_color: Color::srgb(0.95, 0.72, 0.58),
                    ambient_night_color: Color::srgb(0.55, 0.40, 0.36),
                    axis_yaw: 1.3,
                    declination: 0.32,
                },
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
                sky: Color::srgb(0.010, 0.002, 0.018),
                sun: Color::srgb(0.75, 0.55, 1.0),
                enemy_tint: Color::srgb(0.8, 0.5, 1.0),
                rocks: 120,
                crystals: 85,
                pots: 38,
                flora: 55,
                flora_style: FloraStyle::GlowShrooms,
                has_earthrise: false,
                meteor_showers: false,
                threat: 1.25,
                // almost no key light, and a near-black night: the fungus IS the light
                light: SkyDef {
                    sun_lux: 4_000.0,
                    ambient_day: 60.0,
                    ambient_night: 18.0,
                    ambient_day_color: Color::srgb(0.62, 0.52, 0.9),
                    ambient_night_color: Color::srgb(0.42, 0.3, 0.7),
                    axis_yaw: 2.4,
                    declination: 0.12,
                },
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
