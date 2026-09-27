//! Tomes (GDD §7): pre-run passives bought with Silver and slotted before the drop. The 8
//! built tomes plus the 15 new ones — 23 in all, each `TOME_MAX_RANK` ranks.
//!
//! A tome is a list of stat lines in the ordinary `Stats` vocabulary, so it folds into the
//! sheet in `PlayerState::recompute_stats` like any item boost, and in co-op it reaches the
//! host inside the peer's derived `Stats` (`net::PlayerBuildMsg`) with no tome-specific wire.
//! What the lines that are not plain stats DO lives at their effect sites — `tomes.rs` has
//! the map.

use crate::config::{TOME_COST_BASE, TOME_COST_GROWTH};
use crate::stats::StatKind;
use serde::{Deserialize, Serialize};

/// Saved by variant NAME (the save's `tome_levels` map and loadout): only ever append,
/// never rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum TomeKind {
    Damage,
    Health,
    Agility,
    Cooldown,
    Precision,
    Golden,
    Xp,
    Cursed,
    // ---- §7 "add 15 new" ----
    Orbit,
    Encirclement,
    Nightfall,
    Gravity,
    Swarm,
    Salvage,
    Ricochet,
    Momentum,
    Vampirism,
    Elite,
    Banishment,
    Duplication,
    Horizon,
    Static,
    Ascension,
}

/// One line of what a tome does: `first` of `stat` at rank 1, plus `per_rank` for every
/// rank above it.
#[derive(Clone, Copy, Debug)]
pub struct TomeEffect {
    pub stat: StatKind,
    pub first: f32,
    pub per_rank: f32,
}

impl TomeEffect {
    /// The same step every rank (rank 10 = 10×).
    const fn each(stat: StatKind, v: f32) -> Self {
        Self { stat, first: v, per_rank: v }
    }
    /// A headline granted whole at rank 1 (a Banish charge, an evolution slot) that later
    /// ranks leave alone — a tome's one promise does not wait for 18,000 Silver.
    const fn unlock(stat: StatKind, v: f32) -> Self {
        Self { stat, first: v, per_rank: 0.0 }
    }
    /// Nothing at rank 1, then `v` for every rank above it: the companion line a discrete
    /// tome's later ranks grow, so every rank of every tome buys something.
    const fn after_first(stat: StatKind, v: f32) -> Self {
        Self { stat, first: 0.0, per_rank: v }
    }

    /// The line's value at `rank` (0 = the tome is not owned).
    pub fn at(&self, rank: u32) -> f32 {
        if rank == 0 {
            0.0
        } else {
            self.first + self.per_rank * (rank - 1) as f32
        }
    }
}

pub struct TomeDef {
    pub kind: TomeKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub effects: &'static [TomeEffect],
}

impl TomeKind {
    pub const ALL: [TomeKind; 23] = [
        TomeKind::Damage,
        TomeKind::Health,
        TomeKind::Agility,
        TomeKind::Cooldown,
        TomeKind::Precision,
        TomeKind::Golden,
        TomeKind::Xp,
        TomeKind::Cursed,
        TomeKind::Orbit,
        TomeKind::Encirclement,
        TomeKind::Nightfall,
        TomeKind::Gravity,
        TomeKind::Swarm,
        TomeKind::Salvage,
        TomeKind::Ricochet,
        TomeKind::Momentum,
        TomeKind::Vampirism,
        TomeKind::Elite,
        TomeKind::Banishment,
        TomeKind::Duplication,
        TomeKind::Horizon,
        TomeKind::Static,
        TomeKind::Ascension,
    ];

