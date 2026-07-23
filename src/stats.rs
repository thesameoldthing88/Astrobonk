//! The full run-time stat sheet (mirrors the survivors-genre stat list) plus the
//! boost vocabulary items/tomes/passives use to modify it.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatKind {
    MaxHp,
    Regen, // hp per minute
    Shield,
    Armor,   // % reduction, diminishing
    Evasion, // % dodge, diminishing
    Lifesteal,
    Thorns,
    Damage, // additive to multiplier (0.10 = +10%)
    CritChance,
    CritDamage,
    AttackSpeed,
    Projectiles,
    ProjSpeed,
    Size,
    Duration,
    EliteDamage,
    Knockback,
    MoveSpeed,
    ExtraJumps,
    JumpHeight,
    Luck,
    Difficulty,
    PickupRange,
    XpGain,
    GoldGain,
    SilverGain,
    ChestDiscount,
}

#[derive(Clone, Debug)]
pub struct Stats {
    pub max_hp: f32,
    pub regen: f32,
    pub shield: f32,
    pub armor: f32,
    pub evasion: f32,
    pub lifesteal: f32,
    pub thorns: f32,
    pub damage: f32,      // multiplier, base 1.0
    pub crit_chance: f32, // 0..1+, overcrit past 1
    pub crit_damage: f32, // multiplier, base 2.0
    pub attack_speed: f32,
    pub projectiles: i32, // additive bonus
    pub proj_speed: f32,
    pub size: f32,
    pub duration: f32,
    pub elite_damage: f32,
    pub knockback: f32,
    pub move_speed: f32,
    pub extra_jumps: i32,
    pub jump_height: f32,
    pub luck: f32, // 0.10 = +10% rarity weighting
    pub difficulty: f32,
    pub pickup_range: f32, // multiplier
    pub xp_gain: f32,      // multiplier, capped 10
    pub gold_gain: f32,
    pub silver_gain: f32,
    pub chest_discount: f32, // 0..0.6
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            max_hp: 100.0,
            regen: 0.0,
            shield: 0.0,
            armor: 0.0,
            evasion: 0.0,
            lifesteal: 0.0,
            thorns: 0.0,
            damage: 1.0,
            crit_chance: 0.05,
            crit_damage: 2.0,
            attack_speed: 1.0,
            projectiles: 0,
            proj_speed: 1.0,
            size: 1.0,
            duration: 1.0,
            elite_damage: 1.0,
            knockback: 1.0,
            move_speed: 1.0,
            extra_jumps: 0,
            jump_height: 1.0,
            luck: 0.0,
            difficulty: 0.0,
            pickup_range: 1.0,
            xp_gain: 1.0,
            gold_gain: 1.0,
            silver_gain: 1.0,
            chest_discount: 0.0,
        }
    }
}

impl Stats {
    pub fn apply(&mut self, kind: StatKind, v: f32) {
        use StatKind::*;
        match kind {
            MaxHp => self.max_hp += v,
            Regen => self.regen += v,
            Shield => self.shield += v,
            Armor => self.armor += v,
            Evasion => self.evasion += v,
            Lifesteal => self.lifesteal += v,
            Thorns => self.thorns += v,
            Damage => self.damage += v,
            CritChance => self.crit_chance += v,
            CritDamage => self.crit_damage += v,
            AttackSpeed => self.attack_speed += v,
            Projectiles => self.projectiles += v as i32,
            ProjSpeed => self.proj_speed += v,
            Size => self.size += v,
            Duration => self.duration += v,
            EliteDamage => self.elite_damage += v,
            Knockback => self.knockback += v,
            MoveSpeed => self.move_speed += v,
            ExtraJumps => self.extra_jumps += v as i32,
            JumpHeight => self.jump_height += v,
            Luck => self.luck += v,
            Difficulty => self.difficulty += v,
            PickupRange => self.pickup_range += v,
            XpGain => self.xp_gain = (self.xp_gain + v).min(10.0),
            GoldGain => self.gold_gain += v,
            SilverGain => self.silver_gain += v,
            ChestDiscount => self.chest_discount = (self.chest_discount + v).min(0.6),
        }
    }

    /// Armor/evasion use diminishing curves and can never hit 100%.
    pub fn armor_fraction(&self) -> f32 {
        let a = self.armor.max(0.0);
        a / (a + 100.0)
    }
    pub fn evasion_fraction(&self) -> f32 {
        let e = self.evasion.max(0.0);
        e / (e + 100.0)
    }
}

impl StatKind {
    pub fn label(&self, v: f32) -> String {
        use StatKind::*;
        match self {
            MaxHp => format!("+{v:.0} Max HP"),
            Regen => format!("+{v:.0} HP/min"),
            Shield => format!("+{v:.0} Shield"),
            Armor => format!("+{v:.0} Armor"),
            Evasion => format!("+{v:.0} Evasion"),
            Lifesteal => format!("+{:.0}% Lifesteal", v * 100.0),
            Thorns => format!("+{v:.0} Thorns"),
            Damage => format!("+{:.0}% Damage", v * 100.0),
            CritChance => format!("+{:.0}% Crit Chance", v * 100.0),
            CritDamage => format!("+{:.1}x Crit Damage", v),
            AttackSpeed => format!("+{:.0}% Attack Speed", v * 100.0),
            Projectiles => format!("+{v:.0} Projectile"),
            ProjSpeed => format!("+{:.0}% Projectile Speed", v * 100.0),
            Size => format!("+{:.0}% Size", v * 100.0),
            Duration => format!("+{:.0}% Duration", v * 100.0),
            EliteDamage => format!("+{:.0}% Damage to Elites", v * 100.0),
            Knockback => format!("+{:.0}% Knockback", v * 100.0),
            MoveSpeed => format!("+{:.0}% Move Speed", v * 100.0),
            ExtraJumps => format!("+{v:.0} Jump"),
            JumpHeight => format!("+{:.0}% Jump Height", v * 100.0),
            Luck => format!("+{:.0}% Luck", v * 100.0),
            Difficulty => format!("+{:.0}% Difficulty", v * 100.0),
            PickupRange => format!("+{:.0}% Pickup Range", v * 100.0),
            XpGain => format!("+{:.0}% XP Gain", v * 100.0),
            GoldGain => format!("+{:.0}% Gold Gain", v * 100.0),
            SilverGain => format!("+{:.0}% Silver Gain", v * 100.0),
            ChestDiscount => format!("-{:.0}% Chest Cost", v * 100.0),
        }
    }
}
