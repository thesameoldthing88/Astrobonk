//! The Suit Wardrobe (locked direction #3/#4): ONE astronaut, MILO, trying to get home to
//! his pet turtle, and the twelve suits he can wear. Each suit is a kit (signature weapon +
//! passive) and a look (palette, trim, helmet); the body inside, his proportions and the
//! turtle-shell patch on his backpack, never changes (`player::build_astronaut_rig`).
//!
//! Staged rename: the twelve were once twelve heroes, and `AstronautKind` is kept as an
//! alias of `SuitKind` so code that other packages are still writing keeps compiling. New code
//! says `SuitKind`; the variant names are the save's and the wire's keys, so they stay.

use super::weapons::WeaponKind;
use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

/// The protagonist's name, everywhere a player-facing line needs him (PROJECT_STATUS §2).
pub const PROTAGONIST: &str = "MILO";
/// The turtle-shell patch on his backpack: the one mark every suit keeps (and the first
/// thing the chase camera sees).
pub const TURTLE_SHELL: Color = Color::srgb(0.30, 0.62, 0.30);
pub const TURTLE_SCUTES: Color = Color::srgb(0.16, 0.36, 0.18);
pub const TURTLE_SKIN: Color = Color::srgb(0.62, 0.80, 0.42);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuitKind {
    #[default]
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

// Staged-rename alias (see the module doc). A re-export rather than a type alias, so
// `use AstronautKind::*` still reaches the variants.
pub use self::SuitKind as AstronautKind;

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

/// Each suit's helmet. The dome stays Milo-sized (he is recognisable in every suit); what
/// changes is the shell's shape detail, its trim and what glows (`suits::helmet_meshes`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Helmet {
    /// NASA dome, visor rim and a comm box.
    Classic,
    /// Sokol pressure hood: a crest stripe over the top.
    Sokol,
    /// A probe casing: boxy, slit visor, antenna with a blinking tip.
    Probe,
    /// Narrow visor and a headband whose tails stream behind.
    Shinobi,
    /// Mercury-era: round ear cups.
    Mercury,
    /// Hard-hat brim and a headlamp.
    HardHat,
    /// A scope over one eye.
    Monocle,
    /// Swept-back racing fins.
    Racer,
    /// A bolted diving bell with a porthole.
    Diver,
    /// A little crown and a gem.
    Crown,
    /// A floating halo.
    Halo,
    /// A combat brim and a boom mic.
    Combat,
}

impl Helmet {
    pub const ALL: [Helmet; 12] = [
        Helmet::Classic,
        Helmet::Sokol,
        Helmet::Probe,
        Helmet::Shinobi,
        Helmet::Mercury,
        Helmet::HardHat,
        Helmet::Monocle,
        Helmet::Racer,
        Helmet::Diver,
        Helmet::Crown,
        Helmet::Halo,
        Helmet::Combat,
    ];
}

/// How a suit dresses the rig. (P34's visible upgrades layer on top of this, per planet.)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SuitLook {
    /// The shell: torso, limbs, helmet dome.
    pub suit: Color,
    /// The visor and whatever else on the helmet glows.
    pub visor: Color,
    /// Collar, belt, cuffs, boots and the helmet's detail.
    pub trim: Color,
    pub helmet: Helmet,
}

impl SuitLook {
    /// A suit still locked in the wardrobe: its shape (the helmet) teases, its colours don't.
    pub fn silhouette(self) -> Self {
        Self {
            suit: Color::srgb(0.07, 0.07, 0.09),
            visor: Color::srgb(0.16, 0.17, 0.22),
            trim: Color::srgb(0.11, 0.11, 0.14),
            helmet: self.helmet,
        }
    }
}

pub struct SuitDef {
    pub kind: SuitKind,
    /// The suit's name (after the astronaut who wore it first).
    pub name: &'static str,
    /// Where the suit came from.
    pub origin: &'static str,
    pub desc: &'static str,
    pub weapon: WeaponKind,
    pub passive: Passive,
    pub passive_desc: &'static str,
    pub suit: Color,
    pub visor: Color,
    pub trim: Color,
    pub helmet: Helmet,
    pub unlock_desc: &'static str,
}

