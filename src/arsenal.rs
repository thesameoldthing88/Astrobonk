//! The six Tier-1 weapons' own behaviours (GDD §6) and the evolution fanfare (§12).
//!
//! | Weapon | Behaviour | Evolution |
//! |---|---|---|
//! | Meatball Comet | `Lob`: a mortar lobbed onto the densest clump in range — over the horizon — splatting an AoE | RAGÙ RAIN splits into 3 on the way down |
//! | Static Cling | `Hug`: a tight field that bites hardest on what hugs you, harder the more hug | FULL DISCHARGE also lets the stored charge go as a periodic screen-clearing nova |
//! | Ricochet Disc | `Disc`: bounces enemy to enemy, then rolls on round its great circle for a lap | THE OMNIDISC never runs out of bounces |
//! | Sonic Whoopee | `Cone`: shoves and stuns a cone; recharges faster when you are mobbed | THE BROWN NOTE: a lethal repulsor ring rolls out from you |
//! | Cosmonaut's Bell | `Toll`: every 4 s, damages and MARKS everything near for +crit | THE ANGELUS raises the recent dead as friendly Static-wisps |
//! | Yo-Yo of Damocles | `Tether`: yo-yos on a cord; scale with the un-hit move combo | SWORD-YO: at max combo the cord pays out and garrotes the ring |
//!
//! CO-OP: like every weapon (see `combat.rs`), these fire on every machine for each
//! astronaut it holds a sheet for — the host's copy is the real one (only its `HitMsg`s are
//! applied), a joiner's is the cosmetic prediction of its own guns. What a hit DOES beyond
//! damage — a Whoopee's stun, a bell mark's crit — is resolved host-side in
//! `combat::apply_hits`, keyed off `HitMsg::weapon`. What a joiner cannot predict rides the
//! wire: the Yo-Yo combo (`NetWeaponVis`, replicated), THE ANGELUS's wisps and every
//! evolution's fanfare (`WeaponFx`, on the hazard event lane).

use crate::combat::{Foes, Volley, WeaponAssets};
use crate::config::*;
use crate::content::enemies::EnemyKind;
use crate::content::weapons::{Behavior, WeaponKind};
use crate::enemies::{Enemy, EnemyAssets, SpatialHash};
use crate::fx::{self, ParticleAssets, Pcolor};
use crate::interact::Pot;
use crate::messages::*;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::run::PlayerState;
use crate::sphere;
use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ─── per-astronaut state ─────────────────────────────────────────────────────

/// What an astronaut's weapons remember between volleys. Per stage, like `ItemProcs`.
#[derive(Component, Default)]
pub struct WeaponProcs {
    /// Yo-Yo of Damocles: the un-hit move combo, in stacks (0..=YOYO_COMBO_MAX). The host
    /// counts it; a joiner adopts its own from `NetWeaponVis`.
    pub combo: f32,
    /// Metres moved toward the next stack.
    pub combo_m: f32,
    /// SWORD-YO: how far the cord has paid out toward the garrote (0..1).
    pub payout: f32,
    /// FULL DISCHARGE: seconds of charge toward the next nova.
    pub discharge: f32,
    /// Virtual time of the last bell toll (the bell over your shoulder swings from it).
    pub last_toll: f32,
}

/// The weapon state a joiner cannot predict, replicated per astronaut. HOST-written, only on
/// change (`push_net_weapon_vis`).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetWeaponVis {
    /// The Yo-Yo combo, whole stacks.
    pub combo: u8,
}

/// What the new weapons did — the headless probe reads it (`--weapons`).
#[derive(Resource, Default, Debug)]
pub struct ArsenalTelemetry {
    pub lobs: u32,
    pub splits: u32,
    pub splats: u32,
    pub lob_hits: u32,
    /// The farthest a lob was aimed (arc m) — "over the horizon".
    pub lob_far: f32,
    pub hug_hits: u32,
    pub max_huggers: u32,
    pub novas: u32,
    pub nova_hits: u32,
    pub discs: u32,
    pub bounces: u32,
    pub disc_hits: u32,
    pub disc_laps: u32,
    pub cones: u32,
    pub cone_hits: u32,
    pub panic_secs: f32,
    pub repulsors: u32,
    pub repulsor_hits: u32,
    pub stuns: u32,
    pub tolls: u32,
    pub marks: u32,
    pub mark_crits: u32,
    pub wisps: u32,
    pub wisp_hits: u32,
    pub yoyo_hits: u32,
    pub max_combo: f32,
    pub combo_breaks: u32,
    pub garrote_hits: u32,
    pub evolutions: u32,
    pub fanfare_pops: u32,
}

// ─── one-shots on the wire ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub enum WeaponFx {
    /// Astronaut `owner` (PlayerId) just evolved a weapon into `weapon`: the §12 fanfare.
    Evolve { owner: u8, weapon: WeaponKind },
    /// THE ANGELUS raised a friendly wisp for `owner` at `dir`; `power` is its hit (only the
    /// host, which applies hits, reads it).
    Wisp { owner: u8, dir: Vec3, power: f32 },
}

/// Written locally where a weapon one-shot happens (`from_wire: false`; the host forwards
/// those on the hazard lane) and by a client re-reading the lane (`from_wire: true`).
/// `weapon_fx_presentation` draws both the same way.
#[derive(Message, Clone, Copy, Debug)]
pub struct WeaponFxMsg {
    pub fx: WeaponFx,
    pub from_wire: bool,
}

/// Where foes died lately — the corpses THE ANGELUS raises. HOST-only (`record_kills`).
#[derive(Resource, Default)]
pub struct RecentKills {
    /// (virtual time, direction), oldest first.
    pub list: VecDeque<(f32, Vec3)>,
}

/// Everything the new behaviours need from `weapon_fire` beyond what a volley already
/// carries.
pub struct ArsenalCx<'a, 'w> {
    pub hash: &'a SpatialHash,
    pub planet: &'a CurrentPlanet,
    pub kills: &'a mut RecentKills,
    pub fx: &'a mut MessageWriter<'w, WeaponFxMsg>,
    pub tm: &'a mut ArsenalTelemetry,
    /// This machine applies hits (host / solo): only it raises wisps.
    pub simulating: bool,
    pub now: f32,
    /// Live discs per owner (DISC_MAX_LIVE).
    pub live_discs: &'a HashMap<Entity, usize>,
    /// Set by a toll, so the caller can swing the owner's bell.
    pub tolled: bool,
}

/// How long a hit from `w` stuns (Sonic Whoopee, THE BROWN NOTE); `None` for the rest.
pub fn stun_secs(w: WeaponKind) -> Option<f32> {
    match w.def().behavior {
        Behavior::Cone { .. } => Some(WHOOPEE_STUN_SECS),
        Behavior::Repulsor { .. } => Some(BROWN_NOTE_STUN_SECS),
        _ => None,
    }
}

// ─── Meatball Comet / RAGÙ RAIN: the lob ─────────────────────────────────────

#[derive(Component)]
pub struct Meatball {
    pub owner: Entity,
    pub weapon: WeaponKind,
    pub from: Vec3,
    pub to: Vec3,
    /// Flight progress 0..1 over `dur` seconds.
    pub t: f32,
    pub dur: f32,
    pub peak: f32,
    pub damage: f32,
    pub aoe: f32,
    /// Pieces it breaks into on the way down (0 = lands whole).
    pub split: u32,
    pub size: f32,
}

/// The sauce a landed meatball leaves (cosmetic).
#[derive(Component)]
pub struct Splat {
    pub life: f32,
    pub max: f32,
    pub radius: f32,
}

/// Is this a foe worth aiming at: alive, mobile (pots are scenery), not buried.
fn live_foe(e: Entity, en: &Enemy, pots: &Query<(), With<Pot>>) -> bool {
    en.speed > 0.0 && en.hp > 0.0 && pots.get(e).is_err()
}

