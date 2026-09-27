//! What the §7 items DO beyond their stat boosts: the proc & conditional items, the cursed
//! run-wide levers, and the ONE death-save resolver.
//!
//! Stat items need nothing here — `PlayerState::recompute_stats` folds their graded boosts
//! into the sheet. Conditional damage (Icarus, Downhill, Encirclement) and Widow's Ring live
//! in the sheet's accessors (`damage_mult`, `widow_mult`, …) because weapons already ask
//! those. The Overheat's jam, Anti-Grav Boots' 360° ring and Second Astronaut's ghost volley
//! are firing rules, so they live in `combat::weapon_fire`; the hover is movement, in
//! `player::player_physics`.
//!
//! CO-OP: everything that deals damage or moves enemies is HOST-simulated here, for every
//! astronaut — a joiner's items reach the host through `net::PlayerBuildMsg`. What a client
//! must SEE rides existing lanes:
//!   * one-shots (a yo-yo throw, a singularity, a trail ignition, a death-save) go out as
//!     `ItemFxMsg`s, which `netenemy` forwards on the hazard EVENT lane and a client turns
//!     back into the same local message — so `item_fx_presentation` is one code path on
//!     every machine;
//!   * persistent looks (the ghost co-pilot and its weapon, a burning trail being laid, a
//!     hover, Widow's halo) ride the replicated `net::NetItemVis` on each astronaut, which
//!     every machine's visual systems read for every body it draws — with the host-owned
//!     state its owner's own screen needs (an Overheat jam, Widow's recharge).

use crate::config::*;
use crate::content::items::ItemKind;
use crate::content::weapons::{Behavior, WeaponKind};
use crate::content::Rarity;
use crate::enemies::{Boss, Buried, Enemy, SpatialHash};
use crate::fx::{self, ParticleAssets, Pcolor};
use crate::messages::*;
use crate::net::NetItemVis;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::remote::RemoteAstronaut;
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;
use std::collections::{HashMap, HashSet, VecDeque};

// ─── per-astronaut item state ────────────────────────────────────────────────

/// One astronaut's item timers and short memories. Per STAGE (a fresh one comes with every
/// body `spawn_player` builds); anything that must outlive a teleporter — Dead Man's
/// Tether's once-per-run — lives on `PlayerState` instead.
///
/// Present on every astronaut the machine simulates or predicts: all of them on the host,
/// its own on a client (whose cosmetic `weapon_fire` needs the jam and hover state too).
#[derive(Component)]
pub struct ItemProcs {
    pub yoyo_cd: f32,
    pub hole_cd: f32,
    /// Comet Tail: time to the next patch, seconds stood still, and whether this stand-still
    /// already lit the trail (it takes moving again to re-arm).
    pub trail_drop: f32,
    pub still: f32,
    pub ignited: bool,
    /// The Overheat: volleys fired since the last jam, and the jam left.
    pub volleys: u32,
    pub jam: f32,
    /// Anti-Grav Boots: hover left this airtime, and whether it is holding us up right now.
    pub hover_left: f32,
    pub hovering: bool,
    /// Downhill Momentum: our own clock, last altitude, and (time, metres dropped) samples
    /// inside the window.
    pub clock: f32,
    pub last_alt: Option<f32>,
    pub drops: VecDeque<(f32, f32)>,
    pub encircle_cd: f32,
    /// Second Astronaut: the ghost's own weapon cooldown and time to its next weapon swap.
    pub ghost_cd: f32,
    pub ghost_swap: f32,
    pub widow_cd: f32,
    /// Dead Man's Tether: where we stood, (clock, dir), sampled at 10 Hz.
    pub path: VecDeque<(f32, Vec3)>,
    pub path_cd: f32,
    /// Boomerang Insurance fires once per dip below its threshold.
    pub insurance_armed: bool,
}

impl Default for ItemProcs {
    fn default() -> Self {
        Self {
            yoyo_cd: YOYO_PERIOD,
            hole_cd: BLACK_HOLE_PERIOD,
            trail_drop: 0.0,
            still: 0.0,
            ignited: false,
            volleys: 0,
            jam: 0.0,
            hover_left: ANTIGRAV_HOVER_SECS,
            hovering: false,
            clock: 0.0,
            last_alt: None,
            drops: VecDeque::new(),
            encircle_cd: 0.0,
            ghost_cd: 0.0,
            ghost_swap: 0.0,
            widow_cd: 0.0,
            path: VecDeque::new(),
            path_cd: 0.0,
            insurance_armed: true,
        }
    }
}

impl ItemProcs {
    /// Downhill Momentum's reading: fold this frame's altitude in, return metres lost over
    /// the window. Called by `player_physics` on every body it moves (host: all; client: its
    /// own), so the owner's HUD and the host's damage agree.
    pub fn track_descent(&mut self, altitude: f32, dt: f32) -> f32 {
        self.clock += dt;
        if let Some(last) = self.last_alt {
            let drop = last - altitude;
            if drop > 0.0 {
                self.drops.push_back((self.clock, drop));
            }
        }
        self.last_alt = Some(altitude);
        while self.drops.front().is_some_and(|(t, _)| self.clock - t > DOWNHILL_WINDOW_SECS) {
            self.drops.pop_front();
        }
        self.drops.iter().map(|(_, m)| m).sum()
    }

    /// A teleport (a tether rewind, a blink) is not a descent.
    pub fn forget_altitude(&mut self) {
        self.last_alt = None;
        self.drops.clear();
    }
}

/// Period of an "every X seconds" item for this sheet: Tome of the Swarm ("+proc frequency
/// on all every-X-seconds items") divides it by the sheet's Proc Frequency.
pub fn proc_period(base: f32, ps: &PlayerState) -> f32 {
    base / ps.stats.proc_rate.max(0.1)
}

/// Where Second Astronaut's ghost floats: off the owner's right shoulder, bobbing — out to
/// the side rather than behind, where the chase camera would put it between you and you. A pure function of the body's pose, so the host's firing origin and
/// every machine's drawn ghost agree without sending a position.
pub fn ghost_anchor(body: &Transform, t: f32) -> Vec3 {
    let up = body.translation.normalize_or_zero();
    let fwd = body.rotation * Vec3::NEG_Z;
    let fwd = (fwd - up * fwd.dot(up)).try_normalize().unwrap_or_else(|| sphere::tangent_frame(up).0);
    let right = fwd.cross(up);
    body.translation + up * (0.9 + (t * 2.3).sin() * 0.15) - fwd * 0.3 + right * 1.5
}

/// Weapons the ghost can mirror: the ones that FIRE. An orbit or aura is a field around its
/// owner — a ghost copy would just be a second one around the same astronaut.
pub fn ghost_can_mirror(w: WeaponKind) -> bool {
    !matches!(w.def().behavior, Behavior::Orbit { .. } | Behavior::Aura { .. })
}

// ─── the death-save resolver (§7 stacking rule, §15) ─────────────────────────

/// The death-saves, in the canon priority. At most ONE resolves per would-be-death.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathSave {
    /// Dead Man's Tether: rewind 3 s, 1 HP, once per run.
    Tether,
    /// Warden Solongo's ghost-revive — hero #20, outside the 1.0 roster (BUILD_PLAN scope:
    /// heroes 13–21 are post-launch). Her passive slots in here, in its canon place.
    WardenRevive,
    /// Boomerang Insurance / Antipode Blink's escape to the far pole (P06's blink).
    AntipodeEscape,
    /// Widow's Ring: survive at exactly 1 HP (recharging).
    WidowsRing,
}

impl DeathSave {
    pub const PRIORITY: [DeathSave; 4] =
        [DeathSave::Tether, DeathSave::WardenRevive, DeathSave::AntipodeEscape, DeathSave::WidowsRing];

