//! Run-scoped state: the live character sheet, level-up logic, upgrade rolls.
//! Pure data + logic here; the systems that drive a run live in `director.rs`.

use crate::config;
use crate::content::characters::{AstronautKind, Passive};
use crate::content::items::ItemKind;
use crate::content::planets::PlanetKind;
use crate::content::weapons::WeaponKind;
use crate::content::Rarity;
use crate::save::{AssistOptions, MetaSave};
use crate::stats::{StatKind, Stats};
use bevy::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;

pub mod scaling;

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
    /// Stage bosses killed this run — the §10 Silver formula's `boss_kills`.
    pub boss_kills: u64,
    /// Seconds spent in The Static across ALL stages (`static_timer` restarts per stage) —
    /// the §10 formula's `Static_overtime_seconds`.
    pub static_secs_total: f32,
    /// Of `silver_run`, the Silver The Static's ghosts dropped (`pickups::StaticSilver`) —
    /// what Tome of Static multiplies at banking. HOST state: only the host banks.
    pub static_silver_found: u64,
    /// Where the guaranteed miniboss-#1 cache stands, while it is unopened. Set by the host
    /// when miniboss #1 dies and cleared when it is opened; streamed in `RunSnapMsg` so a
    /// client draws the same chest (`interact::sync_reward_cache` owns the entity).
    pub reward_chest: Option<Vec3>,
    pub result: Option<RunResult>,
    /// Aggregated difficulty from all players (Cursed items/tomes). World-level in co-op.
    pub difficulty: f32,
    /// Someone in the party carries The Static Radio: The Static comes early and angrier
    /// (§7). Party-wide like `difficulty`, set by the host each frame (`items::run_wide_items`).
    pub static_radio: bool,
    /// Tome of the Elite: how much more loot elite kills drop (the party's best, ≥ 1). HOST
    /// state like `static_radio`, set each frame by `items::item_upkeep`; drops are the
    /// host's, so it never crosses the wire.
    pub elite_loot: f32,
    /// How far the sun has turned this stage (radians; `daynight::Sun` turns it into the
    /// sunward direction). The host advances it, `RunSnapMsg` carries it and a client
    /// dead-reckons between snapshots, so both machines light — and judge night by — one sun.
    pub sun_phase: f32,
    /// §3 diegetic difficulty: how far the day side has shrunk toward night-lock, 0..1 —
    /// eaten by the Devoured Sun Shard and the party's Difficulty (`daynight::advance_sun`).
    /// Kept across stages: the world stays dying. Streamed like `sun_phase`.
    pub sun_shrink: f32,
    /// HOST: seconds since the sun was last eaten.
    pub sun_eat_secs: f32,
    /// The §13 "difficulty as options" in force right now — the HOST's, kept current from
    /// its settings by `director::sync_assist_options` and streamed to joiners.
    pub assist: AssistOptions,
    /// This run has used an assist at any point (sticky: easing the first stage and
    /// restoring canon for the boss still counts). Shown on results; keeps the daily score
    /// off the unassisted board.
    pub assisted: bool,
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
    pub items: Vec<ItemStack>,
    pub stats: Stats,
    pub hp: f32,
    pub shield: f32,
    pub shield_cd: f32,
    pub iframes: f32,
    /// Banish charges left this run.
    pub banishes: u32,
    /// FREE refreshes left this run; once spent, refreshes cost Gold (`refresh_price`).
    pub refreshes: u32,
    /// Paid refreshes bought so far — drives the rising Gold price.
    pub paid_refreshes: u32,
    pub banned_items: HashSet<ItemKind>,
    /// Weapons (and evolutions) banished out of this run's card pool.
    pub banned_weapons: HashSet<WeaponKind>,
    /// Extra evolution slots on top of `config::EVOLUTION_CAP`: Tome of Ascension's, set
    /// when the sheet is made. Read only through `evo_cap()`.
    pub evo_slots_bonus: u32,
    /// Free Microwave uses left this stage (Tome of Duplication); refilled by `enter_stage`.
    pub free_microwave: u32,
    pub frenzy_timer: f32,
    pub fast_move: bool,     // above base run speed (Nova / Aurora passives)
    pub reticle_timer: f32,  // cycles 0..1.5 for Reticle's focus pulse
    pub powerups: Vec<(PowerupKind, f32)>,
    pub dead: bool,
    // ---- live conditions the conditional items read (refreshed every frame, like
    // `fast_move`, by player_physics / items::encirclement_scan) ----
    /// Off the ground — Icarus Boots, Anti-Grav Boots' ring.
    pub airborne: bool,
    /// Metres of altitude lost over the last second — Downhill Momentum.
    pub descent_m: f32,
    /// Bitmask of the compass octants holding an enemy nearby — Encirclement Bonus.
    pub encircle_dirs: u8,
    /// Foes within TOME_CROWD_RADIUS (capped) — Tome of Encirclement.
    pub crowd: u32,
    /// Standing on the night side (`daynight::Sun::is_night`) — Tome of Nightfall.
    pub night: bool,
    /// Seconds left of Mars's thorn-flora slow (`gimmicks::thorn_contact`). Set by
    /// `player_physics` on every body a machine moves, so a joiner predicts its own snag.
    pub thorned: f32,
    /// Seconds of unbroken movement, 0..MOMENTUM_RAMP_SECS — Tome of Momentum.
    pub momentum: f32,
    /// Dead Man's Tether already rewound this run (it is once per run, so it rides the
    /// sheet across stages rather than the per-stage `items::ItemProcs`).
    pub tether_used: bool,
    /// The weapon Second Astronaut's ghost is mirroring. Picked by the host, adopted by a
    /// client from the replicated `net::NetItemVis`, so its own cosmetic fire matches.
    pub ghost_weapon: Option<WeaponKind>,
    /// "One more chance" revives spent this run (the token is one per astronaut per run).
    /// A counter rather than a flag so a joiner's HUD, which sees it through
    /// `PlayerVitals`, can tell a revive happened even if it missed the frame it did.
    pub revives: u32,
}