impl SuitDef {
    pub fn look(&self) -> SuitLook {
        SuitLook { suit: self.suit, visor: self.visor, trim: self.trim, helmet: self.helmet }
    }
}

impl SuitKind {
    pub const ALL: [SuitKind; 12] = [
        SuitKind::Buzz,
        SuitKind::Valentina,
        SuitKind::B0nk,
        SuitKind::Yuki,
        SuitKind::ChimpO,
        SuitKind::Doug,
        SuitKind::Reticle,
        SuitKind::Nova,
        SuitKind::Ironclad,
        SuitKind::Fortuna,
        SuitKind::Aurora,
        SuitKind::Gristle,
    ];

    pub fn def(&self) -> SuitDef {
        use SuitKind::*;
        // Kits (weapon + passive) are the twelve former heroes', unchanged. Player-facing
        // text is ASCII only (the UI font, KNOWN_ISSUES L66).
        match self {
            Buzz => SuitDef {
                kind: *self,
                name: "BUZZ",
                origin: "NASA surplus, retired and angry",
                desc: "Fixes problems. Percussively.",
                weapon: WeaponKind::Wrench,
                passive: Passive::DamageMult(0.10),
                passive_desc: "+10% Damage",
                suit: Color::srgb(0.92, 0.92, 0.95),
                visor: Color::srgb(1.0, 0.75, 0.2),
                trim: Color::srgb(0.22, 0.38, 0.90),
                helmet: Helmet::Classic,
                unlock_desc: "Available from launch",
            },
            Valentina => SuitDef {
                kind: *self,
                name: "VALENTINA",
                origin: "Roscosmos flight suit, legend grade",
                desc: "First suit to bonk in orbit. It remembers how.",
                weapon: WeaponKind::LaserPistol,
                passive: Passive::AttackSpeed(0.15),
                passive_desc: "+15% Attack Speed",
                suit: Color::srgb(0.85, 0.45, 0.40),
                visor: Color::srgb(0.6, 0.9, 1.0),
                trim: Color::srgb(1.0, 0.80, 0.28),
                helmet: Helmet::Sokol,
                unlock_desc: "Available from launch",
            },
            B0nk => SuitDef {
                kind: *self,
                name: "B0-NK",
                origin: "Bolted together from spare probes",
                desc: "Beeps in a threatening manner. Nobody is inside. Probably.",
                weapon: WeaponKind::RivetGun,
                passive: Passive::CritPerLevel(0.005),
                passive_desc: "+0.5% Crit Chance per level",
                suit: Color::srgb(0.55, 0.60, 0.68),
                visor: Color::srgb(1.0, 0.2, 0.2),
                trim: Color::srgb(0.95, 0.78, 0.15),
                helmet: Helmet::Probe,
                unlock_desc: "Clear MOON Tier 1",
            },
            Yuki => SuitDef {
                kind: *self,
                name: "YUKI",
                origin: "JAXA special-ops stealth weave",
                desc: "The vacuum makes no sound. This suit makes less.",
                weapon: WeaponKind::Kunai,
                passive: Passive::SlideFrenzy { bonus: 0.30, secs: 3.0 },
                passive_desc: "Slide grants +30% Attack Speed for 3s",
                suit: Color::srgb(0.25, 0.28, 0.35),
                visor: Color::srgb(1.0, 0.4, 0.7),
                trim: Color::srgb(0.85, 0.16, 0.26),
                helmet: Helmet::Shinobi,
                unlock_desc: "Clear MARS Tier 1",
            },
            ChimpO => SuitDef {
                kind: *self,
                name: "CHIMP-O",
                origin: "Project Mercury, let out at the seams",
                desc: "Its first owner never made it home. It waited for you.",
                weapon: WeaponKind::Boomerang,
                passive: Passive::ExtraJumps(1),
                passive_desc: "+1 Jump",
                suit: Color::srgb(0.45, 0.32, 0.22),
                visor: Color::srgb(0.9, 0.9, 0.5),
                trim: Color::srgb(0.80, 0.82, 0.86),
                helmet: Helmet::Mercury,
                unlock_desc: "Find and open the cage on the MOON",
            },
            Doug => SuitDef {
                kind: *self,
                name: "DOUG",
                origin: "Asteroid miners' union, local 7",
                desc: "Paid by the rock. Hard hat included.",
                weapon: WeaponKind::MiningLaser,
                passive: Passive::GoldGain(0.25),
                passive_desc: "+25% Gold Gain",
                suit: Color::srgb(0.95, 0.75, 0.15),
                visor: Color::srgb(0.3, 0.3, 0.35),
                trim: Color::srgb(1.0, 0.42, 0.10),
                helmet: Helmet::HardHat,
                unlock_desc: "Bonk 2,500 invaders (lifetime)",
            },
            Reticle => SuitDef {
                kind: *self,
                name: "DR. RETICLE",
                origin: "Sniper school, dishonorably retired",
                desc: "Never misses. The scope keeps watching after you look away.",
                weapon: WeaponKind::LaserPistol,
                passive: Passive::CritChance(0.10),
                passive_desc: "+10% Crit; a guaranteed-crit focus pulse every ~1.5s",
                suit: Color::srgb(0.20, 0.30, 0.25),
                visor: Color::srgb(1.0, 0.25, 0.25),
                trim: Color::srgb(0.80, 0.64, 0.30),
                helmet: Helmet::Monocle,
                unlock_desc: "Land 500 crits in one run",
            },
            Nova => SuitDef {
                kind: *self,
                name: "SLIPSTREAM NOVA",
                origin: "Artemis racing skin",
                desc: "Stop moving and you stop living.",
                weapon: WeaponKind::CryoVent,
                passive: Passive::MoveSpeed(0.15),
                passive_desc: "+15% Move; weapons barely cool down while sprinting",
                suit: Color::srgb(0.25, 0.85, 0.80),
                visor: Color::srgb(1.0, 0.4, 0.9),
                trim: Color::srgb(0.95, 0.97, 1.0),
                helmet: Helmet::Racer,
                unlock_desc: "Circle a planet 3x in under 40s",
            },
            Ironclad => SuitDef {
                kind: *self,
                name: "OLD IRONCLAD",
                origin: "Apollo-era diving iron, once welded shut",
                desc: "Does not move. Cannot be moved. Creaks at night.",
                weapon: WeaponKind::RocketPod,
                passive: Passive::MaxHp(90.0),
                passive_desc: "+90 Max HP; armor doubles below 30% HP",
                suit: Color::srgb(0.45, 0.48, 0.52),
                visor: Color::srgb(0.9, 0.85, 0.5),
                trim: Color::srgb(0.76, 0.45, 0.24),
                helmet: Helmet::Diver,
                unlock_desc: "Survive THE STATIC for 5 minutes",
            },
            Fortuna => SuitDef {
                kind: *self,
                name: "LADY FORTUNA",
                origin: "Casino couture, pressurized",
                desc: "The house always bonks.",
                weapon: WeaponKind::Boomerang,
                passive: Passive::Luck(0.30),
                passive_desc: "+30% Luck; a free level-up refresh every level",
                suit: Color::srgb(0.85, 0.20, 0.55),
                visor: Color::srgb(1.0, 0.85, 0.3),
                trim: Color::srgb(0.98, 0.78, 0.26),
                helmet: Helmet::Crown,
                unlock_desc: "Open 20 Legendary chests (lifetime)",
            },
            Aurora => SuitDef {
                kind: *self,
                name: "AURORA PRIME",
                origin: "Vestments of the dead sun's priesthood",
                desc: "Hums hymns into the static. You get used to it.",
                weapon: WeaponKind::StaticCling,
                passive: Passive::Size(0.20),
                passive_desc: "+20% Size; auras swell by +35% while sprinting",
                suit: Color::srgb(0.55, 0.45, 0.90),
                visor: Color::srgb(0.7, 1.0, 1.0),
                trim: Color::srgb(1.0, 0.90, 0.60),
                helmet: Helmet::Halo,
                unlock_desc: "Kill 1,000 enemies with an aura weapon",
            },
            Gristle => SuitDef {
                kind: *self,
                name: "SGT. GRISTLE",
                origin: "ESA marine plate, one lung short",
                desc: "Solves crowding with a shockwave.",
                weapon: WeaponKind::SonicWhoopee,
                passive: Passive::DamageMult(0.15),
                passive_desc: "+15% Damage; +40% more while below half HP",
                suit: Color::srgb(0.35, 0.42, 0.30),
                visor: Color::srgb(1.0, 0.55, 0.2),
                trim: Color::srgb(0.72, 0.62, 0.42),
                helmet: Helmet::Combat,
                unlock_desc: "Kill 3 bosses without dodging",
            },
        }
    }

