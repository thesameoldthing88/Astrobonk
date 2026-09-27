//! GDD §9 "Glitched" elite affixes (P10). Elites are larger, glowing, loot-guaranteed
//! variants that roll 1-3 stacked affixes (`content::enemies::roll_affixes`, on
//! `Scaling::affix_extra`), each telegraphed by an aura colour — and, for the colourblind, by
//! its aura ring's shape:
//!
//! | Affix | Effect | Where |
//! |---|---|---|
//! | Overclocked | +60% move and attack speed | `stat_mods`, `affix_upkeep` |
//! | Leaden | huge HP, a gravity well on death | `stat_mods`, `affix_deaths`, `gravity_wells` |
//! | Warden | regenerating front shield | `combat::apply_hits` (`guard_hit`), `affix_upkeep` |
//! | Contagious | buds a copy at HP thresholds | `combat::apply_hits` (`AffixCore::splits`), `contagious_splits` |
//! | Magnetar | bends your projectiles into itself | `combat::projectile_move` (`magnetar_bend`) |
//! | Nightborne | untouchable by day, doubled bite at night | `combat::apply_hits` (`guard_hit`), `affix_upkeep` |
//! | Meteoric | leaps skyward, slams a ring where you stood | `meteor_leaps` |
//! | Cursed-Touched | a Legendary for the killing blow, a mini-Static | `affix_deaths`, `affix_fx_presentation` |
//!
//! Split like the rest of the horde. SIMULATION is the host's (`net::is_simulating`), for
//! every astronaut. What a client must SEE comes three ways: the affix bits ride the crowd
//! lane's SPAWN descriptor (one extra byte, elites only — `netenemy::stream_enemies`), so a
//! proxy is built already wearing its aura; a Warden's shield and a Meteoric's altitude ride
//! the enemy-state lane as `AffixVis`; and the one-shots (a well opening, a bud, a shield
//! breaking, a slam, a Cursed death) ride the hazard lane as `AffixFxMsg`. Everything that
//! hurts is the host's.
//!
//! Crowd rules hold: the body is the kind's own mesh with one shared material per affix
//! (`EnemyAssets::affix_mats`), and the dressing — aura rings, a shield plate, a leap's
//! shadow — is loose entities sharing one mesh and material per affix. Elites are a handful
//! at a time, never the 1,200.

use crate::bestiary::{self, SelfSteered, TelegraphOwner};
use crate::config::*;
use crate::content::enemies::{Affix, AffixSet, EnemyKind};
use crate::enemies::{self, Beamer, Boss, Buried, Enemy, EnemyAssets, Lobber, Spitter, Stunned, Telegraph};
use crate::fx::{self, ParticleAssets, Pcolor, Shake};
use crate::messages::*;
use crate::planet::{CurrentPlanet, StageScoped};
use crate::player::{LocalPlayer, Player, PlayerId};
use crate::run::{PlayerState, RunState};
use crate::sphere;
use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

// ─── components ─────────────────────────────────────────────────────────────

/// An elite's stacked affixes. The host rolls them at spawn; a client reads them off the
/// spawn descriptor, so its proxy wears them from its first frame.
#[derive(Component, Clone, Copy, Debug)]
pub struct Affixes(pub AffixSet);

/// What an affixed elite SHOWS that its position does not say: a Warden's shield (0 = down,
/// else its strength 0..1) and facing (world tangent), a Meteoric's height over the ground.
/// The host writes it; a client gets it on the enemy-state lane.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AffixVis {
    pub shield: f32,
    pub alt: f32,
    pub heading: Vec3,
}

impl AffixVis {
    /// Only these affixes have a look to stream.
    pub fn wanted(set: AffixSet) -> bool {
        set.has(Affix::Warden) || set.has(Affix::Meteoric)
    }
}

/// HOST: an affixed elite's bookkeeping.
#[derive(Component, Clone, Copy, Debug)]
pub struct AffixCore {
    /// Its pace and bite as spawned (Nightborne scales from these by night).
    pub base_speed: f32,
    pub base_damage: f32,
    /// Seconds before its next IMMUNE / BLOCK read-out may show.
    pub readout: f32,
    /// Contagious: the next `CONTAGIOUS_THRESHOLDS` entry still to bud at.
    pub split_next: usize,
}

impl AffixCore {
    /// Contagious: buds owed after a hit that left it at `frac` of its max HP (alive).
    pub fn splits(&mut self, frac: f32) -> u32 {
        let mut n = 0;
        while self.split_next < CONTAGIOUS_THRESHOLDS.len() && frac < CONTAGIOUS_THRESHOLDS[self.split_next] {
            self.split_next += 1;
            n += 1;
        }
        n
    }
    /// A read-out may show now (and the throttle restarts).
    pub fn readout_ready(&mut self) -> bool {
        let ready = self.readout <= 0.0;
        if ready {
            self.readout = AFFIX_READOUT_SECS;
        }
        ready
    }
}

/// HOST: a Warden's front shield.
#[derive(Component, Clone, Copy, Debug)]
pub struct WardenShield {
    pub hp: f32,
    pub max: f32,
    /// World tangent it faces, turned at WARDEN_TURN_RATE toward its mark.
    pub facing: Vec3,
    /// Seconds it stays broken (0 = up).
    pub down: f32,
    pub since_hit: f32,
}

/// HOST: a Meteoric's leap — waiting (`air` 0, `cd` counting down) or airborne from `from`
/// to `to` with `air` seconds left.
#[derive(Component, Clone, Copy, Debug)]
pub struct MeteorLeap {
    pub cd: f32,
    pub air: f32,
    pub from: Vec3,
    pub to: Vec3,
}

/// A Leaden elite's death well, on every machine (the host's crushes; a client's copy only
/// drags its own astronaut, as the host's drags the host's copy of it).
#[derive(Component, Clone, Copy, Debug)]
pub struct GravityWell {
    pub dir: Vec3,
    pub left: f32,
    pub max: f32,
    /// Damage per contact tick in the core (0 on a client's copy).
    pub crush: f32,
    pub tick: f32,
}

/// An elite already named on this machine's callout banner.
#[derive(Component)]
pub struct AffixCalled;