    /// Explicit wire code (hazard lane). Never renumber, only append.
    pub fn code(&self) -> u8 {
        match self {
            DeathSave::Tether => 0,
            DeathSave::WardenRevive => 1,
            DeathSave::AntipodeEscape => 2,
            DeathSave::WidowsRing => 3,
        }
    }
    pub fn from_code(c: u8) -> DeathSave {
        match c {
            1 => DeathSave::WardenRevive,
            2 => DeathSave::AntipodeEscape,
            3 => DeathSave::WidowsRing,
            _ => DeathSave::Tether,
        }
    }
    pub fn banner(&self) -> &'static str {
        match self {
            DeathSave::Tether => "DEAD MAN'S TETHER: REWOUND",
            DeathSave::WardenRevive => "THE WARDEN REFUSES",
            DeathSave::AntipodeEscape => "BOOMERANG INSURANCE PAID OUT",
            DeathSave::WidowsRing => "WIDOW'S RING: HANGING ON AT 1 HP",
        }
    }
}

/// HOOK for Warden Solongo (hero #20, post-1.0): her once-a-run ghost-revive.
fn warden_revive(_ps: &PlayerState) -> bool {
    false
}

/// P06 HOOK — Antipode Blink. Returns where the astronaut lands (the exact antipode) when a
/// blink may fire now for this sheet: Boomerang Insurance's panic escape, or an Antipode
/// Blink item off cooldown. Until P06 lands the mechanic nothing can blink, so this is the
/// safe "no escape" answer — and Boomerang Insurance stays out of the pools
/// (`ItemDef::pooled`) so nobody is dealt an item that cannot fire.
pub fn antipode_escape(_ps: &PlayerState, _procs: &mut ItemProcs, _here: Vec3) -> Option<Vec3> {
    None
}

/// Boomerang Insurance's non-lethal trigger (§7: auto-blink at 20% HP). Called after every
/// hit that leaves the astronaut standing; re-arms once they heal back above the line.
pub fn boomerang_insurance(ps: &PlayerState, procs: &mut ItemProcs, here: Vec3) -> Option<Vec3> {
    let low = ps.hp <= ps.stats.max_hp * BOOMERANG_INSURANCE_HP;
    if !low {
        procs.insurance_armed = true;
        return None;
    }
    if !procs.insurance_armed || !ps.has_item(ItemKind::BoomerangInsurance) {
        return None;
    }
    let landing = antipode_escape(ps, procs, here)?;
    procs.insurance_armed = false;
    Some(landing)
}

/// THE death-save resolver: called once when a hit would kill. Walks the canon priority and
/// resolves the FIRST save that can fire — never two, so they cannot compound into
/// immortality — leaving the astronaut at 1 HP with a few i-frames. Returns which save
/// fired and where the astronaut now stands (`here` unless it moved them).
pub fn resolve_death_save(ps: &mut PlayerState, procs: &mut ItemProcs, here: Vec3) -> Option<(DeathSave, Vec3)> {
    for save in DeathSave::PRIORITY {
        let (landing, iframes) = match save {
            DeathSave::Tether if ps.has_item(ItemKind::DeadMansTether) && !ps.tether_used => {
                ps.tether_used = true;
                (tether_point(procs, here), TETHER_IFRAMES)
            }
            DeathSave::WardenRevive if warden_revive(ps) => (here, TETHER_IFRAMES),
            DeathSave::AntipodeEscape if ps.has_item(ItemKind::BoomerangInsurance) => {
                match antipode_escape(ps, procs, here) {
                    Some(d) => (d, TETHER_IFRAMES),
                    None => continue,
                }
            }
            DeathSave::WidowsRing if ps.has_item(ItemKind::WidowsRing) && procs.widow_cd <= 0.0 => {
                procs.widow_cd = WIDOW_SAVE_COOLDOWN;
                (here, WIDOW_IFRAMES)
            }
            _ => continue,
        };
        ps.hp = 1.0;
        ps.dead = false;
        ps.iframes = ps.iframes.max(iframes);
        return Some((save, landing));
    }
    None
}

/// Where Dead Man's Tether rewinds to: the newest remembered spot at least
/// TETHER_REWIND_SECS old, or the oldest we have (a death seconds after landing).
fn tether_point(procs: &ItemProcs, here: Vec3) -> Vec3 {
    let cutoff = procs.clock - TETHER_REWIND_SECS;
    procs
        .path
        .iter()
        .rev()
        .find(|(t, _)| *t <= cutoff)
        .or(procs.path.front())
        .map(|(_, d)| *d)
        .unwrap_or(here)
}

// ─── presentation messages ───────────────────────────────────────────────────

/// A one-shot item effect worth showing. The host's item systems write these; `netenemy`
/// forwards them on the hazard event lane, and a client writes the same message from the
/// wire with `from_wire` set — so presentation is one system on every machine.
#[derive(Clone, Copy, Debug)]
pub enum ItemFx {
    /// An Orbital Yo-Yo throw around astronaut `owner` (a PlayerId).
    Orbit { owner: u8, chunks: u8, radius: f32, dur: f32 },
    /// A Little Black Hole opened at `dir`.
    Singularity { dir: Vec3, radius: f32, dur: f32 },
    /// `owner`'s Comet Tail ignited.
    Ignite { owner: u8 },
    /// `owner` cheated death; `dir` is where they stand now.
    DeathSave { owner: u8, save: DeathSave, dir: Vec3 },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ItemFxMsg {
    pub fx: ItemFx,
    /// Rebuilt from the hazard lane on a client: the visuals are NOT already standing.
    pub from_wire: bool,
}

/// What the item systems did this run — for the headless probes (`--items`, `--deathsave`)
/// and nothing else.
#[derive(Resource, Default, Debug)]
pub struct ItemTelemetry {
    pub yoyo_throws: u32,
    pub yoyo_hits: u32,
    pub trail_patches: u32,
    pub trail_hits: u32,
    pub ignites: u32,
    pub jams: u32,
    pub ghost_volleys: u32,
    pub ring_volleys: u32,
    pub hover_secs: f32,
    pub airborne_secs: f32,
    pub singularities: u32,
    pub pulled: u32,
    pub max_encircle: u32,
    pub max_descent: f32,
    pub sun_steps: u32,
    pub radio_static_at: Option<f32>,
    /// Resolved death-saves by `DeathSave::code`.
    pub saves: [u32; 4],
    /// Arc metres each Tether rewind moved its astronaut — the probe checks it moved.
    pub rewinds: Vec<f32>,
}

// ─── assets ──────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct ItemAssets {
    pub debris_mesh: Handle<Mesh>,
    pub debris_mat: Handle<StandardMaterial>,
    pub patch_mesh: Handle<Mesh>,
    pub patch_mat: Handle<StandardMaterial>,
    pub patch_hot_mat: Handle<StandardMaterial>,
    pub hole_mesh: Handle<Mesh>,
    pub hole_mat: Handle<StandardMaterial>,
    pub ring_mesh: Handle<Mesh>,
    pub ring_mat: Handle<StandardMaterial>,
    pub ghost_mesh: Handle<Mesh>,
    pub ghost_mat: Handle<StandardMaterial>,
    pub orb_mesh: Handle<Mesh>,
    pub glow_mesh: Handle<Mesh>,
    pub glow_mat: Handle<StandardMaterial>,
    pub halo_mat: Handle<StandardMaterial>,
}

