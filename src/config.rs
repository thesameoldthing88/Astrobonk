//! Central tuning constants. Change numbers here, not in systems.

pub const PLAYER_RUN_SPEED: f32 = 8.5;
pub const PLAYER_ACCEL: f32 = 55.0;
pub const PLAYER_FRICTION: f32 = 38.0;
/// Air control, §4 table: "~60% authority — curve over a crater lip, not cheese a full
/// 180". The share of ground acceleration a wish gets in the air...
pub const PLAYER_AIR_CONTROL: f32 = 0.6;
/// ...and the share of THAT which may push against your own momentum. Curving keeps the
/// whole 60%; braking gets a third of it, so a run-speed hop can be slowed but never turned
/// round before it lands. Neither ever ADDS speed past your run (or slide) speed — see
/// `player::steer`: speed above it is only banked (a slide, a slope, a rail) and kept.
pub const PLAYER_AIR_BRAKE: f32 = 0.35;
pub const PLAYER_JUMP_VEL: f32 = 8.0;
pub const PLAYER_GRAVITY: f32 = 22.0;
pub const PLAYER_HEIGHT: f32 = 1.7;
pub const PLAYER_RADIUS: f32 = 0.45;

pub const SLIDE_BOOST: f32 = 1.65; // multiplier over run speed
pub const SLIDE_TIME: f32 = 0.85;
pub const SLIDE_COOLDOWN: f32 = 1.1;
pub const BHOP_WINDOW: f32 = 0.16; // seconds after landing to keep slide speed
pub const SPEED_HARD_CAP: f32 = 2.1; // × run speed, bhop chains can't exceed

// ── Movement techs (GDD §4) ──────────────────────────────────────────────────
/// Slide slope-boost (§4 "accelerates downhill"): a sliding astronaut is pulled along the
/// ground's fall line at gravity × sin(slope) × this — and slowed the same way uphill, so a
/// crater is a half-pipe, not a treadmill. Walking ignores slope entirely (ground feel).
pub const SLIDE_SLOPE_GAIN: f32 = 1.0;
/// A slide heading down a grade steeper than this (rise per metre) doesn't run out: the
/// whole wall of a crater is one slide.
pub const SLIDE_SUSTAIN_SLOPE: f32 = 0.06;
/// Slide knockback (§4 "knocks back enemies you plow through"): crowd enemies within this
/// reach of a sliding (or grinding) astronaut are shoved aside at this speed (m/s, decaying
/// at 7/s like every knock ≈ 2 m). Pure physics — no damage, and the astronaut never slows.
pub const SLIDE_PLOW_REACH: f32 = 1.2;
pub const SLIDE_PLOW_KNOCK: f32 = 15.0;

/// Orbital Slingshot's Slam (§4): holding slide this long in the air (after pressing it
/// there) commits the dive — long enough that a tap meant for a landing slide doesn't
/// throw a jump away.
pub const SLAM_HOLD_SECS: f32 = 0.12;
/// No slam from a hop that has barely left the ground.
pub const SLAM_MIN_HEIGHT: f32 = 0.6;
/// The dive: straight down at this speed, keeping this share of the horizontal speed —
/// the rest is what the shockwave is made of.
pub const SLAM_DIVE_SPEED: f32 = 30.0;
pub const SLAM_KEEP: f32 = 0.15;
/// "Whiff the ramp, whiff the bomb": the banked speed, in units of the slammer's OWN run
/// speed (base × move-speed stat), maps onto the shockwave from nothing at SLAM_MIN_POWER
/// to the full bomb at SLAM_FULL_POWER (redline — the bhop hard cap). The floor sits just
/// over a slide's own boost, so a slide-jump off flat ground is a dud: the bomb is made of
/// the speed a slope (or a hot rail) added on top of it.
pub const SLAM_MIN_POWER: f32 = SLIDE_BOOST + 0.05;
pub const SLAM_FULL_POWER: f32 = SPEED_HARD_CAP;
/// Speed builds: a redline that is faster in metres per second hits harder — the bomb's
/// damage scales with the move-speed stat, up to this.
pub const SLAM_SPEED_SCALE_MAX: f32 = 2.0;
/// The shockwave at full power: damage (× the slammer's damage multiplier, rolls crits),
/// radius (grows from MIN at the first useful speed) and outward knock (m/s).
pub const SLAM_DAMAGE: f32 = 90.0;
pub const SLAM_RADIUS_MIN: f32 = 3.0;
pub const SLAM_RADIUS_MAX: f32 = 7.0;
pub const SLAM_KNOCK: f32 = 26.0;
/// The slammer's own screen: a kick of shake that grows with the bomb (§13 budget — a
/// full-power slam sits with the elite/boss slams, a dud barely registers).
pub const SLAM_SHAKE_MIN: f32 = 0.06;
pub const SLAM_SHAKE_MAX: f32 = 0.26;
/// The shockwave's ring grows to its radius over this long.
pub const SLAM_RING_SECS: f32 = 0.35;