/// Lob `count` meatballs onto the densest clumps within range. Range is an ARC, so on a
/// 105–160 m planet the far end is already past the horizon you can see.
#[allow(clippy::too_many_arguments)]
pub fn fire_lob(
    v: &Volley,
    range: f32,
    flight: f32,
    aoe: f32,
    split: u32,
    assets: &WeaponAssets,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    commands: &mut Commands,
    ax: &mut ArsenalCx,
    rng: &mut impl Rng,
) {
    let r_planet = ax.planet.radius;
    let here = v.center.normalize_or_zero();
    let aoe = aoe * v.size;
    let n = if v.ring { v.count.max(ANTIGRAV_RING_SHOTS) } else { v.count } as usize;
    let mut targets: Vec<Vec3> = Vec::with_capacity(n);
    if v.ring {
        // Anti-Grav Boots: a full ring of meatballs round you
        for h in crate::combat::ring_headings(v.up, v.aim, n as u32) {
            targets.push(sphere::offset_dir(here, h, range * 0.45, r_planet));
        }
    } else {
        // Weigh a thin, even sample of the foes in range by how many share their splat.
        let cos_range = (range / r_planet).cos();
        let in_range: Vec<Vec3> = enemies
            .iter()
            .filter(|(e, _, en)| live_foe(*e, en, pots) && en.dir.dot(here) > cos_range)
            .map(|(_, tf, _)| tf.translation)
            .collect();
        let step = (in_range.len() / LOB_CANDIDATES).max(1);
        let mut scored: Vec<(Vec3, usize)> =
            in_range.iter().step_by(step).map(|p| (*p, ax.hash.near(*p, aoe).count())).collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        for (p, _) in scored {
            if targets.len() >= n {
                break;
            }
            let d = p.normalize_or_zero();
            if targets.iter().all(|t| sphere::arc_dist(*t, d, r_planet) > aoe * LOB_TARGET_SEPARATION) {
                targets.push(d);
            }
        }
        // nobody (or too few clumps) in range: scatter the rest ahead
        while targets.len() < n {
            let h = Quat::from_axis_angle(v.up, rng.gen_range(-0.7..0.7)) * v.aim;
            targets.push(sphere::offset_dir(here, h, range * rng.gen_range(0.35..0.7), r_planet));
        }
    }
    let size = v.size;
    for to in targets {
        let arc = sphere::arc_dist(here, to, r_planet);
        ax.tm.lob_far = ax.tm.lob_far.max(arc);
        commands.spawn((
            Meatball {
                owner: v.owner,
                weapon: v.kind,
                from: here,
                to,
                t: 0.0,
                dur: flight * (LOB_FLIGHT_BASE + arc / LOB_FLIGHT_RANGE),
                peak: LOB_PEAK + arc * LOB_PEAK_PER_M,
                damage: v.dmg,
                aoe,
                split,
                size,
            },
            Mesh3d(assets.proj_mesh.clone()),
            MeshMaterial3d(assets.mats[&v.kind].clone()),
            Transform::from_translation(v.origin).with_scale(Vec3::splat(2.3 * size)),
            StageScoped,
        ));
        ax.tm.lobs += 1;
    }
}

/// Fly every meatball on its arc; RAGÙ RAIN breaks up on the way down; a landing splats
/// everything in its AoE. Every machine (a joiner's lands its own cosmetic splats).
#[allow(clippy::too_many_arguments)]
pub fn lob_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    assets: Res<WeaponAssets>,
    particles: Option<Res<ParticleAssets>>,
    q_ps: Query<&PlayerState>,
    enemies: Query<&Enemy>,
    mut q: Query<(Entity, &mut Meatball, &mut Transform)>,
    mut hits: MessageWriter<HitMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (e, mut m, mut tf) in &mut q {
        m.t += dt / m.dur.max(0.05);
        if m.split > 0 && m.t >= RAGU_SPLIT_AT {
            // RAGÙ RAIN: the one meatball becomes `split`, fanned round where it was headed
            let (t, b) = sphere::tangent_frame(m.to);
            let spin = rng.gen_range(0.0..std::f32::consts::TAU);
            for i in 0..m.split {
                let a = spin + i as f32 / m.split as f32 * std::f32::consts::TAU;
                let to = sphere::offset_dir(m.to, t * a.cos() + b * a.sin(), RAGU_SPREAD * m.size.sqrt(), planet.radius);
                commands.spawn((
                    Meatball { to, split: 0, ..*m },
                    Mesh3d(assets.proj_mesh.clone()),
                    MeshMaterial3d(assets.mats[&m.weapon].clone()),
                    Transform::from_translation(tf.translation).with_scale(Vec3::splat(1.7 * m.size)),
                    StageScoped,
                ));
            }
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, tf.translation, m.to, Pcolor::Gold, 8, 4.0);
            }
            tm.splits += 1;
            commands.entity(e).despawn();
            continue;
        }
        if m.t >= 1.0 {
            let pos = planet.surface_point(m.to);
            let owner = q_ps.get(m.owner).ok();
            let mut n = 0;
            for (te, tpos) in hash.near(pos, m.aoe + 1.5) {
                let Ok(en) = enemies.get(te) else { continue };
                let reach = m.aoe + en.scale * 0.5;
                if tpos.distance_squared(pos) > reach * reach {
                    continue;
                }
                let (cm, crit) = owner.map(|ps| crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng)).unwrap_or((1.0, false));
                let elite = if en.elite { owner.map(|ps| ps.stats.elite_damage).unwrap_or(1.0) } else { 1.0 };
                let away = (tpos - pos) - m.to * (tpos - pos).dot(m.to);
                hits.write(HitMsg {
                    source: Some(m.owner),
                    target: te,
                    amount: m.damage * cm * elite,
                    crit,
                    knock: away.normalize_or_zero() * 8.0,
                    by: HitBy::Weapon(m.weapon),
                });
                n += 1;
            }
            tm.lob_hits += n;
            tm.splats += 1;
            commands.spawn((
                Splat { life: SPLAT_SECS, max: SPLAT_SECS, radius: m.aoe },
                Mesh3d(assets.splat_mesh.clone()),
                MeshMaterial3d(assets.aura_mats[&m.weapon].clone()),
                Transform::from_translation(pos + m.to * TELEGRAPH_LIFT)
                    .with_rotation(sphere::frame_quat(m.to, sphere::tangent_frame(m.to).0))
                    .with_scale(Vec3::new(m.aoe, 1.0, m.aoe)),
                StageScoped,
            ));
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, pos + m.to * 0.4, m.to, Pcolor::Gold, 14, 7.0);
            }
            sfx.write(SfxMsg(Sfx::Splat));
            commands.entity(e).despawn();
            continue;
        }
        let dir = m.from.slerp(m.to, m.t).normalize_or_zero();
        let height = 1.0 + m.peak * 4.0 * m.t * (1.0 - m.t);
        tf.translation = planet.surface_point(dir) + dir * height;
        tf.rotation = Quat::from_rotation_x(m.t * 14.0) * Quat::from_rotation_z(m.t * 9.0);
    }
}

/// Splats sink back into the ground.
pub fn splat_fade(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Splat, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf) in &mut q {
        s.life -= dt;
        if s.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let k = (s.life / s.max).clamp(0.0, 1.0);
        let r = s.radius * (0.55 + 0.45 * k.sqrt());
        tf.scale = Vec3::new(r, k.max(0.05), r);
    }
}

// ─── Static Cling / FULL DISCHARGE: the hug field ────────────────────────────

/// One pulse of the hug field around an astronaut standing at `pos`: every foe inside
/// `radius` takes damage that swells toward contact and with the number hugging. A few of
/// them get a visible zap (photosensitivity: through the shared flash budget).
#[allow(clippy::too_many_arguments)]
pub fn hug_pulse(
    owner: Entity,
    kind: WeaponKind,
    pos: Vec3,
    radius: f32,
    dmg: f32,
    crit_ch: f32,
    stats: &crate::stats::Stats,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    hash: &SpatialHash,
    assets: &WeaponAssets,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    zaps: &mut crate::combat::ZapLook,
    tm: &mut ArsenalTelemetry,
    rng: &mut impl Rng,
) {
    let mut huggers: Vec<(Entity, Vec3, f32, bool)> = Vec::new();
    for (e, tpos) in hash.near(pos, radius + 1.0) {
        let Ok((_, _, en)) = enemies.get(e) else { continue };
        if !live_foe(e, en, pots) {
            continue;
        }
        let d = (tpos.distance(pos) - en.scale * 0.5).max(0.0);
        if d < radius {
            huggers.push((e, tpos, d, en.elite));
        }
    }
    if huggers.is_empty() {
        return;
    }
    let n = huggers.len() as u32;
    tm.max_huggers = tm.max_huggers.max(n);
    let crowd = 1.0 + HUG_PER_HUGGER * n.min(HUG_CROWD_CAP) as f32;
    for (e, _, d, elite) in &huggers {
        let close = 1.0 - (d / radius).clamp(0.0, 1.0);
        let share = HUG_EDGE_SHARE + (1.0 - HUG_EDGE_SHARE) * close;
        let (cm, crit) = crate::combat::roll_crit(crit_ch, stats.crit_damage, rng);
        let elite = if *elite { stats.elite_damage } else { 1.0 };
        hits.write(HitMsg { source: Some(owner), target: *e, amount: dmg * share * crowd * cm * elite, crit, knock: Vec3::ZERO, by: HitBy::Weapon(kind) });
    }
    tm.hug_hits += n;
    // the cling you can see: short arcs to the nearest few huggers
    if !zaps.photo || zaps.gate.allow(zaps.now) {
        huggers.sort_by(|a, b| a.2.total_cmp(&b.2));
        let (w, fader) = if zaps.photo {
            (0.0, crate::combat::Fader { life: PHOTO_ZAP_SECS, max: PHOTO_ZAP_SECS, swell: Some(0.07) })
        } else {
            (0.07, crate::combat::Fader { life: 0.1, max: 0.1, swell: None })
        };
        for (_, tpos, _, _) in huggers.iter().take(HUG_ZAPS) {
            let from = pos + (*tpos - pos).normalize_or_zero() * 0.4;
            let len = from.distance(*tpos);
            commands.spawn((
                Mesh3d(assets.beam_mesh.clone()),
                MeshMaterial3d(assets.mats[&kind].clone()),
                Transform::from_translation((from + *tpos) / 2.0)
                    .with_rotation(Quat::from_rotation_arc(Vec3::Z, (*tpos - from).normalize_or_zero()))
                    .with_scale(Vec3::new(w, w, len)),
                fader,
                StageScoped,
            ));
        }
    }
}

