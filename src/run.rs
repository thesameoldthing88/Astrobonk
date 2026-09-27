//! Run-scoped state: the live character sheet, level-up logic, upgrade rolls.
//! Pure data + logic here; the systems that drive a run live in `director.rs`.

use crate::config;
use crate::content::characters::{AstronautKind, Passive};
use crate::content::items::ItemKind;
use crate::content::planets::PlanetKind;
use crate::content::weapons::WeaponKind;
use crate::content::Rarity;
use crate::save::MetaSave;
use crate::stats::{StatKind, Stats};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;

/// The run's deterministic random source. Seeded from `RunState::run_seed` at stage entry
/// so the world layout + spawn stream are reproducible — the foundation for the daily
/// seeded planet and, later, co-op determinism. Only the (chained) sim systems draw from
/// it, so draw order is deterministic.
#[derive(Resource)]
pub struct GameRng(pub StdRng);

impl Default for GameRng {
    fn default() -> Self {
        Self(StdRng::seed_from_u64(0))
    }
}

impl GameRng {
    pub fn reseed(&mut self, seed: u64) {
        self.0 = StdRng::seed_from_u64(seed);
    }
}

/// A fresh non-reproducible seed for normal play (daily/testing override this).
pub fn fresh_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15)
}

/// Days since the Unix epoch (UTC) — the "day number" the daily is keyed on.
pub fn today() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0)
}

