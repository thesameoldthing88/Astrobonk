//! Co-op set-pieces (GDD §11 "Opposite-pole ultimates", P18): STATIC CASCADE, the named duo
//! combos, and the squad tally the results screen reads.
//!
//! * STATIC CASCADE (`static_cascade`): two storm-callers (Tesla Coil / STORM CORE owners,
//!   both standing) at least CASCADE_MIN_SEP_DEG apart round the sphere charge a link; full,
//!   the whole planet is wrapped in a lightning belt along the great circle through both,
//!   and every foe in its band takes both owners' chain hit many times over.
//! * The named duos (`content::duos`): a teammate's SETUP family marks a foe (frost chills,
//!   a return weapon herds, rivets pin — `DuoLedger`, fed by `combat::apply_hits` from each
//!   hit's `HitBy`), then YOUR finisher family on that foe within DUO_WINDOW pays off (a
//!   shatter, a melt, a pin) and counts on the kill.
//!
//! All host-simulated; every machine sees the belt and the bursts (`coop::CoopFx` on the
//! hazard lane) and the tally (`RunState::feats`, in `RunSnapMsg`).

use crate::config::*;
use crate::content::characters::AstronautKind;
use crate::content::duos::{CoopFeat, Finisher, STORM_CALLERS};
use crate::coop::{CoopAssets, CoopFx, CoopFxMsg, CoopTelemetry, ForceKind, FriendlyForce};
use crate::enemies::{Boss, Enemy, SpatialHash};
use crate::interact::Pot;
use crate::messages::{HitBy, HitMsg};
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{Player, PlayerId};
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::prelude::*;
use std::collections::HashMap;

/// One co-op feat for one pair, tallied over the run. `a` did the first half (set up, or
/// rescued), `b` the second (finished, or was hauled up) — for STATIC CASCADE, the pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SquadFeat {
    pub feat: CoopFeat,
    /// (PlayerId, the suit they wore).
    pub a: (u8, AstronautKind),
    pub b: (u8, AstronautKind),
    pub count: u32,
}

impl SquadFeat {
    /// The results line: "DEEP FREEZE PROTOCOL x3 - P2 NOVA + P1 DOUG: chilled ranks shattered".
    pub fn line(&self) -> String {
        let d = self.feat.def();
        let join = if self.feat == CoopFeat::Rescue { "->" } else { "+" };
        format!(
            "{} x{}  P{} {} {join} P{} {}: {}",
            d.name,
            self.count,
            self.a.0 + 1,
            self.a.1.def().name,
            self.b.0 + 1,
            self.b.1.def().name,
            d.blurb
        )
    }
}

/// Count one feat for a pair. True the first time this pair lands it this run (the banner).
/// STATIC CASCADE's pair is unordered; the others keep who did which half.
pub fn tally(feats: &mut Vec<SquadFeat>, feat: CoopFeat, a: (u8, AstronautKind), b: (u8, AstronautKind)) -> bool {
    let same = |f: &SquadFeat| {
        f.feat == feat
            && ((f.a.0 == a.0 && f.b.0 == b.0) || (feat == CoopFeat::StaticCascade && f.a.0 == b.0 && f.b.0 == a.0))
    };
    if let Some(f) = feats.iter_mut().find(|f| same(f)) {
        f.count += 1;
        // a suit swap (a joiner's first build) keeps the pair's line honest
        f.a.1 = a.1;
        f.b.1 = b.1;
        return false;
    }
    feats.push(SquadFeat { feat, a, b, count: 1 });
    true
}

// ─── the named duos ───────────────────────────────────────────────────────────

/// HOST: who last set each marked foe up, per duo: (PlayerId + 1, when). Fed from every
/// hit in `combat::apply_hits`; entries age out after DUO_WINDOW (and with their foe).
#[derive(Resource, Default)]
pub struct DuoLedger {
    marks: HashMap<Entity, [(u8, f32); 3]>,
    next_prune: f32,
}

fn in_family(by: HitBy, family: &[crate::content::weapons::WeaponKind]) -> bool {
    matches!(by, HitBy::Weapon(w) if family.contains(&w))
}

fn finishes(feat: CoopFeat, by: HitBy) -> bool {
    match feat.finisher() {
        Finisher::Weapons(ws) => in_family(by, ws),
        Finisher::ThornsOr(ws) => by == HitBy::Thorns || in_family(by, ws),
    }
}