pub fn setup_item_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use crate::meshkit::{at, MeshData};
    // A lumpy chunk of hull plating: three fused ellipsoids read as "debris" at speed.
    let mut rock = MeshData::new();
    rock.add_ellipsoid(Vec3::new(0.42, 0.3, 0.36), 1, at(Vec3::ZERO), Color::WHITE);
    rock.add_ellipsoid(Vec3::new(0.24, 0.2, 0.28), 1, at(Vec3::new(0.26, 0.1, 0.05)), Color::srgb(0.7, 0.7, 0.75));
    rock.add_box(Vec3::new(0.5, 0.08, 0.2), at(Vec3::new(-0.1, 0.22, 0.0)), Color::srgb(0.55, 0.55, 0.6));
    // The ghost: a translucent suit silhouette — capsule body, helmet, visor.
    let mut ghost = MeshData::new();
    ghost.add_capsule(0.28, 0.5, at(Vec3::ZERO), Color::WHITE);
    ghost.add_sphere(0.26, 1, at(Vec3::new(0.0, 0.62, 0.0)), Color::WHITE);
    ghost.add_ellipsoid(Vec3::new(0.2, 0.14, 0.1), 1, at(Vec3::new(0.0, 0.64, -0.18)), Color::srgb(0.6, 0.9, 1.0));
    let unlit = |c: Color, glow: f32, alpha: f32| StandardMaterial {
        base_color: c.with_alpha(alpha),
        emissive: c.to_linear() * glow,
        unlit: true,
        alpha_mode: if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
        ..default()
    };
    commands.insert_resource(ItemAssets {
        debris_mesh: meshes.add(rock.build()),
        debris_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.72, 0.7, 0.66),
            emissive: LinearRgba::rgb(0.9, 0.55, 0.2),
            perceptual_roughness: 0.8,
            ..default()
        }),
        patch_mesh: meshes.add(Mesh::from(Cylinder::new(1.0, 0.06))),
        // Amber, not red: §12 keeps red for danger telegraphs, and this is YOUR fire.
        patch_mat: materials.add(unlit(Color::srgb(1.0, 0.55, 0.12), 1.8, 0.4)),
        patch_hot_mat: materials.add(unlit(Color::srgb(1.0, 0.85, 0.35), 4.0, 0.8)),
        hole_mesh: meshes.add(Mesh::from(Sphere::new(1.0))),
        hole_mat: materials.add(unlit(Color::srgb(0.02, 0.0, 0.05), 0.0, 1.0)),
        ring_mesh: meshes.add(Mesh::from(Torus::new(1.1, 1.5))),
        ring_mat: materials.add(unlit(Color::srgb(0.7, 0.4, 1.0), 3.5, 0.7)),
        ghost_mesh: meshes.add(ghost.build()),
        ghost_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(0.7, 0.9, 1.0, 0.35),
            emissive: LinearRgba::rgb(0.25, 0.45, 0.6),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        orb_mesh: meshes.add(Mesh::from(Sphere::new(0.14))),
        glow_mesh: meshes.add(Mesh::from(Cylinder::new(0.55, 0.05))),
        glow_mat: materials.add(unlit(Color::srgb(0.4, 0.9, 1.0), 3.0, 0.6)),
        // Widow's Ring: the item's own purple, worn as a halo — "one hit from gone"
        halo_mat: materials.add(unlit(Color::srgb(0.8, 0.45, 1.0), 3.0, 0.85)),
    });
}

// ─── HOST simulation ─────────────────────────────────────────────────────────

/// HOST: the run-wide cursed levers and each astronaut's slow-ticking item state.
///   * The Static Radio / Devoured Sun Shard act on the WORLD, so any carrier sets them for
///     the whole party (like `RunState::difficulty`).
///   * Tether memory, Widow's recharge, the ghost's weapon pick, Insurance's re-arm.
pub fn item_upkeep(
    time: Res<Time>,
    mut run: ResMut<RunState>,
    mut telemetry: ResMut<ItemTelemetry>,
    mut banners: MessageWriter<BannerMsg>,
    mut q: Query<(&Player, &mut PlayerState, &mut ItemProcs)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    run.static_radio = q.iter().any(|(_, ps, _)| ps.has_item(ItemKind::StaticRadio));
    // Tome of the Elite: elites are world loot, so the best Elite Loot in the party pays
    run.elite_loot = q.iter().map(|(_, ps, _)| ps.stats.elite_loot).fold(1.0f32, f32::max);
    if q.iter().any(|(_, ps, _)| ps.has_item(ItemKind::DevouredSunShard)) && run.sun_shrink < 1.0 {
        run.sun_shard_secs += dt;
        if run.sun_shard_secs >= SUN_SHARD_PERIOD {
            run.sun_shard_secs -= SUN_SHARD_PERIOD;
            run.sun_shrink = (run.sun_shrink + SUN_SHARD_STEP).min(1.0);
            telemetry.sun_steps += 1;
            banners.write(BannerMsg(if run.sun_shrink >= 1.0 {
                "THE SUN IS GONE".into()
            } else {
                format!("THE SUN SHRINKS ({:.0}% EATEN)", run.sun_shrink * 100.0)
            }));
        }
    }

    let mut rng = rand::thread_rng();
    for (p, mut ps, mut procs) in &mut q {
        procs.widow_cd = (procs.widow_cd - dt).max(0.0);
        // Tether memory: the clock is the one Downhill's tracker advances in player_physics.
        procs.path_cd -= dt;
        if procs.path_cd <= 0.0 {
            procs.path_cd = 0.1;
            let now = procs.clock;
            procs.path.push_back((now, p.dir));
            while procs.path.front().is_some_and(|(t, _)| now - t > TETHER_REWIND_SECS + 0.5) {
                procs.path.pop_front();
            }
        }
        // Second Astronaut: mirror a random FIRING weapon, re-picked every GHOST_SWAP_SECS
        // (or at once when the mirrored one evolved away or was never picked).
        if ps.has_item(ItemKind::SecondAstronaut) {
            procs.ghost_swap -= dt;
            let owned = ps.ghost_weapon.is_some_and(|g| ps.weapons.iter().any(|w| w.kind == g));
            if procs.ghost_swap <= 0.0 || !owned {
                let choices: Vec<WeaponKind> =
                    ps.weapons.iter().map(|w| w.kind).filter(|w| ghost_can_mirror(*w)).collect();
                ps.ghost_weapon = if choices.is_empty() {
                    None
                } else {
                    Some(choices[rng.gen_range(0..choices.len())])
                };
                procs.ghost_swap = GHOST_SWAP_SECS;
            }
        } else if ps.ghost_weapon.is_some() {
            ps.ghost_weapon = None;
        }
    }
}

/// Every machine: who is surrounding each astronaut — which compass octants hold an enemy
/// (Encirclement Bonus) and how many foes stand within TOME_CROWD_RADIUS (Tome of
/// Encirclement). The host's numbers deal the damage; a client's (from its streamed
/// proxies) are its own HUD's. One spatial-hash query per astronaut every
/// ENCIRCLE_SCAN_SECS — never a pass over the horde.
pub fn encirclement_scan(
    time: Res<Time>,
    hash: Res<SpatialHash>,
    enemies: Query<&Enemy>,
    mut telemetry: ResMut<ItemTelemetry>,
    mut tome_tel: ResMut<crate::tomes::TomeTelemetry>,
    mut q: Query<(&Player, &mut PlayerState, &mut ItemProcs, &Transform)>,
) {
    let dt = time.delta_secs();
    for (p, mut ps, mut procs, tf) in &mut q {
        let octants = ps.has_item(ItemKind::EncirclementBonus);
        let crowd = ps.stats.crowd_damage > 0.0;
        if !(octants || crowd) || ps.dead {
            ps.encircle_dirs = 0;
            ps.crowd = 0;
            continue;
        }
        procs.encircle_cd -= dt;
        if procs.encircle_cd > 0.0 {
            continue;
        }
        procs.encircle_cd = ENCIRCLE_SCAN_SECS;
        let up = p.dir;
        let (t, b) = sphere::tangent_frame(up);
        let reach = if octants { ENCIRCLE_RADIUS.max(TOME_CROWD_RADIUS) } else { TOME_CROWD_RADIUS };
        let mut dirs = 0u8;
        let mut near = 0u32;
        for (e, pos) in hash.near(tf.translation, reach) {
            // pots share the hash (speed 0); they surround nobody
            if !enemies.get(e).is_ok_and(|en| en.speed > 0.0) {
                continue;
            }
            let v = pos - tf.translation;
            let vt = v - up * v.dot(up);
            let d = vt.length();
            if d <= TOME_CROWD_RADIUS {
                near += 1;
            }
            if d < 0.3 || d > ENCIRCLE_RADIUS {
                continue;
            }
            let ang = vt.dot(b).atan2(vt.dot(t)) + std::f32::consts::PI;
            let octant = ((ang / std::f32::consts::TAU * 8.0) as u32).min(7);
            dirs |= 1 << octant;
        }
        ps.encircle_dirs = if octants { dirs } else { 0 };
        ps.crowd = if crowd { near.min(TOME_CROWD_CAP) } else { 0 };
        telemetry.max_encircle = telemetry.max_encircle.max(ps.encircle_dirs.count_ones());
        tome_tel.max_crowd_bonus = tome_tel.max_crowd_bonus.max(ps.crowd_bonus());
    }
}

