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
}

#[derive(Clone, Copy, Debug)]
pub enum Passive {
    DamageMult(f32),
    AttackSpeed(f32),
    CritPerLevel(f32),
    SlideFrenzy { bonus: f32, secs: f32 },
    ExtraJumps(i32),
    GoldGain(f32),
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
    pub const ALL: [AstronautKind; 6] = [
        AstronautKind::Buzz,
        AstronautKind::Valentina,
        AstronautKind::B0nk,
        AstronautKind::Yuki,
        AstronautKind::ChimpO,
        AstronautKind::Doug,
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
        }
    }

    pub fn starts_unlocked(&self) -> bool {
        matches!(self, AstronautKind::Buzz | AstronautKind::Valentina)
    }
}
