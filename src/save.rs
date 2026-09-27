//! Persistent meta progression: silver, unlocks, tome levels, quest counters.

use crate::config;
use crate::content::characters::AstronautKind;
use crate::content::palettes::Palette;
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

/// How floating damage numbers are drawn (GDD §13 "Toggle: Full / Merged-only /
/// Crits-only / Off"). Heal and DODGE readouts are about the player, not damage dealt, so
/// only Off hides them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumberMode {
    /// Every hit, coalescing only per the §13 rule (0.3 m / 0.1 s) or when the screen is
    /// crowded.
    #[default]
    Full,
    /// Every nearby hit folds into a running sum — totals, never individual hits.
    Merged,
    CritsOnly,
    Off,
}

impl NumberMode {
    pub const ALL: [NumberMode; 4] = [NumberMode::Full, NumberMode::Merged, NumberMode::CritsOnly, NumberMode::Off];

    pub fn name(&self) -> &'static str {
        match self {
            NumberMode::Full => "FULL",
            NumberMode::Merged => "MERGED ONLY",
            NumberMode::CritsOnly => "CRITS ONLY",
            NumberMode::Off => "OFF",
        }
    }
}

/// Accessibility settings (GDD §13). Presentation only — each machine reads its own, so in
/// co-op every player sees the game their way; none of it touches the simulation.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Accessibility {
    /// Colorblind palette for the colors that carry meaning (danger, rarity).
    pub palette: Palette,
    /// "Danger = white outline": every telegraph, aim line and enemy shot gets a white hull.
    pub high_contrast: bool,
    /// Kills the evolution white-flash, clamps bloom, greys the hit-flash, dims particles and
    /// the hurt tint.
    pub flash_reduction: bool,
    /// Nothing strobes faster than 3/s: hit-flashes, death bursts and telegraph pulses are
    /// rate-limited; STORM CORE / Tesla zaps become dim slow fades and the DEATH RAY a dim
    /// see-through beam, under clamped bloom.
    pub photosensitive: bool,
    /// Bevy `UiScale`, `UI_SCALE_MIN..=UI_SCALE_MAX`.
    pub ui_scale: f32,
    pub numbers: NumberMode,
    /// Damage-number size multiplier, `NUMBER_SIZE_MIN..=NUMBER_SIZE_MAX`.
    pub number_size: f32,
}

impl Default for Accessibility {
    fn default() -> Self {
        Self {
            palette: Palette::Standard,
            high_contrast: false,
            flash_reduction: false,
            photosensitive: false,
            ui_scale: 1.0,
            numbers: NumberMode::Full,
            number_size: 1.0,
        }
    }
}

/// "Difficulty as options, not menus" (GDD §13): independent sliders layered on top of the
/// canon Difficulty/Cursed systems. They only ever EASE the run, and a run that used any of
/// them is flagged (`RunState::assisted`) — it still earns Silver, but its daily score is
/// kept apart from the unassisted board. In co-op the HOST's options govern the run.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct AssistOptions {
    /// Spawn-rate and live-cap multiplier, `ASSIST_DENSITY_MIN..=1`.
    pub enemy_density: f32,
    /// Multiplier on every hit an astronaut takes, `ASSIST_DAMAGE_MIN..=1`.
    pub enemy_damage: f32,
    /// "One more chance": each astronaut survives one would-be-lethal hit per run.
    pub revive_token: bool,
}

impl Default for AssistOptions {
    fn default() -> Self {
        Self { enemy_density: 1.0, enemy_damage: 1.0, revive_token: false }
    }
}

impl AssistOptions {
    /// Any option away from canon — the test that flags a run.
    pub fn is_assisted(&self) -> bool {
        self.enemy_density < 0.999 || self.enemy_damage < 0.999 || self.revive_token
    }

    /// Into range. The sliders cannot leave it, but a hand-edited save (or a future build's
    /// save opened by this one) can.
    pub fn clamped(self) -> Self {
        let fix = |v: f32, min: f32| if v.is_finite() { v.clamp(min, 1.0) } else { 1.0 };
        Self {
            enemy_density: fix(self.enemy_density, config::ASSIST_DENSITY_MIN),
            enemy_damage: fix(self.enemy_damage, config::ASSIST_DAMAGE_MIN),
            revive_token: self.revive_token,
        }
    }

