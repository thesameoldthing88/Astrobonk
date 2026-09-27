//! Session logging for playtests.
//!
//! A tester who double-clicks the exe has no console, so stdout logging produces nothing to
//! hand back. This writes a plain-text session file next to the save, capturing the handful
//! of things that actually explain a bad co-op session after the fact:
//!
//!   * what build and role this instance was, and its command line;
//!   * connect / disconnect / identity / stage changes, as they happen;
//!   * a health line every few seconds — role, state, players, proxies, hp, level, fps —
//!     so a desync or a stall is visible in hindsight rather than only as "it got weird";
//!   * PANICS, which are otherwise invisible when there is no console. This is the one that
//!     turns "it crashed" into a stack trace.
//!
//! Deliberately not the tracing/`LogPlugin` layer: this needs to survive a panic and be
//! readable by a human who is not a developer.

use bevy::prelude::*;
use std::fmt::Write as _;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

/// Where this session is being written.
#[derive(Resource)]
pub struct PlayLog {
    pub path: PathBuf,
    next: f32,
}

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

fn log_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join(crate::config::SAVE_DIR).join("logs")
}

/// Append one line. Opens per write on purpose — a crashed session must not lose the tail
/// to an unflushed buffer, which is exactly when the log matters most.
pub fn line(msg: impl AsRef<str>) {
    let guard = LOG_PATH.lock().ok();
    let Some(Some(path)) = guard.as_deref() else { return };
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}", msg = msg.as_ref());
    }
}

pub struct PlayLogPlugin;

impl Plugin for PlayLogPlugin {
    fn build(&self, app: &mut App) {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("session-{stamp}.log"));

        if let Ok(mut g) = LOG_PATH.lock() {
            *g = Some(path.clone());
        }

        let args: Vec<String> = std::env::args().collect();
        line(format!(
            "=== ASTROBONK session log ===\nversion : {}\nstarted : unix {stamp}\nargs    : {}\n",
            env!("CARGO_PKG_VERSION"),
            args.join(" ")
        ));

        // Without this a panic is completely silent to a tester with no console.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            line(format!("\n!!! PANIC !!!\n{info}\n"));
            prev(info);
        }));

        app.insert_resource(PlayLog { path, next: 0.0 })
            .add_systems(Update, health_line);
    }
}

/// Periodic snapshot. Cheap, and it is the difference between "it desynced" and knowing
/// which side stopped receiving.
#[allow(clippy::too_many_arguments)]
fn health_line(
    time: Res<Time<Real>>,
    mut log: ResMut<PlayLog>,
    role: Res<crate::net::NetRole>,
    state: Res<State<crate::AppState>>,
    counts: (
        Query<(), With<crate::player::Player>>,
        Query<(), With<crate::netenemy::NetEnemy>>,
        // the crowd, as the cap counts it: no pots, no bosses (L16)
        Query<(), (With<crate::enemies::Enemy>, Without<crate::interact::Pot>, Without<crate::enemies::Boss>)>,
        Query<&crate::run::PlayerState, With<crate::player::LocalPlayer>>,
    ),
    run: Option<Res<crate::run::RunState>>,
    mine: Res<crate::net::MyPlayerId>,
) {
    let now = time.elapsed_secs();
    if now < log.next {
        return;
    }
    // skip the very first sample: dt is 0 on frame one and reports a nonsense fps
    let first = log.next == 0.0;
    log.next = now + 5.0;
    if first {
        return;
    }

    let (players, proxies, enemies, local) = &counts;
    let mut s = String::new();
    let _ = write!(
        s,
        "[{now:7.1}s] {:?} state={:?} me={:?} players={} enemies={} proxies={} fps={:.0}",
        *role,
        state.get(),
        mine.0,
        players.iter().count(),
        enemies.iter().count(),
        proxies.iter().count(),
        1.0 / time.delta_secs().max(1e-6),
    );
    if let Some(ps) = local.iter().next() {
        let _ = write!(s, " hp={:.0}/{:.0} lvl={} gold={}", ps.hp, ps.stats.max_hp, ps.level, ps.gold);
    }
    if let Some(run) = run {
        let _ = write!(s, " stage={} timer={:.0} kills={}", run.stage, run.timer, run.kills);
    }
    line(s);
}
