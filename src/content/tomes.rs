use crate::stats::StatKind;
use serde::{Deserialize, Serialize};

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
}

pub struct TomeDef {
    pub kind: TomeKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub stat: StatKind,
    pub per_level: f32,
    pub max_level: u32,
}

impl TomeKind {
    pub const ALL: [TomeKind; 8] = [
        TomeKind::Damage,
        TomeKind::Health,
        TomeKind::Agility,
        TomeKind::Cooldown,
        TomeKind::Precision,
        TomeKind::Golden,
        TomeKind::Xp,
        TomeKind::Cursed,
    ];

    pub fn def(&self) -> TomeDef {
        use StatKind as S;
        use TomeKind::*;
        match self {
            Damage => TomeDef { kind: *self, name: "Tome of Damage", desc: "Hit harder, forever", stat: S::Damage, per_level: 0.02, max_level: 20 },
            Health => TomeDef { kind: *self, name: "Tome of Health", desc: "Thicker suit lining", stat: S::MaxHp, per_level: 6.0, max_level: 20 },
            Agility => TomeDef { kind: *self, name: "Tome of Agility", desc: "Lower gravity legs", stat: S::MoveSpeed, per_level: 0.012, max_level: 20 },
            Cooldown => TomeDef { kind: *self, name: "Tome of Cooldown", desc: "Weapons on espresso", stat: S::AttackSpeed, per_level: 0.012, max_level: 20 },
            Precision => TomeDef { kind: *self, name: "Tome of Precision", desc: "Aim like you mean it", stat: S::CritChance, per_level: 0.006, max_level: 20 },
            Golden => TomeDef { kind: *self, name: "Golden Tome", desc: "Coins find you cuter", stat: S::GoldGain, per_level: 0.02, max_level: 20 },
            Xp => TomeDef { kind: *self, name: "Tome of XP", desc: "Learn from the bonk", stat: S::XpGain, per_level: 0.015, max_level: 20 },
            Cursed => TomeDef { kind: *self, name: "Cursed Tome", desc: "More danger. More everything", stat: S::Difficulty, per_level: 0.02, max_level: 20 },
        }
    }

    /// Silver cost to buy the next level (current -> current+1).
    pub fn cost(&self, current: u32) -> u64 {
        let l = current as f32 + 1.0;
        (8.0 * l * l.sqrt()).round() as u64
    }
}