    /// One line for the results screen and the HUD tag, e.g. "density 50% · damage 70%".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.enemy_density < 0.999 {
            parts.push(format!("enemy density {:.0}%", self.enemy_density * 100.0));
        }
        if self.enemy_damage < 0.999 {
            parts.push(format!("enemy damage {:.0}%", self.enemy_damage * 100.0));
        }
        if self.revive_token {
            parts.push("one more chance".to_string());
        }
        parts.join(" / ")
    }
}

/// Save-format generation `migrate()` brings every save up to. 1: tomes went from 20 levels
/// to 10 ranks (P05). Bump when a stored value's MEANING changes; a new field alone needs no
/// bump (`#[serde(default)]` covers it).
pub const SAVE_VERSION: u32 = 1;

#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
#[serde(default)] // missing fields (e.g. from older saves) fall back to Default — never wipe progress
pub struct MetaSave {
    /// See `SAVE_VERSION`. Field-level default (0) rather than the struct's: a save written
    /// before the field existed is the OLDEST format, not the current one.
    #[serde(default)]
    pub version: u32,
    pub silver: u64,
    /// Rank per tome, 0..=TOME_MAX_RANK.
    pub tome_levels: HashMap<TomeKind, u32>,
    pub tome_loadout: Vec<TomeKind>,
    /// Loadout slots: TOME_BASE_SLOTS plus quest rewards (P21 adds more of those).
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
    pub shake_scale: f32, // screenshake slider, 0..=1 (§13 "0–100%")
    pub accessibility: Accessibility,
    pub assist: AssistOptions,
    // --- daily seeded planet ---
    pub daily_day: u64,   // day-number of the last daily played
    pub daily_best: u64,  // best score on that day
    /// Best ASSISTED score on that day — kept apart so assists never touch the real board.
    pub daily_best_assisted: u64,
    pub tutorial_done: bool, // first-run onboarding seen
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
            version: SAVE_VERSION,
            silver: 0,
            tome_levels: HashMap::new(),
            // empty: a new player owns no tome, and the first rank of one slots it
            // (`buy_tome`) — never three unowned rank-0 tomes squatting in the slots
            tome_loadout: Vec::new(),
            tome_slots: config::TOME_BASE_SLOTS,
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
            accessibility: Accessibility::default(),
            assist: AssistOptions::default(),
            daily_day: 0,
            daily_best: 0,
            daily_best_assisted: 0,
            tutorial_done: false,
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
    pub(crate) fn migrate(&mut self) {
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
        // veterans (any prior run) skip the first-run tutorial
        if self.counters.runs_started > 0 {
            self.tutorial_done = true;
        }
        // The shake slider was 0–150% before §13 pinned it to 0–100% (100% is the canon
        // budget that keeps the camera inside its 1.8° clamp).
        self.shake_scale = if self.shake_scale.is_finite() { self.shake_scale.clamp(0.0, 1.0) } else { 1.0 };
        let a = &mut self.accessibility;
        a.ui_scale = if a.ui_scale.is_finite() { a.ui_scale.clamp(config::UI_SCALE_MIN, config::UI_SCALE_MAX) } else { 1.0 };
        a.number_size = if a.number_size.is_finite() {
            a.number_size.clamp(config::NUMBER_SIZE_MIN, config::NUMBER_SIZE_MAX)
        } else {
            1.0
        };
        self.assist = self.assist.clamped();
        self.migrate_tomes();
        self.version = SAVE_VERSION;
    }