/// FULL DISCHARGE's nova: the stored charge goes off across `radius` (arc m) — screen-clearing.
#[allow(clippy::too_many_arguments)]
pub fn discharge_nova(
    owner: Entity,
    kind: WeaponKind,
    center: Vec3,
    radius: f32,
    dmg: f32,
    crit_ch: f32,
    stats: &crate::stats::Stats,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    planet: &CurrentPlanet,
    assets: &WeaponAssets,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    tm: &mut ArsenalTelemetry,
    rng: &mut impl Rng,
) {
    let here = center.normalize_or_zero();
    let cos_r = (radius / planet.radius).cos();
    for (e, _, en) in enemies.iter() {
        if !live_foe(e, en, pots) || en.dir.dot(here) < cos_r {
            continue;
        }
        let (cm, crit) = crate::combat::roll_crit(crit_ch, stats.crit_damage, rng);
        let elite = if en.elite { stats.elite_damage } else { 1.0 };
        let away = (en.dir - here * en.dir.dot(here)).normalize_or_zero();
        hits.write(HitMsg {
            source: Some(owner),
            target: e,
            amount: dmg * FULL_DISCHARGE_NOVA_MULT * cm * elite,
            crit,
            knock: away * FULL_DISCHARGE_KNOCK * stats.knockback,
            by: HitBy::Weapon(kind),
        });
        tm.nova_hits += 1;
    }
    tm.novas += 1;
    spawn_wave(commands, assets, kind, here, radius, 0.45, 0.0, planet);
    sfx.write(SfxMsg(Sfx::Discharge));
}

// ─── expanding rings (toll, nova, repulsor) and the Whoopee cone ─────────────

/// A ring that grows from its centre to `radius` over `dur` seconds, then fades — drawn
/// at chest height round a point on the ground. Visual only.
#[derive(Component)]
pub struct Wave {
    pub dir: Vec3,
    pub radius: f32,
    pub age: f32,
    pub dur: f32,
}

#[allow(clippy::too_many_arguments)]
fn spawn_wave(commands: &mut Commands, assets: &WeaponAssets, kind: WeaponKind, dir: Vec3, radius: f32, dur: f32, delay: f32, planet: &CurrentPlanet) {
    commands.spawn((
        Wave { dir, radius, age: -delay, dur },
        Mesh3d(assets.ring_mesh.clone()),
        MeshMaterial3d(assets.mats[&kind].clone()),
        Transform::from_translation(planet.surface_point(dir) + dir * 1.0)
            .with_rotation(sphere::frame_quat(dir, sphere::tangent_frame(dir).0))
            .with_scale(Vec3::ZERO),
        StageScoped,
    ));
}

/// The Whoopee's cone of rude noise, flaring out and gone.
#[derive(Component)]
pub struct ConeBlast {
    pub age: f32,
    pub range: f32,
}

const CONE_SECS: f32 = 0.28;

/// Sonic Whoopee: everything in the cone is hurt, shoved away and (in `apply_hits`, off the
/// hit's weapon) stunned.
#[allow(clippy::too_many_arguments)]
pub fn fire_cone(
    v: &Volley,
    arc_deg: f32,
    range: f32,
    stats: &crate::stats::Stats,
    assets: &WeaponAssets,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    ax: &mut ArsenalCx,
    rng: &mut impl Rng,
) {
    let arc_deg = if v.ring { 360.0 } else { arc_deg };
    let r = range * v.size;
    let cos_half = (arc_deg.to_radians() / 2.0).cos();
    let here = v.center.normalize_or_zero();
    for (e, _, en) in enemies.iter() {
        if !live_foe(e, en, pots) || sphere::arc_dist(here, en.dir, ax.planet.radius) > r + en.scale * 0.5 {
            continue;
        }
        let vt = (en.dir - here * en.dir.dot(here)).normalize_or_zero();
        if arc_deg < 360.0 && vt.dot(v.aim) < cos_half {
            continue;
        }
        let (cm, crit) = crate::combat::roll_crit(v.crit_ch, stats.crit_damage, rng);
        let elite = if en.elite { stats.elite_damage } else { 1.0 };
        hits.write(HitMsg {
            source: Some(v.owner),
            target: e,
            amount: v.dmg * cm * elite,
            crit,
            knock: vt * WHOOPEE_KNOCK * stats.knockback,
            by: HitBy::Weapon(v.kind),
        });
        ax.tm.cone_hits += 1;
    }
    ax.tm.cones += 1;
    if v.ring {
        spawn_wave(commands, assets, v.kind, here, r, CONE_SECS, 0.0, ax.planet);
    } else {
        commands.spawn((
            ConeBlast { age: 0.0, range: r },
            Mesh3d(assets.cone_mesh.clone()),
            MeshMaterial3d(assets.mats[&v.kind].clone()),
            Transform::from_translation(v.origin).with_rotation(sphere::frame_quat(v.up, v.aim)).with_scale(Vec3::ZERO),
            StageScoped,
        ));
    }
    sfx.write(SfxMsg(Sfx::Whoopee));
}

/// THE BROWN NOTE's repulsor ring: rolls out from where it went off, hitting each foe once
/// as it passes. Runs on every machine (a joiner's hits are its cosmetic copy's, dropped).
#[derive(Component)]
pub struct Repulsor {
    pub owner: Entity,
    pub weapon: WeaponKind,
    pub dir: Vec3,
    pub radius: f32,
    pub max: f32,
    pub damage: f32,
    pub crit_ch: f32,
    pub hit: HashSet<Entity>,
}

pub fn fire_repulsor(v: &Volley, radius: f32, assets: &WeaponAssets, commands: &mut Commands, sfx: &mut MessageWriter<SfxMsg>, ax: &mut ArsenalCx) {
    let here = v.center.normalize_or_zero();
    commands.spawn((
        Repulsor {
            owner: v.owner,
            weapon: v.kind,
            dir: here,
            radius: 0.5,
            max: radius * v.size,
            damage: v.dmg,
            crit_ch: v.crit_ch,
            hit: HashSet::new(),
        },
        Mesh3d(assets.ring_mesh.clone()),
        MeshMaterial3d(assets.mats[&v.kind].clone()),
        Transform::from_translation(ax.planet.surface_point(here) + here).with_scale(Vec3::ZERO),
        StageScoped,
    ));
    ax.tm.repulsors += 1;
    sfx.write(SfxMsg(Sfx::Whoopee));
}

#[allow(clippy::too_many_arguments)]
pub fn repulsor_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    q_ps: Query<&PlayerState>,
    enemies: Query<&Enemy>,
    pots: Query<(), With<Pot>>,
    mut q: Query<(Entity, &mut Repulsor, &mut Transform)>,
    mut hits: MessageWriter<HitMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    for (e, mut rp, mut tf) in &mut q {
        rp.radius = (rp.radius + BROWN_NOTE_SPEED * dt).min(rp.max);
        let center = planet.surface_point(rp.dir);
        let owner = q_ps.get(rp.owner).ok();
        let (knock_k, crit_dmg, elite_k) =
            owner.map(|ps| (ps.stats.knockback, ps.crit_damage(), ps.stats.elite_damage)).unwrap_or((1.0, 2.0, 1.0));
        let here = rp.dir;
        let fresh: Vec<(Entity, Vec3)> = hash
            .near(center, rp.radius + 1.5)
            .filter(|(te, _)| !rp.hit.contains(te))
            .collect();
        for (te, _) in fresh {
            let Ok(en) = enemies.get(te) else { continue };
            if !live_foe(te, en, &pots) || sphere::arc_dist(here, en.dir, planet.radius) > rp.radius + en.scale * 0.5 {
                continue;
            }
            rp.hit.insert(te);
            let (cm, crit) = crate::combat::roll_crit(rp.crit_ch, crit_dmg, &mut rng);
            let elite = if en.elite { elite_k } else { 1.0 };
            let away = (en.dir - here * en.dir.dot(here)).normalize_or_zero();
            hits.write(HitMsg {
                source: Some(rp.owner),
                target: te,
                amount: rp.damage * cm * elite,
                crit,
                knock: away * BROWN_NOTE_KNOCK * knock_k,
                by: HitBy::Weapon(rp.weapon),
            });
            tm.repulsor_hits += 1;
        }
        let k = rp.radius / rp.max.max(0.01);
        tf.translation = center + here * 1.0;
        tf.rotation = sphere::frame_quat(here, sphere::tangent_frame(here).0);
        tf.scale = Vec3::new(rp.radius, 0.5 + 1.5 * (1.0 - k), rp.radius);
        if rp.radius >= rp.max {
            commands.entity(e).despawn();
        }
    }
}

/// Grow the rings and the Whoopee cone, then retire them.
pub fn animate_waves(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut waves: Query<(Entity, &mut Wave, &mut Transform), Without<ConeBlast>>,
    mut cones: Query<(Entity, &mut ConeBlast, &mut Transform), Without<Wave>>,
) {
    let dt = time.delta_secs();
    for (e, mut w, mut tf) in &mut waves {
        w.age += dt;
        if w.age < 0.0 {
            continue;
        }
        let k = w.age / w.dur.max(0.01);
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let grow = 1.0 - (1.0 - k).powi(3);
        let r = (w.radius * grow).max(0.2);
        tf.translation = planet.surface_point(w.dir) + w.dir * 1.0;
        tf.scale = Vec3::new(r, 1.2 * (1.0 - k) + 0.15, r);
    }
    for (e, mut c, mut tf) in &mut cones {
        c.age += dt;
        let k = c.age / CONE_SECS;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let r = c.range * (0.35 + 0.65 * (1.0 - (1.0 - k).powi(2)));
        tf.scale = Vec3::new(r, 1.0 - k * 0.8, r);
    }
}