/// Every copy of one item a player holds. Grades are kept per copy because each copy was
/// rolled on its own (§7): the stack's effect is the SUM of its copies' grade multipliers.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStack {
    pub kind: ItemKind,
    pub grades: Vec<Rarity>,
}

impl ItemStack {
    pub fn count(&self) -> u32 {
        self.grades.len() as u32
    }
    /// Σ grade multipliers — what boosts and proc numbers scale by.
    pub fn power(&self) -> f32 {
        self.grades.iter().map(|g| self.kind.grade_mult(*g)).sum()
    }
    /// The highest grade held (HUD chip colour, Microwave duplication).
    pub fn best(&self) -> Rarity {
        self.grades.iter().copied().max().unwrap_or(self.kind.def().rarity)
    }
}

impl RunState {
    pub fn new(character: AstronautKind, start: PlanetKind, tier: u32, save: &MetaSave) -> Self {
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
            boss_kills: 0,
            static_secs_total: 0.0,
            static_silver_found: 0,
            reward_chest: None,
            result: None,
            difficulty: 0.0,
            static_radio: false,
            elite_loot: 1.0,
            sun_phase: 0.0,
            sun_shrink: 0.0,
            sun_eat_secs: 0.0,
            assist: save.assist,
            assisted: save.assist.is_assisted(),
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
            // + Tome of Banishment's, once the sheet knows its stats (below)
            banishes: config::BANISH_CHARGES,
            refreshes: config::FREE_REFRESHES,
            paid_refreshes: 0,
            banned_items: HashSet::new(),
            banned_weapons: HashSet::new(),
            evo_slots_bonus: 0,
            free_microwave: 0,
            frenzy_timer: 0.0,
            fast_move: false,
            reticle_timer: 0.0,
            powerups: Vec::new(),
            dead: false,
            airborne: false,
            descent_m: 0.0,
            encircle_dirs: 0,
            crowd: 0,
            night: false,
            thorned: 0.0,
            momentum: 0.0,
            tether_used: false,
            ghost_weapon: None,
            revives: 0,
        };
        s.recompute_stats(save, 0);
        s.hp = s.stats.max_hp;
        s.shield = s.stats.shield;
        // The per-run allowances the tome loadout widens (Banishment, Ascension). Set once:
        // the loadout is fixed for the run, and these are spent, not recomputed.
        s.banishes += s.stats.extra_banishes.max(0) as u32;
        s.refreshes += s.stats.extra_refreshes.max(0) as u32;
        s.evo_slots_bonus = s.stats.evo_slots.max(0) as u32;
        s.enter_stage();
        s
    }

    /// A new stage begins for this sheet (the first one, or through a teleporter): refill
    /// what is granted per stage.
    pub fn enter_stage(&mut self) {
        self.free_microwave = self.stats.free_microwave.max(0) as u32;
    }

    pub fn recompute_stats(&mut self, save: &MetaSave, greed_stacks: u32) {
        let mut st = Stats::default();
        // Tomes (meta loadout): every line of each slotted tome at its rank
        for t in &save.tome_loadout {
            let rank = save.tome_level(*t);
            for e in t.def().effects {
                if rank > 0 {
                    st.apply(e.stat, e.at(rank));
                }
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
        // Items: each boost scaled by the stack's summed grade multipliers
        for stack in &self.items {
            // Tome of Vampirism feeds Vampire Visor (the tome lines were applied first)
            let boost = if stack.kind == ItemKind::VampireVisor { 1.0 + st.visor_boost } else { 1.0 };
            let power = stack.power() * boost;
            for (k, v) in stack.kind.def().boosts {
                st.apply(*k, v * power);
            }
        }
        // Greed shrines (run-global, passed in)
        st.apply(StatKind::Difficulty, 0.12 * greed_stacks as f32);
        st.apply(StatKind::Luck, 0.08 * greed_stacks as f32);
        // Level-ups grant +1 max hp each (genre staple)
        st.apply(StatKind::MaxHp, (self.level - 1) as f32);
        // Percentage max HP (Widow's Ring) lands on the finished flat total.
        st.max_hp = (st.max_hp * st.max_hp_mult.max(0.1)).max(1.0);

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
        (a * self.widow_mult()).max(0.1)
    }

    pub fn damage_mult(&self) -> f32 {
        let mut d = (self.stats.damage + self.conditional_damage()).max(0.1);
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Damage2x) {
            d *= 2.0;
        }
        // Sgt. Gristle: cornered and furious — big damage below half HP.
        if self.character == AstronautKind::Gristle && self.hp < self.stats.max_hp * 0.5 {
            d *= 1.4;
        }
        d * self.widow_mult()
    }

    /// The §7 conditional damage items, additive to the Damage stat like any "+X% dmg".
    /// They read live conditions (`airborne`, `descent_m`, `encircle_dirs`) that each
    /// machine refreshes every frame, so the number is the same on the host that deals the
    /// damage and on the owner's HUD.
    pub fn conditional_damage(&self) -> f32 {
        let mut bonus = 0.0;
        let icarus = self.item_power(ItemKind::IcarusBoots);
        if icarus > 0.0 {
            bonus += if self.airborne {
                config::ICARUS_AIR_BONUS * icarus
            } else {
                -config::ICARUS_GROUND_PENALTY * self.item_count(ItemKind::IcarusBoots) as f32
            };
        }
        bonus += config::DOWNHILL_DMG_PER_M * self.descent_m * self.item_power(ItemKind::DownhillMomentum);
        bonus += config::ENCIRCLE_DMG_PER_DIR
            * self.encircle_dirs.count_ones() as f32
            * self.item_power(ItemKind::EncirclementBonus);
        bonus + self.tome_conditional_damage()
    }

    /// The tomes' conditional damage — Encirclement's crowd, Nightfall's dark side,
    /// Momentum's run-up — read from the same per-frame conditions as the items above.
    pub fn tome_conditional_damage(&self) -> f32 {
        self.crowd_bonus() + self.night_bonus() + self.momentum_bonus()
    }

    pub fn crowd_bonus(&self) -> f32 {
        self.stats.crowd_damage * self.crowd.min(config::TOME_CROWD_CAP) as f32
    }

    pub fn night_bonus(&self) -> f32 {
        if self.night {
            self.stats.night_damage
        } else {
            0.0
        }
    }

    pub fn momentum_bonus(&self) -> f32 {
        self.stats.momentum_damage * (self.momentum / config::MOMENTUM_RAMP_SECS).clamp(0.0, 1.0)
    }

    /// Widow's Ring is live: the ring is worn and the astronaut hangs at 1 HP.
    pub fn widow_active(&self) -> bool {
        self.hp > 0.0 && self.hp < config::WIDOW_HP_BAND && self.has_item(ItemKind::WidowsRing)
    }

    /// Widow's Ring "+30% all stats": one factor every stat accessor applies.
    pub fn widow_mult(&self) -> f32 {
        if self.widow_active() {
            1.0 + config::WIDOW_STAT_BONUS
        } else {
            1.0
        }
    }

    /// Effective crit chance — Dr. Reticle's focus pulse guarantees a crit briefly each cycle.
    pub fn crit_chance(&self) -> f32 {
        let mut c = self.stats.crit_chance * self.widow_mult();
        if self.character == AstronautKind::Reticle && self.reticle_timer < 0.35 {
            c += 1.0;
        }
        c
    }

    pub fn crit_damage(&self) -> f32 {
        self.stats.crit_damage * self.widow_mult()
    }

    /// The sheet as weapons should read it right now: the scalar stats with Widow's Ring's
    /// bonus folded in. Weapons clone the sheet once per volley anyway.
    pub fn live_stats(&self) -> Stats {
        let mut s = self.stats.clone();
        let w = self.widow_mult();
        if w != 1.0 {
            s.crit_damage *= w;
            s.proj_speed *= w;
            s.size *= w;
            s.duration *= w;
            s.knockback *= w;
            s.elite_damage *= w;
        }
        s
    }

    /// Aura-radius multiplier — Aurora Prime's fields swell while she's sprinting.
    pub fn aura_scale(&self) -> f32 {
        if self.character == AstronautKind::Aurora && self.fast_move {
            1.35
        } else {
            1.0
        }
    }

    /// Armor after hero mechanics — Old Ironclad's plating doubles when badly hurt. Armor
    /// and evasion use diminishing curves and can never reach 100%.
    pub fn effective_armor_fraction(&self) -> f32 {
        let mut armor = self.stats.armor.max(0.0) * self.widow_mult();
        if self.character == AstronautKind::Ironclad && self.hp < self.stats.max_hp * 0.3 {
            armor *= 2.0;
        }
        armor / (armor + 100.0)
    }

    pub fn effective_evasion_fraction(&self) -> f32 {
        let e = self.stats.evasion.max(0.0) * self.widow_mult();
        e / (e + 100.0)
    }

    pub fn move_speed_mult(&self) -> f32 {
        let mut m = self.stats.move_speed * self.widow_mult();
        if self.powerups.iter().any(|(k, _)| *k == PowerupKind::Speed) {
            m *= 1.5;
        }
        if self.thorned > 0.0 {
            m *= config::THORN_SLOW;
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
        self.items.iter().any(|s| s.kind == kind)
    }

    pub fn item_count(&self, kind: ItemKind) -> u32 {
        self.item_stack(kind).map(|s| s.count()).unwrap_or(0)
    }

    pub fn item_stack(&self, kind: ItemKind) -> Option<&ItemStack> {
        self.items.iter().find(|s| s.kind == kind)
    }

    /// Σ grade multipliers of every copy held; 0 without the item. What proc numbers scale by.
    pub fn item_power(&self, kind: ItemKind) -> f32 {
        self.item_stack(kind).map(|s| s.power()).unwrap_or(0.0)
    }

    /// Take one copy of `kind` at `grade`. Stats are NOT recomputed here — the caller owns
    /// the save/greed context that needs.
    pub fn add_item(&mut self, kind: ItemKind, grade: Rarity) {
        match self.items.iter_mut().find(|s| s.kind == kind) {
            Some(s) => s.grades.push(grade),
            None => self.items.push(ItemStack { kind, grades: vec![grade] }),
        }
    }

    /// Rarity Grades The Static Radio adds to every loot roll this player makes (§7).
    pub fn grade_bonus(&self) -> u32 {
        u32::from(self.has_item(ItemKind::StaticRadio))
    }

    /// Signal Flare: the horde always knows where this astronaut is — no storm or night
    /// hides them, the chase prefers them, and their share of each wave lands closer.
    pub fn revealed(&self) -> bool {
        self.has_item(ItemKind::SignalFlare)
    }

    /// Is the §13 "one more chance" token still in hand? It exists only while the run's
    /// assist options grant it, and each astronaut spends it once.
    pub fn revive_token_ready(&self, assist: &AssistOptions) -> bool {
        assist.revive_token && self.revives == 0
    }

    /// Spend the token on a would-be-lethal hit: back up at REVIVE_TOKEN_HP_FRAC with a few
    /// seconds of invulnerability. Returns false (and changes nothing) without a token.
    ///
    /// The LAST link of the death-save chain: item death-saves (P03's resolver — Dead Man's
    /// Tether, Boomerang Insurance, Widow's Ring) get the hit first, so an accessibility
    /// option never eats a save the build paid for.
    pub fn try_revive_token(&mut self, assist: &AssistOptions) -> bool {
        if !self.revive_token_ready(assist) {
            return false;
        }
        self.revives += 1;
        self.dead = false;
        self.hp = (self.stats.max_hp * config::REVIVE_TOKEN_HP_FRAC).max(1.0);
        self.iframes = self.iframes.max(config::REVIVE_TOKEN_IFRAMES);
        true
    }

    /// Evolved weapons this astronaut already owns.
    pub fn evolutions_used(&self) -> u32 {
        self.weapons.iter().filter(|w| w.kind.is_evolution()).count() as u32
    }

    /// How many evolved weapons this astronaut may own (§15: 1 per run, +1 with Tome of
    /// Ascension). The ONE place the cap is read, so the tome only has to raise the bonus.
    pub fn evo_cap(&self) -> u32 {
        config::EVOLUTION_CAP + self.evo_slots_bonus
    }

    /// Weapons ready to evolve: at max level, paired item owned, evolution not banished,
    /// and an evolution slot still free.
    pub fn evolvable(&self) -> Vec<WeaponKind> {
        if self.evolutions_used() >= self.evo_cap() {
            return Vec::new();
        }
        self.weapons
            .iter()
            .filter(|w| w.level >= config::MAX_WEAPON_LEVEL)
            .filter_map(|w| {
                let def = w.kind.def();
                match (def.evolves_to, def.evo_item) {
                    (Some(evo), Some(item)) if self.has_item(item) && !self.banned_weapons.contains(&evo) => {
                        Some(w.kind)
                    }
                    _ => None,
                }
            })
            .collect()
    }

    // ------------------------------------------------ level-up choice economy (§3)

    /// What the next level-up Refresh costs. Lady Fortuna never pays (her passive); everyone
    /// else spends the run's free refreshes first, then Gold that rises with each paid use.
    pub fn refresh_price(&self) -> RefreshPrice {
        if self.character == AstronautKind::Fortuna || self.refreshes > 0 {
            RefreshPrice::Free
        } else {
            let cost = config::REFRESH_BASE_COST as f32
                * config::REFRESH_COST_GROWTH.powi(self.paid_refreshes as i32)
                * (1.0 - self.stats.refresh_discount.clamp(0.0, 0.9));
            RefreshPrice::Gold((cost.round() as u64).max(1))
        }
    }

    /// Pay for one Refresh. False (and nothing spent) when it can't be afforded.
    pub fn spend_refresh(&mut self) -> bool {
        match self.refresh_price() {
            RefreshPrice::Free => {
                if self.character != AstronautKind::Fortuna {
                    self.refreshes -= 1;
                }
                true
            }
            RefreshPrice::Gold(cost) if self.gold >= cost => {
                self.gold -= cost;
                self.paid_refreshes += 1;
                true
            }
            RefreshPrice::Gold(_) => false,
        }
    }

    /// What Skip pays right now: (gold tip, xp). The xp is a slice of the current bar, so
    /// the boost stays "small" at every level instead of fading to nothing.
    pub fn skip_reward(&self) -> (u64, f32) {
        let gold = (config::SKIP_GOLD_BASE + config::SKIP_GOLD_PER_LEVEL * self.level as f32)
            * self.stats.gold_gain;
        (gold.round() as u64, self.xp_needed * config::SKIP_XP_FRACTION)
    }

    /// Take the Skip payout. The xp goes through `gain_xp`, so if it tips the bar the
    /// level-up simply queues like any other.
    pub fn take_skip(&mut self) -> (u64, f32) {
        let (gold, xp) = self.skip_reward();
        self.gold += gold;
        self.xp += xp;
        self.gain_xp(0.0);
        (gold, xp)
    }

    /// Banish a card: spend a charge and strike what it offers from this run's pool for
    /// good. False when there's no charge or the card isn't a pool entry (a Gold pile is
    /// filler, not a card anyone can steer away from).
    pub fn banish(&mut self, opt: &UpgradeOption) -> bool {
        if self.banishes == 0 {
            return false;
        }
        match opt {
            // A banish strikes the ITEM, every grade of it.
            UpgradeOption::NewItem(i, _) | UpgradeOption::ItemUp(i, _) => {
                self.banned_items.insert(*i);
            }
            UpgradeOption::NewWeapon(w) | UpgradeOption::WeaponUp(w) => {
                self.banned_weapons.insert(*w);
            }
            UpgradeOption::Evolve(base) => match base.def().evolves_to {
                Some(evo) => {
                    self.banned_weapons.insert(evo);
                }
                None => return false,
            },
            UpgradeOption::GoldPile(_) => return false,
        }
        self.banishes -= 1;
        true
    }
}

/// Price of the next Refresh (see `PlayerState::refresh_price`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshPrice {
    Free,
    Gold(u64),
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
    /// An item and the Rarity Grade this copy was rolled at.
    NewItem(ItemKind, Rarity),
    ItemUp(ItemKind, Rarity),
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
            UpgradeOption::NewItem(_, g) | UpgradeOption::ItemUp(_, g) => *g,
            UpgradeOption::GoldPile(_) => Rarity::Common,
        }
    }

    /// The card for one copy of `item` at `grade`, NEW or a stack-up by what `ps` holds.
    pub fn item(item: ItemKind, grade: Rarity, ps: &PlayerState) -> Self {
        if ps.has_item(item) {
            UpgradeOption::ItemUp(item, grade)
        } else {
            UpgradeOption::NewItem(item, grade)
        }
    }

    /// The item this card deals, if it is an item card.
    pub fn item_kind(&self) -> Option<ItemKind> {
        match self {
            UpgradeOption::NewItem(i, _) | UpgradeOption::ItemUp(i, _) => Some(*i),
            _ => None,
        }
    }

    pub fn title(&self) -> String {
        match self {
            UpgradeOption::NewWeapon(w) => format!("NEW: {}", w.def().name),
            UpgradeOption::WeaponUp(w) => w.def().name.to_string(),
            UpgradeOption::Evolve(w) => {
                format!("EVOLVE: {}", w.def().evolves_to.map(|e| e.def().name).unwrap_or("?"))
            }
            UpgradeOption::NewItem(i, _) => format!("NEW: {}", i.def().name),
            UpgradeOption::ItemUp(i, _) => i.def().name.to_string(),
            UpgradeOption::GoldPile(g) => format!("{g} GOLD"),
        }
    }

    pub fn body(&self, run: &PlayerState) -> String {
        match self {
            UpgradeOption::NewWeapon(w) => {
                let d = w.def();
                match (d.evolves_to, d.evo_item) {
                    (Some(_), Some(_)) if run.evolutions_used() >= run.evo_cap() => {
                        format!("{}\n{}", d.desc, evo_slots_full(run))
                    }
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
                    if run.evolutions_used() >= run.evo_cap() {
                        s.push('\n');
                        s.push_str(&evo_slots_full(run));
                    } else if run.has_item(item) {
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
            UpgradeOption::NewItem(i, g) => format!("{}\n{}{}", i.def().desc, item_lines(*i, *g), catalyst_line(*i, run)),
            UpgradeOption::ItemUp(i, g) => {
                let d = i.def();
                let held = if d.max_stacks >= config::ITEM_STACKS_UNCAPPED {
                    format!("{} held", run.item_count(*i))
                } else {
                    format!("{}/{}", run.item_count(*i), d.max_stacks)
                };
                format!("{} ({held})\n{}{}", d.desc, item_lines(*i, *g), catalyst_line(*i, run))
            }
            UpgradeOption::GoldPile(_) => "Cold hard currency".into(),
        }
    }
}

/// What one copy of `item` at `grade` gives: its graded stat lines, what it does, and —
/// when it rolled above its native grade — by how much the roll beat the base (§7: a bigger
/// roll of the SAME effect). Shared by every card that deals an item (level-up, chest,
/// vendor, shrine, cache).
pub fn item_lines(item: ItemKind, grade: Rarity) -> String {
    let m = item.grade_mult(grade);
    let mut lines: Vec<String> = Vec::new();
    let stats: Vec<String> = item.def().boosts.iter().map(|(k, v)| k.label(v * m)).collect();
    if !stats.is_empty() {
        lines.push(stats.join(", "));
    }
    if let Some(e) = item.effect(m) {
        lines.push(e);
    }
    if m > 1.0 {
        lines.push(format!("{} roll: x{m:.1} effect", grade.name()));
    }
    lines.join("\n")
}

/// Card line shown instead of an evolution hint once every evolution slot is taken, so a
/// player never levels a weapon to 7 expecting an evolution the cap will refuse.
fn evo_slots_full(run: &PlayerState) -> String {
    format!("Evolution slots full ({}/{})", run.evolutions_used(), run.evo_cap())
}

/// "Evo catalyst: Wrench" line for item cards (empty if the item evolves nothing). Once
/// every evolution slot is taken it says so instead: a catalyst hint would sell the player
/// an evolution the cap will refuse.
pub fn catalyst_line(item: ItemKind, run: &PlayerState) -> String {
    let weapons = WeaponKind::catalyst_for(item);
    if weapons.is_empty() {
        String::new()
    } else if run.evolutions_used() >= run.evo_cap() {
        format!("\n{}", evo_slots_full(run))
    } else {
        let names: Vec<&str> = weapons.iter().map(|w| w.def().name).collect();
        format!("\nEvo catalyst: {}", names.join(", "))
    }
}

/// The grade a dealt copy of `item` comes at (§7 "Luck raises grade rolls everywhere"): a
/// ladder roll at `luck`, never below the item's native grade, then `bonus` rungs up (The
/// Static Radio). Cursed items are always Cursed. Always exactly one draw.
pub fn roll_grade(item: ItemKind, luck: f32, bonus: u32, rng: &mut impl Rng) -> Rarity {
    let native = item.def().rarity;
    let rolled = Rarity::roll(luck, rng);
    if native == Rarity::Cursed {
        return Rarity::Cursed;
    }
    rolled.max(native).step_up(bonus)
}

/// Can `ps` still be dealt `item` (pooled, not banished, below its stack cap)?
pub fn item_available(ps: &PlayerState, item: ItemKind) -> bool {
    let d = item.def();
    d.pooled && !ps.banned_items.contains(&item) && ps.item_count(item) < d.max_stacks
}

/// Roll one loot item and the grade it drops at — chests, the Shady Guy, shrines, the Moai
/// and the miniboss cache all deal through here (§7). First a `CURSED_LOOT_CHANCE` for a
/// cursed item; otherwise a grade from `luck` (+ The Static Radio's rung), then an item that
/// can drop at that grade: one native to it `GRADE_NATIVE_SHARE` of the time, else a
/// lower-grade item rolled up to it.
///
/// A FIXED three draws whatever the sheet: the Shady Guy's stock is rolled from the seeded
/// world stream (CLAUDE.md rule 5), where a draw count that depended on a machine's own save
/// would shift every later draw.
pub fn roll_item(ps: &PlayerState, luck: f32, rng: &mut impl Rng) -> (ItemKind, Rarity) {
    let cursed = rng.gen_bool(config::CURSED_LOOT_CHANCE);
    let grade = Rarity::roll(luck, rng).step_up(ps.grade_bonus());
    let pick: f32 = rng.gen_range(0.0..1.0);

    // Cursed: one of the cursed family, if any is still available. Otherwise an item that
    // can drop at the rolled grade: one NATIVE to it GRADE_NATIVE_SHARE of the time (a
    // Legendary roll is mostly Legendaries), else a lower-grade item rolled up to it (a
    // huge Borgar). A fixed share, not per-item weights, so the pool's grade mix can't
    // drown the rare natives out. The one `pick` draw chooses both bucket and item.
    let cursed_items: Vec<ItemKind> = if cursed {
        ItemKind::pool().filter(|i| i.is_cursed() && item_available(ps, *i)).collect()
    } else {
        Vec::new()
    };
    let (bucket, x) = if !cursed_items.is_empty() {
        (cursed_items, pick)
    } else {
        let (native, lower): (Vec<ItemKind>, Vec<ItemKind>) = ItemKind::pool()
            .filter(|i| !i.is_cursed() && item_available(ps, *i) && i.def().rarity <= grade)
            .partition(|i| i.def().rarity == grade);
        let share = config::GRADE_NATIVE_SHARE;
        if lower.is_empty() || (!native.is_empty() && pick < share) {
            let x = if lower.is_empty() { pick } else { pick / share };
            (native, x)
        } else if native.is_empty() {
            (lower, pick)
        } else {
            (lower, (pick - share) / (1.0 - share))
        }
    };
    let grade_of = |i: ItemKind| if i.is_cursed() { Rarity::Cursed } else { grade };
    match bucket.get(((x * bucket.len() as f32) as usize).min(bucket.len().saturating_sub(1))) {
        Some(item) => (*item, grade_of(*item)),
        // Everything capped or banished: a Borgar — it never caps out.
        None => (ItemKind::SpaceBorgar, grade),
    }
}

/// Roll the level-up options (`config::LEVELUP_CARDS` of them).
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
        if w.level < config::MAX_WEAPON_LEVEL && !run.banned_weapons.contains(&w.kind) {
            pool.push(UpgradeOption::WeaponUp(w.kind));
            pool.push(UpgradeOption::WeaponUp(w.kind)); // weight x2
        }
    }
    // New weapons (unlocked, slot free)
    if run.weapons.len() < config::WEAPON_SLOTS {
        for w in WeaponKind::BASE {
            if save.unlocked_weapons.contains(&w) && run.weapon_slot_free(w) && !run.banned_weapons.contains(&w) {
                pool.push(UpgradeOption::NewWeapon(w));
            }
        }
    }
    // Items, each copy dealt at its own rolled grade
    for i in ItemKind::pool() {
        if !item_available(run, i) {
            continue;
        }
        // rarity gate: rarer items appear less often in the raw pool
        let native = i.def().rarity;
        let copies = match native {
            Rarity::Common => 3,
            Rarity::Rare => 2,
            Rarity::Epic | Rarity::Legendary | Rarity::Cursed => 1,
        };
        for _ in 0..copies {
            if rng.gen_bool(rarity_pass(native, run.stats.luck)) {
                let grade = roll_grade(i, run.stats.luck, run.grade_bonus(), rng);
                pool.push(UpgradeOption::item(i, grade, run));
            }
        }
    }

    pool.shuffle(rng);
    for p in pool {
        if opts.len() >= config::LEVELUP_CARDS {
            break;
        }
        // one card per weapon/item: two grades of the same item is not a choice
        let dup = opts.iter().any(|o| o == &p || (o.item_kind().is_some() && o.item_kind() == p.item_kind()));
        if !dup {
            opts.push(p);
        }
    }
    while opts.len() < config::LEVELUP_CARDS {
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
        // Luck-blind on purpose: Cursed is the honeypot you walk into, not a reward tier.
        Rarity::Cursed => config::CURSED_CARD_PASS,
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
                // Re-check the cap here, not only when dealing: a card dealt before another
                // evolution landed (queued level-ups) must not slip past it.
                let slot_free = self.evolutions_used() < self.evo_cap();
                if let (Some(evo), true) = (base.def().evolves_to, slot_free) {
                    if let Some(inst) = self.weapon_mut(*base) {
                        inst.kind = evo;
                        inst.level = 1;
                        inst.cd = 0.0;
                        evolved = true;
                    }
                }
            }
            UpgradeOption::NewItem(i, g) | UpgradeOption::ItemUp(i, g) => {
                // Re-checked at pick time too: a queued hand can deal the last copy twice.
                if self.item_count(*i) < i.def().max_stacks {
                    self.add_item(*i, *g);
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

/// Headless self-check of the §3 choice economy and the §15 evolution cap on synthetic
/// sheets. Returns the first violated rule. Pure data — no world needed.
pub fn rules_self_check(save: &MetaSave) -> Result<(), String> {
    let mut rng = StdRng::seed_from_u64(0xA570);

    // Refresh: FREE_REFRESHES free, then Gold rising per paid use; refused when broke.
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    for _ in 0..config::FREE_REFRESHES {
        if ps.refresh_price() != RefreshPrice::Free || !ps.spend_refresh() {
            return Err("free refreshes were not free".into());
        }
    }
    let RefreshPrice::Gold(first) = ps.refresh_price() else {
        return Err("refresh stayed free past the free allowance".into());
    };
    ps.gold = first - 1;
    if ps.spend_refresh() || ps.gold != first - 1 {
        return Err("an unaffordable refresh went through".into());
    }
    ps.gold = 10_000;
    if !ps.spend_refresh() || ps.gold != 10_000 - first {
        return Err("a paid refresh did not charge its price".into());
    }
    match ps.refresh_price() {
        RefreshPrice::Gold(next) if next > first => {}
        other => return Err(format!("refresh price did not rise ({first} -> {other:?})")),
    }
    let mut fortuna = PlayerState::new(AstronautKind::Fortuna, save);
    for _ in 0..10 {
        if !fortuna.spend_refresh() || fortuna.gold != 0 {
            return Err("Lady Fortuna paid for a refresh".into());
        }
    }

    // Banish: BANISH_CHARGES charges, strikes the card from the pool for good.
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    if ps.banishes != config::BANISH_CHARGES {
        return Err("wrong starting banish charges".into());
    }
    if ps.banish(&UpgradeOption::GoldPile(20)) {
        return Err("a gold pile was banishable".into());
    }
    let item = ItemKind::ALL[0];
    let own = ps.weapons[0].kind;
    if !ps.banish(&UpgradeOption::NewItem(item, Rarity::Common)) || !ps.banish(&UpgradeOption::WeaponUp(own)) {
        return Err("a banish with charges left was refused".into());
    }
    for _ in 0..300 {
        for o in roll_upgrades(&ps, save, &mut rng) {
            match o {
                UpgradeOption::NewItem(i, _) | UpgradeOption::ItemUp(i, _) if i == item => {
                    return Err("a banished item was dealt again".into());
                }
                UpgradeOption::WeaponUp(w) | UpgradeOption::NewWeapon(w) if w == own => {
                    return Err("a banished weapon was dealt again".into());
                }
                _ => {}
            }
        }
    }
    ps.banishes = 0;
    if ps.banish(&UpgradeOption::NewItem(ItemKind::ALL[1], Rarity::Common)) {
        return Err("banished with no charges".into());
    }

    // Skip: a Gold tip and an XP boost, nothing else.
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    let (gold, xp) = ps.skip_reward();
    let before = (ps.gold, ps.xp, ps.level);
    ps.take_skip();
    if gold == 0 || xp <= 0.0 || ps.gold != before.0 + gold || (ps.xp <= before.1 && ps.level == before.2) {
        return Err("skip did not pay its Gold tip and XP boost".into());
    }

    // Evolution cap: two weapons ready, one slot.
    let pairs: Vec<(WeaponKind, ItemKind)> = WeaponKind::BASE
        .iter()
        .filter_map(|w| match (w.def().evolves_to, w.def().evo_item) {
            (Some(_), Some(i)) => Some((*w, i)),
            _ => None,
        })
        .take(2)
        .collect();
    if pairs.len() < 2 {
        return Err("need two evolvable weapons to test the cap".into());
    }
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    ps.weapons = pairs
        .iter()
        .map(|(w, _)| WeaponInstance { kind: *w, level: config::MAX_WEAPON_LEVEL, cd: 0.0 })
        .collect();
    for (_, i) in &pairs {
        ps.add_item(*i, i.def().rarity);
    }
    if ps.evolvable().len() != 2 {
        return Err("both ready weapons should be evolvable before the cap bites".into());
    }
    if !ps.apply_upgrade(&UpgradeOption::Evolve(pairs[0].0), save, 0) {
        return Err("the first evolution failed".into());
    }
    if !ps.evolvable().is_empty() || ps.apply_upgrade(&UpgradeOption::Evolve(pairs[1].0), save, 0) {
        return Err("a second evolution got past the cap of 1".into());
    }
    if roll_upgrades(&ps, save, &mut rng).iter().any(|o| matches!(o, UpgradeOption::Evolve(_))) {
        return Err("an evolution card was dealt past the cap".into());
    }
    let catalyst_card = UpgradeOption::ItemUp(pairs[1].1, Rarity::Common).body(&ps);
    if catalyst_card.contains("Evo catalyst") || !catalyst_card.contains("Evolution slots full") {
        return Err("an item card still advertised an evolution past the cap".into());
    }
    ps.evo_slots_bonus = 1; // what Tome of Ascension grants
    if ps.evolvable() != vec![pairs[1].0] {
        return Err("an extra evolution slot did not reopen the second evolution".into());
    }
    if !UpgradeOption::ItemUp(pairs[1].1, Rarity::Common).body(&ps).contains("Evo catalyst") {
        return Err("a free evolution slot hid the catalyst hint".into());
    }

    // "One more chance" (§13): one per astronaut per run, only while the option is on.
    let canon = AssistOptions::default();
    let token = AssistOptions { revive_token: true, ..canon };
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    ps.hp = 0.0;
    if ps.try_revive_token(&canon) || ps.revives != 0 {
        return Err("a revive token fired with the option off".into());
    }
    if !ps.try_revive_token(&token) || ps.dead || ps.hp <= 0.0 || ps.iframes < config::REVIVE_TOKEN_IFRAMES {
        return Err("the revive token did not bring the astronaut back".into());
    }
    ps.hp = 0.0;
    if ps.try_revive_token(&token) || ps.revives != 1 {
        return Err("the revive token fired twice in one run".into());
    }
    Ok(())
}