    pub fn look(&self) -> SuitLook {
        self.def().look()
    }

    /// Parse a suit from a short lowercase name (the `--suit`/`--hero` flags).
    pub fn from_name(s: &str) -> Option<SuitKind> {
        use SuitKind::*;
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

    /// `--suit <name>` on the command line (or its old spelling, `--hero <name>`).
    pub fn from_args(args: &[String]) -> Option<SuitKind> {
        let i = args.iter().position(|a| a == "--suit" || a == "--hero")?;
        args.get(i + 1).and_then(|s| SuitKind::from_name(s))
    }

    /// Batch-1 recruit suits start in the wardrobe so they're immediately wearable.
    /// (Their quest gating is P21's; `unlock_desc` is the condition it will use.)
    pub fn starts_unlocked(&self) -> bool {
        !matches!(self, SuitKind::B0nk | SuitKind::Yuki | SuitKind::ChimpO | SuitKind::Doug)
    }
}

/// Headless self-check: twelve suits, each with its own helmet and a trim that reads apart
/// from its shell, and every former hero's kit carried over exactly (locked direction #4).
pub fn self_check() -> Result<(), String> {
    use std::collections::HashSet;
    use SuitKind::*;
    // the kits as the twelve heroes shipped them (weapon, passive's Debug form)
    let kits: [(SuitKind, WeaponKind, &str); 12] = [
        (Buzz, WeaponKind::Wrench, "DamageMult(0.1)"),
        (Valentina, WeaponKind::LaserPistol, "AttackSpeed(0.15)"),
        (B0nk, WeaponKind::RivetGun, "CritPerLevel(0.005)"),
        (Yuki, WeaponKind::Kunai, "SlideFrenzy { bonus: 0.3, secs: 3.0 }"),
        (ChimpO, WeaponKind::Boomerang, "ExtraJumps(1)"),
        (Doug, WeaponKind::MiningLaser, "GoldGain(0.25)"),
        (Reticle, WeaponKind::LaserPistol, "CritChance(0.1)"),
        (Nova, WeaponKind::CryoVent, "MoveSpeed(0.15)"),
        (Ironclad, WeaponKind::RocketPod, "MaxHp(90.0)"),
        (Fortuna, WeaponKind::Boomerang, "Luck(0.3)"),
        (Aurora, WeaponKind::StaticCling, "Size(0.2)"),
        (Gristle, WeaponKind::SonicWhoopee, "DamageMult(0.15)"),
    ];
    for (k, weapon, passive) in kits {
        let d = k.def();
        if d.kind != k || d.weapon != weapon || format!("{:?}", d.passive) != passive {
            return Err(format!("the {} suit lost its kit ({:?}, {:?})", d.name, d.weapon, d.passive));
        }
        let text = [d.name, d.origin, d.desc, d.passive_desc, d.unlock_desc];
        if text.iter().any(|t| !t.is_ascii() || t.is_empty()) {
            return Err(format!("the {} suit has empty or non-ASCII text (L66)", d.name));
        }
        let (s, t) = (d.suit.to_srgba(), d.trim.to_srgba());
        let apart = (s.red - t.red).abs() + (s.green - t.green).abs() + (s.blue - t.blue).abs();
        if apart < 0.35 {
            return Err(format!("the {} suit's trim does not read against its shell ({apart:.2})", d.name));
        }
        if SuitKind::from_name(&format!("{k:?}")) != Some(k) {
            return Err(format!("--suit cannot name {k:?}"));
        }
    }
    let helmets: HashSet<Helmet> = SuitKind::ALL.iter().map(|k| k.def().helmet).collect();
    if helmets.len() != SuitKind::ALL.len() || Helmet::ALL.len() != SuitKind::ALL.len() {
        return Err(format!("{} suits share {} helmets", SuitKind::ALL.len(), helmets.len()));
    }
    let names: HashSet<&str> = SuitKind::ALL.iter().map(|k| k.def().name).collect();
    if names.len() != SuitKind::ALL.len() {
        return Err("two suits share a name".into());
    }
    Ok(())
}
