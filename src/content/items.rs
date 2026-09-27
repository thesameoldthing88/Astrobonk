//! The item table (GDD §7). Items never level: power is fixed at pickup, so the levers are
//! the Rarity Grade an item is rolled at (a bigger roll of the SAME effect, see
//! `ItemKind::grade_mult`) and stacking. Stat items are pure `boosts`; the items that *do
//! things* carry their numbers in config.rs and their behaviour in `crate::items`.

use super::Rarity;
use crate::config::*;
use crate::stats::StatKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    SpaceBorgar,
    MoonCheese,
    DuctTape,
    SlipperyVisor,
    ProteinPaste,
    OverclockedCpu,
    FishBowlHelmet,
    RocketBoots,
    TrampolineSoles,
    MagnetBoots,
    LuckyMeteorite,
    SpaceCreditCard,
    GoldenAntenna,
    StarChart,
    LaserSight,
    HeavyPayload,
    ExtraBattery,
    SplitterChip,
    CursedMoonRock,
    ThornPlating,
    VampireVisor,
    CaffeineIv,
    // ---- §7 "the ones that do things": proc & conditional ----
    OrbitalYoYo,
    CometTail,
    TheOverheat,
    DownhillMomentum,
    SecondAstronaut,
    EncirclementBonus,
    IcarusBoots,
    AntiGravBoots,
    LittleBlackHole,
    DeadMansTether,
    // ---- §7 cursed / high-risk ----
    CrackedHelmet,
    StaticRadio,
    WidowsRing,
    SignalFlare,
    DevouredSunShard,
    // ---- the §15 antipode escape, as an item (it blinks through `techs`) ----
    BoomerangInsurance,
    // ---- §4's signature verb, as the §7 item that grants it ----
    AntipodeBlink,
}

pub struct ItemDef {
    pub kind: ItemKind,
    pub name: &'static str,
    /// Three-letter HUD chip label — two-letter initials collide once there are 38 items.
    pub tag: &'static str,
    pub desc: &'static str,
    /// The NATIVE grade: the lowest it drops at. Luck rolls it higher (`grade_mult`).
    pub rarity: Rarity,
    /// Stat boosts per copy at native grade; scaled by the copy's grade multiplier.
    pub boosts: &'static [(StatKind, f32)],
    /// §7: Legendaries and marked items are capped (usually 1–2); everything else stacks
    /// "near-infinitely" (`ITEM_STACKS_UNCAPPED`).
    pub max_stacks: u32,
    /// In the level-up / chest / vendor / shrine pools. Only false for an item whose
    /// mechanic belongs to a later package — dealing a card that does nothing is worse than
    /// not dealing it.
    pub pooled: bool,
}

impl ItemKind {
    pub const ALL: [ItemKind; 39] = [
        ItemKind::SpaceBorgar,
        ItemKind::MoonCheese,
        ItemKind::DuctTape,
        ItemKind::SlipperyVisor,
        ItemKind::ProteinPaste,
        ItemKind::OverclockedCpu,
        ItemKind::FishBowlHelmet,
        ItemKind::RocketBoots,
        ItemKind::TrampolineSoles,
        ItemKind::MagnetBoots,
        ItemKind::LuckyMeteorite,
        ItemKind::SpaceCreditCard,
        ItemKind::GoldenAntenna,
        ItemKind::StarChart,
        ItemKind::LaserSight,
        ItemKind::HeavyPayload,
        ItemKind::ExtraBattery,
        ItemKind::SplitterChip,
        ItemKind::CursedMoonRock,
        ItemKind::ThornPlating,
        ItemKind::VampireVisor,
        ItemKind::CaffeineIv,
        ItemKind::OrbitalYoYo,
        ItemKind::CometTail,
        ItemKind::TheOverheat,
        ItemKind::DownhillMomentum,
        ItemKind::SecondAstronaut,
        ItemKind::EncirclementBonus,
        ItemKind::IcarusBoots,
        ItemKind::AntiGravBoots,
        ItemKind::LittleBlackHole,
        ItemKind::DeadMansTether,
        ItemKind::CrackedHelmet,
        ItemKind::StaticRadio,
        ItemKind::WidowsRing,
        ItemKind::SignalFlare,
        ItemKind::DevouredSunShard,
        ItemKind::BoomerangInsurance,
        ItemKind::AntipodeBlink,
    ];