/// A chunk of Orbital Yo-Yo debris on its one lap around `owner` (the body entity it
/// circles on THIS machine). `damage` 0 marks a client's visual copy.
#[derive(Component)]
pub struct OrbitChunk {
    pub owner: Entity,
    pub phase: f32,
    pub age: f32,
    pub dur: f32,
    pub radius: f32,
    pub damage: f32,
    pub hit: Vec<Entity>,
}

fn spawn_orbit(
    commands: &mut Commands,
    assets: &ItemAssets,
    owner: Entity,
    at_pos: Vec3,
    chunks: u32,
    radius: f32,
    dur: f32,
    damage: f32,
) {
    for i in 0..chunks.max(1) {
        commands.spawn((
            OrbitChunk {
                owner,
                phase: i as f32 / chunks.max(1) as f32 * std::f32::consts::TAU,
                age: 0.0,
                dur,
                radius,
                damage,
                hit: Vec::new(),
            },
            Mesh3d(assets.debris_mesh.clone()),
            MeshMaterial3d(assets.debris_mat.clone()),
            Transform::from_translation(at_pos),
            StageScoped,
        ));
    }
}

/// The swing radius on this world: §7 "bigger world, wider swing".
pub fn yoyo_radius(planet: &CurrentPlanet) -> f32 {
    YOYO_RADIUS * planet.radius / YOYO_REF_PLANET_RADIUS
}

/// HOST: Orbital Yo-Yo — every YOYO_PERIOD s, debris laps each carrier once.
#[allow(clippy::too_many_arguments)]
pub fn orbital_yoyo(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<ItemAssets>,
    mut telemetry: ResMut<ItemTelemetry>,
    mut fx: MessageWriter<ItemFxMsg>,
    mut q: Query<(Entity, &PlayerId, &PlayerState, &mut ItemProcs, &Transform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, pid, ps, mut procs, tf) in &mut q {
        let Some(stack) = ps.item_stack(ItemKind::OrbitalYoYo) else { continue };
        if ps.dead {
            continue;
        }
        procs.yoyo_cd -= dt;
        if procs.yoyo_cd > 0.0 {
            continue;
        }
        procs.yoyo_cd = proc_period(YOYO_PERIOD, ps);
        // Each copy adds a chunk (up to the cap); the stack's graded power is shared out,
        // so a throw always carries YOYO_DAMAGE × power in total.
        let chunks = stack.count().min(YOYO_MAX_CHUNKS);
        let damage = YOYO_DAMAGE * stack.power() / stack.count() as f32 * ps.damage_mult();
        // Tome of Orbit: a wider swing, lapped faster — it is orbiting debris
        let orbit = ps.stats.orbit.max(0.1);
        let radius = yoyo_radius(&planet) * orbit;
        let dur = YOYO_ORBIT_SECS / orbit;
        spawn_orbit(&mut commands, &assets, e, tf.translation, chunks, radius, dur, damage);
        telemetry.yoyo_throws += 1;
        fx.write(ItemFxMsg {
            fx: ItemFx::Orbit { owner: pid.0, chunks: chunks as u8, radius, dur },
            from_wire: false,
        });
    }
}

/// Every machine: carry each chunk round its owner once; on the host, bonk what it passes
/// (each foe once per lap).
#[allow(clippy::too_many_arguments)]
pub fn orbit_chunks(
    mut commands: Commands,
    time: Res<Time>,
    hash: Res<SpatialHash>,
    enemies: Query<&Enemy>,
    sheets: Query<&PlayerState>,
    bodies: Query<&Transform, (Or<(With<Player>, With<RemoteAstronaut>)>, Without<OrbitChunk>)>,
    mut q: Query<(Entity, &mut OrbitChunk, &mut Transform)>,
    mut hits: MessageWriter<HitMsg>,
    mut telemetry: ResMut<ItemTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (e, mut c, mut tf) in &mut q {
        c.age += dt;
        let Ok(body) = bodies.get(c.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        if c.age >= c.dur {
            commands.entity(e).despawn();
            continue;
        }
        let up = body.translation.normalize_or_zero();
        let (t, b) = sphere::tangent_frame(up);
        let k = c.age / c.dur;
        let a = c.phase + k * std::f32::consts::TAU;
        // Out and back like a yo-yo on its string: the lap sweeps the whole disc, so it
        // catches the foes hugging you as well as the ring at full swing.
        let reach = c.radius * (std::f32::consts::PI * k).sin().max(0.25);
        let offset = (t * a.cos() + b * a.sin()) * reach;
        // at crowd height (the body's origin is half a suit above the ground)
        tf.translation = body.translation + offset - up * 0.2;
        tf.rotation = Quat::from_axis_angle(up, a * 3.0) * Quat::from_rotation_x(c.age * 9.0);
        if c.damage <= 0.0 {
            continue; // a client's copy only shows the lap
        }
        let Ok(ps) = sheets.get(c.owner) else { continue };
        let pos = tf.translation;
        for (te, tpos) in hash.near(pos, 2.2) {
            if c.hit.contains(&te) {
                continue;
            }
            let Ok(en) = enemies.get(te) else { continue };
            let reach = 1.1 + en.scale * 0.5;
            if en.speed <= 0.0 || tpos.distance_squared(pos) > reach * reach {
                continue;
            }
            let (cm, crit) = crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng);
            let elite = if en.elite { ps.stats.elite_damage } else { 1.0 };
            hits.write(HitMsg {
                source: Some(c.owner),
                target: te,
                amount: c.damage * cm * elite,
                crit,
                knock: offset.normalize_or_zero() * 6.0,
            });
            c.hit.push(te);
            telemetry.yoyo_hits += 1;
        }
    }
}

/// One Comet Tail patch. Host patches burn (`dps` > 0); a client's are visual copies laid
/// behind whichever drawn body `net::NetItemVis` says is trailing fire.
#[derive(Component)]
pub struct TrailPatch {
    pub owner: Entity,
    pub owner_pid: u8,
    pub dir: Vec3,
    pub life: f32,
    pub dps: f32,
    pub tick: f32,
    pub ignited: bool,
}

fn spawn_patch(commands: &mut Commands, assets: &ItemAssets, planet: &CurrentPlanet, owner: Entity, owner_pid: u8, dir: Vec3, dps: f32) {
    commands.spawn((
        TrailPatch { owner, owner_pid, dir, life: COMET_TAIL_LIFE, dps, tick: COMET_TAIL_TICK, ignited: false },
        Mesh3d(assets.patch_mesh.clone()),
        MeshMaterial3d(assets.patch_mat.clone()),
        Transform::from_translation(planet.surface_point(dir) + dir * 0.08)
            .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
            .with_scale(Vec3::new(COMET_TAIL_RADIUS, 1.0, COMET_TAIL_RADIUS)),
        StageScoped,
    ));
}