    pub fn def(&self) -> TomeDef {
        use StatKind as S;
        use TomeEffect as E;
        use TomeKind::*;
        let d = |name, desc, effects| TomeDef { kind: *self, name, desc, effects };
        // `&const { … }`: a const fn call is not promoted to 'static on its own, so each
        // table row is evaluated at compile time explicitly.
        match self {
            // The built eight keep their stat and their maximum: with 20 levels folded into
            // 10 ranks, one rank is worth two old levels (`MetaSave::migrate` converts).
            Damage => d("Tome of Damage", "Hit harder, forever", &const { [E::each(S::Damage, 0.04)] }),
            Health => d("Tome of Health", "Thicker suit lining", &const { [E::each(S::MaxHp, 12.0)] }),
            Agility => d("Tome of Agility", "Lower gravity legs", &const { [E::each(S::MoveSpeed, 0.024)] }),
            Cooldown => d("Tome of Cooldown", "Weapons on espresso", &const { [E::each(S::AttackSpeed, 0.024)] }),
            Precision => d("Tome of Precision", "Aim like you mean it", &const { [E::each(S::CritChance, 0.012)] }),
            // …and its rank multiplies the run's Silver (§10: `director::silver_payout`)
            Golden => d("Golden Tome", "Coins find you cuter", &const { [E::each(S::GoldGain, 0.04)] }),
            Xp => d("Tome of XP", "Learn from the bonk", &const { [E::each(S::XpGain, 0.03)] }),
            Cursed => d("Cursed Tome", "More danger. More everything", &const { [E::each(S::Difficulty, 0.04)] }),

            Orbit => d("Tome of Orbit", "Longer strings, faster laps", &const { [E::each(S::Orbit, 0.04)] }),
            Encirclement => d("Tome of Encirclement", "Surrounded? Good.", &const { [E::each(S::CrowdDamage, 0.001)] }),
            Nightfall => d(
                "Tome of Nightfall",
                "The dark side has better lighting",
                &const { [E::each(S::NightDamage, 0.03), E::each(S::Flashlight, 0.1)] },
            ),
            Gravity => d(
                "Tome of Gravity",
                "Fall with purpose",
                &const { [E::each(S::FallSpeed, 0.06), E::each(S::JumpHeight, 0.03)] },
            ),
            Swarm => d("Tome of the Swarm", "Every X seconds, but sooner", &const { [E::each(S::ProcRate, 0.04)] }),
            Salvage => d("Tome of Salvage", "Haggle with the void", &const { [E::each(S::ChestDiscount, 0.03)] }),
            Ricochet => d("Tome of Ricochet", "Shots skip like stones on regolith", &const { [E::each(S::Ricochet, 0.1)] }),
            Momentum => d("Tome of Momentum", "A body in motion stays lethal", &const { [E::each(S::MomentumDamage, 0.03)] }),
            Vampirism => d(
                "Tome of Vampirism",
                "Sips, not bites",
                &const { [E::each(S::Lifesteal, 0.004), E::each(S::VisorBoost, 0.1)] },
            ),
            Elite => d(
                "Tome of the Elite",
                "Big game hunting. Big game hunts back",
                &const { [E::each(S::EliteLoot, 0.15), E::each(S::EliteDamageTaken, 0.04)] },
            ),
            Banishment => d(
                "Tome of Banishment",
                "Strike it from the record",
                &const { [
                    E::unlock(S::ExtraBanishes, 1.0),
                    E::unlock(S::ExtraRefreshes, 1.0),
                    E::after_first(S::RefreshDiscount, 0.05),
                ] },
            ),
            Duplication => d(
                "Tome of Duplication",
                "The microwave owes you one",
                &const { [E::unlock(S::FreeMicrowave, 1.0), E::after_first(S::DupeKeepGrade, 0.06)] },
            ),
            Horizon => d("Tome of the Horizon", "The planet mails you your XP", &const { [E::each(S::HorizonCollect, 0.05)] }),
            Static => d(
                "Tome of Static",
                "It pays in Silver. It bites in kind",
                &const { [E::each(S::StaticSilver, 0.1), E::each(S::StaticDamageTaken, 0.05)] },
            ),
            Ascension => d(
                "Tome of Ascension",
                "Two evolutions. One astronaut",
                &const { [E::unlock(S::EvoSlots, 1.0), E::after_first(S::EvoDamage, 0.03)] },
            ),
        }
    }

    /// Silver to buy the next rank (`current` → `current + 1`): §7's `100 × 1.6^level`.
    pub fn cost(&self, current: u32) -> u64 {
        (TOME_COST_BASE * TOME_COST_GROWTH.powi(current as i32)).round() as u64
    }

    /// The tome's lines at `rank` as card text ("+12% Damage"); empty at rank 0.
    pub fn lines(&self, rank: u32) -> Vec<String> {
        self.def()
            .effects
            .iter()
            .filter(|e| e.at(rank) != 0.0)
            .map(|e| e.stat.label(e.at(rank)))
            .collect()
    }

    /// From a command-line name (`--tomes orbit,horizon`): the variant name, any case.
    pub fn from_name(s: &str) -> Option<TomeKind> {
        Self::ALL.iter().copied().find(|t| format!("{t:?}").eq_ignore_ascii_case(s))
    }
}
