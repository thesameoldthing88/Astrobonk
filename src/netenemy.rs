//! Streaming the horde to co-op clients.
//!
//! Astronauts replicate as entities (there are at most four). The crowd cannot: with
//! ENEMY_CAP at 1200 — and up to 3600 at a four-player party — per-entity replication is
//! not affordable. So enemies ride a custom, quantized, interest-managed batch message.
//!
//! THE SPHERE IS THE COMPRESSION. An enemy's position IS a unit direction, so instead of
//! three f32s we send its great-circle offset from the receiving client's own astronaut,
//! expressed in that astronaut's tangent frame, in arc metres. Two 16-bit numbers over a
//! ±128 m range give a 3.9 mm step — far finer than anything visible on a 1 m enemy — and
//! the record lands at 6 bytes instead of ~14.
//!
//! What is deliberately NOT on the wire, because the client can derive it:
//!   * FACING — enemies face the nearest astronaut, and the client already knows every
//!     astronaut (its own predicted one plus the replicated teammates), so it runs the
//!     same `nearest_astronaut` the host does.
//!   * WOBBLE — pure per-enemy jitter, never read by the simulation. Hashed from the id.
//!   * STRIDE — integrated from observed motion, exactly as the host integrates it.
//!   * hp / speed / damage / xp — either never displayed for crowd enemies, or a constant
//!     of the enemy kind.
//!
//! MEASURED, and it shapes the whole design: excluding pots (which piggyback on `Enemy`
//! with speed 0 and are static scenery), essentially the ENTIRE mobile horde sits within
//! 80 m of a player — they spawn at 42–58 m and steer inward. So distance culling saves
//! little for one player. The real wins are (a) each client only needs the horde near
//! ITSELF, which is a near-linear saving as players spread out, (b) pots never crossing
//! the wire at all, and (c) the far tier's round-robin. The 1200-record ceiling, not the
//! interest radius, is what actually bounds the worst case.

use crate::config::*;
use crate::content::enemies::EnemyKind;
use crate::enemies::{Enemy, EnemyAssets};
use crate::content::enemies::BossKind;
use crate::enemies::{AnubotBeam, Boss, CraterpillarHead, CraterpillarSegment, WORM_SEGMENTS};
use crate::enemies::{AimLine, Beamer, CrackDecal, EnemyProjectile, MortarShell, Telegraph};
use crate::net::{
    BossRec, BossSnapMsg, EnemySnapMsg, HazardEvent, HazardEventMsg, MyPlayerId, NetRole,
    PeerSlots, PickupEvent, PickupEventMsg,
};
use crate::pickups::{Pickup, PickupAssets, PickupKind};
use crate::planet::CurrentPlanet;
use crate::player::{Player, PlayerId};
use crate::sphere;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use std::collections::VecDeque;

/// Wire identity for one enemy. 15 bits of id + 1 flag bit, so ids must be recycled.
#[derive(Component, Clone, Copy, Debug)]
pub struct NetId(pub u16);

const ID_MASK: u16 = 0x7FFF;
const FLAG_BIT: u16 = 0x8000;

/// Host-side id allocator. Ids are recycled through a QUARANTINE rather than reused
/// immediately: a client keeps an unseen proxy alive for NET_ENEMY_GRACE seconds, so an id
/// handed straight back out would retarget that surviving proxy onto a different enemy.
#[derive(Resource)]
pub struct NetEnemyIds {
    next: u16,
    free: Vec<u16>,
    quarantine: VecDeque<(f32, u16)>,
}

impl Default for NetEnemyIds {
    fn default() -> Self {
        Self { next: 1, free: Vec::new(), quarantine: VecDeque::new() }
    }
}

impl NetEnemyIds {
    fn claim(&mut self) -> u16 {
        if let Some(id) = self.free.pop() {
            return id;
        }
        let id = self.next;
        self.next = self.next.wrapping_add(1) & ID_MASK;
        if self.next == 0 {
            self.next = 1;
        }
        id
    }
    fn release(&mut self, id: u16, now: f32) {
        self.quarantine.push_back((now + NET_ENEMY_GRACE + 0.5, id));
    }
    fn tick(&mut self, now: f32) {
        while let Some(&(due, id)) = self.quarantine.front() {
            if due > now {
                break;
            }
            self.quarantine.pop_front();
            self.free.push(id);
        }
    }
}

/// Which enemy ids each client currently believes in. The diff against this is what turns
/// into spawn / despawn records.
#[derive(Resource, Default)]
pub struct ClientResidency(HashMap<Entity, HashSet<u16>>);

impl ClientResidency {
    /// A client left: drop what we believed it had.
    pub fn forget(&mut self, client: Entity) {
        self.0.remove(&client);
    }
    /// The session ended.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// Snapshot cadence + sequence number.
#[derive(Resource, Default)]
pub struct SnapClock {
    acc: f32,
    seq: u16,
}

// ─── client side ────────────────────────────────────────────────────────────

/// A streamed enemy on a client: drawn, never simulated.
#[derive(Component)]
pub struct NetEnemy {
    /// Last position the host sent, in world direction form.
    pub target: Vec3,
    /// Smoothed position actually drawn.
    pub shown: Vec3,
    pub speed: f32,
    pub last_seen: f32,
}

#[derive(Resource, Default)]
pub struct NetEnemyIndex(HashMap<u16, Entity>);

/// A streamed BOSS on a client. Kept separate from NetEnemy because a boss proxy carries a
/// real `Boss` component, which is what makes `update_boss_bar` and `update_edge_markers`
/// work unchanged on the joiner.
#[derive(Component)]
pub struct NetBoss {
    pub target: Vec3,
    pub shown: Vec3,
    pub last_seen: f32,
}

#[derive(Resource, Default)]
pub struct NetBossIndex(HashMap<u16, Entity>);

/// Rolling receive counters, printed under --netlog.
#[derive(Resource, Default)]
pub struct NetEnemyStats {
    pub records: u32,
    pub chunks: u32,
    pub bytes: u32,
    pub seq_gaps: u32,
    last_seq: u16,
    next_print: f32,
}

pub struct EnemyStreamPlugin;

impl Plugin for EnemyStreamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetEnemyIds>()
            .init_resource::<ClientResidency>()
            .init_resource::<SnapClock>()
            .init_resource::<NetEnemyIndex>()
            .init_resource::<NetBossIndex>()
            .init_resource::<PickupIds>()
            .init_resource::<NetPickupIndex>()
            .init_resource::<NetEnemyStats>()
            // HOST streams, only while a run is live. Hosting starts from the MAIN MENU
            // (HOST CO-OP, then the hero pick), where there is no CurrentPlanet yet — and a
            // missing Res is a hard error, so ungated this crashed the moment you hosted.
            .add_systems(
                Update,
                (
                    assign_net_ids,
                    stream_pickups,
                    stream_hazards,
                    stream_bosses,
                    stream_enemies.after(assign_net_ids),
                )
                    .run_if(in_state(crate::AppState::InRun))
                    .run_if(crate::net::is_simulating)
                    .run_if(is_networked),
            )
            // ONE ordered chain. `client_stage_transition` MUST come first: it swaps
            // CurrentPlanet, and every decode below converts wire offsets to positions using
            // that planet's RADIUS. Decoding a Mars snapshot against Moon's 140 m would put
            // the whole horde at the wrong arc scale — floating or sunk.
            .add_systems(
                Update,
                (
                    client_stage_transition,
                    receive_bosses,
                    receive_pickups,
                    receive_hazards,
                    receive_enemies,
                    drive_boss_proxies,
                    drive_proxies,
                    drive_net_aim_lines,
                    animate_net_pickups,
                )
                    .chain()
                    .run_if(crate::net::is_client),
            )
            // Every proxy carries StageScoped, so `despawn_stage` eats them on OnExit(InRun)
            // — but the id->entity maps survive. Left uncleared, every id stays pointing at a
            // dead entity, `receive_enemies` sees `contains_key` and silently refuses to
            // respawn it, and the joiner lands on an empty planet while the host is buried in
            // a horde. The grace reaper cannot save us: it needs the entity to still exist.
            .add_systems(OnExit(crate::AppState::InRun), clear_stream_indices);
    }
}

/// Drop every id->entity mapping when the stage teardown despawns the proxies themselves.
fn clear_stream_indices(
    mut enemies: ResMut<NetEnemyIndex>,
    mut bosses: ResMut<NetBossIndex>,
    mut pickups: ResMut<NetPickupIndex>,
) {
    enemies.0.clear();
    bosses.0.clear();
    pickups.0.clear();
}

fn is_networked(role: Res<NetRole>) -> bool {
    role.is_networked()
}

