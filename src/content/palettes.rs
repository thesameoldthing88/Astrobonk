//! Color-vision palettes (GDD §13 Accessibility): the colors that CARRY MEANING — danger and
//! rarity — per palette. Everything else keeps its art-directed color; only these two
//! signal families are remapped, because they are the ones a player must read at a glance.
//!
//! Chosen by simulation, not by eye. Each candidate was run through the Machado et al.
//! (2009) full-severity dichromacy matrices and scored by the smallest CIELAB ΔE against
//! what it must stand out from:
//!   * danger — every planet's ground (low/mid/high), XP gems, coins, elites, each world's
//!     enemy tint, silver and powerups. The canon red-orange falls to ΔE 11.9 against the
//!     orange elite for a deuteranope (and 18.5 against Mars rock for a protanope); the
//!     electric blue below holds ΔE 54.7 / 35.5, and a purer red holds 46.4 for tritanopes.
//!   * rarity — the four grades against each other, with every color kept bright enough to
//!     read as text on a dark card and none of them red (red is reserved for danger, §12).
//!     Canon grey/blue/purple/gold collapses to ΔE 9.6 for deuteranopes (purple reads blue);
//!     the remapped sets hold ΔE 44 (deut/prot) and 42 (trit).
//!   * cursed (P03's off-ladder family) — against the four grades AND the danger colors.
//!     Standard's static-magenta collapses to ΔE 3.6 against a grade for a deuteranope, so
//!     red-green dichromats get a sickly olive (min ΔE 40.7 for both deut and prot, 54 from
//!     white name text) and tritanopes a violet (min ΔE 33.8, 54 from white).
//!
//! Every telegraph ALSO reads by shape and motion (pulsing ring, marching dashes, cracking
//! decal) and every rarity by its printed name — color is never the only channel.

use super::Rarity;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Palette {
    #[default]
    Standard,
    Deuteranopia,
    Protanopia,
    Tritanopia,
}

impl Palette {
    pub const ALL: [Palette; 4] = [Palette::Standard, Palette::Deuteranopia, Palette::Protanopia, Palette::Tritanopia];

    pub fn name(&self) -> &'static str {
        match self {
            Palette::Standard => "STANDARD",
            Palette::Deuteranopia => "DEUTERANOPIA",
            Palette::Protanopia => "PROTANOPIA",
            Palette::Tritanopia => "TRITANOPIA",
        }
    }

    /// Telegraph rings, aim lines, railbolts, the verdict beam, the boss marker.
    pub fn danger(&self) -> Color {
        match self {
            Palette::Standard => Color::srgb(1.0, 0.25, 0.10),
            Palette::Deuteranopia | Palette::Protanopia => Color::srgb(0.25, 0.15, 1.0),
            Palette::Tritanopia => Color::srgb(1.0, 0.05, 0.10),
        }
    }

    /// Enemy shots (spitter/UFO needles, boss bursts). Canon magenta keeps them apart from
    /// the telegraph rings; for red-green dichromats that magenta already reads as the
    /// danger blue, so the two are merged into one unmistakable hue instead.
    pub fn danger_shot(&self) -> Color {
        match self {
            Palette::Standard | Palette::Tritanopia => Color::srgb(0.9, 0.3, 0.9),
            Palette::Deuteranopia | Palette::Protanopia => self.danger(),
        }
    }

    /// The verdict beam while it charges: a see-through warning, not yet the hit.
    pub fn danger_charge(&self) -> Color {
        match self {
            Palette::Standard => Color::srgba(1.0, 0.7, 0.2, 0.35),
            _ => self.danger().with_alpha(0.35),
        }
    }

    /// The verdict beam once it fires.
    pub fn beam_fire(&self) -> Color {
        match self {
            Palette::Standard => Color::srgb(1.0, 0.3, 0.15),
            _ => self.danger(),
        }
    }

    pub fn rarity(&self, r: Rarity) -> Color {
        let [common, rare, epic, legendary, cursed] = match self {
            // Cursed (P03's off-ladder family) is static-magenta: it reads "wrong" next to the
            // grey/blue/purple/gold ladder and stays clear of red, which §12 reserves for
            // danger telegraphs.
            Palette::Standard => [
                Color::srgb(0.75, 0.78, 0.80),
                Color::srgb(0.30, 0.65, 1.00),
                Color::srgb(0.75, 0.35, 1.00),
                Color::srgb(1.00, 0.72, 0.15),
                Color::srgb(1.00, 0.30, 0.70),
            ],
            // Purple is the grade red-green dichromats lose (it reads as blue), so Epic
            // becomes pearl: apart from Rare's blue by hue AND from Common's grey by
            // lightness. Magenta reads as a grade too, so Cursed goes sickly olive.
            Palette::Deuteranopia | Palette::Protanopia => [
                Color::srgb(0.60, 0.62, 0.65),
                Color::srgb(0.20, 0.60, 1.00),
                Color::srgb(1.00, 1.00, 0.80),
                Color::srgb(1.00, 0.72, 0.15),
                Color::srgb(0.65, 0.55, 0.30),
            ],
            // Tritanopes merge blue with green and yellow with pink: Rare goes cyan, Epic
            // magenta, and gold stays gold (it reads apart from both). Cursed goes violet,
            // apart from Epic's magenta and the danger red.
            Palette::Tritanopia => [
                Color::srgb(0.60, 0.62, 0.65),
                Color::srgb(0.00, 0.80, 1.00),
                Color::srgb(1.00, 0.00, 0.80),
                Color::srgb(1.00, 0.72, 0.15),
                Color::srgb(0.70, 0.30, 1.00),
            ],
        };
        match r {
            Rarity::Common => common,
            Rarity::Rare => rare,
            Rarity::Epic => epic,
            Rarity::Legendary => legendary,
            Rarity::Cursed => cursed,
        }
    }
}