/// One loose piece of an affixed elite's dressing, drawn on every machine.
#[derive(Component, Clone, Copy, Debug)]
pub struct AffixDress {
    pub owner: Entity,
    pub part: DressPart,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DressPart {
    /// The `slot`-th aura ring (inner first), in `affix`'s colour and shape.
    Ring { slot: u8, affix: Affix },
    /// A Warden's front shield plate.
    Shield,
    /// A Meteoric's shadow while it is in the air.
    Shadow,
}

#[derive(Resource)]
pub struct AffixAssets {
    /// Per `Affix::index`: its aura ring (segmented by `ring_segments`) and glow material.
    pub rings: Vec<Handle<Mesh>>,
    pub ring_mats: Vec<Handle<StandardMaterial>>,
    pub shield_mesh: Handle<Mesh>,
    pub shield_mat: Handle<StandardMaterial>,
    pub well_mesh: Handle<Mesh>,
    pub well_mat: Handle<StandardMaterial>,
}

/// Counters for the headless probe and `--netlog`.
#[derive(Resource, Default, Debug)]
pub struct AffixTelemetry {
    /// Affixed elites spawned, per `Affix::index`, and by stack size (1..=3).
    pub rolled: [u32; 8],
    pub stacks: [u32; 4],
    pub immune: u32,
    pub warden_blocks: u32,
    pub warden_breaks: u32,
    pub splits: u32,
    pub bends: u32,
    pub leaps: u32,
    pub slams: u32,
    pub wells: u32,
    /// Metres astronauts were dragged by wells.
    pub dragged: f32,
    pub crushes: u32,
    pub mini_static: u32,
    pub legendaries: u32,
    pub callouts: u32,
    /// One-shots that arrived from the host (a client's count).
    pub fx_seen: u32,
}

// ─── one-shots ──────────────────────────────────────────────────────────────

/// The affixes' one-shots, said locally on the host and rebuilt from the hazard lane on a
/// client (`from_wire`), so `affix_fx_presentation` draws them the same.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AffixFx {
    /// A Leaden elite fell at `dir`: its gravity well stands for `secs`.
    Well { dir: Vec3, secs: f32 },
    /// A Contagious elite budded a copy at `dir`.
    Split { dir: Vec3 },
    /// A Warden's shield broke at `dir`.
    ShieldBreak { dir: Vec3 },
    /// A Meteoric elite came down at `dir`.
    Slam { dir: Vec3 },
    /// A Cursed-Touched elite died at `dir`; astronaut `owner` (PlayerId) takes its Legendary.
    Cursed { dir: Vec3, owner: u8 },
}

#[derive(Message, Clone, Copy, Debug)]
pub struct AffixFxMsg {
    pub fx: AffixFx,
    pub from_wire: bool,
}

/// HOST: a Contagious elite owes a bud (`combat::apply_hits` -> `contagious_splits`).
#[derive(Message, Clone, Copy, Debug)]
pub struct AffixSplitMsg {
    pub kind: EnemyKind,
    pub dir: Vec3,
}

// ─── spawning ───────────────────────────────────────────────────────────────

/// What an affix set does to a fresh elite's numbers: (HP, pace, bite) multipliers.
pub fn stat_mods(set: AffixSet) -> (f32, f32, f32) {
    let (mut hp, mut speed, mut dmg) = (1.0, 1.0, 1.0);
    if set.has(Affix::Overclocked) {
        speed *= OVERCLOCK_SPEED;
    }
    if set.has(Affix::Leaden) {
        hp *= LEADEN_HP;
        speed *= LEADEN_SPEED;
    }
    if set.has(Affix::CursedTouched) {
        hp *= CURSED_TOUCHED_HP;
        dmg *= CURSED_TOUCHED_DMG;
    }
    (hp, speed, dmg)
}

/// Give a fresh elite what its affixes need (`enemies::spawn_enemy_affixed`, host).
pub fn attach(cmd: &mut EntityCommands, set: AffixSet, dir: Vec3, en: &Enemy, rng: &mut impl Rng) {
    if set.is_empty() {
        return;
    }
    let facing = sphere::tangent_frame(dir).0;
    cmd.insert((
        Affixes(set),
        AffixCore { base_speed: en.speed, base_damage: en.damage, readout: 0.0, split_next: 0 },
    ));
    if AffixVis::wanted(set) {
        let shield = if set.has(Affix::Warden) { 1.0 } else { 0.0 };
        cmd.insert(AffixVis { shield, alt: 0.0, heading: facing });
    }
    if set.has(Affix::Warden) {
        let max = en.max_hp * WARDEN_SHIELD_FRAC;
        cmd.insert(WardenShield { hp: max, max, facing, down: 0.0, since_hit: 0.0 });
    }
    if set.has(Affix::Meteoric) {
        cmd.insert(MeteorLeap { cd: rng.gen_range(METEOR_CD.0..METEOR_CD.1), air: 0.0, from: dir, to: dir });
    }
}

/// The components a client proxy of an affixed elite is built with (`netenemy`).
pub fn proxy_bundle(cmd: &mut EntityCommands, set: AffixSet, dir: Vec3) {
    if set.is_empty() {
        return;
    }
    cmd.insert(Affixes(set));
    if AffixVis::wanted(set) {
        let shield = if set.has(Affix::Warden) { 1.0 } else { 0.0 };
        cmd.insert(AffixVis { shield, alt: 0.0, heading: sphere::tangent_frame(dir).0 });
    }
}

// ─── hit hooks (combat::apply_hits) ─────────────────────────────────────────

/// What an elite's affixes did to one hit landing on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guard {
    /// The hit lands as it came.
    Open,
    /// Nightborne in the day: nothing lands at all.
    Immune,
    /// A Warden's shield took `soaked` of it (and `broke` if that was its last).
    Shield { soaked: f32, broke: bool },
}

/// §9 Nightborne and Warden, for one hit of `amount` on an elite at `at`: the day makes a
/// Nightborne untouchable; a Warden's shield soaks what comes at its face (the hit's line
/// is its knockback, else where its shooter stands — `bestiary::cone_blocks`, the Aegis
/// Drone's rule) until it breaks, and a hit that breaks it carries on through with the rest.
pub fn guard_hit(set: AffixSet, at: Vec3, night: bool, shield: Option<&mut WardenShield>, knock: Vec3, shooter: Option<Vec3>, amount: &mut f32) -> Guard {
    if set.has(Affix::Nightborne) && !night {
        *amount = 0.0;
        return Guard::Immune;
    }
    let Some(sh) = shield else { return Guard::Open };
    if sh.down > 0.0 || sh.hp <= 0.0 || !bestiary::cone_blocks(at, sh.facing, knock, shooter, WARDEN_SHIELD_HALF) {
        return Guard::Open;
    }
    let soaked = amount.min(sh.hp);
    sh.hp -= soaked;
    sh.since_hit = 0.0;
    *amount -= soaked;
    let broke = sh.hp <= 1e-3;
    if broke {
        sh.hp = 0.0;
        sh.down = WARDEN_DOWN_SECS;
    }
    Guard::Shield { soaked, broke }
}