/// HOST: Comet Tail — lay burning patches while moving; stand still to ignite the lot.
#[allow(clippy::too_many_arguments)]
pub fn comet_tail(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<ItemAssets>,
    hash: Res<SpatialHash>,
    enemies: Query<&Enemy>,
    mut q: Query<(Entity, &PlayerId, &Player, &PlayerState, &mut ItemProcs, &Transform)>,
    mut patches: Query<(&mut TrailPatch, &mut MeshMaterial3d<StandardMaterial>)>,
    mut hits: MessageWriter<HitMsg>,
    mut fx: MessageWriter<ItemFxMsg>,
    mut telemetry: ResMut<ItemTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, pid, p, ps, mut procs, tf) in &mut q {
        let power = ps.item_power(ItemKind::CometTail);
        if power <= 0.0 || ps.dead {
            continue;
        }
        let speed = p.vel_t.length();
        let dmg_mult = ps.damage_mult();
        if speed > COMET_TAIL_MOVING_SPEED {
            procs.still = 0.0;
            procs.ignited = false;
            procs.trail_drop -= dt;
            if procs.trail_drop <= 0.0 {
                procs.trail_drop = COMET_TAIL_DROP_SECS;
                spawn_patch(&mut commands, &assets, &planet, e, pid.0, p.dir, COMET_TAIL_DPS * power * dmg_mult);
                telemetry.trail_patches += 1;
            }
            continue;
        }
        if speed > COMET_TAIL_STILL_SPEED {
            continue; // drifting: neither laying nor charging
        }
        procs.still += dt;
        if procs.still < COMET_TAIL_STILL_SECS || procs.ignited {
            continue;
        }
        // IGNITE: every patch of ours flares and bursts, plus a ring where we stand. One hit
        // per foe however many patches overlap it.
        procs.ignited = true;
        let burst = COMET_TAIL_IGNITE_DAMAGE * power * dmg_mult;
        let mut centres: Vec<(Vec3, f32)> = vec![(tf.translation, COMET_TAIL_IGNITE_RADIUS)];
        for (mut patch, mut mat) in &mut patches {
            if patch.owner != e {
                continue;
            }
            patch.ignited = true;
            patch.life = patch.life.max(1.2);
            mat.0 = assets.patch_hot_mat.clone();
            centres.push((planet.surface_point(patch.dir), COMET_TAIL_RADIUS + 0.4));
        }
        let mut struck: HashSet<Entity> = HashSet::new();
        for (c, r) in centres {
            for (te, tpos) in hash.near(c, r + 1.0) {
                let Ok(en) = enemies.get(te) else { continue };
                if en.speed <= 0.0 || tpos.distance(c) > r + en.scale * 0.5 || !struck.insert(te) {
                    continue;
                }
                hits.write(HitMsg { source: Some(e), target: te, amount: burst, crit: false, knock: Vec3::ZERO });
                telemetry.trail_hits += 1;
            }
        }
        telemetry.ignites += 1;
        fx.write(ItemFxMsg { fx: ItemFx::Ignite { owner: pid.0 }, from_wire: false });
    }
}

/// Every machine: burn, age and fade the patches. Only host patches carry `dps`.
pub fn trail_patches(
    mut commands: Commands,
    time: Res<Time>,
    hash: Res<SpatialHash>,
    enemies: Query<&Enemy>,
    mut q: Query<(Entity, &mut TrailPatch, &mut Transform)>,
    mut hits: MessageWriter<HitMsg>,
    mut telemetry: ResMut<ItemTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut p, mut tf) in &mut q {
        p.life -= dt;
        if p.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // shrink over the last second so a trail visibly burns out rather than blinking off
        let fade = p.life.min(1.0);
        let flare = if p.ignited { 1.35 } else { 1.0 };
        tf.scale = Vec3::new(COMET_TAIL_RADIUS * fade * flare, 1.0, COMET_TAIL_RADIUS * fade * flare);
        if p.dps <= 0.0 {
            continue;
        }
        p.tick -= dt;
        if p.tick > 0.0 {
            continue;
        }
        p.tick = COMET_TAIL_TICK;
        let amount = p.dps * COMET_TAIL_TICK * if p.ignited { COMET_TAIL_IGNITED_DPS_MULT } else { 1.0 };
        let pos = tf.translation;
        for (te, tpos) in hash.near(pos, COMET_TAIL_RADIUS + 1.0) {
            let Ok(en) = enemies.get(te) else { continue };
            if en.speed <= 0.0 || tpos.distance(pos) > COMET_TAIL_RADIUS + en.scale * 0.5 {
                continue;
            }
            hits.write(HitMsg { source: Some(p.owner), target: te, amount, crit: false, knock: Vec3::ZERO });
            telemetry.trail_hits += 1;
        }
    }
}

/// CLIENT: lay visual patches behind every drawn body the host says is trailing fire. The
/// host's own patches are the burning ones; these only have to look the same, so they are
/// dropped from the positions this screen draws, on the same cadence.
pub fn trail_client_drops(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<ItemAssets>,
    bodies: Query<(Entity, &PlayerId, &Transform, &NetItemVis), Or<(With<Player>, With<RemoteAstronaut>)>>,
    mut timers: Local<HashMap<Entity, f32>>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    timers.retain(|e, _| bodies.contains(*e));
    for (e, pid, tf, vis) in &bodies {
        if vis.flags & crate::net::ITEMVIS_TRAIL == 0 {
            timers.remove(&e);
            continue;
        }
        let t = timers.entry(e).or_insert(0.0);
        *t -= dt;
        if *t <= 0.0 {
            *t = COMET_TAIL_DROP_SECS;
            let dir = tf.translation.normalize_or_zero();
            spawn_patch(&mut commands, &assets, &planet, e, pid.0, dir, 0.0);
        }
    }
}

/// A Little Black Hole. `pull` on the host (it drags the horde); a client's copy only
/// draws — the pulled enemies reach it through the crowd stream.
#[derive(Component)]
pub struct Singularity {
    pub dir: Vec3,
    pub life: f32,
    pub max: f32,
    pub radius: f32,
    pub pull: bool,
    pub caught: HashSet<Entity>,
}

fn spawn_singularity(commands: &mut Commands, assets: &ItemAssets, planet: &CurrentPlanet, dir: Vec3, radius: f32, dur: f32, pull: bool) {
    let pos = planet.surface_point(dir) + dir * 1.2;
    commands
        .spawn((
            Singularity { dir, life: dur, max: dur, radius, pull, caught: HashSet::new() },
            Mesh3d(assets.hole_mesh.clone()),
            MeshMaterial3d(assets.hole_mat.clone()),
            Transform::from_translation(pos)
                .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
                .with_scale(Vec3::splat(0.2)),
            StageScoped,
        ))
        .with_children(|c| {
            // the accretion ring, lying in the local horizontal so it reads from above
            c.spawn((
                Mesh3d(assets.ring_mesh.clone()),
                MeshMaterial3d(assets.ring_mat.clone()),
                Transform::from_scale(Vec3::new(1.6, 0.4, 1.6)),
            ));
        });
}

/// HOST: Little Black Hole — every BLACK_HOLE_PERIOD s a singularity opens just ahead.
pub fn little_black_hole(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<ItemAssets>,
    mut q: Query<(&Player, &PlayerState, &mut ItemProcs)>,
    mut fx: MessageWriter<ItemFxMsg>,
    mut telemetry: ResMut<ItemTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (p, ps, mut procs) in &mut q {
        if !ps.has_item(ItemKind::LittleBlackHole) || ps.dead {
            continue;
        }
        procs.hole_cd -= dt;
        if procs.hole_cd > 0.0 {
            continue;
        }
        procs.hole_cd = proc_period(BLACK_HOLE_PERIOD, ps);
        // Just ahead, where melee arcs and orbitals sweep: THE WHIRLPOOL (§7) is the horde
        // sucked into one point and ground on orbiting death.
        let dir = sphere::offset_dir(p.dir, p.facing, BLACK_HOLE_AHEAD, planet.radius);
        spawn_singularity(&mut commands, &assets, &planet, dir, BLACK_HOLE_RADIUS, BLACK_HOLE_PULL_SECS, true);
        telemetry.singularities += 1;
        fx.write(ItemFxMsg {
            fx: ItemFx::Singularity { dir, radius: BLACK_HOLE_RADIUS, dur: BLACK_HOLE_PULL_SECS },
            from_wire: false,
        });
    }
}