/// Give every simulated enemy a wire id, and recycle the ids of the ones that died.
fn assign_net_ids(
    mut commands: Commands,
    mut ids: ResMut<NetEnemyIds>,
    time: Res<Time>,
    fresh: Query<Entity, (With<Enemy>, Without<NetId>)>,
    mut known: Local<HashMap<Entity, u16>>,
    all: Query<(Entity, &NetId)>,
) {
    let now = time.elapsed_secs();
    ids.tick(now);

    // Track ids from what ACTUALLY HAS a NetId, never from what we just queued.
    // `commands.insert` is deferred, so an entity assigned an id this run is not visible
    // in `all` until the next flush. Recording it at insert time and then reconciling
    // against `all` released the id one run later while the entity was still about to
    // receive it — the id went back on the free list and got handed to a second enemy.
    // Duplicate ids collapse the per-client residency set (it is keyed by id), which
    // presented as "most of the horde silently never streams".
    let live: HashMap<Entity, u16> = all.iter().map(|(e, n)| (e, n.0)).collect();
    for (e, id) in known.iter() {
        if !live.contains_key(e) {
            ids.release(*id, now); // it really died
        }
    }
    *known = live;

    // `try_insert`: an enemy spawned last frame can be swept by a stage change queued this
    // same frame (the teleporter despawns everything StageScoped), and a plain insert on it
    // panics the host the moment the despawn lands first. Its id is simply never recorded;
    // the wrapping counter hands the value out again in time.
    for e in &fresh {
        commands.entity(e).try_insert(NetId(ids.claim()));
    }
}

fn quantize(x: f32) -> u16 {
    (((x + NET_ENEMY_RANGE) / (2.0 * NET_ENEMY_RANGE)) * 65535.0).clamp(0.0, 65535.0) as u16
}
fn dequantize(q: u16) -> f32 {
    (q as f32 / 65535.0) * 2.0 * NET_ENEMY_RANGE - NET_ENEMY_RANGE
}

/// HOST: build and send one snapshot per client, at NET_ENEMY_HZ.
#[allow(clippy::too_many_arguments)]
fn stream_enemies(
    time: Res<Time>,
    mut clock: ResMut<SnapClock>,
    mut residency: ResMut<ClientResidency>,
    planet: Res<CurrentPlanet>,
    slots: Res<PeerSlots>,
    clients: Query<Entity, (With<ConnectedClient>, With<AuthorizedClient>)>,
    astronauts: Query<(&Player, &PlayerId)>,
    enemies: Query<(&Enemy, &NetId, Option<&crate::interact::Pot>, Option<&crate::enemies::Boss>)>,
    mut out: MessageWriter<ToClients<EnemySnapMsg>>,
) {
    clock.acc += time.delta_secs();
    let period = 1.0 / NET_ENEMY_HZ;
    if clock.acc < period {
        return;
    }
    clock.acc = 0.0;
    clock.seq = clock.seq.wrapping_add(1);
    let seq = clock.seq;

    let pi = planet_index(&planet);
    let interest_in = NET_ENEMY_INTEREST_IN[pi];
    let interest_out = NET_ENEMY_INTEREST_OUT[pi];

    for client in &clients {
        let Some(pid) = slots.player_id(client) else { continue };
        let Some(anchor) = astronauts
            .iter()
            .find(|(_, id)| id.0 == pid)
            .map(|(p, _)| p.dir)
        else {
            continue;
        };
        let (tan, bit) = sphere::tangent_frame(anchor);
        let resident = residency.0.entry(client).or_default();

        let mut spawns: Vec<u8> = Vec::new();
        let mut updates: Vec<u8> = Vec::new();
        let mut n_spawn = 0u16;
        let mut n_update = 0u16;
        let mut seen: HashSet<u16> = HashSet::new();
        let mut far: Vec<(u16, f32, f32)> = Vec::new();
        let mut records = 0usize;

        for (e, nid, pot, boss) in &enemies {
            // Pots are static scenery derived from the stage seed, and bosses ride their
            // own lane (they must never be culled — the HUD edge marker needs them).
            if pot.is_some() || boss.is_some() {
                continue;
            }
            let arc = sphere::arc_dist(e.dir, anchor, planet.radius);
            let was_resident = resident.contains(&nid.0);
            // Hysteresis: enter at IN, leave at OUT, so an enemy pacing the boundary
            // doesn't spawn and despawn every single snapshot.
            let inside = if was_resident { arc <= interest_out } else { arc <= interest_in };
            if !inside {
                continue;
            }
            seen.insert(nid.0);

            let cosang = e.dir.dot(anchor).clamp(-1.0, 1.0);
            let s = cosang.acos() * planet.radius;
            let tangent = (e.dir - anchor * cosang).normalize_or_zero();
            let (u, v) = (s * tangent.dot(tan), s * tangent.dot(bit));
            let (qu, qv) = (quantize(u), quantize(v));

            if !was_resident {
                // descriptor: carries everything needed to BUILD the proxy, position included
                let idw = nid.0 & ID_MASK | if e.elite { FLAG_BIT } else { 0 };
                spawns.extend_from_slice(&idw.to_le_bytes());
                spawns.push(kind_code(e.kind));
                spawns.push((((e.scale - 0.70) / 2.55).clamp(0.0, 1.0) * 255.0) as u8);
                spawns.extend_from_slice(&qu.to_le_bytes());
                spawns.extend_from_slice(&qv.to_le_bytes());
                n_spawn += 1;
                records += 1;
                continue;
            }

            if arc <= NET_ENEMY_NEAR_ARC {
                let idw = nid.0 & ID_MASK | if e.flash > 0.0 { FLAG_BIT } else { 0 };
                updates.extend_from_slice(&idw.to_le_bytes());
                updates.extend_from_slice(&qu.to_le_bytes());
                updates.extend_from_slice(&qv.to_le_bytes());
                n_update += 1;
                records += 1;
            } else {
                far.push((nid.0, u, v));
            }
        }

        // Far tier: round-robin a deterministic slice each snapshot. Staleness out here is
        // a fraction of a degree of angular error at 50 m+, and it keeps the near band —
        // the whole combat volume — at full rate no matter how big the horde gets.
        if !far.is_empty() {
            let budget = NET_ENEMY_MAX_RECORDS.saturating_sub(records);
            let stride = if budget == 0 { 8 } else { (far.len() / budget.max(1) + 1).clamp(4, 8) };
            for (id, u, v) in far.iter().copied() {
                if (id as usize + seq as usize) % stride != 0 {
                    continue;
                }
                if records >= NET_ENEMY_MAX_RECORDS {
                    break;
                }
                updates.extend_from_slice(&(id & ID_MASK).to_le_bytes());
                updates.extend_from_slice(&quantize(u).to_le_bytes());
                updates.extend_from_slice(&quantize(v).to_le_bytes());
                n_update += 1;
                records += 1;
            }
        }

        // Despawns: anything the client believes in that we no longer sent.
        let mut despawns: Vec<u8> = Vec::new();
        let mut n_despawn = 0u16;
        for id in resident.iter().copied().collect::<Vec<_>>() {
            if !seen.contains(&id) {
                despawns.extend_from_slice(&(id & ID_MASK).to_le_bytes());
                n_despawn += 1;
                resident.remove(&id);
            }
        }
        for id in seen {
            resident.insert(id);
        }

        // Chunk so no packet approaches renet's 1200-byte slice limit (a chunk over it
        // gets sliced by renet and becomes all-or-nothing on an unreliable channel).
        //
        // Counts are PER CHUNK and chunks split on RECORD boundaries. Putting the totals
        // only on chunk 0 would make every chunk after the first undecodable — invisible
        // at ~30 enemies (one chunk) and catastrophic at 200+.
        let mut chunks_out: Vec<(u16, u16, u16, Vec<u8>)> = Vec::new();
        let (mut si, mut ui, mut di) = (0usize, 0usize, 0usize);
        let (ns_tot, nu_tot, nd_tot) = (n_spawn as usize, n_update as usize, n_despawn as usize);
        while si < ns_tot || ui < nu_tot || di < nd_tot {
            let mut buf: Vec<u8> = Vec::new();
            let (mut cs, mut cu, mut cd) = (0u16, 0u16, 0u16);
            while si < ns_tot && buf.len() + 8 <= NET_ENEMY_CHUNK_BYTES {
                buf.extend_from_slice(&spawns[si * 8..si * 8 + 8]);
                si += 1;
                cs += 1;
            }
            while ui < nu_tot && buf.len() + 6 <= NET_ENEMY_CHUNK_BYTES {
                buf.extend_from_slice(&updates[ui * 6..ui * 6 + 6]);
                ui += 1;
                cu += 1;
            }
            while di < nd_tot && buf.len() + 2 <= NET_ENEMY_CHUNK_BYTES {
                buf.extend_from_slice(&despawns[di * 2..di * 2 + 2]);
                di += 1;
                cd += 1;
            }
            if buf.is_empty() {
                break; // nothing fits — should be impossible, but never spin
            }
            chunks_out.push((cs, cu, cd, buf));
        }
        if chunks_out.is_empty() {
            continue;
        }
        if seq % 30 == 0 {
            debug!(
                "SNAP seq={seq} spawn={n_spawn} update={n_update} despawn={n_despawn} far={} resident={} chunks={}",
                far.len(),
                resident.len(),
                chunks_out.len()
            );
        }
        let total_chunks = chunks_out.len() as u8;
        for (i, (cs, cu, cd, buf)) in chunks_out.into_iter().enumerate() {
            out.write(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: EnemySnapMsg {
                    seq,
                    chunk: i as u8,
                    chunks: total_chunks,
                    anchor: [anchor.x, anchor.y, anchor.z],
                    n_spawn: cs,
                    n_update: cu,
                    n_despawn: cd,
                    data: buf,
                },
            });
        }
    }
}