/// Grind-Lines (§4): a rail runs at base run speed × move speed × this ("+50% speed"). A
/// faster entry keeps its extra and bleeds it off slowly; a slower one is brought up fast.
pub const GRIND_SPEED_MULT: f32 = 1.5;
pub const GRIND_ACCEL: f32 = 18.0;
pub const GRIND_OVERSPEED_DECAY: f32 = 2.5;
/// Catching a rail: a sliding astronaut within this arc of a spine, no higher than
/// GRIND_CATCH_HEIGHT off the ground, heading within ~60° of the rail's run.
pub const GRIND_CATCH_ARC: f32 = 1.1;
pub const GRIND_CATCH_HEIGHT: f32 = 0.9;
pub const GRIND_ALIGN_MIN: f32 = 0.5;
/// After leaving a rail (jumped, ran off its end) it can't catch you again for this long.
pub const GRIND_RECATCH_SECS: f32 = 0.4;
/// The rail stands this far above the crest, and a grinder's boots ride on it.
pub const GRIND_RAIL_LIFT: f32 = 0.22;
/// Spine extraction (`sphere::Terrain::ridge_spines`): the ridge term a crest must hold to
/// carry a rail, the shortest rail worth laying, the trace's step, the sharpest turn per
/// step, and how many seed points cover the sphere.
pub const GRIND_CREST_MIN: f32 = 0.1;
pub const GRIND_MIN_LEN: f32 = 15.0;
pub const GRIND_STEP: f32 = 1.5;
pub const GRIND_MAX_TURN_DEG: f32 = 35.0;
pub const GRIND_SEEDS: usize = 12000;
/// A rail stops this far short of a solid prop rather than running through it.
pub const GRIND_PROP_CLEARANCE: f32 = 0.6;
/// Chests, shrines and vendors are placed at least this far off a rail (inside the rail
/// lookup's ~4 m reach), so none stands on one.
pub const GRIND_INTERACT_CLEARANCE: f32 = 2.5;
/// Metres an interactable (pot, chest, shrine…) keeps clear of a solid prop's rim (L7).
pub const INTERACT_PROP_CLEARANCE: f32 = 1.0;
/// Great-circle metres around the drop point (`Vec3::Y`, where every stage starts) kept free
/// of props, so the first view of a planet is never from inside a boulder (the chase camera
/// sits 7.5 m back and avoids terrain, not props).
pub const START_CLEAR_ARC: f32 = 12.0;
/// Within this arc of a spine (it must be inside the rail lookup's reach, ~4 m), a player
/// who has not ridden one yet this run is told how.
pub const GRIND_HINT_ARC: f32 = 4.0;

/// Antipode Blink (§4, §7): recharge at the item's native grade; a bigger grade (or a
/// second copy) divides it, down to BLINK_MIN_COOLDOWN. Boomerang Insurance's escape rides
/// the same recharge (one mechanic, §15) — alone, at the base.
pub const BLINK_COOLDOWN: f32 = 14.0;
pub const BLINK_MIN_COOLDOWN: f32 = 5.0;
/// A blink lands with this much grace: as long as the camera's glide to the far side
/// takes (CAM_TELEPORT_GLIDE_SECS), so nothing hits you before you can see it.
pub const BLINK_IFRAMES: f32 = 0.5;
/// The antipode read (§4 "the off-screen threat ring shows antipode density"): enemies
/// within this arc of the far pole, recounted this often. Bands: under THIN is clear, from
/// WALL up it's a wall.
pub const ANTIPODE_SCAN_ARC: f32 = 14.0;
pub const ANTIPODE_SCAN_SECS: f32 = 0.25;
pub const ANTIPODE_THIN: u32 = 3;
pub const ANTIPODE_CROWDED: u32 = 10;
pub const ANTIPODE_WALL: u32 = 25;

pub const CAM_DISTANCE: f32 = 7.5;
pub const CAM_HEIGHT: f32 = 3.2;
pub const CAM_STIFFNESS: f32 = 14.0;
/// Arc metres the local body may jump in one frame (beyond what its speed covers) before
/// the camera treats it as a teleport — a Tether rewind, a blink — and glides across.
pub const CAM_TELEPORT_ARC: f32 = 3.0;
/// How long that glide takes: long enough to read as a move, short enough not to leave
/// you blind to the horde at the landing spot.
pub const CAM_TELEPORT_GLIDE_SECS: f32 = 0.45;

/// How hard a co-op teammate's drawn pose chases the last replicated snapshot. Raise it and
/// motion becomes stuttery in lockstep with the packet rate; lower it and teammates lag.
pub const REMOTE_SMOOTH_RATE: f32 = 14.0;
/// Arc metres of error past which we teleport a teammate instead of easing (stage changes,
/// teleporter use — easing across half a planet would look like a ghost gliding through it).
pub const REMOTE_SNAP_ARC: f32 = 8.0;
pub const CAM_SENS: f32 = 0.0032;

pub const ENEMY_CAP: usize = 1200;
pub const ENEMY_SEPARATION_CELL: f32 = 2.2;
pub const SPAWN_ARC_MIN: f32 = 42.0; // meters beyond the player (over the horizon)
pub const SPAWN_ARC_MAX: f32 = 58.0;
pub const CONTACT_TICK: f32 = 0.5; // seconds between contact hits from one enemy
/// A Beamer's telegraph: how long it paints the aim line, and the final stretch where the
/// line stops tracking and holds (the dodge window). A co-op client derives the same sweep
/// from these, so they are the one copy of the numbers.
pub const BEAMER_CHARGE_SECS: f32 = 1.1;
pub const BEAMER_LOCK_SECS: f32 = 0.25;
/// A Burrower's ambush: seconds underground (its crack decal spreads for exactly this long,
/// on every machine), and the eruption's reach.
pub const BURROW_SECS: f32 = 1.3;
pub const BURROW_ERUPT_RADIUS: f32 = 2.6;
/// A boss slam's ring only hurts OUTSIDE this share of its radius — inside is the dodge.
pub const SLAM_SAFE_FRACTION: f32 = 0.35;
/// How far above the ground telegraph rings float: enough to stay above small bumps in the
/// terrain, low enough to read as painted on it.
pub const TELEGRAPH_LIFT: f32 = 0.3;
/// A Beamer's aim line length, metres.
pub const AIM_LINE_LEN: f32 = 24.0;

