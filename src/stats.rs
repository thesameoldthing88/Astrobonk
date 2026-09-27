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
    // appended
    /// Incoming damage multiplier (+1.0 = double damage taken) — Cracked Helmet.
    DamageTaken,
    /// Max HP as a fraction, applied after every flat bonus (−0.2 = −20%) — Widow's Ring.
    MaxHpMult,
    // ---- the §7 tomes' lines (P05); what each one DOES is mapped in `tomes.rs` ----
    /// Size and speed of orbiting and returning weapons (+0.04 = +4%) — Tome of Orbit.
    Orbit,
    /// Damage per foe within TOME_CROWD_RADIUS (up to TOME_CROWD_CAP) — Tome of Encirclement.
    CrowdDamage,
    /// Damage while standing on the night side — Tome of Nightfall.
    NightDamage,
    /// Flashlight reach and brightness — Tome of Nightfall.
    Flashlight,
    /// Gravity while falling (+0.06 = lands 6% harder) — Tome of Gravity.
    FallSpeed,
    /// Frequency of every "every X seconds" item — Tome of the Swarm.
    ProcRate,
    /// Chance a shot skips off the ground once when it comes down — Tome of Ricochet.
    Ricochet,
    /// Damage at full momentum (MOMENTUM_RAMP_SECS of unbroken movement) — Tome of Momentum.
    MomentumDamage,
    /// Extra share of Vampire Visor's own lifesteal — Tome of Vampirism.
    VisorBoost,
    /// Loot from elite kills — Tome of the Elite (party-wide, like Difficulty).
    EliteLoot,
    /// Multiplier on hits from elites — Tome of the Elite's price.
    EliteDamageTaken,
    /// Banish charges / free Refreshes per run — Tome of Banishment.
    ExtraBanishes,
    ExtraRefreshes,
    /// Off the Gold price of paid Refreshes — Tome of Banishment.
    RefreshDiscount,
    /// Free Microwave uses per stage — Tome of Duplication.
    FreeMicrowave,
    /// Chance a Microwave duplicate keeps its full grade — Tome of Duplication.
    DupeKeepGrade,
    /// Rate (1/s) at which XP lying over the horizon flies home — Tome of the Horizon.
    HorizonCollect,
    /// Silver The Static pays (overtime and its ghosts' drops) — Tome of Static.
    StaticSilver,
    /// Multiplier on hits from The Static — Tome of Static's price.
    StaticDamageTaken,
    /// Evolution slots on top of `config::EVOLUTION_CAP` — Tome of Ascension.
    EvoSlots,
    /// Damage of evolved weapons — Tome of Ascension.
    EvoDamage,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// Multiplier on every hit taken, base 1.0.
    pub damage_taken: f32,
    /// Multiplier on max HP after every flat bonus, base 1.0 (`PlayerState::recompute_stats`).
    pub max_hp_mult: f32,
    // ---- tome lines (see StatKind) ----
    pub orbit: f32, // multiplier, base 1.0
    pub crowd_damage: f32,
    pub night_damage: f32,
    pub flashlight: f32, // multiplier, base 1.0
    pub fall_speed: f32, // gravity multiplier while falling, base 1.0
    pub proc_rate: f32,  // multiplier, base 1.0
    pub ricochet: f32,   // 0..1
    pub momentum_damage: f32,
    pub visor_boost: f32,
    pub elite_loot: f32,         // multiplier, base 1.0
    pub elite_damage_taken: f32, // multiplier, base 1.0
    pub extra_banishes: i32,
    pub extra_refreshes: i32,
    pub refresh_discount: f32, // 0..0.9
    pub free_microwave: i32,
    pub dupe_keep_grade: f32, // 0..1
    pub horizon_collect: f32, // 1/s; 0 = off
    pub static_silver: f32,      // multiplier, base 1.0
    pub static_damage_taken: f32, // multiplier, base 1.0
    pub evo_slots: i32,
    pub evo_damage: f32, // multiplier, base 1.0
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
            damage_taken: 1.0,
            max_hp_mult: 1.0,
            orbit: 1.0,
            crowd_damage: 0.0,
            night_damage: 0.0,
            flashlight: 1.0,
            fall_speed: 1.0,
            proc_rate: 1.0,
            ricochet: 0.0,
            momentum_damage: 0.0,
            visor_boost: 0.0,
            elite_loot: 1.0,
            elite_damage_taken: 1.0,
            extra_banishes: 0,
            extra_refreshes: 0,
            refresh_discount: 0.0,
            free_microwave: 0,
            dupe_keep_grade: 0.0,
            horizon_collect: 0.0,
            static_silver: 1.0,
            static_damage_taken: 1.0,
            evo_slots: 0,
            evo_damage: 1.0,
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
            DamageTaken => self.damage_taken += v,
            MaxHpMult => self.max_hp_mult += v,
            Orbit => self.orbit += v,
            CrowdDamage => self.crowd_damage += v,
            NightDamage => self.night_damage += v,
            Flashlight => self.flashlight += v,
            FallSpeed => self.fall_speed += v,
            ProcRate => self.proc_rate += v,
            Ricochet => self.ricochet = (self.ricochet + v).min(1.0),
            MomentumDamage => self.momentum_damage += v,
            VisorBoost => self.visor_boost += v,
            EliteLoot => self.elite_loot += v,
            EliteDamageTaken => self.elite_damage_taken += v,
            ExtraBanishes => self.extra_banishes += v as i32,
            ExtraRefreshes => self.extra_refreshes += v as i32,
            RefreshDiscount => self.refresh_discount = (self.refresh_discount + v).min(0.9),
            FreeMicrowave => self.free_microwave += v as i32,
            DupeKeepGrade => self.dupe_keep_grade = (self.dupe_keep_grade + v).min(1.0),
            HorizonCollect => self.horizon_collect += v,
            StaticSilver => self.static_silver += v,
            StaticDamageTaken => self.static_damage_taken += v,
            EvoSlots => self.evo_slots += v as i32,
            EvoDamage => self.evo_damage += v,
        }
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
            Lifesteal => format!("+{}% Lifesteal", pct(v)),
            Thorns => format!("+{v:.0} Thorns"),
            Damage => format!("+{}% Damage", pct(v)),
            CritChance => format!("+{}% Crit Chance", pct(v)),
            CritDamage => format!("+{:.1}x Crit Damage", v),
            AttackSpeed => format!("+{}% Attack Speed", pct(v)),
            Projectiles => format!("+{v:.0} Projectile"),
            ProjSpeed => format!("+{}% Projectile Speed", pct(v)),
            Size => format!("+{}% Size", pct(v)),
            Duration => format!("+{}% Duration", pct(v)),
            EliteDamage => format!("+{}% Damage to Elites", pct(v)),
            Knockback => format!("+{}% Knockback", pct(v)),
            MoveSpeed => format!("+{}% Move Speed", pct(v)),
            ExtraJumps => format!("+{v:.0} Jump"),
            JumpHeight => format!("+{}% Jump Height", pct(v)),
            Luck => format!("+{}% Luck", pct(v)),
            Difficulty => format!("+{}% Difficulty", pct(v)),
            PickupRange => format!("+{}% Pickup Range", pct(v)),
            XpGain => format!("+{}% XP Gain", pct(v)),
            GoldGain => format!("+{}% Gold Gain", pct(v)),
            SilverGain => format!("+{}% Silver Gain", pct(v)),
            ChestDiscount => format!("-{}% Chest Cost", pct(v)),
            DamageTaken => format!("+{}% Damage Taken", pct(v)),
            MaxHpMult => format!("{:+.0}% Max HP", v * 100.0),
            Orbit => format!("+{}% Orbit & Return Size/Speed", pct(v)),
            CrowdDamage => format!(
                "+{}% Damage per foe within {:.0}m (max {})",
                pct(v),
                crate::config::TOME_CROWD_RADIUS,
                crate::config::TOME_CROWD_CAP
            ),
            NightDamage => format!("+{}% Damage on the night side", pct(v)),
            Flashlight => format!("+{}% Flashlight", pct(v)),
            FallSpeed => format!("+{}% Fall Speed", pct(v)),
            ProcRate => format!("+{}% Proc Frequency", pct(v)),
            Ricochet => format!("{}% of shots skip off the ground", pct(v)),
            MomentumDamage => {
                format!("Up to +{}% Damage after {:.0}s on the move", pct(v), crate::config::MOMENTUM_RAMP_SECS)
            }
            VisorBoost => format!("+{}% Vampire Visor", pct(v)),
            EliteLoot => format!("+{}% Elite Loot", pct(v)),
            EliteDamageTaken => format!("+{}% Damage from Elites", pct(v)),
            ExtraBanishes => format!("+{v:.0} Banish per run"),
            ExtraRefreshes => format!("+{v:.0} free Refresh per run"),
            RefreshDiscount => format!("-{}% paid Refresh price", pct(v)),
            FreeMicrowave => format!("+{v:.0} free Microwave use per stage"),
            DupeKeepGrade => format!("{}% of duplicates keep their grade", pct(v)),
            HorizonCollect => format!("XP over the horizon flies home after {:.0}s", 1.0 / v.max(1e-3)),
            StaticSilver => format!("+{}% Silver from The Static", pct(v)),
            StaticDamageTaken => format!("+{}% Damage from The Static", pct(v)),
            EvoSlots => format!("+{v:.0} Evolution slot"),
            EvoDamage => format!("+{}% Evolved weapon Damage", pct(v)),
        }
    }
}

/// A fraction as a percentage for card text: whole when it is whole ("+4%"), one decimal
/// when it is not ("+2.4%", "+0.4%") — tome ranks move some lines by fractions of a percent.
fn pct(v: f32) -> String {
    let p = v * 100.0;
    if (p - p.round()).abs() < 0.05 {
        format!("{:.0}", p)
    } else {
        format!("{:.1}", p)
    }
}
