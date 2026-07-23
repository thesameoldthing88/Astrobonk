use super::items::ItemKind;
use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WeaponKind {
    // Base weapons
    Wrench,
    LaserPistol,
    RivetGun,
    Kunai,
    Boomerang,
    MiningLaser,
    Drones,
    Tesla,
    RocketPod,
    CryoVent,
    // Evolutions
    MegaWrench,
    GatlingLaser,
    Riveter9000,
    BladeStorm,
    SatelliteArray,
    DeathRay,
    DroneSwarm,
    StormCore,
    MirvPod,
    AbsoluteZero,
}

#[derive(Clone, Copy, Debug)]
pub enum Behavior {
    /// Sweeping melee arc in the facing direction.
    MeleeArc { arc_deg: f32, range: f32 },
    /// Straight tangent-following shots at the nearest enemy.
    Shot { speed: f32, pierce: u32, spread_deg: f32 },
    /// Projectile that curves to seek the closest enemy.
    Seek { speed: f32, pierce: u32 },
    /// Flies out and returns, hitting on both legs.
    Boomerang { speed: f32, range: f32 },
    /// Instant beam toward the nearest enemy, damage ticks while held.
    Beam { range: f32, width: f32 },
    /// Bodies orbiting the player.
    Orbit { radius: f32, deg_per_sec: f32 },
    /// Zap the nearest enemy, chaining to neighbors.
    Chain { jumps: u32, range: f32, link_range: f32 },
    /// Slow homing rocket with an area explosion.
    Rocket { speed: f32, aoe: f32 },
    /// Damaging + slowing aura around the player.
    Aura { radius: f32, slow: f32 },
}

pub struct WeaponDef {
    pub kind: WeaponKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub damage: f32,
    pub cooldown: f32,
    pub projectiles: u32,
    pub behavior: Behavior,
    pub color: Color,
    pub evolves_to: Option<WeaponKind>,
    pub evo_item: Option<ItemKind>,
}

impl WeaponKind {
    pub const BASE: [WeaponKind; 10] = [
        WeaponKind::Wrench,
        WeaponKind::LaserPistol,
        WeaponKind::RivetGun,
        WeaponKind::Kunai,
        WeaponKind::Boomerang,
        WeaponKind::MiningLaser,
        WeaponKind::Drones,
        WeaponKind::Tesla,
        WeaponKind::RocketPod,
        WeaponKind::CryoVent,
    ];