pub const GEM_CAP: usize = 550;
pub const PICKUP_BASE_RANGE: f32 = 3.2;
pub const PICKUP_FLY_SPEED: f32 = 26.0;

pub const STAGE_SECONDS: [f32; 3] = [600.0, 540.0, 480.0];
pub const MINIBOSS_MARKS: [f32; 2] = [420.0, 120.0]; // timer values (counting down)
pub const BOSS_MARK: f32 = 90.0;

// ── Difficulty scaling (GDD §3 "Difficulty scaling model") ────────────────────
// t = run minutes (whole chain), d = chain depth (stage index), T = planet threat,
// Δ = Difficulty points. See `run::scaling` for the formulas these feed.
/// HP_base / DMG_base anchors: multipliers on every `EnemyDef` number, which the §15 ledger
/// says to anchor on ("from code"). Pinned at 1 by the cold open ("I have room"): the
/// trickle is one Shambler (14 HP) a second, which the Wrench (14), Boomerang (15) and
/// Rocket Pod (26) starters one-shot; any anchor above 1 doubles the swings a level-1 hero
/// needs per Shambler. The price: late T1 is softer than the retired ad-hoc curve (×2.7 at
/// 10:00 vs ×8). But `t` runs across the whole chain, so a T3 finale reaches about ×11 where
/// the old curve reset to ×1 at every teleporter. `--headless N --balance` prints the horde's
/// live numbers against the party's kill rate.
pub const SCALE_HP_BASE: f32 = 1.0;
pub const SCALE_DMG_BASE: f32 = 1.0;
pub const SCALE_HP_T: f32 = 0.11;
pub const SCALE_HP_EXP: f32 = 1.35;
pub const SCALE_HP_D: f32 = 0.20;
pub const SCALE_HP_DELTA: f32 = 0.06;
pub const SCALE_DMG_T: f32 = 0.08;
pub const SCALE_DMG_D: f32 = 0.15;
pub const SCALE_DMG_DELTA: f32 = 0.05;
pub const SCALE_RATE_T: f32 = 0.14;
pub const SCALE_RATE_D: f32 = 0.10;
pub const SCALE_RATE_DELTA: f32 = 0.04;
pub const ELITE_CHANCE_T: f32 = 0.02;
pub const ELITE_CHANCE_D: f32 = 0.05;
pub const ELITE_CHANCE_DELTA: f32 = 0.03;
pub const ELITE_CHANCE_CAP: f32 = 0.35;
/// The sheet's Difficulty stat is a fraction ("+15% Difficulty"); Δ counts it in points of
/// +10%, so one Cursed Moon Rock (+15%) is Δ 1.5 and a maxed Cursed Tome (+40%) is Δ 4.
pub const DIFFICULTY_POINTS_PER_UNIT: f32 = 10.0;
/// Δ added per Tier above 1 — a longer chain is a harder run from its first stage.
pub const TIER_DIFFICULTY_POINTS: f32 = 1.0;
/// Elites are rolled on a cadence, not per crowd enemy: at the formula's 10-35% a per-body
/// roll would make a tenth of a 1,200 horde loot-guaranteed 8× HP giants. One roll every
/// 6 s puts a mid-stage elite at ~60 s — the §3 "chest/elite ping" rhythm.
pub const ELITE_ROLL_SECS: f32 = 6.0;
/// Past the cold open, never go longer than this without an elite (early t rolls ~0%).
pub const ELITE_PITY_SECS: f32 = 75.0;
/// Rate_base: the §3 run-arc beats as (stage timer left, spawns/s) keypoints, linearly
/// interpolated. Cold open trickle → first escalation → miniboss → density ramp → hazards
/// live → endgame. Keyed on the countdown so shorter chained stages join the arc mid-way.
pub const SPAWN_RATE_BEATS: [(f32, f32); 7] = [
    (600.0, 1.0),
    (540.0, 1.8),
    (450.0, 4.0),
    (420.0, 4.4),
    (270.0, 7.2),
    (150.0, 9.0),
    (0.0, 10.0),
];
/// "Hold/miniboss": while a miniboss is up the horde stops escalating around it.
pub const SPAWN_HOLD_MULT: f32 = 0.7;
/// "Exhale/loot": after any boss falls, the horde thins for a breath so the reward lands.
pub const SPAWN_EXHALE_MULT: f32 = 0.35;
pub const SPAWN_EXHALE_SECS: f32 = 15.0;
/// The Static's own base rate (then scaled like everything else) — it never stops growing.
pub const STATIC_RATE_BASE: f32 = 4.2;
pub const STATIC_RATE_GROWTH: f32 = 0.065; // per second of overtime
/// Seconds of the §3 spawn-mix arc a chained world starts ahead per step of depth, on top
/// of joining the arc where its shorter countdown begins (`scaling::mix_secs`).
pub const MIX_DEPTH_HEAD_START_SECS: f32 = 60.0;
/// The HUD's horizon test (`hud::update_edge_markers`) treats the planet as a ball this
/// many metres under its mean surface (or under the target's own ground, if lower): what
/// the camera-to-target line passes through that ball for is over the horizon.
pub const EDGE_MARKER_OCCLUDER_INSET: f32 = 0.5;
/// The overflow valve (`enemies::director_spawn`, GDD §9). A crowd enemy this many arc
/// metres from every astronaut is over any horizon — the spawn band is 42–58 m, and on the
/// Dark Moon, the smallest world, nothing is more than 330 m away — so dissolving it into The Static costs
/// the fight nothing, and its slot spawns fresh where the players are.
pub const STATIC_RECYCLE_ARC: f32 = 100.0;
/// Spawns the cap had no room for, banked for The Static at most this many per stage.
pub const STATIC_BACKLOG_MAX: f32 = 300.0;
/// Extra ghosts per second the backlog adds once The Static rises (room permitting).
pub const STATIC_BACKLOG_DRAIN: f32 = 4.0;
/// Backlog size at which the host is told The Static is gathering (once per stage).
pub const STATIC_GATHERING_TELL: f32 = 40.0;
/// Party spawn scaling (GDD §11): 100 / 175 / 240 / 300 %.
pub const PARTY_SPAWN_SCALE: [f32; 4] = [1.0, 1.75, 2.4, 3.0];