// ─── Ricochet Disc / THE OMNIDISC ────────────────────────────────────────────

#[derive(Component)]
pub struct Disc {
    pub owner: Entity,
    pub weapon: WeaponKind,
    pub dir: Vec3,
    pub heading: Vec3,
    pub speed: f32,
    pub damage: f32,
    /// Bounces left (`u32::MAX`: THE OMNIDISC, never runs out).
    pub bounces: u32,
    pub link: f32,
    /// The foe it is bouncing onto; `None` = rolling on round its great circle.
    pub target: Option<Entity>,
    /// Foes it cut through with no bounce left before it shatters.
    pub pierce: i32,
    pub hit_cd: HashMap<Entity, f32>,
    pub life: f32,
    pub travelled: f32,
    /// One circumference of this planet: travelled that far, it has lapped.
    pub lap: f32,
    pub lapped: bool,
    pub size: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn fire_disc(
    v: &Volley,
    speed: f32,
    bounces: u32,
    link: f32,
    stats: &crate::stats::Stats,
    assets: &WeaponAssets,
    commands: &mut Commands,
    sfx: &mut MessageWriter<SfxMsg>,
    ax: &mut ArsenalCx,
) {
    let live = ax.live_discs.get(&v.owner).copied().unwrap_or(0);
    if live >= DISC_MAX_LIVE {
        return; // the sky is full of your discs; the next throw waits for one to come home
    }
    let headings: Vec<Vec3> = if v.ring {
        crate::combat::ring_headings(v.up, v.aim, v.count.max(ANTIGRAV_RING_SHOTS)).collect()
    } else {
        (0..v.count)
            .map(|i| Quat::from_axis_angle(v.up, i as f32 * 0.4 - (v.count as f32 - 1.0) * 0.2) * v.aim)
            .collect()
    };
    let lap = std::f32::consts::TAU * ax.planet.radius;
    let speed = speed * stats.proj_speed;
    for h in headings.into_iter().take(DISC_MAX_LIVE - live) {
        commands.spawn((
            Disc {
                owner: v.owner,
                weapon: v.kind,
                dir: v.center.normalize_or_zero(),
                heading: h,
                speed,
                damage: v.dmg,
                bounces,
                link: link * v.size,
                target: None,
                pierce: DISC_CRUISE_PIERCE,
                hit_cd: HashMap::new(),
                life: lap / speed * DISC_LAP_SLACK * stats.duration.max(1.0),
                travelled: 0.0,
                lap,
                lapped: false,
                size: v.size,
            },
            Mesh3d(assets.disc_mesh.clone()),
            MeshMaterial3d(assets.mats[&v.kind].clone()),
            Transform::from_translation(v.origin).with_scale(Vec3::splat(v.size)),
            StageScoped,
        ));
        ax.tm.discs += 1;
    }
    sfx.write(SfxMsg(Sfx::Hit));
}

/// Fly the discs: onto their bounce target, else straight on round the planet; on a hit,
/// skip to the nearest foe not just struck. Every machine (a joiner's are cosmetic).
#[allow(clippy::too_many_arguments)]
pub fn disc_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    particles: Option<Res<ParticleAssets>>,
    q_ps: Query<&PlayerState>,
    enemies: Query<(&Enemy, &Transform)>,
    pots: Query<(), With<Pot>>,
    mut q: Query<(Entity, &mut Disc, &mut Transform), Without<Enemy>>,
    mut hits: MessageWriter<HitMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut rng = rand::thread_rng();
    let spin = time.elapsed_secs() * 22.0;
    for (e, mut d, mut tf) in &mut q {
        d.life -= dt;
        let Ok(ps) = q_ps.get(d.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        if d.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        // steer onto the bounce target (a ricochet commits; it doesn't drift)
        if let Some(t) = d.target {
            match enemies.get(t) {
                Ok((_, ttf)) => {
                    let v = ttf.translation - tf.translation;
                    let vt = (v - d.dir * v.dot(d.dir)).normalize_or_zero();
                    if vt != Vec3::ZERO {
                        d.heading = (d.heading + vt * DISC_TURN * dt).normalize_or_zero();
                    }
                }
                Err(_) => d.target = None,
            }
        }
        let r = planet.surface(d.dir) + 1.0;
        let (nd, nv) = sphere::advance(d.dir, d.heading * d.speed, r, dt);
        d.dir = nd;
        d.heading = nv.normalize_or_zero();
        d.travelled += d.speed * dt;
        if !d.lapped && d.travelled >= d.lap {
            d.lapped = true;
            tm.disc_laps += 1;
        }
        tf.translation = planet.surface_point(d.dir) + d.dir * 1.0;
        tf.rotation = sphere::frame_quat(d.dir, d.heading) * Quat::from_rotation_y(spin);
        d.hit_cd.retain(|_, t| {
            *t -= dt;
            *t > 0.0
        });

        // one strike per frame: a bounce re-aims the disc, so the rest of this frame's
        // contacts belong to the old line
        let reach = 0.45 * d.size + 0.55;
        let pos = tf.translation;
        let struck = hash.near(pos, reach + 1.2).find(|(te, tpos)| {
            !d.hit_cd.contains_key(te)
                && enemies.get(*te).is_ok_and(|(en, _)| {
                    let rr = reach + en.scale * 0.5;
                    en.hp > 0.0 && tpos.distance_squared(pos) < rr * rr
                })
        });
        let Some((te, _)) = struck else { continue };
        let Ok((en, _)) = enemies.get(te) else { continue };
        let (cm, crit) = crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng);
        let elite = if en.elite { ps.stats.elite_damage } else { 1.0 };
        hits.write(HitMsg {
            source: Some(d.owner),
            target: te,
            amount: d.damage * cm * elite,
            crit,
            knock: d.heading * 3.0 * ps.stats.knockback,
            by: HitBy::Weapon(d.weapon),
        });
        tm.disc_hits += 1;
        d.hit_cd.insert(te, DISC_REHIT_SECS);
        if d.bounces > 0 {
            // skip to the nearest live foe in link range that it didn't just hit
            let next = hash
                .near(pos, d.link)
                .filter(|(oe, _)| !d.hit_cd.contains_key(oe) && enemies.get(*oe).is_ok_and(|(o, _)| live_foe(*oe, o, &pots)))
                .map(|(oe, opos)| (oe, opos, opos.distance_squared(pos)))
                .filter(|(_, _, d2)| *d2 < d.link * d.link)
                .min_by(|a, b| a.2.total_cmp(&b.2));
            match next {
                Some((oe, opos, _)) => {
                    let v = opos - pos;
                    let vt = (v - d.dir * v.dot(d.dir)).normalize_or_zero();
                    if vt != Vec3::ZERO {
                        d.heading = vt;
                    }
                    d.target = Some(oe);
                    if d.bounces != u32::MAX {
                        d.bounces -= 1;
                    }
                    tm.bounces += 1;
                    if let Some(pa) = &particles {
                        fx::burst(&mut commands, pa, pos, d.dir, Pcolor::Cyan, 3, 3.0);
                    }
                }
                // nobody left to skip to: roll on round the planet
                None => d.target = None,
            }
        } else {
            d.target = None;
            d.pierce -= 1;
            if d.pierce < 0 {
                if let Some(pa) = &particles {
                    fx::burst(&mut commands, pa, pos, d.dir, Pcolor::Cyan, 6, 4.0);
                }
                commands.entity(e).despawn();
            }
        }
    }
}

// ─── Cosmonaut's Bell / THE ANGELUS ──────────────────────────────────────────

/// Tolled by a bell: hits on it roll BELL_MARK_CRIT extra crit chance (`apply_hits`).
/// Inserted by the toll on every machine — the host's are the real marks; a joiner's, on
/// its streamed proxies, are its cosmetic copy (the halos).
#[derive(Component)]
pub struct BellMark {
    pub secs: f32,
}

/// The little halo drawn over a marked foe (capped; the mark itself is not).
#[derive(Component)]
pub struct MarkHalo {
    pub target: Entity,
}

/// The bell itself, hovering over its owner's shoulder and swinging on each toll.
#[derive(Component)]
pub struct BellBody {
    pub owner: Entity,
    pub weapon: WeaponKind,
}

