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
use crate::net::{EnemySnapMsg, MyPlayerId, NetRole, PeerSlots};
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
            .init_resource::<NetEnemyStats>()
            .add_systems(
                Update,
                assign_net_ids
                    .run_if(crate::net::is_simulating)
                    .run_if(is_networked),
            )
            .add_systems(
                Update,
                stream_enemies
                    .after(assign_net_ids)
                    .run_if(crate::net::is_simulating)
                    .run_if(is_networked),
            )
            .add_systems(
                Update,
                (receive_enemies, drive_proxies)
                    .chain()
                    .run_if(crate::net::is_client),
            );
    }
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

    for e in &fresh {
        commands.entity(e).insert(NetId(ids.claim()));
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
                commands.entity(ent).despawn();
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
    enemies: Query<(), With<Enemy>>,
    breakdown: Query<(&Enemy, Option<&crate::interact::Pot>, Option<&crate::enemies::Boss>)>,
    anchors: Query<(&PlayerId, &Player)>,
    with_id: Query<(), (With<Enemy>, With<NetId>)>,
    planet: Option<Res<CurrentPlanet>>,
    mine: Res<MyPlayerId>,
) {
    let Some(planet) = planet else { return };
    let anchors: Vec<(u8, Vec3)> = anchors.iter().map(|(id, p)| (id.0, p.dir)).collect();
    let now = time.elapsed_secs();
    if now < stats.next_print {
        return;
    }
    stats.next_print = now + 1.0;
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
                "NETENEMY[Host] total={} mobile={} with_netid={} in_interest[{}] resident_sent={}",
                enemies.iter().count(),
                mobile,
                with_id.iter().count(),
                reach.join(" "),
                resident
            );
        }
        NetRole::Client => {
            info!(
                "NETENEMY[Client] proxies={} rx_records={}/s chunks={}/s bytes={}/s ({:.1} KB/s) seq_gaps={} me={:?}",
                proxies.iter().count(),
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