/// Marks a hazard visual a client built from a streamed event. These carry real
/// `EnemyProjectile` / `Telegraph` / `MortarShell` components so the existing integrators
/// animate them for free — but always with `damage: 0.0`, because the host resolves all
/// damage and a client must never invent a hit on itself.
#[derive(Component)]
pub struct NetHazard;

/// HOST: turn freshly spawned hazards into events.
///
/// Uses `Added<T>` rather than touching all nine spawn sites across enemies.rs — the
/// component values at spawn are exactly the event payload, so the spawn code stays
/// untouched and no future hazard can be added without this seeing it.
#[allow(clippy::too_many_arguments)]
fn stream_hazards(
    added_proj: Query<&EnemyProjectile, Added<EnemyProjectile>>,
    added_tel: Query<&Telegraph, Added<Telegraph>>,
    added_mortar: Query<&MortarShell, Added<MortarShell>>,
    added_lines: Query<(Entity, &AimLine), Added<AimLine>>,
    added_cracks: Query<&CrackDecal, (Added<CrackDecal>, Without<NetHazard>)>,
    added_spores: Query<&crate::gimmicks::SporeBurst, Added<crate::gimmicks::SporeBurst>>,
    beamers: Query<(&NetId, &Beamer)>,
    astronauts: Query<&PlayerId>,
    mut removed_lines: RemovedComponents<AimLine>,
    // line entity -> its beamer's NetId, because by the time a removal is seen the line
    // (and possibly the beamer) is gone and can no longer be asked
    mut live_lines: Local<HashMap<Entity, u16>>,
    // §7 item one-shots (a yo-yo throw, a singularity, an ignition, a death-save): the item
    // systems already say them as local messages, so they ride this lane as-is
    mut item_fx: MessageReader<crate::items::ItemFxMsg>,
    // §4 movement-tech one-shots (a Slam landing, a blink), the same way
    mut tech_fx: MessageReader<crate::techs::TechFxMsg>,
    // §6/§12 weapon one-shots (an evolution's fanfare, THE ANGELUS's wisps), the same way
    mut weapon_fx: MessageReader<crate::arsenal::WeaponFxMsg>,
    mut out: MessageWriter<ToClients<HazardEventMsg>>,
) {
    use crate::items::ItemFx;
    use crate::techs::TechFx;
    let mut events: Vec<HazardEvent> = Vec::new();
    for m in item_fx.read().filter(|m| !m.from_wire) {
        events.push(match m.fx {
            ItemFx::Orbit { owner, chunks, radius, dur } => HazardEvent::ItemOrbit { owner, chunks, radius, dur },
            ItemFx::Singularity { dir, radius, dur } => HazardEvent::Singularity { dir: dir.to_array(), radius, dur },
            ItemFx::Ignite { owner } => HazardEvent::TrailIgnite { owner },
            ItemFx::DeathSave { owner, save, dir } => HazardEvent::DeathSave { owner, kind: save.code(), dir: dir.to_array() },
        });
    }
    for m in tech_fx.read().filter(|m| !m.from_wire) {
        events.push(match m.fx {
            TechFx::Slam { owner, dir, power } => HazardEvent::Slam { owner, dir: dir.to_array(), power },
            TechFx::Blink { owner, from, to, axis, insured } => HazardEvent::Blink {
                owner,
                from: from.to_array(),
                to: to.to_array(),
                axis: axis.to_array(),
                insured,
            },
        });
    }
    for m in weapon_fx.read().filter(|m| !m.from_wire) {
        use crate::arsenal::WeaponFx;
        events.push(match m.fx {
            WeaponFx::Evolve { owner, weapon } => HazardEvent::Evolve { owner, weapon: weapon.code() },
            WeaponFx::Wisp { owner, dir, power } => HazardEvent::Wisp { owner, dir: dir.to_array(), power },
        });
    }
    // Beamer aim lines: the start carries WHO it is locked onto, not where — the line
    // tracks that astronaut and every client already knows where they all are.
    for (le, line) in &added_lines {
        let Ok((nid, b)) = beamers.get(line.owner) else { continue };
        let Some(target) = b.target.and_then(|t| astronauts.get(t).ok()) else { continue };
        live_lines.insert(le, nid.0);
        events.push(HazardEvent::AimLine { enemy: nid.0, target: target.0, charge: b.charging });
    }
    for le in removed_lines.read() {
        if let Some(enemy) = live_lines.remove(&le) {
            events.push(HazardEvent::AimLineEnd { enemy });
        }
    }
    for p in &added_proj {
        events.push(HazardEvent::Projectile {
            dir: [p.dir.x, p.dir.y, p.dir.z],
            heading: [p.heading.x, p.heading.y, p.heading.z],
            speed: p.speed,
            life: p.life,
            // the railbolt is the fast one, and it wears a different material
            style: if p.speed > 30.0 { 1 } else { 0 },
        });
    }
    for t in &added_tel {
        events.push(HazardEvent::Telegraph {
            dir: [t.dir.x, t.dir.y, t.dir.z],
            radius: t.radius,
            max: t.max,
            ring: t.ring,
        });
    }
    for m in &added_mortar {
        events.push(HazardEvent::Mortar {
            from: [m.from.x, m.from.y, m.from.z],
            to: [m.to.x, m.to.y, m.to.z],
            dur: m.dur,
        });
    }
    for c in &added_cracks {
        events.push(HazardEvent::Crack { dir: c.dir.to_array(), dur: c.timer });
    }
    // Dark Moon spore caps: the plant's index is enough, both machines lay the same flora
    for b in added_spores.iter().filter(|b| b.live) {
        events.push(HazardEvent::Spore { plant: b.plant });
    }
    if events.is_empty() {
        return;
    }
    out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: HazardEventMsg { events } });
}

/// A client's streamed Beamer that is painting an aim line: whose astronaut it tracks.
/// The proxy also wears a real `Beamer`, so the shared `aim_line_visuals` draws the line.
#[derive(Component)]
pub struct NetAimTarget(pub u8);