impl DuoLedger {
    /// A hit by astronaut `pid` with `by` on `target` at `now`. Returns the duo it FINISHES
    /// (and the teammate who set it up) if the foe carries a live mark of that duo from
    /// somebody else — you cannot combo with yourself — then records this hit's own setup
    /// mark, if it is a setup family.
    pub fn hit(&mut self, target: Entity, pid: u8, by: HitBy, now: f32) -> Option<(CoopFeat, u8)> {
        if by == HitBy::Other {
            return None;
        }
        let mut done = None;
        if let Some(marks) = self.marks.get(&target) {
            for (i, feat) in CoopFeat::DUOS.iter().enumerate() {
                let (who, at) = marks[i];
                if who != 0 && who - 1 != pid && now - at <= DUO_WINDOW && finishes(*feat, by) {
                    done = Some((*feat, who - 1));
                    break;
                }
            }
        }
        for (i, feat) in CoopFeat::DUOS.iter().enumerate() {
            if in_family(by, feat.setup()) {
                self.marks.entry(target).or_insert([(0, 0.0); 3])[i] = (pid + 1, now);
            }
        }
        done
    }

    /// Forget marks older than the window (their foe died, or nobody followed up).
    pub fn prune(&mut self, now: f32) {
        if now < self.next_prune {
            return;
        }
        self.next_prune = now + 1.0;
        self.marks.retain(|_, m| m.iter().any(|(who, at)| *who != 0 && now - *at <= DUO_WINDOW));
    }
}

/// HOST: a duo landed its finishing blow (written by `combat::apply_hits` on the kill).
#[derive(Message, Clone, Copy, Debug)]
pub struct DuoMsg {
    pub feat: CoopFeat,
    /// PlayerIds: who set it up, who finished it.
    pub setup: u8,
    pub finisher: u8,
    /// Where the foe fell, and how much it had — Deep Freeze's shatter is sized by it.
    pub dir: Vec3,
    pub max_hp: f32,
}

/// HOST: pay each landed duo off — tally it for the pair, shatter the chilled ranks around a
/// Deep Freeze kill (a burst of the fallen foe's max HP × DUO_SHATTER_FRAC to every foe
/// within DUO_SHATTER_RADIUS; `HitBy::Other`, so a shatter never chains into another), and
/// tell every machine (the first of a pair's duo with a banner; later ones as a burst, at
/// most one per DUO_FX_MIN_INTERVAL on the wire).
#[allow(clippy::too_many_arguments)]
pub fn duo_payoffs(
    time: Res<Time>,
    mut msgs: MessageReader<DuoMsg>,
    mut run: ResMut<RunState>,
    planet: Res<CurrentPlanet>,
    hash: Res<SpatialHash>,
    squad: Query<(&PlayerId, &PlayerState)>,
    enemies: Query<&Enemy, Without<Pot>>,
    mut hits: MessageWriter<HitMsg>,
    mut fx: MessageWriter<CoopFxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
    mut last_fx: Local<f32>,
) {
    let now = time.elapsed_secs();
    let fallback = run.character;
    for m in msgs.read() {
        let hero = |id: u8| squad.iter().find(|(p, _)| p.0 == id).map(|(_, ps)| ps.character).unwrap_or(fallback);
        let (a, b) = ((m.setup, hero(m.setup)), (m.finisher, hero(m.finisher)));
        let first = tally(&mut run.feats, m.feat, a, b);
        telemetry.feats[m.feat.code() as usize] += 1;
        if m.feat == CoopFeat::DeepFreeze {
            let at = planet.surface_point(m.dir);
            let burst = m.max_hp * DUO_SHATTER_FRAC;
            eprintln!("DBG shatter at {:?} cands {:?}", m.dir, hash.near(at, DUO_SHATTER_RADIUS + 1.5).map(|(e, p)| (e, enemies.get(e).map(|en| (en.hp, sphere::arc_dist(en.dir, m.dir, planet.radius))).ok(), p.distance(at))).collect::<Vec<_>>());
            for (te, _) in hash.near(at, DUO_SHATTER_RADIUS + 1.5) {
                let Ok(en) = enemies.get(te) else { continue };
                if en.hp <= 0.0 || sphere::arc_dist(en.dir, m.dir, planet.radius) > DUO_SHATTER_RADIUS {
                    continue;
                }
                let away = (en.dir - m.dir * en.dir.dot(m.dir)).normalize_or_zero();
                hits.write(HitMsg { source: None, target: te, amount: burst, crit: false, knock: away * 6.0, by: HitBy::Other });
                telemetry.shatter_hits += 1;
            }
        }
        if first || now - *last_fx >= DUO_FX_MIN_INTERVAL {
            *last_fx = now;
            fx.write(CoopFxMsg { fx: CoopFx::Duo { feat: m.feat, a: m.setup, b: m.finisher, dir: m.dir, first }, from_wire: false });
        }
        if first {
            info!("COOP duo {} landed: P{} + P{}", m.feat.def().name, m.setup + 1, m.finisher + 1);
        }
    }
}

