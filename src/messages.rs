//! Cross-module messages (Bevy 0.18 Message API).

use crate::content::enemies::EnemyKind;
use crate::content::weapons::WeaponKind;
use bevy::prelude::*;

/// Damage dealt to an enemy-side entity (enemies, bosses, pots). `amount` is final.
#[derive(Message)]
pub struct HitMsg {
    /// The astronaut that dealt this hit; `None` = world/environment (e.g. comet cash-out).
    /// Makes cryo-slow and lifesteal resolve against the actual SHOOTER rather than
    /// whichever player a `.single()` happened to return.
    pub source: Option<Entity>,
    pub target: Entity,
    pub amount: f32,
    pub crit: bool,
    pub knock: Vec3,
    /// What dealt it: a weapon, a thorns reflection, or anything else (items, techs, the
    /// comet, set-pieces). A weapon's family drives the §11 duo combos (`duos::DuoLedger`),
    /// and what a hit DOES beyond damage keys off the weapon (`HitMsg::weapon`) — a
    /// Whoopee's stun, a bell's mark, a cryo vent's own slow, the §13 hitstop's evolved bonus.
    pub by: HitBy,
}

impl HitMsg {
    /// The weapon that dealt this hit, if a weapon did.
    pub fn weapon(&self) -> Option<WeaponKind> {
        match self.by {
            HitBy::Weapon(w) => Some(w),
            _ => None,
        }
    }
}

/// Attribution for a `HitMsg`, beyond the astronaut in `source`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HitBy {
    Weapon(WeaponKind),
    /// Thorns reflecting an enemy's hit back at it (`combat::apply_player_hits`).
    Thorns,
    /// Items, movement techs, the Comet Combo, co-op set-pieces, the world, dev/test kills.
    #[default]
    Other,
}

/// Player took a hit (pre-mitigation).
#[derive(Message)]
pub struct PlayerHitMsg {
    /// WHICH astronaut got hit. Required rather than Option: in co-op an unaddressed
    /// player hit has no sane fallback, and making it mandatory forces every writer to
    /// answer the question instead of silently damaging whoever `.single()` returned.
    pub victim: Entity,
    pub amount: f32,
    pub from: Vec3,
    pub attacker: Option<Entity>,
}

/// Something enemy-side died.
#[derive(Message)]
pub struct KillMsg {
    pub pos: Vec3,
    pub dir: Vec3,
    pub kind: Option<EnemyKind>,
    /// An elite of the horde, a miniboss or the stage boss (they share the elite loot).
    pub elite: bool,
    pub xp: f32,
    pub is_boss: bool,
    /// A miniboss (a `Boss` that is not the stage boss) — `elite` is set for it too.
    pub is_miniboss: bool,
    pub is_pot: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NumKind {
    Hit,
    Crit,
    Heal,
    Dodge,
    /// An Aegis Drone's shield took the hit (§9).
    Block,
}

/// Floating combat text request.
#[derive(Message)]
pub struct NumberMsg {
    pub pos: Vec3,
    pub amount: f32,
    pub kind: NumKind,
}

/// One-line announcement across the top of the HUD.
#[derive(Message)]
pub struct BannerMsg(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Hit,
    Crit,
    Hurt,
    Pickup,
    Coin,
    LevelUp,
    Chest,
    Evolve,
    BossRoar,
    Click,
    Pot,
    Teleport,
    Shrine,
    Slide,
    Bhop,
    Comet,
    // ---- §4 movement techs ----
    Slam,
    Grind,
    Blink,
    Flashlight,
    // ---- §8 world gimmicks ----
    Thorns,
    Spore,
    SporePop,
    // ---- §6 Tier-1 weapons ----
    Toll,
    Whoopee,
    Splat,
    Discharge,
    // ---- §9 new enemies (P08) ----
    /// A Sunskimmer commits to its dive.
    Whine,
    /// A chest turns out to be a Mimic.
    Chomp,
    /// A Beacon Tick plants its tracker.
    Tag,
}

#[derive(Message)]
pub struct SfxMsg(pub Sfx);