/// CLIENT: build the visual for each event and let the normal integrators animate it.
fn receive_hazards(
    mut commands: Commands,
    mut msgs: MessageReader<HazardEventMsg>,
    assets: Option<Res<EnemyAssets>>,
    planet: Option<Res<CurrentPlanet>>,
    index: Res<NetEnemyIndex>,
    lines: Query<(Entity, &AimLine)>,
    mut item_fx: MessageWriter<crate::items::ItemFxMsg>,
    mut tech_fx: MessageWriter<crate::techs::TechFxMsg>,
    mut weapon_fx: MessageWriter<crate::arsenal::WeaponFxMsg>,
    flora: Option<Res<crate::gimmicks::WorldFlora>>,
) {
    use crate::items::{DeathSave, ItemFx, ItemFxMsg};
    use crate::techs::{TechFx, TechFxMsg};
    let (Some(assets), Some(planet)) = (assets, planet) else { return };
    for m in msgs.read() {
        for ev in &m.events {
            // Item one-shots become the SAME local message the host's item systems write,
            // so `items::item_fx_presentation` draws them identically on both machines.
            let fx = match *ev {
                HazardEvent::ItemOrbit { owner, chunks, radius, dur } => Some(ItemFx::Orbit { owner, chunks, radius, dur }),
                HazardEvent::Singularity { dir, radius, dur } => Some(ItemFx::Singularity { dir: Vec3::from(dir), radius, dur }),
                HazardEvent::TrailIgnite { owner } => Some(ItemFx::Ignite { owner }),
                HazardEvent::DeathSave { owner, kind, dir } => {
                    Some(ItemFx::DeathSave { owner, save: DeathSave::from_code(kind), dir: Vec3::from(dir) })
                }
                _ => None,
            };
            if let Some(fx) = fx {
                item_fx.write(ItemFxMsg { fx, from_wire: true });
                continue;
            }
            // ...and so do the movement techs', for `techs::tech_fx_presentation`
            let fx = match *ev {
                HazardEvent::Slam { owner, dir, power } => Some(TechFx::Slam { owner, dir: Vec3::from(dir), power }),
                HazardEvent::Blink { owner, from, to, axis, insured } => Some(TechFx::Blink {
                    owner,
                    from: Vec3::from(from),
                    to: Vec3::from(to),
                    axis: Vec3::from(axis),
                    insured,
                }),
                _ => None,
            };
            if let Some(fx) = fx {
                tech_fx.write(TechFxMsg { fx, from_wire: true });
                continue;
            }
            // ...and the weapons', for `arsenal::weapon_fx_presentation`
            let fx = match *ev {
                HazardEvent::Evolve { owner, weapon } => crate::content::weapons::WeaponKind::from_code(weapon)
                    .map(|weapon| crate::arsenal::WeaponFx::Evolve { owner, weapon }),
                HazardEvent::Wisp { owner, dir, power } => {
                    Some(crate::arsenal::WeaponFx::Wisp { owner, dir: Vec3::from(dir), power })
                }
                _ => None,
            };
            if let Some(fx) = fx {
                weapon_fx.write(crate::arsenal::WeaponFxMsg { fx, from_wire: true });
                continue;
            }
            match *ev {
                HazardEvent::AimLine { enemy, target, charge } => {
                    // A beamer outside our interest set has no proxy — and is then far
                    // outside its own 26 m range of anything we can see anyway.
                    let Some(proxy) = index.0.get(&enemy).copied() else { continue };
                    let Ok(mut ec) = commands.get_entity(proxy) else { continue };
                    ec.insert((
                        // INERT cooldown, as with boss proxies: nothing on a client fires
                        Beamer { cd: f32::INFINITY, charging: charge, aim: Vec3::ZERO, target: None },
                        NetAimTarget(target),
                    ));
                    commands.spawn((
                        AimLine { owner: proxy, march: 0.0 },
                        NetHazard,
                        Mesh3d(assets.aim_mesh.clone()),
                        MeshMaterial3d(assets.ring_mat.clone()),
                        // zero-scaled: the proxy's aim is unknown until drive_net_aim_lines
                        // sweeps it, and a unit-sized line would sit at the pole meanwhile
                        Transform::from_translation(planet.surface_point(Vec3::Y)).with_scale(Vec3::ZERO),
                        crate::planet::StageScoped,
                    ));
                }
                HazardEvent::AimLineEnd { enemy } => {
                    let Some(proxy) = index.0.get(&enemy).copied() else { continue };
                    end_aim_line(&mut commands, proxy, &lines);
                }
                HazardEvent::Projectile { dir, heading, speed, life, style } => {
                    let dir = Vec3::from(dir);
                    let mat = if style == 1 { assets.ring_mat.clone() } else { assets.proj_mat.clone() };
                    let mut tf = Transform::from_translation(planet.surface_point(dir) + dir * 1.0);
                    if style == 1 {
                        tf = tf.with_scale(Vec3::new(0.5, 0.5, 2.2));
                    }
                    commands.spawn((
                        EnemyProjectile {
                            dir,
                            heading: Vec3::from(heading),
                            speed,
                            damage: 0.0, // visual only — the host owns damage
                            life,
                            hover: 1.0,
                        },
                        NetHazard,
                        Mesh3d(assets.proj_mesh.clone()),
                        MeshMaterial3d(mat),
                        tf,
                        crate::planet::StageScoped,
                    ));
                }
                HazardEvent::Telegraph { dir, radius, max, ring } => {
                    let dir = Vec3::from(dir);
                    commands.spawn((
                        crate::enemies::telegraph_bundle(
                            &assets,
                            &planet,
                            Telegraph { timer: max, max, radius, damage: 0.0, dir, ring },
                        ),
                        NetHazard,
                    ));
                }
                HazardEvent::Crack { dir, dur } => {
                    let crack = crate::enemies::spawn_crack_decal(&mut commands, &assets, &planet, Vec3::from(dir), dur);
                    commands.entity(crack).insert(NetHazard);
                }
                HazardEvent::Spore { plant } => {
                    // a cap on a world we are not standing on (a straggler across a stage
                    // change) has no plant to swell
                    let Some(pl) = flora
                        .as_ref()
                        .filter(|f| f.style == Some(crate::content::planets::FloraStyle::GlowShrooms))
                        .and_then(|f| f.plants.get(plant as usize).copied())
                    else {
                        continue;
                    };
                    commands.spawn((
                        crate::gimmicks::spore_bundle(&planet, crate::gimmicks::SporeBurst::new(plant, pl.dir, false)),
                        NetHazard,
                    ));
                }
                // handled above, as ItemFxMsg / TechFxMsg
                HazardEvent::ItemOrbit { .. }
                | HazardEvent::Singularity { .. }
                | HazardEvent::TrailIgnite { .. }
                | HazardEvent::DeathSave { .. }
                | HazardEvent::Slam { .. }
                | HazardEvent::Blink { .. }
                | HazardEvent::Evolve { .. }
                | HazardEvent::Wisp { .. } => {}
                HazardEvent::Mortar { from, to, dur } => {
                    let from = Vec3::from(from);
                    commands.spawn((
                        MortarShell { from, to: Vec3::from(to), t: 0.0, dur },
                        NetHazard,
                        Mesh3d(assets.proj_mesh.clone()),
                        MeshMaterial3d(assets.proj_mat.clone()),
                        Transform::from_translation(planet.surface_point(from))
                            .with_scale(Vec3::splat(1.6)),
                        crate::planet::StageScoped,
                    ));
                }
            }
        }
    }
}

/// Retire a client-side aim line: the line entity goes, and the proxy stops being a
/// charging Beamer so it can take the next AimLine cleanly.
fn end_aim_line(commands: &mut Commands, proxy: Entity, lines: &Query<(Entity, &AimLine)>) {
    for (le, line) in lines.iter() {
        if line.owner == proxy {
            commands.entity(le).try_despawn();
        }
    }
    if let Ok(mut ec) = commands.get_entity(proxy) {
        ec.remove::<(Beamer, NetAimTarget)>();
    }
}

/// CLIENT: sweep each streamed aim line exactly as the host's Beamer does — track the
/// locked astronaut until the final BEAMER_LOCK_SECS, then hold — and retire it when the
/// charge runs out (the host fires at that moment; the railbolt arrives on its own event).
pub fn drive_net_aim_lines(
    mut commands: Commands,
    time: Res<Time>,
    mine: Res<MyPlayerId>,
    q_local: Query<&Transform, (With<crate::player::LocalPlayer>, Without<Enemy>)>,
    q_mates: Query<(&PlayerId, &Transform), (With<crate::remote::RemoteAstronaut>, Without<Enemy>)>,
    mut q: Query<(Entity, &Enemy, &mut Beamer, &NetAimTarget, &Transform)>,
    lines: Query<(Entity, &AimLine)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (proxy, e, mut b, target, tf) in &mut q {
        b.charging -= dt;
        if b.charging <= 0.0 {
            end_aim_line(&mut commands, proxy, &lines);
            continue;
        }
        if b.charging <= BEAMER_LOCK_SECS && b.aim != Vec3::ZERO {
            continue; // locked: the line holds while the shot comes
        }
        // Where the mark is drawn on THIS screen: our own predicted body, or a teammate.
        let pos = if mine.0 == Some(target.0) {
            q_local.single().ok().map(|t| t.translation)
        } else {
            q_mates.iter().find(|(pid, _)| pid.0 == target.0).map(|(_, t)| t.translation)
        };
        let Some(pos) = pos else { continue };
        let v = pos - tf.translation;
        let vt = (v - e.dir * v.dot(e.dir)).normalize_or_zero();
        if vt != Vec3::ZERO {
            b.aim = vt;
        }
    }
}

/// Wire identity for a pickup, mirroring NetId for enemies.
#[derive(Component, Clone, Copy)]
pub struct PickupNetId(pub u16);

#[derive(Resource, Default)]
pub struct PickupIds {
    next: u16,
}

#[derive(Resource, Default)]
pub struct NetPickupIndex(HashMap<u16, Entity>);

/// HOST: announce every pickup spawn and despawn.
///
/// Only the two endpoints cross the wire: an idle pickup's transform is a pure function of
/// (dir, time, bob), and the fly-to-collector phase is derived locally, so a client can draw
/// the entire life of a gem from these two events. `gem_merge` needs no special case — it
/// reads as N despawns plus one spawn, which is exactly what it is.
fn stream_pickups(
    mut commands: Commands,
    mut ids: ResMut<PickupIds>,
    fresh: Query<(Entity, &Pickup), Without<PickupNetId>>,
    mut known: Local<HashMap<Entity, u16>>,
    live: Query<(Entity, &PickupNetId)>,
    mut withdrawn: RemovedComponents<crate::pickups::HorizonBound>,
    settled: Query<(&PickupNetId, &Pickup), Without<crate::pickups::HorizonBound>>,
    called: Query<(&PickupNetId, &crate::pickups::HorizonBound), Changed<crate::pickups::HorizonBound>>,
    mut out: MessageWriter<ToClients<PickupEventMsg>>,
) {
    let mut events: Vec<PickupEvent> = Vec::new();
    // Tome of the Horizon: the one flight a client cannot work out for itself. A withdrawn
    // call first (a gem collected or merged is despawned, fails the lookup and goes out as
    // a Despawn below), then new calls — Changed, so a gem re-called by someone else after
    // it settled is announced again.
    for e in withdrawn.read() {
        if let Ok((id, p)) = settled.get(e) {
            events.push(PickupEvent::HorizonSettle { id: id.0, dir: p.dir.to_array() });
        }
    }
    for (id, hb) in &called {
        events.push(PickupEvent::Horizon { id: id.0, owner: hb.0 });
    }

    for (e, p) in &fresh {
        ids.next = ids.next.wrapping_add(1).max(1);
        let id = ids.next;
        // try_insert: the same stage-change race as `assign_net_ids`
        commands.entity(e).try_insert(PickupNetId(id));
        let (kind, value) = match p.kind {
            PickupKind::Xp(v) => (0u8, v.to_bits()),
            PickupKind::Gold(g) => (1, g as u32),
            PickupKind::Silver(v) => (2, v as u32),
            PickupKind::Food => (3, 0),
            PickupKind::Powerup(k) => (
                4,
                match k {
                    crate::run::PowerupKind::Damage2x => 1,
                    crate::run::PowerupKind::Magnet => 2,
                    crate::run::PowerupKind::Speed => 3,
                },
            ),
        };
        events.push(PickupEvent::Spawn {
            id,
            kind,
            value,
            dir: [p.dir.x, p.dir.y, p.dir.z],
            bob: p.bob,
        });
    }

    // Same deferred-command trap as enemy ids: track only what ACTUALLY carries the
    // component, never what was just queued, or a despawn is announced for a pickup that
    // was merely still waiting for its insert to flush.
    let now: HashMap<Entity, u16> = live.iter().map(|(e, n)| (e, n.0)).collect();
    for (e, id) in known.iter() {
        if !now.contains_key(e) {
            events.push(PickupEvent::Despawn { id: *id });
        }
    }
    *known = now;

    if !events.is_empty() {
        out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: PickupEventMsg { events } });
    }
}