/// §9 Magnetar, for one of your projectiles at `pos` flying `heading` over `up`: the
/// heading bent toward every Magnetar within reach (full pull at its body, none at the
/// edge), a seeker's by only `MAGNETAR_SEEKER_RESIST` of it — it homes through. Returns
/// whether it bent.
pub fn magnetar_bend(heading: &mut Vec3, pos: Vec3, up: Vec3, magnetars: &[Vec3], seeker: bool, dt: f32) -> bool {
    let mut bent = false;
    for m in magnetars {
        let v = *m - pos;
        let d = v.length();
        if d >= MAGNETAR_RADIUS || d < 0.05 {
            continue;
        }
        let to = (v - up * v.dot(up)).normalize_or_zero();
        if to == Vec3::ZERO {
            continue;
        }
        let resist = if seeker { MAGNETAR_SEEKER_RESIST } else { 1.0 };
        let rate = MAGNETAR_PULL * (1.0 - d / MAGNETAR_RADIUS) * resist;
        let turned = bestiary::turn_toward(*heading, to, up, rate * dt);
        if turned != *heading {
            *heading = turned;
            bent = true;
        }
    }
    bent
}

// ─── simulation (host) ──────────────────────────────────────────────────────

/// HOST, every frame: Overclocked cooldowns run fast, Nightborne takes its night pace and
/// bite from where it stands, a Warden's shield turns toward its mark, regrows and comes
/// back after a break — and the streamed look (`AffixVis`) follows.
#[allow(clippy::type_complexity)]
pub fn affix_upkeep(
    time: Res<Time>,
    run: Res<RunState>,
    planet: Res<CurrentPlanet>,
    q_player: Query<(&Player, &PlayerState), Without<Enemy>>,
    mut q: Query<(
        &mut Enemy,
        &Affixes,
        &mut AffixCore,
        Option<&mut WardenShield>,
        Option<&mut AffixVis>,
        (Option<&mut Spitter>, Option<&mut Beamer>, Option<&mut Lobber>, Option<&mut bestiary::Trencher>),
        Has<bestiary::Rollo>,
        Has<Stunned>,
    )>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let sun = crate::daynight::Sun::of(&run);
    let marks: Vec<Vec3> = q_player.iter().filter(|(_, ps)| !ps.dead).map(|(p, _)| p.dir).collect();
    for (mut e, aff, mut core, shield, vis, (spit, beam, lob, trench), rollo, stunned) in &mut q {
        let set = aff.0;
        core.readout = (core.readout - dt).max(0.0);
        if set.has(Affix::Overclocked) {
            // the attacker systems already ticked dt; this is the extra 60%
            let extra = (OVERCLOCK_ATTACK - 1.0) * dt;
            if let Some(mut s) = spit {
                s.cd -= extra;
            }
            if let Some(mut b) = beam {
                if b.charging <= 0.0 {
                    b.cd -= extra;
                }
            }
            if let Some(mut l) = lob {
                l.cd -= extra;
            }
            if let Some(mut t) = trench {
                t.cd -= extra;
            }
            e.contact_cd = (e.contact_cd - extra).max(0.0);
        }
        if set.has(Affix::Nightborne) {
            let night = sun.is_night(e.dir);
            let bite = if night { NIGHTBORNE_NIGHT_DMG } else { 1.0 };
            if rollo {
                // a Rollo re-derives its bite from its roll every frame (`rollo_roll`)
                e.damage *= bite;
            } else {
                e.damage = core.base_damage * bite;
                e.speed = core.base_speed * if night { NIGHTBORNE_NIGHT_SPEED } else { 1.0 };
            }
        }
        if let Some(mut sh) = shield {
            if sh.down > 0.0 {
                sh.down -= dt;
                if sh.down <= 0.0 {
                    sh.down = 0.0;
                    sh.hp = sh.max;
                }
            } else {
                sh.since_hit += dt;
                if sh.since_hit >= WARDEN_REGEN_DELAY {
                    sh.hp = (sh.hp + sh.max * WARDEN_REGEN_RATE * dt).min(sh.max);
                }
            }
            let up = e.dir;
            let mut facing = (sh.facing - up * sh.facing.dot(up)).normalize_or_zero();
            if facing == Vec3::ZERO {
                facing = sphere::tangent_frame(up).0;
            }
            if !stunned {
                if let Some(t) = marks
                    .iter()
                    .copied()
                    .min_by(|a, b| sphere::arc_dist(up, *a, planet.radius).total_cmp(&sphere::arc_dist(up, *b, planet.radius)))
                {
                    facing = bestiary::turn_toward(facing, t - up * t.dot(up), up, WARDEN_TURN_RATE * dt);
                }
            }
            sh.facing = facing;
            if let Some(mut v) = vis {
                v.shield = if sh.down > 0.0 { 0.0 } else { (sh.hp / sh.max.max(1e-3)).max(0.05) };
                v.heading = facing;
            }
        }
    }
}

/// HOST: the Meteoric leap. Grounded, it waits out its cooldown (Overclocked: faster), then —
/// with a standing astronaut in reach — hurls itself up and marks where that astronaut
/// stands with a ring telegraph that dies with it (`TelegraphOwner`, streamed as
/// `OwnedTelegraph`). It flies there on a great-circle arc, APEX high, and lands as the
/// ring goes off (the ring's own `telegraphs` resolution is the hit).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn meteor_leaps(
    mut commands: Commands,
    time: Res<Time>,
    planet: Res<CurrentPlanet>,
    assets: Res<EnemyAssets>,
    q_player: Query<(&Player, &PlayerState), Without<Enemy>>,
    mut q: Query<(Entity, &mut Enemy, &mut Transform, &mut MeteorLeap, &mut AffixVis, &Affixes, Has<Stunned>, Has<Buried>)>,
    mut fx_out: MessageWriter<AffixFxMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let marks: Vec<Vec3> = q_player.iter().filter(|(_, ps)| !ps.dead).map(|(p, _)| p.dir).collect();
    let mut rng = rand::thread_rng();
    for (entity, mut e, mut tf, mut leap, mut vis, aff, stunned, buried) in &mut q {
        if leap.air > 0.0 {
            // airborne: nothing else steers it (SelfSteered), so it ticks its own decays
            enemies::tick_enemy(&mut e, dt);
            leap.air = (leap.air - dt).max(0.0);
            let s = 1.0 - leap.air / METEOR_AIR_SECS;
            e.dir = leap.from.slerp(leap.to, s).normalize();
            e.knock = Vec3::ZERO;
            vis.alt = 4.0 * METEOR_APEX * s * (1.0 - s);
            let up = e.dir;
            let fwd = (leap.to - up * leap.to.dot(up)).normalize_or_zero();
            tf.translation = planet.surface_point(up) + up * (e.hover + e.scale * 0.6 + vis.alt);
            // tucked into a tumble on the way up, feet-first on the way down
            tf.rotation = sphere::frame_quat(up, if fwd == Vec3::ZERO { sphere::tangent_frame(up).0 } else { fwd })
                * Quat::from_rotation_x(-std::f32::consts::TAU * s.min(0.75) / 0.75);
            tf.scale = Vec3::splat(e.scale);
            if leap.air <= 0.0 {
                vis.alt = 0.0;
                leap.cd = rng.gen_range(METEOR_CD.0..METEOR_CD.1);
                commands.entity(entity).remove::<SelfSteered>();
                tele.slams += 1;
                fx_out.write(AffixFxMsg { fx: AffixFx::Slam { dir: e.dir }, from_wire: false });
            }
            continue;
        }
        let pace = if aff.0.has(Affix::Overclocked) { OVERCLOCK_ATTACK } else { 1.0 };
        leap.cd -= dt * pace;
        if leap.cd > 0.0 || stunned || buried {
            continue;
        }
        let target = marks
            .iter()
            .copied()
            .map(|m| (m, sphere::arc_dist(e.dir, m, planet.radius)))
            .filter(|(_, arc)| (METEOR_MIN_ARC..=METEOR_MAX_ARC).contains(arc))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((to, _)) = target else {
            leap.cd = 0.4; // look again shortly
            continue;
        };
        leap.from = e.dir;
        leap.to = to;
        leap.air = METEOR_AIR_SECS;
        tele.leaps += 1;
        commands.entity(entity).insert(SelfSteered);
        commands.spawn((
            enemies::telegraph_bundle(
                &assets,
                &planet,
                Telegraph {
                    timer: METEOR_AIR_SECS,
                    max: METEOR_AIR_SECS,
                    radius: METEOR_RING_RADIUS,
                    damage: e.damage * METEOR_SLAM_MULT,
                    dir: to,
                    ring: true,
                },
            ),
            TelegraphOwner(entity),
        ));
    }
}

