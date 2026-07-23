pub mod characters;
pub mod enemies;
pub mod items;
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
    pub fn color(&self) -> bevy::prelude::Color {
        use bevy::prelude::Color;
        match self {
            Rarity::Common => Color::srgb(0.75, 0.78, 0.80),
            Rarity::Rare => Color::srgb(0.30, 0.65, 1.00),
            Rarity::Epic => Color::srgb(0.75, 0.35, 1.00),
            Rarity::Legendary => Color::srgb(1.00, 0.72, 0.15),
        }
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