/// CLIENT: draw the loot. Visual only — collection and XP are the host's, and arrive as
/// grants, so these carry no gameplay value the client could double-count.
fn receive_pickups(
    mut commands: Commands,
    mut msgs: MessageReader<PickupEventMsg>,
    mut index: ResMut<NetPickupIndex>,
    mut q_pickups: Query<&mut Pickup>,
    assets: Option<Res<PickupAssets>>,
    planet: Option<Res<CurrentPlanet>>,
) {
    let (Some(assets), Some(planet)) = (assets, planet) else { return };
    for m in msgs.read() {
        for ev in &m.events {
            match *ev {
                PickupEvent::Spawn { id, kind, value, dir, bob } => {
                    if index.0.contains_key(&id) {
                        continue;
                    }
                    let dir = Vec3::from(dir);
                    let k = match kind {
                        0 => PickupKind::Xp(f32::from_bits(value)),
                        1 => PickupKind::Gold(value as u64),
                        2 => PickupKind::Silver(value as u64),
                        3 => PickupKind::Food,
                        _ => PickupKind::Powerup(match value {
                            1 => crate::run::PowerupKind::Damage2x,
                            2 => crate::run::PowerupKind::Magnet,
                            _ => crate::run::PowerupKind::Speed,
                        }),
                    };
                    let (mesh, mat, scale) = match k {
                        PickupKind::Xp(_) => (assets.gem_mesh.clone(), assets.gem_mat.clone(), 1.0),
                        PickupKind::Gold(_) => (assets.coin_mesh.clone(), assets.coin_mat.clone(), 1.0),
                        PickupKind::Silver(_) => (assets.coin_mesh.clone(), assets.silver_mat.clone(), 1.0),
                        PickupKind::Food => (assets.food_mesh.clone(), assets.food_mat.clone(), 1.0),
                        PickupKind::Powerup(_) => (assets.power_mesh.clone(), assets.power_mat.clone(), 1.0),
                    };
                    let e = commands
                        .spawn((
                            Pickup::new(k, dir, bob),
                            PickupNetId(id),
                            Mesh3d(mesh),
                            MeshMaterial3d(mat),
                            Transform::from_translation(planet.surface_point(dir) + dir * 0.35)
                                .with_scale(Vec3::splat(scale)),
                            crate::planet::StageScoped,
                        ))
                        .id();
                    index.0.insert(id, e);
                }
                PickupEvent::Despawn { id } => {
                    if let Some(e) = index.0.remove(&id) {
                        // tolerate an entity despawn_stage already reaped
                        if let Ok(mut ec) = commands.get_entity(e) {
                            ec.despawn();
                        }
                    }
                }
                PickupEvent::Horizon { id, owner } => {
                    // `animate_net_pickups` flies it home to that astronaut from here on
                    let Some(&e) = index.0.get(&id) else { continue };
                    if let Ok(mut p) = q_pickups.get_mut(e) {
                        p.call_home();
                        commands.entity(e).try_insert(crate::pickups::HorizonBound(owner));
                    }
                }
                PickupEvent::HorizonSettle { id, dir } => {
                    let Some(&e) = index.0.get(&id) else { continue };
                    if let Ok(mut p) = q_pickups.get_mut(e) {
                        p.settle(Vec3::from(dir));
                        commands.entity(e).remove::<crate::pickups::HorizonBound>();
                    }
                }
            }
        }
    }
}

/// CLIENT: bob and spin streamed loot, and fly it toward whoever is close.
///
/// Purely cosmetic — collection and the XP grant belong to the host, and `pickup_update` is
/// gated off here. Without this a joiner's gems would hang motionless in the air and then
/// blink out when the host collected them.
#[allow(clippy::type_complexity)]
fn animate_net_pickups(
    mut commands: Commands,
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    q_players: Query<(&Player, &crate::run::PlayerState, &Transform), Without<Pickup>>,
    bodies: Query<
        (&PlayerId, &Transform, Option<&crate::run::PlayerState>, Option<&crate::net::PlayerVitals>),
        (Or<(With<Player>, With<crate::remote::RemoteAstronaut>)>, Without<Pickup>),
    >,
    mut q: Query<(Entity, &mut Pickup, &mut Transform, Option<&crate::pickups::HorizonBound>), Without<Player>>,
) {
    let Some(planet) = planet else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t_now = time.elapsed_secs();
    let attractors: Vec<(Vec3, f32)> = q_players
        .iter()
        .filter(|(_, ps, _)| !ps.dead)
        .map(|(_, ps, tf)| (tf.translation, ps.pickup_range()))
        .collect();

    for (e, mut p, mut tf, called) in &mut q {
        // Called home over the horizon (the host announced it): the host's own flight
        // (`Pickup::fly_home`, then the magnet) to whichever astronaut called it — ours or a
        // teammate's. The host's despawn ends it, or its HorizonSettle withdraws it.
        let caller = called.and_then(|hb| bodies.iter().find(|(id, ..)| id.0 == hb.0));
        if let Some((_, body, ps, vitals)) = caller {
            if ps.is_some_and(|ps| ps.dead) || vitals.is_some_and(|v| v.down) {
                // The host drops a call whose caller went down; stop here rather than fly
                // to a body the gem will never reach (its HorizonSettle puts it exactly).
                let dir = p.dir;
                p.settle(dir);
                commands.entity(e).remove::<crate::pickups::HorizonBound>();
                continue;
            }
            let body = body.translation;
            tf.translation = if p.arc {
                p.fly_home(body.normalize_or_zero(), dt, &planet)
            } else {
                p.magnet_step(tf.translation, body, dt)
            };
            continue;
        }
        let near = attractors
            .iter()
            .filter(|(pos, range)| tf.translation.distance(*pos) < *range)
            .min_by(|a, b| {
                tf.translation
                    .distance(a.0)
                    .total_cmp(&tf.translation.distance(b.0))
            })
            .map(|(pos, _)| *pos);
        if let Some(pos) = near {
            p.flying = true;
            tf.translation = p.magnet_step(tf.translation, pos, dt);
        } else {
            let up = p.dir;
            tf.translation =
                planet.surface_point(up) + up * (0.35 + ((t_now * 2.0 + p.bob).sin() * 0.08));
            tf.rotation = Quat::from_axis_angle(up, t_now * 1.5 + p.bob);
        }
    }
}

