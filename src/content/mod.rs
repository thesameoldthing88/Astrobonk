pub mod characters;
pub mod enemies;
pub mod items;
pub mod palettes;
pub mod planets;
pub mod quests;
pub mod tomes;
pub mod weapons;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    /// The grade's signal color in the viewer's palette (colorblind palettes remap it).
    pub fn color(&self, palette: palettes::Palette) -> bevy::prelude::Color {
        palette.rarity(*self)
    }
    pub fn name(&self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
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
    pub fn roll(luck: f32, rng: &mut impl rand::Rng) -> Rarity {
        let w = Self::weights(luck);
        let total: f32 = w.iter().sum();
        let mut x = rng.gen_range(0.0..total);
        for (i, wi) in w.iter().enumerate() {
            if x < *wi {
                return [Rarity::Common, Rarity::Rare, Rarity::Epic, Rarity::Legendary][i];
            }
            x -= wi;
        }
        Rarity::Common
    }
}