/// HOST: what an affixed elite leaves when it dies. Leaden opens its gravity well; a
/// Cursed-Touched raises a mini-Static around its corpse and names the astronaut that
/// takes its Legendary — the killing blow's (§11), or the nearest standing one when no
/// astronaut landed it.
#[allow(clippy::too_many_arguments)]
pub fn affix_deaths(
    mut commands: Commands,
    mut kills: MessageReader<KillMsg>,
    assets: Res<EnemyAssets>,
    affix_assets: Res<AffixAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_ps: Query<(Entity, &Player, &PlayerState, &PlayerId)>,
    mut fx_out: MessageWriter<AffixFxMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let mut rng = rand::thread_rng();
    let mut sc = None;
    for k in kills.read() {
        if k.affixes.is_empty() {
            continue;
        }
        let sc = *sc.get_or_insert_with(|| crate::run::scaling::Scaling::for_run(&run, crate::run::scaling::living_party(q_ps.iter().map(|(_, _, ps, _)| ps))));
        if k.affixes.has(Affix::Leaden) {
            let bite = k.kind.map_or(10.0, |kind| kind.def().damage) * crate::content::enemies::EliteMods::DMG * sc.dmg;
            spawn_well(&mut commands, &affix_assets, &assets, &planet, k.dir, LEADEN_WELL_SECS, bite * LEADEN_WELL_CRUSH);
            tele.wells += 1;
            fx_out.write(AffixFxMsg { fx: AffixFx::Well { dir: k.dir, secs: LEADEN_WELL_SECS }, from_wire: false });
        }
        if k.affixes.has(Affix::CursedTouched) {
            for i in 0..CURSED_MINI_STATIC {
                let heading = {
                    let (t, b) = sphere::tangent_frame(k.dir);
                    let a = std::f32::consts::TAU * (i as f32 + rng.gen_range(0.0..0.6)) / CURSED_MINI_STATIC as f32;
                    t * a.cos() + b * a.sin()
                };
                let dir = sphere::offset_dir(k.dir, heading, rng.gen_range(CURSED_MINI_STATIC_ARC.0..CURSED_MINI_STATIC_ARC.1), planet.radius);
                enemies::spawn_enemy_at(&mut commands, &assets, &planet, EnemyKind::Ghost, dir, false, &sc, &mut rng);
                tele.mini_static += 1;
            }
            let killer = k.by.and_then(|b| q_ps.get(b).ok()).filter(|(_, _, ps, _)| !ps.claimed).map(|(_, _, _, pid)| pid.0);
            let owner = killer.or_else(|| {
                q_ps.iter()
                    .filter(|(_, _, ps, _)| !ps.dead)
                    .min_by(|a, b| sphere::arc_dist(a.1.dir, k.dir, planet.radius).total_cmp(&sphere::arc_dist(b.1.dir, k.dir, planet.radius)))
                    .map(|(_, _, _, pid)| pid.0)
            });
            if let Some(owner) = owner {
                fx_out.write(AffixFxMsg { fx: AffixFx::Cursed { dir: k.dir, owner }, from_wire: false });
            }
        }
    }
}

/// HOST: a Contagious elite's buds — a plain foe of its kind beside it, `CONTAGIOUS_SPLIT_HP`
/// times as tough as the crowd's (they are not elites: no aura, no elite loot).
#[allow(clippy::too_many_arguments)]
pub fn contagious_splits(
    mut commands: Commands,
    mut reader: MessageReader<AffixSplitMsg>,
    assets: Res<EnemyAssets>,
    planet: Res<CurrentPlanet>,
    run: Res<RunState>,
    q_ps: Query<&PlayerState>,
    mut fx_out: MessageWriter<AffixFxMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let mut rng = rand::thread_rng();
    for m in reader.read() {
        let sc = crate::run::scaling::Scaling::for_run(&run, crate::run::scaling::living_party(q_ps.iter()));
        let heading = {
            let (t, b) = sphere::tangent_frame(m.dir);
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            t * a.cos() + b * a.sin()
        };
        let dir = sphere::offset_dir(m.dir, heading, 1.8, planet.radius);
        enemies::spawn_enemy(&mut commands, &assets, &planet, m.kind, dir, false, sc.hp * CONTAGIOUS_SPLIT_HP, sc.dmg, &mut rng);
        tele.splits += 1;
        fx_out.write(AffixFxMsg { fx: AffixFx::Split { dir: m.dir }, from_wire: false });
    }
}

/// Open a Leaden well at `dir` (host with its crush, a client's copy with none).
pub fn spawn_well(commands: &mut Commands, assets: &AffixAssets, enemy_assets: &EnemyAssets, planet: &CurrentPlanet, dir: Vec3, secs: f32, crush: f32) {
    commands
        .spawn((
            GravityWell { dir, left: secs, max: secs, crush, tick: CONTACT_TICK },
            Mesh3d(assets.well_mesh.clone()),
            MeshMaterial3d(assets.well_mat.clone()),
            enemies::ground_decal(planet, dir, 0.07).with_scale(Vec3::splat(LEADEN_WELL_RADIUS)),
            StageScoped,
        ))
        .with_children(|c| {
            // the crushing core is danger: the palette's telegraph red (§13), not the well's brown
            c.spawn((
                Mesh3d(enemy_assets.ring_mesh.clone()),
                MeshMaterial3d(enemy_assets.ring_mat.clone()),
                Transform::from_translation(Vec3::Y * 0.02).with_scale(Vec3::splat(LEADEN_WELL_CORE / LEADEN_WELL_RADIUS)),
            ));
        });
}

