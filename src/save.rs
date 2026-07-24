//! Persistent meta progression: silver, unlocks, tome levels, quest counters.

use crate::config;
use crate::content::characters::AstronautKind;
use crate::content::planets::PlanetKind;
use crate::content::quests::{QuestKind, Reward};
use crate::content::tomes::TomeKind;
use crate::content::weapons::WeaponKind;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Resource, Serialize, Deserialize, Clone, Debug, Default)]
pub struct Counters {
    pub kills: u64,
    pub pots: u64,
    pub chests: u64,
    pub shrines: u64,
    pub gold: u64,
    pub evolves: u64,
    pub best_level: u32,
    pub static_secs_best: f32,
    pub runs_started: u64,
    pub runs_won: u64,
    pub chimp_freed: bool,
    /// (planet, tier) cleared flags, e.g. ("Moon", 2)
    pub cleared: HashSet<(PlanetKind, u32)>,
}

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
#[serde(default)] // missing fields (e.g. from older saves) fall back to Default — never wipe progress
pub struct MetaSave {
    pub silver: u64,
    pub tome_levels: HashMap<TomeKind, u32>,
    pub tome_loadout: Vec<TomeKind>,
    pub tome_slots: u32,
    pub unlocked_chars: HashSet<AstronautKind>,
    pub unlocked_weapons: HashSet<WeaponKind>,
    pub unlocked_planets: HashSet<PlanetKind>,
    pub quests_done: HashSet<QuestKind>,
    pub counters: Counters,
    pub volume: f32,
    // --- settings ---
    pub music_volume: f32,
    pub sfx_volume: f32,
    pub sensitivity: f32, // camera-sensitivity multiplier
    pub shake_scale: f32, // screenshake intensity multiplier
}

impl Default for MetaSave {
    fn default() -> Self {
        let mut unlocked_chars = HashSet::new();
        for c in AstronautKind::ALL {
            if c.starts_unlocked() {
                unlocked_chars.insert(c);
            }
        }
        let mut unlocked_weapons = HashSet::new();
        // Signature weapons of the starting roster + a couple of drops.
        for w in [
            WeaponKind::Wrench,
            WeaponKind::LaserPistol,
            WeaponKind::RivetGun,
            WeaponKind::Kunai,
            WeaponKind::Boomerang,
            WeaponKind::MiningLaser,
            // Batch-1 weapons available in the level-up pool from the start.
            WeaponKind::MeatballComet,
            WeaponKind::StaticCling,
            WeaponKind::RicochetDisc,
            WeaponKind::SonicWhoopee,
            WeaponKind::CosmonautsBell,
            WeaponKind::YoYo,
        ] {
            unlocked_weapons.insert(w);
        }
        let mut unlocked_planets = HashSet::new();
        unlocked_planets.insert(PlanetKind::Moon);
        unlocked_planets.insert(PlanetKind::Mars); // dev: Mars selectable for playtesting
        Self {
            silver: 0,
            tome_levels: HashMap::new(),
            tome_loadout: vec![TomeKind::Damage, TomeKind::Health, TomeKind::Xp],
            tome_slots: 3,
            unlocked_chars,
            unlocked_weapons,
            unlocked_planets,
            quests_done: HashSet::new(),
            counters: Counters::default(),
            volume: 0.7,
            music_volume: 1.0,
            sfx_volume: 1.0,
            sensitivity: 1.0,
            shake_scale: 1.0,
        }
    }
}

fn save_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join(config::SAVE_DIR).join(config::SAVE_FILE)
}

impl MetaSave {
    pub fn load() -> Self {
        let path = save_path();
        let mut s = match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("save corrupt ({e}), starting fresh");
                Self::default()
            }),
            Err(_) => Self::default(),
        };
        s.migrate();
        s
    }

    /// Fold in content that ships unlocked-by-default so existing saves gain
    /// newly added starter heroes/weapons without wiping progress.
    fn migrate(&mut self) {
        let fresh = Self::default();
        for c in &fresh.unlocked_chars {
            self.unlocked_chars.insert(*c);
        }
        for w in &fresh.unlocked_weapons {
            self.unlocked_weapons.insert(*w);
        }
        for p in &fresh.unlocked_planets {
            self.unlocked_planets.insert(*p);
        }
    }

    pub fn save(&self) {
        let path = save_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(s) => {
                if let Err(e) = std::fs::write(&path, s) {
                    warn!("could not write save: {e}");
                }
            }
            Err(e) => warn!("could not serialize save: {e}"),
        }
    }

    pub fn tome_level(&self, t: TomeKind) -> u32 {
        *self.tome_levels.get(&t).unwrap_or(&0)
    }

    /// Check all quests against counters; apply rewards for newly completed ones.
    /// Returns the newly completed quests (for banners).
    pub fn check_quests(&mut self) -> Vec<QuestKind> {
        let mut newly = Vec::new();
        for q in QuestKind::ALL {
            if self.quests_done.contains(&q) {
                continue;
            }
            if self.quest_met(q) {
                self.quests_done.insert(q);
                for r in q.def().rewards {
                    self.apply_reward(*r);
                }
                newly.push(q);
            }
        }
        newly
    }

    fn quest_met(&self, q: QuestKind) -> bool {
        use QuestKind::*;
        let c = &self.counters;
        match q {
            Kill100 => c.kills >= 100,
            Kill1000 => c.kills >= 1000,
            Kill2500 => c.kills >= 2500,
            Kill10000 => c.kills >= 10000,
            Pots50 => c.pots >= 50,
            Chests10 => c.chests >= 10,
            Shrines5 => c.shrines >= 5,
            Gold5000 => c.gold >= 5000,
            Level20 => c.best_level >= 20,
            EvolveWeapon => c.evolves >= 1,
            FreeChimp => c.chimp_freed,
            SurviveStatic2Min => c.static_secs_best >= 120.0,
            ClearMoonT1 => c.cleared.contains(&(PlanetKind::Moon, 1)),
            ClearMoonT2 => c.cleared.contains(&(PlanetKind::Moon, 2)),
            ClearMoonT3 => c.cleared.contains(&(PlanetKind::Moon, 3)),
            ClearMarsT1 => c.cleared.contains(&(PlanetKind::Mars, 1)),
        }
    }

    fn apply_reward(&mut self, r: Reward) {
        match r {
            Reward::Silver(s) => self.silver += s,
            Reward::UnlockChar(c) => {
                self.unlocked_chars.insert(c);
            }
            Reward::UnlockWeapon(w) => {
                self.unlocked_weapons.insert(w);
            }
            Reward::UnlockPlanet(p) => {
                self.unlocked_planets.insert(p);
            }
            Reward::TomeSlot => self.tome_slots = (self.tome_slots + 1).min(5),
        }
    }

    /// Quest progress line for the UI, e.g. "3,204 / 10,000".
    pub fn quest_progress(&self, q: QuestKind) -> Option<(u64, u64)> {
        use QuestKind::*;
        let c = &self.counters;
        Some(match q {
            Kill100 => (c.kills.min(100), 100),
            Kill1000 => (c.kills.min(1000), 1000),
            Kill2500 => (c.kills.min(2500), 2500),
            Kill10000 => (c.kills.min(10000), 10000),
            Pots50 => (c.pots.min(50), 50),
            Chests10 => (c.chests.min(10), 10),
            Shrines5 => (c.shrines.min(5), 5),
            Gold5000 => (c.gold.min(5000), 5000),
            _ => return None,
        })
    }
}
