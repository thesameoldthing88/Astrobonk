use super::weapons::WeaponKind;
use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AstronautKind {
    Buzz,
    Valentina,
    B0nk,
    Yuki,
    ChimpO,
    Doug,
    // Recruits (batch 1)
    Reticle,
    Nova,
    Ironclad,
    Fortuna,
    Aurora,
    Gristle,
}

#[derive(Clone, Copy, Debug)]
pub enum Passive {
    DamageMult(f32),
    AttackSpeed(f32),
    CritPerLevel(f32),
    SlideFrenzy { bonus: f32, secs: f32 },
    ExtraJumps(i32),
    GoldGain(f32),
    // Batch-1 recruit passives (flat stat identities)
    CritChance(f32),
    MoveSpeed(f32),
    MaxHp(f32),
    Luck(f32),
    Size(f32),
}

pub struct AstronautDef {
    pub kind: AstronautKind,
    pub name: &'static str,
    pub agency: &'static str,
    pub desc: &'static str,
    pub weapon: WeaponKind,
    pub passive: Passive,
    pub passive_desc: &'static str,
    pub suit: Color,
    pub visor: Color,
    pub unlock_desc: &'static str,
}

impl AstronautKind {
    pub const ALL: [AstronautKind; 12] = [
        AstronautKind::Buzz,
        AstronautKind::Valentina,
        AstronautKind::B0nk,
        AstronautKind::Yuki,
        AstronautKind::ChimpO,
        AstronautKind::Doug,
        AstronautKind::Reticle,
        AstronautKind::Nova,
        AstronautKind::Ironclad,
        AstronautKind::Fortuna,
        AstronautKind::Aurora,
        AstronautKind::Gristle,
    ];