    /// The §7 "new items" this build adds and deals, plus the §15 Boomerang Insurance —
    /// the `--items new` harness set.
    pub const NEW: [ItemKind; 17] = [
        ItemKind::OrbitalYoYo,
        ItemKind::CometTail,
        ItemKind::TheOverheat,
        ItemKind::DownhillMomentum,
        ItemKind::SecondAstronaut,
        ItemKind::EncirclementBonus,
        ItemKind::IcarusBoots,
        ItemKind::AntiGravBoots,
        ItemKind::LittleBlackHole,
        ItemKind::DeadMansTether,
        ItemKind::CrackedHelmet,
        ItemKind::StaticRadio,
        ItemKind::WidowsRing,
        ItemKind::SignalFlare,
        ItemKind::DevouredSunShard,
        ItemKind::BoomerangInsurance,
        ItemKind::AntipodeBlink,
    ];

    /// Every item a run can be dealt.
    pub fn pool() -> impl Iterator<Item = ItemKind> {
        Self::ALL.into_iter().filter(|i| i.def().pooled)
    }

    pub fn def(&self) -> ItemDef {
        use ItemKind::*;
        use Rarity::*;
        use StatKind as S;
        const UNCAPPED: u32 = ITEM_STACKS_UNCAPPED;
        let d = |name: &'static str,
                 tag: &'static str,
                 desc: &'static str,
                 rarity: Rarity,
                 boosts: &'static [(StatKind, f32)],
                 max_stacks: u32| ItemDef {
            kind: *self,
            name,
            tag,
            desc,
            rarity,
            boosts,
            max_stacks,
            pooled: true,
        };
        match self {
            SpaceBorgar => d("Space Borgar", "BRG", "Zero-g grease. Sticks to your ribs", Common, &[(S::MaxHp, 20.0)], UNCAPPED),
            MoonCheese => d("Moon Cheese", "CHZ", "Aged 4.5 billion years", Common, &[(S::Regen, 12.0)], UNCAPPED),
            DuctTape => d("Duct Tape", "TAP", "Suit patch, armor plating, life philosophy", Common, &[(S::Armor, 8.0)], UNCAPPED),
            SlipperyVisor => d("Slippery Visor", "VSR", "They literally cannot hit you", Rare, &[(S::Evasion, 8.0)], UNCAPPED),
            ProteinPaste => d("Protein Paste", "PRO", "Tube gains", Common, &[(S::Damage, 0.10)], UNCAPPED),
            OverclockedCpu => d("Overclocked CPU", "CPU", "Suit firmware set to UNSAFE", Rare, &[(S::AttackSpeed, 0.12)], UNCAPPED),
            FishBowlHelmet => d("Fish Bowl Helmet", "FSH", "Bigger bubble, bigger booms", Rare, &[(S::Size, 0.12)], UNCAPPED),
            RocketBoots => d("Rocket Boots", "RKT", "Walking is for Earth", Common, &[(S::MoveSpeed, 0.08)], UNCAPPED),
            TrampolineSoles => d("Trampoline Soles", "TRM", "Boing certified", Common, &[(S::JumpHeight, 0.15)], UNCAPPED),
            MagnetBoots => d("Magnet Boots", "MAG", "Loot learns to love you", Common, &[(S::PickupRange, 0.25)], UNCAPPED),
            LuckyMeteorite => d("Lucky Meteorite", "LCK", "Statistically improbable rock", Epic, &[(S::Luck, 0.12)], UNCAPPED),
            // Marked: chest discount saturates at 60% (Stats::apply), so a 7th card is dead.
            SpaceCreditCard => d("Space Credit Card", "CRD", "Interplanetary cashback", Rare, &[(S::ChestDiscount, 0.10)], 6),
            GoldenAntenna => d("Golden Antenna", "ANT", "Tuned to the money frequency", Rare, &[(S::GoldGain, 0.15)], UNCAPPED),
            StarChart => d("Star Chart", "STR", "Knowledge is XP", Rare, &[(S::XpGain, 0.10)], UNCAPPED),
            LaserSight => d("Laser Sight", "LSR", "Red dot of destiny", Rare, &[(S::CritChance, 0.07)], UNCAPPED),
            HeavyPayload => d("Heavy Payload", "PAY", "Crits with extra gravity", Epic, &[(S::CritDamage, 0.5)], UNCAPPED),
            ExtraBattery => d("Extra Battery", "BAT", "Everything lasts longer", Rare, &[(S::Duration, 0.15)], UNCAPPED),
            SplitterChip => d("Splitter Chip", "SPL", "One shot becomes friends", Legendary, &[(S::Projectiles, 1.0)], 2),
            // Marked: each rock also multiplies the §10 Silver payout, so it stays capped.
            CursedMoonRock => d("Cursed Moon Rock", "ROK", "Do NOT lick. More danger, more loot", Epic, &[(S::Difficulty, 0.15), (S::Luck, 0.10)], 5),
            ThornPlating => d("Thorn Plating", "THN", "Hug at your own risk", Rare, &[(S::Thorns, 12.0)], UNCAPPED),
            VampireVisor => d("Vampire Visor", "VMP", "Sunproof. Ironically", Epic, &[(S::Lifesteal, 0.05)], UNCAPPED),
            CaffeineIv => d("Caffeine IV", "CAF", "Projectiles share your jitters", Common, &[(S::ProjSpeed, 0.15)], UNCAPPED),

            OrbitalYoYo => d("Orbital Yo-Yo", "YOY", "A chunk of debris on a very long string", Rare, &[], UNCAPPED),
            CometTail => d("Comet Tail", "TAL", "Kite to win. Stop to light it", Epic, &[], UNCAPPED),
            TheOverheat => d("The Overheat", "HOT", "Punishes greed, loves crit", Epic, &[(S::AttackSpeed, 0.40)], UNCAPPED),
            DownhillMomentum => d("Downhill Momentum", "DWN", "Run down craters to nuke", Epic, &[], UNCAPPED),
            SecondAstronaut => d("Second Astronaut", "2ND", "A ghost co-pilot. Literal co-op flavor", Epic, &[], UNCAPPED),
            EncirclementBonus => d("Encirclement Bonus", "ENC", "The horde surrounds you. Good", Epic, &[], UNCAPPED),
            IcarusBoots => d("Icarus Boots", "ICR", "Never touch the ground again", Epic, &[], UNCAPPED),
            AntiGravBoots => d("Anti-Grav Boots", "AGB", "Hold jump to hover. Fire everywhere", Legendary, &[], 1),
            LittleBlackHole => d("Little Black Hole", "LBH", "It's already a singularity", Legendary, &[], 1),
            DeadMansTether => d("Dead Man's Tether", "DMT", "Death, but with an undo button", Legendary, &[], 1),

            CrackedHelmet => d("Cracked Helmet", "CRK", "Vacuum is a state of mind", Cursed, &[(S::Damage, 0.5), (S::CritChance, 0.5), (S::DamageTaken, 1.0)], 2),
            StaticRadio => d("The Static Radio", "RAD", "It's picking something up", Cursed, &[], 1),
            WidowsRing => d("Widow's Ring", "WDW", "Hang on by a thread. Thrive there", Cursed, &[(S::MaxHpMult, -0.20)], 1),
            SignalFlare => d("Signal Flare", "FLR", "HERE I AM. COME AND GET ME", Cursed, &[(S::GoldGain, 0.40), (S::XpGain, 0.40)], 1),
            DevouredSunShard => d("Devoured Sun Shard", "SUN", "A piece of what it ate", Cursed, &[(S::Damage, 1.0)], 1),

            BoomerangInsurance => d("Boomerang Insurance", "BMR", "The policy pays out at the far pole", Epic, &[], 1),
            // Marked: each copy (or grade) shortens the recharge, which bottoms out at
            // BLINK_MIN_COOLDOWN by the third — past that a card would be dead.
            AntipodeBlink => d("Antipode Blink", "APB", "Only a sphere has a far side", Rare, &[], 3),
        }
    }