/// CLIENT: rebuild the world when the host moves to the next planet.
///
/// `director::stage_transition` is unreachable on a client — its only trigger is the
/// teleporter interaction, which is host-only — so without this the joiner keeps the entire
/// previous planet (terrain, rocks, chests, shrines) while its HUD describes the new one,
/// and every streamed enemy decodes against the wrong planet radius. The same rebuild
/// covers a world built from any seed other than the host's (see `RunSync::built_for`).
///
/// This deliberately mirrors only the TEARDOWN+REBUILD half of stage_transition. The victory
/// branch and the `save.counters.cleared` writes stay host-only: a client must never bank
/// progress for a stage it did not simulate.
#[allow(clippy::too_many_arguments)]
fn client_stage_transition(
    mut commands: Commands,
    mut sync: ResMut<crate::net::RunSync>,
    run: Res<crate::run::RunState>,
    save: Res<crate::save::MetaSave>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut game_rng: ResMut<crate::run::GameRng>,
    mut phase: ResMut<crate::run::RunPhase>,
    mut indices: (ResMut<NetEnemyIndex>, ResMut<NetBossIndex>, ResMut<NetPickupIndex>),
    scoped: Query<Entity, With<crate::planet::StageScoped>>,
    mine: Query<&crate::run::PlayerState, With<crate::player::LocalPlayer>>,
    my_id: Res<MyPlayerId>,
    mut banners: MessageWriter<crate::messages::BannerMsg>,
) {
    let Some(stage) = sync.pending_stage.take() else { return };

    // Carry our build across, exactly as the host carries every player's — but only into
    // the next stage of the SAME run. A world from another seed means another run, and a
    // sheet from it (levels, gold, items) must not come along.
    let same_run = sync.built_for.map(|(seed, _)| seed) == Some(run.run_seed);
    let carried = if same_run { mine.single().ok().cloned() } else { None };
    sync.built_for = Some((run.run_seed, stage));

    // Tear the old stage down. This eats our astronaut and every streamed proxy too —
    // they are all StageScoped — so the id maps must be cleared or `receive_*` would
    // refuse to respawn those ids for the rest of the run.
    for e in &scoped {
        commands.entity(e).despawn();
    }
    indices.0 .0.clear();
    indices.1 .0.clear();
    indices.2 .0.clear();

    let stage_seed = run.run_seed.wrapping_add(stage as u64);
    game_rng.reseed(stage_seed);
    let planet = CurrentPlanet::from_kind(run.planet());
    let (props, rails) = crate::planet::spawn_stage(&mut commands, &mut meshes, &mut materials, &planet, stage_seed);
    crate::player::spawn_player(
        &mut commands,
        &mut meshes,
        &mut materials,
        &planet,
        &run,
        &save,
        // our own slot's drop point — where the host re-seats our server-side body
        my_id.0.unwrap_or(0),
        carried.as_ref().map(|p| p.character).unwrap_or(run.character),
        true,
        carried,
    );
    crate::interact::spawn_interactables(
        &mut commands,
        &mut meshes,
        &mut materials,
        &planet,
        &run,
        &crate::run::PlayerState::new(run.character, &save),
        &save,
        &rails,
        &props,
        Vec3::Y,
    );
    commands.insert_resource(props);
    commands.insert_resource(rails);
    commands.insert_resource(planet);
    *phase = crate::run::RunPhase::Playing;
    banners.write(crate::messages::BannerMsg(format!(
        "STAGE {} — {}",
        stage + 1,
        run.planet().def().name
    )));
    info!("NET client rebuilt world for stage {stage}");
    crate::playlog::line(format!("NET client rebuilt world for stage {stage}"));
}

fn boss_code(k: BossKind) -> u8 {
    match k {
        BossKind::CraterpillarJr => 0,
        BossKind::RoverGoneWrong => 1,
        BossKind::Craterpillar => 2,
        BossKind::Anubot => 3,
    }
}
fn boss_from_code(c: u8) -> BossKind {
    match c {
        1 => BossKind::RoverGoneWrong,
        2 => BossKind::Craterpillar,
        3 => BossKind::Anubot,
        _ => BossKind::CraterpillarJr,
    }
}

/// HOST: broadcast every boss to every client, never interest-culled — the HUD edge marker
/// must point at a boss from the far side of the planet. CLIENTS_ONLY so the host does not
/// receive its own snapshot back on top of the authoritative fight.
fn stream_bosses(
    time: Res<Time>,
    mut acc: Local<f32>,
    q: Query<(&Enemy, &NetId, &Boss, Option<&AnubotBeam>)>,
    mut out: MessageWriter<ToClients<BossSnapMsg>>,
) {
    *acc += time.delta_secs();
    if *acc < 1.0 / 20.0 {
        return;
    }
    *acc = 0.0;
    let bosses: Vec<BossRec> = q
        .iter()
        .map(|(e, nid, b, beam)| BossRec {
            id: nid.0,
            kind: boss_code(b.kind),
            dir: [e.dir.x, e.dir.y, e.dir.z],
            hp_frac: if e.max_hp > 0.0 { (e.hp / e.max_hp).clamp(0.0, 1.0) } else { 0.0 },
            phase: b.phase,
            beam_angle: beam.map(|x| x.angle).unwrap_or(0.0),
            beam_state: beam.map(|x| x.state).unwrap_or(0),
        })
        .collect();
    // Sent even when empty: an empty list is how a client learns the boss died.
    out.write(ToClients { targets: SendTargets::CLIENTS_ONLY, message: BossSnapMsg { bosses } });
}

/// CLIENT: reconcile boss proxies. The proxy carries `Enemy` with max_hp = 1.0 and
/// hp = the streamed fraction, which is exactly what the boss bar divides.
fn receive_bosses(
    mut commands: Commands,
    mut msgs: MessageReader<BossSnapMsg>,
    mut index: ResMut<NetBossIndex>,
    mut meshes: ResMut<Assets<Mesh>>,
    assets: Option<Res<EnemyAssets>>,
    planet: Option<Res<CurrentPlanet>>,
    time: Res<Time>,
    mut q: Query<(&mut Enemy, &mut Boss, &mut NetBoss, Option<&mut AnubotBeam>)>,
) {
    let (Some(assets), Some(planet)) = (assets, planet) else { return };
    let now = time.elapsed_secs();

    for m in msgs.read() {
        let mut seen: HashSet<u16> = HashSet::new();
        for r in &m.bosses {
            seen.insert(r.id);
            let dir = Vec3::from(r.dir);
            let kind = boss_from_code(r.kind);
            if let Some(ent) = index.0.get(&r.id).copied() {
                if let Ok((mut e, mut b, mut nb, beam)) = q.get_mut(ent) {
                    nb.target = dir;
                    nb.last_seen = now;
                    e.hp = r.hp_frac;
                    b.phase = r.phase;
                    if let Some(mut beam) = beam {
                        // A new state restarts the local timer the charge-up pose reads;
                        // the angle is re-anchored every snapshot and spun in between by
                        // drive_boss_proxies with the host's own sweep speeds.
                        if beam.state != r.beam_state {
                            beam.state = r.beam_state;
                            beam.timer = AnubotBeam::state_secs(r.beam_state, r.phase);
                        }
                        beam.angle = r.beam_angle;
                    }
                }
                continue;
            }
            // Mesh comes from BossKind, NOT Enemy.kind: spawn_boss hardcodes Enemy.kind to
            // Bruiser and selects the real look from BossKind, so trusting the crowd path
            // here would draw THE CRATERPILLAR as a grunt.
            let def = kind.def();
            let (mesh, mat) = match kind {
                BossKind::Craterpillar | BossKind::CraterpillarJr => {
                    (assets.worm_head_mesh.clone(), assets.worm_mat.clone())
                }
                BossKind::Anubot => (assets.anubot_mesh.clone(), assets.anubot_mat.clone()),
                _ => (meshes.add(crate::enemies::boss_mesh()), assets.boss_mat.clone()),
            };
            let pos = planet.surface_point(dir) + dir * def.scale * 0.8;
            let ent = commands
                .spawn((
                    Enemy {
                        kind: crate::content::enemies::EnemyKind::Bruiser,
                        dir,
                        hover: 0.0,
                        speed: def.speed,
                        damage: 0.0,
                        xp: 0.0,
                        hp: r.hp_frac,
                        max_hp: 1.0,
                        elite: true,
                        contact_cd: 0.0,
                        slow: 0.0,
                        knock: Vec3::ZERO,
                        flash: 0.0,
                        scale: def.scale,
                        wobble: 0.0,
                        stride: 0.0,
                    },
                    // INERT timers: 0.0 would make `boss_attacks` fire every frame on the client.
                    // The host owns attack scheduling; the proxy only wears the marker.
                    Boss { kind, attack_timer: f32::INFINITY, burst_timer: f32::INFINITY, phase: r.phase },
                    NetBoss { target: dir, shown: dir, last_seen: now },
                    NetId(r.id),
                    Mesh3d(mesh),
                    MeshMaterial3d(mat),
                    Transform::from_translation(pos).with_scale(Vec3::splat(def.scale)),
                    crate::planet::StageScoped,
                ))
                .id();
            // THE WORM'S BODY COSTS NOTHING ON THE WIRE. Segments carry no Enemy, no Boss
            // and are not in the spatial hash - they are pure decoration placed from the
            // head's trail. So the client grows its own body: attach a CraterpillarHead to
            // the proxy, spawn the 12 segments, and the existing `craterpillar_update`
            // (which needs only those two components) animates them exactly as on the host.
            if kind == BossKind::Craterpillar {
                commands
                    .entity(ent)
                    .insert(CraterpillarHead { trail: std::collections::VecDeque::new() });
                for i in 0..WORM_SEGMENTS {
                    let seg_scale = def.scale * (0.85 - 0.03 * i as f32).max(0.4);
                    commands.spawn((
                        CraterpillarSegment {
                            head: ent,
                            idx: i,
                            damage: 0.0, // visual only - the host resolves contact damage
                            scale: seg_scale,
                        },
                        Mesh3d(assets.worm_seg_mesh.clone()),
                        MeshMaterial3d(assets.worm_mat.clone()),
                        Transform::from_translation(pos).with_scale(Vec3::splat(seg_scale)),
                        crate::planet::StageScoped,
                    ));
                }
            }
            // THE VERDICT BEAM rides the boss record (angle + state), not the hazard lane:
            // it is continuous, and a proxy wearing a real AnubotBeam gets the shared
            // anubot_beam_visuals — pose tell and slab — for free.
            if kind == BossKind::Anubot {
                commands.entity(ent).insert(AnubotBeam {
                    angle: r.beam_angle,
                    state: r.beam_state,
                    timer: AnubotBeam::state_secs(r.beam_state, r.phase),
                });
                crate::enemies::spawn_anubot_beam_vis(&mut commands, &assets, ent, pos);
            }
            index.0.insert(r.id, ent);
            info!("NET boss proxy spawned: {:?} (id {})", kind, r.id);
        }
        let gone: Vec<u16> = index.0.keys().copied().filter(|k| !seen.contains(k)).collect();
        for id in gone {
            if let Some(ent) = index.0.remove(&id) {
                if let Ok(mut ec) = commands.get_entity(ent) {
                    ec.despawn();
                }
            }
        }
    }
}