/// Every machine: each gravity well drags the astronauts THIS machine moves toward its core
/// (the host: all of them — a joiner's copy included; a joiner: its own predicted body, from
/// its copy of the well) — so both machines move a joiner the same way. The host also
/// drags the horde and crushes whoever stands in the core. Wells spin down and close.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn gravity_wells(
    mut commands: Commands,
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    hash: Res<enemies::SpatialHash>,
    mut wells: Query<(Entity, &mut GravityWell, &mut Transform)>,
    mut bodies: Query<(Entity, &mut Player, &PlayerState, &Transform), Without<GravityWell>>,
    mut horde: Query<&mut Enemy, (Without<Boss>, Without<crate::interact::Pot>)>,
    mut hits: MessageWriter<PlayerHitMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let dt = time.delta_secs();
    let Some(planet) = planet else { return };
    if dt <= 0.0 {
        return;
    }
    let r = planet.radius;
    for (we, mut w, mut tf) in &mut wells {
        w.left -= dt;
        if w.left <= 0.0 {
            commands.entity(we).despawn();
            continue;
        }
        w.tick -= dt;
        let crush_now = w.crush > 0.0 && w.tick <= 0.0;
        if crush_now {
            w.tick = CONTACT_TICK;
        }
        let center = planet.surface_point(w.dir);
        for (pe, mut p, ps, ptf) in &mut bodies {
            if ps.dead {
                continue;
            }
            let arc = sphere::arc_dist(p.dir, w.dir, r);
            if arc >= LEADEN_WELL_RADIUS {
                continue;
            }
            // strongest at the core, nothing at the rim; never past the centre
            let pull = LEADEN_WELL_PULL * (1.0 - arc / LEADEN_WELL_RADIUS);
            let step = (pull * dt).min(arc);
            p.dir = sphere::step_toward(p.dir, w.dir, step / r);
            tele.dragged += step;
            if crush_now && arc < LEADEN_WELL_CORE {
                hits.write(PlayerHitMsg { victim: pe, amount: w.crush, from: ptf.translation, attacker: None });
                tele.crushes += 1;
            }
        }
        if w.crush > 0.0 {
            for (he, _) in hash.near(center, LEADEN_WELL_RADIUS + 1.0) {
                let Ok(mut en) = horde.get_mut(he) else { continue };
                if en.speed <= 0.0 {
                    continue;
                }
                let arc = sphere::arc_dist(en.dir, w.dir, r);
                if arc < LEADEN_WELL_RADIUS && arc > 0.3 {
                    let step = (LEADEN_WELL_ENEMY_PULL * (1.0 - arc / LEADEN_WELL_RADIUS) * dt).min(arc);
                    en.dir = sphere::step_toward(en.dir, w.dir, step / r);
                }
            }
        }
        // the spiral winds inward, and the well shrinks shut over its last half second
        let close = (w.left / 0.5).min(1.0);
        let open = ((w.max - w.left) / 0.25).min(1.0);
        let mut t = enemies::ground_decal(&planet, w.dir, 0.07);
        t.rotation *= Quat::from_rotation_y(-(w.max - w.left) * 2.4);
        t.scale = Vec3::splat(LEADEN_WELL_RADIUS * close * open);
        *tf = t;
    }
}

// ─── presentation (every machine) ───────────────────────────────────────────

/// CLIENT: a Meteoric proxy flies at the altitude the host streams (`drive_proxies` drew it
/// on the ground).
pub fn affix_lift(mut q: Query<(&AffixVis, &Enemy, &mut Transform), With<crate::netenemy::NetEnemy>>) {
    for (vis, e, mut tf) in &mut q {
        if vis.alt > 0.0 {
            tf.translation += e.dir * vis.alt;
        }
    }
}

/// Is an elite out of sight (a Burrower under the ground, a Trencher tunnelling)?
fn hidden(e: &Enemy, buried: bool, vis: Option<&bestiary::EnemyVis>) -> bool {
    buried
        || (e.kind == EnemyKind::Trencher
            && vis.is_some_and(|v| matches!(v.state, bestiary::EnemyVis::SINKING | bestiary::EnemyVis::TUNNEL)))
}

