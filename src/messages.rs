//! Cross-module messages (Bevy 0.18 Message API).

use crate::content::enemies::EnemyKind;
use bevy::prelude::*;

/// Damage dealt to an enemy-side entity (enemies, bosses, pots). `amount` is final.
#[derive(Message)]
pub struct HitMsg {
    pub target: Entity,
    pub amount: f32,
    pub crit: bool,
    pub knock: Vec3,
}

/// Player took a hit (pre-mitigation).
#[derive(Message)]
pub struct PlayerHitMsg {
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
    pub elite: bool,
    pub xp: f32,
    pub is_boss: bool,
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
}

#[derive(Message)]
pub struct SfxMsg(pub Sfx);
