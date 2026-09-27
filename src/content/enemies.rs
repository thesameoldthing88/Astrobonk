use super::planets::PlanetKind;
use crate::config::*;
use bevy::prelude::Color;
use rand::{Rng, SeedableRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnemyKind {
    Shambler,
    Sprinter,
    Bruiser,
    Spitter,
    Ufo,
    Burrower,
    Beamer, // long-range railbolt sniper
    Lobber, // mortar artillery with AoE telegraph
    Ghost,  // The Static
    // ---- GDD §9 new enemies, batch 1 (P08) ----
    Rollo,       // curls up and rolls the great circle; accelerates downhill, turns wide
    Trencher,    // burrows a visible ridge to you, then an uppercut launch
    AegisDrone,  // front shield cone that tracks you slowly — flank it
    Sunskimmer,  // high kamikaze diver, whines as it commits
    BeaconTick,  // harmless touch that plants a tracker: the horde paths to you
    Mimic,       // a chest that bites, shockwaves and flees with your gold
    BeamerPrime, // elite sniper: leads its mark and fires over the curve
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BossKind {
    CraterpillarJr, // miniboss
    RoverGoneWrong, // miniboss
    Craterpillar,   // Moon stage boss
    Anubot,         // Mars stage boss
}

pub struct EnemyDef {
    pub kind: EnemyKind,
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub xp: f32,
    pub scale: f32,
    pub color: Color,
    /// Hover height above terrain (0 = walks).
    pub hover: f32,
    /// Preferred combat range in meters (0 = melee, beelines forever).
    pub standoff: f32,
}

impl EnemyKind {
    pub fn def(&self) -> EnemyDef {
        use EnemyKind::*;
        match self {
            Shambler => EnemyDef {
                kind: *self,
                name: "Shambler",
                hp: 14.0,
                speed: 3.1,
                damage: 5.0,
                xp: 1.0,
                scale: 1.0,
                color: Color::srgb(0.45, 0.75, 0.45),
                hover: 0.0,
                standoff: 0.0,
            },
            Sprinter => EnemyDef {
                kind: *self,
                name: "Sprinter",
                hp: 9.0,
                speed: 6.2,
                damage: 5.0,
                xp: 1.2,
                scale: 0.8,
                color: Color::srgb(0.85, 0.85, 0.30),
                hover: 0.0,
                standoff: 0.0,
            },
            Bruiser => EnemyDef {
                kind: *self,
                name: "Bruiser",
                hp: 70.0,
                speed: 2.2,
                damage: 14.0,
                xp: 4.0,
                scale: 1.7,
                color: Color::srgb(0.70, 0.35, 0.30),
                hover: 0.0,
                standoff: 0.0,
            },
            Spitter => EnemyDef {
                kind: *self,
                name: "Spitter",
                hp: 18.0,
                speed: 2.6,
                damage: 8.0,
                xp: 2.0,
                scale: 1.1,
                color: Color::srgb(0.60, 0.45, 0.85),
                hover: 0.0,
                standoff: 13.0,
            },
            Ufo => EnemyDef {
                kind: *self,
                name: "UFO",
                hp: 24.0,
                speed: 4.5,
                damage: 7.0,
                xp: 2.5,
                scale: 1.0,
                color: Color::srgb(0.55, 0.85, 0.95),
                hover: 4.0,
                standoff: 9.0,
            },
            Burrower => EnemyDef {
                kind: *self,
                name: "Burrower",
                hp: 30.0,
                speed: 5.0,
                damage: 12.0,
                xp: 3.0,
                scale: 1.2,
                color: Color::srgb(0.75, 0.55, 0.35),
                hover: 0.0,
                standoff: 0.0,
            },
            Beamer => EnemyDef {
                kind: *self,
                name: "Beamer",
                hp: 26.0,
                speed: 2.8,
                damage: 14.0,
                xp: 3.0,
                scale: 1.3,
                color: Color::srgb(1.0, 0.30, 0.55),
                hover: 0.0,
                standoff: 22.0,
            },
            Lobber => EnemyDef {
                kind: *self,
                name: "Lobber",
                hp: 45.0,
                speed: 1.8,
                damage: 16.0,
                xp: 3.5,
                scale: 1.5,
                color: Color::srgb(0.55, 0.60, 0.30),
                hover: 0.0,
                standoff: 17.0,
            },
            Ghost => EnemyDef {
                kind: *self,
                name: "The Static",
                hp: 20.0,
                speed: 5.4,
                damage: 10.0,
                xp: 0.0, // drops silver instead
                scale: 1.1,
                color: Color::srgb(0.80, 0.85, 1.00),
                hover: 0.6,
                standoff: 0.0,
            },
            // ---- batch 1 (P08): behaviour lives in `bestiary.rs`, tuning in config.rs ----
            Rollo => EnemyDef {
                kind: *self,
                name: "Rollo",
                hp: 38.0,
                speed: ROLLO_CRUISE_SPEED, // its rolling cruise; slopes push it off this
                damage: 11.0,              // at cruise; scales with how fast it is rolling
                xp: 2.5,
                scale: 1.15,
                color: Color::srgb(0.46, 0.60, 0.72),
                hover: 0.0,
                standoff: 0.0,
            },
            Trencher => EnemyDef {
                kind: *self,
                name: "Trencher",
                hp: 55.0,
                speed: 2.6, // on the surface; it tunnels at TRENCH_TUNNEL_SPEED
                damage: 15.0,
                xp: 3.5,
                scale: 1.3,
                color: Color::srgb(0.55, 0.38, 0.27),
                hover: 0.0,
                standoff: 0.0,
            },
            AegisDrone => EnemyDef {
                kind: *self,
                name: "Aegis Drone",
                hp: 45.0,
                speed: 3.3,
                damage: 9.0,
                xp: 3.0,
                scale: 1.1,
                color: Color::srgb(0.56, 0.60, 0.68),
                hover: 1.0,
                standoff: 0.0,
            },
            Sunskimmer => EnemyDef {
                kind: *self,
                name: "Sunskimmer",
                hp: 16.0,
                speed: 11.0, // high cruise; the dive is timed, not paced
                damage: 18.0, // its blast; it has no contact bite at altitude
                xp: 2.0,
                scale: 1.0,
                color: Color::srgb(0.95, 0.46, 0.20),
                hover: SKIM_ALTITUDE,
                standoff: 0.0,
            },
            BeaconTick => EnemyDef {
                kind: *self,
                name: "Beacon Tick",
                hp: 8.0,
                speed: 5.2,
                damage: 0.0, // harmless-looking: its touch plants a tracker instead
                xp: 1.0,
                scale: 0.6,
                color: Color::srgb(0.32, 0.32, 0.36),
                hover: 0.0,
                standoff: 0.0,
            },
            Mimic => EnemyDef {
                kind: *self,
                name: "Mimic Chest",
                hp: 90.0,
                speed: MIMIC_FLEE_SPEED,
                damage: 10.0,
                xp: 6.0,
                scale: 1.2,
                // the chest interactable's own gold, so the thing that jumps up IS the chest
                color: Color::srgb(0.85, 0.60, 0.20),
                hover: 0.0,
                standoff: 0.0,
            },
            BeamerPrime => EnemyDef {
                kind: *self,
                name: "Longshot Beamer Prime",
                hp: 110.0,
                speed: 2.0,
                damage: 24.0,
                xp: 9.0,
                scale: 1.6,
                color: Color::srgb(0.72, 0.14, 0.52),
                hover: 0.0,
                standoff: PRIME_STANDOFF,
            },
        }
    }

    /// Every kind, in wire-code order (`netenemy::kind_code`). Asset tables and per-kind
    /// counters index by position in this list.
    pub const ALL: [EnemyKind; 16] = [
        EnemyKind::Shambler,
        EnemyKind::Sprinter,
        EnemyKind::Bruiser,
        EnemyKind::Spitter,
        EnemyKind::Ufo,
        EnemyKind::Burrower,
        EnemyKind::Beamer,
        EnemyKind::Lobber,
        EnemyKind::Ghost,
        EnemyKind::Rollo,
        EnemyKind::Trencher,
        EnemyKind::AegisDrone,
        EnemyKind::Sunskimmer,
        EnemyKind::BeaconTick,
        EnemyKind::Mimic,
        EnemyKind::BeamerPrime,
    ];

    /// Position in [`EnemyKind::ALL`].
    pub fn index(self) -> usize {
        EnemyKind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// The one emissive accent a kind's silhouette carries (a lens, a core, a shield edge),
    /// drawn from the SAME material through an emissive mask — the crowd stays one draw call
    /// per kind. The built eight predate the mask and glow faintly all over instead.
    pub fn accent(self) -> Option<Color> {
        use EnemyKind::*;
        match self {
            Rollo => Some(Color::srgb(1.0, 0.55, 0.15)),      // the eye peeking out of the curl
            Trencher => Some(Color::srgb(1.0, 0.72, 0.20)),   // hot claw tips
            AegisDrone => Some(Color::srgb(0.30, 0.90, 1.0)), // the shield's lit edge
            Sunskimmer => Some(Color::srgb(1.0, 0.88, 0.40)), // the little sun it burns on
            BeaconTick => Some(Color::srgb(1.0, 0.12, 0.10)), // the tracker bulb
            Mimic => Some(Color::srgb(1.0, 0.95, 0.35)),      // eyes in the dark of the lid
            BeamerPrime => Some(Color::srgb(1.0, 0.55, 0.90)), // the scope lens
            _ => None,
        }
    }

    /// Kills that pay elite-grade loot without the elite affix (and its ×8 HP): the sniper
    /// the GDD calls an elite, and a mimic coughing up what it swallowed.
    pub fn elite_loot(self) -> bool {
        matches!(self, EnemyKind::BeamerPrime | EnemyKind::Mimic)
    }

    /// Fliers whose hitbox is the COLUMN under them: weapons fly at chest height, and a
    /// diver at altitude would otherwise be unhittable until it had already hit you. The
    /// spatial hash files them at the ground point under their body (`enemies::rebuild_hash`).
    pub fn column_hitbox(self) -> bool {
        matches!(self, EnemyKind::Sunskimmer | EnemyKind::AegisDrone)
    }
}

/// One row of a world's spawn table: WHEN a kind joins the stage's mix (stage seconds), how
/// often it is picked against the other joined kinds, how many may be alive at once for a
/// solo run (scaled by the party's spawn multiplier, GDD §11), and whether the elite roll
/// may promote it. P01's `run::scaling` sets HOW MANY spawn and how tough they are; this
/// table only picks WHICH kind each spawn is.
#[derive(Clone, Copy, Debug)]
pub struct SpawnEntry {
    pub kind: EnemyKind,
    pub join: f32,
    pub weight: f32,
    pub cap: u16,
    pub can_elite: bool,
}

const fn row(kind: EnemyKind, join: f32, weight: f32, cap: u16, can_elite: bool) -> SpawnEntry {
    SpawnEntry { kind, join, weight, cap, can_elite }
}

/// No per-kind ceiling beyond the horde's own live cap.
const NO_CAP: u16 = u16::MAX;

/// The built eight's §3 arc — the same join marks the global mix used, at equal weight —
/// which every world shares; each world then adds its §9 kinds.
macro_rules! classic_rows {
    () => {
        [
            row(EnemyKind::Shambler, 0.0, 1.0, NO_CAP, true),
            row(EnemyKind::Sprinter, 90.0, 1.0, NO_CAP, true),
            row(EnemyKind::Spitter, 180.0, 1.0, NO_CAP, true),
            row(EnemyKind::Bruiser, 270.0, 1.0, NO_CAP, true),
            row(EnemyKind::Ufo, 360.0, 1.0, NO_CAP, true),
            row(EnemyKind::Beamer, 360.0, 1.0, NO_CAP, true),
            row(EnemyKind::Burrower, 450.0, 1.0, NO_CAP, true),
            row(EnemyKind::Lobber, 540.0, 1.0, NO_CAP, true),
        ]
    };
}

const fn concat<const A: usize, const B: usize, const N: usize>(a: [SpawnEntry; A], b: [SpawnEntry; B]) -> [SpawnEntry; N] {
    let mut out = [row(EnemyKind::Shambler, 0.0, 0.0, 0, false); N];
    let mut i = 0;
    while i < A {
        out[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < B {
        out[A + j] = b[j];
        j += 1;
    }
    out
}

/// Moon (§9): Rollo and Trencher are the Moon's; the Beacon Tick walks every world.
static MOON_TABLE: [SpawnEntry; 11] = concat(
    classic_rows!(),
    [
        row(EnemyKind::Rollo, 150.0, 0.55, 8, true),
        row(EnemyKind::BeaconTick, 240.0, 0.22, 3, false),
        row(EnemyKind::Trencher, 320.0, 0.40, 6, true),
    ],
);
/// Mars (§9): Aegis Drones, Sunskimmers and the Longshot Beamer Prime.
static MARS_TABLE: [SpawnEntry; 12] = concat(
    classic_rows!(),
    [
        row(EnemyKind::AegisDrone, 200.0, 0.55, 10, true),
        row(EnemyKind::BeaconTick, 240.0, 0.22, 3, false),
        row(EnemyKind::Sunskimmer, 280.0, 0.40, 5, false),
        row(EnemyKind::BeamerPrime, 420.0, 0.14, 2, false),
    ],
);
/// Dark Moon: its own §9 kinds are batch 2 (P11); of batch 1 only the "Any" Beacon Tick
/// lives here, earlier than elsewhere — the last world of a chain starts hunting at once.
static DARK_MOON_TABLE: [SpawnEntry; 9] = concat(classic_rows!(), [row(EnemyKind::BeaconTick, 120.0, 0.25, 3, false)]);

/// The world's spawn table. The Mimic Chest is in none of them: it is laid out with the
/// chests (`interact::spawn_interactables`), never spawned by the director.
pub fn spawn_table(planet: PlanetKind) -> &'static [SpawnEntry] {
    match planet {
        PlanetKind::Moon => &MOON_TABLE,
        PlanetKind::Mars => &MARS_TABLE,
        PlanetKind::DarkMoon => &DARK_MOON_TABLE,
    }
}

/// Pick the next spawn's kind: a weighted draw over the rows that have joined by `elapsed`
/// (stage seconds) and are under their live cap (`alive[kind.index()]` against
/// `cap × party_scale`). Exactly one `rng` draw whatever the outcome, so a cap filling up
/// never shifts the director's stream. `None` only if nothing is eligible.
pub fn pick_spawn(table: &[SpawnEntry], elapsed: f32, alive: &[u32], party_scale: f32, rng: &mut impl Rng) -> Option<SpawnEntry> {
    let roll: f32 = rng.gen_range(0.0..1.0);
    let open = |r: &&SpawnEntry| {
        r.join <= elapsed && r.weight > 0.0 && (alive.get(r.kind.index()).copied().unwrap_or(0) as f32) < live_cap(r, party_scale)
    };
    let total: f32 = table.iter().filter(open).map(|r| r.weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut at = roll * total;
    let mut last = None;
    for r in table.iter().filter(open) {
        last = Some(*r);
        if at < r.weight {
            return Some(*r);
        }
        at -= r.weight;
    }
    last
}

/// A row's live ceiling for a party (`PARTY_SPAWN_SCALE`): 5 Sunskimmers solo, 9 for two.
pub fn live_cap(r: &SpawnEntry, party_scale: f32) -> f32 {
    if r.cap == NO_CAP {
        f32::INFINITY
    } else {
        (r.cap as f32 * party_scale.max(1.0)).ceil()
    }
}

/// Headless self-check: every world's table is well-formed and fields its §9 kinds, the
/// picker honours join times, weights and caps.
pub fn table_self_check() -> Result<(), String> {
    use EnemyKind::*;
    let want: [(PlanetKind, &[EnemyKind]); 3] = [
        (PlanetKind::Moon, &[Rollo, Trencher, BeaconTick]),
        (PlanetKind::Mars, &[AegisDrone, Sunskimmer, BeamerPrime, BeaconTick]),
        (PlanetKind::DarkMoon, &[BeaconTick]),
    ];
    for (planet, kinds) in want {
        let t = spawn_table(planet);
        for k in kinds {
            if !t.iter().any(|r| r.kind == *k) {
                return Err(format!("{planet:?}'s spawn table lacks {}", k.def().name));
            }
        }
        if t.iter().any(|r| matches!(r.kind, Mimic | Ghost)) {
            return Err(format!("{planet:?}: the Mimic and The Static are never director spawns"));
        }
        if t.iter().any(|r| r.weight <= 0.0 || r.cap == 0 || r.join < 0.0) {
            return Err(format!("{planet:?}: a row with no weight, no cap or a negative join"));
        }
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let alive = vec![0u32; EnemyKind::ALL.len()];
        // the cold open is Shamblers only
        for _ in 0..200 {
            if pick_spawn(t, 10.0, &alive, 1.0, &mut rng).map(|r| r.kind) != Some(Shambler) {
                return Err(format!("{planet:?}: something besides Shamblers in the cold open"));
            }
        }
        // late in the stage every row comes up
        let mut seen = vec![0u32; EnemyKind::ALL.len()];
        for _ in 0..20_000 {
            if let Some(r) = pick_spawn(t, 599.0, &alive, 1.0, &mut rng) {
                seen[r.kind.index()] += 1;
            }
        }
        for r in t {
            if seen[r.kind.index()] == 0 {
                return Err(format!("{planet:?}: {} never picked late in the stage", r.kind.def().name));
            }
        }
        // a capped kind at its ceiling is never picked
        for r in t.iter().filter(|r| r.cap != NO_CAP) {
            let mut full = alive.clone();
            full[r.kind.index()] = live_cap(r, 1.75) as u32;
            if (0..2000).any(|_| pick_spawn(t, 599.0, &full, 1.75, &mut rng).is_some_and(|p| p.kind == r.kind)) {
                return Err(format!("{planet:?}: {} picked past its cap", r.kind.def().name));
            }
        }
    }
    Ok(())
}

pub struct BossDef {
    pub kind: BossKind,
    pub name: &'static str,
    pub hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub scale: f32,
    pub color: Color,
    pub is_stage_boss: bool,
}

impl BossKind {
    pub fn def(&self) -> BossDef {
        use BossKind::*;
        match self {
            CraterpillarJr => BossDef {
                kind: *self,
                name: "Craterpillar Jr",
                hp: 700.0,
                speed: 3.4,
                damage: 16.0,
                scale: 2.6,
                color: Color::srgb(0.55, 0.65, 0.75),
                is_stage_boss: false,
            },
            RoverGoneWrong => BossDef {
                kind: *self,
                name: "Rover Gone Wrong",
                hp: 1100.0,
                speed: 5.2,
                damage: 20.0,
                scale: 2.2,
                color: Color::srgb(0.85, 0.75, 0.55),
                is_stage_boss: false,
            },
            Craterpillar => BossDef {
                kind: *self,
                name: "THE CRATERPILLAR",
                hp: 5200.0,
                speed: 3.0,
                damage: 26.0,
                scale: 4.6,
                color: Color::srgb(0.60, 0.60, 0.70),
                is_stage_boss: true,
            },
            Anubot => BossDef {
                kind: *self,
                name: "JUDGE ANUBOT",
                hp: 8200.0,
                speed: 3.6,
                damage: 32.0,
                scale: 4.2,
                color: Color::srgb(0.80, 0.60, 0.20),
                is_stage_boss: true,
            },
        }
    }
}

/// Elite modifier applied to a base enemy: bigger, tougher, shinier, generous.
pub struct EliteMods;
impl EliteMods {
    pub const HP: f32 = 8.0;
    pub const DMG: f32 = 1.8;
    pub const SCALE: f32 = 1.65;
    pub const XP: f32 = 8.0;
}

// ─── GDD §9 "Glitched" elite affixes (P10) ──────────────────────────────────

/// One stacked elite affix. What each DOES lives in `affixes.rs`, its numbers in config.rs.
/// The order is the WIRE order (`bit`, the spawn descriptor's affix byte): append only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Affix {
    Overclocked,
    Leaden,
    Warden,
    Contagious,
    Magnetar,
    Nightborne,
    Meteoric,
    CursedTouched,
}

pub struct AffixDef {
    /// Shown on the callout banner (ASCII: the UI font lacks most glyphs, L66).
    pub name: &'static str,
    /// §9's counterplay, the callout's teaching line.
    pub counterplay: &'static str,
    /// The aura colour that telegraphs it (§9 table). Never red: red is danger's alone (§13).
    pub aura: Color,
    /// Relative odds in the roll (Cursed-Touched also grows with Δ, see `roll_affixes`).
    pub weight: f32,
    /// Segments of its aura ring — the affix reads by SHAPE as well as colour (§13).
    pub ring_segments: u32,
}

impl Affix {
    pub const ALL: [Affix; 8] = [
        Affix::Overclocked,
        Affix::Leaden,
        Affix::Warden,
        Affix::Contagious,
        Affix::Magnetar,
        Affix::Nightborne,
        Affix::Meteoric,
        Affix::CursedTouched,
    ];

    pub fn index(self) -> usize {
        Affix::ALL.iter().position(|a| *a == self).unwrap_or(0)
    }

    /// Its bit in an [`AffixSet`] (and on the wire).
    pub fn bit(self) -> u8 {
        1 << self.index()
    }

    pub fn def(self) -> AffixDef {
        use Affix::*;
        let (name, counterplay, aura, weight, ring_segments) = match self {
            Overclocked => ("OVERCLOCKED", "KITE, DON'T TRADE", Color::srgb(0.95, 0.97, 1.0), 1.0, 3),
            Leaden => ("LEADEN", "BAIT THE WELL AWAY", Color::srgb(0.55, 0.36, 0.20), 1.0, 1),
            Warden => ("WARDEN", "FLANK THE CURVE", Color::srgb(0.25, 0.92, 1.0), 1.0, 4),
            Contagious => ("CONTAGIOUS", "BURST, DON'T CHIP", Color::srgb(0.35, 0.95, 0.30), 1.0, 8),
            Magnetar => ("MAGNETAR", "USE SEEKERS AND PIERCERS", Color::srgb(0.68, 0.32, 1.0), 1.0, 6),
            Nightborne => ("NIGHTBORNE", "FIGHT IT AT DAWN", Color::srgb(0.16, 0.28, 0.95), 1.0, 2),
            Meteoric => ("METEORIC", "WATCH THE SHADOW", Color::srgb(1.0, 0.55, 0.12), 1.0, 5),
            CursedTouched => ("CURSED-TOUCHED", "HIGH RISK, HIGH LOOT", Color::srgb(0.95, 0.75, 0.15), AFFIX_CURSED_WEIGHT, 12),
        };
        AffixDef { name, counterplay, aura, weight, ring_segments }
    }

    /// Can an elite of `kind` carry it? An Aegis Drone already has a front shield (and a
    /// Rollo's ball has no front, and its pace is the slope's, not a stride to overclock);
    /// the Meteoric leap needs a walker that steers with the crowd — not a roller, a
    /// burrower, a tunneller or a flier.
    pub fn allowed_on(self, kind: EnemyKind) -> bool {
        use EnemyKind::*;
        match self {
            Affix::Warden => !matches!(kind, AegisDrone | Rollo),
            Affix::Overclocked => kind != Rollo,
            Affix::Meteoric => !matches!(kind, Rollo | Burrower | Trencher) && kind.def().hover <= 0.0,
            _ => true,
        }
    }
}

/// An elite's stacked affixes, as bits of [`Affix::bit`]. The first set bit in `ALL` order
/// is its PRIMARY (the body's colour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AffixSet(pub u8);

impl AffixSet {
    pub const EMPTY: AffixSet = AffixSet(0);
    pub fn has(self, a: Affix) -> bool {
        self.0 & a.bit() != 0
    }
    pub fn with(self, a: Affix) -> AffixSet {
        AffixSet(self.0 | a.bit())
    }
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub fn len(self) -> u32 {
        self.0.count_ones()
    }
    pub fn iter(self) -> impl Iterator<Item = Affix> {
        Affix::ALL.into_iter().filter(move |a| self.has(*a))
    }
    pub fn primary(self) -> Option<Affix> {
        self.iter().next()
    }
    /// "OVERCLOCKED METEORIC".
    pub fn label(self) -> String {
        self.iter().map(|a| a.def().name).collect::<Vec<_>>().join(" ")
    }
    /// §9's named nightmares.
    pub fn nickname(self) -> Option<&'static str> {
        use Affix::*;
        let pair = |a: Affix, b: Affix| self.has(a) && self.has(b);
        if pair(Overclocked, Meteoric) {
            Some("THE POGO GOBLIN")
        } else if pair(Warden, Magnetar) {
            Some("THE UNHITTABLE TURTLE")
        } else {
            None
        }
    }
    /// Parse `--affixes` names ("warden", "cursedtouched" / "cursed").
    pub fn from_names(list: &str) -> AffixSet {
        let mut set = AffixSet::EMPTY;
        for name in list.split(',').map(|s| s.trim().to_ascii_lowercase().replace(['-', '_'], "")) {
            if name == "all" {
                return AffixSet(u8::MAX);
            }
            if let Some(a) = Affix::ALL.iter().find(|a| {
                let n = a.def().name.to_ascii_lowercase().replace('-', "");
                n == name || (name.len() >= 4 && n.starts_with(&name))
            }) {
                set = set.with(*a);
            }
        }
        set
    }
}

/// Roll an elite's affixes (§9 "1-3 stacked, scaling with Difficulty/Cursed"): one, then a
/// second at `extra` and a third at `extra × AFFIX_THIRD_SHARE` (`Scaling::affix_extra`
/// carries the §3 run-time/depth/Δ shape), each drawn by weight from the affixes `kind` may
/// carry and does not have yet. Cursed-Touched's weight grows with `delta` (Δ points): the
/// Cursed items are the honeypot. Always exactly `1 + 2 + 2` draws, so the director's
/// stream never depends on which affixes came up.
pub fn roll_affixes(kind: EnemyKind, extra: f32, delta: f32, rng: &mut impl Rng) -> AffixSet {
    let more = rng.gen_range(0.0..1.0f32);
    let count = 1 + u32::from(more < extra) + u32::from(more < extra * AFFIX_THIRD_SHARE);
    let mut set = AffixSet::EMPTY;
    for i in 0..3 {
        let pick = rng.gen_range(0.0..1.0f32);
        if i >= count {
            continue;
        }
        let weight = |a: &Affix| {
            let w = a.def().weight;
            if *a == Affix::CursedTouched {
                w + AFFIX_CURSED_WEIGHT_DELTA * delta.max(0.0)
            } else {
                w
            }
        };
        let open: Vec<Affix> = Affix::ALL.into_iter().filter(|a| a.allowed_on(kind) && !set.has(*a)).collect();
        let total: f32 = open.iter().map(weight).sum();
        let mut at = pick * total;
        for a in &open {
            let w = weight(a);
            if at < w {
                set = set.with(*a);
                break;
            }
            at -= w;
        }
    }
    set
}

/// Headless self-check: the affix table and the roll.
pub fn affix_self_check() -> Result<(), String> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(0xAF1);
    let mut seen = [0u32; 8];
    let mut counts = [0u32; 4];
    for i in 0..20_000 {
        let kind = EnemyKind::ALL[i % EnemyKind::ALL.len()];
        let set = roll_affixes(kind, 0.5, 0.0, &mut rng);
        if set.is_empty() || set.len() > 3 {
            return Err(format!("an elite rolled {} affixes", set.len()));
        }
        if let Some(bad) = set.iter().find(|a| !a.allowed_on(kind)) {
            return Err(format!("a {} rolled {}", kind.def().name, bad.def().name));
        }
        counts[set.len() as usize] += 1;
        for a in set.iter() {
            seen[a.index()] += 1;
        }
    }
    if let Some(i) = seen.iter().position(|n| *n == 0) {
        return Err(format!("{} never rolled", Affix::ALL[i].def().name));
    }
    if counts[2] == 0 || counts[3] == 0 {
        return Err(format!("stacks never reached 2 or 3 affixes: {counts:?}"));
    }
    // no extra chance: always exactly one; a sure one: always three
    if (0..500).any(|_| roll_affixes(EnemyKind::Shambler, 0.0, 0.0, &mut rng).len() != 1) {
        return Err("a zero extra chance still stacked affixes".into());
    }
    if (0..500).any(|_| roll_affixes(EnemyKind::Shambler, 1.0 / AFFIX_THIRD_SHARE, 0.0, &mut rng).len() != 3) {
        return Err("a sure extra chance did not stack three".into());
    }
    // the Cursed honeypot: Δ makes Cursed-Touched likelier
    let cursed = |delta: f32, rng: &mut rand::rngs::StdRng| {
        (0..6000).filter(|_| roll_affixes(EnemyKind::Shambler, 0.0, delta, rng).has(Affix::CursedTouched)).count()
    };
    let (calm, cursed_run) = (cursed(0.0, &mut rng), cursed(6.0, &mut rng));
    if cursed_run <= calm * 3 / 2 {
        return Err(format!("Cursed-Touched did not grow with Delta ({calm} -> {cursed_run} of 6000)"));
    }
    // wire bits are distinct and the names parse back
    for a in Affix::ALL {
        if AffixSet::from_names(&a.def().name.to_ascii_lowercase()) != AffixSet::EMPTY.with(a) {
            return Err(format!("--affixes cannot name {}", a.def().name));
        }
        if a.def().name.bytes().any(|b| !b.is_ascii()) || a.def().counterplay.bytes().any(|b| !b.is_ascii()) {
            return Err(format!("{}'s text is not ASCII (L66)", a.def().name));
        }
    }
    if AffixSet::EMPTY.with(Affix::Overclocked).with(Affix::Meteoric).nickname() != Some("THE POGO GOBLIN") {
        return Err("the pogo goblin went unnamed".into());
    }
    Ok(())
}