/// The aura on every machine: a ring per affix round the elite's feet (inner = primary),
/// each its own colour AND shape, spinning (an Overclocked one fast; a Nightborne one
/// breathing while the day keeps it untouchable); a Warden's shield plate where the
/// shield faces, thinning as it drains and gone while broken; a Meteoric's shadow under it
/// in the air. Loose entities, reaped with their elite.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn affix_dress(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<RunState>,
    planet: Option<Res<CurrentPlanet>>,
    assets: Option<Res<AffixAssets>>,
    shadows: Option<Res<bestiary::BestiaryAssets>>,
    q: Query<(Entity, &Enemy, &Affixes, Option<&AffixVis>, Has<Buried>, Option<&bestiary::EnemyVis>)>,
    mut dress: Query<(Entity, &AffixDress, &mut Transform, &mut Visibility), Without<Enemy>>,
    mut have: Local<std::collections::HashSet<Entity>>,
) {
    let (Some(planet), Some(assets), Some(shadows)) = (planet, assets, shadows) else { return };
    let t_now = time.elapsed_secs();
    let sun = crate::daynight::Sun::of(&run);
    struct Look {
        dir: Vec3,
        scale: f32,
        vis: AffixVis,
        hidden: bool,
    }
    let mut live: HashMap<Entity, (AffixSet, Look)> = HashMap::new();
    for (e, en, aff, vis, buried, evis) in &q {
        let look = Look { dir: en.dir, scale: en.scale, vis: vis.copied().unwrap_or_default(), hidden: hidden(en, buried, evis) };
        live.insert(e, (aff.0, look));
    }
    for (de, d, mut tf, mut shown) in &mut dress {
        let Some((_, look)) = live.get(&d.owner) else {
            commands.entity(de).try_despawn();
            continue;
        };
        let up = look.dir;
        let base = sphere::frame_quat(up, sphere::tangent_frame(up).0);
        let show = match d.part {
            DressPart::Ring { slot, affix } => {
                let dirn = if slot % 2 == 0 { 1.0 } else { -1.0 };
                let spin = if affix == Affix::Overclocked { 4.5 } else { 0.8 };
                let mut radius = look.scale * 0.8 + 0.45 + 0.4 * slot as f32;
                if affix == Affix::Nightborne {
                    radius *= if sun.is_night(up) { 1.12 } else { 1.0 + 0.07 * (t_now * 7.0).sin() };
                }
                *tf = Transform::from_translation(planet.surface_point(up) + up * (0.1 + 0.02 * slot as f32))
                    .with_rotation(base * Quat::from_rotation_y(dirn * spin * t_now + slot as f32))
                    .with_scale(Vec3::new(radius, 1.0, radius));
                !look.hidden
            }
            DressPart::Shield => {
                let facing = (look.vis.heading - up * look.vis.heading.dot(up)).normalize_or_zero();
                let s = look.scale * 0.85 + 0.35;
                let strength = look.vis.shield.clamp(0.0, 1.0);
                *tf = Transform::from_translation(planet.surface_point(up) + up * 0.05)
                    .with_rotation(sphere::frame_quat(up, if facing == Vec3::ZERO { sphere::tangent_frame(up).0 } else { facing }))
                    .with_scale(Vec3::new(s, look.scale * (0.55 + 0.6 * strength), s));
                // lifted with its bearer mid-leap
                tf.translation += up * look.vis.alt;
                !look.hidden && strength > 0.02
            }
            DressPart::Shadow => {
                let k = 0.55 + 0.6 * (1.0 - look.vis.alt / METEOR_APEX).clamp(0.0, 1.0);
                let mut t = enemies::ground_decal(&planet, up, 0.08);
                t.scale = Vec3::splat(look.scale * k);
                *tf = t;
                look.vis.alt > 0.25
            }
        };
        *shown = if show { Visibility::Visible } else { Visibility::Hidden };
    }
    have.retain(|owner| live.contains_key(owner));
    for (owner, (set, look)) in &live {
        if have.contains(owner) {
            continue;
        }
        have.insert(*owner);
        let at = enemies::ground_decal(&planet, look.dir, 0.1).with_scale(Vec3::ZERO);
        for (slot, affix) in set.iter().enumerate() {
            commands.spawn((
                AffixDress { owner: *owner, part: DressPart::Ring { slot: slot as u8, affix } },
                Mesh3d(assets.rings[affix.index()].clone()),
                MeshMaterial3d(assets.ring_mats[affix.index()].clone()),
                at,
                Visibility::Hidden,
                StageScoped,
            ));
        }
        if set.has(Affix::Warden) {
            commands.spawn((
                AffixDress { owner: *owner, part: DressPart::Shield },
                Mesh3d(assets.shield_mesh.clone()),
                MeshMaterial3d(assets.shield_mat.clone()),
                at,
                Visibility::Hidden,
                bevy::light::NotShadowCaster,
                StageScoped,
            ));
        }
        if set.has(Affix::Meteoric) {
            commands.spawn((
                AffixDress { owner: *owner, part: DressPart::Shadow },
                Mesh3d(shadows.shadow_mesh.clone()),
                MeshMaterial3d(shadows.shadow_mat.clone()),
                at,
                Visibility::Hidden,
                StageScoped,
            ));
        }
    }
}

/// The callout: an elite coming within `AFFIX_CALLOUT_ARC` of THIS machine's astronaut is
/// named once — its affixes, and §9's nickname for a named pair or the counterplay for the
/// rest — at most one banner per `AFFIX_CALLOUT_GAP` (an elite left unnamed by the gap is
/// named when it clears, if it is still close).
#[allow(clippy::type_complexity)]
pub fn affix_callouts(
    mut commands: Commands,
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    me: Query<&Player, With<LocalPlayer>>,
    q: Query<(Entity, &Enemy, &Affixes), Without<AffixCalled>>,
    mut last: Local<Option<f32>>,
    mut banners: MessageWriter<BannerMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let (Some(planet), Ok(me)) = (planet, me.single()) else { return };
    let now = time.elapsed_secs();
    if last.is_some_and(|t| now - t < AFFIX_CALLOUT_GAP && now >= t) {
        return;
    }
    let Some((e, _, aff)) = q
        .iter()
        .map(|(e, en, aff)| (e, sphere::arc_dist(en.dir, me.dir, planet.radius), aff))
        .filter(|(_, arc, _)| *arc < AFFIX_CALLOUT_ARC)
        .min_by(|a, b| a.1.total_cmp(&b.1))
    else {
        return;
    };
    banners.write(BannerMsg(callout(aff.0)));
    commands.entity(e).try_insert(AffixCalled);
    *last = Some(now);
    tele.callouts += 1;
}

/// "ELITE: OVERCLOCKED METEORIC - THE POGO GOBLIN" / "ELITE: WARDEN - FLANK THE CURVE".
pub fn callout(set: AffixSet) -> String {
    let tail = set.nickname().or_else(|| set.primary().map(|a| a.def().counterplay)).unwrap_or("");
    format!("ELITE: {} - {tail}", set.label())
}

/// The affixes' one-shots, on every machine (the host's local messages, a client's from the
/// hazard lane): a client opens its copy of a well (which drags its own astronaut); bursts
/// for a bud, a broken shield, a landing; and a Cursed-Touched death pays its Legendary on
/// the machine of the astronaut it names — that machine owns its sheet (a joiner's reaches
/// the host with its next build), so the item is rolled there, from its own pool and luck.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn affix_fx_presentation(
    mut commands: Commands,
    mut reader: MessageReader<AffixFxMsg>,
    planet: Option<Res<CurrentPlanet>>,
    (assets, enemy_assets, particles): (Option<Res<AffixAssets>>, Option<Res<EnemyAssets>>, Option<Res<ParticleAssets>>),
    (mut shake, run, save): (ResMut<Shake>, Res<RunState>, Res<crate::save::MetaSave>),
    mut me: Query<(&Player, &mut PlayerState, &PlayerId), With<LocalPlayer>>,
    mut banners: MessageWriter<BannerMsg>,
    mut sfx: MessageWriter<SfxMsg>,
    mut tele: ResMut<AffixTelemetry>,
) {
    let (Some(planet), Some(assets), Some(enemy_assets)) = (planet, assets, enemy_assets) else {
        reader.clear();
        return;
    };
    let burst = |commands: &mut Commands, dir: Vec3, color: Pcolor, n: usize, speed: f32| {
        if let Some(pa) = &particles {
            fx::burst(commands, pa, planet.surface_point(dir), dir, color, n, speed);
        }
    };
    for m in reader.read() {
        if m.from_wire {
            tele.fx_seen += 1;
        }
        match m.fx {
            AffixFx::Well { dir, secs } => {
                if m.from_wire {
                    spawn_well(&mut commands, &assets, &enemy_assets, &planet, dir, secs, 0.0);
                }
                burst(&mut commands, dir, Pcolor::Gold, 14, 5.0);
                sfx.write(SfxMsg(Sfx::Slam));
            }
            AffixFx::Split { dir } => burst(&mut commands, dir, Pcolor::Green, 12, 6.0),
            AffixFx::ShieldBreak { dir } => {
                burst(&mut commands, dir, Pcolor::Cyan, 14, 7.0);
                sfx.write(SfxMsg(Sfx::Crit));
            }
            AffixFx::Slam { dir } => {
                let near = me
                    .single()
                    .map_or(0.0, |(p, ..)| (1.0 - sphere::arc_dist(p.dir, dir, planet.radius) / SHAKE_ENEMY_SLAM_FALLOFF).clamp(0.0, 1.0));
                // the host's ring already burst and shook as it resolved (`enemies::telegraphs`)
                if m.from_wire {
                    burst(&mut commands, dir, Pcolor::Danger, 18, 9.0);
                    shake.add(SHAKE_ENEMY_SLAM_MAX * near);
                }
                if near > 0.0 {
                    sfx.write(SfxMsg(Sfx::Slam));
                }
            }
            AffixFx::Cursed { dir, owner } => {
                burst(&mut commands, dir, Pcolor::Gold, 26, 9.0);
                burst(&mut commands, dir, Pcolor::Purple, 12, 5.0);
                let Ok((_, mut ps, pid)) = me.single_mut() else { continue };
                if pid.0 != owner {
                    continue;
                }
                tele.legendaries += 1;
                let mut rng = rand::thread_rng();
                match crate::run::roll_legendary(&ps, &mut rng) {
                    Some(item) => {
                        let opt = crate::run::UpgradeOption::NewItem(item, crate::content::Rarity::Legendary);
                        ps.apply_upgrade(&opt, &save, run.greed_stacks);
                        banners.write(BannerMsg(format!("CURSED-TOUCHED LOOT: {} (LEGENDARY)", item.def().name.to_ascii_uppercase())));
                    }
                    None => {
                        ps.gold += CURSED_LEGENDARY_GOLD;
                        banners.write(BannerMsg(format!("CURSED-TOUCHED LOOT: +{CURSED_LEGENDARY_GOLD} GOLD")));
                    }
                }
                sfx.write(SfxMsg(Sfx::LevelUp));
            }
        }
    }
}

