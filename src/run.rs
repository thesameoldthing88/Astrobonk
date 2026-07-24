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
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashSet;

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
}

#[derive(Resource, Clone, Debug)]
pub struct RunState {
    pub character: AstronautKind,
    pub tier: u32,
    pub chain: Vec<PlanetKind>,
    pub stage: usize,
    pub timer: f32,
    pub elapsed: f32,
    pub total_elapsed: f32,
    pub xp: f32,
    pub level: u32,
    pub xp_needed: f32,
    pub pending_levelups: u32,
    pub gold: u64,
    pub silver_run: u64,
    pub kills: u64,
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
    pub greed_stacks: u32,
    pub boss_spawned: bool,
    pub boss_dead: bool,
    pub minibosses_spawned: [bool; 2],
    pub static_active: bool,
    pub static_timer: f32,
    pub teleporter_open: bool,
    pub frenzy_timer: f32,
    pub powerups: Vec<(PowerupKind, f32)>,
    pub microwave_used: bool,
    pub chest_opens: u32,
    pub shrines_charged: u64,
    pub pots_broken: u64,
    pub chests_opened: u64,
    pub gold_collected: u64,
    pub evolves: u64,
    pub result: Option<RunResult>,
}

impl RunState {
    pub fn new(character: AstronautKind, start: PlanetKind, tier: u32, save: &MetaSave) -> Self {
        let chain = PlanetKind::chain_from(start, tier);
        let mut s = Self {
            character,
            tier,
            chain,
            stage: 0,
            timer: config::STAGE_SECONDS[0],
            elapsed: 0.0,
            total_elapsed: 0.0,
            xp: 0.0,
            level: 1,
            xp_needed: xp_needed(1),
            pending_levelups: 0,
            gold: 0,
            silver_run: 0,
            kills: 0,
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
            greed_stacks: 0,
            boss_spawned: false,
            boss_dead: false,
            minibosses_spawned: [false; 2],
            static_active: false,
            static_timer: 0.0,
            teleporter_open: false,
            frenzy_timer: 0.0,
            powerups: Vec::new(),
            microwave_used: false,
            chest_opens: 0,
            shrines_charged: 0,
            pots_broken: 0,
            chests_opened: 0,
            gold_collected: 0,
            evolves: 0,
            result: None,
        };
        s.recompute_stats(save);
        s.hp = s.stats.max_hp;
        s.shield = s.stats.shield;
        // Lady Fortuna gambles harder — extra level-up rerolls.
        if character == AstronautKind::Fortuna {
            s.refreshes += 2;
        }
        s
    }

    pub fn planet(&self) -> PlanetKind {
        self.chain[self.stage.min(self.chain.len() - 1)]
    }

    pub fn recompute_stats(&mut self, save: &MetaSave) {
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
        // Greed shrines
        st.apply(StatKind::Difficulty, 0.12 * self.greed_stacks as f32);
        st.apply(StatKind::Luck, 0.08 * self.greed_stacks as f32);
        // Level-ups grant +1 max hp each (genre staple)
        st.apply(StatKind::MaxHp, (self.level - 1) as f32);

        let hp_frac = if self.stats.max_hp > 0.0 { self.hp / self.stats.max_hp } else { 1.0 };
        self.stats = st;
        self.hp = (self.stats.max_hp * hp_frac).clamp(0.0, self.stats.max_hp);
    }

    /// Effective attack speed including Yuki frenzy + powerups.
    pub fn attack_speed(&self) -> f32 {
        let mut a = self.stats.attack_speed;
        if self.frenzy_timer > 0.0 {
            if let Passive::SlideFrenzy { bonus, .. } = self.character.def().passive {
                a += bonus;
            }
        }
        a.max(0.1)
    }

    pub fn damage_mult(&self) -> f32 {
        let mut d = self.stats.damage;
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Damage2x) {
            d *= 2.0;
        }
        d
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

    pub fn body(&self, run: &RunState) -> String {
        match self {
            UpgradeOption::NewWeapon(w) => w.def().desc.to_string(),
            UpgradeOption::WeaponUp(w) => {
                let lvl = run.weapons.iter().find(|i| i.kind == *w).map(|i| i.level).unwrap_or(0);
                format!("Level {} -> {}\n{}", lvl, lvl + 1, w.def().desc)
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
                format!("{}\n{}", d.desc, stats.join(", "))
            }
            UpgradeOption::ItemUp(i) => {
                let d = i.def();
                let stats: Vec<String> =
                    d.boosts.iter().map(|(k, v)| k.label(*v)).collect();
                format!("{} ({}/{})\n{}", d.desc, run.item_count(*i), d.max_stacks, stats.join(", "))
            }
            UpgradeOption::GoldPile(_) => "Cold hard currency".into(),
        }
    }
}

/// Roll the four level-up options.
pub fn roll_upgrades(run: &RunState, save: &MetaSave, rng: &mut impl Rng) -> Vec<UpgradeOption> {
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

impl RunState {
    fn weapon_slot_free(&self, w: WeaponKind) -> bool {
        !self.weapons.iter().any(|i| {
            i.kind == w || i.kind.def().evolves_to == Some(w) || w.def().evolves_to == Some(i.kind)
        })
    }

    /// Apply a chosen upgrade. Returns true if it was an evolution (for fanfare).
    pub fn apply_upgrade(&mut self, opt: &UpgradeOption, save: &MetaSave) -> bool {
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
                self.evolves += 1;
            }
            UpgradeOption::NewItem(i) => self.items.push((*i, 1)),
            UpgradeOption::ItemUp(i) => {
                if let Some(entry) = self.items.iter_mut().find(|(k, _)| k == i) {
                    entry.1 += 1;
                }
            }
            UpgradeOption::GoldPile(g) => self.gold += g,
        }
        self.recompute_stats(save);
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