// ── Silver payout (GDD §10) ──────────────────────────────────────────────────
pub const SILVER_SURVIVAL_SECS_PER: f32 = 6.0; // 1 Silver per 6 s survived
pub const SILVER_KILLS_PER: f32 = 4.0; // 1 Silver per 4 kills
pub const SILVER_PER_BOSS: u64 = 150;
pub const SILVER_PER_TIER: u64 = 50;
pub const SILVER_PER_STATIC_SEC: f32 = 1.0;
pub const SILVER_GOLDEN_TOME_PER_LEVEL: f32 = 0.05;
pub const SILVER_CURSED_ROCK_EACH: f32 = 0.15;

// ── Tomes (GDD §7) ───────────────────────────────────────────────────────────
/// Ranks per tome, and the Silver price of the next one: `100 × 1.6^rank` (a full tome is
/// ~18,000 Silver — "even a bad run funds a fraction of a tier").
pub const TOME_MAX_RANK: u32 = 10;
pub const TOME_COST_BASE: f32 = 100.0;
pub const TOME_COST_GROWTH: f32 = 1.6;
/// Loadout slots every save starts with ("limited to 4 of the pool"); quest rewards widen
/// it (`Reward::TomeSlot`) up to TOME_SLOTS_MAX.
pub const TOME_BASE_SLOTS: u32 = 4;
pub const TOME_SLOTS_MAX: u32 = 8;
/// Tome of Encirclement counts the foes within this many metres of you, up to this many.
pub const TOME_CROWD_RADIUS: f32 = 8.0;
pub const TOME_CROWD_CAP: u32 = 30;
/// Tome of Momentum: the bonus builds over this many seconds of unbroken movement above
/// MOMENTUM_MIN_SPEED and drains MOMENTUM_DRAIN× as fast once you stop — a landing or a
/// sharp turn does not wipe it, standing around does.
pub const MOMENTUM_RAMP_SECS: f32 = 6.0;
pub const MOMENTUM_MIN_SPEED: f32 = 3.0;
pub const MOMENTUM_DRAIN: f32 = 3.0;
/// Tome of Ricochet: a shot that comes down skips once — a fresh leg of RICOCHET_LIFE s
/// (× Duration) aimed at the nearest foe within RICOCHET_SEEK m, hopping RICOCHET_HOP m up
/// over RICOCHET_HOP_SECS so the skip reads.
pub const RICOCHET_LIFE: f32 = 1.1;
pub const RICOCHET_SEEK: f32 = 16.0;
pub const RICOCHET_HOP: f32 = 1.3;
pub const RICOCHET_HOP_SECS: f32 = 0.3;
/// Tome of the Horizon: XP lying more than this arc from you is over the horizon (§4: "the
/// horizon is a curved lip ~40 m out"). After `1 / HorizonCollect` s on the ground it flies
/// home along the surface at HORIZON_FLY_SPEED, lifting up to HORIZON_FLY_LIFT m so a gem
/// crossing the planet reads as a streak rather than a crawl through the dirt.
/// The flight sets off at HORIZON_FLY_START m/s, gains HORIZON_FLY_ACCEL m/s every second
/// up to HORIZON_FLY_SPEED, and within HORIZON_HANDOFF_ARC m of its caller the ordinary
/// magnet flight takes it in (host and client share these, so both draw one flight).
pub const HORIZON_ARC: f32 = 40.0;
pub const HORIZON_FLY_START: f32 = 8.0;
pub const HORIZON_FLY_ACCEL: f32 = 40.0;
pub const HORIZON_FLY_SPEED: f32 = 60.0;
pub const HORIZON_FLY_LIFT: f32 = 3.0;
pub const HORIZON_HANDOFF_ARC: f32 = 2.5;
/// The astronaut's weapon-mounted flashlight (§4, §13: toggled with F). Tome of Nightfall
/// multiplies the intensity by its Flashlight stat and the reach by half as much again (a 2×
/// beam throws 1.5× as far).
pub const FLASHLIGHT_INTENSITY: f32 = 6_000_000.0;
pub const FLASHLIGHT_RANGE: f32 = 55.0;

pub const XP_BASE: f32 = 6.0;
pub const XP_PER_LEVEL: f32 = 3.4;
pub const XP_QUAD: f32 = 0.18;

pub const CHEST_BASE_COST: u64 = 25;
pub const CHEST_COST_GROWTH: f32 = 1.75;
/// Luck added to the guaranteed miniboss-#1 cache roll: the first build fork should feel
/// like a reward, not another Common.
pub const REWARD_CACHE_LUCK: f32 = 0.35;
/// Items offered by the miniboss cache (pick one).
pub const REWARD_CACHE_CHOICES: usize = 3;