/// A friendly Static-wisp raised by THE ANGELUS. `owner` is the astronaut entity whose hit
/// it is — set only on the machine that applies hits; a joiner's copies just fly.
#[derive(Component)]
pub struct Wisp {
    pub pid: u8,
    pub owner: Option<Entity>,
    pub dir: Vec3,
    pub heading: Vec3,
    pub damage: f32,
    pub life: f32,
    pub hit_cd: f32,
    pub phase: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn fire_toll(
    v: &Volley,
    pid: u8,
    radius: f32,
    wisps: bool,
    stats: &crate::stats::Stats,
    assets: &WeaponAssets,
    enemies: &Foes,
    pots: &Query<(), With<Pot>>,
    commands: &mut Commands,
    hits: &mut MessageWriter<HitMsg>,
    sfx: &mut MessageWriter<SfxMsg>,
    ax: &mut ArsenalCx,
    rng: &mut impl Rng,
) {
    let r = radius * v.size;
    let here = v.center.normalize_or_zero();
    let cos_r = (r / ax.planet.radius).cos();
    for (e, _, en) in enemies.iter() {
        if !live_foe(e, en, pots) || en.dir.dot(here) < cos_r {
            continue;
        }
        let (cm, crit) = crate::combat::roll_crit(v.crit_ch, stats.crit_damage, rng);
        let elite = if en.elite { stats.elite_damage } else { 1.0 };
        let away = (en.dir - here * en.dir.dot(here)).normalize_or_zero();
        hits.write(HitMsg {
            source: Some(v.owner),
            target: e,
            amount: v.dmg * cm * elite,
            crit,
            knock: away * 3.0,
            by: HitBy::Weapon(v.kind),
        });
        commands.entity(e).try_insert(BellMark { secs: BELL_MARK_SECS });
        ax.tm.marks += 1;
    }
    // the toll you can see: two rings, the second a beat behind — "DONG... dong"
    spawn_wave(commands, assets, v.kind, here, r, 0.55, 0.0, ax.planet);
    spawn_wave(commands, assets, v.kind, here, r * 0.7, 0.5, 0.14, ax.planet);
    ax.tm.tolls += 1;
    ax.tolled = true;
    sfx.write(SfxMsg(Sfx::Toll));

    // THE ANGELUS: raise the freshest dead near the bell (each corpse only once). Host
    // only: the wisps reach every machine — this one included — as WeaponFx.
    if wisps && ax.simulating {
        let cutoff = ax.now - ANGELUS_MEMORY_SECS;
        ax.kills.list.retain(|(t, _)| *t >= cutoff);
        let mut raised = 0;
        let n = v.count as usize;
        let reach_cos = (ANGELUS_REACH / ax.planet.radius).cos();
        let mut i = ax.kills.list.len();
        while i > 0 && raised < n {
            i -= 1;
            let (_, dir) = ax.kills.list[i];
            if dir.dot(here) < reach_cos {
                continue;
            }
            ax.kills.list.remove(i);
            ax.fx.write(WeaponFxMsg { fx: WeaponFx::Wisp { owner: pid, dir, power: v.dmg * WISP_DAMAGE }, from_wire: false });
            raised += 1;
        }
    }
}

/// HOST: remember where foes die, for THE ANGELUS. Only while a bell that raises is carried
/// would it matter, but the list is short-lived and cheap either way.
pub fn record_kills(time: Res<Time>, mut kills: MessageReader<KillMsg>, mut recent: ResMut<RecentKills>) {
    let now = time.elapsed_secs();
    for k in kills.read() {
        if k.is_pot || k.is_boss || k.is_miniboss {
            continue;
        }
        recent.list.push_back((now, k.dir));
    }
    let cutoff = now - ANGELUS_MEMORY_SECS;
    while recent.list.front().is_some_and(|(t, _)| *t < cutoff) {
        recent.list.pop_front();
    }
    // a horde can die faster than bells ring: keep the newest few hundred
    while recent.list.len() > 400 {
        recent.list.pop_front();
    }
}

/// Every machine: age bell marks and keep a halo over (up to BELL_HALO_CAP of) them.
pub fn bell_marks(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    mut marked: Query<(Entity, &mut BellMark, &Transform, &Enemy)>,
    mut halos: Query<(Entity, &MarkHalo, &mut Transform), Without<BellMark>>,
) {
    let dt = time.delta_secs();
    for (e, mut m, _, _) in &mut marked {
        m.secs -= dt;
        if m.secs <= 0.0 {
            commands.entity(e).remove::<BellMark>();
        }
    }
    let t = time.elapsed_secs();
    let mut haloed: HashSet<Entity> = HashSet::new();
    for (he, halo, mut tf) in &mut halos {
        match marked.get(halo.target) {
            Ok((_, m, etf, en)) if m.secs > 0.0 => {
                haloed.insert(halo.target);
                let up = en.dir;
                tf.translation = etf.translation + up * (en.scale * 0.9 + 0.35);
                tf.rotation = sphere::frame_quat(up, sphere::tangent_frame(up).0) * Quat::from_rotation_y(t * 2.0);
                tf.scale = Vec3::new(0.32 * en.scale.max(0.8), 0.25, 0.32 * en.scale.max(0.8));
            }
            _ => commands.entity(he).despawn(),
        }
    }
    let mut count = haloed.len();
    for (e, m, _, _) in &marked {
        if count >= BELL_HALO_CAP {
            break;
        }
        if m.secs > 0.0 && !haloed.contains(&e) {
            commands.spawn((
                MarkHalo { target: e },
                Mesh3d(assets.ring_mesh.clone()),
                MeshMaterial3d(assets.mats[&WeaponKind::CosmonautsBell].clone()),
                Transform::from_scale(Vec3::ZERO),
                StageScoped,
            ));
            count += 1;
        }
    }
}

/// Every machine: a bell over each carrier's shoulder, swinging on its tolls.
#[allow(clippy::type_complexity)]
pub fn bell_bodies(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<WeaponAssets>,
    q_player: Query<(Entity, &Player, &PlayerState, &WeaponProcs, &Transform), Without<BellBody>>,
    mut bells: Query<(Entity, &BellBody, &mut Transform, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let now = time.elapsed_secs();
    let mut want: Vec<(Entity, WeaponKind)> = Vec::new();
    for (pe, _, ps, _, _) in &q_player {
        for w in &ps.weapons {
            if matches!(w.kind.def().behavior, Behavior::Toll { .. }) {
                want.push((pe, w.kind));
            }
        }
    }
    let mut have: Vec<Entity> = Vec::new();
    for (be, bell, mut tf, mut mat) in &mut bells {
        // one bell per owner; an evolution re-casts it in the new metal
        let Some(&(_, kind)) = want.iter().find(|(o, _)| *o == bell.owner) else {
            commands.entity(be).despawn();
            continue;
        };
        have.push(bell.owner);
        if bell.weapon != kind {
            mat.0 = assets.mats[&kind].clone();
            commands.entity(be).insert(BellBody { owner: bell.owner, weapon: kind });
        }
        let Ok((_, p, _, procs, ptf)) = q_player.get(bell.owner) else { continue };
        let up = p.dir;
        let (t, b) = sphere::tangent_frame(up);
        let side = {
            let f = p.facing - up * p.facing.dot(up);
            if f.length_squared() > 1e-4 { f.normalize().cross(up) } else { t }
        };
        let since = (now - procs.last_toll).max(0.0);
        let swing = (since * 13.0).sin() * (-since * 2.6).exp() * 0.9;
        // small, and off the shoulder: the chase camera looks past it, not through it
        let back = side.cross(up);
        tf.translation = ptf.translation + up * (1.45 + (now * 1.7).sin() * 0.06) + side * 0.85 + back * 0.25;
        tf.rotation = sphere::frame_quat(up, b) * Quat::from_rotation_x(swing);
        let s = if kind == WeaponKind::Angelus { 0.75 } else { 0.6 };
        tf.scale = Vec3::splat(s);
    }
    for (owner, kind) in want {
        if have.contains(&owner) {
            continue;
        }
        commands.spawn((
            BellBody { owner, weapon: kind },
            Mesh3d(assets.bell_mesh.clone()),
            MeshMaterial3d(assets.mats[&kind].clone()),
            Transform::from_scale(Vec3::ZERO),
            StageScoped,
        ));
    }
}

/// Every machine: wisps hunt the nearest foe and hit what they touch. Only the machine
/// that applies hits gave them an `owner`, so a joiner's wisps fly but never write a hit.
#[allow(clippy::too_many_arguments)]
pub fn wisp_update(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    particles: Option<Res<ParticleAssets>>,
    enemies: Query<(&Enemy, &Transform)>,
    pots: Query<(), With<Pot>>,
    mut q: Query<(Entity, &mut Wisp, &mut Transform), Without<Enemy>>,
    mut hits: MessageWriter<HitMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t = time.elapsed_secs();
    for (e, mut w, mut tf) in &mut q {
        w.life -= dt;
        w.hit_cd -= dt;
        if w.life <= 0.0 {
            if let Some(pa) = &particles {
                fx::burst(&mut commands, pa, tf.translation, w.dir, Pcolor::Gold, 6, 2.5);
            }
            commands.entity(e).despawn();
            continue;
        }
        let pos = tf.translation;
        let prey = hash
            .near(pos, 18.0)
            .filter(|(oe, _)| enemies.get(*oe).is_ok_and(|(o, _)| live_foe(*oe, o, &pots)))
            .min_by(|a, b| a.1.distance_squared(pos).total_cmp(&b.1.distance_squared(pos)));
        if let Some((pe, ppos)) = prey {
            let v = ppos - pos;
            let vt = (v - w.dir * v.dot(w.dir)).normalize_or_zero();
            if vt != Vec3::ZERO {
                w.heading = (w.heading + vt * 6.0 * dt).normalize_or_zero();
            }
            if let (Some(owner), true) = (w.owner, w.hit_cd <= 0.0) {
                let (en, _) = enemies.get(pe).expect("filtered above");
                let reach = 1.0 + en.scale * 0.5;
                if ppos.distance_squared(pos) < reach * reach {
                    hits.write(HitMsg {
                        source: Some(owner),
                        target: pe,
                        amount: w.damage,
                        crit: false,
                        knock: w.heading * 2.0,
                        by: HitBy::Weapon(WeaponKind::Angelus),
                    });
                    w.hit_cd = WISP_HIT_SECS;
                    tm.wisp_hits += 1;
                }
            }
        }
        let r = planet.surface(w.dir) + 1.4;
        let (nd, nv) = sphere::advance(w.dir, w.heading * WISP_SPEED, r, dt);
        w.dir = nd;
        if nv != Vec3::ZERO {
            w.heading = nv.normalize();
        }
        let bob = (t * 3.1 + w.phase).sin() * 0.3;
        tf.translation = planet.surface_point(w.dir) + w.dir * (1.4 + bob);
        tf.rotation = sphere::frame_quat(w.dir, w.heading);
        // fade in over the first half second, out over the last
        let s = 0.75 * (w.life / 0.6).min(1.0) * ((WISP_LIFE - w.life) / 0.5).clamp(0.2, 1.0);
        tf.scale = Vec3::splat(s);
    }
}

// ─── Yo-Yo of Damocles / SWORD-YO ────────────────────────────────────────────

/// One yo-yo on its cord. The cord is its own entity (a stretched cube), not a child, so
/// it can span owner to yo-yo in world space.
#[derive(Component)]
pub struct YoYoBody {
    pub owner: Entity,
    pub weapon: WeaponKind,
    pub idx: usize,
    pub tick: f32,
    /// SWORD-YO's garrote: foes the cord cut lately.
    pub cut_cd: HashMap<Entity, f32>,
}

#[derive(Component)]
pub struct YoYoCord {
    pub body: Entity,
}

/// The damage bonus the combo gives right now.
pub fn combo_mult(combo: f32) -> f32 {
    1.0 + YOYO_COMBO_DMG * combo.clamp(0.0, YOYO_COMBO_MAX)
}

/// HOST: build every Yo-Yo carrier's combo from metres moved; standing still bleeds it (a
/// hit clears it — `combat::apply_player_hits`). Mirrored to `NetWeaponVis` for joiners.
pub fn yoyo_combo(
    time: Res<Time>,
    mut q: Query<(&Player, &PlayerState, &mut WeaponProcs, &mut NetWeaponVis)>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (p, ps, mut procs, mut vis) in &mut q {
        let carries = ps.weapons.iter().any(|w| matches!(w.kind.def().behavior, Behavior::Tether { .. }));
        if !carries || ps.dead {
            procs.combo = 0.0;
            procs.combo_m = 0.0;
        } else {
            let speed = p.vel_t.length();
            if speed >= MOMENTUM_MIN_SPEED {
                procs.combo_m += speed * dt;
                while procs.combo_m >= YOYO_COMBO_METRES {
                    procs.combo_m -= YOYO_COMBO_METRES;
                    procs.combo = (procs.combo + 1.0).min(YOYO_COMBO_MAX);
                }
            } else {
                procs.combo = (procs.combo - YOYO_COMBO_DECAY * dt).max(0.0);
                procs.combo_m = 0.0;
            }
            tm.max_combo = tm.max_combo.max(procs.combo);
        }
        let next = NetWeaponVis { combo: procs.combo.floor() as u8 };
        if *vis != next {
            *vis = next;
        }
    }
}

/// CLIENT: our own combo is the host's (it decides who got hit).
pub fn adopt_my_weapon_vis(
    mine: Res<crate::net::MyPlayerId>,
    server: Query<(&PlayerId, &NetWeaponVis), Without<Player>>,
    mut q: Query<&mut WeaponProcs, (With<LocalPlayer>, With<Player>)>,
) {
    let Some(my_id) = mine.0 else { return };
    let Ok(mut procs) = q.single_mut() else { return };
    let Some((_, host)) = server.iter().find(|(pid, _)| pid.0 == my_id) else { return };
    if procs.combo.floor() as u8 != host.combo {
        procs.combo = host.combo as f32;
    }
}

/// Every machine: keep each carrier's yo-yos (and cords) in the air, swing them, and let
/// them hit; SWORD-YO at max combo pays the cord out and the cord itself cuts.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tether_update(
    mut commands: Commands,
    time: Res<Time>,
    hash: Res<SpatialHash>,
    assets: Res<WeaponAssets>,
    mut q_player: Query<(Entity, &Player, &PlayerState, &mut WeaponProcs, &Transform), (Without<YoYoBody>, Without<YoYoCord>)>,
    enemies: Query<&Enemy>,
    mut bodies: Query<(Entity, &mut YoYoBody, &mut Transform), (Without<Player>, Without<YoYoCord>)>,
    mut cords: Query<(Entity, &YoYoCord, &mut Transform), (Without<Player>, Without<YoYoBody>)>,
    mut hits: MessageWriter<HitMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let mut rng = rand::thread_rng();

    // what each carrier wants in the air: (owner, weapon, count, radius, dps, garrote, dmg)
    let mut want: Vec<(Entity, WeaponKind, usize, f32, f32, bool, f32)> = Vec::new();
    for (pe, _, ps, mut procs, _) in &mut q_player {
        let mut garrote_carried = false;
        if !ps.dead {
            let stats = ps.live_stats();
            for wi in &ps.weapons {
                let Behavior::Tether { radius, deg_per_sec, garrote } = wi.kind.def().behavior else { continue };
                let (dmg, count, size) = crate::combat::weapon_numbers(ps, &stats, wi);
                let reach = radius * size * stats.orbit * (1.0 + YOYO_COMBO_REACH * procs.combo);
                want.push((pe, wi.kind, count as usize, reach, deg_per_sec * stats.orbit, garrote, dmg * combo_mult(procs.combo)));
                garrote_carried |= garrote;
            }
        }
        // the cord pays out only at max combo, and snaps back fast when the combo breaks
        procs.payout = if garrote_carried && procs.combo >= YOYO_COMBO_MAX {
            (procs.payout + dt / SWORDYO_PAYOUT_SECS).min(1.0)
        } else {
            (procs.payout - dt * 2.0 / SWORDYO_PAYOUT_SECS).max(0.0)
        };
    }

    // reconcile: one body + cord per (owner, weapon, idx)
    let mut have: HashSet<(Entity, WeaponKind, usize)> = HashSet::new();
    for (be, body, _) in &bodies {
        let keep = want.iter().any(|w| w.0 == body.owner && w.1 == body.weapon && body.idx < w.2);
        if keep {
            have.insert((body.owner, body.weapon, body.idx));
        } else {
            commands.entity(be).despawn();
        }
    }
    for &(owner, weapon, count, ..) in &want {
        for idx in 0..count {
            if have.contains(&(owner, weapon, idx)) {
                continue;
            }
            let Ok((_, _, _, _, ptf)) = q_player.get(owner) else { continue };
            let body = commands
                .spawn((
                    YoYoBody { owner, weapon, idx, tick: 0.0, cut_cd: HashMap::new() },
                    Mesh3d(assets.yoyo_mesh.clone()),
                    MeshMaterial3d(assets.mats[&weapon].clone()),
                    Transform::from_translation(ptf.translation).with_scale(Vec3::ZERO),
                    StageScoped,
                ))
                .id();
            commands.spawn((
                YoYoCord { body },
                Mesh3d(assets.beam_mesh.clone()),
                MeshMaterial3d(assets.mats[&weapon].clone()),
                Transform::from_scale(Vec3::ZERO),
                StageScoped,
            ));
        }
    }

    // swing, hit, cut
    let mut hand_of: HashMap<Entity, Vec3> = HashMap::new();
    for (be, mut body, mut tf) in &mut bodies {
        let Some(&(owner, weapon, count, radius, dps, garrote, dmg)) =
            want.iter().find(|w| w.0 == body.owner && w.1 == body.weapon)
        else {
            continue;
        };
        let Ok((_, p, ps, procs, ptf)) = q_player.get(owner) else { continue };
        let up = p.dir;
        let hand = ptf.translation + up * 0.5;
        hand_of.insert(be, hand);
        let (tan, bit) = sphere::tangent_frame(up);
        let ang = t * dps.to_radians() + body.idx as f32 / count.max(1) as f32 * std::f32::consts::TAU;
        // the throw: out and back like a real yo-yo, twice a lap
        let throw = 0.72 + 0.28 * (ang * 2.0).sin().abs();
        let payout = if garrote { procs.payout } else { 0.0 };
        let r = radius * throw + (SWORDYO_GARROTE_RADIUS * ps.stats.size - radius * throw).max(0.0) * payout;
        let offset = (tan * ang.cos() + bit * ang.sin()) * r;
        tf.translation = hand + offset + up * 0.1;
        tf.rotation = sphere::frame_quat(up, offset.normalize_or_zero()) * Quat::from_rotation_x(t * 18.0);
        tf.scale = Vec3::splat(1.0 + procs.combo / YOYO_COMBO_MAX * 0.35);
        if dt <= 0.0 {
            continue;
        }

        // the yo-yo itself
        body.tick -= dt;
        if body.tick <= 0.0 {
            let mut hit_any = false;
            for (te, tpos) in hash.near(tf.translation, 1.6) {
                let Ok(en) = enemies.get(te) else { continue };
                let reach = 0.7 + en.scale * 0.5;
                if en.hp <= 0.0 || tpos.distance_squared(tf.translation) > reach * reach {
                    continue;
                }
                let (cm, crit) = crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng);
                let elite = if en.elite { ps.stats.elite_damage } else { 1.0 };
                hits.write(HitMsg {
                    source: Some(owner),
                    target: te,
                    amount: dmg * cm * elite,
                    crit,
                    knock: offset.normalize_or_zero() * 5.0 * ps.stats.knockback,
                    by: HitBy::Weapon(weapon),
                });
                tm.yoyo_hits += 1;
                hit_any = true;
            }
            if hit_any {
                body.tick = YOYO_HIT_SECS;
            }
        }

        // SWORD-YO: the paid-out cord garrotes everything along it
        body.cut_cd.retain(|_, c| {
            *c -= dt;
            *c > 0.0
        });
        if garrote && payout > 0.95 {
            let a = hand;
            let b = tf.translation;
            let len = a.distance(b);
            let steps = (len / 1.6).ceil() as usize;
            let mut cut: HashSet<Entity> = HashSet::new();
            for s in 0..=steps {
                let pt = a.lerp(b, s as f32 / steps.max(1) as f32);
                for (te, tpos) in hash.near(pt, 1.6) {
                    if cut.contains(&te) || body.cut_cd.contains_key(&te) {
                        continue;
                    }
                    let Ok(en) = enemies.get(te) else { continue };
                    let ab = b - a;
                    let k = ((tpos - a).dot(ab) / ab.length_squared().max(1e-4)).clamp(0.0, 1.0);
                    let reach = SWORDYO_CORD_WIDTH + en.scale * 0.5;
                    if en.hp <= 0.0 || tpos.distance_squared(a + ab * k) > reach * reach {
                        continue;
                    }
                    cut.insert(te);
                }
            }
            for te in cut {
                let (cm, crit) = crate::combat::roll_crit(ps.crit_chance(), ps.crit_damage(), &mut rng);
                let elite = if enemies.get(te).is_ok_and(|en| en.elite) { ps.stats.elite_damage } else { 1.0 };
                hits.write(HitMsg { source: Some(owner), target: te, amount: dmg * cm * elite, crit, knock: Vec3::ZERO, by: HitBy::Weapon(weapon) });
                body.cut_cd.insert(te, SWORDYO_REHIT_SECS);
                tm.garrote_hits += 1;
            }
        }
    }
    // cords span hand to yo-yo; taut and bright while garrotting
    for (ce, cord, mut tf) in &mut cords {
        let (Ok((_, body, btf)), Some(hand)) = (bodies.get(cord.body), hand_of.get(&cord.body)) else {
            if bodies.get(cord.body).is_err() {
                commands.entity(ce).despawn();
            }
            continue;
        };
        let taut = q_player.get(body.owner).map(|(_, _, _, pr, _)| pr.payout).unwrap_or(0.0);
        let len = hand.distance(btf.translation);
        let w = 0.035 + 0.07 * taut;
        tf.translation = (*hand + btf.translation) / 2.0;
        tf.rotation = Quat::from_rotation_arc(Vec3::Z, (btf.translation - *hand).normalize_or_zero());
        tf.scale = Vec3::new(w, w, len);
    }
}