/// A stable seed for a given day — everyone who plays the daily gets the same world.
pub fn daily_seed(day: u64) -> u64 {
    // splitmix64 avalanche so consecutive days feel unrelated
    let mut z = day.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// A procedural code-name for a daily world, e.g. "GRIEF-7B".
pub fn daily_name(seed: u64) -> String {
    const WORDS: [&str; 8] = ["GRIEF", "HUSH", "EMBER", "VIGIL", "DROSS", "WANE", "SILT", "PALL"];
    format!("{}-{:X}{:X}", WORDS[(seed % 8) as usize], (seed >> 8) % 16, (seed >> 3) % 16)
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunPhase {
    #[default]
    Playing,
    LevelUp,
    /// A non-levelup choice panel is open (chest reveal, shrine loot, shady guy...).
    Modal,
    Paused,
    Dead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerupKind {
    Damage2x,
    Magnet,
    Speed,
}

#[derive(Clone, Debug)]
pub struct WeaponInstance {
    pub kind: WeaponKind,
    pub level: u32,
    pub cd: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunResult {
    Victory,
    Death,
    /// Quit from the pause menu. Banks like a death; told apart so a co-op host can tell
    /// its squad WHY the run is over (a wipe and "the host left" read very differently).
    Abandoned,
}

/// Run-GLOBAL state: the clock, the world chain, boss flags, shared counters.
/// Everything here is shared by every player in the run (co-op ready).
#[derive(Resource, Clone, Debug)]
pub struct RunState {
    pub tier: u32,
    pub chain: Vec<PlanetKind>,
    pub stage: usize,
    pub timer: f32,
    pub elapsed: f32,
    pub total_elapsed: f32,
    pub silver_run: u64,
    pub kills: u64,
    pub greed_stacks: u32,
    pub boss_spawned: bool,
    pub boss_dead: bool,
    pub minibosses_spawned: [bool; 2],
    pub static_active: bool,
    pub static_timer: f32,
    pub teleporter_open: bool,
    pub run_seed: u64,       // deterministic seed for world gen + spawns
    pub is_daily: bool,      // this run is the daily seeded challenge
    pub microwave_used: bool,
    pub chest_opens: u32,
    pub shrines_charged: u64,
    pub pots_broken: u64,
    pub chests_opened: u64,
    pub gold_collected: u64,
    pub evolves: u64,
    pub result: Option<RunResult>,
    /// Aggregated difficulty from all players (Cursed items/tomes). World-level in co-op.
    pub difficulty: f32,
    /// Character of the local/primary player — kept so menus and results screens
    /// have something to show without querying the world.
    pub character: AstronautKind,
}

/// PER-PLAYER state, attached as a component to each astronaut entity. In co-op every
/// player owns their own HP, build, level, and gold (the GDD model); XP is granted to
/// all players when anyone collects a gem.
#[derive(Component, Clone, Debug)]
pub struct PlayerState {
    pub character: AstronautKind,
    pub xp: f32,
    pub level: u32,
    pub xp_needed: f32,
    pub pending_levelups: u32,
    pub gold: u64,
    pub weapons: Vec<WeaponInstance>,
    pub items: Vec<(ItemKind, u32)>,
    pub stats: Stats,
    pub hp: f32,
    pub shield: f32,
    pub shield_cd: f32,
    pub iframes: f32,
    pub banishes: u32,
    pub refreshes: u32,
    pub banned_items: HashSet<ItemKind>,
    pub frenzy_timer: f32,
    pub fast_move: bool,     // above base run speed (Nova / Aurora passives)
    pub reticle_timer: f32,  // cycles 0..1.5 for Reticle's focus pulse
    pub powerups: Vec<(PowerupKind, f32)>,
    pub dead: bool,
}

impl RunState {
    pub fn new(character: AstronautKind, start: PlanetKind, tier: u32, _save: &MetaSave) -> Self {
        Self {
            tier,
            chain: PlanetKind::chain_from(start, tier),
            stage: 0,
            timer: config::STAGE_SECONDS[0],
            elapsed: 0.0,
            total_elapsed: 0.0,
            silver_run: 0,
            kills: 0,
            greed_stacks: 0,
            boss_spawned: false,
            boss_dead: false,
            minibosses_spawned: [false; 2],
            static_active: false,
            static_timer: 0.0,
            teleporter_open: false,
            run_seed: fresh_seed(),
            is_daily: false,
            microwave_used: false,
            chest_opens: 0,
            shrines_charged: 0,
            pots_broken: 0,
            chests_opened: 0,
            gold_collected: 0,
            evolves: 0,
            result: None,
            difficulty: 0.0,
            character,
        }
    }

    pub fn planet(&self) -> PlanetKind {
        self.chain[self.stage.min(self.chain.len() - 1)]
    }
}

impl PlayerState {
    pub fn new(character: AstronautKind, save: &MetaSave) -> Self {
        let mut s = Self {
            character,
            xp: 0.0,
            level: 1,
            xp_needed: xp_needed(1),
            pending_levelups: 0,
            gold: 0,
            weapons: vec![WeaponInstance { kind: character.def().weapon, level: 1, cd: 0.0 }],
            items: Vec::new(),
            stats: Stats::default(),
            hp: 0.0,
            shield: 0.0,
            shield_cd: 0.0,
            iframes: 0.0,
            banishes: 3,
            refreshes: 2,
            banned_items: HashSet::new(),
            frenzy_timer: 0.0,
            fast_move: false,
            reticle_timer: 0.0,
            powerups: Vec::new(),
            dead: false,
        };
        s.recompute_stats(save, 0);
        s.hp = s.stats.max_hp;
        s.shield = s.stats.shield;
        // Lady Fortuna gambles harder — extra level-up rerolls.
        if character == AstronautKind::Fortuna {
            s.refreshes += 2;
        }
        s
    }

    pub fn recompute_stats(&mut self, save: &MetaSave, greed_stacks: u32) {
        let mut st = Stats::default();
        // Tomes (meta loadout)
        for t in &save.tome_loadout {
            let lvl = save.tome_level(*t);
            if lvl > 0 {
                let d = t.def();
                st.apply(d.stat, d.per_level * lvl as f32);
            }
        }
        // Character passive
        match self.character.def().passive {
            Passive::DamageMult(v) => st.apply(StatKind::Damage, v),
            Passive::AttackSpeed(v) => st.apply(StatKind::AttackSpeed, v),
            Passive::CritPerLevel(v) => st.apply(StatKind::CritChance, v * self.level as f32),
            Passive::ExtraJumps(n) => st.apply(StatKind::ExtraJumps, n as f32),
            Passive::GoldGain(v) => st.apply(StatKind::GoldGain, v),
            Passive::SlideFrenzy { .. } => {}
            Passive::CritChance(v) => st.apply(StatKind::CritChance, v),
            Passive::MoveSpeed(v) => st.apply(StatKind::MoveSpeed, v),
            Passive::MaxHp(v) => st.apply(StatKind::MaxHp, v),
            Passive::Luck(v) => st.apply(StatKind::Luck, v),
            Passive::Size(v) => st.apply(StatKind::Size, v),
        }
        // Items
        for (item, count) in &self.items {
            for (k, v) in item.def().boosts {
                st.apply(*k, v * *count as f32);
            }
        }
        // Greed shrines (run-global, passed in)
        st.apply(StatKind::Difficulty, 0.12 * greed_stacks as f32);
        st.apply(StatKind::Luck, 0.08 * greed_stacks as f32);
        // Level-ups grant +1 max hp each (genre staple)
        st.apply(StatKind::MaxHp, (self.level - 1) as f32);

        let hp_frac = if self.stats.max_hp > 0.0 { self.hp / self.stats.max_hp } else { 1.0 };
        self.stats = st;
        self.hp = (self.stats.max_hp * hp_frac).clamp(0.0, self.stats.max_hp);
    }

    /// Effective attack speed including Yuki frenzy, powerups, and hero mechanic-passives.
    pub fn attack_speed(&self) -> f32 {
        let mut a = self.stats.attack_speed;
        if self.frenzy_timer > 0.0 {
            if let Passive::SlideFrenzy { bonus, .. } = self.character.def().passive {
                a += bonus;
            }
        }
        // Slipstream Nova: weapons barely cool down while she's sprinting.
        if self.character == AstronautKind::Nova && self.fast_move {
            a += 1.3;
        }
        a.max(0.1)
    }

    pub fn damage_mult(&self) -> f32 {
        let mut d = self.stats.damage;
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Damage2x) {
            d *= 2.0;
        }
        // Sgt. Gristle: cornered and furious — big damage below half HP.
        if self.character == AstronautKind::Gristle && self.hp < self.stats.max_hp * 0.5 {
            d *= 1.4;
        }
        d
    }

    /// Effective crit chance — Dr. Reticle's focus pulse guarantees a crit briefly each cycle.
    pub fn crit_chance(&self) -> f32 {
        let mut c = self.stats.crit_chance;
        if self.character == AstronautKind::Reticle && self.reticle_timer < 0.35 {
            c += 1.0;
        }
        c
    }

    /// Aura-radius multiplier — Aurora Prime's fields swell while she's sprinting.
    pub fn aura_scale(&self) -> f32 {
        if self.character == AstronautKind::Aurora && self.fast_move {
            1.35
        } else {
            1.0
        }
    }

    /// Armor after hero mechanics — Old Ironclad's plating doubles when badly hurt.
    pub fn effective_armor_fraction(&self) -> f32 {
        let mut armor = self.stats.armor.max(0.0);
        if self.character == AstronautKind::Ironclad && self.hp < self.stats.max_hp * 0.3 {
            armor *= 2.0;
        }
        armor / (armor + 100.0)
    }

    pub fn move_speed_mult(&self) -> f32 {
        let mut m = self.stats.move_speed;
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Speed) {
            m *= 1.5;
        }
        m
    }

    pub fn pickup_range(&self) -> f32 {
        let base = config::PICKUP_BASE_RANGE * self.stats.pickup_range;
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Magnet) {
            base * 40.0
        } else {
            base
        }
    }

    pub fn gain_xp(&mut self, v: f32) {
        self.xp += v * self.stats.xp_gain;
        while self.xp >= self.xp_needed {
            self.xp -= self.xp_needed;
            self.level += 1;
            self.pending_levelups += 1;
            self.xp_needed = xp_needed(self.level);
        }
    }

    pub fn weapon_mut(&mut self, kind: WeaponKind) -> Option<&mut WeaponInstance> {
        self.weapons.iter_mut().find(|w| w.kind == kind)
    }

    pub fn has_item(&self, kind: ItemKind) -> bool {
        self.items.iter().any(|(k, _)| *k == kind)
    }

    pub fn item_count(&self, kind: ItemKind) -> u32 {
        self.items.iter().find(|(k, _)| *k == kind).map(|(_, c)| *c).unwrap_or(0)
    }

    /// Weapons ready to evolve: at max level and paired item owned.
    pub fn evolvable(&self) -> Vec<WeaponKind> {
        self.weapons
            .iter()
            .filter(|w| w.level >= config::MAX_WEAPON_LEVEL)
            .filter_map(|w| {
                let def = w.kind.def();
                match (def.evolves_to, def.evo_item) {
                    (Some(evo), Some(item)) if self.has_item(item) => Some((w.kind, evo)),
                    _ => None,
                }
            })
            .map(|(base, _)| base)
            .collect()
    }
}

pub fn xp_needed(level: u32) -> f32 {
    let l = level as f32;
    config::XP_BASE + config::XP_PER_LEVEL * (l - 1.0) + config::XP_QUAD * (l - 1.0) * (l - 1.0)
}

// ---------------------------------------------------------------- upgrades

#[derive(Clone, Debug, PartialEq)]
pub enum UpgradeOption {
    NewWeapon(WeaponKind),
    WeaponUp(WeaponKind),
    Evolve(WeaponKind),
    NewItem(ItemKind),
    ItemUp(ItemKind),
    GoldPile(u64),
}

impl UpgradeOption {
    pub fn rarity(&self) -> Rarity {
        match self {
            UpgradeOption::Evolve(_) => Rarity::Legendary,
            UpgradeOption::NewWeapon(_) => Rarity::Rare,
            UpgradeOption::WeaponUp(w) => {
                if w.is_evolution() {
                    Rarity::Epic
                } else {
                    Rarity::Common
                }
            }
            UpgradeOption::NewItem(i) | UpgradeOption::ItemUp(i) => i.def().rarity,
            UpgradeOption::GoldPile(_) => Rarity::Common,
        }
    }

    pub fn title(&self) -> String {
        match self {
            UpgradeOption::NewWeapon(w) => format!("NEW: {}", w.def().name),
            UpgradeOption::WeaponUp(w) => w.def().name.to_string(),
            UpgradeOption::Evolve(w) => {
                format!("EVOLVE: {}", w.def().evolves_to.map(|e| e.def().name).unwrap_or("?"))
            }
            UpgradeOption::NewItem(i) => format!("NEW: {}", i.def().name),
            UpgradeOption::ItemUp(i) => i.def().name.to_string(),
            UpgradeOption::GoldPile(g) => format!("{g} GOLD"),
        }
    }

    pub fn body(&self, run: &PlayerState) -> String {
        match self {
            UpgradeOption::NewWeapon(w) => {
                let d = w.def();
                match (d.evolves_to, d.evo_item) {
                    (Some(evo), Some(item)) => format!(
                        "{}\nEvolves: {} (with {})",
                        d.desc,
                        evo.def().name,
                        item.def().name
                    ),
                    _ => d.desc.to_string(),
                }
            }
            UpgradeOption::WeaponUp(w) => {
                let lvl = run.weapons.iter().find(|i| i.kind == *w).map(|i| i.level).unwrap_or(0);
                let d = w.def();
                let mut s = format!("Level {} -> {}\n{}", lvl, lvl + 1, d.desc);
                if let (Some(evo), Some(item)) = (d.evolves_to, d.evo_item) {
                    if run.has_item(item) {
                        s.push_str(&format!(
                            "\nEvolves: {} at Lv{} (catalyst owned!)",
                            evo.def().name,
                            crate::config::MAX_WEAPON_LEVEL
                        ));
                    } else {
                        s.push_str(&format!(
                            "\nEvolves: {} (with {})",
                            evo.def().name,
                            item.def().name
                        ));
                    }
                }
                s
            }
            UpgradeOption::Evolve(w) => w
                .def()
                .evolves_to
                .map(|e| e.def().desc.to_string())
                .unwrap_or_default(),
            UpgradeOption::NewItem(i) => {
                let d = i.def();
                let stats: Vec<String> =
                    d.boosts.iter().map(|(k, v)| k.label(*v)).collect();
                format!("{}\n{}{}", d.desc, stats.join(", "), catalyst_line(*i))
            }
            UpgradeOption::ItemUp(i) => {
                let d = i.def();
                let stats: Vec<String> =
                    d.boosts.iter().map(|(k, v)| k.label(*v)).collect();
                format!(
                    "{} ({}/{})\n{}{}",
                    d.desc,
                    run.item_count(*i),
                    d.max_stacks,
                    stats.join(", "),
                    catalyst_line(*i)
                )
            }
            UpgradeOption::GoldPile(_) => "Cold hard currency".into(),
        }
    }
}

/// "Evo catalyst: Wrench" line for item cards (empty if the item evolves nothing).
pub fn catalyst_line(item: ItemKind) -> String {
    let weapons = WeaponKind::catalyst_for(item);
    if weapons.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = weapons.iter().map(|w| w.def().name).collect();
        format!("\nEvo catalyst: {}", names.join(", "))
    }
}