/// Every machine: spin and collapse each singularity; on the host, drag everything inside
/// its radius into the centre. The drag is written as the enemy's knockback impulse, which
/// `enemy_move` already integrates along the sphere — set every frame, it IS the pull.
/// Bosses and pots do not move.
#[allow(clippy::type_complexity)]
pub fn singularity_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    mut enemies: Query<(&mut Enemy, Has<Boss>), Without<Buried>>,
    mut q: Query<(Entity, &mut Singularity, &mut Transform), Without<Enemy>>,
    mut telemetry: ResMut<ItemTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (e, mut s, mut tf) in &mut q {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = s.life / s.max;
        // swell in fast, then collapse to a point as the pull completes
        let size = (1.0 - k).min(0.25) * 4.0 * k.sqrt() * 1.4 + 0.2;
        tf.scale = Vec3::splat(size);
        tf.rotate_axis(Dir3::new(s.dir).unwrap_or(Dir3::Y), dt * 6.0);
        if !s.pull {
            continue;
        }
        let centre = planet.surface_point(s.dir);
        let remaining = s.life.max(0.15);
        let radius = s.radius;
        let dir = s.dir;
        let mut newly = 0;
        for (te, _) in hash.near(centre, radius + 1.0) {
            let Ok((mut en, is_boss)) = enemies.get_mut(te) else { continue };
            if is_boss || en.speed <= 0.0 {
                continue;
            }
            let arc = sphere::arc_dist(en.dir, dir, planet.radius);
            if arc > radius {
                continue;
            }
            let to = (dir - en.dir * dir.dot(en.dir)).normalize_or_zero();
            en.knock = to * (arc / remaining).min(BLACK_HOLE_MAX_PULL_SPEED);
            en.slow = en.slow.max(0.5);
            if s.caught.insert(te) {
                newly += 1;
            }
        }
        telemetry.pulled += newly;
    }
}

/// HOST: mirror what a client must draw of each astronaut's items onto its replicated
/// `NetItemVis` — only on change, since replicon sends whatever is touched.
pub fn push_net_item_vis(mut q: Query<(&Player, &PlayerState, &ItemProcs, &mut NetItemVis)>) {
    use crate::net::{ITEMVIS_HOVER, ITEMVIS_JAMMED, ITEMVIS_TETHER_SPENT, ITEMVIS_TRAIL, ITEMVIS_WIDOW};
    for (p, ps, procs, mut vis) in &mut q {
        let mut flags = 0u8;
        if !ps.dead && ps.has_item(ItemKind::CometTail) && p.vel_t.length() > COMET_TAIL_MOVING_SPEED {
            flags |= ITEMVIS_TRAIL;
        }
        if procs.hovering {
            flags |= ITEMVIS_HOVER;
        }
        if ps.widow_active() {
            flags |= ITEMVIS_WIDOW;
        }
        if ps.tether_used {
            flags |= ITEMVIS_TETHER_SPENT;
        }
        if procs.jam > 0.0 {
            flags |= ITEMVIS_JAMMED;
        }
        let ghost = ps.ghost_weapon.map(|w| w.code() + 1).unwrap_or(0);
        let widow_cd = procs.widow_cd.ceil().min(255.0) as u8;
        let lamp = crate::net::lamp_code(ps.stats.flashlight);
        let next = NetItemVis { ghost, flags, widow_cd, lamp };
        if *vis != next {
            *vis = next;
        }
    }
}

// ─── presentation (every machine) ────────────────────────────────────────────

/// Every machine: turn item one-shots into something visible. Host-simulated effects are
/// already standing on the host (its systems spawned the real, damaging thing), so the
/// orbit and singularity visuals are built only from the wire; the rest — bursts, the
/// owner's banner, a tether's snap — are the same everywhere.
#[allow(clippy::too_many_arguments)]
pub fn item_fx_presentation(
    mut commands: Commands,
    mut msgs: MessageReader<ItemFxMsg>,
    planet: Res<CurrentPlanet>,
    assets: Res<ItemAssets>,
    particles: Option<Res<ParticleAssets>>,
    bodies: Query<(Entity, &PlayerId, &Transform), Or<(With<Player>, With<RemoteAstronaut>)>>,
    mut local: Query<(&PlayerId, &mut Player, Option<&mut ItemProcs>), With<LocalPlayer>>,
    mut patches: Query<(&mut TrailPatch, &mut MeshMaterial3d<StandardMaterial>)>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
) {
    let local_pid = local.single().ok().map(|(pid, _, _)| pid.0);
    for m in msgs.read() {
        match m.fx {
            ItemFx::Orbit { owner, chunks, radius, dur } => {
                if !m.from_wire {
                    continue;
                }
                let Some((body, _, tf)) = bodies.iter().find(|(_, pid, _)| pid.0 == owner) else { continue };
                spawn_orbit(&mut commands, &assets, body, tf.translation, chunks as u32, radius, dur, 0.0);
            }
            ItemFx::Singularity { dir, radius, dur } => {
                if m.from_wire {
                    spawn_singularity(&mut commands, &assets, &planet, dir, radius, dur, false);
                }
            }
            ItemFx::Ignite { owner } => {
                if m.from_wire {
                    for (mut patch, mut mat) in &mut patches {
                        if patch.owner_pid == owner {
                            patch.ignited = true;
                            patch.life = patch.life.max(1.2);
                            mat.0 = assets.patch_hot_mat.clone();
                        }
                    }
                }
                if let (Some(pa), Some((_, _, tf))) = (&particles, bodies.iter().find(|(_, pid, _)| pid.0 == owner)) {
                    let up = tf.translation.normalize_or_zero();
                    fx::burst(&mut commands, pa, tf.translation, up, Pcolor::Gold, 24, 9.0);
                }
                if local_pid == Some(owner) {
                    sfx.write(SfxMsg(Sfx::Comet));
                }
            }
            ItemFx::DeathSave { owner, save, dir } => {
                let pos = planet.surface_point(dir) + dir * 1.0;
                if let Some(pa) = &particles {
                    let c = if save == DeathSave::WidowsRing { Pcolor::Purple } else { Pcolor::Cyan };
                    fx::burst(&mut commands, pa, pos, dir, c, 30, 10.0);
                }
                if local_pid != Some(owner) {
                    continue;
                }
                banners.write(BannerMsg(save.banner().into()));
                sfx.write(SfxMsg(Sfx::Teleport));
                // A client's own body is predicted here, so a rewind the host did to our
                // server-side copy has to be done to it too — the soft reconciliation would
                // otherwise drag us metres across the ground for a second.
                if m.from_wire && save != DeathSave::WidowsRing {
                    if let Ok((_, mut p, procs)) = local.single_mut() {
                        p.dir = dir;
                        p.vel_t = Vec3::ZERO;
                        p.vel_r = 0.0;
                        if let Some(mut procs) = procs {
                            procs.forget_altitude();
                        }
                    }
                }
            }
        }
    }
}

/// Second Astronaut's ghost co-pilot, drawn beside `owner` (a body on this machine).
#[derive(Component)]
pub struct GhostCopilot {
    pub owner: Entity,
    pub weapon: u8,
}

/// A status glow on a drawn body while its `NetItemVis` carries `flag`: Anti-Grav Boots'
/// pad under a hovering body, Widow's Ring's halo over one hanging on at 1 HP.
#[derive(Component)]
pub struct ItemGlow {
    pub owner: Entity,
    pub flag: u8,
}