// ─── the evolution fanfare (§12) ─────────────────────────────────────────────

/// Every machine: turn each sheet's base weapon becoming its evolution into a
/// `WeaponFx::Evolve`. The host watches every astronaut (a joiner's evolution reaches it in
/// its build heartbeat) and forwards the event to clients; a client watches only its own
/// sheet, so its fanfare plays the moment it picks the card.
pub fn detect_evolutions(
    q: Query<(Entity, &PlayerId, &PlayerState)>,
    mut seen: Local<HashMap<Entity, Vec<WeaponKind>>>,
    mut fx: MessageWriter<WeaponFxMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let mut live: HashSet<Entity> = HashSet::new();
    for (e, pid, ps) in &q {
        live.insert(e);
        let now: Vec<WeaponKind> = ps.weapons.iter().map(|w| w.kind).collect();
        if let Some(prev) = seen.get(&e) {
            for w in &now {
                // a base weapon we had turned into this evolution — not a sheet arriving with
                // it (a stage's fresh astronaut, a peer's first heartbeat)
                if !prev.contains(w) && w.evolved_from().is_some_and(|b| prev.contains(&b)) {
                    fx.write(WeaponFxMsg { fx: WeaponFx::Evolve { owner: pid.0, weapon: *w }, from_wire: false });
                    tm.evolutions += 1;
                }
            }
        }
        seen.insert(e, now);
    }
    seen.retain(|e, _| live.contains(e));
}

