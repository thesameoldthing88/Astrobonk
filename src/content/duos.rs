//! Co-op set-pieces (GDD §11): the opposite-pole ultimate, the named duo combos, and the
//! rescue callout — what the results screen names a squad for.
//!
//! The GDD pairs each duo with two heroes; the pairing that carries over to the twelve suits
//! (locked direction #4) is the WEAPON FAMILY each hero brought, so a duo is "a teammate's
//! setup family, then your finisher family on the same foe". Where the built roster and the
//! GDD disagree the family wins: Deep Freeze's "Yuki's frost" is the frost family (Cryo Vent
//! / ABSOLUTE ZERO), since the built Yuki carries Kunai.

use super::weapons::WeaponKind;

/// One named co-op feat. Wire codes are explicit (`code`) — never renumber, only append.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoopFeat {
    /// Two storm-callers on opposite sides of the planet wrap it in a lightning belt.
    StaticCascade,
    /// Frost (A) chills the ranks, a beam (B) shatters them.
    DeepFreeze,
    /// A return weapon (A) herds the knot, a laser (B) melts it.
    MagnetCircus,
    /// Rivets (A) pin a foe down, the tank (B) finishes it with thorns or the wrench.
    RivetRescue,
    /// A teammate hauled back up from their Tumbling Beacon (A rescued B).
    Rescue,
}

/// How a duo's finisher is recognised (the setup side is always a weapon family).
#[derive(Clone, Copy, Debug)]
pub enum Finisher {
    Weapons(&'static [WeaponKind]),
    /// Thorns reflecting a hit, or one of these weapons.
    ThornsOr(&'static [WeaponKind]),
}

pub struct CoopFeatDef {
    pub name: &'static str,
    /// The results-screen line after the name.
    pub blurb: &'static str,
}

pub const FROST: &[WeaponKind] = &[WeaponKind::CryoVent, WeaponKind::AbsoluteZero];
pub const BEAMS: &[WeaponKind] = &[WeaponKind::MiningLaser, WeaponKind::DeathRay];
pub const HERDERS: &[WeaponKind] = &[WeaponKind::Boomerang, WeaponKind::SatelliteArray];
pub const LASERS: &[WeaponKind] = &[WeaponKind::LaserPistol, WeaponKind::GatlingLaser];
pub const RIVETS: &[WeaponKind] = &[WeaponKind::RivetGun, WeaponKind::Riveter9000];
pub const WRENCHES: &[WeaponKind] = &[WeaponKind::Wrench, WeaponKind::MegaWrench];
/// The storm-callers whose owners can link a STATIC CASCADE.
pub const STORM_CALLERS: &[WeaponKind] = &[WeaponKind::Tesla, WeaponKind::StormCore];

impl CoopFeat {
    /// The three hit-driven duos, in the order `duos::DuoLedger` keeps their marks.
    pub const DUOS: [CoopFeat; 3] = [CoopFeat::DeepFreeze, CoopFeat::MagnetCircus, CoopFeat::RivetRescue];
    pub const ALL: [CoopFeat; 5] =
        [CoopFeat::StaticCascade, CoopFeat::DeepFreeze, CoopFeat::MagnetCircus, CoopFeat::RivetRescue, CoopFeat::Rescue];

    pub fn def(&self) -> CoopFeatDef {
        match self {
            CoopFeat::StaticCascade => CoopFeatDef { name: "STATIC CASCADE", blurb: "the whole planet wrapped in lightning" },
            CoopFeat::DeepFreeze => CoopFeatDef { name: "DEEP FREEZE PROTOCOL", blurb: "chilled ranks shattered" },
            CoopFeat::MagnetCircus => CoopFeatDef { name: "MAGNET CIRCUS", blurb: "herded knots melted" },
            CoopFeat::RivetRescue => CoopFeatDef { name: "RIVET & RESCUE", blurb: "pinned foes finished" },
            CoopFeat::Rescue => CoopFeatDef { name: "HERO'S ADRENALINE", blurb: "hauled a teammate off their Beacon" },
        }
    }

    /// A duo's setup family: the teammate whose hit marks the foe.
    pub fn setup(&self) -> &'static [WeaponKind] {
        match self {
            CoopFeat::DeepFreeze => FROST,
            CoopFeat::MagnetCircus => HERDERS,
            CoopFeat::RivetRescue => RIVETS,
            _ => &[],
        }
    }

    pub fn finisher(&self) -> Finisher {
        match self {
            CoopFeat::DeepFreeze => Finisher::Weapons(BEAMS),
            CoopFeat::MagnetCircus => Finisher::Weapons(LASERS),
            CoopFeat::RivetRescue => Finisher::ThornsOr(WRENCHES),
            _ => Finisher::Weapons(&[]),
        }
    }

    /// The finisher's damage on a marked foe (Deep Freeze pays in its shatter instead).
    pub fn finisher_mult(&self) -> f32 {
        match self {
            CoopFeat::MagnetCircus => crate::config::DUO_MELT_MULT,
            CoopFeat::RivetRescue => crate::config::DUO_RIVET_MULT,
            _ => 1.0,
        }
    }

    pub fn code(&self) -> u8 {
        match self {
            CoopFeat::StaticCascade => 0,
            CoopFeat::DeepFreeze => 1,
            CoopFeat::MagnetCircus => 2,
            CoopFeat::RivetRescue => 3,
            CoopFeat::Rescue => 4,
        }
    }

    pub fn from_code(c: u8) -> Option<CoopFeat> {
        Self::ALL.into_iter().find(|f| f.code() == c)
    }
}
