use bevy::prelude::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Shambler,
    Sprinter,
    Bruiser,
    Spitter,
    Ufo,
    Burrower,
    Beamer, // long-range railbolt sniper
    Lobber, // mortar artillery with AoE telegraph
    Ghost,  // The Static
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BossKind {
    CraterpillarJr, // miniboss
    RoverGoneWrong, // miniboss
    Craterpillar,   // Moon stage boss
    Anubot,         // Mars stage boss
}

pub struct EnemyDef {
    pub kind: EnemyKind,
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub xp: f32,
    pub scale: f32,
    pub color: Color,
    /// Hover height above terrain (0 = walks).
    pub hover: f32,
    /// Preferred combat range in meters (0 = melee, beelines forever).
    pub standoff: f32,
}

impl EnemyKind {
    pub fn def(&self) -> EnemyDef {
        use EnemyKind::*;
        match self {
            Shambler => EnemyDef {
                kind: *self,
                name: "Shambler",
                hp: 14.0,
                speed: 3.1,
                damage: 5.0,
                xp: 1.0,
                scale: 1.0,
                color: Color::srgb(0.45, 0.75, 0.45),
                hover: 0.0,
                standoff: 0.0,
            },
            Sprinter => EnemyDef {
                kind: *self,
                name: "Sprinter",
                hp: 9.0,
                speed: 6.2,
                damage: 5.0,
                xp: 1.2,
                scale: 0.8,
                color: Color::srgb(0.85, 0.85, 0.30),
                hover: 0.0,
                standoff: 0.0,
            },
            Bruiser => EnemyDef {
                kind: *self,
                name: "Bruiser",
                hp: 70.0,
                speed: 2.2,
                damage: 14.0,
                xp: 4.0,
                scale: 1.7,
                color: Color::srgb(0.70, 0.35, 0.30),
                hover: 0.0,
                standoff: 0.0,
            },
            Spitter => EnemyDef {
                kind: *self,
                name: "Spitter",
                hp: 18.0,
                speed: 2.6,
                damage: 8.0,
                xp: 2.0,
                scale: 1.1,
                color: Color::srgb(0.60, 0.45, 0.85),
                hover: 0.0,
                standoff: 13.0,
            },
            Ufo => EnemyDef {
                kind: *self,
                name: "UFO",
                hp: 24.0,
                speed: 4.5,
                damage: 7.0,
                xp: 2.5,
                scale: 1.0,
                color: Color::srgb(0.55, 0.85, 0.95),
                hover: 4.0,
                standoff: 9.0,
            },
            Burrower => EnemyDef {
                kind: *self,
                name: "Burrower",
                hp: 30.0,
                speed: 5.0,
                damage: 12.0,
                xp: 3.0,
                scale: 1.2,
                color: Color::srgb(0.75, 0.55, 0.35),
                hover: 0.0,
                standoff: 0.0,
            },
            Beamer => EnemyDef {
                kind: *self,
                name: "Beamer",
                hp: 26.0,
                speed: 2.8,
                damage: 14.0,
                xp: 3.0,
                scale: 1.3,
                color: Color::srgb(1.0, 0.30, 0.55),
                hover: 0.0,
                standoff: 22.0,
            },
            Lobber => EnemyDef {
                kind: *self,
                name: "Lobber",
                hp: 45.0,
                speed: 1.8,
                damage: 16.0,
                xp: 3.5,
                scale: 1.5,
                color: Color::srgb(0.55, 0.60, 0.30),
                hover: 0.0,
                standoff: 17.0,
            },
            Ghost => EnemyDef {
                kind: *self,
                name: "The Static",
                hp: 20.0,
                speed: 5.4,
                damage: 10.0,
                xp: 0.0, // drops silver instead
                scale: 1.1,
                color: Color::srgb(0.80, 0.85, 1.00),
                hover: 0.6,
                standoff: 0.0,
            },
        }
    }

    /// Which kinds are in the spawn mix at a given elapsed-seconds mark.
    pub fn mix(elapsed: f32) -> &'static [EnemyKind] {
        use EnemyKind::*;
        match elapsed as u32 {
            0..=89 => &[Shambler],
            90..=179 => &[Shambler, Sprinter],
            180..=269 => &[Shambler, Sprinter, Spitter],
            270..=359 => &[Shambler, Sprinter, Spitter, Bruiser],
            360..=449 => &[Shambler, Sprinter, Spitter, Bruiser, Ufo, Beamer],
            450..=539 => &[Shambler, Sprinter, Spitter, Bruiser, Ufo, Beamer, Burrower],
            _ => &[Shambler, Sprinter, Spitter, Bruiser, Ufo, Beamer, Burrower, Lobber],
        }
    }
}

pub struct BossDef {
    pub kind: BossKind,
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub scale: f32,
    pub color: Color,
    pub is_stage_boss: bool,
}

impl BossKind {
    pub fn def(&self) -> BossDef {
        use BossKind::*;
        match self {
            CraterpillarJr => BossDef {
                kind: *self,
                name: "Craterpillar Jr",
                hp: 700.0,
                speed: 3.4,
                damage: 16.0,
                scale: 2.6,
                color: Color::srgb(0.55, 0.65, 0.75),
                is_stage_boss: false,
            },
            RoverGoneWrong => BossDef {
                kind: *self,
                name: "Rover Gone Wrong",
                hp: 1100.0,
                speed: 5.2,
                damage: 20.0,
                scale: 2.2,
                color: Color::srgb(0.85, 0.75, 0.55),
                is_stage_boss: false,
            },
            Craterpillar => BossDef {
                kind: *self,
                name: "THE CRATERPILLAR",
                hp: 5200.0,
                speed: 3.0,
                damage: 26.0,
                scale: 4.6,
                color: Color::srgb(0.60, 0.60, 0.70),
                is_stage_boss: true,
            },
            Anubot => BossDef {
                kind: *self,
                name: "JUDGE ANUBOT",
                hp: 8200.0,
                speed: 3.6,
                damage: 32.0,
                scale: 4.2,
                color: Color::srgb(0.80, 0.60, 0.20),
                is_stage_boss: true,
            },
        }
    }
}

/// Elite modifier applied to a base enemy: bigger, tougher, shinier, generous.
pub struct EliteMods;
impl EliteMods {
    pub const HP: f32 = 8.0;
    pub const DMG: f32 = 1.8;
    pub const SCALE: f32 = 1.65;
    pub const XP: f32 = 8.0;
}

/// Global time scaling of enemy stats (the swarm gets meaner as the clock runs).
pub fn time_scaling(elapsed: f32, difficulty: f32) -> (f32, f32) {
    let t = elapsed / 60.0;
    let hp_mult = (1.0 + 0.22 * t * t.sqrt().max(1.0) * 0.5 + 0.35 * t) * (1.0 + difficulty);
    let dmg_mult = (1.0 + 0.12 * t) * (1.0 + difficulty * 0.5);
    (hp_mult, dmg_mult)
}