/// The gold shockwave ring of an evolution.
#[derive(Component)]
pub struct FanfareRing {
    pub dir: Vec3,
    pub age: f32,
}

/// One "assembly" shard flying in to the evolving astronaut.
#[derive(Component)]
pub struct FanfareShard {
    pub owner: Entity,
    pub from: Vec3,
    pub age: f32,
}

/// On the evolving astronaut: seconds since the fanfare began (the pop lands at
/// FANFARE_SHARD_SECS), and whether it is this machine's own player.
#[derive(Component)]
pub struct EvolvePop {
    pub age: f32,
    pub mine: bool,
    pub popped: bool,
}

/// The world's saturation dip for this machine's own evolution (virtual seconds in).
#[derive(Resource, Default)]
pub struct Fanfare {
    pub desat: Option<f32>,
}

/// Present weapon one-shots: an evolution's fanfare on whoever evolved, THE ANGELUS's
/// wisps. Host and client alike (a client's come off the hazard lane).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn weapon_fx_presentation(
    mut commands: Commands,
    mut msgs: MessageReader<WeaponFxMsg>,
    planet: Res<CurrentPlanet>,
    assets: Res<WeaponAssets>,
    enemy_assets: Option<Res<EnemyAssets>>,
    mut fanfare: ResMut<Fanfare>,
    local: Query<&PlayerId, With<LocalPlayer>>,
    bodies: Query<(Entity, &PlayerId, &Transform, Has<PlayerState>), Or<(With<Player>, With<crate::remote::RemoteAstronaut>)>>,
    wisps: Query<(Entity, &Wisp)>,
    mut banners: MessageWriter<BannerMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let local_pid = local.single().ok().map(|p| p.0);
    let mut rng = rand::thread_rng();
    for m in msgs.read() {
        match m.fx {
            WeaponFx::Evolve { owner, weapon } => {
                // our own evolution was presented when we picked it; the host's echo is late
                if m.from_wire && local_pid == Some(owner) {
                    continue;
                }
                let Some((be, _, btf, _)) = bodies.iter().find(|(_, pid, _, _)| pid.0 == owner) else { continue };
                let mine = local_pid == Some(owner);
                let dir = btf.translation.normalize_or_zero();
                commands.spawn((
                    FanfareRing { dir, age: 0.0 },
                    Mesh3d(assets.ring_mesh.clone()),
                    MeshMaterial3d(assets.fanfare_mat.clone()),
                    Transform::from_translation(btf.translation).with_scale(Vec3::ZERO),
                    StageScoped,
                ));
                let (t, b) = sphere::tangent_frame(dir);
                for i in 0..FANFARE_SHARDS {
                    let a = i as f32 / FANFARE_SHARDS as f32 * std::f32::consts::TAU;
                    let lift = rng.gen_range(-0.4..1.6);
                    let from = (t * a.cos() + b * a.sin()) * FANFARE_SHARD_RADIUS + dir * lift;
                    commands.spawn((
                        FanfareShard { owner: be, from, age: 0.0 },
                        Mesh3d(assets.drone_mesh.clone()),
                        MeshMaterial3d(if i % 2 == 0 { assets.mats[&weapon].clone() } else { assets.fanfare_mat.clone() }),
                        Transform::from_translation(btf.translation + from).with_scale(Vec3::splat(0.7)),
                        StageScoped,
                    ));
                }
                commands.entity(be).try_insert(EvolvePop { age: 0.0, mine, popped: false });
                if mine {
                    fanfare.desat = Some(0.0);
                    banners.write(BannerMsg(format!("WEAPON EVOLVED: {}", weapon.def().name)));
                } else {
                    banners.write(BannerMsg(format!("PLAYER {} EVOLVED {}", owner + 1, weapon.def().name)));
                }
            }
            WeaponFx::Wisp { owner, dir, power } => {
                let Some(ea) = &enemy_assets else { continue };
                // a bell keeps at most ANGELUS_WISP_CAP: the oldest fades for the newest
                let mut mine: Vec<(Entity, f32)> = wisps.iter().filter(|(_, w)| w.pid == owner).map(|(e, w)| (e, w.life)).collect();
                if mine.len() >= ANGELUS_WISP_CAP {
                    mine.sort_by(|a, b| a.1.total_cmp(&b.1));
                    for (e, _) in mine.iter().take(mine.len() + 1 - ANGELUS_WISP_CAP) {
                        commands.entity(*e).try_despawn();
                    }
                }
                // the hit belongs to the astronaut, where hits are applied (its sheet is here)
                let owner_e = bodies.iter().find(|(_, pid, _, sim)| pid.0 == owner && *sim).map(|(e, ..)| e);
                let (t, _) = sphere::tangent_frame(dir);
                commands.spawn((
                    Wisp {
                        pid: owner,
                        owner: owner_e,
                        dir,
                        heading: t,
                        damage: power,
                        life: WISP_LIFE,
                        hit_cd: 0.3,
                        phase: rng.gen_range(0.0..6.28),
                    },
                    Mesh3d(ea.meshes[&EnemyKind::Ghost].clone()),
                    MeshMaterial3d(assets.wisp_mat.clone()),
                    Transform::from_translation(planet.surface_point(dir) + dir * 1.4).with_scale(Vec3::ZERO),
                    StageScoped,
                ));
                tm.wisps += 1;
            }
        }
    }
}