/// Every machine: count the affixed elites that appear (the host's spawns, a client's
/// proxies), per affix and by stack size.
pub fn affix_census(q: Query<&Affixes, Added<Affixes>>, mut tele: ResMut<AffixTelemetry>) {
    for a in &q {
        for x in a.0.iter() {
            tele.rolled[x.index()] += 1;
        }
        tele.stacks[(a.0.len() as usize).min(3)] += 1;
    }
}

/// `--netlog`: once a second, the Glitched elites on this machine (both sides; a joiner's
/// `fx_seen` counts one-shots that crossed the wire, `dressed` the aura pieces it draws).
pub fn log_affixes(
    time: Res<Time>,
    tele: Res<AffixTelemetry>,
    q: Query<(&Affixes, Option<&AffixVis>)>,
    dress: Query<&AffixDress>,
    mut next: Local<f32>,
) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 1.0;
    let live: Vec<String> = q
        .iter()
        .map(|(a, v)| match v {
            Some(v) => format!("{}[sh{:.2} alt{:.1}]", a.0.label().replace(' ', "+"), v.shield, v.alt),
            None => a.0.label().replace(' ', "+"),
        })
        .collect();
    info!(
        "AFFIX live={} [{}] dressed={} rolled={:?} stacks={:?} immune={} blocks={} breaks={} splits={} bends={} leaps={} slams={} wells={} dragged={:.1}m crushes={} mini_static={} legendaries={} callouts={} fx_seen={}",
        live.len(),
        live.join(" "),
        dress.iter().count(),
        tele.rolled,
        &tele.stacks[1..],
        tele.immune,
        tele.warden_blocks,
        tele.warden_breaks,
        tele.splits,
        tele.bends,
        tele.leaps,
        tele.slams,
        tele.wells,
        tele.dragged,
        tele.crushes,
        tele.mini_static,
        tele.legendaries,
        tele.callouts,
        tele.fx_seen
    );
}

// ─── assets ─────────────────────────────────────────────────────────────────

/// One aura ring of radius 1 in the XZ plane, cut into `segments` arcs (1 = whole) — and
/// Cursed-Touched's crown of teeth. The affix reads by shape as well as colour (§13).
fn ring_mesh(segments: u32, crown: bool) -> Mesh {
    let mut m = crate::meshkit::MeshData::new();
    let n = segments.max(1);
    // each segment spans 62% of its share of the circle (all of it when whole)
    let span = if n == 1 { std::f32::consts::TAU } else { std::f32::consts::TAU / n as f32 * 0.62 };
    let pieces = if n == 1 { 28 } else { (28 / n).max(3) };
    let piece = span / pieces as f32;
    let (w, h) = (0.085, 0.05);
    for s in 0..n {
        let a0 = std::f32::consts::TAU * s as f32 / n as f32;
        for p in 0..pieces {
            let a = a0 + piece * (p as f32 + 0.5);
            let len = 2.0 * (piece * 0.5).sin() * 1.02 + 0.01;
            m.add_box(
                Vec3::new(w, h, len),
                Transform::from_translation(Vec3::new(a.cos(), 0.0, a.sin())).with_rotation(Quat::from_rotation_y(-a)),
                Color::WHITE,
            );
        }
        if crown {
            let a = a0 + span * 0.5;
            m.add_box(
                Vec3::new(0.07, 0.34, 0.07),
                Transform::from_translation(Vec3::new(a.cos(), 0.17, a.sin())).with_rotation(Quat::from_rotation_y(-a)),
                Color::WHITE,
            );
        }
    }
    m.build()
}

/// A Warden's shield: a curved plate of the cone it guards, radius 1 around local -Z (the
/// facing), a little taller than wide.
fn shield_mesh() -> Mesh {
    let mut m = crate::meshkit::MeshData::new();
    let pieces = 9;
    let half = WARDEN_SHIELD_HALF;
    for i in 0..pieces {
        let a = -half + 2.0 * half * (i as f32 + 0.5) / pieces as f32;
        // around -Z: a = 0 is straight ahead
        let (x, z) = (a.sin(), -a.cos());
        let len = 2.0 * (half / pieces as f32).sin() * 1.04;
        m.add_box(
            Vec3::new(len, 1.2, 0.07),
            Transform::from_translation(Vec3::new(x, 0.6, z)).with_rotation(Quat::from_rotation_y(-a)),
            Color::WHITE,
        );
    }
    m.build()
}

/// The well: three spiral arms winding into the centre (the spin reads as a pull).
fn well_mesh() -> Mesh {
    let mut m = crate::meshkit::MeshData::new();
    for arm in 0..3 {
        let a0 = std::f32::consts::TAU * arm as f32 / 3.0;
        let steps = 16;
        for i in 0..steps {
            let f = i as f32 / steps as f32;
            let r = 1.0 - 0.85 * f;
            let a = a0 + f * 3.4;
            let w = 0.16 * r + 0.03;
            m.add_box(
                Vec3::new(w, 0.02, 0.2),
                Transform::from_translation(Vec3::new(a.cos() * r, 0.0, a.sin() * r)).with_rotation(Quat::from_rotation_y(-a + 0.9)),
                Color::WHITE,
            );
        }
    }
    m.add_cylinder(0.14, 0.02, 10, crate::meshkit::at(Vec3::ZERO), Color::srgb(0.4, 0.4, 0.4));
    m.build()
}

