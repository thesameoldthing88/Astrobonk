use super::characters::SuitKind;
use super::planets::PlanetKind;
use super::weapons::WeaponKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestKind {
    Kill100,
    Kill1000,
    Kill2500,
    Kill10000,
    Pots50,
    Chests10,
    Shrines5,
    Gold5000,
    Level20,
    EvolveWeapon,
    FreeChimp,
    SurviveStatic2Min,
    ClearMoonT1,
    ClearMoonT2,
    ClearMoonT3,
    ClearMarsT1,
}

#[derive(Clone, Copy, Debug)]
pub enum Reward {
    Silver(u64),
    /// A suit joins the wardrobe (quests unlock suits, not heroes: locked direction #4).
    UnlockSuit(SuitKind),
    UnlockWeapon(WeaponKind),
    UnlockPlanet(PlanetKind),
    TomeSlot,
}

impl Reward {
    /// What the reward is, for the quest log and the results screen.
    pub fn label(&self) -> String {
        match self {
            Reward::Silver(s) => format!("+{s} silver"),
            Reward::UnlockSuit(k) => format!("{} suit", k.def().name),
            Reward::UnlockWeapon(w) => w.def().name.to_uppercase(),
            Reward::UnlockPlanet(p) => format!("world: {}", p.def().name),
            Reward::TomeSlot => "+1 tome slot".into(),
        }
    }
}

pub struct QuestDef {
    pub kind: QuestKind,
    pub name: &'static str,
    pub desc: &'static str,
    pub rewards: &'static [Reward],
}

impl QuestKind {
    pub const ALL: [QuestKind; 16] = [
        QuestKind::Kill100,
        QuestKind::Kill1000,
        QuestKind::Kill2500,
        QuestKind::Kill10000,
        QuestKind::Pots50,
        QuestKind::Chests10,
        QuestKind::Shrines5,
        QuestKind::Gold5000,
        QuestKind::Level20,
        QuestKind::EvolveWeapon,
        QuestKind::FreeChimp,
        QuestKind::SurviveStatic2Min,
        QuestKind::ClearMoonT1,
        QuestKind::ClearMoonT2,
        QuestKind::ClearMoonT3,
        QuestKind::ClearMarsT1,
    ];

    pub fn def(&self) -> QuestDef {
        use QuestKind::*;
        use Reward::*;
        match self {
            Kill100 => QuestDef { kind: *self, name: "First Contact", desc: "Bonk 100 invaders", rewards: &[Silver(20)] },
            Kill1000 => QuestDef { kind: *self, name: "Pest Control", desc: "Bonk 1,000 invaders", rewards: &[Silver(60), UnlockWeapon(WeaponKind::Tesla)] },
            Kill2500 => QuestDef { kind: *self, name: "Exterminator", desc: "Bonk 2,500 invaders", rewards: &[Silver(90), UnlockSuit(SuitKind::Doug)] },
            Kill10000 => QuestDef { kind: *self, name: "Solar Defender", desc: "Bonk 10,000 invaders", rewards: &[Silver(250), TomeSlot] },
            Pots50 => QuestDef { kind: *self, name: "Pottery Critic", desc: "Break 50 pots", rewards: &[Silver(40), UnlockWeapon(WeaponKind::Drones)] },
            Chests10 => QuestDef { kind: *self, name: "Cache Money", desc: "Open 10 chests", rewards: &[Silver(50), UnlockWeapon(WeaponKind::RocketPod)] },
            Shrines5 => QuestDef { kind: *self, name: "Devout", desc: "Charge 5 shrines", rewards: &[Silver(50), UnlockWeapon(WeaponKind::CryoVent)] },
            Gold5000 => QuestDef { kind: *self, name: "Space Capitalist", desc: "Collect 5,000 gold (lifetime)", rewards: &[Silver(80)] },
            Level20 => QuestDef { kind: *self, name: "Overachiever", desc: "Reach level 20 in one run", rewards: &[Silver(60)] },
            EvolveWeapon => QuestDef { kind: *self, name: "Ascension", desc: "Evolve any weapon", rewards: &[Silver(100)] },
            FreeChimp => QuestDef { kind: *self, name: "Cold Case", desc: "Open the cage on the Moon", rewards: &[Silver(40), UnlockSuit(SuitKind::ChimpO)] },
            SurviveStatic2Min => QuestDef { kind: *self, name: "Signal In The Noise", desc: "Survive THE STATIC for 2:00", rewards: &[Silver(150), TomeSlot] },
            ClearMoonT1 => QuestDef { kind: *self, name: "One Small Bonk", desc: "Clear MOON Tier 1", rewards: &[Silver(50), UnlockSuit(SuitKind::B0nk)] },
            ClearMoonT2 => QuestDef { kind: *self, name: "One Giant Bonk", desc: "Clear MOON Tier 2", rewards: &[Silver(120), UnlockPlanet(PlanetKind::Mars)] },
            ClearMoonT3 => QuestDef { kind: *self, name: "The Full Tour", desc: "Clear MOON Tier 3", rewards: &[Silver(300)] },
            ClearMarsT1 => QuestDef { kind: *self, name: "Red Planet Standing", desc: "Clear MARS Tier 1", rewards: &[Silver(120), UnlockSuit(SuitKind::Yuki)] },
        }
    }
}