/// CLIENT: ease boss proxies toward their streamed position, and keep Anubot's beam
/// sweeping between snapshots.
pub fn drive_boss_proxies(
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    mut q: Query<(&mut Enemy, &mut NetBoss, &mut Transform, &Boss, Option<&mut AnubotBeam>)>,
) {
    let Some(planet) = planet else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let k = 1.0 - (-NET_ENEMY_SMOOTH_RATE * dt).exp();
    for (mut e, mut nb, mut tf, b, beam) in &mut q {
        if let Some(mut beam) = beam {
            beam.angle = (beam.angle + AnubotBeam::spin(beam.state, b.phase) * dt) % std::f32::consts::TAU;
            beam.timer = (beam.timer - dt).max(0.0);
        }
        let ang = nb.shown.angle_between(nb.target);
        nb.shown = if ang * planet.radius > NET_ENEMY_SNAP_ARC {
            nb.target
        } else {
            sphere::step_toward(nb.shown, nb.target, ang * k)
        };
        e.dir = nb.shown;
        let up = nb.shown;
        let def = b.kind.def();
        tf.translation = planet.surface_point(up) + up * def.scale * 0.8;
        tf.rotation = sphere::frame_quat(up, sphere::tangent_frame(up).0);
        // Re-set every frame, as enemy_move does on the host: the beam's charge-up pose
        // multiplies onto scale, and without a reset it would compound until the proxy
        // towered over the planet.
        tf.scale = Vec3::splat(def.scale);
    }
}

fn planet_index(planet: &CurrentPlanet) -> usize {
    use crate::content::planets::PlanetKind;
    match planet.kind {
        PlanetKind::Moon => 0,
        PlanetKind::Mars => 1,
        _ => 2,
    }
}

/// Explicit discriminants rather than a derived index: this is a WIRE format, so the
/// mapping must not silently shift if the enum is ever reordered.
fn kind_code(k: EnemyKind) -> u8 {
    match k {
        EnemyKind::Shambler => 0,
        EnemyKind::Sprinter => 1,
        EnemyKind::Bruiser => 2,
        EnemyKind::Spitter => 3,
        EnemyKind::Ufo => 4,
        EnemyKind::Burrower => 5,
        EnemyKind::Beamer => 6,
        EnemyKind::Lobber => 7,
        EnemyKind::Ghost => 8,
    }
}
fn kind_from_code(c: u8) -> EnemyKind {
    match c {
        1 => EnemyKind::Sprinter,
        2 => EnemyKind::Bruiser,
        3 => EnemyKind::Spitter,
        4 => EnemyKind::Ufo,
        5 => EnemyKind::Burrower,
        6 => EnemyKind::Beamer,
        7 => EnemyKind::Lobber,
        8 => EnemyKind::Ghost,
        _ => EnemyKind::Shambler,
    }
}

