pub mod characters;
pub mod duos;
pub mod enemies;
pub mod items;
pub mod palettes;
pub mod planets;
pub mod quests;
pub mod tomes;
pub mod weapons;

use serde::{Deserialize, Serialize};

/// Rarity Grade (GDD §7). Common → Legendary is the LADDER an item's grade is rolled on:
/// every item has a native grade (the lowest it drops at) and Luck rolls it higher, where the
/// same effect comes out bigger. Cursed is not a rung: cursed items are their own family,
/// always Cursed, never rolled up or down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
    // appended: never reorder (the ladder order above is what `Ord` and the grade steps use)
    Cursed,
}

impl Rarity {
    /// The rolled rungs, in order. Cursed is deliberately absent.
    pub const LADDER: [Rarity; 4] = [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary];

    /// The grade's signal color in the viewer's palette (colorblind palettes remap it,
    /// Cursed included — see `palettes::Palette::rarity`).
    pub fn color(&self, palette: palettes::Palette) -> bevy::prelude::Color {
        palette.rarity(*self)
    }
    pub fn name(&self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Cursed => "Cursed",
        }
    }
    /// Position on the ladder (Common 0 … Legendary 3). Cursed sits off it.
    pub fn rung(&self) -> Option<usize> {
        Self::LADDER.iter().position(|r| r == self)
    }
    /// `steps` rungs up the ladder, stopping at Legendary. Cursed stays Cursed.
    pub fn step_up(&self, steps: u32) -> Rarity {
        match self.rung() {
            Some(i) => Self::LADDER[(i + steps as usize).min(Self::LADDER.len() - 1)],
            None => *self,
        }
    }
    /// `steps` rungs down, stopping at `floor` (an item never drops below its native grade).
    pub fn step_down(&self, steps: u32, floor: Rarity) -> Rarity {
        match (self.rung(), floor.rung()) {
            (Some(i), Some(f)) => Self::LADDER[i.saturating_sub(steps as usize).max(f)],
            _ => *self,
        }
    }
    /// Explicit wire code (co-op build sync). Never renumber, only append.
    pub fn code(&self) -> u8 {
        match self {
            Rarity::Common => 0,
            Rarity::Rare => 1,
            Rarity::Epic => 2,
            Rarity::Legendary => 3,
            Rarity::Cursed => 4,
        }
    }
    pub fn from_code(c: u8) -> Rarity {
        match c {
            1 => Rarity::Rare,
            2 => Rarity::Epic,
            3 => Rarity::Legendary,
            4 => Rarity::Cursed,
            _ => Rarity::Common,
        }
    }
    /// Base roll weights, shifted by luck: positive luck bleeds weight upward.
    pub fn weights(luck: f32) -> [f32; 4] {
        let l = luck.clamp(0.0, 3.0);
        [
            (62.0 - 30.0 * l).max(8.0),
            26.0 + 8.0 * l,
            9.0 + 14.0 * l,
            3.0 + 8.0 * l,
        ]
    }
    /// Roll a rung of the ladder (never Cursed). Always exactly one draw, so a seeded stream
    /// stays aligned whatever the luck.
    pub fn roll(luck: f32, rng: &mut impl rand::Rng) -> Rarity {
        let w = Self::weights(luck);
        let total: f32 = w.iter().sum();
        let mut x = rng.gen_range(0.0..total);
        for (i, wi) in w.iter().enumerate() {
            if x < *wi {
                return Self::LADDER[i];
            }
            x -= wi;
        }
        Rarity::Common
    }
}