// ─── STATIC CASCADE ───────────────────────────────────────────────────────────

/// A new run: no link charging, no marks on foes that no longer exist.
pub fn reset_squad_state(mut state: ResMut<CascadeState>, mut ledger: ResMut<DuoLedger>) {
    *state = CascadeState::default();
    *ledger = DuoLedger::default();
}

/// HOST: the link between the best-separated pair of standing storm-callers.
#[derive(Resource, Default)]
pub struct CascadeState {
    pub cooldown: f32,
    /// The pair the charge belongs to — a different pair starts over.
    pub pair: Option<(u8, u8)>,
}

/// An astronaut's storm-caller chain hit, as `combat::weapon_fire` rolls it before crits.
fn storm_hit(ps: &PlayerState) -> Option<f32> {
    ps.weapons
        .iter()
        .filter(|w| STORM_CALLERS.contains(&w.kind))
        .map(|w| {
            let evo = if w.kind.is_evolution() { ps.stats.evo_damage } else { 1.0 };
            w.kind.def().damage * w.kind.level_scaling(w.level).0 * ps.damage_mult() * evo
        })
        .max_by(|a, b| a.total_cmp(b))
}

/// HOST: STATIC CASCADE. While two standing storm-callers stand at least
/// CASCADE_MIN_SEP_DEG apart, the link charges (`RunState::cascade_charge`, which every HUD
/// shows); stepping closer, going down or a third party taking the lead lets it drain. Full,
/// it fires along the great circle through both — the whole sphere's belt — and every foe
/// within CASCADE_BAND of that circle takes both owners' chain hit × CASCADE_DAMAGE_MULT
/// (bosses a share); teammates standing in the belt are jolted (friendly physics). One pass
/// over the horde, once per cascade.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn static_cascade(
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut run: ResMut<RunState>,
    mut state: ResMut<CascadeState>,
    callers: Query<(Entity, &PlayerId, &Player, &PlayerState, &Transform)>,
    enemies: Query<(Entity, &Enemy, Has<Boss>), (Without<Pot>, Without<crate::enemies::Buried>)>,
    mut hits: MessageWriter<HitMsg>,
    mut forces: MessageWriter<FriendlyForce>,
    mut fx: MessageWriter<CoopFxMsg>,
    mut telemetry: ResMut<CoopTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    state.cooldown = (state.cooldown - dt).max(0.0);
    let owners: Vec<(Entity, u8, Vec3, f32, AstronautKind)> = callers
        .iter()
        .filter(|(_, _, _, ps, _)| !ps.dead)
        .filter_map(|(e, id, p, ps, _)| storm_hit(ps).map(|h| (e, id.0, p.dir, h, ps.character)))
        .collect();
    // the widest-apart pair that clears the bar
    let min_cos = CASCADE_MIN_SEP_DEG.to_radians().cos();
    let mut best: Option<(usize, usize, f32)> = None;
    for i in 0..owners.len() {
        for j in i + 1..owners.len() {
            let c = owners[i].2.dot(owners[j].2);
            if c <= min_cos && best.is_none_or(|(_, _, bc)| c < bc) {
                best = Some((i, j, c));
            }
        }
    }
    let Some((i, j, _)) = best.filter(|_| state.cooldown <= 0.0) else {
        run.cascade_charge = (run.cascade_charge - dt / CASCADE_CHARGE_SECS).max(0.0);
        return;
    };
    let pair = (owners[i].1.min(owners[j].1), owners[i].1.max(owners[j].1));
    if state.pair != Some(pair) {
        state.pair = Some(pair);
        run.cascade_charge = 0.0;
    }
    run.cascade_charge += dt / CASCADE_CHARGE_SECS;
    if run.cascade_charge < 1.0 {
        return;
    }
    run.cascade_charge = 0.0;
    state.cooldown = CASCADE_COOLDOWN;
    let (a, b) = (&owners[i], &owners[j]);
    // the great circle through both; two exact antipodes have no one circle — then the one
    // through the first owner's facing
    let axis = a.2.cross(b.2).try_normalize().unwrap_or_else(|| {
        let (t, _) = sphere::tangent_frame(a.2);
        a.2.cross(t).normalize()
    });
    let damage = (a.3 + b.3) * CASCADE_DAMAGE_MULT;
    let half_band = CASCADE_BAND / planet.radius; // as a sine of the angle off the circle
    for (te, en, boss) in &enemies {
        if en.hp <= 0.0 {
            continue;
        }
        if en.dir.dot(axis).abs() > half_band {
            continue;
        }
        let amount = if boss { damage * CASCADE_BOSS_MULT } else { damage };
        // alternate the credit so each owner's lifesteal/elite bonuses are honoured
        let src = if telemetry.cascade_hits % 2 == 0 { a.0 } else { b.0 };
        hits.write(HitMsg { source: Some(src), target: te, amount, crit: true, knock: Vec3::ZERO, by: HitBy::Other });
        telemetry.cascade_hits += 1;
    }
    // friendly physics: every OTHER astronaut standing in the belt is jolted
    for (e, _, p, ps, tf) in &callers {
        if e != a.0 && e != b.0 && !ps.dead && p.dir.dot(axis).abs() <= half_band {
            forces.write(FriendlyForce { from: a.0, at: tf.translation, radius: 0.5, cone: None, kind: ForceKind::Jolt });
        }
    }
    tally(&mut run.feats, CoopFeat::StaticCascade, (a.1, a.4), (b.1, b.4));
    telemetry.cascades += 1;
    telemetry.feats[CoopFeat::StaticCascade.code() as usize] += 1;
    fx.write(CoopFxMsg { fx: CoopFx::Cascade { a: a.1, b: b.1, axis }, from_wire: false });
    info!("COOP STATIC CASCADE: P{} + P{} ({:.0} per foe)", a.1 + 1, b.1 + 1, damage);
}