/// Roll the four level-up options.
pub fn roll_upgrades(run: &PlayerState, save: &MetaSave, rng: &mut impl Rng) -> Vec<UpgradeOption> {
    let mut opts: Vec<UpgradeOption> = Vec::new();

    // Guaranteed evolution card if eligible.
    for base in run.evolvable() {
        opts.push(UpgradeOption::Evolve(base));
        if opts.len() >= 2 {
            break;
        }
    }

    let mut pool: Vec<UpgradeOption> = Vec::new();
    // Weapon level-ups
    for w in &run.weapons {
        if w.level < config::MAX_WEAPON_LEVEL {
            pool.push(UpgradeOption::WeaponUp(w.kind));
            pool.push(UpgradeOption::WeaponUp(w.kind)); // weight x2
        }
    }
    // New weapons (unlocked, slot free)
    if run.weapons.len() < config::WEAPON_SLOTS {
        for w in WeaponKind::BASE {
            if save.unlocked_weapons.contains(&w) && run.weapon_slot_free(w) {
                pool.push(UpgradeOption::NewWeapon(w));
            }
        }
    }
    // Items
    for i in ItemKind::ALL {
        if run.banned_items.contains(&i) {
            continue;
        }
        let count = run.item_count(i);
        if count >= i.def().max_stacks {
            continue;
        }
        // rarity gate: rarer items appear less often in the raw pool
        let copies = match i.def().rarity {
            Rarity::Common => 3,
            Rarity::Rare => 2,
            Rarity::Epic => 1,
            Rarity::Legendary => 1,
        };
        for _ in 0..copies {
            if rng.gen_bool(rarity_pass(i.def().rarity, run.stats.luck)) {
                pool.push(if count == 0 {
                    UpgradeOption::NewItem(i)
                } else {
                    UpgradeOption::ItemUp(i)
                });
            }
        }
    }

    pool.shuffle(rng);
    for p in pool {
        if opts.len() >= 4 {
            break;
        }
        if !opts.contains(&p) {
            opts.push(p);
        }
    }
    while opts.len() < 4 {
        opts.push(UpgradeOption::GoldPile(rng.gen_range(15..45)));
    }
    opts
}