/// The status glows `item_visuals` keeps, by NetItemVis flag.
const GLOW_FLAGS: [u8; 2] = [crate::net::ITEMVIS_HOVER, crate::net::ITEMVIS_WIDOW];

/// Where a status glow sits on its body: the hover pad pulses under the boots, the halo
/// turns slowly over the helmet.
fn glow_pose(flag: u8, body: &Transform, t: f32) -> Transform {
    let up = body.translation.normalize_or_zero();
    let (tan, _) = sphere::tangent_frame(up);
    if flag == crate::net::ITEMVIS_WIDOW {
        Transform::from_translation(body.translation + up * (PLAYER_HEIGHT * 0.5 + 0.3))
            .with_rotation(Quat::from_axis_angle(up, t * 1.5) * sphere::frame_quat(up, tan))
            .with_scale(Vec3::new(0.3, 0.2, 0.3) * (1.0 + (t * 3.0).sin() * 0.05))
    } else {
        Transform::from_translation(body.translation - up * (PLAYER_HEIGHT * 0.5 + 0.1))
            .with_rotation(sphere::frame_quat(up, tan))
            .with_scale(Vec3::splat(1.0 + (t * 14.0).sin() * 0.12))
    }
}

/// Every machine: keep a ghost beside every drawn body whose `NetItemVis` names one (in its
/// mirrored weapon's colour), and the status glows (`GLOW_FLAGS`) on every body that has
/// them. The host's own bodies carry the NetItemVis it writes; a client's teammates carry
/// the replicated one, and its own body the copy `net::adopt_my_item_vis` takes from the
/// host — so a teammate at 1 HP on Widow's Ring is visible from across the planet.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn item_visuals(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<ItemAssets>,
    weapons: Res<crate::combat::WeaponAssets>,
    bodies: Query<(Entity, &Transform, &NetItemVis), Or<(With<Player>, With<RemoteAstronaut>)>>,
    mut ghosts: Query<(Entity, &GhostCopilot, &mut Transform), (Without<Player>, Without<RemoteAstronaut>, Without<ItemGlow>)>,
    mut glows: Query<(Entity, &ItemGlow, &mut Transform), (Without<Player>, Without<RemoteAstronaut>, Without<GhostCopilot>)>,
) {
    let t = time.elapsed_secs();
    // ghosts: retire the stale, move the live, raise the missing
    let mut have: Vec<Entity> = Vec::new();
    for (ge, g, mut gtf) in &mut ghosts {
        let body = bodies.get(g.owner).ok().filter(|(_, _, v)| v.ghost == g.weapon && v.ghost != 0);
        let Some((_, btf, _)) = body else {
            commands.entity(ge).despawn();
            continue;
        };
        have.push(g.owner);
        gtf.translation = ghost_anchor(btf, t);
        gtf.rotation = btf.rotation;
    }
    for (be, btf, vis) in &bodies {
        if vis.ghost == 0 || have.contains(&be) {
            continue;
        }
        let Some(kind) = WeaponKind::from_code(vis.ghost - 1) else { continue };
        let orb = weapons.mats.get(&kind).cloned();
        commands
            .spawn((
                GhostCopilot { owner: be, weapon: vis.ghost },
                Mesh3d(assets.ghost_mesh.clone()),
                MeshMaterial3d(assets.ghost_mat.clone()),
                Transform::from_translation(ghost_anchor(btf, t)).with_rotation(btf.rotation),
                StageScoped,
            ))
            .with_children(|c| {
                // the mirrored weapon, glowing in the ghost's hands
                if let Some(mat) = orb {
                    c.spawn((Mesh3d(assets.orb_mesh.clone()), MeshMaterial3d(mat), Transform::from_xyz(0.3, 0.15, -0.3)));
                }
            });
    }
    // status glows: retire the stale, move the live, raise the missing
    let mut lit: Vec<(Entity, u8)> = Vec::new();
    for (ge, g, mut gtf) in &mut glows {
        let body = bodies.get(g.owner).ok().filter(|(_, _, v)| v.flags & g.flag != 0);
        let Some((_, btf, _)) = body else {
            commands.entity(ge).despawn();
            continue;
        };
        lit.push((g.owner, g.flag));
        *gtf = glow_pose(g.flag, btf, t);
    }
    for (be, btf, vis) in &bodies {
        for flag in GLOW_FLAGS {
            if vis.flags & flag == 0 || lit.contains(&(be, flag)) {
                continue;
            }
            let (mesh, mat) = if flag == crate::net::ITEMVIS_WIDOW {
                (assets.ring_mesh.clone(), assets.halo_mat.clone())
            } else {
                (assets.glow_mesh.clone(), assets.glow_mat.clone())
            };
            commands.spawn((ItemGlow { owner: be, flag }, Mesh3d(mesh), MeshMaterial3d(mat), glow_pose(flag, btf, t), StageScoped));
        }
    }
}

/// The planet's sun, as `planet::spawn_stage` builds it.
#[derive(Component)]
pub struct SunLight {
    pub base: f32,
}
#[derive(Component)]
pub struct SunDisc;

/// Every machine: the Devoured Sun Shard's lever made visible — the sun dims and its disc
/// shrinks with `RunState::sun_shrink` (streamed in RunSnapMsg, so both machines match).
/// A stand-in until P07's day/night cycle, which shrinks the lit HEMISPHERE toward
/// night-lock from the same value and replaces this.
pub fn apply_sun_shrink(
    run: Res<RunState>,
    mut lights: Query<(&mut DirectionalLight, &SunLight)>,
    mut discs: Query<&mut Transform, With<SunDisc>>,
) {
    let s = run.sun_shrink.clamp(0.0, 1.0);
    for (mut l, sun) in &mut lights {
        let want = sun.base * (1.0 - 0.8 * s);
        if (l.illuminance - want).abs() > 1.0 {
            l.illuminance = want;
        }
    }
    let scale = 1.0 - 0.85 * s;
    for mut tf in &mut discs {
        if (tf.scale.x - scale).abs() > 1e-3 {
            tf.scale = Vec3::splat(scale);
        }
    }
}