pub fn setup_affix_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut rings = Vec::new();
    let mut ring_mats = Vec::new();
    for a in Affix::ALL {
        let d = a.def();
        rings.push(meshes.add(ring_mesh(d.ring_segments, a == Affix::CursedTouched)));
        ring_mats.push(materials.add(StandardMaterial {
            base_color: d.aura,
            emissive: d.aura.to_linear() * AFFIX_RING_GLOW,
            unlit: true,
            ..default()
        }));
    }
    let warden = Affix::Warden.def().aura;
    let leaden = Affix::Leaden.def().aura;
    commands.insert_resource(AffixAssets {
        rings,
        ring_mats,
        shield_mesh: meshes.add(shield_mesh()),
        shield_mat: materials.add(StandardMaterial {
            base_color: warden.with_alpha(0.42),
            emissive: warden.to_linear() * 1.6,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
        well_mesh: meshes.add(well_mesh()),
        well_mat: materials.add(StandardMaterial {
            base_color: leaden.with_alpha(0.75),
            emissive: leaden.to_linear() * 0.9,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            depth_bias: 10.0,
            ..default()
        }),
    });
}

/// The body material of an elite whose primary affix is `a` (one per affix, every kind
/// shares it — `EnemyAssets::affix_mats`). Cursed-Touched is black lit gold from within.
pub fn body_material(a: Affix) -> StandardMaterial {
    let aura = a.def().aura;
    let (base, glow) = match a {
        Affix::CursedTouched => (Color::srgb(0.10, 0.08, 0.05), 0.55),
        // white would read as the hit-flash: keep Overclocked's body a cool grey that glows
        Affix::Overclocked => (Color::srgb(0.78, 0.82, 0.9), 0.35),
        _ => (aura, AFFIX_BODY_GLOW),
    };
    StandardMaterial { base_color: base, emissive: aura.to_linear() * glow, perceptual_roughness: 0.5, ..default() }
}

// ─── self-check ─────────────────────────────────────────────────────────────

/// Headless RULES: the pure pieces of the affixes, pinned.
pub fn self_check() -> Result<(), String> {
    crate::content::enemies::affix_self_check()?;
    use Affix::*;
    let set = AffixSet::EMPTY;
    // stat mods
    let (hp, speed, dmg) = stat_mods(set.with(Overclocked).with(Leaden).with(CursedTouched));
    if (speed - OVERCLOCK_SPEED * LEADEN_SPEED).abs() > 1e-4 || (hp - LEADEN_HP * CURSED_TOUCHED_HP).abs() > 1e-4 || (dmg - CURSED_TOUCHED_DMG).abs() > 1e-4 {
        return Err(format!("affix stat mods are off: hp {hp} speed {speed} dmg {dmg}"));
    }
    // Nightborne: nothing lands by day, all of it at night
    let up = Vec3::Y;
    let mut amount = 50.0;
    if guard_hit(set.with(Nightborne), up, false, None, Vec3::X, None, &mut amount) != Guard::Immune || amount != 0.0 {
        return Err("a Nightborne elite took a hit by day".into());
    }
    let mut amount = 50.0;
    if guard_hit(set.with(Nightborne), up, true, None, Vec3::X, None, &mut amount) != Guard::Open || amount != 50.0 {
        return Err("a Nightborne elite shrugged off a hit at night".into());
    }
    // Warden: the face soaks, the flank lands, a break passes the rest through
    let mut sh = WardenShield { hp: 30.0, max: 30.0, facing: Vec3::X, down: 0.0, since_hit: 9.0 };
    let mut amount = 20.0;
    // a shot flying -X (knock -X) comes from +X: into its face
    if guard_hit(set.with(Warden), up, true, Some(&mut sh), Vec3::NEG_X, None, &mut amount) != (Guard::Shield { soaked: 20.0, broke: false }) || amount != 0.0 {
        return Err(format!("a Warden's face did not soak a frontal hit ({amount} landed)"));
    }
    let mut amount = 20.0;
    if guard_hit(set.with(Warden), up, true, Some(&mut sh), Vec3::X, None, &mut amount) != Guard::Open || amount != 20.0 {
        return Err("a Warden blocked a hit from behind".into());
    }
    let mut amount = 25.0;
    let g = guard_hit(set.with(Warden), up, true, Some(&mut sh), Vec3::NEG_X, None, &mut amount);
    if g != (Guard::Shield { soaked: 10.0, broke: true }) || (amount - 15.0).abs() > 1e-4 || sh.down != WARDEN_DOWN_SECS {
        return Err(format!("a Warden's last 10 shield did not break and pass 15 through: {g:?}, {amount}"));
    }
    let mut amount = 25.0;
    if guard_hit(set.with(Warden), up, true, Some(&mut sh), Vec3::NEG_X, None, &mut amount) != Guard::Open {
        return Err("a broken Warden shield still blocked".into());
    }
    // Contagious: thresholds bud once each, a burst through them buds all it crossed
    let mut core = AffixCore { base_speed: 1.0, base_damage: 1.0, readout: 0.0, split_next: 0 };
    if core.splits(0.9) != 0 || core.splits(0.7) != 1 || core.splits(0.69) != 0 || core.splits(0.1) != 2 || core.splits(0.05) != 0 {
        return Err("Contagious budded off its thresholds".into());
    }
    // Magnetar: a shot passing beside it curves toward it; a seeker less so; out of reach none
    let pos = Vec3::new(0.0, 100.0, 0.0);
    let mag = [pos + Vec3::new(3.0, 0.0, 0.0)];
    let (mut h, mut hs) = (Vec3::Z, Vec3::Z);
    magnetar_bend(&mut h, pos, up, &mag, false, 0.1);
    magnetar_bend(&mut hs, pos, up, &mag, true, 0.1);
    if !(h.x > hs.x && hs.x > 0.0) {
        return Err(format!("a Magnetar did not bend shots toward it (plain {h:?}, seeker {hs:?})"));
    }
    let mut far = Vec3::Z;
    if magnetar_bend(&mut far, pos, up, &[pos + Vec3::X * (MAGNETAR_RADIUS + 1.0)], false, 0.1) {
        return Err("a Magnetar bent a shot out of its reach".into());
    }
    // the callout is ASCII and names the pair
    let c = callout(set.with(Warden).with(Magnetar));
    if c != "ELITE: WARDEN MAGNETAR - THE UNHITTABLE TURTLE" || !c.is_ascii() {
        return Err(format!("the callout read {c:?}"));
    }
    Ok(())
}