    pub fn def(&self) -> WeaponDef {
        use Behavior::*;
        use WeaponKind::*;
        match self {
            Wrench => WeaponDef {
                kind: *self,
                name: "Wrench",
                desc: "Bonks everything in a wide arc",
                damage: 14.0,
                cooldown: 1.05,
                projectiles: 1,
                behavior: MeleeArc { arc_deg: 150.0, range: 3.4 },
                color: Color::srgb(0.85, 0.55, 0.20),
                evolves_to: Some(MegaWrench),
                evo_item: Some(ItemKind::ProteinPaste),
            },
            MegaWrench => WeaponDef {
                kind: *self,
                name: "MEGA WRENCH",
                desc: "The bonk heard around the planet",
                damage: 46.0,
                cooldown: 0.95,
                projectiles: 1,
                behavior: MeleeArc { arc_deg: 360.0, range: 4.6 },
                color: Color::srgb(1.0, 0.62, 0.10),
                evolves_to: None,
                evo_item: None,
            },
            LaserPistol => WeaponDef {
                kind: *self,
                name: "Laser Pistol",
                desc: "Zaps the nearest invader",
                damage: 10.0,
                cooldown: 0.75,
                projectiles: 1,
                behavior: Shot { speed: 30.0, pierce: 0, spread_deg: 5.0 },
                color: Color::srgb(1.0, 0.25, 0.25),
                evolves_to: Some(GatlingLaser),
                evo_item: Some(ItemKind::OverclockedCpu),
            },
            GatlingLaser => WeaponDef {
                kind: *self,
                name: "Gatling Laser",
                desc: "The trigger is taped down",
                damage: 9.0,
                cooldown: 0.11,
                projectiles: 1,
                behavior: Shot { speed: 38.0, pierce: 1, spread_deg: 9.0 },
                color: Color::srgb(1.0, 0.1, 0.1),
                evolves_to: None,
                evo_item: None,
            },
            RivetGun => WeaponDef {
                kind: *self,
                name: "Rivet Gun",
                desc: "Sprays hot rivets in a cone",
                damage: 7.0,
                cooldown: 1.1,
                projectiles: 4,
                behavior: Shot { speed: 26.0, pierce: 0, spread_deg: 26.0 },
                color: Color::srgb(0.9, 0.8, 0.3),
                evolves_to: Some(Riveter9000),
                evo_item: Some(ItemKind::LaserSight),
            },
            Riveter9000 => WeaponDef {
                kind: *self,
                name: "RIVETER 9000",
                desc: "OSHA has left the solar system",
                damage: 11.0,
                cooldown: 0.8,
                projectiles: 8,
                behavior: Shot { speed: 30.0, pierce: 1, spread_deg: 40.0 },
                color: Color::srgb(1.0, 0.9, 0.2),
                evolves_to: None,
                evo_item: None,
            },
            Kunai => WeaponDef {
                kind: *self,
                name: "Kunai",
                desc: "Smart blades hunt the closest target",
                damage: 12.0,
                cooldown: 0.9,
                projectiles: 1,
                behavior: Seek { speed: 24.0, pierce: 0 },
                color: Color::srgb(0.7, 0.85, 1.0),
                evolves_to: Some(BladeStorm),
                evo_item: Some(ItemKind::CaffeineIv),
            },
            BladeStorm => WeaponDef {
                kind: *self,
                name: "BLADE STORM",
                desc: "A weather system of knives",
                damage: 16.0,
                cooldown: 0.35,
                projectiles: 3,
                behavior: Seek { speed: 32.0, pierce: 1 },
                color: Color::srgb(0.75, 0.95, 1.0),
                evolves_to: None,
                evo_item: None,
            },
            Boomerang => WeaponDef {
                kind: *self,
                name: "Boomerang Antenna",
                desc: "Comes back. Usually.",
                damage: 15.0,
                cooldown: 1.5,
                projectiles: 1,
                behavior: Behavior::Boomerang { speed: 22.0, range: 14.0 },
                color: Color::srgb(0.4, 1.0, 0.6),
                evolves_to: Some(SatelliteArray),
                evo_item: Some(ItemKind::GoldenAntenna),
            },
            SatelliteArray => WeaponDef {
                kind: *self,
                name: "SATELLITE ARRAY",
                desc: "Full-orbit broadcast of pain",
                damage: 24.0,
                cooldown: 1.1,
                projectiles: 3,
                behavior: Behavior::Boomerang { speed: 28.0, range: 20.0 },
                color: Color::srgb(0.3, 1.0, 0.75),
                evolves_to: None,
                evo_item: None,
            },
            MiningLaser => WeaponDef {
                kind: *self,
                name: "Mining Laser",
                desc: "Cuts a line through the swarm",
                damage: 8.0, // per tick
                cooldown: 2.4,
                projectiles: 1,
                behavior: Beam { range: 16.0, width: 0.9 },
                color: Color::srgb(1.0, 0.5, 0.1),
                evolves_to: Some(DeathRay),
                evo_item: Some(ItemKind::HeavyPayload),
            },
            DeathRay => WeaponDef {
                kind: *self,
                name: "DEATH RAY",
                desc: "Strip-mines the horizon",
                damage: 18.0,
                cooldown: 1.8,
                projectiles: 1,
                behavior: Beam { range: 26.0, width: 2.2 },
                color: Color::srgb(1.0, 0.2, 0.05),
                evolves_to: None,
                evo_item: None,
            },
            Drones => WeaponDef {
                kind: *self,
                name: "Orbital Drones",
                desc: "Little buddies on patrol",
                damage: 11.0,
                cooldown: 0.0,
                projectiles: 2,
                behavior: Orbit { radius: 3.4, deg_per_sec: 190.0 },
                color: Color::srgb(0.6, 0.7, 0.9),
                evolves_to: Some(DroneSwarm),
                evo_item: Some(ItemKind::SplitterChip),
            },
            DroneSwarm => WeaponDef {
                kind: *self,
                name: "DRONE SWARM",
                desc: "The airspace is closed",
                damage: 16.0,
                cooldown: 0.0,
                projectiles: 6,
                behavior: Orbit { radius: 4.4, deg_per_sec: 260.0 },
                color: Color::srgb(0.7, 0.8, 1.0),
                evolves_to: None,
                evo_item: None,
            },
            Tesla => WeaponDef {
                kind: *self,
                name: "Tesla Coil",
                desc: "Lightning that likes company",
                damage: 13.0,
                cooldown: 1.3,
                projectiles: 1,
                behavior: Chain { jumps: 3, range: 12.0, link_range: 5.0 },
                color: Color::srgb(0.5, 0.8, 1.0),
                evolves_to: Some(StormCore),
                evo_item: Some(ItemKind::ExtraBattery),
            },
            StormCore => WeaponDef {
                kind: *self,
                name: "STORM CORE",
                desc: "You are the weather now",
                damage: 22.0,
                cooldown: 0.9,
                projectiles: 2,
                behavior: Chain { jumps: 6, range: 15.0, link_range: 7.0 },
                color: Color::srgb(0.6, 0.9, 1.0),
                evolves_to: None,
                evo_item: None,
            },
            RocketPod => WeaponDef {
                kind: *self,
                name: "Rocket Pod",
                desc: "Homing hugs that explode",
                damage: 26.0,
                cooldown: 2.1,
                projectiles: 1,
                behavior: Rocket { speed: 14.0, aoe: 3.6 },
                color: Color::srgb(1.0, 0.6, 0.6),
                evolves_to: Some(MirvPod),
                evo_item: Some(ItemKind::CursedMoonRock),
            },
            MirvPod => WeaponDef {
                kind: *self,
                name: "MIRV POD",
                desc: "One rocket, many opinions",
                damage: 30.0,
                cooldown: 1.7,
                projectiles: 3,
                behavior: Rocket { speed: 17.0, aoe: 4.6 },
                color: Color::srgb(1.0, 0.4, 0.4),
                evolves_to: None,
                evo_item: None,
            },
            CryoVent => WeaponDef {
                kind: *self,
                name: "Cryo Vent",
                desc: "A personal winter",
                damage: 5.0, // per tick
                cooldown: 0.5,
                projectiles: 1,
                behavior: Aura { radius: 4.2, slow: 0.45 },
                color: Color::srgb(0.6, 0.9, 1.0),
                evolves_to: Some(AbsoluteZero),
                evo_item: Some(ItemKind::FishBowlHelmet),
            },
            AbsoluteZero => WeaponDef {
                kind: *self,
                name: "ABSOLUTE ZERO",
                desc: "Physics files a complaint",
                damage: 12.0,
                cooldown: 0.45,
                projectiles: 1,
                behavior: Aura { radius: 6.4, slow: 0.75 },
                color: Color::srgb(0.75, 0.98, 1.0),
                evolves_to: None,
                evo_item: None,
            },
        }
    }

    /// Per-level scaling: damage multiplier, bonus projectiles, size multiplier.
    pub fn level_scaling(&self, level: u32) -> (f32, u32, f32) {
        let l = level.saturating_sub(1) as f32;
        let dmg = 1.0 + 0.24 * l;
        let extra = match level {
            0..=2 => 0,
            3..=4 => 1,
            5..=6 => 2,
            _ => 3,
        };
        let size = 1.0 + 0.06 * l;
        (dmg, extra, size)
    }

    pub fn is_evolution(&self) -> bool {
        self.def().evolves_to.is_none() && self.def().evo_item.is_none() && !Self::BASE.contains(self)
    }
}