    /// Effect multiplier of one copy rolled at `grade`: ×1 at the native grade, +
    /// `ITEM_GRADE_STEP` per rung above it. Cursed items (and a grade below native, which
    /// no roll produces) carry ×1.
    pub fn grade_mult(&self, grade: Rarity) -> f32 {
        match (self.def().rarity.rung(), grade.rung()) {
            (Some(native), Some(g)) if g > native => 1.0 + ITEM_GRADE_STEP * (g - native) as f32,
            _ => 1.0,
        }
    }

    pub fn is_cursed(&self) -> bool {
        self.def().rarity == Rarity::Cursed
    }

    /// What the item DOES beyond its stat boosts, at an effect multiplier of `m` (the
    /// graded power a card or stack carries). None for pure stat items.
    pub fn effect(&self, m: f32) -> Option<String> {
        use ItemKind::*;
        Some(match self {
            OrbitalYoYo => format!(
                "Every {YOYO_PERIOD:.0}s debris orbits you once: {:.0} dmg. Wider on bigger worlds",
                YOYO_DAMAGE * m
            ),
            CometTail => format!(
                "Burning trail while moving ({:.0} dmg/s). Stand still {COMET_TAIL_STILL_SECS:.0}s to ignite it: {:.0} dmg",
                COMET_TAIL_DPS * m,
                COMET_TAIL_IGNITE_DAMAGE * m
            ),
            TheOverheat => format!("Every {OVERHEAT_JAM_EVERY}th volley jams your guns {OVERHEAT_JAM_SECS:.0}s"),
            DownhillMomentum => format!(
                "+{:.1}% dmg per metre descended in the last second",
                DOWNHILL_DMG_PER_M * m * 100.0
            ),
            SecondAstronaut => format!(
                "A ghost co-pilot fires one of your weapons at {:.0}%",
                GHOST_MIRROR * m * 100.0
            ),
            EncirclementBonus => format!(
                "+{:.0}% dmg per compass direction the horde holds (max +{:.0}%)",
                ENCIRCLE_DMG_PER_DIR * m * 100.0,
                ENCIRCLE_DMG_PER_DIR * 8.0 * m * 100.0
            ),
            IcarusBoots => format!(
                "+{:.0}% dmg airborne, -{:.0}% grounded",
                ICARUS_AIR_BONUS * m * 100.0,
                ICARUS_GROUND_PENALTY * 100.0
            ),
            AntiGravBoots => format!(
                "Hold jump to hover {ANTIGRAV_HOVER_SECS:.0}s. Airborne, every weapon fires a full 360 ring"
            ),
            LittleBlackHole => format!(
                "Every {BLACK_HOLE_PERIOD:.0}s pulls all enemies within {BLACK_HOLE_RADIUS:.0}m to one point"
            ),
            DeadMansTether => format!(
                "When you'd die, rewind {TETHER_REWIND_SECS:.0}s to where you were, at 1 HP. Once per run"
            ),
            CrackedHelmet => "You take double damage".into(),
            StaticRadio => format!(
                "+1 Rarity Grade to all your loot. The Static arrives {STATIC_RADIO_LEAD_SECS:.0}s early, and angrier"
            ),
            WidowsRing => format!(
                "+{:.0}% to every stat at 1 HP. A lethal hit leaves you at 1 HP instead (every {WIDOW_SAVE_COOLDOWN:.0}s)",
                WIDOW_STAT_BONUS * 100.0
            ),
            SignalFlare => "The horde always knows where you are: it chases you first and lands closer".into(),
            DevouredSunShard => format!(
                "Every {SUN_SHARD_PERIOD:.0}s the day side shrinks toward total night"
            ),
            BoomerangInsurance => format!(
                "Below {:.0}% HP, auto-blink to the antipode. Once per dip; shares the blink's recharge",
                BOOMERANG_INSURANCE_HP * 100.0
            ),
            AntipodeBlink => format!(
                "Q: blink to the planet's exact opposite point. Recharge {:.0}s. Read the far side first",
                crate::techs::blink_cooldown_for(m)
            ),
            _ => return None,
        })
    }

