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
    // Base weapons — batch 1
    MeatballComet,
    StaticCling,
    RicochetDisc,
    SonicWhoopee,
    CosmonautsBell,
    YoYo,
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
    // Evolutions — batch 1
    RaguRain,
    FullDischarge,
    Omnidisc,
    BrownNote,
    Angelus,
    SwordYo,
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
    // ---- the six Tier-1 weapons' own behaviours (GDD §6; `arsenal.rs`) ----
    /// A mortar lobbed high onto the densest clump in `range` (arc m) — over the horizon —
    /// landing after about `flight` s in an `aoe` splatter. `split` > 0: on the way down it
    /// breaks into that many, blanketing the ring round the target (RAGÙ RAIN).
    Lob { range: f32, flight: f32, aoe: f32, split: u32 },
    /// A tight damage field that bites hardest on what hugs you, and harder the more of
    /// them there are. `nova` > 0: every FULL_DISCHARGE_SECS the stored charge goes off as
    /// a screen-clearing nova of that radius (FULL DISCHARGE).
    Hug { radius: f32, nova: f32 },
    /// A disc that bounces enemy to enemy (`bounces`; `u32::MAX` = never runs out), and with
    /// no one left in `link` range rolls on round its great circle for a whole lap.
    Disc { speed: f32, bounces: u32, link: f32 },
    /// A cone blast that shoves and stuns what it catches — the panic button.
    Cone { arc_deg: f32, range: f32 },
    /// A ring that rolls out from you to `radius`, shoving, stunning and hurting everything
    /// it passes (THE BROWN NOTE).
    Repulsor { radius: f32 },
    /// Tolls on its cooldown: damages everything in `radius` and marks it for +crit.
    /// `wisps`: each toll raises recent dead as friendly Static-wisps (THE ANGELUS).
    Toll { radius: f32, wisps: bool },
    /// Yo-yos on a cord, orbiting close; damage and reach grow with the un-hit move combo.
    /// `garrote`: at max combo the cord pays out to garrote the whole ring (SWORD-YO).
    Tether { radius: f32, deg_per_sec: f32, garrote: bool },
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
    pub const BASE: [WeaponKind; 16] = [
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
        WeaponKind::MeatballComet,
        WeaponKind::StaticCling,
        WeaponKind::RicochetDisc,
        WeaponKind::SonicWhoopee,
        WeaponKind::CosmonautsBell,
        WeaponKind::YoYo,
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

            // ---- Batch 1: new base weapons + evolutions ----
            MeatballComet => WeaponDef {
                kind: *self,
                name: "Meatball Comet",
                desc: "Lobs a frozen meatball over the horizon; splat",
                damage: 24.0,
                cooldown: 1.7,
                projectiles: 1,
                behavior: Lob { range: 38.0, flight: 1.05, aoe: 3.6, split: 0 },
                color: Color::srgb(0.75, 0.35, 0.20),
                evolves_to: Some(RaguRain),
                evo_item: Some(ItemKind::FishBowlHelmet),
            },
            RaguRain => WeaponDef {
                kind: *self,
                name: "RAGÙ RAIN",
                desc: "Nonna's recipe. Survived reentry. Barely.",
                damage: 30.0,
                cooldown: 1.35,
                projectiles: 1,
                behavior: Lob { range: 42.0, flight: 1.2, aoe: 4.0, split: 3 },
                color: Color::srgb(0.9, 0.3, 0.15),
                evolves_to: None,
                evo_item: None,
            },
            StaticCling => WeaponDef {
                kind: *self,
                name: "Static Cling",
                desc: "Shocks whatever hugs you. The more, the worse",
                damage: 7.0, // per tick, at contact
                cooldown: 0.35,
                projectiles: 1,
                behavior: Hug { radius: 2.9, nova: 0.0 },
                color: Color::srgb(0.7, 0.7, 1.0),
                evolves_to: Some(FullDischarge),
                evo_item: Some(ItemKind::ThornPlating),
            },
            FullDischarge => WeaponDef {
                kind: *self,
                name: "FULL DISCHARGE",
                desc: "Ghost-cosmonauts hate this one weird trick",
                damage: 12.0,
                cooldown: 0.3,
                projectiles: 1,
                behavior: Hug { radius: 3.5, nova: 18.0 },
                color: Color::srgb(0.8, 0.8, 1.0),
                evolves_to: None,
                evo_item: None,
            },
            RicochetDisc => WeaponDef {
                kind: *self,
                name: "Ricochet Disc",
                desc: "Skips enemy to enemy, then laps the planet",
                damage: 12.0,
                cooldown: 1.3,
                projectiles: 1,
                behavior: Disc { speed: 40.0, bounces: 4, link: 7.0 },
                color: Color::srgb(0.6, 1.0, 0.9),
                evolves_to: Some(Omnidisc),
                evo_item: Some(ItemKind::LuckyMeteorite),
            },
            Omnidisc => WeaponDef {
                kind: *self,
                name: "THE OMNIDISC",
                desc: "Home is wherever you're standing",
                damage: 16.0,
                cooldown: 1.0,
                projectiles: 2,
                behavior: Disc { speed: 46.0, bounces: u32::MAX, link: 9.0 },
                color: Color::srgb(0.7, 1.0, 1.0),
                evolves_to: None,
                evo_item: None,
            },
            SonicWhoopee => WeaponDef {
                kind: *self,
                name: "Sonic Whoopee",
                desc: "A cone of rude noise: shoves, stuns. Panic button",
                damage: 12.0,
                cooldown: 1.7,
                projectiles: 1,
                behavior: Cone { arc_deg: 80.0, range: 7.0 },
                color: Color::srgb(0.6, 0.5, 0.35),
                evolves_to: Some(BrownNote),
                evo_item: Some(ItemKind::RocketBoots),
            },
            BrownNote => WeaponDef {
                kind: *self,
                name: "THE BROWN NOTE",
                desc: "Discovered by accident. Weaponized on purpose.",
                damage: 30.0,
                cooldown: 1.4,
                projectiles: 1,
                behavior: Repulsor { radius: 10.0 },
                color: Color::srgb(0.7, 0.55, 0.3),
                evolves_to: None,
                evo_item: None,
            },
            CosmonautsBell => WeaponDef {
                kind: *self,
                name: "Cosmonaut's Bell",
                desc: "Tolls every 4s; the tolled are marked for crits",
                damage: 18.0,
                cooldown: 4.0,
                projectiles: 1,
                behavior: Toll { radius: 8.0, wisps: false },
                color: Color::srgb(0.95, 0.9, 0.6),
                evolves_to: Some(Angelus),
                evo_item: Some(ItemKind::StarChart),
            },
            Angelus => WeaponDef {
                kind: *self,
                name: "THE ANGELUS",
                desc: "Every toll is a mercy and a threat",
                damage: 28.0,
                cooldown: 4.0,
                projectiles: 1,
                behavior: Toll { radius: 10.0, wisps: true },
                color: Color::srgb(1.0, 0.95, 0.7),
                evolves_to: None,
                evo_item: None,
            },
            YoYo => WeaponDef {
                kind: *self,
                name: "Yo-Yo of Damocles",
                desc: "Spiked yo-yo on a cord. Keep moving, don't get hit",
                damage: 12.0,
                cooldown: 0.0,
                projectiles: 1,
                behavior: Tether { radius: 2.7, deg_per_sec: 250.0, garrote: false },
                color: Color::srgb(0.9, 0.4, 0.5),
                evolves_to: Some(SwordYo),
                evo_item: Some(ItemKind::HeavyPayload),
            },
            SwordYo => WeaponDef {
                kind: *self,
                name: "SWORD-YO",
                desc: "Down. Up. Existential. Down again.",
                damage: 19.0,
                cooldown: 0.0,
                projectiles: 2,
                behavior: Tether { radius: 3.4, deg_per_sec: 290.0, garrote: true },
                color: Color::srgb(1.0, 0.5, 0.6),
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

    /// Explicit wire code (Second Astronaut's ghost weapon in `net::NetItemVis`). Never
    /// renumber, only append.
    pub fn code(&self) -> u8 {
        use WeaponKind::*;
        match self {
            Wrench => 0,
            LaserPistol => 1,
            RivetGun => 2,
            Kunai => 3,
            Boomerang => 4,
            MiningLaser => 5,
            Drones => 6,
            Tesla => 7,
            RocketPod => 8,
            CryoVent => 9,
            MeatballComet => 10,
            StaticCling => 11,
            RicochetDisc => 12,
            SonicWhoopee => 13,
            CosmonautsBell => 14,
            YoYo => 15,
            MegaWrench => 16,
            GatlingLaser => 17,
            Riveter9000 => 18,
            BladeStorm => 19,
            SatelliteArray => 20,
            DeathRay => 21,
            DroneSwarm => 22,
            StormCore => 23,
            MirvPod => 24,
            AbsoluteZero => 25,
            RaguRain => 26,
            FullDischarge => 27,
            Omnidisc => 28,
            BrownNote => 29,
            Angelus => 30,
            SwordYo => 31,
        }
    }
    pub fn from_code(c: u8) -> Option<WeaponKind> {
        use WeaponKind::*;
        [
            Wrench, LaserPistol, RivetGun, Kunai, Boomerang, MiningLaser, Drones, Tesla, RocketPod,
            CryoVent, MeatballComet, StaticCling, RicochetDisc, SonicWhoopee, CosmonautsBell, YoYo,
            MegaWrench, GatlingLaser, Riveter9000, BladeStorm, SatelliteArray, DeathRay, DroneSwarm,
            StormCore, MirvPod, AbsoluteZero, RaguRain, FullDischarge, Omnidisc, BrownNote, Angelus,
            SwordYo,
        ]
        .into_iter()
        .find(|w| w.code() == c)
    }

    /// The base weapon this evolution grew from (`None` for a base weapon).
    pub fn evolved_from(&self) -> Option<WeaponKind> {
        Self::BASE.into_iter().find(|b| b.def().evolves_to == Some(*self))
    }

    /// Which base weapons use `item` as their evolution catalyst (for item-card hints).
    pub fn catalyst_for(item: ItemKind) -> Vec<WeaponKind> {
        Self::BASE
            .into_iter()
            .filter(|w| w.def().evo_item == Some(item))
            .collect()
    }
}