    /// Tomes: 20 levels became 10 ranks worth two levels each (P05), so an old level L is
    /// rank ⌈L/2⌉ — rounded UP, so no tome ever loses power it was paid for. The loadout
    /// grew from 3 base slots to TOME_BASE_SLOTS; slots are re-derived from the quests that
    /// granted them, keeping any extra an older build handed out.
    fn migrate_tomes(&mut self) {
        if self.version < 1 {
            for rank in self.tome_levels.values_mut() {
                *rank = rank.div_ceil(2);
            }
        }
        for rank in self.tome_levels.values_mut() {
            *rank = (*rank).min(config::TOME_MAX_RANK);
        }
        let earned = self
            .quests_done
            .iter()
            .flat_map(|q| q.def().rewards.iter())
            .filter(|r| matches!(r, Reward::TomeSlot))
            .count() as u32;
        self.tome_slots = self.tome_slots.max(config::TOME_BASE_SLOTS + earned).min(config::TOME_SLOTS_MAX);
        // Old defaults slotted Damage/Health/XP before any was bought, and a hand-edited or
        // corrupt loadout can hold anything: keep only owned tomes, once each, within the
        // slots.
        let ranks = self.tome_levels.clone();
        let mut seen = HashSet::new();
        self.tome_loadout.retain(|t| ranks.get(t).is_some_and(|r| *r > 0) && seen.insert(*t));
        self.tome_loadout.truncate(self.tome_slots as usize);
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

    /// The rank a tome contributes to a run: its rank when slotted, 0 otherwise.
    pub fn tome_rank_equipped(&self, t: TomeKind) -> u32 {
        if self.tome_loadout.contains(&t) {
            self.tome_level(t)
        } else {
            0
        }
    }

    /// Buy the next rank of `t` if it has one and the Silver is there. True if bought. A
    /// first rank goes straight into a free slot: a tome bought and left on the shelf would
    /// read as Silver spent on nothing.
    pub fn buy_tome(&mut self, t: TomeKind) -> bool {
        let rank = self.tome_level(t);
        let cost = t.cost(rank);
        if rank >= config::TOME_MAX_RANK || self.silver < cost {
            return false;
        }
        self.silver -= cost;
        self.tome_levels.insert(t, rank + 1);
        if rank == 0 && !self.tome_loadout.contains(&t) && (self.tome_loadout.len() as u32) < self.tome_slots {
            self.tome_loadout.push(t);
        }
        true
    }

    /// Slot `t` into the loadout, or take it out. False when it would not fit.
    pub fn toggle_tome(&mut self, t: TomeKind) -> bool {
        if let Some(i) = self.tome_loadout.iter().position(|x| *x == t) {
            self.tome_loadout.remove(i);
            true
        } else if (self.tome_loadout.len() as u32) < self.tome_slots {
            self.tome_loadout.push(t);
            true
        } else {
            false
        }
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
            Reward::TomeSlot => self.tome_slots = (self.tome_slots + 1).min(config::TOME_SLOTS_MAX),
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

/// Headless self-check: a save written before the settings existed must load with every
/// new setting at its default (nothing wiped), and the out-of-range values an old build or
/// a hand edit can leave behind must come back in range.
pub fn settings_self_check() -> Result<(), String> {
    let legacy = r#"{"silver": 321, "shake_scale": 1.5, "volume": 0.4}"#;
    let mut s: MetaSave = serde_json::from_str(legacy).map_err(|e| format!("legacy save rejected: {e}"))?;
    s.migrate();
    if s.silver != 321 || (s.volume - 0.4).abs() > 1e-6 {
        return Err("a legacy save lost its progress/settings".into());
    }
    if s.accessibility != Accessibility::default() || s.assist != AssistOptions::default() || s.daily_best_assisted != 0 {
        return Err("new settings did not default on a legacy save".into());
    }
    if s.shake_scale != 1.0 {
        return Err(format!("legacy 150% shake should clamp to 100%, got {}", s.shake_scale));
    }
    let wild = r#"{"accessibility": {"ui_scale": 9.0, "number_size": 0.0, "palette": "Tritanopia"},
                   "assist": {"enemy_density": -3.0, "enemy_damage": 4.0}}"#;
    let mut w: MetaSave = serde_json::from_str(wild).map_err(|e| format!("partial settings rejected: {e}"))?;
    w.migrate();
    let a = &w.accessibility;
    if a.ui_scale != config::UI_SCALE_MAX || a.number_size != config::NUMBER_SIZE_MIN || a.palette != Palette::Tritanopia {
        return Err(format!("accessibility not clamped: {a:?}"));
    }
    if w.assist.enemy_density != config::ASSIST_DENSITY_MIN || w.assist.enemy_damage != 1.0 || w.assist.revive_token {
        return Err(format!("assists not clamped: {:?}", w.assist));
    }
    if !w.assist.is_assisted() || AssistOptions::default().is_assisted() {
        return Err("is_assisted() disagrees with the options".into());
    }
    // and a full round trip keeps every field
    let mut r = MetaSave::default();
    r.accessibility.palette = Palette::Deuteranopia;
    r.accessibility.numbers = NumberMode::CritsOnly;
    r.assist.revive_token = true;
    let back: MetaSave = serde_json::from_str(&serde_json::to_string(&r).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if back.accessibility != r.accessibility || back.assist != r.assist {
        return Err("settings did not survive a save/load round trip".into());
    }
    Ok(())
}