/// `--netlog`: what of the items this machine has seen and is drawing, every 5 s — on a
/// joiner this is the proof the hazard lane and `NetItemVis` carry them
/// ("ITEMFX[Client] … wire=N" counts events that arrived from the host).
#[allow(clippy::type_complexity)]
pub fn log_item_fx(
    time: Res<Time>,
    role: Res<crate::net::NetRole>,
    mut msgs: MessageReader<ItemFxMsg>,
    ghosts: Query<(), With<GhostCopilot>>,
    patches: Query<(), With<TrailPatch>>,
    chunks: Query<(), With<OrbitChunk>>,
    holes: Query<(), With<Singularity>>,
    mut tally: Local<([u32; 4], u32)>,
    mut next: Local<f32>,
) {
    for m in msgs.read() {
        let i = match m.fx {
            ItemFx::Orbit { .. } => 0,
            ItemFx::Singularity { .. } => 1,
            ItemFx::Ignite { .. } => 2,
            ItemFx::DeathSave { .. } => 3,
        };
        tally.0[i] += 1;
        tally.1 += u32::from(m.from_wire);
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 5.0;
    info!(
        "ITEMFX[{:?}] orbits={} holes={} ignites={} saves={} wire={} | drawn: ghosts={} patches={} chunks={} holes={}",
        *role,
        tally.0[0],
        tally.0[1],
        tally.0[2],
        tally.0[3],
        tally.1,
        ghosts.iter().count(),
        patches.iter().count(),
        chunks.iter().count(),
        holes.iter().count()
    );
}

// ─── test harness ────────────────────────────────────────────────────────────

/// The items named by `--items a,b,…` (names as `ItemKind::from_name` reads them; `new` =
/// the fifteen §7 additions). Unknown names are reported and skipped.
pub fn items_from_args() -> Vec<ItemKind> {
    let args: Vec<String> = std::env::args().collect();
    let Some(list) = args.iter().position(|a| a == "--items").and_then(|i| args.get(i + 1)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in list.split(',').filter(|n| !n.is_empty()) {
        if name == "new" {
            out.extend(ItemKind::NEW);
        } else if let Some(i) = ItemKind::from_name(name) {
            out.push(i);
        } else {
            warn!("--items: no item called {name:?}");
        }
    }
    out
}

/// Give a sheet one copy of each item at its native grade (respecting stack caps) and
/// recompute it, exactly as picking the cards would. Returns what was taken.
pub fn grant_items(ps: &mut PlayerState, items: &[ItemKind], save: &crate::save::MetaSave, greed_stacks: u32) -> Vec<ItemKind> {
    let mut taken = Vec::new();
    for item in items {
        if ps.item_count(*item) < item.def().max_stacks {
            ps.add_item(*item, item.def().rarity);
            taken.push(*item);
        }
    }
    ps.recompute_stats(save, greed_stacks);
    taken
}

// ─── rules self-check ────────────────────────────────────────────────────────

/// Headless self-check of the §7 item rules on synthetic sheets: grades, caps, the cursed
/// family, the loot roll, the cursed stats, and the death-save order. Returns the first
/// violated rule.
pub fn self_check(save: &crate::save::MetaSave) -> Result<(), String> {
    use crate::content::characters::AstronautKind;
    use rand::SeedableRng;

    // Grades: a bigger roll of the SAME effect, never below native; Cursed is fixed.
    if ItemKind::OrbitalYoYo.grade_mult(Rarity::Legendary) != 1.0 + 2.0 * ITEM_GRADE_STEP
        || ItemKind::SpaceBorgar.grade_mult(Rarity::Common) != 1.0
        || ItemKind::CrackedHelmet.grade_mult(Rarity::Legendary) != 1.0
    {
        return Err("grade multipliers do not follow the ladder".into());
    }
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x17E5);
    for item in ItemKind::ALL {
        let d = item.def();
        if ItemKind::from_code(item.code()) != Some(item) {
            return Err(format!("{} has no round-trip wire code", d.name));
        }
        if d.rarity == Rarity::Legendary && d.max_stacks > 2 {
            return Err(format!("Legendary {} stacks past 2 (§7)", d.name));
        }
        for _ in 0..50 {
            let g = crate::run::roll_grade(item, 3.0, 1, &mut rng);
            if (item.is_cursed() && g != Rarity::Cursed) || (!item.is_cursed() && (g < d.rarity || g == Rarity::Cursed)) {
                return Err(format!("{} rolled at {:?}", d.name, g));
            }
        }
    }
    for marked in [ItemKind::LittleBlackHole, ItemKind::DeadMansTether, ItemKind::DevouredSunShard] {
        if marked.def().max_stacks != 1 {
            return Err(format!("{} is marked cap 1 in §7", marked.def().name));
        }
    }
    if ItemKind::NEW.iter().any(|i| !i.def().pooled) || ItemKind::pool().count() != ItemKind::ALL.len() - 1 {
        return Err("every new §7 item must be in the pools (only Boomerang Insurance waits on P06)".into());
    }

    // The loot roll: legal, graded, and a FIXED draw count whatever the sheet (rule 5).
    let plain = PlayerState::new(AstronautKind::Buzz, save);
    let mut radio = PlayerState::new(AstronautKind::Buzz, save);
    radio.add_item(ItemKind::StaticRadio, Rarity::Cursed);
    radio.banned_items.insert(ItemKind::SpaceBorgar);
    let mut cursed_seen = false;
    for seed in 0..400u64 {
        let mut a = rand::rngs::StdRng::seed_from_u64(seed);
        let mut b = rand::rngs::StdRng::seed_from_u64(seed);
        let (ia, ga) = crate::run::roll_item(&plain, 0.4, &mut a);
        let (ib, gb) = crate::run::roll_item(&radio, 0.4, &mut b);
        if a.gen::<u64>() != b.gen::<u64>() {
            return Err("roll_item's draw count depends on the sheet".into());
        }
        for (i, g, ps) in [(ia, ga, &plain), (ib, gb, &radio)] {
            if !crate::run::item_available(ps, i) || (i.is_cursed() != (g == Rarity::Cursed)) || (!i.is_cursed() && g < i.def().rarity) {
                return Err(format!("roll_item dealt {} at {:?}", i.def().name, g));
            }
            cursed_seen |= i.is_cursed();
        }
        if !ib.is_cursed() && gb == Rarity::Common {
            return Err("The Static Radio's +1 grade did not apply to a loot roll".into());
        }
    }
    if !cursed_seen {
        return Err("400 loot rolls never came up Cursed".into());
    }
    // A Legendary roll is mostly Legendaries (GRADE_NATIVE_SHARE), whatever the pool's mix.
    let (mut legendary_rolls, mut legendary_items) = (0u32, 0u32);
    for seed in 0..2000u64 {
        let mut r = rand::rngs::StdRng::seed_from_u64(seed);
        let (i, g) = crate::run::roll_item(&plain, 3.0, &mut r);
        if g == Rarity::Legendary {
            legendary_rolls += 1;
            legendary_items += u32::from(i.def().rarity == Rarity::Legendary);
        }
    }
    let share = legendary_items as f32 / legendary_rolls.max(1) as f32;
    if legendary_rolls < 100 || (share - GRADE_NATIVE_SHARE).abs() > 0.1 {
        return Err(format!("a Legendary roll dealt a Legendary item {:.0}% of the time", share * 100.0));
    }

    // Cursed stats.
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    let base_hp = ps.stats.max_hp;
    ps.add_item(ItemKind::CrackedHelmet, Rarity::Cursed);
    ps.add_item(ItemKind::WidowsRing, Rarity::Cursed);
    ps.recompute_stats(save, 0);
    if (ps.stats.damage_taken - 2.0).abs() > 1e-4 || (ps.stats.max_hp - base_hp * 0.8).abs() > 0.01 {
        return Err(format!("cursed stats wrong: taken x{} max hp {} (base {base_hp})", ps.stats.damage_taken, ps.stats.max_hp));
    }
    ps.hp = 1.0;
    if !ps.widow_active() || (ps.widow_mult() - (1.0 + WIDOW_STAT_BONUS)).abs() > 1e-4 {
        return Err("Widow's Ring is not live at 1 HP".into());
    }

    // Death-saves: Tether first, then Widow's Ring, never both on one death, never forever.
    let mut ps = PlayerState::new(AstronautKind::Buzz, save);
    let mut procs = ItemProcs::default();
    let start = Vec3::X;
    procs.path.push_back((0.0, Vec3::Z));
    procs.clock = 5.0;
    if resolve_death_save(&mut ps, &mut procs, start).is_some() {
        return Err("a death-save fired with no death-save item".into());
    }
    for item in [ItemKind::WidowsRing, ItemKind::DeadMansTether] {
        ps.add_item(item, item.def().rarity);
    }
    ps.hp = 0.0;
    let first = resolve_death_save(&mut ps, &mut procs, start);
    if first != Some((DeathSave::Tether, Vec3::Z)) || ps.hp != 1.0 || procs.widow_cd > 0.0 || !ps.tether_used {
        return Err(format!("the first death did not resolve as ONE Tether rewind: {first:?}"));
    }
    ps.hp = 0.0;
    if resolve_death_save(&mut ps, &mut procs, start).map(|s| s.0) != Some(DeathSave::WidowsRing) {
        return Err("the second death did not fall through to Widow's Ring".into());
    }
    ps.hp = 0.0;
    if resolve_death_save(&mut ps, &mut procs, start).is_some() {
        return Err("a third death was saved with the Tether spent and the Ring recharging".into());
    }
    Ok(())
}
