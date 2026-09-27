//! Cross-module messages (Bevy 0.18 Message API).

use crate::content::enemies::EnemyKind;
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
}

#[derive(Message)]
pub struct SfxMsg(pub Sfx);