// ── Items (GDD §7) ───────────────────────────────────────────────────────────
/// Effect added per Rarity Grade an item is rolled ABOVE its native grade: a Rare-native
/// item rolled Legendary carries ×2.0 of the same effect (§7 "a bigger roll of the same
/// effect"). Cursed items never roll, so they always carry ×1.
pub const ITEM_GRADE_STEP: f32 = 0.5;
/// Stack limit for the items §7 lets stack "additively and near-infinitely". A number, not
/// "no cap", so the card can still say (n/cap) — nobody reaches it in a 30-minute chain.
pub const ITEM_STACKS_UNCAPPED: u32 = 99;
/// Share of chest / vendor / shrine rolls that come up Cursed, before the normal grade roll.
/// Luck never touches it: Cursed is a temptation, not a reward tier.
pub const CURSED_LOOT_CHANCE: f64 = 0.07;
/// A Cursed item's chance to make each of its level-up pool slots (commons pass at 0.9).
pub const CURSED_CARD_PASS: f64 = 0.2;
/// Shady Guy price of a Cursed item: cheap, because the price is paid in risk.
pub const CURSED_ITEM_PRICE: f32 = 90.0;
/// Share of a loot roll at grade G that deals an item NATIVE to G; the rest is a
/// lower-grade item rolled up to G (a Legendary roll is mostly Legendaries, sometimes a
/// huge Borgar). A share, not a per-item weight, so the 4 Legendaries aren't outvoted by
/// the 28 items below them.
pub const GRADE_NATIVE_SHARE: f32 = 0.75;

/// Orbital Yo-Yo: every YOYO_PERIOD s a chunk orbits you once, hitting each foe once for
/// YOYO_DAMAGE. The swing radius scales with the planet (§7 "bigger world, wider swing"):
/// YOYO_RADIUS on a planet of YOYO_REF_PLANET_RADIUS.
pub const YOYO_PERIOD: f32 = 4.0;
pub const YOYO_DAMAGE: f32 = 40.0;
pub const YOYO_RADIUS: f32 = 5.0;
pub const YOYO_REF_PLANET_RADIUS: f32 = 140.0;
pub const YOYO_ORBIT_SECS: f32 = 1.1;
/// Extra stacks add chunks up to this many (spaced evenly), sharing the stack's damage.
pub const YOYO_MAX_CHUNKS: u32 = 4;

/// Comet Tail: a burning patch every COMET_TAIL_DROP_SECS while moving faster than
/// COMET_TAIL_MOVING_SPEED; patches live COMET_TAIL_LIFE — longer than the stand-still, so
/// the loop you just ran is still burning when you stop and ignite it.
pub const COMET_TAIL_DROP_SECS: f32 = 0.25;
pub const COMET_TAIL_LIFE: f32 = 4.0;
pub const COMET_TAIL_RADIUS: f32 = 1.4;
pub const COMET_TAIL_DPS: f32 = 14.0;
pub const COMET_TAIL_TICK: f32 = 0.5;
pub const COMET_TAIL_MOVING_SPEED: f32 = 1.5;
/// Stand (nearly) still this long to ignite the trail around you.
pub const COMET_TAIL_STILL_SECS: f32 = 2.0;
pub const COMET_TAIL_STILL_SPEED: f32 = 0.6;
/// Ignition: a burst at every live patch and a ring around you; patches then burn hotter.
pub const COMET_TAIL_IGNITE_DAMAGE: f32 = 45.0;
pub const COMET_TAIL_IGNITE_RADIUS: f32 = 4.5;
pub const COMET_TAIL_IGNITED_DPS_MULT: f32 = 2.0;

/// The Overheat: every Nth volley jams all guns for OVERHEAT_JAM_SECS.
pub const OVERHEAT_JAM_EVERY: u32 = 10;
pub const OVERHEAT_JAM_SECS: f32 = 1.0;

/// Downhill Momentum: +1% damage per metre of altitude lost over the last second.
pub const DOWNHILL_DMG_PER_M: f32 = 0.01;
pub const DOWNHILL_WINDOW_SECS: f32 = 1.0;

/// Second Astronaut: the ghost fires one of your weapons at this share of its damage, and
/// picks a new one every GHOST_SWAP_SECS.
pub const GHOST_MIRROR: f32 = 0.5;
pub const GHOST_SWAP_SECS: f32 = 20.0;

/// Encirclement Bonus: +2% per compass octant (of 8) with an enemy inside ENCIRCLE_RADIUS.
pub const ENCIRCLE_DMG_PER_DIR: f32 = 0.02;
pub const ENCIRCLE_RADIUS: f32 = 12.0;
pub const ENCIRCLE_SCAN_SECS: f32 = 0.2;

/// Icarus Boots: +40% damage airborne, −10% grounded.
pub const ICARUS_AIR_BONUS: f32 = 0.40;
pub const ICARUS_GROUND_PENALTY: f32 = 0.10;

/// Anti-Grav Boots: hover budget per airtime (refilled on landing), and the fewest shots a
/// straight-firing weapon puts in its 360° ring — a "ring" of one shot is not a ring.
pub const ANTIGRAV_HOVER_SECS: f32 = 2.0;
pub const ANTIGRAV_RING_SHOTS: u32 = 6;
/// Beams fan out into this many spokes while airborne.
pub const ANTIGRAV_RING_BEAMS: u32 = 3;

/// Little Black Hole: every BLACK_HOLE_PERIOD s, a singularity opens BLACK_HOLE_AHEAD m in
/// front of you and drags everything within BLACK_HOLE_RADIUS into it over the pull.
pub const BLACK_HOLE_PERIOD: f32 = 6.0;
pub const BLACK_HOLE_RADIUS: f32 = 8.0;
pub const BLACK_HOLE_AHEAD: f32 = 3.0;
pub const BLACK_HOLE_PULL_SECS: f32 = 0.9;
pub const BLACK_HOLE_MAX_PULL_SPEED: f32 = 26.0;

/// Dead Man's Tether: rewind this far, and stay untouchable this long after.
pub const TETHER_REWIND_SECS: f32 = 3.0;
pub const TETHER_IFRAMES: f32 = 1.5;