/// The belt on screen: segments round the great circle, crackling for CASCADE_VIS_SECS.
#[derive(Component)]
pub struct CascadeBelt {
    axis: Vec3,
    /// Where round the circle this segment starts and ends (radians).
    from: f32,
    to: f32,
    life: f32,
    /// Photosensitivity: no crackle, a smooth fade.
    calm: bool,
}

const BELT_SEGMENTS: usize = 72;

/// Draw STATIC CASCADE's belt round the whole planet (every machine, from `CoopFx`).
pub fn spawn_cascade_belt(commands: &mut Commands, assets: &CoopAssets, planet: &CurrentPlanet, axis: Vec3, calm: bool) {
    let axis = axis.normalize_or_zero();
    if axis == Vec3::ZERO {
        return;
    }
    let step = std::f32::consts::TAU / BELT_SEGMENTS as f32;
    for k in 0..BELT_SEGMENTS {
        let (from, to) = (k as f32 * step, (k + 1) as f32 * step);
        commands.spawn((
            CascadeBelt { axis, from, to, life: CASCADE_VIS_SECS, calm },
            Mesh3d(assets.belt_mesh.clone()),
            MeshMaterial3d(assets.belt_mat.clone()),
            // placed on its first update
            Transform::from_translation(axis * planet.radius).with_scale(Vec3::ZERO),
            bevy::light::NotShadowCaster,
            StageScoped,
        ));
    }
}

/// A point on the belt's circle `ang` radians round, at ground level plus the lift, knocked
/// `jitter` off it (up/down, and sideways across the ground) — the lightning's zigzag.
fn belt_point(planet: &CurrentPlanet, axis: Vec3, ang: f32, jitter: Vec2) -> Vec3 {
    let (t, _) = sphere::tangent_frame(axis);
    let dir = (Quat::from_axis_angle(axis, ang) * t).normalize();
    planet.surface_point(dir) + dir * (CASCADE_VIS_LIFT + jitter.y) + axis * jitter.x
}

/// EVERY machine: crackle the belt (each segment's ends jump a little every frame — the
/// lightning read; in photosensitivity mode they hold still and fade) and let it die.
pub fn animate_cascade_belts(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    mut q: Query<(Entity, &mut CascadeBelt, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    for (e, mut belt, mut tf) in &mut q {
        belt.life -= dt;
        if belt.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        let fade = (belt.life / CASCADE_VIS_SECS).clamp(0.0, 1.0);
        // each joint of the belt jumps about ~12 times a second (a crackle, not a strobe:
        // the light never goes out) — held still in photosensitivity mode
        let crackle = |ang: f32| {
            if belt.calm {
                return Vec2::ZERO;
            }
            let k = (t * 12.0).floor();
            Vec2::new((ang * 53.0 + k * 1.7).sin() * 0.9, (ang * 37.0 + k * 2.3).sin() * 0.6)
        };
        let a = belt_point(&planet, belt.axis, belt.from, crackle(belt.from));
        let b = belt_point(&planet, belt.axis, belt.to, crackle(belt.to));
        let len = a.distance(b);
        tf.translation = (a + b) * 0.5;
        tf.rotation = Quat::from_rotation_arc(Vec3::Z, (b - a).normalize_or_zero());
        let w = 0.5 * fade.sqrt();
        tf.scale = Vec3::new(w, w, len);
    }
}