    /// Explicit wire code (co-op build sync). Never renumber, only append.
    pub fn code(&self) -> u8 {
        use ItemKind::*;
        match self {
            SpaceBorgar => 0,
            MoonCheese => 1,
            DuctTape => 2,
            SlipperyVisor => 3,
            ProteinPaste => 4,
            OverclockedCpu => 5,
            FishBowlHelmet => 6,
            RocketBoots => 7,
            TrampolineSoles => 8,
            MagnetBoots => 9,
            LuckyMeteorite => 10,
            SpaceCreditCard => 11,
            GoldenAntenna => 12,
            StarChart => 13,
            LaserSight => 14,
            HeavyPayload => 15,
            ExtraBattery => 16,
            SplitterChip => 17,
            CursedMoonRock => 18,
            ThornPlating => 19,
            VampireVisor => 20,
            CaffeineIv => 21,
            OrbitalYoYo => 22,
            CometTail => 23,
            TheOverheat => 24,
            DownhillMomentum => 25,
            SecondAstronaut => 26,
            EncirclementBonus => 27,
            IcarusBoots => 28,
            AntiGravBoots => 29,
            LittleBlackHole => 30,
            DeadMansTether => 31,
            CrackedHelmet => 32,
            StaticRadio => 33,
            WidowsRing => 34,
            SignalFlare => 35,
            DevouredSunShard => 36,
            BoomerangInsurance => 37,
            AntipodeBlink => 38,
        }
    }
    pub fn from_code(c: u8) -> Option<ItemKind> {
        Self::ALL.into_iter().find(|i| i.code() == c)
    }

    /// Test-harness lookup (`--items`): the name lowercased with everything but letters and
    /// digits dropped, and a leading "the" optional — "orbitalyoyo", "staticradio".
    pub fn from_name(s: &str) -> Option<ItemKind> {
        let norm = |n: &str| n.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
        let want = norm(s);
        Self::ALL.into_iter().find(|i| {
            let n = norm(i.def().name);
            n == want || n.strip_prefix("the") == Some(want.as_str())
        })
    }
}