/// Widow's Ring: +30% to every stat while at 1 HP (below WIDOW_HP_BAND, since HP is
/// fractional). Its 1-HP save recharges, or it would be immortality with extra steps.
pub const WIDOW_STAT_BONUS: f32 = 0.30;
pub const WIDOW_HP_BAND: f32 = 1.5;
pub const WIDOW_SAVE_COOLDOWN: f32 = 20.0;
pub const WIDOW_IFRAMES: f32 = 1.0;

/// Boomerang Insurance arms its antipode escape when a hit leaves you under this HP share.
pub const BOOMERANG_INSURANCE_HP: f32 = 0.20;

/// The Static Radio: The Static arrives this many seconds early, and angrier.
pub const STATIC_RADIO_LEAD_SECS: f32 = 90.0;
pub const STATIC_RADIO_HP: f32 = 1.35;
pub const STATIC_RADIO_DMG: f32 = 1.25;
pub const STATIC_RADIO_RATE: f32 = 1.3;

/// Signal Flare: the horde rates your distance at this fraction when picking whom to chase,
/// and your share of each wave lands this close.
pub const SIGNAL_FLARE_LURE: f32 = 0.5;
pub const SIGNAL_FLARE_SPAWN_ARC_MIN: f32 = 30.0;
pub const SIGNAL_FLARE_SPAWN_ARC_MAX: f32 = 42.0;

/// Devoured Sun Shard: every SUN_SHARD_PERIOD s the day side shrinks one SUN_SHARD_STEP
/// toward total night (RunState::sun_shrink, 0..1).
pub const SUN_SHARD_PERIOD: f32 = 60.0;
pub const SUN_SHARD_STEP: f32 = 0.1;

pub const WEAPON_SLOTS: usize = 4;
pub const MAX_WEAPON_LEVEL: u32 = 7;
/// Evolved weapons one astronaut may own per run (GDD §15 ledger). Tome of Ascension adds
/// one more through `PlayerState::evo_cap`.
pub const EVOLUTION_CAP: u32 = 1;

// ── Level-up choice economy (GDD §3 table, §15 ledger) ───────────────────────
/// Cards dealt per level-up. The §15 ledger sets no count, so the shipped four stay.
pub const LEVELUP_CARDS: usize = 4;
/// Free Refreshes per run; after that each Refresh costs Gold.
pub const FREE_REFRESHES: u32 = 2;
/// Gold price of the first PAID refresh, growing by REFRESH_COST_GROWTH per paid use —
/// chasing a card is allowed, but it competes with chests for the same Gold.
pub const REFRESH_BASE_COST: u64 = 15;
pub const REFRESH_COST_GROWTH: f32 = 1.5;
/// Banish charges per run.
pub const BANISH_CHARGES: u32 = 3;
/// Skip pays a small XP boost (this fraction of the current level's bar) plus a Gold tip
/// that grows with level so it stays worth a thought late in the run.
pub const SKIP_XP_FRACTION: f32 = 0.2;
pub const SKIP_GOLD_BASE: f32 = 8.0;
pub const SKIP_GOLD_PER_LEVEL: f32 = 1.0;

pub const DAMAGE_NUMBER_POOL: usize = 64;

pub const INTERACT_RANGE: f32 = 3.0;

// Comet Combo — the signature scored lap-kill.
pub const WAKE_RADIUS: f32 = 14.0; // enemies this close (and behind you) join the tail
pub const COMET_MIN_TAIL: u32 = 8; // tail must reach this to start charging
pub const COMET_CHARGE_GOAL: f32 = 900.0; // charge = Σ(tail_count · speed · dt)
pub const COMET_RADIUS: f32 = 22.0; // cash-out detonation radius
pub const COMET_GRACE: f32 = 0.7; // seconds the tail can dip before the combo breaks

pub const SAVE_DIR: &str = "astrobonk";
pub const SAVE_FILE: &str = "save.json";

// ── Co-op enemy streaming ────────────────────────────────────────────────────
// A client cannot receive 1200+ enemies as replicated ENTITIES, so the crowd rides a
// custom quantized batch message. Positions are sent as a great-circle offset from the
// receiving client's OWN astronaut, in that astronaut's tangent frame, in arc metres —
// on a sphere a position is a direction, so two numbers beat three floats.

/// Half-width of the quantized position range, in arc metres. Must exceed the largest
/// interest radius. 16 bits over ±128 m gives a 3.9 mm step — the slowest enemy (Lobber,
/// 1.8 m/s) still advances ~31 steps per snapshot, so nothing stair-steps.
pub const NET_ENEMY_RANGE: f32 = 128.0;
/// Snapshot rate. Matches NET_ENEMY_SMOOTH_RATE's time constant, so the interpolation
/// filter is itself the reconstructor and 30 Hz would buy nothing visible.
pub const NET_ENEMY_HZ: f32 = 15.0;
/// Inside this arc, an enemy is sent EVERY snapshot. Comfortably covers the whole combat
/// volume (auto-target 40 m, longest weapon 26 m, director spawn band 42–58 m).
pub const NET_ENEMY_NEAR_ARC: f32 = 50.0;
/// Enters interest at IN, leaves at OUT — the gap is hysteresis so an enemy pacing the
/// boundary doesn't spawn/despawn every snapshot. Indexed like PlanetKind::ALL.
pub const NET_ENEMY_INTEREST_IN: [f32; 3] = [95.0, 110.0, 82.0];
pub const NET_ENEMY_INTEREST_OUT: [f32; 3] = [107.0, 122.0, 93.0];
/// Hard ceiling on records per snapshot — the bandwidth backstop.
pub const NET_ENEMY_MAX_RECORDS: usize = 1200;
/// Payload bytes per chunk. Under renet's SLICE_SIZE (1200) so a chunk is never
/// auto-fragmented, which would make it all-or-nothing on an unreliable channel.
pub const NET_ENEMY_CHUNK_BYTES: usize = 1024;
/// How hard a proxy chases its last streamed position (framerate-independent).
pub const NET_ENEMY_SMOOTH_RATE: f32 = 12.0;
/// Arc error past which a proxy teleports instead of easing.
pub const NET_ENEMY_SNAP_ARC: f32 = 12.0;
/// Seconds an unseen proxy survives before despawning — long enough that a dropped
/// unreliable packet doesn't flicker the horde.
pub const NET_ENEMY_GRACE: f32 = 2.0;