/// CLIENT: apply snapshots — spawn proxies for descriptors, retarget for updates, drop for
/// despawns. Proxies get an `Enemy` component so they can reuse the real animator, but they
/// are never simulated: no system that damages, steers or scores them runs on a client.
#[allow(clippy::too_many_arguments)]
fn receive_enemies(
    mut commands: Commands,
    mut msgs: MessageReader<EnemySnapMsg>,
    mut index: ResMut<NetEnemyIndex>,
    mut stats: ResMut<NetEnemyStats>,
    assets: Option<Res<EnemyAssets>>,
    planet: Option<Res<CurrentPlanet>>,
    time: Res<Time>,
    mut q: Query<(&mut Enemy, &mut NetEnemy)>,
) {
    let (Some(assets), Some(planet)) = (assets, planet) else { return };
    let now = time.elapsed_secs();

    for m in msgs.read() {
        stats.chunks += 1;
        stats.bytes += m.data.len() as u32 + 29;
        if m.chunk == 0 {
            let gap = m.seq.wrapping_sub(stats.last_seq);
            if stats.last_seq != 0 && gap > 1 {
                stats.seq_gaps += 1;
            }
            stats.last_seq = m.seq;
        }

        let anchor = Vec3::from(m.anchor);
        let (tan, bit) = sphere::tangent_frame(anchor);
        let decode = |qu: u16, qv: u16| -> Vec3 {
            let (u, v) = (dequantize(qu), dequantize(qv));
            let s = (u * u + v * v).sqrt();
            if s < 1e-4 {
                return anchor;
            }
            let heading = (tan * (u / s) + bit * (v / s)).normalize_or_zero();
            // offset_dir is the exact inverse of the encode, so decoding reuses the same
            // tested great-circle math rather than a second implementation of it.
            sphere::offset_dir(anchor, heading, s, planet.radius)
        };

        let mut off = 0usize;
        let d = &m.data;

        for _ in 0..m.n_spawn {
            if off + 8 > d.len() {
                break;
            }
            let idw = u16::from_le_bytes([d[off], d[off + 1]]);
            let id = idw & ID_MASK;
            let elite = idw & FLAG_BIT != 0;
            let kind = kind_from_code(d[off + 2]);
            let scale = 0.70 + (d[off + 3] as f32 / 255.0) * 2.55;
            let dir = decode(
                u16::from_le_bytes([d[off + 4], d[off + 5]]),
                u16::from_le_bytes([d[off + 6], d[off + 7]]),
            );
            off += 8;
            stats.records += 1;

            if index.0.contains_key(&id) {
                continue;
            }
            let def = kind.def();
            let mat = if elite { assets.elite_mat.clone() } else { assets.mats[&kind].clone() };
            let e = commands
                .spawn((
                    Enemy {
                        kind,
                        dir,
                        hover: def.hover,
                        speed: def.speed,
                        damage: 0.0, // proxies never damage anyone — the host owns combat
                        xp: 0.0,
                        hp: 1.0,
                        max_hp: 1.0,
                        elite,
                        contact_cd: 0.0,
                        slow: 0.0,
                        knock: Vec3::ZERO,
                        flash: 0.0,
                        scale,
                        // hashed from the id: pure jitter, never worth a wire byte
                        wobble: (id as f32 * 0.618_034 * std::f32::consts::TAU) % std::f32::consts::TAU,
                        stride: 0.0,
                    },
                    NetEnemy { target: dir, shown: dir, speed: 0.0, last_seen: now },
                    NetId(id),
                    Mesh3d(assets.meshes[&kind].clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(planet.surface_point(dir)).with_scale(Vec3::splat(scale)),
                    crate::planet::StageScoped,
                ))
                .id();
            index.0.insert(id, e);
        }

        for _ in 0..m.n_update {
            if off + 6 > d.len() {
                break;
            }
            let idw = u16::from_le_bytes([d[off], d[off + 1]]);
            let id = idw & ID_MASK;
            let flash = idw & FLAG_BIT != 0;
            let dir = decode(
                u16::from_le_bytes([d[off + 2], d[off + 3]]),
                u16::from_le_bytes([d[off + 4], d[off + 5]]),
            );
            off += 6;
            stats.records += 1;
            if let Some(ent) = index.0.get(&id).copied() {
                if let Ok((mut en, mut ne)) = q.get_mut(ent) {
                    ne.target = dir;
                    ne.last_seen = now;
                    if flash {
                        en.flash = 1.0;
                    }
                }
            }
        }

        for _ in 0..m.n_despawn {
            if off + 2 > d.len() {
                break;
            }
            let id = u16::from_le_bytes([d[off], d[off + 1]]) & ID_MASK;
            off += 2;
            if let Some(ent) = index.0.remove(&id) {
                if let Ok(mut ec) = commands.get_entity(ent) {
                    ec.despawn();
                }
            }
        }
    }
}

/// CLIENT: ease each proxy toward its last streamed position and animate it with the same
/// crowd animator the host uses, so a streamed horde is indistinguishable from a simulated
/// one. Also reaps proxies whose updates stopped arriving.
fn drive_proxies(
    mut commands: Commands,
    time: Res<Time>,
    planet: Option<Res<CurrentPlanet>>,
    mut index: ResMut<NetEnemyIndex>,
    q_players: Query<(&Player, &Transform), Without<NetEnemy>>,
    q_remote: Query<&crate::remote::RemoteAstronaut>,
    mut q: Query<(Entity, &NetId, &mut Enemy, &mut NetEnemy, &mut Transform)>,
) {
    let Some(planet) = planet else { return };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = time.elapsed_secs();
    let k = 1.0 - (-NET_ENEMY_SMOOTH_RATE * dt).exp();

    // Facing is derived, not sent: enemies look at the nearest astronaut, and we know where
    // every astronaut is (our own predicted one, plus replicated teammates).
    let mut snaps: Vec<crate::player::AstronautSnap> = q_players
        .iter()
        .map(|(p, tf)| crate::player::AstronautSnap { entity: Entity::PLACEHOLDER, dir: p.dir, pos: tf.translation })
        .collect();
    for r in &q_remote {
        snaps.push(crate::player::AstronautSnap {
            entity: Entity::PLACEHOLDER,
            dir: r.dir,
            pos: planet.surface_point(r.dir),
        });
    }

    for (ent, nid, mut en, mut ne, mut tf) in &mut q {
        if now - ne.last_seen > NET_ENEMY_GRACE {
            index.0.remove(&nid.0);
            commands.entity(ent).despawn();
            continue;
        }

        let prev = ne.shown;
        let ang = ne.shown.angle_between(ne.target);
        ne.shown = if ang * planet.radius > NET_ENEMY_SNAP_ARC {
            ne.target
        } else {
            sphere::step_toward(ne.shown, ne.target, ang * k)
        };
        en.dir = ne.shown;

        // Speed for the gait, finite-differenced from the SMOOTHED position and measured
        // as a CHORD — the per-frame angle is tiny, and acos(dot) there quantizes toward
        // zero at f32 precision and would leave the horde sliding in an idle pose.
        let local_r = planet.surface(ne.shown);
        let inst = (ne.shown - prev).length() * local_r / dt;
        ne.speed += (inst - ne.speed) * (1.0 - (-9.0 * dt).exp());
        en.flash = (en.flash - dt * 6.0).max(0.0);

        let target_pos = crate::player::nearest_astronaut(en.dir, &snaps, planet.radius)
            .map(|s| s.pos)
            .unwrap_or(Vec3::ZERO);
        crate::enemies::animate_crowd(&mut en, &mut tf, &planet, target_pos, ne.speed, dt, now);
    }
}

/// --netlog: what actually crossed the wire, so the bandwidth budget stops being arithmetic.
pub fn log_stream_stats(
    time: Res<Time>,
    role: Res<NetRole>,
    mut stats: ResMut<NetEnemyStats>,
    residency: Res<ClientResidency>,
    proxies: Query<(), With<NetEnemy>>,
    local_sim: Query<(), (With<Enemy>, Without<NetEnemy>, Without<NetBoss>, Without<crate::interact::Pot>)>,
    enemies: Query<(), With<Enemy>>,
    breakdown: Query<(&Enemy, Option<&crate::interact::Pot>, Option<&crate::enemies::Boss>)>,
    anchors: Query<(&PlayerId, &Player)>,
    with_id: Query<(), (With<Enemy>, With<NetId>)>,
    builds: Query<(&PlayerId, &crate::run::PlayerState)>,
    // Bundled into one param: this diagnostic hit Bevy's 16-system-param cap.
    counts: (
        Query<(), With<crate::enemies::Boss>>,
        Query<(), With<CraterpillarSegment>>,
        Query<&Transform, With<CraterpillarSegment>>,
        Query<(), With<crate::pickups::Pickup>>,
        Query<&crate::run::PlayerState, With<crate::player::LocalPlayer>>,
        Query<(), Or<(With<crate::enemies::EnemyProjectile>, With<crate::enemies::Telegraph>)>>,
    ),
    planet: Option<Res<CurrentPlanet>>,
    mine: Res<MyPlayerId>,
    // What a joiner must SEE of the per-player and event state — the co-op parity lanes.
    parity: (
        Query<&Visibility, With<crate::enemies::AnubotBeamVis>>,
        Query<(), With<AimLine>>,
        Res<crate::events_world::DustStorm>,
        Res<crate::comet::Comet>,
        Query<(&PlayerId, Has<crate::events_world::InStorm>, &crate::net::NetComet, Option<&crate::net::NetHero>)>,
        // prediction error: our body vs the host's copy of it
        Query<&Player, With<crate::player::LocalPlayer>>,
        Query<(&PlayerId, &crate::net::NetTransform), Without<Player>>,
    ),
) {
    let (n_boss, n_segs, seg_pos, n_pick, my_ps, n_hazard) = &counts;
    let Some(planet) = planet else { return };
    let anchors: Vec<(u8, Vec3)> = anchors.iter().map(|(id, p)| (id.0, p.dir)).collect();
    let now = time.elapsed_secs();
    if now < stats.next_print {
        return;
    }
    stats.next_print = now + 1.0;
    let (beam_vis, aim_lines, storm, comet, per_player, local, copies) = &parity;
    let pred_err = local
        .iter()
        .next()
        .zip(mine.0)
        .and_then(|(p, id)| copies.iter().find(|(pid, _)| pid.0 == id).map(|(_, nt)| (p.dir, nt.dir)))
        .map(|(a, b)| sphere::arc_dist(a, b, planet.radius));
    let parity_line = format!(
        "pred_err={} beam_vis={}/{} aim_lines={} storm={}{} comet[me x{} {:.0}% fires={}] players[{}]",
        pred_err.map(|e| format!("{e:.2}m")).unwrap_or_else(|| "-".into()),
        beam_vis.iter().filter(|v| **v != Visibility::Hidden).count(),
        beam_vis.iter().count(),
        aim_lines.iter().count(),
        if storm.active { "on" } else { "off" },
        if storm.player_inside { "(me inside)" } else { "" },
        comet.count,
        comet.progress * 100.0,
        comet.fires,
        per_player
            .iter()
            .map(|(id, hidden, nc, hero)| format!(
                "p{}:{}{} comet{}/f{}",
                id.0,
                hero.map(|h| crate::net::hero_from_code(h.0).def().name).unwrap_or("?"),
                if hidden { " IN-STORM" } else { "" },
                nc.count,
                nc.fires
            ))
            .collect::<Vec<_>>()
            .join(" ")
    );
    match *role {
        NetRole::Host => {
            let resident: usize = residency.0.values().map(|s| s.len()).sum();
            // Break the total down, or "resident=8 of 97" is unreadable: most of that 97
            // is static pots, which never cross the wire at all.
            let mobile = breakdown.iter().filter(|(e, pot, boss)| {
                let _ = e;
                pot.is_none() && boss.is_none()
            }).count();
            let reach: Vec<String> = anchors
                .iter()
                .map(|(pid, dir)| {
                    let n = breakdown
                        .iter()
                        .filter(|(e, pot, boss)| {
                            pot.is_none()
                                && boss.is_none()
                                && sphere::arc_dist(e.dir, *dir, planet.radius) <= NET_ENEMY_INTEREST_IN[planet_index(&planet)]
                        })
                        .count();
                    format!("p{pid}:{n}")
                })
                .collect();
            info!(
                "NETENEMY[Host] total={} mobile={} bosses={} hazards={} pickups={} with_netid={} in_interest[{}] resident_sent={} builds[{}]",
                enemies.iter().count(),
                mobile,
                n_boss.iter().count(),
                n_hazard.iter().count(),
                n_pick.iter().count(),
                with_id.iter().count(),
                reach.join(" "),
                resident,
                builds
                    .iter()
                    .map(|(id, ps)| format!(
                        "p{}:lvl{} dmg{:.2} hp{:.0} weps{} spd{:.2}",
                        id.0,
                        ps.level,
                        ps.stats.damage,
                        ps.stats.max_hp,
                        ps.weapons.len(),
                        ps.stats.attack_speed
                    ))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            info!("NETPARITY[Host] {parity_line}");
        }
        NetRole::Client => {
            info!("NETPARITY[Client] {parity_line}");
            info!(
                "NETENEMY[Client] proxies={} local_sim={} bosses={} worm_segs={} worm_len={:.1}m hazards={} pickups={} lvl={} xp={:.0} hp={:.0} gold={} rx_records={}/s chunks={}/s bytes={}/s ({:.1} KB/s) seq_gaps={} me={:?}",
                proxies.iter().count(),
                local_sim.iter().count(),
                n_boss.iter().count(),
                n_segs.iter().count(),
                // spread of the body: 12 stacked segments would read ~0
                {
                    let ps: Vec<Vec3> = seg_pos.iter().map(|t| t.translation).collect();
                    let mut d: f32 = 0.0;
                    for a in &ps {
                        for b in &ps {
                            d = d.max(a.distance(*b));
                        }
                    }
                    d
                },
                n_hazard.iter().count(),
                n_pick.iter().count(),
                my_ps.iter().next().map(|p| p.level).unwrap_or(0),
                my_ps.iter().next().map(|p| p.xp).unwrap_or(0.0),
                my_ps.iter().next().map(|p| p.hp).unwrap_or(0.0),
                my_ps.iter().next().map(|p| p.gold).unwrap_or(0),
                stats.records,
                stats.chunks,
                stats.bytes,
                stats.bytes as f32 / 1024.0,
                stats.seq_gaps,
                mine.0
            );
            stats.records = 0;
            stats.chunks = 0;
            stats.bytes = 0;
        }
        NetRole::Solo => {}
    }
}