fn rarity_pass(r: Rarity, luck: f32) -> f64 {
    let l = luck.clamp(0.0, 2.0) as f64;
    match r {
        Rarity::Common => 0.9,
        Rarity::Rare => 0.55 + 0.2 * l,
        Rarity::Epic => 0.3 + 0.25 * l,
        Rarity::Legendary => 0.12 + 0.2 * l,
    }
}

impl PlayerState {
    fn weapon_slot_free(&self, w: WeaponKind) -> bool {
        !self.weapons.iter().any(|i| {
            i.kind == w || i.kind.def().evolves_to == Some(w) || w.def().evolves_to == Some(i.kind)
        })
    }

    /// Apply a chosen upgrade. Returns true if it was an evolution (for fanfare).
    pub fn apply_upgrade(&mut self, opt: &UpgradeOption, save: &MetaSave, greed_stacks: u32) -> bool {
        let mut evolved = false;
        match opt {
            UpgradeOption::NewWeapon(w) => {
                self.weapons.push(WeaponInstance { kind: *w, level: 1, cd: 0.0 });
            }
            UpgradeOption::WeaponUp(w) => {
                if let Some(inst) = self.weapon_mut(*w) {
                    inst.level = (inst.level + 1).min(config::MAX_WEAPON_LEVEL);
                }
            }
            UpgradeOption::Evolve(base) => {
                if let Some(evo) = base.def().evolves_to {
                    if let Some(inst) = self.weapon_mut(*base) {
                        inst.kind = evo;
                        inst.level = 1;
                        inst.cd = 0.0;
                        evolved = true;
                    }
                }
            }
            UpgradeOption::NewItem(i) => self.items.push((*i, 1)),
            UpgradeOption::ItemUp(i) => {
                if let Some(entry) = self.items.iter_mut().find(|(k, _)| k == i) {
                    entry.1 += 1;
                }
            }
            UpgradeOption::GoldPile(g) => self.gold += g,
        }
        self.recompute_stats(save, greed_stacks);
        evolved
    }
}

/// The level-up (or shrine/moai) choice panel contents, read by the UI.
#[derive(Resource, Default)]
pub struct ChoicePanel {
    pub title: String,
    pub options: Vec<UpgradeOption>,
    /// True while the player is picking which option to BANISH.
    pub banishing: bool,
    /// Modal panels (shrine loot) don't consume pending level-ups.
    pub is_levelup: bool,
}