// ── Co-op session lifecycle ──────────────────────────────────────────────────
/// Real seconds between the host sending SessionEndMsg and dropping the transport. Disconnect
/// packets go out instantly, so without this gap they overtake the reliable goodbye and a
/// joiner only ever learns "connection lost", never why.
pub const NET_SESSION_CLOSE_SECS: f32 = 0.5;
/// A connected client that has heard NOTHING from the host for this long (not even renet's
/// keep-alives, which flow while the host is paused or on a menu) treats it as gone. netcode's
/// own timeout is 15 s — a long time to stare at a frozen planet.
pub const NET_HOST_SILENCE_SECS: f32 = 6.0;
/// CLIENT reconciliation: how far the host's authoritative copy of our astronaut may sit
/// off the path we ran before we pull ourselves toward it. Measured against our recent PATH,
/// not our current position — the copy trails us by speed × latency (metres at a low frame
/// rate or mid-bhop), and correcting that lag would drag every joiner backwards. What it
/// catches is real divergence: a host hitstop or open panel that froze our server-side body
/// while we ran on, a lost jump packet.
pub const NET_RECONCILE_DEADZONE: f32 = 1.0;
/// How much of our own path to remember for that comparison. Must exceed the copy's lag.
pub const NET_RECONCILE_WINDOW_SECS: f32 = 1.0;
/// How fast the excess beyond the deadzone is closed, per second (exponential, so a big
/// error eases home in about a second instead of snapping the camera).
pub const NET_RECONCILE_RATE: f32 = 2.5;

// ── Accessibility (GDD §13 "Accessibility — humane by default") ─────────────
/// UI scale range, §13 "UI scale 75–150%".
pub const UI_SCALE_MIN: f32 = 0.75;
pub const UI_SCALE_MAX: f32 = 1.5;
/// The smallest canvas, in UI units, every screen is laid out to hold: 1280x720 at the 150%
/// maximum. A window too small for the chosen scale gets the largest scale that still gives
/// the UI this much room (1280x720 and the Steam Deck's 1280x800 both allow the full 150%).
pub const UI_FIT_CANVAS: (f32, f32) = (1280.0 / 1.5, 720.0 / 1.5);
/// In-run choice panels (level-up, chest, shop) keep clear of the HUD's top band (timer) and
/// bottom band (weapon tray, HP, XP, level), in UI units — the HUD scales with the UI too.
pub const HUD_TOP_BAND: f32 = 64.0;
/// The top band while a boss bar is up under the timer (80 px + its name line + the bar).
pub const HUD_TOP_BAND_BOSS: f32 = 120.0;
pub const HUD_BOTTOM_BAND: f32 = 118.0;
/// Damage-number size multiplier range.
pub const NUMBER_SIZE_MIN: f32 = 0.6;
pub const NUMBER_SIZE_MAX: f32 = 2.0;
/// Full mode's merge rule, §13: hits within 0.3 m / 0.1 s coalesce into a running sum...
pub const NUMBER_MERGE_RADIUS_FULL: f32 = 0.3;
pub const NUMBER_MERGE_SECS_FULL: f32 = 0.1;
/// ...and "at high counts" — once this share of the pool is on screen — every hit folds
/// into the nearest live number within NUMBER_MERGE_RADIUS, which is Merged mode's rule.
pub const NUMBER_CROWDED_FRACTION: f32 = 0.75;
pub const NUMBER_MERGE_RADIUS: f32 = 1.6;
/// Photosensitivity mode: nothing that strobes may repeat faster than this. WCAG 2.3.1
/// allows three flashes in any one second; 0.36 s keeps every source under 2.8 a second.
pub const PHOTO_MIN_FLASH_INTERVAL: f32 = 0.36;
/// Telegraph rings pulse faster as impact nears (Hz at spawn → at impact). Photosensitivity
/// caps the rate below three a second.
pub const TELEGRAPH_PULSE_HZ: (f32, f32) = (1.5, 5.0);
pub const TELEGRAPH_PULSE_HZ_PHOTO: f32 = 2.5;
pub const TELEGRAPH_PULSE_AMP: f32 = 0.07;
/// Beamer aim lines: dashes per line, and how fast they march toward the target while the
/// line is still tracking. A locked line stops marching — the "dodge now" tell.
pub const AIM_DASHES: usize = 12;
pub const AIM_DASH_SPEED: f32 = 9.0;
/// Camera bloom at rest and under flash reduction (Bloom::NATURAL is 0.15). The toon look
/// keeps it sparing: flat cel colors read cleaner without a haze over them.
pub const BLOOM_INTENSITY: f32 = 0.1;
pub const BLOOM_INTENSITY_REDUCED: f32 = 0.04;
/// ...and only the brightest pixels feed it (soft knee), so a lit suit or a pale rock never
/// glows — suns, stars, glows and hit flashes still do.
pub const BLOOM_THRESHOLD: f32 = 0.6;
pub const BLOOM_THRESHOLD_SOFTNESS: f32 = 0.5;