/// Run the fanfares: the gold ring rolls out, the shards fly home, and on arrival the
/// astronaut pops (with, for our own: the brass sting, the §13 evolution shake, the white
/// flash — gone under flash reduction — and a solo hitstop). The world's colour drains and
/// returns meanwhile.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn animate_fanfare(
    mut commands: Commands,
    time: Res<Time>,
    save: Res<crate::save::MetaSave>,
    role: Res<crate::net::NetRole>,
    particles: Option<Res<ParticleAssets>>,
    mut fx_res: (ResMut<fx::Shake>, ResMut<fx::Hitstop>, ResMut<fx::ScreenFlash>, ResMut<Fanfare>),
    mut rings: Query<(Entity, &mut FanfareRing, &mut Transform), (Without<FanfareShard>, Without<EvolvePop>)>,
    mut shards: Query<(Entity, &mut FanfareShard, &mut Transform), (Without<FanfareRing>, Without<EvolvePop>)>,
    mut pops: Query<(Entity, &mut EvolvePop, &mut Transform), (Without<FanfareRing>, Without<FanfareShard>)>,
    mut grading: Query<&mut bevy::render::view::ColorGrading, With<crate::player::PlayerRig>>,
    mut sfx: MessageWriter<SfxMsg>,
    mut tm: ResMut<ArsenalTelemetry>,
) {
    let (shake, hitstop, flash, fanfare) = &mut fx_res;
    let dt = time.delta_secs();
    for (e, mut r, mut tf) in &mut rings {
        r.age += dt;
        let k = r.age / FANFARE_RING_SECS;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let grow = 1.0 - (1.0 - k).powi(3);
        let rad = (FANFARE_RING_RADIUS * grow).max(0.3);
        tf.rotation = sphere::frame_quat(r.dir, sphere::tangent_frame(r.dir).0);
        tf.scale = Vec3::new(rad, 2.2 * (1.0 - k) + 0.2, rad);
    }
    for (e, mut s, mut tf) in &mut shards {
        s.age += dt;
        let Ok((_, _, ptf)) = pops.get(s.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        let k = (s.age / FANFARE_SHARD_SECS).min(1.0);
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        // ease in: slow off the mark, slamming home
        let pull = k * k * k;
        tf.translation = ptf.translation + s.from * (1.0 - pull);
        tf.rotation = Quat::from_rotation_y(s.age * 20.0) * Quat::from_rotation_x(s.age * 13.0);
        tf.scale = Vec3::splat(0.7 * (1.0 - 0.5 * pull));
    }
    for (e, mut p, mut tf) in &mut pops {
        p.age += dt;
        let since = p.age - FANFARE_SHARD_SECS;
        if since >= 0.0 && !p.popped {
            p.popped = true;
            tm.fanfare_pops += 1;
            if let Some(pa) = &particles {
                let up = tf.translation.normalize_or_zero();
                fx::burst(&mut commands, pa, tf.translation, up, Pcolor::Gold, 26, 9.0);
                fx::burst(&mut commands, pa, tf.translation, up, Pcolor::White, 10, 6.0);
            }
            if p.mine {
                // the brass sting (P22 may re-voice Sfx::Evolve; this is where it plays)
                sfx.write(SfxMsg(Sfx::Evolve));
                shake.add(SHAKE_EVOLVE);
                flash.fire(Color::WHITE, &save);
                if fx::freezes(&role) {
                    hitstop.stop(0.18);
                }
            }
        }
        if since >= FANFARE_POP_SECS {
            tf.scale = Vec3::ONE;
            commands.entity(e).remove::<EvolvePop>();
            continue;
        }
        let punch = if since >= 0.0 { (std::f32::consts::PI * since / FANFARE_POP_SECS).sin() * 0.3 } else { -0.06 * (p.age / FANFARE_SHARD_SECS) };
        tf.scale = Vec3::splat(1.0 + punch);
    }
    // the world drains to grey and comes back; highlights (the gold, the sparks) keep
    // their colour, so the ring reads against it
    let sat = match fanfare.desat {
        Some(age) => {
            let age = age + dt;
            let k = age / FANFARE_DESAT_SECS;
            if k >= 1.0 {
                fanfare.desat = None;
                1.0
            } else {
                fanfare.desat = Some(age);
                let smooth = |a: f32, b: f32, x: f32| {
                    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
                    t * t * (3.0 - 2.0 * t)
                };
                let down = smooth(0.0, 0.12, k);
                let up = smooth(0.45, 1.0, k);
                1.0 + (FANFARE_DESAT - 1.0) * down * (1.0 - up)
            }
        }
        None => 1.0,
    };
    // relative to the toon grade's own section saturations (the world's look stays intact)
    let (shadows, midtones) = (crate::toon::TOON_SHADOW_SATURATION * sat, crate::toon::TOON_MIDTONE_SATURATION * sat);
    for mut g in &mut grading {
        if (g.midtones.saturation - midtones).abs() > 1e-4 || (g.shadows.saturation - shadows).abs() > 1e-4 {
            g.shadows.saturation = shadows;
            g.midtones.saturation = midtones;
        }
    }
}

/// Headless self-check of the pure parts: the combo curve and the evolution lookup the
/// fanfare hangs on.
pub fn self_check() -> Result<(), String> {
    if (combo_mult(0.0) - 1.0).abs() > 1e-5 || combo_mult(YOYO_COMBO_MAX * 3.0) > combo_mult(YOYO_COMBO_MAX) + 1e-5 {
        return Err("the Yo-Yo combo multiplier is not 1 at rest or not capped at max".into());
    }
    for w in [
        WeaponKind::RaguRain,
        WeaponKind::FullDischarge,
        WeaponKind::Omnidisc,
        WeaponKind::BrownNote,
        WeaponKind::Angelus,
        WeaponKind::SwordYo,
    ] {
        let Some(base) = w.evolved_from() else { return Err(format!("{} evolves from nothing", w.def().name)) };
        if base.def().evolves_to != Some(w) {
            return Err(format!("{} does not evolve into {}", base.def().name, w.def().name));
        }
    }
    // the six Tier-1 weapons fire their own behaviours, not the generic ones they borrowed
    for w in [
        WeaponKind::MeatballComet,
        WeaponKind::StaticCling,
        WeaponKind::RicochetDisc,
        WeaponKind::SonicWhoopee,
        WeaponKind::CosmonautsBell,
        WeaponKind::YoYo,
    ] {
        let generic = matches!(
            w.def().behavior,
            Behavior::MeleeArc { .. }
                | Behavior::Shot { .. }
                | Behavior::Seek { .. }
                | Behavior::Boomerang { .. }
                | Behavior::Beam { .. }
                | Behavior::Orbit { .. }
                | Behavior::Chain { .. }
                | Behavior::Rocket { .. }
                | Behavior::Aura { .. }
        );
        if generic {
            return Err(format!("{} still borrows a generic behaviour", w.def().name));
        }
    }
    if WeaponKind::CosmonautsBell.def().cooldown != 4.0 {
        return Err("Cosmonaut's Bell must toll every 4 s (§6)".into());
    }
    Ok(())
}

/// `--netlog`: every 5 s, the weapon one-shots this machine saw (evolutions by owner, wisps;
/// how many came off the wire) and what it draws — how a two-instance run proves a joiner's
/// evolution fanfare and THE ANGELUS's wisps crossed the lane both ways.
#[allow(clippy::type_complexity)]
pub fn log_weapon_fx(
    time: Res<Time>,
    role: Res<crate::net::NetRole>,
    mut msgs: MessageReader<WeaponFxMsg>,
    wisps: Query<(), With<Wisp>>,
    bells: Query<(), With<BellBody>>,
    yoyos: Query<(), With<YoYoBody>>,
    combos: Query<(&PlayerId, &NetWeaponVis)>,
    mut tally: Local<(Vec<(u8, bool)>, u32, u32)>,
    mut next: Local<f32>,
) {
    for m in msgs.read() {
        match m.fx {
            WeaponFx::Evolve { owner, .. } => tally.0.push((owner, m.from_wire)),
            WeaponFx::Wisp { .. } => {
                tally.1 += 1;
                tally.2 += u32::from(m.from_wire);
            }
        }
    }
    let now = time.elapsed_secs();
    if now < *next {
        return;
    }
    *next = now + 5.0;
    let combo: Vec<String> = combos.iter().map(|(pid, v)| format!("p{}:{}", pid.0, v.combo)).collect();
    info!(
        "WEAPONFX[{:?}] evolves(owner,wire)={:?} wisps={} (wire {}) | drawn: wisps={} bells={} yoyos={} | combo {}",
        *role,
        tally.0,
        tally.1,
        tally.2,
        wisps.iter().count(),
        bells.iter().count(),
        yoyos.iter().count(),
        combo.join(" ")
    );
}