    pub fn def(&self) -> AstronautDef {
        use AstronautKind::*;
        match self {
            Buzz => AstronautDef {
                kind: *self,
                name: "BUZZ",
                agency: "NASA (retired, angry)",
                desc: "Fixes problems. Percussively.",
                weapon: WeaponKind::Wrench,
                passive: Passive::DamageMult(0.10),
                passive_desc: "+10% Damage",
                suit: Color::srgb(0.92, 0.92, 0.95),
                visor: Color::srgb(1.0, 0.75, 0.2),
                unlock_desc: "Available from launch",
            },
            Valentina => AstronautDef {
                kind: *self,
                name: "VALENTINA",
                agency: "Roscosmos legend",
                desc: "First woman to bonk in orbit.",
                weapon: WeaponKind::LaserPistol,
                passive: Passive::AttackSpeed(0.15),
                passive_desc: "+15% Attack Speed",
                suit: Color::srgb(0.85, 0.45, 0.40),
                visor: Color::srgb(0.6, 0.9, 1.0),
                unlock_desc: "Available from launch",
            },
            B0nk => AstronautDef {
                kind: *self,
                name: "B0-NK",
                agency: "Assembled from spare probes",
                desc: "Beeps in a threatening manner.",
                weapon: WeaponKind::RivetGun,
                passive: Passive::CritPerLevel(0.005),
                passive_desc: "+0.5% Crit Chance per level",
                suit: Color::srgb(0.55, 0.60, 0.68),
                visor: Color::srgb(1.0, 0.2, 0.2),
                unlock_desc: "Clear MOON Tier 1",
            },
            Yuki => AstronautDef {
                kind: *self,
                name: "YUKI",
                agency: "JAXA special ops",
                desc: "The vacuum makes no sound. She does.",
                weapon: WeaponKind::Kunai,
                passive: Passive::SlideFrenzy { bonus: 0.30, secs: 3.0 },
                passive_desc: "Slide grants +30% Attack Speed for 3s",
                suit: Color::srgb(0.25, 0.28, 0.35),
                visor: Color::srgb(1.0, 0.4, 0.7),
                unlock_desc: "Clear MARS Tier 1",
            },
            ChimpO => AstronautDef {
                kind: *self,
                name: "CHIMP-O",
                agency: "Project Mercury alumnus",
                desc: "They never brought him home. He waited.",
                weapon: WeaponKind::Boomerang,
                passive: Passive::ExtraJumps(1),
                passive_desc: "+1 Jump",
                suit: Color::srgb(0.45, 0.32, 0.22),
                visor: Color::srgb(0.9, 0.9, 0.5),
                unlock_desc: "Find and free the cage on the MOON",
            },
            Doug => AstronautDef {
                kind: *self,
                name: "DOUG",
                agency: "Asteroid mining union, local 7",
                desc: "Paid by the rock.",
                weapon: WeaponKind::MiningLaser,
                passive: Passive::GoldGain(0.25),
                passive_desc: "+25% Gold Gain",
                suit: Color::srgb(0.95, 0.75, 0.15),
                visor: Color::srgb(0.3, 0.3, 0.35),
                unlock_desc: "Bonk 2,500 invaders (lifetime)",
            },
            Reticle => AstronautDef {
                kind: *self,
                name: "DR. RETICLE",
                agency: "Sniper school (dishonorably retired)",
                desc: "Never misses. Rarely welcome.",
                weapon: WeaponKind::LaserPistol,
                passive: Passive::CritChance(0.20),
                passive_desc: "+20% Crit Chance",
                suit: Color::srgb(0.20, 0.30, 0.25),
                visor: Color::srgb(1.0, 0.25, 0.25),
                unlock_desc: "Land 500 crits in one run",
            },
            Nova => AstronautDef {
                kind: *self,
                name: "SLIPSTREAM NOVA",
                agency: "Artemis stowaway",
                desc: "Stop moving and you stop living.",
                weapon: WeaponKind::CryoVent,
                passive: Passive::MoveSpeed(0.15),
                passive_desc: "+15% Move; weapons barely cool down while sprinting",
                suit: Color::srgb(0.25, 0.85, 0.80),
                visor: Color::srgb(1.0, 0.4, 0.9),
                unlock_desc: "Circle a planet 3x in under 40s",
            },
            Ironclad => AstronautDef {
                kind: *self,
                name: "OLD IRONCLAD",
                agency: "Apollo-era, welded into his suit",
                desc: "Does not move. Cannot be moved.",
                weapon: WeaponKind::RocketPod,
                passive: Passive::MaxHp(90.0),
                passive_desc: "+90 Max HP; armor doubles below 30% HP",
                suit: Color::srgb(0.45, 0.48, 0.52),
                visor: Color::srgb(0.9, 0.85, 0.5),
                unlock_desc: "Survive THE STATIC for 5 minutes",
            },
            Fortuna => AstronautDef {
                kind: *self,
                name: "LADY FORTUNA",
                agency: "Casino heiress who bought a seat",
                desc: "The house always bonks.",
                weapon: WeaponKind::Boomerang,
                passive: Passive::Luck(0.30),
                passive_desc: "+30% Luck, +2 Refreshes",
                suit: Color::srgb(0.85, 0.20, 0.55),
                visor: Color::srgb(1.0, 0.85, 0.3),
                unlock_desc: "Open 20 Legendary chests (lifetime)",
            },
            Aurora => AstronautDef {
                kind: *self,
                name: "AURORA PRIME",
                agency: "Priestess of the dead sun",
                desc: "Blesses the void with static.",
                weapon: WeaponKind::StaticCling,
                passive: Passive::Size(0.25),
                passive_desc: "+25% Size (bigger auras & hits)",
                suit: Color::srgb(0.55, 0.45, 0.90),
                visor: Color::srgb(0.7, 1.0, 1.0),
                unlock_desc: "Kill 1,000 enemies with an aura weapon",
            },
            Gristle => AstronautDef {
                kind: *self,
                name: "SGT. GRISTLE",
                agency: "ESA marine, one lung, no chill",
                desc: "Solves crowding with a shockwave.",
                weapon: WeaponKind::SonicWhoopee,
                passive: Passive::DamageMult(0.15),
                passive_desc: "+15% Damage; +40% more while below half HP",
                suit: Color::srgb(0.35, 0.42, 0.30),
                visor: Color::srgb(1.0, 0.55, 0.2),
                unlock_desc: "Kill 3 bosses without dodging",
            },
        }
    }

    /// Parse a hero from a short lowercase name (for the headless `--hero` flag).
    pub fn from_name(s: &str) -> Option<AstronautKind> {
        use AstronautKind::*;
        Some(match s.to_lowercase().as_str() {
            "buzz" => Buzz,
            "valentina" => Valentina,
            "b0nk" | "bonk" => B0nk,
            "yuki" => Yuki,
            "chimpo" | "chimp" => ChimpO,
            "doug" => Doug,
            "reticle" => Reticle,
            "nova" => Nova,
            "ironclad" => Ironclad,
            "fortuna" => Fortuna,
            "aurora" => Aurora,
            "gristle" => Gristle,
            _ => return None,
        })
    }

    /// Batch-1 recruits start unlocked so they're immediately playable.
    /// (Proper quest-gating is a follow-up once the quest batch lands.)
    pub fn starts_unlocked(&self) -> bool {
        !matches!(
            self,
            AstronautKind::B0nk | AstronautKind::Yuki | AstronautKind::ChimpO | AstronautKind::Doug
        )
    }
}