// ── Toon look (locked direction #7, `toon.rs`) ────────────────────────────────
/// Cel bands on N·L for every lit StandardMaterial: below the first edge a face gets no
/// direct light (ambient only), between the edges the MID level, above the second FULL.
/// Each step is smoothed over ±TOON_BAND_SOFT of N·L so a band edge isn't a jagged pixel stair.
pub const TOON_BAND_EDGES: (f32, f32) = (0.06, 0.38);
pub const TOON_BAND_MID: f32 = 0.3;
pub const TOON_BAND_FULL: f32 = 0.72;
pub const TOON_BAND_SOFT: f32 = 0.035;
/// A lit rim just inside every silhouette turned toward the sun (drawn by the ink pass, so
/// only real edges get it, never a grazing stretch of ground): brightens the surface by this
/// share. Scales with the sun (a shrunken sun, a lower rim); the night side never has one.
pub const TOON_RIM_STRENGTH: f32 = 0.35;
/// Specular turns into a hard cel highlight: glossy parts (visors) keep a crisp spot above
/// this brightness, rough ones (rock, dust, cloth) lose their soft sheen.
pub const TOON_SPEC_EDGE: (f32, f32) = (0.08, 0.14);
/// How much of the terrain's own bumpy normal the ground shades with (the rest is the
/// planet's radial normal), so cel bands follow the world's curve and its big hills instead
/// of speckling across every small bump.
pub const TOON_TERRAIN_NORMAL_DETAIL: f32 = 0.55;
/// The terrain's height colors (crater floor → peak) come in this many flat steps.
pub const TOON_TERRAIN_BANDS: usize = 4;
/// The flashlight cone in two cel steps: a dimmer outer ring at this share, the full core.
/// Edges are in the spot's own 0..1 falloff (1 = inside the inner angle).
pub const TOON_CONE_RING: f32 = 0.4;
pub const TOON_CONE_EDGES: (f32, f32) = (0.03, 0.4);
/// Ink outlines (post-process over the depth + normal prepass): line width in pixels at
/// 720p (scaled with the window height), and ×this in the §13 high-contrast mode.
pub const TOON_INK_PX: f32 = 1.0;
pub const TOON_INK_HIGH_CONTRAST: f32 = 2.0;
/// A silhouette is inked where the depth's second difference, relative to the pixel's own
/// depth, exceeds this (planes are linear in reverse-Z depth, so they never trip it)...
pub const TOON_INK_DEPTH_EDGE: f32 = 0.025;
/// ...and a crease where neighbouring normals turn by more than this (1 − cos), at this
/// opacity — a touch lighter than the silhouette, so an outline still reads as the outside.
pub const TOON_INK_NORMAL_EDGE: f32 = 0.3;
pub const TOON_INK_CREASE_ALPHA: f32 = 0.85;
/// Ink fades out between these view distances (m): the planet's far limb is well inside,
/// the starfield and the sun disc (1400 m+) never get a line.
pub const TOON_INK_FADE: (f32, f32) = (180.0, 320.0);
/// Every glow below lives on UNLIT materials, which draw their base color and ignore
/// emissive (bevy_pbr's unlit branch), so a flash is softened by dimming its base color —
/// the camera bloom then has less to spread, too.
/// The enemy hit-flash under flash reduction: a pale grey instead of pure white. Still a flat
/// read (it is the single most important readability tool, §12), minus the glare that turns
/// a swarm under an aura into a strobing white cloud.
pub const HIT_FLASH_GREY_REDUCED: f32 = 0.62;
/// Particle brightness under flash reduction (share of the canon color).
pub const PARTICLE_BRIGHTNESS_REDUCED: f32 = 0.55;
/// The full-screen hurt tint: peak alpha when a hit takes HP (or shield), and under flash
/// reduction, fading out over HURT_TINT_SECS.
pub const HURT_TINT_PEAK: f32 = 0.22;
pub const HURT_TINT_PEAK_REDUCED: f32 = 0.12;
pub const HURT_TINT_SECS: f32 = 0.4;
/// The evolution white-flash (flash reduction removes it outright).
pub const EVOLVE_FLASH_ALPHA: f32 = 0.75;
pub const EVOLVE_FLASH_SECS: f32 = 0.4;
/// Photosensitivity mode (§13 "disables Storm Core strobe, softens Death Ray bloom"): the
/// DEATH RAY / STORM CORE / Tesla materials drop to this opacity — a dim, see-through beam
/// instead of a saturated slab of light...
pub const PHOTO_WEAPON_ALPHA: f32 = 0.3;
/// ...and a chain zap, instead of popping on for 0.12 s at the fire rate, fades in and out
/// over this long, at most once per PHOTO_MIN_FLASH_INTERVAL on the whole screen.
pub const PHOTO_ZAP_SECS: f32 = 0.45;

// ── Difficulty as options (GDD §13) ───────────────────────────────────────────
/// Enemy density slider: scales the spawn rate AND the live-enemy cap. Only ever eases.
pub const ASSIST_DENSITY_MIN: f32 = 0.25;
/// Enemy damage slider: scales every hit an astronaut takes. Only ever eases.
pub const ASSIST_DAMAGE_MIN: f32 = 0.1;
/// "One more chance": the would-be-lethal hit instead leaves the astronaut at this share of
/// max HP, with these seconds of invulnerability...
pub const REVIVE_TOKEN_HP_FRAC: f32 = 0.5;
pub const REVIVE_TOKEN_IFRAMES: f32 = 3.0;
/// ...and shoves the crowd back this far (knock impulse, m/s, decaying at 7/s ≈ 6 m), so
/// the second chance does not open inside the same ring that closed the first.
pub const REVIVE_NOVA_RADIUS: f32 = 7.0;
pub const REVIVE_NOVA_KNOCK: f32 = 42.0;
