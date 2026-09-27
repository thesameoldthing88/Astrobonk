# ASTROBONK — Technical Reference

This is the engineering manual for the ASTROBONK codebase: how to build it, how it is wired, what every system and data type does, how the co-op protocol works on the wire, and how to extend it. It is written for a human or an AI picking the project up cold.

**Scope and ground truth.** Everything here describes the code at commit `99d012f` (2026-07-26, "Fix two live co-op bugs and make the interactable layout save-independent"). Every factual claim cites `path:line` in that tree. When this document and the code disagree, the code wins. Behaviour that was derived by reading code but never reproduced at runtime is marked *code-derived*. Line citations into other documents (`GDD.md`, `PROJECT_STATUS.md`, `DEVLOG.md`) refer to those files as committed at `99d012f`; later revisions renumber them, so read them with `git show 99d012f:PROJECT_STATUS.md` and so on.

**Companion documents.**

| Document | What it is | Status |
|---|---|---|
| `GDD.md` | The full game design document (vision, systems, content, roadmap) | Design intent as committed at `99d012f`; its "current build state" section predates the co-op work. The 2026-09-26 `PROJECT_STATUS.md` (section 0) says it is being rebuilt from `plan/` |
| `DESIGN.md` | The original Megabonk research digest and v0.1 architecture sketch | Historical |
| `PROJECT_STATUS.md` | Status page | Rewritten on 2026-09-26 alongside this document (the version committed at `99d012f` was stale after co-op Stage 5) |
| `DEVLOG.md` | Session log | Updated on 2026-09-26 with the 2026-07-26 Session 3 entries and the 2026-09-26 catch-up session (the version committed at `99d012f` stopped at 2026-07-24, session 2k) |
| `src/net.rs:1199-1262` | "NETCODE NOTES" running log | Stale after Stage 3 |
| `docs/KNOWN_ISSUES.md` | The defect tracker, with full write-ups, repros and suggested fixes; uses the same H/M IDs as section 22 | Current at `99d012f` |
| `docs/CONTENT_CATALOG_v0.1.md` | Exhaustive catalog of every piece of content with its numbers | Current at `99d012f` |
| `plan/` | The forward plan for the full game | Written separately |
| **this file** | Technical reference for the code as it is | Current at `99d012f` |

---

## Contents

1. [Quick facts](#1-quick-facts)
2. [Building and running](#2-building-and-running)
3. [Command-line flags](#3-command-line-flags)
4. [The headless smoke harness](#4-the-headless-smoke-harness)
5. [Module map](#5-module-map)
6. [App state machine, run phases and the pause model](#6-app-state-machine-run-phases-and-the-pause-model)
7. [Schedules and systems (every system)](#7-schedules-and-systems-every-system)
8. [ECS data model (every component and resource)](#8-ecs-data-model-every-component-and-resource)
9. [Local message types](#9-local-message-types)
10. [Math and world model](#10-math-and-world-model)
11. [Gameplay rules as implemented](#11-gameplay-rules-as-implemented)
12. [Seeded RNG streams and the determinism contract](#12-seeded-rng-streams-and-the-determinism-contract)
13. [Save format](#13-save-format)
14. [Co-op netcode protocol](#14-co-op-netcode-protocol)
15. [Session log format](#15-session-log-format)
16. [Rendering and asset pipeline](#16-rendering-and-asset-pipeline)
17. [Audio and music synthesis](#17-audio-and-music-synthesis)
18. [UI structure](#18-ui-structure)
19. [Performance notes and budgets](#19-performance-notes-and-budgets)
20. [Bevy 0.18 gotchas this project hit](#20-bevy-018-gotchas-this-project-hit)
21. [How to add content and systems (step by step)](#21-how-to-add-content-and-systems-step-by-step)
22. [Known defects index](#22-known-defects-index)
23. [Appendix A: tuning constants](#appendix-a-tuning-constants-srcconfigrs)
24. [Appendix B: content tables](#appendix-b-content-tables)

---

## 1. Quick facts

| Item | Value | Where |
|---|---|---|
| Genre | Third-person 3D survivors roguelite on tiny spherical planets | `GDD.md` |
| Language / engine | Rust 2021 edition, Bevy 0.18.1 | `Cargo.toml:4,7` |
| Source size | 16,042 lines across 40 `.rs` files (26 in `src/`, 8 in `src/content/`, 6 in `src/ui/`) | `src/` |
| External assets | None. Every mesh is built in code, every sound and the music are synthesized in code at startup | `src/meshkit.rs`, `src/audio.rs`, `src/music.rs` |
| Binary | One crate, one binary `astrobonk` (windowed game and headless smoke test in the same exe) | `src/main.rs:51-70` |
| Players | Solo, or co-op for up to 4 (`MAX_PLAYERS`); only 2 have ever been tested, and only on one PC | `src/net.rs:38` |
| Netcode | Host-authoritative listen server, bevy_replicon 0.40.4 + bevy_replicon_renet 0.16.0, renet netcode over UDP, direct IP, port 5011 | `src/net.rs:21-38` |
| Simulation clock | Variable `dt` in `Update`; there is no `FixedUpdate` simulation | `src/main.rs:184-333` |
| Enemy cap | 1200 per party-size factor (1200 solo, up to 3600 at 4 players) | `src/config.rs:30`, `src/enemies.rs:568,592` |
| Save | JSON at `%APPDATA%/astrobonk/save.json` | `src/save.rs:107-112`, `src/config.rs:65-66` |
| Session logs | `%APPDATA%/astrobonk/logs/session-<unix-seconds>.log` | `src/playlog.rs:33-38,56-60` |
| Tests | No unit or integration tests; the headless smoke harness (section 4) is the only automated check | `src/headless.rs` |

---

## 2. Building and running

### 2.1 Toolchain

| Item | Requirement / observed | Notes |
|---|---|---|
| Rust | Stable. The dev machine builds with `rustc 1.96.0` on `stable-x86_64-pc-windows-msvc` | There is no `rust-toolchain.toml`; any recent stable that Bevy 0.18.1 accepts works |
| Target | Developed and played on Windows x86_64 (MSVC). Nothing in the code is Windows-only except the `%APPDATA%` lookup, which falls back to `.` when the variable is unset (`src/save.rs:108-110`, `src/playlog.rs:34-36`) | Linux and macOS builds are untested |
| C/C++ toolchain | MSVC build tools (Visual Studio Build Tools) on Windows, as for any Bevy project | |
| Cargo features | `bevy` with default features plus `wav` (`Cargo.toml:7`). `wav` is what lets the synthesized `AudioSource { bytes }` buffers decode | |

### 2.2 Dependencies

| Crate | Requested (`Cargo.toml`) | Resolved (`Cargo.lock`) | Why it is here |
|---|---|---|---|
| `bevy` | `0.18.1`, features `["wav"]` | 0.18.1 | Engine |
| `serde` | `1` with `derive` | 1.0.228 | Save file and wire message derives |
| `serde_json` | `1` | 1.0.150 | Save file format |
| `rand` | `0.8` | 0.8.7 | All randomness (`StdRng` for seeded streams, `thread_rng` elsewhere) |
| `bevy_replicon` | `0.40.0` | 0.40.4 | Replication, messages, client/server state |
| `bevy_replicon_renet` | `0.16.0` | 0.16.0 | renet transport backend for replicon (pulls `renet` 2.0.0 and `renet_netcode` 2.0.0) |

**The replicon pins are load-bearing.** The next releases of `bevy_replicon` and `bevy_replicon_renet` depend on Bevy 0.19; letting cargo upgrade them links two Bevy versions into one binary (`PROJECT_STATUS.md:52-53`, commit `a0924d2`). Do not run `cargo update -p bevy_replicon` without moving the whole project to Bevy 0.19. `Cargo.lock` is committed and resolves `rand` twice (0.8.7 for this crate, 0.9.5 for a dependency); only 0.8.7 is used by our code.

### 2.3 Profiles

| Profile | Settings | Effect |
|---|---|---|
| `dev` | `opt-level = 1` for this crate; `opt-level = 3` for every dependency (`Cargo.toml:14-18`) | A playable debug build: Bevy and friends are fully optimized, our code keeps debug assertions (integer-overflow checks included) and fast incremental builds. Typical no-op rebuild about 0.4 s; the debug exe is about 98 MB |
| `release` | `lto = "thin"`, `codegen-units = 1` (`Cargo.toml:20-22`) | Ship build. Slow link, about 69 MB exe. This is what playtests should use |

Debug assertions matter: the project already hit one debug-only integer-overflow panic (crater-width hash, fixed with `wrapping_mul`, `src/sphere.rs:97`; `DEVLOG.md:332`). Always hash with `wrapping_*` arithmetic.

### 2.4 Everyday commands

| Goal | Command |
|---|---|
| Build (dev) | `cargo build` |
| Run the game (dev) | `cargo run` |
| Ship build | `cargo build --release` then run `target/release/astrobonk.exe` |
| Main smoke test | `cargo run -- --headless 2400` (or run the built exe with the same flags) |
| Boss-path smoke | `cargo run -- --headless 1200 --fast-boss` |
| Two-player host smoke | `cargo run -- --headless 2400 --coop2` |
| Two instances on one PC | Instance A: `astrobonk.exe --host --autodrop --netlog` ; instance B: `astrobonk.exe --join 127.0.0.1 --autodrop --netlog` (see 3.3) |

`cargo build` currently finishes with 0 errors and 35 warnings (unused imports and variables, never-read fields such as `Joint.lag` at `src/player.rs:100` and `PlanetDef.sky`/`meteor_showers`, and the never-called `net::disconnect` at `src/net.rs:1133`). None of the warnings is load-bearing.

### 2.5 Windows notes

| Topic | What to know |
|---|---|
| Console window | There is no `#![windows_subsystem = "windows"]` attribute, so the exe opens a console window next to the game. `info!`/`warn!` output and the `--netlog` and headless output go there. Closing the console kills the game, and on a panic the console closes too; that is why the session log file exists (section 15) |
| Locked exe | Windows locks a running `.exe`. `cargo build --release` fails to replace `target/release/astrobonk.exe` while any instance of it is running. Close the game first |
| Killing processes | Never kill the game by image name (`taskkill /IM astrobonk.exe` or `Stop-Process -Name astrobonk`). The user may have a playtest instance open. Kill only the PID you started |
| Stale binaries | Build output is not in the repository (`target/` is git-ignored), and a locally built release exe can silently lag the source: on the development machine at the time of writing, the release exe predated HEAD by 4 minutes and lacked the last commit's fixes. Rebuild before any playtest, and give both co-op machines a byte-identical exe (the protocol check does not catch field-layout changes, see 14.2) |
| Firewall | The host binds UDP `0.0.0.0:5011` (`src/net.rs:1073`). Windows Defender Firewall prompts on first host; allow it on private networks, or add an inbound UDP 5011 rule, before a two-machine test |
| LAN address | The host's menu prints the LAN IP picked by `net::local_ip()` (`src/net.rs:1051-1060`), a UDP-connect trick that sends nothing. Behind a VPN it can pick the wrong interface; use `ipconfig` to confirm. The note is currently written in the same click that leaves the main menu, so it is effectively invisible (`src/ui/menus.rs:265-270`); press BACK to see it |
| Save and logs | `%APPDATA%\astrobonk\save.json` and `%APPDATA%\astrobonk\logs\`. Delete `save.json` to start a fresh profile |
| Shells | Both PowerShell and Git Bash work. From Git Bash use forward slashes: `./target/debug/astrobonk.exe --headless 2400` |

---

## 3. Command-line flags

All flags are plain `std::env::args()` tokens; there is no argument parser. Several are re-read every frame by run conditions (`src/main.rs:164,170`), which is harmless but means the dev CLI ships in release builds.

### 3.1 Flag reference

| Flag | Argument | Mode | Effect | Read at |
|---|---|---|---|---|
| `--headless` | optional tick count, **only** the token directly after the flag; default 1500 | Headless | Runs the smoke harness instead of the game (section 4). `--headless --coop2 2400` runs 1500 ticks because `--coop2` is not a number; write `--headless 2400 --coop2` | `src/main.rs:53-54` |
| `--fast-boss` | none | Headless | Veteran tomes (Damage 20, Health 20), clock at 95 s (boss mark is 90 s), `elapsed = 570` (full late-game spawn mix); skips the zero-kill check, requires the boss to spawn | `src/main.rs:55`, `src/headless.rs:204-216,338,350` |
| `--hero` | hero name (`buzz`, `valentina`, `b0nk`/`bonk`, `yuki`, `chimpo`/`chimp`, `doug`, `reticle`, `nova`, `ironclad`, `fortuna`, `aurora`, `gristle`; case-insensitive) | Headless | Starting hero; default Buzz; unknown names fall back to Buzz | `src/main.rs:56-61`, `src/content/characters.rs:218-235` |
| `--planet` | `mars` or `darkmoon` | Headless | Starting planet; anything else is the Moon | `src/main.rs:62-66` |
| `--seed` | `u64` | Headless | Overrides `run_seed` | `src/main.rs:67`, `src/headless.rs:210-212` |
| `--coop2` | none | Headless | Spawns a second (non-local) astronaut `PlayerId(1)`, reproducing a 2-player host without networking | `src/headless.rs:377-379` |
| `--enemydist` | none | Headless | Every 450 ticks prints a histogram of mobile-enemy distance to the nearest astronaut | `src/headless.rs:113-162,292` |
| `--autodrop` | none | Windowed | Boot straight into a run (Buzz, Moon tier 1, or tier 3 with `--stagenow`) instead of the main menu. Ignored when `--join` is present | `src/main.rs:398-403` |
| `--host` | none | Windowed | At startup, start hosting on `--port` or 5011 (`NetRole::Host`) | `src/net.rs:1148-1157` |
| `--join` | IPv4/IPv6 address; default `127.0.0.1` if missing or unparsable | Windowed | At startup, connect to the host (`NetRole::Client`). The client waits for the host's run snapshot before entering the run | `src/net.rs:1158-1172` |
| `--port` | `u16` | Windowed | Port for `--host`/`--join`; default 5011. The menu path always uses 5011 | `src/net.rs:1149-1154,1163-1168` |
| `--botinput` | none | Windowed | Overrides the local `InputIntent` with a slow circle-strafe, so an instance walks with nobody at the keyboard | `src/net.rs:790-796,1145` |
| `--netlog` | none | Windowed | Once a second prints astronaut poses, the world-layout and interactable checksums, and stream statistics (host and client) to stdout | `src/net.rs:800-880`, `src/netenemy.rs:1269-1386` |
| `--autopick` | none | Windowed | Takes option 1 of every card panel and closes chest/shop panels immediately | `src/main.rs:166-171,455-480` |
| `--stagenow` | none | Windowed | Boot tier becomes 3 (Moon, Mars, Dark Moon) and, about 20 s into stage 1, the host requests stage 2 | `src/main.rs:160-165,381-385,484-500` |
| `--bossnow` | none | Windowed | On the first in-run frame (host or solo), winds the clock to `BOSS_MARK + 4` s and marks both minibosses as spawned | `src/main.rs:504-520` |
| `--warp` (with `--dev`) | `noon`, `dusk` or `night` | Windowed | A second into the run, sets the local astronaut (host/solo only) where the sun stands high, just on the lit side of the terminator, or deep in the night, facing away from the sun — the toon look's windowed checks (section 16.9) | `dev_warp` in `src/main.rs` |

### 3.2 Flag interactions and traps

| Combination | What happens |
|---|---|
| `--host` without `--autodrop` | *Code-derived:* should panic as soon as the host is networked and no run has happened in this process, because `stream_enemies` takes a non-optional `Res<CurrentPlanet>` and `CurrentPlanet` is first inserted by `enter_run` (`src/netenemy.rs:284`, `src/main.rs:446`). Clicking HOST CO-OP in the menu before any run hits the same path |
| `--join` with `--autodrop` | `--autodrop` is ignored; the joiner enters the run only when the first `RunSnapMsg` arrives (`src/main.rs:354-376,398`) |
| Joining while the host is in its menus | The joiner builds the host's placeholder or previous world and does not rebuild when the host starts at stage 0 (bug H8, section 22). Connect after the host is in the run |
| `--stagenow` on the client | The client also boots tier 3 but never triggers the stage change; it follows the host (`src/main.rs:491`) |
| `--bossnow` and `--stagenow` on a client | No-ops; clients never own the clock (`src/main.rs:491,509`) |
| `--seed`, `--hero`, `--planet` in windowed mode | Ignored; they are read only by the headless branch |

### 3.3 Recipes

| Goal | Commands |
|---|---|
| Walk-test input routing | A: `astrobonk.exe --host --autodrop --netlog` ; B: `astrobonk.exe --join 127.0.0.1 --autodrop --botinput --netlog`. On A, player 1's `dir` should trace B's circle while A's own astronaut stands still (`src/net.rs:1204-1215`) |
| Build-sync test (both levelling) | Add `--autopick --botinput` to both instances; without `--autopick` a bot client stalls on its first level-up panel |
| Stage-follow test | A: `--host --autodrop --stagenow --autopick --netlog` ; B: `--join 127.0.0.1 --autodrop --autopick --netlog`. About 20 s in, both should log a Moon to Mars rebuild |
| Boss lane test | Add `--bossnow` to the host |
| Layout parity check | With `--netlog`, compare `layout_sum=` and `inter_sum=` on both machines each second. `inter_sum` legitimately differs when the two saves disagree on `chimp_freed` (the cage is per-machine) and never includes the host-only teleporter on the client (`src/net.rs:835-842`) |

---

## 4. The headless smoke harness

`astrobonk --headless [ticks]` (`src/main.rs:53-70`) runs `headless::run_headless` (`src/headless.rs:182-359`) instead of the game: a separate `App` with no window, no renderer, no audio and no networking, driven by a bot, with a fixed 33 ms step. It prints a summary and exits with status 1 on failure. It is the only automated test.

### 4.1 How the headless app differs from the real one

| Aspect | Headless (`src/headless.rs`) | Real game (`src/main.rs`) |
|---|---|---|
| Plugins | `MinimalPlugins` with `ScheduleRunnerPlugin::run_once()`, `TransformPlugin`, `StatesPlugin`, `AssetPlugin` (`:191-196`) | `DefaultPlugins`, `NetPlugin`, `RemoteVisualsPlugin`, `EnemyStreamPlugin`, `PlayLogPlugin` (`:72-89`) |
| Time | `TimeUpdateStrategy::ManualDuration(33 ms)` (`:197`); the loop calls `app.update()` once per tick (`:306-313`) | Real frame time |
| Networking | None. `NetRole` does not exist; the co-op code paths are exercised only through `--coop2`'s second astronaut. Of the net module's types only the local `GrantOut` message is registered, because `pickup_update` writes it (`:238`) | Full `NetPlugin` |
| Save | `MetaSave::default()`, never loaded or written (`:203`) | Loaded at boot, written at results |
| State | `insert_state(AppState::InRun)` directly (`:304`); `headless_enter` runs at `Startup` and builds the world (`:361-382`) | Menus and `enter_run` |
| System list | A hand-maintained duplicate of the simulation (`:246-301`) with these differences: no `gather_local_input`/`player_input` (the bot writes `vel_t` directly), no `aura_follow`, `enemy_flash`, `animate_player`, `interact_system`, `gem_merge`, `death_watch`, `bank_results`, UI, audio, camera or net systems; the enemy chain and the combat chain are gated only on `playing` (no `AppState` check, no role gates); `charge_shrines`, `pickup_update`, `run_clock`, `levelup_trigger`, `comet_system` and `dust_storm_system` join the combat chain | Section 7 |
| Pause model | No `phase_time_control`, so virtual time never pauses. The bot resolves any `LevelUp`/`Modal` phase the next time `bot_drive` runs (usually the following tick, since `bot_drive` is unordered relative to `levelup_trigger`) | Section 6.3 |
| Assets | `Assets<Mesh>` and `Assets<StandardMaterial>` exist so spawn code runs unchanged; nothing renders | |

Because the list is duplicated, **any new simulation system must be added to both `src/main.rs` and `src/headless.rs`**, and any new message a simulation system writes must be `add_message`'d in both.

### 4.2 The bot

| System | Behaviour | Where |
|---|---|---|
| `bot_drive` | If the phase is `LevelUp` or `Modal`: applies option 1 of the choice panel to the **local** astronaut only (decrementing `pending_levelups` for level-ups), closes chest and shop panels, and sets `Playing`. Otherwise, for every astronaut: below 45% HP it runs directly away from the nearest non-pot enemy; above, it chases the nearest XP gem within 30 m; with neither, it wanders on a heading that rotates at 0.25 rad/s. Speed is 85% of the run speed, written straight into `Player.vel_t` and `facing` | `src/headless.rs:22-110` |
| `bot_watchdog` | Every tick: panics with `SMOKE FAIL: enemy cap breached (N)` if live `Enemy` entities exceed `ENEMY_CAP + 400`, and with `SMOKE FAIL: non-finite run state` if the local HP or the run timer is not finite. Every 300 ticks (9.9 s) prints a progress line | `src/headless.rs:164-180` |
| `enemy_distance_probe` | `--enemydist` only: histogram of mobile enemies (pots excluded) by great-circle distance to the nearest astronaut, buckets <20, <40, <60, <80, <120, <200, 200+ m | `src/headless.rs:116-162` |

The run stops early when the phase becomes `Dead` (`src/headless.rs:308-312`).

### 4.3 Output

```
ASTROBONK headless smoke: 2400 ticks @33ms hero=BUZZ planet=Moon seed=None
  t=  59s timer=540.7 lvl=2 kills=7 hp=22 enemies=173 gold=0 static=false
  ...
--- SMOKE SUMMARY ---
phase=Playing level=7 kills=155 gold=57 hp=35/106 timer=521 enemies=92 comets=2 storm[spawned=false active=false] boss_spawned=false boss_dead=false
SMOKE OK
```

Progress lines come from `bot_watchdog` (`src/headless.rs:174-179`); the summary from `src/headless.rs:327-331`. `enemies=` counts every `Enemy`, so it includes pots (60 on the Moon at the start).

### 4.4 Pass/fail rules

| Check | Fails when | Skipped when | Where |
|---|---|---|---|
| Enemy cap | Live `Enemy` count > 1600 at any tick (panic) | never | `src/headless.rs:166-169` |
| Finite state | Local HP or `RunState.timer` is NaN/infinite (panic) | never | `src/headless.rs:170-173` |
| Stuck panel | Final phase is `LevelUp` or `Modal` | never | `src/headless.rs:334-337` |
| Something died | `run.kills == 0` | `--fast-boss` | `src/headless.rs:338-341` |
| XP pipeline | Summary level < 2 while `kills > 50` | never | `src/headless.rs:342-345` |
| Spawner alive | No live `Enemy` and the boss is not dead | never | `src/headless.rs:346-349` |
| Boss path | `--fast-boss` and `boss_spawned == false` | not `--fast-boss` | `src/headless.rs:350-353` |

If none fail it prints `SMOKE OK` and exits 0; otherwise it prints each `FAIL: ...` line and calls `std::process::exit(1)` (`src/headless.rs:354-358`).

**Harness bug.** The summary reads the first `PlayerState` from an **unfiltered** query (`src/headless.rs:317`). With `--coop2` that can be the peer, which may have died at level 1, so the XP-pipeline check fails spuriously. The progress lines filter `With<LocalPlayer>` correctly. The fix is `query_filtered::<&PlayerState, With<LocalPlayer>>()`.

### 4.5 The smoke paths and what each proves

| Command | Proves | Does not prove | Status at `99d012f` (debug exe) |
|---|---|---|---|
| `--headless 2400` | Spawner, steering, the weapon chain, kills, drops, XP to level-ups, panel resolution, comet cash-outs, no cap breach, no NaN over about 79 s of sim time | Menus, input, rendering, audio, co-op wire, interactables other than charge shrines | Pass |
| `--headless 2400 --coop2` | Host correctness with two `Player`/`PlayerState` astronauts: shared XP, per-player targeting, run continues while one player is alive. Before Stage 3 this path showed a frozen clock and 0 kills | Anything on the wire; the second astronaut is not driven by `InputIntent` | Pass (last run as `--headless --coop2`, i.e. 1500 ticks) |
| `--headless 1200 --fast-boss` | The boss spawns at the boss mark and the late-game mix (Beamer, Lobber, UFO, Burrower) runs without panicking | That the bot can fight: today it dies at level 1 with 0 kills and still passes, because the kill check is skipped | Pass (weak) |
| `--headless 1200 --fast-boss --coop2` | Same, with two astronauts | | Flaky, 1 of 3 passed, because of the summary-query bug above |
| `--headless ... --planet mars` | Dust storm spawns and blows (the first storm comes 10 s in); with `--fast-boss` added, Anubot and the Verdict Beam run (a plain 2400-tick run ends about 79 s into a 600 s stage, long before the 90 s boss mark) | | Not re-run for this document |

### 4.6 Limitations

- **Not deterministic, even with `--seed`.** The step is a fixed 33 ms, but combat, loot, boss placement, level-up cards and the storm draw from `thread_rng` (section 12). Two runs of the same build diverge, so the smoke cannot be a before/after oracle for a refactor (`PROJECT_STATUS.md:141-144`).
- It cannot exercise `InputIntent`, `phase_time_control`, any UI, the renderer, audio, or any network path.
- Its system list drifts from the real one (section 4.1).

---

## 5. Module map

### 5.1 Files

| File | Lines | Role | Key items |
|---|---|---|---|
| `src/main.rs` | 536 | App assembly. Declares every module, `AppState`, the `playing` run condition, the headless branch, every system registration and its role gate (THE SWITCH), `boot`, `enter_run`, the client run-follow system and the dev systems | `AppState` `:35-44`; `playing` `:47-49`; `main` `:51-335`; `setup_camera` `:337-346`; `client_follow_host_run` `:354-376`; `boot` `:379-404`; `enter_run` `:406-449`; `dev_autopick` `:455-480`; `dev_stage_now` `:484-500`; `dev_fast_boss` `:504-520`; `clear_panels` `:523-536` |
| `src/config.rs` | 99 | Every tuning constant (movement, camera, remote smoothing, enemy cap, spawn arcs, pickups, stage marks, XP curve, chest cost, comet, save names, net streaming) | Appendix A |
| `src/headless.rs` | 382 | Smoke harness | Section 4 |
| `src/sphere.rs` | 176 | Spherical math and the analytic terrain | `hills` `:26-37`; `hash_dir` `:59-68`; `Terrain` `:72-111`; `tangent_frame` `:114-119`; `frame_quat` `:122-126`; `step_toward` `:130-137`; `advance` `:141-151`; `arc_dist` `:154-156`; `offset_dir` `:159-165`; `fib_sphere` `:168-176` |
| `src/planet.rs` | 517 | Stage world: `StageScoped`, prop colliders, `CurrentPlanet`, the terrain mesh, prop/flora/sky/sun spawning | `PropColliders::resolve` `:33-55`; `CurrentPlanet` `:59-89`; `despawn_stage` `:91-95`; `planet_mesh` `:98-130`; `spawn_stage` `:143-503`; `random_dir` `:505-517` |
| `src/toon.rs` | 440 | The toon look (P36): `ToonPlugin`, the bevy_pbr lighting patch, the ink/rim render-graph pass (`src/toon_outline.wgsl`), per-world grading, the camera bundle | Section 16.9 |
| `src/meshkit.rs` | 224 | Procedural mesh compositor (bakes primitives into one vertex-coloured `Mesh`) and `icosphere()` | `MeshData` `:13-173`; `icosphere` `:182-224` |
| `src/player.rs` | 875 | The astronaut: components, spawning, the rig, local input gathering, movement and physics, the animator, the camera, cursor lock, per-player upkeep | Section 8; `spawn_player` `:117-175`; `build_astronaut_rig` `:183-313`; `gather_local_input` `:382-414`; `player_input` `:418-520`; `player_physics` `:523-589`; `animate_rig` `:617-706`; `camera_rig` `:747-824`; `cursor_control` `:827-839`; `player_upkeep` `:842-875` |
| `src/enemies.rs` | 1663 | The horde and bosses: components, assets, spatial hash, spawn director, steering, crowd animation, every enemy attack, the Craterpillar and Anubot, telegraphs, hit-flash, the DEV boss key | `director_spawn` `:539-626`; `spawn_boss` `:628-714`; `enemy_move` `:1019-1121`; `animate_crowd` `:1129-1193` |
| `src/combat.rs` | 1024 | Player weapons (nine behaviours), projectiles, drones, beams, auras, faders, and damage resolution in both directions | `weapon_fire` `:212-577`; `apply_hits` `:877-970`; `apply_player_hits` `:973-1024` |
| `src/pickups.rs` | 404 | Drops and collection: XP gems, gold, silver, food, powerups; kill loot; gem merging | `pickup_update` `:110-234`; `kill_drops` `:296-372`; `gem_merge` `:375-404` |
| `src/interact.rs` | 551 | Interactables: pots, chests, Shady Guy, greed/magnet shrines, Moai, microwave, cage, teleporter, charge shrines; the seeded layout | `spawn_interactables` `:135-296`; `spawn_teleporter` `:299-335`; `charge_shrines` `:338-390`; `interact_system` `:394-551` |
| `src/director.rs` | 346 | Run flow: stage clock, boss marks, THE STATIC, level-up trigger, stage transitions, downed/death watch, results banking | `run_clock` `:35-115`; `levelup_trigger` `:118-143`; `stage_transition` `:147-232`; `downed_watch` `:240-252`; `death_watch` `:254-269`; `bank_results` `:272-346` |
| `src/run.rs` | 640 | Run data: `GameRng`, seeds and the daily, `RunPhase`, `RunState` (run-global), `PlayerState` (per astronaut), stat recompute, upgrade rolls, `ChoicePanel` | Section 8 |
| `src/stats.rs` | 180 | The 27-stat sheet `Stats`, `StatKind` and labels | `Stats::apply` `:103-134` |
| `src/comet.rs` | 127 | The Comet Combo (tail charge and cash-out) | `comet_system` `:33-127` |
| `src/events_world.rs` | 130 | Mars migrating dust storm, the only planet event | `dust_storm_system` `:32-130` |
| `src/fx.rs` | 186 | Screenshake, hitstop, `phase_time_control` (the pause model), particles | `phase_time_control` `:47-59` |
| `src/save.rs` | 240 | `MetaSave`/`Counters`, load/migrate/save, quests | Section 13 |
| `src/tutorial.rs` | 88 | Six Mission Control radio lines for a first run | `tutorial_system` `:32-88` |
| `src/messages.rs` | 84 | Local (never networked) Bevy messages: `HitMsg`, `PlayerHitMsg`, `KillMsg`, `NumberMsg`, `BannerMsg`, `SfxMsg` | Section 9 |
| `src/audio.rs` | 197 | 16 synthesized SFX, playback with throttling | Section 17 |
| `src/music.rs` | 320 | 32 s, 6-stem adaptive synthwave | Section 17 |
| `src/net.rs` | 1262 | Co-op core: `NetRole`, all wire types, `NetPlugin` (replicon/renet setup, channels, gates), identity, input routing, run snapshots, build sync, grant relay, seating, transport, CLI, `--netlog`; the stale NETCODE NOTES at `:1199-1262` | Section 14 |
| `src/netenemy.rs` | 1386 | `EnemyStreamPlugin`: the crowd, boss, hazard and pickup lanes (host) and the ordered client proxy chain, including client stage-follow | Section 14 |
| `src/remote.rs` | 209 | `RemoteVisualsPlugin`: client-side rigs for teammates driven by `NetTransform` | Section 14.15 |
| `src/playlog.rs` | 133 | `PlayLogPlugin`: session log file, panic hook, 5 s health line | Section 15 |
| `src/content/mod.rs` | 59 | `Rarity` (colours, names, luck-weighted roll) | `Rarity::weights` `:38-46` |
| `src/content/characters.rs` | 245 | 12 heroes: `AstronautKind`, `Passive`, `AstronautDef` | Appendix B |
| `src/content/weapons.rs` | 518 | 16 base weapons + 16 evolutions: `WeaponKind`, `Behavior`, `WeaponDef`, level scaling | Appendix B |
| `src/content/items.rs` | 249 | 22 items: `ItemKind`, `ItemDef` | Appendix B |
| `src/content/enemies.rs` | 243 | 9 enemy kinds, 4 bosses, the spawn mix, elite modifiers, time scaling | Appendix B |
| `src/content/planets.rs` | 147 | 3 planets: `PlanetKind`, `PlanetDef`, `FloraStyle`, tier chains | Appendix B |
| `src/content/quests.rs` | 84 | 16 quests: `QuestKind`, `Reward` | Appendix B |
| `src/content/tomes.rs` | 57 | 8 tomes: `TomeKind`, `TomeDef`, silver cost | Appendix B |
| `src/ui/mod.rs` | 64 | UI constants, `txt`/`overlay_root`/`button_node` helpers, `button_hover` | Section 18 |
| `src/ui/hud.rs` | 629 | In-run HUD, edge markers, banners, boss bar, comet readout, dust haze | Section 18 |
| `src/ui/menus.rs` | 815 | Main menu (tome shop, quest log, HOST/JOIN, DAILY), hero select, planet select, results, IP entry | Section 18 |
| `src/ui/panels.rs` | 586 | Modal panels: level-up/shrine/Moai/microwave cards, chest reveal, Shady Guy shop, pause | Section 18 |
| `src/ui/numbers.rs` | 163 | Pooled, merging floating damage numbers | Section 18 |
| `src/ui/settings.rs` | 204 | Settings overlay (volumes, sensitivity, shake) | Section 18 |

### 5.2 Plugins

| Plugin | File | Adds |
|---|---|---|
| `DefaultPlugins` (window title "ASTROBONK") | `src/main.rs:73-79` | Engine |
| `net::NetPlugin` | `src/net.rs:341-458` | `RepliconPlugins`, `RepliconRenetPlugins`, replication rules, all wire messages, identity, run sync, build sync, grants, input routing, seating, `--netlog` |
| `remote::RemoteVisualsPlugin` | `src/remote.rs:43-57` | Teammate rigs on clients |
| `netenemy::EnemyStreamPlugin` | `src/netenemy.rs:150-222` | Stream lanes and the client proxy chain |
| `toon::ToonPlugin` | `src/toon.rs` | Cel lighting patch, ink/rim pass (render app: `RenderStartup` pipeline, `InkNode` between tonemapping and FXAA), per-world grade, sun tracking. Windowed only |
| `playlog::PlayLogPlugin` | `src/playlog.rs:50-83` | Session log and panic hook (the log file is opened in `build`, before the app runs) |

Everything else is registered directly in `main()` (`src/main.rs:90-333`).

### 5.3 Cross-cutting patterns a newcomer must know

| Pattern | Rule | Why |
|---|---|---|
| `InputIntent` | Hardware input and network input both write the per-astronaut `InputIntent` component (`src/player.rs:70-77`); movement reads only the component | Movement, physics and combat contain no networking code. Anything that writes intent must run `.before(send_local_input)` (`src/net.rs:433-443`) |
| `RunState` vs `PlayerState` | `RunState` is one run-global resource (clock, chain, counters). `PlayerState` is a component on every simulated astronaut (HP, build, level, gold) | Co-op: the host holds N `PlayerState`s, a client holds exactly one |
| `LocalPlayer` | Exactly one per machine. Every per-machine `.single()` (HUD, camera, panels, level-up, comet, storm haze) must filter `With<LocalPlayer>` | About 30 systems use `.single()`; a second match makes them silently return |
| Remote rigs | A teammate drawn on a client never gets `Player`, `PlayerState` or `StageScoped` (`src/remote.rs:8-19`) | `player_physics` would stomp its transform, `.single()` sites would break, and `despawn_stage` would despawn a replicated entity |
| `NetRole` gating | Anything that mutates world state, deals damage, rolls loot, changes `RunState` counters or writes the save runs `.run_if(net::is_simulating)` (Solo and Host). Visual and local systems stay ungated | THE SWITCH (section 14.13) |
| Pause | Every non-`Playing` `RunPhase` pauses `Time<Virtual>` (`src/fx.rs:47-59`) | Section 6.3 |
| `StageScoped` | Everything spawned for a stage carries it; stage changes and `OnExit(InRun)` despawn all of it, astronauts included | Carry anything you need across (sheets) before the sweep |
| Pots and bosses are `Enemy` | A pot is `Pot` + `Enemy{speed 0, contact_cd INF, hp 1}` (`src/interact.rs:164-192`); a boss is `Enemy{kind: Bruiser}` + `Boss` (`src/enemies.rs:659-677`) | Filter pots out (`Without<Pot>`, `speed == 0`) wherever "enemy" means horde; key boss visuals off `BossKind`, never `Enemy.kind` |
| Snapshot before mutate | Systems that loop `&mut` over enemies first copy astronaut positions into a `Vec<AstronautSnap>` (`src/player.rs:44-53`) | Bevy query conflicts (B0001) and cost |
| Great-circle distance | Targeting uses `sphere::arc_dist`, never `Vec3::distance`, for "who is nearest" (`src/player.rs:55-64`) | Chords under-read across the planet |

---

## 6. App state machine, run phases and the pause model

### 6.1 `AppState`

`AppState` (`src/main.rs:35-44`) is a Bevy `States` enum: `Boot` (default), `MainMenu`, `CharSelect`, `PlanetSelect`, `InRun`, `Results`.

```
Boot ──boot()──► MainMenu ──LAUNCH/HOST CO-OP──► CharSelect ──card──► PlanetSelect ──tier──► InRun ──death/victory──► Results ──continue──► MainMenu
  │                 │  ▲                             │  ▲  └──daily card───────────────────────► InRun                      
  │                 │  └───────BACK──────────────────┘  └──────────BACK──────────────┘
  └──--autodrop─────┼────────────────────────────────────────────────────────────────────────► InRun
                    └──client: host's seed arrived (client_follow_host_run)───────────────────► InRun
```

| From | To | Trigger | Where |
|---|---|---|---|
| `Boot` | `MainMenu` | `boot` at `Startup` (default) | `src/main.rs:402` |
| `Boot` | `InRun` | `boot` with `--autodrop` and without `--join` | `src/main.rs:399-400` |
| `MainMenu` | `CharSelect` | LAUNCH (clears `Selected.daily`), DAILY (sets daily, Moon T1), or a successful HOST CO-OP | `src/ui/menus.rs:285-299,270` |
| `MainMenu` or `Boot` | `InRun` | Client only: `RunSync.seeded` became true | `src/main.rs:354-376` |
| `CharSelect` | `PlanetSelect` | Hero card clicked (normal run) | `src/ui/menus.rs:493-495` |
| `CharSelect` | `InRun` | Hero card clicked on a daily run: builds `RunState` with the daily seed, Moon T1, `is_daily` | `src/ui/menus.rs:486-492` |
| `CharSelect` | `MainMenu` | BACK | `src/ui/menus.rs:499-503` |
| `PlanetSelect` | `InRun` | Tier button: builds a fresh `RunState` (new `fresh_seed`) from `Selected` | `src/ui/menus.rs:589-596` |
| `PlanetSelect` | `CharSelect` | BACK | `src/ui/menus.rs:598-602` |
| `InRun` | `Results` | `death_watch` 1.6 real seconds after `RunPhase::Dead` (host/solo only) | `src/director.rs:254-269` |
| `InRun` | `Results` | `stage_transition` with a target beyond the chain (victory) | `src/director.rs:165-177` |
| `Results` | `MainMenu` | CONTINUE, Space or Enter | `src/ui/menus.rs:648-662` |

State hooks:

| Hook | Systems | Where |
|---|---|---|
| `OnEnter(MainMenu)` / `OnExit(MainMenu)` | `spawn_main_menu` / `despawn_menu` | `src/main.rs:134-135` |
| `OnEnter(CharSelect)` / `OnExit` | `spawn_char_select` / `despawn_menu` | `src/main.rs:136-137` |
| `OnEnter(PlanetSelect)` / `OnExit` | `spawn_planet_select` / `despawn_menu` | `src/main.rs:138-139` |
| `OnEnter(InRun)` | `enter_run`, `spawn_hud`, `start_music` (unordered) | `src/main.rs:140` |
| `OnExit(InRun)` | `despawn_stage`, `despawn_hud`, `clear_panels`, `stop_music` (`src/main.rs:141-144`); `clear_stream_indices` (`src/netenemy.rs:220`); `despawn_remote_rigs` (`src/remote.rs:55`) | |
| `OnEnter(Results)` | `bank_results.run_if(is_simulating)` then `spawn_results` (chained) | `src/main.rs:145-155` |
| `OnExit(Results)` | `despawn_menu` | `src/main.rs:156` |

`boot` (`src/main.rs:379-404`) loads `MetaSave` and inserts a **placeholder** `RunState` (Buzz, Moon, tier 1, or tier 3 with `--stagenow`) so every `Res<RunState>` exists from the first frame; menus replace it before a run. `CurrentPlanet` is not inserted until `enter_run`, and afterwards it is never removed.

`enter_run` (`src/main.rs:406-449`) resets `Comet`, arms the tutorial (first non-daily run on a fresh save), sets `RunSync.world_built`, reseeds `GameRng` to `run_seed + stage`, builds the stage (`spawn_stage`), inserts `PropColliders`, spawns the local astronaut as `PlayerId(0)` at `Vec3::Y`, spawns the interactables from a **fresh** level-1 sheet, inserts `CurrentPlanet`, resets `Director` and sets `RunPhase::Playing`.

### 6.2 `RunPhase`

`RunPhase` (`src/run.rs:68-77`) is a plain resource, not a Bevy state: `Playing` (default), `LevelUp`, `Modal` (a non-level-up panel), `Paused`, `Dead`. It is per machine; in co-op each machine has its own.

| From | To | Trigger | Where |
|---|---|---|---|
| `Playing` | `LevelUp` | Local `PlayerState.pending_levelups > 0` | `src/director.rs:118-143` |
| `LevelUp` | `Playing` | Last pending level-up resolved (pick or skip) | `src/ui/panels.rs:244-256` |
| `Playing` | `Modal` | Charge shrine completed (blessing); chest, shop, Moai or microwave opened | `src/interact.rs:386,473,478,512,533` |
| `Modal` | `Playing` | Card picked; chest TAKE/LEAVE; shop closed | `src/ui/panels.rs:254-255,360-365,492-495` |
| `Playing` | `Paused` | Esc | `src/ui/panels.rs:524-527` |
| `Paused` | `Playing` | Esc or RESUME (unless the settings overlay is open) | `src/ui/panels.rs:568-576` |
| `Paused` | `Dead` | ABANDON RUN (also sets `RunResult::Death`) | `src/ui/panels.rs:577-582` |
| `Playing` | `Dead` | Every astronaut's `PlayerState.dead` is true (host/solo) | `src/director.rs:240-252` |
| any | `Playing` | `enter_run`, `stage_transition`, `client_stage_transition`, `bank_results` | `src/main.rs:448`; `src/director.rs:231,344`; `src/netenemy.rs:832` |
| `LevelUp`/`Modal` | `Playing` | Dev: `--autopick`; headless bot | `src/main.rs:455-480`; `src/headless.rs:40-58` |

### 6.3 The pause model: `Time<Virtual>`

| Mechanism | Behaviour | Where |
|---|---|---|
| `phase_time_control` | Runs every frame in every state; acts only when `RunPhase` changed. `Playing` unpauses virtual time and resets relative speed to 1.0; **every other phase pauses it** | `src/fx.rs:47-59` |
| `hitstop_system` | While `Playing` and `Hitstop.timer > 0`, sets virtual relative speed to 0.06 and counts the timer down in **real** time; restores 1.0 afterwards. Triggers: stage-boss kill 0.25 s (`src/combat.rs:962`), comet cash-out 0.22 s (`src/comet.rs:111`), weapon evolution 0.18 s (`src/ui/panels.rs:237`) | `src/fx.rs:27-44` |
| `playing` run condition | The four gameplay chains (sets D, E, F and G in 7.2), `gem_merge` and `tutorial_system` also require `RunPhase::Playing` | `src/main.rs:47-49,213,227,247,253,259,299` |
| `dt <= 0` guards | Almost every simulation system returns early when `time.delta_secs() <= 0`, so the "always in run" consumer set (section 7.2, set I) also freezes while paused | e.g. `src/player.rs:529-532` |

Systems that deliberately use `Time<Real>` and keep running while paused: `shake_decay`, `hitstop_system`, `death_watch`, `camera_rig`, `update_numbers`, `update_banners`, `update_dust_overlay`, `play_sfx`, `update_music`, `health_line` (`src/fx.rs:18,28`; `src/director.rs:257`; `src/player.rs:748`; `src/ui/numbers.rs:132`; `src/ui/hud.rs:533,600`; `src/audio.rs:163`; `src/music.rs:278`; `src/playlog.rs:89`). Everything else, including every `on_timer` run condition, uses virtual time.

**Co-op consequence.** The host's virtual clock drives the whole session: replicon's `ServerTick` runs in `FixedPostUpdate` by default (bevy_replicon 0.40.4 `ServerPlugin::default()`), which is driven by virtual time, and every host net timer (`on_timer`, `SnapClock`, the boss-lane accumulator, the id quarantine) is virtual too. A level-up panel, chest, shop, Moai, microwave, shrine blessing or Esc on the host freezes the world and every stream for all players. A client's own panel pauses only the client (its proxies stop easing; the host keeps simulating it, but its input is neutral while the panel is open, `src/net.rs:759-772`).

### 6.4 Run flows

**Start of a run (solo/host).** `planet_select_input` or `char_select_input` inserts a fresh `RunState` and sets `InRun` → `OnEnter(InRun)`: `enter_run`, `spawn_hud`, `start_music` → next `Update`, all chains run.

**Stage change (host/solo).** Boss killed → `apply_hits` sets `run.boss_dead` (`src/combat.rs:959-963`) → `run_clock` opens the teleporter 18 m from the party centroid (`src/director.rs:98-103`) → a player presses E at it → `interact_system` sets `PendingStage(Some(stage + 1))` (`src/interact.rs:545-549`) → `stage_transition` (`src/director.rs:147-232`): if the target is past the chain end, victory (below); otherwise it clones every astronaut's `PlayerState`, despawns every `StageScoped` entity, resets the stage fields of `RunState` (timer to `STAGE_SECONDS[stage]`, `elapsed` 0, boss/miniboss/static/teleporter/microwave flags), resets `Director`, reseeds `GameRng`, builds the new planet, respawns every astronaut **with its carried sheet** (same ids), spawns interactables from the local carried sheet, inserts the new `CurrentPlanet`, banners `STAGE N — NAME` and sets `Playing`.

**Victory.** `stage_transition` with `target >= chain.len()`: `result = Victory`, `counters.cleared` gains `(chain[0], tier)` plus `(planet, 1)` for every later planet in the chain, `NextState(Results)` (`src/director.rs:165-177`).

**Death.** `apply_player_hits` sets `dead` at HP ≤ 0 (`src/combat.rs:1019-1022`) → `downed_watch`: when every `Player`'s `PlayerState` is dead, `result = Death`, `RunPhase::Dead` (`src/director.rs:240-252`) → virtual time pauses → `death_watch` counts 1.6 real seconds → `Results`.

**Abandon.** Pause → ABANDON RUN sets `result = Death` and `RunPhase::Dead`, then the death path runs (`src/ui/panels.rs:577-582`). On a client nothing runs `death_watch`, so an abandoning joiner stays dead until the host's next stage change.

**Results.** `OnExit(InRun)` despawns the stage (astronauts included) → `OnEnter(Results)`: `bank_results` (host/solo) writes counters, silver, the daily best and quests into the save and builds `ResultsData`, then `spawn_results` draws it. Because the astronauts are already gone, `bank_results` cannot find the local `PlayerState` and falls back to level 1 and gold 0 (bug H1, `src/director.rs:280`).

---

## 7. Schedules and systems (every system)

### 7.1 Schedules used

| Schedule | Used for |
|---|---|
| `Startup` | Asset banks, the camera, the number pool, `boot`, `apply_cli_net` |
| `PreUpdate` | Client receivers of server messages, ordered `.after(ClientSystems::Receive)` so they see this frame's packets |
| `StateTransition` (Bevy-internal, between `PreUpdate` and `Update`) | `OnEnter`/`OnExit` hooks (section 6.1) |
| `Update` | Everything else in the game, including the whole simulation. There is no `FixedUpdate` game code |
| `PostUpdate` | `camera_rig`, ordered `.before(TransformSystems::Propagate)` |
| `FixedPostUpdate` | Not used by game code; replicon's `ServerTick` (replication send) runs here by default |

Separate `add_systems` calls are **unordered relative to each other** unless an explicit `.before`/`.after` or `.chain()` says otherwise. Bevy serializes systems with conflicting access, but in an ambiguous order.

### 7.2 Master system table

Role gates: **all** = no role gate; **sim** = `.run_if(net::is_simulating)`, runs in Solo and Host (`src/net.rs:460-462`); **host** = `is_hosting` (Host only, `src/net.rs:466-468`); **client** = `is_client` (`src/net.rs:463-465`); **net-host** = `is_simulating` and `is_networked` (Host only; Solo has no streams, `src/netenemy.rs:235-237`). "InRun & playing" means `in_state(AppState::InRun).and(playing)`.

**A. Startup** (`src/main.rs:119-132`, unordered tuple; plus `src/net.rs:392`)

| System | Defined | Gate | What it does |
|---|---|---|---|
| `setup_camera` | `src/main.rs:337` | all | Spawns the only camera: `Camera3d`, `Hdr`, `Bloom::NATURAL`, `Tonemapping::AcesFitted`, `PlayerRig` marker |
| `fx::setup_particles` | `src/fx.rs:98` | all | `ParticleAssets`: one 0.16 m cube mesh, 7 unlit emissive materials |
| `enemies::setup_enemy_assets` | `src/enemies.rs:366` | all | `EnemyAssets`: a composited mesh and a material per enemy kind, boss/worm/Anubot/beam/projectile/ring meshes and materials |
| `combat::setup_weapon_assets` | `src/combat.rs:30` | all | `WeaponAssets`: projectile/drone/beam/sweep/aura meshes, a material and an aura material per weapon kind |
| `pickups::setup_pickup_assets` | `src/pickups.rs:48` | all | `PickupAssets`: gem, coin, food, powerup meshes and materials |
| `audio::build_sfx_bank` | `src/audio.rs:79` | all | Synthesizes the 16 SFX into `Assets<AudioSource>` (`SfxBank`) |
| `music::build_music_bank` | `src/music.rs:246` | all | Synthesizes the 6 music stems (`MusicBank`) |
| `ui::numbers::spawn_number_pool` | `src/ui/numbers.rs:27` | all | 64 hidden damage-number text nodes, `NumberCursor` |
| `boot` | `src/main.rs:379` | all | Loads `MetaSave`, inserts a placeholder `RunState`, chooses `MainMenu` or `InRun` |
| `net::apply_cli_net` | `src/net.rs:1142` | all | Reads `--botinput`/`--netlog` into `NetDebug`; starts hosting or joining for `--host`/`--join` |

**B. Menu input** (`Update`)

| System | Defined | Condition | Gate | What it does |
|---|---|---|---|---|
| `client_follow_host_run` | `src/main.rs:354` | none (`src/main.rs:158`) | all (returns unless Client) | Pulls a seeded client from `MainMenu`/`Boot` into `InRun` |
| `main_menu_input` | `src/ui/menus.rs:216` | `MainMenu` (`src/main.rs:175`) | all | LAUNCH, DAILY, HOST/JOIN, TOMES, QUESTS, SETTINGS, QUIT; tome purchase and loadout toggles; side-panel rebuild |
| `join_panel_sync` | `src/ui/menus.rs:665` | `MainMenu` (`src/main.rs:176-180`) | all | Shows/hides the IP-entry overlay; mirrors `CoopNote` and `JoinAddr` into text |
| `join_addr_input` | `src/ui/menus.rs:770` | `MainMenu` | all | Digit/period/backspace typing (max 15 chars), Enter to join, Esc to cancel |
| `char_select_input` | `src/ui/menus.rs:474` | `CharSelect` (`src/main.rs:181`) | all | Hero pick (daily builds the run here), BACK |
| `planet_select_input` | `src/ui/menus.rs:580` | `PlanetSelect` (`src/main.rs:182`) | all | Tier pick builds a fresh `RunState`, BACK |
| `results_input` | `src/ui/menus.rs:648` | `Results` (`src/main.rs:183`) | all | Continue to the main menu |

**C. Dev systems** (`Update`)

| System | Defined | Condition | Gate | What it does |
|---|---|---|---|---|
| `dev_fast_boss` | `src/main.rs:504` | `InRun` (`src/main.rs:159`) | returns on Client | `--bossnow`: winds the clock once |
| `dev_stage_now` | `src/main.rs:484` | `InRun` and `--stagenow` in args (`src/main.rs:160-165`) | returns on Client | Requests stage 2 after 20 s of virtual time on stage 1 |
| `dev_autopick` | `src/main.rs:455` | `InRun` and `--autopick` in args (`src/main.rs:166-171`) | all | Auto-resolves card, chest and shop panels |

**D. Enemy chain** (`Update`, `.chain()`, InRun & playing; `src/main.rs:185-214`). Order is the listed order.

| # | System | Defined | Gate | What it does |
|---|---|---|---|---|
| D1 | `rebuild_hash` | `src/enemies.rs:473` | all | Rebuilds `SpatialHash` from every `Enemy` transform (pots and client proxies included) |
| D2 | `director_spawn` | `src/enemies.rs:539` | sim | Time-banked wave spawner (section 11.3) |
| D3 | `enemy_move` | `src/enemies.rs:1019` | sim | Great-circle steering, standoff/strafe for ranged kinds, separation, knockback, crowd animation. Must stay gated: proxies carry a real `Enemy` |
| D4 | `craterpillar_update` | `src/enemies.rs:941` | all | Records the worm head's trail and places its 12 segments; segment contact damage (inert on clients, nothing consumes it) |
| D5 | `anubot_beam_system` | `src/enemies.rs:798` | sim | Verdict Beam state machine, damage and visual |
| D6 | `boss_phase_system` | `src/enemies.rs:720` | sim | Enrage at 66%/33% HP plus an add ring |
| D7 | `burrower_emerge` | `src/enemies.rs:1196` | sim | Buried timer, eruption damage, eruption ring |
| D8 | `enemy_contact` | `src/enemies.rs:1237` | sim | Melee contact hits (0.5 s per enemy) |
| D9 | `spitter_attack` | `src/enemies.rs:1274` | sim | Spitter and UFO shots |
| D10 | `beamer_attack` | `src/enemies.rs:1327` | sim | Beamer aim line (latched target) and railbolt |
| D11 | `lobber_attack` | `src/enemies.rs:1446` | sim | Mortar: landing telegraph plus arcing shell |
| D12 | `mortar_shells` | `src/enemies.rs:1499` | all | Moves shells along their arc (also animates streamed hazards on clients) |
| D13 | `enemy_projectiles` | `src/enemies.rs:1522` | all | Moves enemy shots; first astronaut touched takes the hit (damage 0 on clients) |
| D14 | `boss_attacks` | `src/enemies.rs:1561` | sim | Boss slam ring every 6.5 s and a 12-shot radial burst every 9 s |
| D15 | `telegraphs` | `src/enemies.rs:1608` | all | Expands telegraph rings and detonates them (damage 0 on clients) |

**E. Player and weapon chain** (`Update`, `.chain()`, InRun & playing; `src/main.rs:215-228`). All ungated: on a client this is local prediction plus cosmetic weapon fire whose `HitMsg`s nothing reads.

| # | System | Defined | What it does |
|---|---|---|---|
| E1 | `gather_local_input` | `src/player.rs:382` | Keyboard to the `LocalPlayer`'s `InputIntent` (camera-relative WASD, Space, Left Ctrl or C, E) |
| E2 | `player_input` | `src/player.rs:418` | Intent to velocity for every `Player`: accel, friction, speed caps, jumps, bunny-hop, slide |
| E3 | `weapon_fire` | `src/combat.rs:212` | Cooldowns and firing for every living astronaut; reconciles drones and aura bubbles per (owner, weapon) |
| E4 | `projectile_move` | `src/combat.rs:580` | Moves player projectiles over the sphere; homing, boomerang return, collisions, rocket explosions |
| E5 | `drone_update` | `src/combat.rs:738` | Orbits drones around their owner and applies their hits |
| E6 | `beam_update` | `src/combat.rs:790` | Beam corridor ticks every 0.12 s |
| E7 | `aura_follow` | `src/combat.rs:841` | Moves and pulses aura bubbles |

**F. Interaction chain** (`Update`, `.chain()`, InRun & playing; `src/main.rs:229-248`)

| # | System | Defined | Gate | What it does |
|---|---|---|---|---|
| F1 | `charge_shrines` | `src/interact.rs:338` | sim | Fills charge rings by astronauts inside; blessing panel for the local player |
| F2 | `interact_system` | `src/interact.rs:394` | sim | Nearest interactable prompt and E handling, local player only; at Bevy's 16-parameter cap |
| F3 | `pickup_update` | `src/pickups.rs:110` | sim | Attraction, collection, shared XP, per-collector loot, `GrantOut` for the wire |
| F4 | `run_clock` | `src/director.rs:35` | sim | Stage clock, miniboss/boss marks, teleporter opening, THE STATIC |
| F5 | `levelup_trigger` | `src/director.rs:118` | all | Opens the local level-up panel when `pending_levelups > 0` |
| F6 | `debug_spawn_boss` | `src/enemies.rs:915` | all | **DEV key B**: summons the planet's stage boss. Marked "Remove before ship"; not gated |

**G. Comet and storm** (`Update`, `(comet_system, dust_storm_system).run_if(is_simulating).chain()`, InRun & playing; `src/main.rs:249-254`)

| # | System | Defined | Gate | What it does |
|---|---|---|---|---|
| G1 | `comet_system` | `src/comet.rs:33` | sim | Comet Combo for the local player only |
| G2 | `dust_storm_system` | `src/events_world.rs:32` | sim | Mars storm lifecycle; `player_inside` for the local player only |

**H. Gem merge** (`src/main.rs:255-262`)

| System | Defined | Condition | Gate | What it does |
|---|---|---|---|---|
| `gem_merge` | `src/pickups.rs:375` | InRun & playing & `on_timer(1 s)` | sim | Above 550 gems, merges the excess plus 40 into one big gem |

**I. Consumers and always-in-run systems** (`Update`, unchained, `in_state(InRun)` only, so they also run in panel phases, where their `dt` guards stop most work; `src/main.rs:264-288`)

| System | Defined | Gate | What it does |
|---|---|---|---|
| `apply_hits` | `src/combat.rs:877` | sim | Resolves `HitMsg` on pots and enemies: damage, flash, knockback, cryo slow, lifesteal, deaths (`KillMsg`), boss death flag |
| `apply_player_hits` | `src/combat.rs:973` | sim | Resolves `PlayerHitMsg`: i-frames, evasion, armor, shield, HP, thorns, downed flag |
| `kill_drops` | `src/pickups.rs:296` | sim | Reads `KillMsg`: kill and pot counters, loot rolls |
| `fader_update` | `src/combat.rs:857` | all | Shrinks and despawns `Fader` visuals (melee sweeps, chain zaps) |
| `enemy_flash` | `src/enemies.rs:1650` | all | Swaps an enemy to the flash material while `flash > 0` (needs `BaseMat`, so proxies never flash) |
| `player_physics` | `src/player.rs:523` | all | Gravity, sphere integration, prop collision, terrain contact, transform |
| `animate_player` | `src/player.rs:713` | all | Runs `animate_rig` for every `Player` |
| `player_upkeep` | `src/player.rs:842` | sim | Party difficulty, regen, i-frame/shield/powerup timers |
| `update_particles` | `src/fx.rs:164` | all | Particle motion and expiry |
| `stage_transition` | `src/director.rs:147` | all (only the host ever sets `PendingStage`) | Teleport to the next stage or victory |
| `downed_watch` | `src/director.rs:240` | sim | Run ends when every astronaut is down |
| `death_watch` | `src/director.rs:254` | sim | 1.6 real seconds after `Dead`, go to `Results` |

**J. In-run UI** (`Update`, unchained, `in_state(InRun)`; `src/main.rs:290-308`)

| System | Defined | Extra condition | What it does |
|---|---|---|---|
| `update_hud` | `src/ui/hud.rs:315` | | Timer, bonks, gold, silver, level, HP text and bar, XP bar, prompt, powerups, hurt vignette (ParamSet at the 8 cap) |
| `update_weapon_row` | `src/ui/hud.rs:385` | | Rebuilds the weapon/item tray when the weapon list changes |
| `update_boss_bar` | `src/ui/hud.rs:567` | | Shows the boss with the largest `max_hp` |
| `update_comet_hud` | `src/ui/hud.rs:547` | | Comet tail count and charge bar |
| `update_dust_overlay` | `src/ui/hud.rs:532` | | Fades the Mars haze in and out |
| `update_edge_markers` | `src/ui/hud.rs:446` | | Screen-edge squares for off-screen bosses, chests, Shady Guys, the cage, the teleporter and unfinished charge shrines |
| `tutorial_system` | `src/tutorial.rs:32` | `playing` | Mission Control lines; T replays them |
| `update_banners` | `src/ui/hud.rs:599` | | One banner at a time, 2.6 s each |
| `sync_choice_panel` | `src/ui/panels.rs:46` | | Builds the card panel when the phase or panel contents change |
| `choice_input` | `src/ui/panels.rs:139` | | Card picks (1-4 or click), R refresh, B banish, S skip |
| `chest_panel` | `src/ui/panels.rs:260` | | Chest reveal: TAKE (1 or E) / LEAVE (2 or Esc) |
| `shop_panel` | `src/ui/panels.rs:371` | | Shady Guy: buy with 1-3 or click, E or Esc to walk away |
| `pause_panel` | `src/ui/panels.rs:501` | | Esc pause, RESUME, SETTINGS, ABANDON RUN |

**K. Damage numbers** (`Update`, `(claim_numbers, update_numbers).chain()`, no state condition; `src/main.rs:309-312`)

| System | Defined | What it does |
|---|---|---|
| `claim_numbers` | `src/ui/numbers.rs:82` | Reads `NumberMsg`; merges into a live number within 1.6 m or claims the next pool slot |
| `update_numbers` | `src/ui/numbers.rs:131` | Projects numbers to the viewport, rises and fades them |

**L. Camera** (`PostUpdate`; `src/main.rs:313-318`)

| System | Defined | Condition | Ordering | What it does |
|---|---|---|---|---|
| `camera_rig` | `src/player.rs:747` | `InRun` | `.before(TransformSystems::Propagate)` | Mouse orbit chase camera, FOV punch, screenshake |

**M. Global** (`Update`, unchained, every state; `src/main.rs:320-333`)

| System | Defined | Condition / ordering | What it does |
|---|---|---|---|
| `cursor_control` | `src/player.rs:827` | | Locks and hides the cursor only in `InRun` + `Playing` |
| `shake_decay` | `src/fx.rs:18` | | Trauma decays 1.6 per real second |
| `hitstop_system` | `src/fx.rs:27` | | Hitstop slow-motion |
| `phase_time_control` | `src/fx.rs:47` | | Pauses/unpauses `Time<Virtual>` on `RunPhase` change |
| `play_sfx` | `src/audio.rs:161` | | Plays `SfxMsg` with per-sound throttling |
| `update_music` | `src/music.rs:277` | `in_state(InRun)` | Drives stem volumes and speed |
| `settings_panel` | `src/ui/settings.rs:85` | `.after(pause_panel)` | Settings overlay lifecycle and steppers |
| `button_hover` | `src/ui/mod.rs:55` | | Hover tint for every `Button` |

**N. NetPlugin** (`src/net.rs:343-457`)

In this table and the next two, a bare `:line` refers to the plugin's own file.

| System | Defined | Schedule | Condition | Gate | Ordering | What it does |
|---|---|---|---|---|---|---|
| `apply_cli_net` | `:1142` | `Startup` | | all | | See A |
| `send_player_build` | `:601` | `Update` | `on_timer(500 ms)` (virtual) | client | | Sends `PlayerBuildMsg` |
| `apply_player_build` | `:619` | `Update` | | host | | Applies peers' builds |
| `adopt_my_vitals` | `:583` | `Update` | | client | | Copies own hp and down flag from the host's copy |
| `relay_grants` | `:660` | `Update` | | host | | `GrantOut` to `XpGrantMsg`/`LootGrantMsg` |
| `apply_xp_grant` | `:697` | `PreUpdate` | | client | after `ClientSystems::Receive` | Shared XP onto the local sheet |
| `apply_loot_grant` | `:710` | `PreUpdate` | | client | after `ClientSystems::Receive` | Gold, food, powerups onto the local sheet |
| `push_run_snapshot` | `:496` | `Update` | `on_timer(250 ms)` | host | | Sends `RunSnapMsg` (no state gate: also sends from the menus) |
| `apply_run_snapshot` | `:521` | `PreUpdate` | | client | after `ClientSystems::Receive` | Adopts the host's run; raises `pending_stage` |
| `announce_player_ids` | `:478` | `Update` | `on_timer(500 ms)` | host | | Sends `AssignPlayerId` to each authorized client |
| `receive_player_id` | `:736` | `PreUpdate` | | client | after `ClientSystems::Receive` | Latches `MyPlayerId` |
| `report_connection` | `:1020` | `Update` | | all | | Logs client-state and peer-count changes |
| `bot_input` | `:790` | `Update` | `NetDebug.bot` | all | after `gather_local_input`, before `player_input` | `--botinput` circle-strafe |
| `log_astronauts` | `:800` | `Update` | `NetDebug.log` | all | | `--netlog` pose, checksum and invariant dump |
| `netenemy::log_stream_stats` | `src/netenemy.rs:1269` | `Update` | `NetDebug.log` | all | | `--netlog` stream statistics |
| `send_local_input` | `:749` | `Update` | | client | after `gather_local_input` and `bot_input`, before `player_input` | Sends `PlayerInputMsg` every frame |
| `seat_joining_players` | `:883` | `Update` | | host | chain 1 of 3, before `player_input` | Gives each connected client a slot and a body |
| `unseat_leaving_players` | `:954` | `Update` | | host | chain 2 of 3 | Frees slots and bodies of departed clients |
| `apply_remote_input` | `:981` | `Update` | | host | chain 3 of 3 | Routes `PlayerInputMsg` onto the peer's `InputIntent` |
| `push_net_transform` | `:1178` | `Update` | | sim | | `Player` to `NetTransform` for every astronaut |
| `push_player_vitals` | `:1188` | `Update` | | sim | | `PlayerState` to `PlayerVitals` for every astronaut |

**O. EnemyStreamPlugin** (`src/netenemy.rs:152-221`)

| System | Defined | Schedule | Gate | Ordering | What it does |
|---|---|---|---|---|---|
| `assign_net_ids` | `:240` | `Update` | net-host | | `NetId` for every `Enemy` (pots and bosses included); quarantined recycling |
| `stream_pickups` | `:602` | `Update` | net-host | | Pickup spawn/despawn events |
| `stream_hazards` | `:480` | `Update` | net-host | | `Added<EnemyProjectile/Telegraph/MortarShell>` to hazard events |
| `stream_bosses` | `:862` | `Update` | net-host | | 20 Hz boss list |
| `stream_enemies` | `:280` | `Update` | net-host | `.after(assign_net_ids)` | 15 Hz per-client crowd snapshot |
| `client_stage_transition` | `:775` | `Update` | client | client chain 1 of 8 | Rebuilds the world on a host stage change |
| `receive_bosses` | `:891` | `Update` | client | chain 2 | Boss proxies |
| `receive_pickups` | `:657` | `Update` | client | chain 3 | Loot proxies |
| `receive_hazards` | `:519` | `Update` | client | chain 4 | Hazard visuals (damage 0) |
| `receive_enemies` | `:1070` | `Update` | client | chain 5 | Crowd proxies |
| `drive_boss_proxies` | `:1002` | `Update` | client | chain 6 | Eases boss proxies |
| `drive_proxies` | `:1206` | `Update` | client | chain 7 | Eases, animates and reaps crowd proxies |
| `animate_net_pickups` | `:722` | `Update` | client | chain 8 | Bobs streamed loot and flies it to nearby astronauts (cosmetic) |
| `clear_stream_indices` | `:225` | `OnExit(InRun)` | all | | Clears the three id-to-entity maps |

The client chain has no `AppState` condition; its members tolerate a missing `CurrentPlanet` with `Option<Res<_>>`.

**P. RemoteVisualsPlugin** (`src/remote.rs:45-57`)

| System | Defined | Schedule | Condition | Gate | Ordering |
|---|---|---|---|---|---|
| `spawn_remote_rigs` | `:63` | `Update` | `InRun` | client | chain 1 of 3 |
| `drive_remote_transforms` | `:117` | `Update` | `InRun` | client | chain 2 |
| `animate_remote_rigs` | `:172` | `Update` | `InRun` | client | chain 3 |
| `despawn_remote_rigs` | `:199` | `OnExit(InRun)` | | all | |

**Q. PlayLogPlugin**

| System | Defined | Schedule | Gate | What it does |
|---|---|---|---|---|
| `health_line` | `src/playlog.rs:88` | `Update` | all | Writes a health line every 5 real seconds |

### 7.3 Explicit ordering constraints (complete list)

| Constraint | Where | Why |
|---|---|---|
| Chains D, E, F, G, K | `src/main.rs:212,226,246,252,311` | Intra-frame data flow (hash before steering, input before fire, and so on) |
| `bot_input` after `gather_local_input`, before `player_input` | `src/net.rs:421-427` | The bot must overwrite the keyboard intent before movement |
| `send_local_input` after `gather_local_input` and `bot_input`, before `player_input` | `src/net.rs:433-443` | It must ship the final intent. Anything new that writes `InputIntent` must be `.before(send_local_input)` |
| `seat → unseat → apply_remote_input` chain, before `player_input` | `src/net.rs:446-452` | Remote intent lands the same frame it arrives |
| `stream_enemies` after `assign_net_ids` | `src/netenemy.rs:189-195` | Fresh ids first |
| Client chain `client_stage_transition` first | `src/netenemy.rs:196-214` | Every decode uses the new planet's radius |
| Client receivers after `ClientSystems::Receive` | `src/net.rs:371-376,400-405,412-417` | Read this frame's packets |
| `settings_panel` after `pause_panel` | `src/main.rs:330` | One Esc closes settings without also resuming |
| `camera_rig` before `TransformSystems::Propagate` | `src/main.rs:313-318` | Camera transform propagates the same frame |
| `bank_results` before `spawn_results` | `src/main.rs:145-155` | Results screen reads the fresh `ResultsData` |

### 7.4 Notable unordered pairs

| Pair | Consequence |
|---|---|
| `player_input` (E2) and `player_physics` (set I) | No explicit order; the velocity applied can lag one frame |
| Enemy chain D and weapon chain E | `rebuild_hash` may run after `projectile_move` reads the hash; a one-frame staleness |
| E/Esc handlers: `interact_system` (F2), `chest_panel`, `shop_panel`, `pause_panel` (J), `settings_panel` (M) | One key press can be handled by two systems in the same frame |
| `levelup_trigger` (F5) and `choice_input` (J) | Harmless; the panel appears one frame later at worst |
| `stream_*` (O) and the simulation | `Added<>` filters catch anything spawned since the last run, so ordering does not matter |
| `HitMsg`/`PlayerHitMsg` writers (D, E, G) and readers (I) | Messages are double-buffered, so a message written after its reader ran is read the next frame |

---

## 8. ECS data model (every component and resource)

### 8.1 Entity archetypes

| Entity | Components | Spawned by |
|---|---|---|
| Local astronaut | `Player`, `PlayerState`, `PlayerId`, `InputIntent`, `NetTransform`, `PlayerVitals`, `Replicated`, `Transform`, `Visibility`, `StageScoped`, `LocalPlayer`; rig children (section 16.6) | `player::spawn_player` `src/player.rs:117-175` |
| Peer astronaut on the host | The same without `LocalPlayer`; driven by network `InputIntent` | `seat_joining_players` `src/net.rs:883-951`; carried across stages by `stage_transition` |
| Replicated astronaut on a client | `PlayerId`, `NetTransform`, `PlayerVitals` (replicon-owned). If its id is not `MyPlayerId`, the client adds `RemoteAstronaut`, `Transform`, `Visibility` and rig children. **Never** `Player`, `PlayerState` or `StageScoped` | replicon; `spawn_remote_rigs` `src/remote.rs:63-112` |
| Enemy (host/solo) | `Enemy`, `Mesh3d`, `MeshMaterial3d`, `BaseMat`, `Transform`, `StageScoped`; `Spitter` (Spitter, UFO), `Buried` (Burrower), `Beamer`, `Lobber` by kind; `NetId` when hosting | `spawn_enemy` `src/enemies.rs:483-536` |
| Boss or miniboss (host/solo) | `Enemy{kind: Bruiser, elite: true, xp: 50}`, `Boss`, mesh, material, `BaseMat`, `Transform`, `StageScoped`; plus `CraterpillarHead` (Craterpillar) or `AnubotBeam` (Anubot); `NetId` when hosting | `spawn_boss` `src/enemies.rs:628-714` |
| Craterpillar segment | `CraterpillarSegment`, mesh, material, `Transform`, `StageScoped` (no `Enemy`, not in the hash) | `src/enemies.rs:691-700`; client copy `src/netenemy.rs:967-986` |
| Anubot beam visual | `AnubotBeamVis`, mesh, material, `Transform`, `Visibility::Hidden`, `StageScoped` | `src/enemies.rs:703-713` |
| Pot | `Pot`, `Enemy{speed 0, contact_cd INF, hp 1}`, mesh, material, `Transform`, `StageScoped`; `NetId` when hosting (never streamed) | `src/interact.rs:161-193` |
| Crowd proxy (client) | `Enemy{damage 0, xp 0, hp 1, max_hp 1}`, `NetEnemy`, `NetId`, mesh, material, `Transform`, `StageScoped` (no `BaseMat`) | `receive_enemies` `src/netenemy.rs:1132-1160` |
| Boss proxy (client) | `Enemy{kind: Bruiser, damage 0, hp = fraction, max_hp 1}`, `Boss{timers = INFINITY}`, `NetBoss`, `NetId`, mesh, material, `Transform`, `StageScoped`; Craterpillar also gets `CraterpillarHead` and 12 segments | `receive_bosses` `src/netenemy.rs:931-986` |
| Enemy hazard | `EnemyProjectile`, `Telegraph` or `MortarShell`, mesh, material, `Transform`, `StageScoped`; on a client also `NetHazard` with damage 0 | `src/enemies.rs:1223-1231,1314-1320,1386-1400,1478-1493,1578-1601`; `src/netenemy.rs:536-577` |
| Pickup | `Pickup`, mesh, material, `Transform`, `StageScoped`; `PickupNetId` when hosting and on clients | `spawn_pickup` `src/pickups.rs:73-106`; `receive_pickups` `src/netenemy.rs:691-701` |
| Interactable | `Interactable`, pedestal mesh/material, `Transform`, `StageScoped`, one child shape | `src/interact.rs:198-233,299-335` |
| Charge shrine | `ChargeShrine`, `ShrineRing`, torus mesh, material, `Transform`, `StageScoped` | `src/interact.rs:283-295` |
| Player weapon entities | `Projectile`, `Drone`, `Beam` or `AuraVis`, plus `Fader` visuals; all `StageScoped` | `weapon_fire` `src/combat.rs:212-577` |
| Particle | `Particle`, mesh, material, `Transform`, `StageScoped` | `fx::burst` `src/fx.rs:132-162` |
| Scenery | Terrain, rocks, boulders, crystals, wrecks, beacons, flora, 420 stars, Earth, sun light and disc; all `StageScoped` | `spawn_stage` `src/planet.rs:143-503` |
| Storm dome | `DustStormVis`, sphere mesh, material, `Transform`, `Visibility`, `StageScoped` | `src/events_world.rs:62-82` |
| Camera | `Camera3d`, `Hdr`, `Bloom`, `Tonemapping`, `Transform`, `PlayerRig`; lives for the whole process | `src/main.rs:337-346` |
| Music stems | `AudioPlayer`, `PlaybackSettings{Loop}`, `MusicStem`; spawned on entering a run, despawned on leaving | `src/music.rs:257-275` |
| SFX | `AudioPlayer`, `PlaybackSettings{Despawn}` | `src/audio.rs:187-194` |
| UI trees | HUD, menus, panels, overlays, 64 damage numbers | Section 18 |

### 8.2 Gameplay components

| Component | Fields (type) and meaning | Defined |
|---|---|---|
| `Player` | `dir: Vec3` unit direction from the planet core (the position); `height: f32` above terrain; `vel_t: Vec3` world-space tangent velocity; `vel_r: f32` radial velocity; `grounded: bool`; `facing: Vec3` tangent unit vector; `jumps_used: i32`; `slide_timer`, `slide_cd`: slide state; `land_timer` time since landing (bunny-hop window); `coyote` (effectively dead: set only while grounded); animation state `stride`, `gait_amp`, `squash`, `squash_amt`, `lean` | `src/player.rs:14-33` |
| `PlayerId(u8)` | Owner slot. 0 = the host's astronaut **and** every client's own predicted astronaut; 1-3 = peers on the host. Replicated | `src/player.rs:37-38` |
| `LocalPlayer` | Marker: the one astronaut this machine drives | `src/player.rs:41-42` |
| `InputIntent` | `wish: Vec3` normalized tangent move direction; `forward: Vec3` camera forward (tangent); `jump`, `slide`, `interact: bool`. `player_input` reads only `wish`, `jump`, `slide`; nothing reads `forward` or `interact` | `src/player.rs:70-77` |
| `PlayerRig` | Marker on the camera | `src/player.rs:79-80` |
| `Joint` | `limb: Limb` (ArmL, ArmR, LegL, LegR, Head, Body); `rest: Transform` pose composed from every frame; `lag: f32` never read | `src/player.rs:83-101` |
| `PlayerState` | Per-astronaut sheet: `character: AstronautKind`; `xp`, `level`, `xp_needed`, `pending_levelups`; `gold: u64`; `weapons: Vec<WeaponInstance{kind, level, cd}>`; `items: Vec<(ItemKind, u32)>`; `stats: Stats`; `hp`, `shield`, `shield_cd`, `iframes`; `banishes` (3), `refreshes` (2, Fortuna +2); `banned_items: HashSet<ItemKind>`; `frenzy_timer` (Yuki), `fast_move` (above 1.08 x run speed), `reticle_timer` (0-1.5 s cycle); `powerups: Vec<(PowerupKind, secs)>`; `dead: bool` | `src/run.rs:138-161`; `new` at `src/run.rs:202-234` |
| `StageScoped` | Marker: despawned by `despawn_stage` and stage transitions | `src/planet.rs:13-14` |
| `Enemy` | `kind: EnemyKind`; `dir: Vec3` position; `hover` height; `speed`; `damage` (contact); `xp` drop; `hp`, `max_hp`; `elite`; `contact_cd` (0.5 s between contact hits; INF on pots); `slow` 0-0.9; `knock: Vec3` decaying tangent impulse; `flash` hit-flash 0-1; `scale`; `wobble` per-enemy phase; `stride` gait phase | `src/enemies.rs:17-37` |
| `Boss` | `kind: BossKind`; `attack_timer` (slam), `burst_timer` (radial shots); `phase: u8` 0/1/2 at >66% / 33-66% / <33% HP | `src/enemies.rs:39-45` |
| `CraterpillarHead` | `trail: VecDeque<Vec3>` of head directions, newest first, one point per 0.55 m, capped at 28 | `src/enemies.rs:49-52,63-65,963` |
| `CraterpillarSegment` | `head: Entity`, `idx: usize` (0-11), `damage` (70% of head contact), `scale` (tapering) | `src/enemies.rs:55-61` |
| `AnubotBeam` | `angle` sweep angle; `state: u8` 0 idle, 1 charging, 2 firing; `timer` in the current state (starts idle 2.5 s) | `src/enemies.rs:68-79` |
| `AnubotBeamVis` | `boss: Entity` | `src/enemies.rs:81-84` |
| `Spitter` | `cd` shot cooldown (Spitter and UFO) | `src/enemies.rs:89-92` |
| `Beamer` | `cd`; `charging` (>0 while painting the aim line); `aim: Vec3` locked heading; `target: Option<Entity>` latched for the whole telegraph | `src/enemies.rs:95-104` |
| `AimLine` | `owner: Entity` (the Beamer) | `src/enemies.rs:107-110` |
| `Lobber` | `cd` | `src/enemies.rs:113-116` |
| `MortarShell` | `from`, `to` directions; `t` progress 0-1; `dur` flight time. Visual only; the telegraph deals the damage | `src/enemies.rs:119-125` |
| `Buried` | `timer` until eruption (1.3 s) | `src/enemies.rs:127-130` |
| `EnemyProjectile` | `dir`, `heading`, `speed`, `damage`, `life`, `hover` | `src/enemies.rs:132-140` |
| `Telegraph` | `timer`, `max`, `radius`, `damage`, `dir`, `ring` (true = only the edge band hurts) | `src/enemies.rs:145-153` |
| `BaseMat(Handle<StandardMaterial>)` | Material to restore after a hit-flash | `src/enemies.rs:177-178` |
| `Projectile` | `owner: Entity` (shooter); `dir`, `heading`; `speed`; `damage`; `pierce: i32`; `life`; `size`; `kind: ProjKind` (Straight, Seek, Boomerang{age, out_time}, Rocket{aoe}); `hit_cd: HashMap<Entity, f32>` per-target re-hit cooldown (0.5 s) | `src/combat.rs:107-130` |
| `Drone` | `owner`, `weapon`, `idx`, `count`, `damage`, `radius`, `deg_per_sec`, `tick` (0.3 s after a hit) | `src/combat.rs:132-145` |
| `Beam` | `owner`, `heading`, `range`, `width`, `damage`, `ticks_left`, `tick_cd` (0.12 s) | `src/combat.rs:147-159` |
| `Fader` | `life`, `max` | `src/combat.rs:161-165` |
| `AuraVis` | `owner`, `weapon` | `src/combat.rs:167-174` |
| `Pickup` | `kind: PickupKind` (Xp(f32), Gold(u64), Silver(u64), Food, Powerup(PowerupKind)); `dir`; `flying`; `speed`; `bob` phase; `target: Option<Entity>` latched collector | `src/pickups.rs:13-32` |
| `Pot` | `broken: bool` | `src/interact.rs:20-23` |
| `Interactable` | `kind: InteractKind` (Chest, ShadyGuy, GreedShrine, MagnetShrine, Moai, Microwave, Cage, Teleporter); `used`; `chest_item: Option<ItemKind>` (rolled on first open); `stock: Vec<(ItemKind, price, sold)>` (Shady Guy) | `src/interact.rs:25-44` |
| `ChargeShrine` | `progress` 0-1; `done` | `src/interact.rs:46-50` |
| `ShrineRing` | Marker (never queried) | `src/interact.rs:52-53` |
| `DustStormVis` | Marker on the storm dome | `src/events_world.rs:24-25` |
| `Particle` | `vel`, `life`, `max_life`, `gravity` | `src/fx.rs:90-96` |
| `MusicStem` | `stem: Stem`; `cur` smoothed volume | `src/music.rs:30-34` |
| `TutorialText` | Marker on the HUD tutorial line | `src/tutorial.rs:29-30` |

### 8.3 Networking components

| Component | Fields and meaning | Defined |
|---|---|---|
| `NetTransform` | Replicated pose: `dir: Vec3`, `height: f32`, `facing: Vec3`. Written every frame by `push_net_transform` | `src/net.rs:63-68` |
| `PlayerVitals` | Replicated: `hp`, `max_hp`, `level: u32`, `down: bool`. Written every frame by `push_player_vitals` | `src/net.rs:71-77` |
| `Replicated` (replicon) | Marks astronauts for replication | added at `src/player.rs:167` |
| `NetId(u16)` | Enemy wire id: 15-bit id (`ID_MASK 0x7FFF`) plus a flag bit (`0x8000`) used only on the wire | `src/netenemy.rs:50-54` |
| `NetEnemy` | Client crowd proxy: `target` last streamed direction, `shown` smoothed direction, `speed` derived gait speed, `last_seen` (virtual seconds) | `src/netenemy.rs:113-121` |
| `NetBoss` | Client boss proxy: `target`, `shown`, `last_seen` | `src/netenemy.rs:129-134` |
| `NetHazard` | Marker on client-built hazard visuals | `src/netenemy.rs:472-473` |
| `PickupNetId(u16)` | Pickup wire id | `src/netenemy.rs:585-586` |
| `RemoteAstronaut` | Client teammate rig state: smoothed `dir`, `height`, `facing`; derived `speed`, `vel_r`, `grounded`; `anim: RigAnim` | `src/remote.rs:30-41` |

### 8.4 UI components (markers unless noted)

| Module | Components |
|---|---|
| `src/ui/hud.rs:12-52` | `HudRoot`, `TimerText`, `KillsText`, `GoldText`, `SilverText`, `LevelText`, `XpFill`, `HpFill`, `HpText`, `WeaponRow`, `BannerText`, `PromptText`, `BossBarWrap`, `BossBarFill`, `BossBarName`, `PowerupText`, `Vignette`, `CometText`, `DustOverlay`, `EdgeMarker` |
| `src/ui/menus.rs:31-115` | `MenuRoot`, `LaunchBtn`, `TomesBtn`, `QuestsBtn`, `QuitBtn`, `SettingsBtn`, `DailyBtn`, `MenuBtn` (enum: Launch, Daily, Tomes, Quests, Settings, Quit, HostCoop, JoinCoop, JoinConfirm, JoinCancel), `CoopNoteText`, `JoinPanel`, `JoinAddrText`, `SidePanel`, `TomePlus(TomeKind)`, `TomeToggle(TomeKind)`, `CharCard(AstronautKind)`, `PlanetCard(PlanetKind, u32)`, `BackBtn`, `ContinueBtn` |
| `src/ui/panels.rs:11-43` | `ChoiceRoot`, `ChoiceCard(usize)`, `RefreshBtn`, `BanishBtn`, `SkipBtn`, `ChestRoot`, `ChestTake`, `ChestLeave`, `ShopRoot`, `ShopBuy(usize)`, `ShopClose`, `PauseRoot`, `ResumeBtn`, `AbandonBtn`, `PauseSettingsBtn` |
| `src/ui/numbers.rs:13-22` | `DamageNumber{active, world, life, max_life, value, crit, kind}` |
| `src/ui/settings.rs:68-76` | `SettingsRoot`, `SettingsStep{setting, dir}`, `SettingsClose` |

### 8.5 Resources

| Resource | Fields and meaning | Defined | Inserted |
|---|---|---|---|
| `State<AppState>` / `NextState<AppState>` | App state | `src/main.rs:35-44` | `init_state` `src/main.rs:90` |
| `RunPhase` | Playing, LevelUp, Modal, Paused, Dead | `src/run.rs:68-77` | `src/main.rs:91` |
| `RunState` | Run-global state, table 8.6 | `src/run.rs:101-133` | `boot`, menus |
| `ChoicePanel` | `title`, `options: Vec<UpgradeOption>`, `banishing`, `is_levelup` (false for shrine, Moai, microwave) | `src/run.rs:632-640` | `src/main.rs:92` |
| `GameRng(StdRng)` | Seeded spawn stream, reseeded per stage | `src/run.rs:22-35` | `src/main.rs:104` |
| `CamRig` | `forward: Vec3` persistent tangent forward (default `-Z`); `pitch` (default 0.55 rad) | `src/player.rs:103-115` | `src/main.rs:93` |
| `Shake` | `trauma` 0-1 | `src/fx.rs:7-16` | `src/main.rs:94` |
| `Hitstop` | `timer` (real seconds) | `src/fx.rs:22-25` | `src/main.rs:95` |
| `ParticleAssets` | `mesh`, `mats: Vec<(Pcolor, Handle)>` | `src/fx.rs:74-88` | `setup_particles` |
| `SpatialHash` | `map: HashMap<IVec3, Vec<(Entity, Vec3)>>` of enemies in 2.2 m world cells | `src/enemies.rs:180-205` | `src/main.rs:96` |
| `Director` | `spawn_bank` fractional spawns owed; `elite_timer` (45 s, then 40 s); `tick` batch timer | `src/enemies.rs:207-218` | `src/main.rs:97`, reset per stage |
| `EnemyAssets` | Per-kind `meshes` and `mats` maps; `elite_mat`, `flash_mat`, `boss_mat`; worm head/segment meshes and `worm_mat`; `anubot_mesh`/`anubot_mat`; `beam_mesh`, `beam_charge_mat`, `beam_fire_mat`; `proj_mesh`, `proj_mat`; `ring_mesh`, `ring_mat` | `src/enemies.rs:155-174` | `setup_enemy_assets` |
| `WeaponAssets` | `proj_mesh`, `drone_mesh`, `beam_mesh`, `sweep_mesh`, `aura_mesh`; `mats` and `aura_mats` per `WeaponKind` | `src/combat.rs:18-28` | `setup_weapon_assets` |
| `PickupAssets` | Gem, coin, food and powerup meshes; `gem_mat`, `big_gem_mat` (value >= 10), `coin_mat`, `silver_mat`, `food_mat`, `power_mat` | `src/pickups.rs:34-46` | `setup_pickup_assets` |
| `PendingStage(Option<usize>)` | Requested stage index; `>= chain.len()` means victory | `src/director.rs:17-19` | `src/main.rs:98` |
| `ResultsData` | `victory`, `kills`, `level`, `gold`, `silver_earned`, `time`, `quests_completed: Vec<String>`, `daily: Option<(name, best, new_best)>` | `src/director.rs:21-32` | `src/main.rs:99`, replaced by `bank_results` |
| `InteractPrompt(Option<String>)` | HUD prompt text | `src/interact.rs:56-57` | `src/main.rs:100` |
| `ChestPanel` | `open`, `item`, `cost`, `chest: Option<Entity>` | `src/interact.rs:60-66` | `src/main.rs:101` |
| `ShopPanel` | `open`, `vendor: Option<Entity>`, `offers` | `src/interact.rs:68-74` | `src/main.rs:102` |
| `Comet` | `active`, `count` tail size, `peak`, `charge`, `break_timer`, `flash`, `fires` (lifetime cash-outs) | `src/comet.rs:15-30` | `src/main.rs:103`, reset in `enter_run` |
| `PropColliders(Vec<PropCollider{dir, radius, height}>)` | Solid props of the current stage | `src/planet.rs:19-56` | `src/main.rs:105`, replaced per stage |
| `CurrentPlanet` | `kind`, `radius`, `terrain: Terrain` | `src/planet.rs:59-89` | first by `enter_run`; never removed |
| `Tutorial` | `active`, `step` 0-6, `timer` | `src/tutorial.rs:10-15` | `src/main.rs:106` |
| `DustStorm` | `active`, `dir`, `radius` (24 m), `heading`, `timer`, `player_inside` (local player only), `spawned_vis`. Never reset | `src/events_world.rs:13-22` | `src/main.rs:107` |
| `SettingsOpen(bool)` | Settings overlay visible | `src/ui/settings.rs:9-10` | `src/main.rs:108` |
| `Selected` | Menu choice: `character`, `planet`, `tier`, `daily` | `src/ui/menus.rs:17-29` | `src/main.rs:109` |
| `MenuTab` | None, Tomes, Quests | `src/ui/menus.rs:100-106` | `src/main.rs:110` |
| `BannerQueue` | `current: Option<(String, secs)>`, `queue: Vec<String>` | `src/ui/hud.rs:54-58` | `src/main.rs:111` |
| `JoinAddr(String)` | IP being typed (default `127.0.0.1`) | `src/ui/menus.rs:85-92` | `src/main.rs:172` |
| `JoinOpen(bool)` | IP-entry overlay visible | `src/ui/menus.rs:66-67` | `src/main.rs:173` |
| `CoopNote(String)` | Co-op status line | `src/ui/menus.rs:71-72` | `src/main.rs:174` |
| `NumberCursor(usize)` | Round-robin pool cursor | `src/ui/numbers.rs:24-25` | `spawn_number_pool` |
| `SfxBank` | `map: HashMap<Sfx, Handle<AudioSource>>` | `src/audio.rs:74-77` | `build_sfx_bank` |
| `SfxThrottle` | `last: HashMap<Sfx, f32>` real time of last play | `src/audio.rs:156-159` | `src/main.rs:112` |
| `MusicBank` | `stems: Vec<(Stem, Handle<AudioSource>)>` | `src/music.rs:36-39` | `build_music_bank` |
| `MetaSave` | The save file, section 13 | `src/save.rs:31-53` | `boot` |
| `GlobalAmbientLight` | Color (0.65, 0.7, 0.9), brightness 80 | Bevy | `src/main.rs:81-85` |
| `NetRole` | Solo (default), Host, Client. `simulates()` = not Client; `is_networked()` = not Solo | `src/net.rs:42-58` | `NetPlugin`; replaced by `start_host`/`start_join` |
| `RunSync` | Client: `seeded` (first snapshot arrived), `world_built` (set by `enter_run`), `pending_stage: Option<usize>` | `src/net.rs:123-131` | `src/net.rs:368` |
| `MyPlayerId(Option<u8>)` | Client: the id the host assigned | `src/net.rs:288-289` | `src/net.rs:391` |
| `PeerSlots` | Host: `map: HashMap<client Entity, u8>` | `src/net.rs:306-330` | `src/net.rs:393` |
| `NetDebug` | `bot` (`--botinput`), `log` (`--netlog`) | `src/net.rs:335-339` | `src/net.rs:420`, overwritten by `apply_cli_net` |
| `RenetServer`, `NetcodeServerTransport` / `RenetClient`, `NetcodeClientTransport` | Transport, present only while hosting / joining | renet | `start_host` / `start_join` |
| `RepliconChannels`, `State<ClientState>`, `State<ServerState>` | replicon | replicon | `RepliconPlugins` |
| `NetEnemyIds` | Host id allocator: `next`, `free`, `quarantine: VecDeque<(due, id)>` | `src/netenemy.rs:59-96` | `src/netenemy.rs:154` |
| `ClientResidency` | Host: per client entity, the set of enemy ids that client is believed to have | `src/netenemy.rs:100-101` | `src/netenemy.rs:155` |
| `SnapClock` | Host: `acc` time accumulator, `seq: u16` snapshot sequence | `src/netenemy.rs:104-108` | `src/netenemy.rs:156` |
| `NetEnemyIndex`, `NetBossIndex`, `NetPickupIndex` | Client: wire id to proxy entity | `src/netenemy.rs:123-124,136-137,593-594` | `src/netenemy.rs:157-160` |
| `PickupIds` | Host: `next: u16` (wrapping, never 0) | `src/netenemy.rs:588-591` | `src/netenemy.rs:159` |
| `NetEnemyStats` | Client receive counters: `records`, `chunks`, `bytes`, `seq_gaps`, `last_seq`, `next_print` | `src/netenemy.rs:140-148` | `src/netenemy.rs:161` |
| `PlayLog` | `path` of the session file; `next` health-line time | `src/playlog.rs:25-29` | `PlayLogPlugin::build` |

### 8.6 `RunState` fields

| Field | Meaning | Written by | In `RunSnapMsg` |
|---|---|---|---|
| `tier: u32` | Chain length chosen (1-3) | `RunState::new` | no |
| `chain: Vec<PlanetKind>` | Stage planets (`chain_from`) | `new`; client adopts | yes (codes) |
| `stage: usize` | Current stage index | `stage_transition`; client adopts | yes (`u8`) |
| `timer` | Stage countdown (600/540/480 s) | `run_clock` | yes |
| `elapsed` | Seconds into this stage (drives the spawn mix and scaling) | `run_clock` | yes |
| `total_elapsed` | Seconds into the run | `run_clock` | yes |
| `silver_run` | Silver earned this run | pickups, comet | yes |
| `kills` | Kills this run (pots excluded) | `kill_drops` | yes |
| `greed_stacks` | Greed shrines used (+12% difficulty, +8% luck each) | `interact_system` | **no** |
| `boss_spawned`, `boss_dead` | Stage boss flags | `run_clock`, `apply_hits`, DEV B | yes |
| `minibosses_spawned: [bool; 2]` | Miniboss marks consumed | `run_clock`, `--bossnow` | no |
| `static_active`, `static_timer` | THE STATIC and its duration | `run_clock` | yes |
| `teleporter_open` | Teleporter spawned | `run_clock` | yes |
| `run_seed: u64` | World seed | `new` (`fresh_seed`), daily, `--seed`, client adopts | yes |
| `is_daily` | Daily run | `char_select_input` | no |
| `microwave_used` | Microwave spent this stage | `interact_system` | no |
| `chest_opens: u32` | Drives chest price growth; **never incremented** (bug H2) | nothing | no |
| `shrines_charged` | Charge rings completed | `charge_shrines` | no |
| `pots_broken` | Pots broken | `kill_drops` | no |
| `chests_opened` | Banked into `counters.chests`; **never incremented** (H2) | nothing | no |
| `gold_collected` | Gold picked up (host-side `gold_gain` applied) | `collect` | yes |
| `evolves` | Banked into `counters.evolves`; **never incremented** (H3) | nothing | no |
| `result: Option<RunResult>` | Victory or Death | `downed_watch`, `stage_transition`, ABANDON | no |
| `difficulty` | Party difficulty: max over players' `stats.difficulty` | `player_upkeep`, greed shrine | yes |
| `character` | Hero of the local player (menus, fallbacks) | `new` | no |

### 8.7 `Stats` (27 fields)

`Stats` (`src/stats.rs:38-66`) is the derived sheet, rebuilt by `PlayerState::recompute_stats` (`src/run.rs:236-275`) from defaults, equipped tomes, the hero passive, items, greed stacks and `+1 max HP per level above 1`. `recompute_stats` preserves HP as a fraction of max HP. `StatKind` (`src/stats.rs:7-35`) names the same 27 stats for boosts.

| Field | Default | Consumed by | Granted by |
|---|---|---|---|
| `max_hp` | 100 | HP cap, food heal, Gristle/Ironclad thresholds | Health tome, Space Borgar, Ironclad, +1/level |
| `regen` (HP per minute) | 0 | `player_upkeep` | Moon Cheese |
| `shield` | 0 | `apply_player_hits`, shield regen (35%/s after 5 s) | nothing |
| `armor` | 0 | `effective_armor_fraction` a/(a+100) | Duct Tape |
| `evasion` | 0 | Dodge chance e/(e+100) | Slippery Visor |
| `lifesteal` | 0 | Chance per hit to heal 1 HP | Vampire Visor |
| `thorns` | 0 | Flat damage back to the attacker of each hit that names one (contact, Burrower eruption, Anubot beam; not shots, telegraphs or worm segments) | Thorn Plating |
| `damage` (multiplier) | 1.0 | Weapon damage | Damage tome, Protein Paste, Buzz, Gristle |
| `crit_chance` | 0.05 | Crit rolls (overcrit above 1.0) | Precision tome, Laser Sight, Reticle, B0-NK per level |
| `crit_damage` | 2.0 | Crit multiplier | Heavy Payload |
| `attack_speed` | 1.0 | Cooldown drain rate | Cooldown tome, Overclocked CPU, Valentina |
| `projectiles: i32` | 0 | Extra projectiles per volley | Splitter Chip |
| `proj_speed` | 1.0 | Projectile speed | Caffeine IV |
| `size` | 1.0 | Weapon reach/area | Fish Bowl Helmet, Aurora |
| `duration` | 1.0 | Projectile life, beam ticks | Extra Battery |
| `elite_damage` | 1.0 | Damage vs elites | nothing |
| `knockback` | 1.0 | Knockback strength | nothing |
| `move_speed` | 1.0 | Run speed | Agility tome, Rocket Boots, Nova |
| `extra_jumps: i32` | 0 | Air jumps | Chimp-O |
| `jump_height` | 1.0 | Jump velocity x sqrt | Trampoline Soles |
| `luck` | 0.0 | Rarity rolls, level-up pool | Lucky Meteorite, Cursed Moon Rock, Fortuna, greed |
| `difficulty` | 0.0 | Spawn rate and enemy HP/damage | Cursed tome, Cursed Moon Rock, greed |
| `pickup_range` (multiplier) | 1.0 | Pickup radius | Magnet Boots |
| `xp_gain` (capped at 10) | 1.0 | XP multiplier | XP tome, Star Chart |
| `gold_gain` | 1.0 | Gold pickups (host side only) | Golden tome, Golden Antenna, Doug |
| `silver_gain` | 1.0 | Silver pickups and payout | nothing |
| `chest_discount` (capped at 0.6) | 0.0 | Chest and shop prices | Space Credit Card |

---

## 9. Local message types

These are Bevy 0.18 `Message`s (the 0.18 rename of buffered events: `#[derive(Message)]`, `MessageWriter`, `MessageReader`, `add_message`). None of them crosses the network. Wire messages are in section 14.

| Message | Fields | Writers | Readers | Registered |
|---|---|---|---|---|
| `HitMsg` | `source: Option<Entity>` (the shooter; `None` = world, e.g. comet), `target: Entity`, `amount: f32` (final), `crit: bool`, `knock: Vec3` | `weapon_fire` (melee, aura, chain), `projectile_move` and `explode`, `drone_update`, `beam_update`, `comet_system`, `apply_player_hits` (thorns) | `apply_hits` | `src/messages.rs:7-17`; `src/main.rs:113` |
| `PlayerHitMsg` | `victim: Entity` (mandatory), `amount` (pre-mitigation), `from: Vec3`, `attacker: Option<Entity>` | `anubot_beam_system`, `craterpillar_update`, `burrower_emerge`, `enemy_contact`, `enemy_projectiles`, `telegraphs` | `apply_player_hits` | `src/messages.rs:20-29`; `:114` |
| `KillMsg` | `pos`, `dir`, `kind: Option<EnemyKind>`, `elite` (also true for minibosses), `xp`, `is_boss` (stage boss), `is_pot` | `apply_hits` | `kill_drops` | `src/messages.rs:32-41`; `:115` |
| `NumberMsg` | `pos`, `amount`, `kind: NumKind` (Hit, Crit, Heal, Dodge) | `apply_hits`, `apply_player_hits` (Dodge), `collect` (Heal) | `claim_numbers` | `src/messages.rs:43-57`; `:116` |
| `BannerMsg(String)` | Banner text | `run_clock`, `stage_transition`, `client_stage_transition`, `boss_phase_system`, `debug_spawn_boss`, `collect` (powerups), `charge_shrines`, `interact_system`, `comet_system`, `choice_input` (evolution) | `update_banners` | `src/messages.rs:60-61`; `:117` |
| `SfxMsg(Sfx)` | One of 16 sounds | About 20 systems | `play_sfx` | `src/messages.rs:63-84`; `:118` |
| `net::GrantOut` | `Xp(f32)` shared grant, or `Loot(PlayerId, PickupKind)` for a remote collector | `pickup_update` | `relay_grants` (host) | `src/net.rs:189-195`; `:369` |
| `AppExit` (Bevy) | | `main_menu_input` (QUIT) | Bevy | Bevy |

**Rules.** Messages are double-buffered, so a reader must tolerate entities that were despawned between write and read (`apply_player_hits` uses `get_mut(...)` with `continue`, `src/combat.rs:985-988`). On a client the damage messages are still written by the ungated local systems but never read, because `apply_hits` and `apply_player_hits` are host-only; that is intentional and cheap. The headless app registers the same messages (`src/headless.rs:238-244`).

---

## 10. Math and world model

### 10.1 Coordinates

- The planet is centred at the world origin. A surface position is a **unit direction** `dir` from the core plus a **height** above the terrain under it. Local "up" is `dir`.
- The world position of the ground under `dir` is `CurrentPlanet::surface_point(dir) = dir * surface(dir)` (`src/planet.rs:82-88`), where `surface(dir)` is the analytic terrain radius (10.4).
- The astronaut's root transform sits at `surface_point(dir) + dir * (height + PLAYER_HEIGHT * 0.5)` (`src/player.rs:583-587`). Enemies sit at `surface_point(dir) + dir * (hover + bob + 0.6 * scale)` (`src/enemies.rs:1162`).
- Everything moves by rotating its direction around the core. Nothing uses a physics engine.

| Planet | Nominal radius `R` | Circumference |
|---|---|---|
| Moon | 140 m | 880 m |
| Mars | 160 m | 1005 m |
| Dark Moon | 105 m | 660 m |

### 10.2 Frames

| Function | Definition | Where |
|---|---|---|
| `tangent_frame(up) -> (t, b)` | `helper = Y` unless `abs(up.y) >= 0.99`, then `X`; `t = normalize(helper × up)`; `b = up × t`. Stable everywhere but switches basis near the poles, so never store a frame across frames; recompute it or send the anchor it came from | `src/sphere.rs:114-119` |
| `frame_quat(up, forward) -> Quat` | Projects `forward` onto the tangent plane (fallback `t`), `right = fwd × up`, rotation with columns `(right, up, -fwd)`: local +Y becomes `up`, local -Z becomes `forward` | `src/sphere.rs:122-126` |

### 10.3 Motion on the sphere

| Function | Definition | Where |
|---|---|---|
| `advance(dir, vel, r, dt) -> (dir', vel')` | Projects `vel` onto the tangent plane (`v_t`); if it moves at all, rotates `dir` about `dir × v̂_t` by `len(v_t)·dt / r`, then re-projects `v_t` onto the new tangent plane and rescales it to the old speed. This is a first-order **parallel transport** of the velocity: speed is conserved exactly and the heading follows the great circle. `r` is the local radius, `surface(dir) + height` | `src/sphere.rs:141-151` |
| `step_toward(dir, target, angle)` | Slerps along the great circle by `min(angle / full, 1)`. Returns `dir` unchanged when `full < 1e-5` or when nearly antipodal (`full > π - 1e-4`), so callers must snap on large jumps (`src/remote.rs:137`, `src/netenemy.rs:1246`) | `src/sphere.rs:130-137` |
| `arc_dist(a, b, r)` | `angle_between(a, b) · r`: the great-circle distance. Use it for "who is nearest" | `src/sphere.rs:154-156` |
| `offset_dir(dir, heading, arc, r)` | Rotates `dir` about `dir × heading` by `arc / r`: the point `arc` metres away along `heading`. Exact inverse of the crowd-lane encoding (14.8) | `src/sphere.rs:159-165` |
| `fib_sphere(n)` | Fibonacci lattice: `y = 1 - 2(i + 0.5)/n`, azimuth `i · π(3 - √5)`; near-even points, used for rock placement | `src/sphere.rs:168-176` |
| `random_dir(rng)` | Rejection-samples the unit ball and normalizes (consumes a variable number of draws) | `src/planet.rs:505-517` |
| `hash_dir(seed, i)` | Integer xorshift-multiply hash to a unit vector; deterministic without an RNG (crater centres) | `src/sphere.rs:59-68` |

**Camera and input frames are parallel-transported too.** `CamRig.forward` is a persistent world vector. Each frame both `gather_local_input` and `camera_rig` project it onto the current tangent plane (`fwd = forward - up·(forward·up)`, fallback to `tangent_frame`), so a fixed aim stays fixed while walking and nothing flips at the poles (`src/player.rs:388-394,772-789`). Mouse X yaws `fwd` about `up` by `-dx · CAM_SENS · sensitivity`; mouse Y changes `pitch`, clamped to 0.12-1.25 rad.

### 10.4 The analytic terrain

One function describes the ground; the render mesh, the player's ground contact, and every placement sample it.

```
hills(dir, seed) = Σ_{i=0..12} 0.75^i · sin( f_i · (dir · v_i) + p_i + 1.618·seed·(i+1) )  /  Σ 0.75^i        ∈ [-1, 1]
    (v_i, f_i, p_i) = HILL_WAVES[i]: 13 fixed unit vectors, frequencies 3..43, phases        src/sphere.rs:8-37

Terrain::height(dir):                                                                         src/sphere.rs:83-106
    h  = hills(dir, seed)
    if rugged > 0:
        ridge = 1 - 2·|hills_n(dir, seed+101, 6)|              (first 6 waves only)
        mask  = clamp(hills_n(dir, seed+202, 4)·1.6 - 0.15, 0, 1)
        h    += rugged · 1.7 · max(ridge, 0) · mask             (ridged mountains, only where the mask allows)
    for i in 0..craters:
        c = hash_dir(seed+31, i)
        w = crater_width · (0.7 + 0.6·((i·2654435769 + 7) mod 100)/100)      crater_width = 0.16 rad (src/planet.rs:78)
        if angle(dir, c) < w:
            t = angle / w
            h += crater_depth · ( -(cos(πt)·0.5 + 0.5)  +  0.35·sin(πt)^3 )  (bowl + raised rim)

surface(dir) = R · (1 + amp · height(dir))                                                    src/sphere.rs:108-110
```

| Planet | `R` | `amp` (`hill_amp`) | `rugged` | `craters` | `crater_depth` | terrain `seed` | Metres per unit of height |
|---|---|---|---|---|---|---|---|
| Moon | 140 | 0.035 | 0.35 | 10 | 0.8 | 7 | 4.9 |
| Mars | 160 | 0.045 | 0.55 | 6 | 0.5 | 23 | 7.2 |
| Dark Moon | 105 | 0.03 | 0.8 | 4 | 0.6 | 66 | 3.15 |

(`src/content/planets.rs:53-124`; `height` spans roughly -1.5 to +2.) The terrain seed is a constant of the planet, **not** the run seed: every Moon has the same hills and craters. `sphere::surface_radius` (`src/sphere.rs:40-42`) is an unused older variant.

### 10.5 The terrain mesh

`planet_mesh` (`src/planet.rs:98-130`) displaces an `icosphere(7)` (163,842 vertices, 327,680 triangles, `src/meshkit.rs:182-224`) to `R·(1 + amp·h)`, colours each vertex by height band (`t = clamp(0.38h + 0.5, 0, 1)`: lerp `ground_low` to `ground` below 0.5, `ground` to `ground_high` above), computes smooth normals and renders it with a white, roughness-0.95 material so the vertex colours show. Collision stays analytic, so mesh resolution is purely visual. The mesh is rebuilt synchronously on every stage entry.

### 10.6 Props and prop colliders

`spawn_stage` (`src/planet.rs:143-503`) scatters scenery from a seeded `StdRng` (section 12). Solid props register a `PropCollider { dir, radius, height }`, a vertical cylinder on the surface (`src/planet.rs:19-24`):

| Prop | Count | Collider |
|---|---|---|
| Rocks (6 faceted variants) | `def.rocks` on a jittered Fibonacci lattice (200 / 250 / 120) | Only when `scale.x > 1.1`: radius `0.75·scale.x`, height `1.2·scale.y` |
| Boulders (3 variants, scale 3-6) | `max(rocks/20, 4)` | Always: radius `0.7·s`, height `1.4·s` |
| Crystals (emissive cones) | `def.crystals` (42 / 52 / 85) | radius `0.4·s`, height `1.6·s` |
| Wrecks (half-sunk landers) | `rocks/45 + 3` | radius `0.85·s`, height `1.1·s` |
| Radio beacons | `crystals/8 + 4` | radius 0.55, height 2.4 |
| Flora (per-world style) | `def.flora` (40 / 64 / 55) | none |
| Stars, Earth, sun | 420 stars at 1400-1900 m; Earth (Moon only) radius 90 at 1500 m; sun disc radius 45 at 1600 m | none |

`PropColliders::resolve` (`src/planet.rs:33-55`): for each collider the astronaut is not above (`height <= c.height`), if the great-circle distance from its centre (computed with the **nominal** radius) is below `c.radius + PLAYER_RADIUS`, the direction is pushed out along the great circle to exactly that distance. `player_physics` then removes the velocity component pointing into the prop, so the player slides along it (`src/player.rs:552-564`). **Only the player collides with props.** Enemies, projectiles, pickups and interactables ignore them (an interactable can spawn inside a boulder).

### 10.7 The kinematic controller

`player_input` (`src/player.rs:418-520`), for every `Player` with an `InputIntent`:

| Step | Rule |
|---|---|
| Speed multiplier | `m = stats.move_speed × 1.5 if Speed powerup` (`src/run.rs:331-337`) |
| Max speed | `8.5·m`, ×1.65 while sliding |
| Acceleration | With input: `vel_t += wish · 55 · control · dt`, `control = 1` grounded or `0.35` airborne |
| Friction | No input, grounded, not sliding, and more than 0.16 s since landing: speed drops by `38·dt` |
| Speed cap | Grounded, not sliding, past the bunny-hop window: `max speed`. Otherwise (airborne, sliding, or within 0.16 s of landing): the hard cap `8.5·m·2.1` |
| Jump | On `intent.jump`, if grounded (or within coyote time) or `jumps_used < 1 + extra_jumps`: `vel_r = 8·√jump_height`. A ground jump within 0.16 s of landing while faster than `1.05×` run speed is a **bunny-hop** (SFX and particles) and keeps the speed |
| Slide | On `intent.slide` when grounded and `slide_cd <= 0`: `slide_timer = 0.85`, `slide_cd = 1.1`, velocity set to `max(max speed, current speed, 8.5·m·1.65)` along `wish` (or `facing`); Yuki's frenzy starts |
| Facing | `wish` if any, else the velocity direction when faster than 0.5 m/s |

`player_physics` (`src/player.rs:523-589`) then ticks timers, sets `fast_move` (> 1.08× run speed), applies gravity `22 m/s²` to `vel_r`, moves with `advance` at radius `surface(dir) + height`, integrates `height += vel_r·dt`, resolves props, and on `height <= 0` lands (squash scaled by impact speed, `jumps_used = 0`). Height is measured from the terrain under the current direction, so walking over hills needs no slope code. Both W+Space hopping and sliding reach the 2.1× cap.

### 10.8 The spatial hash

`SpatialHash` (`src/enemies.rs:180-205`) buckets every `Enemy` (pots, bosses and client proxies included) into **world-space 3D cells** of `ENEMY_SEPARATION_CELL = 2.2 m`, keyed by `floor(pos / 2.2)`. It is rebuilt each frame by `rebuild_hash` (D1). `near(pos, radius)` walks the full cube of `(2⌈radius/2.2⌉ + 1)³` cells and returns every entry in them, with no distance filter; callers check distances (all except the comet cash-out and the homing target pick, see 11.14 and 11.8). Cost per query: 27 cells for separation (radius 2.2), 3375 for comet tail and homing (radius 14), 9261 for the comet cash-out (radius 22).

### 10.9 Enemy steering

`enemy_move` (`src/enemies.rs:1019-1121`): each enemy picks the nearest living astronaut by arc distance, steps toward it along the great circle by `speed·(1 - slow)·dt / surface(dir)` radians, or, for ranged kinds (`standoff > 0`) inside their standoff arc, backs off below 65% of it and circle-strafes otherwise. It is then pushed apart from up to 6 neighbours closer than `0.9·scale + 0.6 m` (`push · 0.35 · dt`), knocked back by its decaying `knock` impulse via `advance`, and animated by `animate_crowd` (16.7). Pots (`Without<Pot>`) and buried burrowers (`Without<Buried>`) are excluded.

---

## 11. Gameplay rules as implemented

This section states the rules exactly as the code applies them. Design intent lives in `GDD.md`; where the two differ, this is what ships.

### 11.1 Stage clock and marks

| Rule | Value | Where |
|---|---|---|
| Stage length (counting down) | 600 s, 540 s, 480 s for stages 1, 2, 3 | `src/config.rs:40`; `src/director.rs:196` |
| Minibosses | At timer 420 s (Craterpillar Jr) and 120 s (Rover Gone Wrong), on every planet | `src/config.rs:41`; `src/director.rs:74-84` |
| Stage boss | At timer 90 s: THE CRATERPILLAR on the Moon, JUDGE ANUBOT everywhere else (Mars **and** Dark Moon) | `src/config.rs:42`; `src/director.rs:86-96` |
| Spawn anchor for bosses | The normalized centroid of all astronaut directions (downed ones included) | `src/director.rs:63-72` |
| Boss placement | 30 m of arc from the anchor, random heading (`thread_rng`) | `src/enemies.rs:638-644` |
| Teleporter | Once `boss_dead`: spawned 18 m from the anchor in a random direction, banner "TELEPORTER ONLINE — OR STAY AND FARM" | `src/director.rs:98-103`; `src/interact.rs:299-335` |
| Clock at 0 | THE STATIC starts (below) | `src/director.rs:105-114` |
| `elapsed` resets each stage | Stages 2 and 3 restart at 1.0x enemy scaling and a Shambler-only mix | `src/director.rs:197` |

### 11.2 THE STATIC

When the timer reaches 0, `static_active` becomes true and `run_clock` thereafter only advances `static_timer`, `elapsed` and `total_elapsed` and returns early (`src/director.rs:55-59`). Consequences: no further marks, and **a boss killed during the Static never opens the teleporter** (the teleporter check sits after the early return). The spawner emits only `Ghost` enemies at `10 + 0.15·static_timer` per second (times party scale); ghosts give no XP and drop 1 silver half the time (`src/pickups.rs:329-333`). The music detunes (section 17). `counters.static_secs_best` banks the longest Static of the **last** stage only (`src/director.rs:290`).

### 11.3 The spawn director

`director_spawn` (`src/enemies.rs:539-626`), host/solo only:

```
anchors  = directions of every astronaut (downed ones included)
P        = [1.0, 1.75, 2.4, 3.0][clamp(players, 1, 4) - 1]                party scale
rate     = (1 + 2.1 · elapsed/60) · (1 + difficulty) · P                  normal
         = (10 + 0.15 · static_timer) · P                                 during THE STATIC
spawn_bank += rate · dt ; every 0.25 s spawn floor(spawn_bank), limited to room = floor(1200·P) - live Enemy count
```

- The cap counts every `Enemy`, so pots (60 on the Moon) and bosses use cap slots. Overflow is discarded, not banked.
- Each spawn round-robins over the anchors, picks a uniform heading, and places the enemy 42-58 m of arc out (over the horizon). Burrowers instead appear 9-16 m away.
- The kind is uniform over `EnemyKind::mix(elapsed)` (`src/content/enemies.rs:155-167`):

| `elapsed` (s) | Mix |
|---|---|
| 0-89 | Shambler |
| 90-179 | + Sprinter |
| 180-269 | + Spitter |
| 270-359 | + Bruiser |
| 360-449 | + UFO, Beamer |
| 450-539 | + Burrower |
| 540+ | + Lobber |

- Elites: from `elapsed > 150` each spawn has a 1.2% chance, and a guaranteed elite whenever `elite_timer` (45 s, then every 40 s) expires (`src/enemies.rs:612-616`).
- All spawn-side randomness here comes from `GameRng` (section 12).

Solo at difficulty 0 this is about 10 spawns/s at 4.5 minutes and 22/s at the end of a 10-minute stage, roughly 6,900 per stage; in practice the cap is reached mid-stage. This is 15 times steeper than the GDD's `1 + 0.14t` (`GDD.md:263`); whether to keep it is a design decision.

### 11.4 Enemy scaling and elites

| Rule | Formula | Where |
|---|---|---|
| HP multiplier | `(1 + 0.22·t·max(√t, 1)·0.5 + 0.35·t) · (1 + difficulty)`, `t = elapsed/60` | `src/content/enemies.rs:238-243` |
| Damage multiplier | `(1 + 0.12·t) · (1 + 0.5·difficulty)` | same |
| Per-enemy variation | Scale ×0.92-1.1, speed ×0.9-1.15 | `src/enemies.rs:495,504` |
| Elite | HP ×8, damage ×1.8, scale ×1.65, XP ×8, orange emissive material | `src/content/enemies.rs:229-235`; `src/enemies.rs:495-506` |
| Party difficulty | `max` over every player's `stats.difficulty` (one Cursed build raises it for all) | `src/player.rs:851-852` |

Party size scales spawn count only; per-enemy and boss HP do not scale with players.

### 11.5 Enemy attacks

| Attacker | Behaviour | Numbers | Where |
|---|---|---|---|
| Any mover | Contact hit on the nearest astronaut inside reach | Reach `0.55·scale + 0.45 + 0.25 m`; one hit per 0.5 s per enemy | `src/enemies.rs:1237-1271` |
| Spitter | Shot when the nearest astronaut is within range | Range 20 m, cooldown 2.8 s (first 1-3 s), speed 11 m/s, life 4 s, hover 1 m | `src/enemies.rs:1274-1323` |
| UFO | Same component, faster shots | Range 15 m, cooldown 2.2 s, speed 16 m/s | same |
| Beamer | Paints an aim line at a **latched** target for 1.1 s, tracking until the last 0.25 s, then fires a railbolt | Trigger within 26 m, cooldown 4 s (first 2-4 s), bolt 40 m/s, life 1.4 s | `src/enemies.rs:1327-1443` |
| Lobber | Mortar at the target's position: a disc telegraph plus an arcing shell | Range 24 m, cooldown 4.5 s (first 2.5-5 s), flight 1.6 s, radius 3.2 m, shell peak 9 m | `src/enemies.rs:1446-1496,1499-1520` |
| Burrower | Spawns buried near the player, rumbles for 1.3 s, erupts | Eruption hits within 2.6 m | `src/enemies.rs:1196-1234` |
| Enemy projectile | Travels over the sphere; the first astronaut within √1.1 m takes it | | `src/enemies.rs:1522-1558` |
| Telegraph | Grows over its timer, then detonates | Ring: hits `0.35·r < d < r + 1`; disc: `d < r + 0.6` | `src/enemies.rs:1608-1647` |
| Dust storm | Spitters, UFOs, Beamers and Lobbers do nothing while the local player is inside a storm (Beamers also drop their aim lines) | | `src/enemies.rs:1284,1342-1350,1456` |

### 11.6 Bosses

| Boss | HP | Speed | Contact dmg | Scale | Role | Mesh |
|---|---|---|---|---|---|---|
| Craterpillar Jr | 700 | 3.4 | 16 | 2.6 | Miniboss at 7:00 | Generic boss mesh (clients draw a worm head, a known mismatch) |
| Rover Gone Wrong | 1100 | 5.2 | 20 | 2.2 | Miniboss at 2:00 | Generic boss mesh |
| THE CRATERPILLAR | 5200 | 3.0 | 26 | 4.6 | Moon stage boss | Worm head plus 12 trailing segments |
| JUDGE ANUBOT | 8200 | 3.6 | 32 | 4.2 | Mars and Dark Moon stage boss | Jackal-headed rover plus the Verdict Beam |

(`src/content/enemies.rs:181-226`.) At spawn: HP ×`(1 + difficulty)`, contact damage ×`(1 + 0.5·difficulty)`, XP 50, `Enemy.kind = Bruiser`, `elite = true` (`src/enemies.rs:645-677`). Knockback on bosses is scaled by 0.05 (`src/combat.rs:928`).

| Boss mechanic | Rule | Where |
|---|---|---|
| Slam | Every 6.5 s (first at 4 s): ring telegraph radius 7 m over 1.4 s, damage ×1.6 | `src/enemies.rs:1576-1588` |
| Burst | Every 9 s (first at 7 s): 12 radial shots, speed 9, damage ×0.8, life 5 s | `src/enemies.rs:1589-1603` |
| Phases | At 66% and 33% HP: speed ×1.28, damage ×1.22, slam timer ≤ 1.5 s, burst ≤ 2 s, banner (BURROW BLOOM / HELMET CHOIR for the worm, SANDSTORM COURT / FINAL JUDGMENT for Anubot, ENRAGED otherwise), a ring of `8 + 3·phase` adds 42-58 m around the nearest living astronaut from the `mix(max(elapsed, 300))` pool, 25% elite in phase 2. Add rings ignore the cap | `src/enemies.rs:720-793` |
| Craterpillar body | The head records a trail point per 0.55 m; segment `i` sits at trail index `2i`, undulates (`sin(3.9t - 0.8i)`), and hits for 70% of the head's contact damage. Segments despawn when the head is gone | `src/enemies.rs:941-1016` |
| Verdict Beam | Idle (first 2.5 s) → charging `1.3 - 0.3·phase` s → firing `3.0 + 0.6·phase` s → idle `max(2.6 - 0.7·phase, 0.8)` s. Spins 0.25 / 0.5 / `1.15·(1 + 0.3·phase)` rad/s. While firing, every astronaut in the corridor (34 m long, within 2.4 m either side of the beam line, so 4.8 m wide) takes `1.2 × damage` per frame (limited by i-frames). The boss rears up while charging and shudders while firing | `src/enemies.rs:798-911` |
| Death | A stage boss death sets `boss_dead`, shake 0.8, hitstop 0.25 s, drops 14 gold piles | `src/combat.rs:947-966`; `src/pickups.rs:359-364` |

### 11.7 Damage to astronauts

`apply_player_hits` (`src/combat.rs:973-1024`), in order:

1. Skip if the victim has i-frames or HP ≤ 0.
2. Evasion: dodge with probability `evasion/(evasion + 100)`; shows DODGE.
3. Armor: `amount × (1 - a/(a + 100))`; Old Ironclad doubles `a` below 30% HP.
4. Shield absorbs first; `shield_cd = 5 s` on every hit.
5. HP loss; i-frames 0.4 s; local shake and hurt SFX.
6. Thorns: `stats.thorns` flat damage back to `attacker` if the hit named one.
7. HP ≤ 0: `dead = true`. The run ends when every astronaut is down (`downed_watch`). There is no revive.

Upkeep (`player_upkeep`, `src/player.rs:842-875`): regen `stats.regen / 60` HP per second while alive; shield refills at 35% of max per second once `shield_cd` reaches 0; powerup timers count down.

### 11.8 Weapons

`weapon_fire` (`src/combat.rs:212-577`) runs per living astronaut, per weapon:

| Quantity | Formula |
|---|---|
| Damage | `def.damage × (1 + 0.24·(level-1)) × damage_mult()` |
| Projectile count | `max(def.projectiles + extra(level) + stats.projectiles, 1)`, `extra` = 0 at levels 1-2, 1 at 3-4, 2 at 5-6, 3 at 7 |
| Size | `(1 + 0.06·(level-1)) × stats.size` |
| Cooldown | `cd -= dt × attack_speed()`; fires at `cd <= 0`, then `cd = def.cooldown` |
| Target | Nearest non-pot, non-buried enemy within 40 m (straight-line); with none, the astronaut's facing |

(`src/content/weapons.rs:494-505`.) Behaviours:

| Behaviour | Rule |
|---|---|
| `MeleeArc{arc_deg, range}` | Hits every enemy within `range·size` whose tangent direction is inside the arc (360 = all around). Knockback `9 × knockback`. Sweep visual |
| `Shot{speed, pierce, spread_deg}` | `count` projectiles spread evenly over `spread_deg` (a single one jitters ±0.04 rad); life `2.2 × duration` |
| `Seek{speed, pierce}` | Homing: turns at rate 10 toward the nearest non-pot enemy returned by `hash.near(pos, 14)` (the cells around 14 m, with no radius check, so the pick can lie a little past 14 m); life `2.6 × duration` (`src/combat.rs:607-626`) |
| `Boomerang{speed, range}` | Pierce 999; flies out for `range/speed` s, then steers home and despawns within 1.2 m of its owner |
| `Beam{range, width}` | A corridor from the owner toward the target for `5 × duration + 1` ticks of 0.12 s |
| `Orbit{radius, deg_per_sec}` | `count` drones orbit the owner at `radius × size`; each hits what it touches, then rests 0.3 s |
| `Chain{jumps, range, link_range}` | Zaps the nearest enemy within `range`, then `jumps + count - 1` more links within `link_range` |
| `Rocket{speed, aoe}` | Spread ±0.6 rad, homing (rate 6), explodes on contact or expiry (life `4 × duration`) for `aoe × size` |
| `Aura{radius, slow}` | Every cooldown, hits every enemy within `radius × aura_scale() × size`. The per-weapon `slow` field is unused |

Projectile hits: reach `0.45·size + 0.55 + 0.5·enemy.scale`; a projectile re-hits the same target only after 0.5 s; pierce decrements per hit; knockback `heading × 4 × knockback` (`src/combat.rs:580-705`). Drones and auras are reconciled each frame per (owner, weapon), so two players with the same orbital weapon keep separate drones.

### 11.9 Damage to enemies

Crits and the elite bonus are applied by the **weapon code when it writes the `HitMsg`** (`weapon_fire` for melee, auras and chains, `projectile_move` and `explode`, `drone_update`, `beam_update`), so `HitMsg.amount` arrives final:

- **Crits** (`roll_crit`, `src/combat.rs:196-208`, called at `src/combat.rs:268,315,454,684,726,770,831`): while `chance > 0`, roll `min(chance, 1)`; each success multiplies by `crit_damage`; `chance -= 1`. So 150% crit chance is one guaranteed crit plus a 50% second one (overcrit). The comet cash-out is a fixed `crit: true` hit, and thorns never crit.
- Elite bonus: `× stats.elite_damage` against elites (always 1.0 today), same call sites.

`apply_hits` (`src/combat.rs:877-970`) then resolves each message:

- A pot breaks on any hit (`KillMsg{is_pot}`).
- Enemies lose HP, flash, take knockback (×0.05 on bosses), and if the shooter owns Cryo Vent or Absolute Zero gain `+0.25` slow (capped 0.65; slow decays 0.35 per second).
- Lifesteal: the shooter heals 1 HP with probability `lifesteal`.
- At HP ≤ 0: `KillMsg` (minibosses count as elite), despawn; a stage boss sets `boss_dead`.

### 11.10 XP and levelling

- `xp_needed(L) = 6 + 3.4·(L-1) + 0.18·(L-1)²` (`src/run.rs:387-390`): 6 at level 1, 9.6 at 2, 51.2 at 10, 135.6 at 20.
- `gain_xp(v)` adds `v × xp_gain` and can cascade several levels (`src/run.rs:348-356`).
- **XP is a shared pool on individual curves**: any collected gem grants its value to every living astronaut, each through its own `xp_gain` and thresholds (`src/pickups.rs:205-221`).
- Each level-up queues a panel on the machine that owns that astronaut (`levelup_trigger`), and `recompute_stats` adds +1 max HP per level.

### 11.11 Level-up cards

`roll_upgrades` (`src/run.rs:510-577`) always returns 4 options:

1. Up to 2 guaranteed **Evolve** cards for weapons at level 7 whose catalyst item is owned (no per-run evolution cap).
2. A pool: each non-max weapon twice (`WeaponUp`); every unlocked base weapon that fits a free slot (4 slots; a weapon and its evolution share a slot) once (`NewWeapon`); each non-banned, non-maxed item 3/2/1/1 times for Common/Rare/Epic/Legendary, each copy kept with probability 0.9 / `0.55 + 0.2L` / `0.3 + 0.25L` / `0.12 + 0.2L` (`L` = luck clamped 0-2).
3. The pool is shuffled; distinct entries fill the 4 slots; any shortfall becomes a 15-44 gold pile.

Panel controls: pick 1-4; on **level-up** panels only (not shrine, Moai or microwave panels, `src/ui/panels.rs:190-214`): **R** refresh (2 charges; Lady Fortuna refreshes free and unlimited), **B** banish (3 charges; the next pick removes that card instead of taking it, and a banished item never rolls again this run; banishing a weapon or gold card only removes it), **S** skip for +10 gold (`src/ui/panels.rs:139-256`). Rarity of a card: Evolve Legendary, NewWeapon Rare, WeaponUp Common (Epic for an evolved weapon), items their own rarity.

### 11.12 Pickups and drops

| Rule | Value | Where |
|---|---|---|
| Pickup radius | `3.2 m × pickup_range`, ×40 with Magnet | `src/run.rs:339-346` |
| Flight | Starts at 6 m/s, accelerates 60 m/s² up to 46.8 m/s, collected within 0.8 m; the target is latched | `src/pickups.rs:157-196` |
| Gold | `round(g × gold_gain)` to the collector; also `run.gold_collected` | `src/pickups.rs:254-261` |
| Silver | `round(s × silver_gain)` into `run.silver_run` | `src/pickups.rs:262-268` |
| Food | Heals 20% of max HP | `src/pickups.rs:269-276` |
| Powerups | 2x DAMAGE 20 s (damage ×2), MEGA MAGNET 12 s (pickup radius ×40), SPEED BOOST 15 s (move ×1.5); re-collecting refreshes | `src/pickups.rs:277-291`; `src/run.rs:292-346` |
| Normal kill | XP gem worth the enemy's `xp`; 7% a 1-3 gold coin; 1.2% food; 0.6% a random powerup | `src/pickups.rs:334-357` |
| Elite kill | XP gem; 4-7 gold piles of 4-9; 35% a random powerup | `src/pickups.rs:338-345` |
| Stage boss kill | Additionally 14 gold piles of 8-19 | `src/pickups.rs:359-364` |
| Pot | 12% silver (1-2), 12% food, otherwise 1-3 gold coins of 2-6 | `src/pickups.rs:308-325` |
| Static ghost kill | 50% one silver instead of an XP gem; the normal gold, food and powerup rolls still apply | `src/pickups.rs:329-357` |
| Gem cap | Every second, if more than 550 XP gems exist, the first `count - 550 + 40` in query order merge into one gem at their centroid | `src/pickups.rs:375-404` |
| Scatter | Each drop lands up to 1.2 m from the kill | `src/pickups.rs:80-85` |

### 11.13 Interactables and the in-run economy

Per stage, from the seeded layout (section 12): `def.pots` pots at least 8 m from the drop point (10% look silver, same loot), 7 chests (≥ 12 m), 2 Shady Guys with 3 pre-rolled items each (≥ 15 m), 2 Greed and 2 Magnet shrines (≥ 15 m), 1 Moai and 1 microwave (≥ 20 m), the cage (Moon only, while `chimp_freed` is false, ≥ 25 m), 5 charge rings (≥ 18 m) (`src/interact.rs:135-296`). Interaction range is 4.5 m (`INTERACT_RANGE + 1.5`, `src/interact.rs:426`).

| Interactable | Effect | Where |
|---|---|---|
| Chest | Costs `round(25 × 1.75^chest_opens × (1 - chest_discount))` gold (stuck at 25 because `chest_opens` never increments). E reveals an item rolled at the player's luck (kept for that chest); TAKE pays and adds it (ignores `max_stacks`); LEAVE keeps the chest | `src/interact.rs:440-442,466-475`; `src/ui/panels.rs:341-361` |
| Shady Guy | Shop of 3 items priced 30/60/120/240 by rarity × `(1 - chest_discount)`; sold items stay sold. Stock and prices are rolled at stage build from the sheet passed to `spawn_interactables`, so stage 1 always uses a fresh level-1 sheet (no Space Credit Card discount, no item luck) and later stages use the local carried sheet | `src/interact.rs:113-121,241-249,476-480`; `src/ui/panels.rs:449-484` |
| Greed Shrine | One use: `greed_stacks += 1` (+12% difficulty, +8% luck via `recompute_stats`) | `src/interact.rs:481-489` |
| Magnet Shrine | One use: every pickup on the planet flies to the user | `src/interact.rs:490-499` |
| Moai | One use: choose 1 of 3 items rolled at luck +0.15 | `src/interact.rs:500-514` |
| Microwave | Once per stage: choose 1 of up to 3 owned, non-maxed items to duplicate; spent even when nothing fits | `src/interact.rs:515-535` |
| Cage | Sets `counters.chimp_freed` (saved at the next save; unlocks Chimp-O through the FreeChimp quest) | `src/interact.rs:536-544` |
| Charge ring | Fills at `dt/8` per astronaut standing within 4.2 m, drains at `dt/16`; when full: 1 of 3 items at luck +0.3 for the **local** player | `src/interact.rs:338-390` |
| Teleporter | E: `PendingStage(stage + 1)` | `src/interact.rs:545-549` |

Item rolls (`roll_item`, `src/interact.rs:92-111`) pick a rarity with `Rarity::roll(luck)` (weights `[max(62 - 30l, 8), 26 + 8l, 9 + 14l, 3 + 8l]`, `l` = luck clamped 0-3, `src/content/mod.rs:38-58`), then a uniform non-banned, non-maxed item of that rarity, falling back to any non-maxed item.

### 11.14 The Comet Combo

`comet_system` (`src/comet.rs:33-127`), local player only:

- **Tail** = moving enemies within 14 m that are not ahead of the direction of travel (`dot < 0.35`).
- With tail ≥ 8 and speed > 2 m/s, `charge += tail × speed × dt`; ticks at 25/50/75%.
- At `charge ≥ 900`: every moving enemy returned by `hash.near(player, 22)` takes a critical `300 + 18 × peak_tail` hit with knockback `12 × knockback`. There is **no distance check** after the cell lookup (`src/comet.rs:97-107`), so the blast covers the whole 21×21×21-cell cube: about 22-24 m along each axis and up to about 42 m toward its corners, not a 22 m sphere. The cash-out also does `silver_run += peak_tail` and triggers shake 0.8, hitstop 0.22 s and the banner "☄ COMET xN!".
- If the tail condition fails for more than 0.7 s the combo resets with no payout.

### 11.15 The Mars dust storm

`dust_storm_system` (`src/events_world.rs:32-130`): Mars only. A hidden 24 m dome is created the first time; the first storm comes after 10 s, then storms last 26 s with 20 s gaps. A storm appears at a random direction and drifts 3.2 m/s along a great circle. `player_inside` (local player only) hides that player from ranged enemies and fades in a haze. On other planets the storm is switched off. `DustStorm` is never reset between runs.

### 11.16 Hero mechanics

| Hero | Flat passive (via `recompute_stats`) | Extra mechanic | Where |
|---|---|---|---|
| Buzz | +10% damage | | `src/run.rs:248` |
| Valentina | +15% attack speed | | `src/run.rs:249` |
| B0-NK | +0.5% crit per level | | `src/run.rs:250` |
| Yuki | none | Slide grants +30% attack speed for 3 s | `src/run.rs:280-284`; `src/player.rs:495-497` |
| Chimp-O | +1 jump | | `src/run.rs:251` |
| Doug | +25% gold gain | | `src/run.rs:252` |
| Dr. Reticle | +10% crit | Guaranteed crit during the first 0.35 s of each 1.5 s cycle | `src/run.rs:305-311` |
| Slipstream Nova | +15% move speed | +1.3 attack speed while above 1.08× run speed | `src/run.rs:285-288` |
| Old Ironclad | +90 max HP | Armor doubles below 30% HP | `src/run.rs:323-329` |
| Lady Fortuna | +30% luck | 4 refreshes at start and all refreshes free | `src/run.rs:229-232`; `src/ui/panels.rs:192-196` |
| Aurora Prime | +20% size | Auras ×1.35 while above 1.08× run speed | `src/run.rs:314-320` |
| Sgt. Gristle | +15% damage | Damage ×1.4 below 50% HP | `src/run.rs:297-300` |

### 11.17 End of run: silver, counters, quests

`bank_results` (`src/director.rs:272-346`, host/solo only):

```
counters.kills += run.kills ; pots += pots_broken ; chests += chests_opened ; shrines += shrines_charged
counters.gold  += gold_collected ; evolves += run.evolves
best_level = max(best_level, local level) ; static_secs_best = max(.., static_timer) ; runs_started += 1 ; runs_won += victory
payout = silver_run + floor( (kills/40 + level + (victory ? 30·tier : 0)) × silver_gain )
save.silver += payout
daily: if the day changed, reset daily_best; daily_best = max(daily_best, payout)
non-daily run: tutorial_done = true
check_quests(); save()
```

A death still pays out. Quests are checked only here.

---

## 12. Seeded RNG streams and the determinism contract

### 12.1 Seeds

| Seed | Definition | Where |
|---|---|---|
| `run_seed` (normal run) | `fresh_seed()`: nanoseconds since the Unix epoch | `src/run.rs:38-43,181` |
| `run_seed` (daily) | `daily_seed(today())`: splitmix64 avalanche of the UTC day number (days since the epoch), so everyone gets the same world on the same day | `src/run.rs:46-60`; `src/ui/menus.rs:486-490` |
| Daily name | `WORDS[seed % 8]-{(seed >> 8) % 16:X}{(seed >> 3) % 16:X}`, e.g. `GRIEF-7B`; words GRIEF, HUSH, EMBER, VIGIL, DROSS, WANE, SILT, PALL | `src/run.rs:63-66` |
| `run_seed` (headless) | `fresh_seed()` or `--seed` | `src/headless.rs:209-212` |
| `run_seed` (client) | Adopted from the host's `RunSnapMsg` | `src/net.rs:528` |
| `stage_seed` | `run_seed.wrapping_add(stage)` | `src/main.rs:430`; `src/director.rs:206`; `src/netenemy.rs:804`; `src/headless.rs:369` |
| Terrain seed | A per-planet constant (Moon 7, Mars 23, Dark Moon 66) | `src/content/planets.rs:62,86,110` |

### 12.2 Streams

| # | Stream | Seeded from | Consumers, in draw order | Must match across machines? | Where |
|---|---|---|---|---|---|
| 1 | Terrain (no RNG) | Terrain seed | `hills`, the ridge mask, `hash_dir` crater centres; rock and boulder mesh shapes use `hills` with seeds `100 + 13i + terrain_seed` and `400 + 17i + terrain_seed` | Yes, by construction | `src/sphere.rs`; `src/planet.rs:196-198,224-226` |
| 2 | Scenery | `StdRng::seed_from_u64(stage_seed ^ 0xA11CE ^ terrain_seed)` | Rocks (jitter 3 draws, scale 4, mesh variant, dark-material `gen_bool`, yaw), boulders, crystals, wrecks, beacons, flora, 420 stars. `random_dir` rejection-samples, but the draw count depends only on the stream | **Yes**: prop colliders and the look of the world. Checked by `layout_sum` under `--netlog` | `src/planet.rs:152-503` |
| 3 | Interactable layout | `StdRng::seed_from_u64((run_seed + stage) · 0x9e37)` (wrapping) | Pots (`place_dir`, which retries up to 40 times, then `gen_bool(0.1)` for the silver look), 7 chests, 2 Shady Guys (**3 `roll_item` stock rolls each, then `place_dir`**), 2 Greed, 2 Magnet, Moai, microwave, the cage's `place_dir` (drawn even when the cage is not spawned), 5 charge rings | **Yes**: players must stand in the same rings and chests. Checked by `inter_sum` | `src/interact.rs:147-295` |
| 4 | `GameRng` | `StdRng` reseeded to `stage_seed` on every stage build | Only `director_spawn` and the `spawn_enemy` calls it makes (heading, arc, kind, elite roll, scale, speed, wobble, stride, attack cooldowns) | No (clients do not spawn since THE SWITCH). Not reproducible run to run either, because spawns are time-banked on a variable `dt` | `src/run.rs:22-35`; `src/enemies.rs:560` |
| 5 | `rand::thread_rng()` | OS entropy | Crits, lifesteal, evasion (`src/combat.rs:229,596,751,802,890,983`); level-up cards (`src/director.rs:130`; `src/ui/panels.rs:197,248`); boss placement and add rings (`src/enemies.rs:638,736`); the storm (`src/events_world.rs:59`); particles (`src/fx.rs:141`); pickup scatter and loot (`src/pickups.rs:80,306`); teleporter placement (`src/interact.rs:306`); shrine blessing and chest/Moai/microwave rolls (`src/interact.rs:355,418`) | No: host-authoritative results | |
| 6 | Fixed hashes | Constants | `hash_dir`; client proxy wobble `(id · 0.618034 · τ) mod τ` (`src/netenemy.rs:1150`); the audio and music noise LCGs (fixed seeds, so every launch synthesizes bit-identical sounds) | n/a | |

### 12.3 The determinism contract

1. For the same `(run_seed, stage, chain)`, the host and every client must build the **same planet, the same props and the same interactable positions**. Streams 1-3 guarantee this only if every draw sequence is independent of per-machine state.
2. **Rule:** code that consumes stream 2 or 3 must consume the same number of draws regardless of the save, the hero, the player's sheet or anything else that can differ between machines. Conditional content must still make its draws; the cage is the precedent (`src/interact.rs:263-272`).
3. **Known violation (M1).** The Shady Guy stock rolls draw from stream 3 through `roll_item`, whose `choose` over a candidate list consumes a number of words that depends on the list length (rand 0.8 rejection sampling), and the list depends on the `PlayerState` passed in (luck changes the rarity; items and bans change the length). `enter_run` passes a fresh sheet of the machine's own hero (Lady Fortuna's +0.30 luck differs), `stage_transition` passes the host's carried sheet, and `client_stage_transition` passes a fresh sheet (`src/main.rs:442`; `src/director.rs:212-216,227`; `src/netenemy.rs:827`). Everything placed after the Shady Guys (shrines, Moai, microwave, cage, charge rings) can then land in different places. The fix is to draw the stock from its own stream.
4. Nothing else needs cross-machine determinism: the spawn stream, combat and loot run only on the host and reach clients as results.
5. **Not deterministic within one machine either:** the windowed game integrates a variable `dt` (no fixed timestep), and combat, loot, boss placement, level-up cards and the storm draw from `thread_rng`. The headless smoke steps a fixed 33 ms but still diverges between runs, even with `--seed`, because of `thread_rng`. `SpatialHash` is **not** a source: `near()` looks cells up by key in a fixed x/y/z order, and each cell's list keeps the query order of `rebuild_hash` (`src/enemies.rs:190-204,473-481`); the netenemy maps use `bevy::platform` collections, which hash with a fixed seed (`src/netenemy.rs:44`).
6. Float determinism across operating systems (libm `sin`/`cos` in the terrain) is unverified. Both co-op machines should run the same exe.
7. Verification: run both instances with `--netlog` and compare `layout_sum` (props, `src/net.rs:824-834`) and `inter_sum` (interactables, `src/net.rs:839-842`) every second. `inter_sum` legitimately differs when the saves disagree on `chimp_freed` and because the host alone has a teleporter.

---

## 13. Save format

### 13.1 Location and I/O

| Item | Value | Where |
|---|---|---|
| Path | `%APPDATA%/astrobonk/save.json`; `./astrobonk/save.json` when `APPDATA` is unset | `src/save.rs:107-112`; `src/config.rs:65-66` |
| Format | Pretty-printed JSON (`serde_json::to_string_pretty`) | `src/save.rs:152` |
| Write | `std::fs::write` of the whole file: not atomic, no backup | `src/save.rs:147-160` |
| Load | Missing file: `MetaSave::default()`. Parse error: `warn!("save corrupt ...")` and `MetaSave::default()`; **the next write overwrites the old file** | `src/save.rs:115-126` |
| Loaded | Once, in `boot` | `src/main.rs:380` |
| Written | `bank_results` at the end of a run (host/solo only) (`src/director.rs:331`); every settings change and on closing settings (`src/ui/settings.rs:113,125`); tome purchase and tome loadout toggle (`src/ui/menus.rs:325,338`) | |

### 13.2 `MetaSave` schema

`MetaSave` (`src/save.rs:31-53`) has a **container-level `#[serde(default)]`**: a field missing from the file takes its value from `MetaSave::default()`. Unknown fields are ignored.

| Field | Type | Default | Meaning |
|---|---|---|---|
| `silver` | u64 | 0 | Meta currency |
| `tome_levels` | map TomeKind → u32 | empty | Tome levels bought (0-20) |
| `tome_loadout` | list of TomeKind | `["Damage", "Health", "Xp"]` | Equipped tomes (only these apply) |
| `tome_slots` | u32 | 3 | Loadout size; quests raise it, capped at 5 |
| `unlocked_chars` | set of AstronautKind | all heroes except B0nk, Yuki, ChimpO, Doug | Selectable heroes |
| `unlocked_weapons` | set of WeaponKind | Wrench, LaserPistol, RivetGun, Kunai, Boomerang, MiningLaser, MeatballComet, StaticCling, RicochetDisc, SonicWhoopee, CosmonautsBell, YoYo | Weapons that can appear as NEW cards |
| `unlocked_planets` | set of PlanetKind | Moon, Mars ("dev: Mars selectable for playtesting") | Selectable start planets |
| `quests_done` | set of QuestKind | empty | Completed quests |
| `counters` | `Counters` | all zero | Lifetime counters, below |
| `volume` | f32 | 0.7 | Master volume |
| `music_volume` | f32 | 1.0 | Music slider |
| `sfx_volume` | f32 | 1.0 | SFX slider |
| `sensitivity` | f32 | 1.0 | Mouse sensitivity multiplier (0.25-3.0) |
| `shake_scale` | f32 | 1.0 | Screenshake multiplier (0-1.5) |
| `daily_day` | u64 | 0 | Day number of the last daily |
| `daily_best` | u64 | 0 | Best daily payout that day |
| `tutorial_done` | bool | false | First-run tutorial seen |

### 13.3 `Counters` schema

`Counters` (`src/save.rs:14-29`) has **no `#[serde(default)]`**: every field must be present, or the whole file fails to parse and is reset. Add `#[serde(default)]` to `Counters` before adding any counter.

| Field | Type | Banked from | Used by quests |
|---|---|---|---|
| `kills` | u64 | `run.kills` | Kill100, Kill1000, Kill2500, Kill10000 |
| `pots` | u64 | `run.pots_broken` | Pots50 |
| `chests` | u64 | `run.chests_opened` (never incremented, H2) | Chests10 |
| `shrines` | u64 | `run.shrines_charged` | Shrines5 |
| `gold` | u64 | `run.gold_collected` | Gold5000 |
| `evolves` | u64 | `run.evolves` (never incremented, H3) | EvolveWeapon |
| `best_level` | u32 | max with the local level (reads 1 today, H1) | Level20 |
| `static_secs_best` | f32 | max with `static_timer` | SurviveStatic2Min |
| `runs_started` | u64 | +1 per banked run | `migrate` (veterans skip the tutorial) |
| `runs_won` | u64 | +1 per victory | none |
| `chimp_freed` | bool | set by the cage interaction | FreeChimp; hides the cage |
| `cleared` | set of (PlanetKind, u32) | victory: `(chain[0], tier)` and `(later planet, 1)` | ClearMoonT1-T3, ClearMarsT1; tier gating in planet select |

### 13.4 Example

```json
{
  "silver": 521,
  "tome_levels": { "Damage": 3, "Health": 1 },
  "tome_loadout": [ "Damage", "Health", "Xp" ],
  "tome_slots": 3,
  "unlocked_chars": [ "Buzz", "Valentina", "B0nk", "Reticle", "Nova", "Ironclad", "Fortuna", "Aurora", "Gristle" ],
  "unlocked_weapons": [ "Wrench", "LaserPistol", "RivetGun", "Kunai", "Boomerang", "MiningLaser", "MeatballComet", "StaticCling", "RicochetDisc", "SonicWhoopee", "CosmonautsBell", "YoYo", "Tesla" ],
  "unlocked_planets": [ "Moon", "Mars" ],
  "quests_done": [ "Kill100", "Kill1000", "ClearMoonT1" ],
  "counters": {
    "kills": 1525, "pots": 12, "chests": 0, "shrines": 0, "gold": 400, "evolves": 0,
    "best_level": 1, "static_secs_best": 0.0, "runs_started": 2, "runs_won": 1,
    "chimp_freed": false,
    "cleared": [ [ "Moon", 1 ] ]
  },
  "volume": 0.7, "music_volume": 1.0, "sfx_volume": 1.0, "sensitivity": 1.0, "shake_scale": 1.0,
  "daily_day": 0, "daily_best": 0, "tutorial_done": true
}
```

Sets and maps serialize in unspecified order. Enum values are stored **by variant name**, so renaming or removing any variant of `AstronautKind`, `WeaponKind`, `PlanetKind`, `QuestKind` or `TomeKind` makes existing saves fail to parse and reset. Adding variants is safe.

### 13.5 Migration

`migrate` (`src/save.rs:130-145`) runs after every load: it unions the default `unlocked_chars`, `unlocked_weapons` and `unlocked_planets` into the save (so new start-unlocked content appears in old saves) and sets `tutorial_done` when `runs_started > 0`. It never removes anything; re-gating Mars or the recruits later needs an explicit removal migration. There is no version number in the file.

### 13.6 Who may write the save

A client never banks a run (`bank_results.run_if(net::is_simulating)`, `src/main.rs:148-151`) and never writes stage clears (`src/netenemy.rs:771-773`), because its `RunState` is the host's. A joiner therefore earns no silver, counters or quest progress from co-op; settings and tome purchases still save locally.

---

## 14. Co-op netcode protocol

### 14.1 Architecture and roles

One machine, the **host**, is a listen server: it runs the full simulation for every astronaut (its own and each peer's) and plays at the same time. **Clients** send only input intent and their build, predict their own astronaut locally, and draw everything else from what the host streams. Auto-attacks remove the usual "did my shot hit" latency problem: the host decides every hit.

`NetRole` (`src/net.rs:42-58`) is a resource: `Solo` (default; no sockets), `Host`, `Client`. `simulates()` is true for Solo and Host; `is_networked()` is true for Host and Client.

| Concern | Solo | Host | Client |
|---|---|---|---|
| World simulation (AI, spawns, damage, loot, clock, interactions, upkeep, run end, banking) | yes | yes, for all astronauts | **no** (THE SWITCH, 14.13) |
| Own astronaut movement | yes | yes | yes, as unreconciled prediction |
| Weapon firing | yes | yes, for every astronaut | cosmetic only (hits are never applied) |
| Level-up cards | local | local (the host's own) | local (the joiner's own), reported via `PlayerBuildMsg` |
| Streams out | none | replication, run snapshot, crowd, boss, hazard, pickup, grants, identity | input, build |
| Streams in | none | input, build | everything else |
| `push_net_transform` / `push_player_vitals` | runs (harmless) | runs | off |

### 14.2 Transport and handshake

| Item | Value | Where |
|---|---|---|
| Stack | bevy_replicon 0.40.4 over bevy_replicon_renet 0.16.0 (renet 2.0.0, renet_netcode 2.0.0), UDP | `src/net.rs:20-28` |
| Security | `ServerAuthentication::Unsecure` / `ClientAuthentication::Unsecure`: no encryption, no tokens. Anyone who can reach the port and knows `PROTOCOL_ID` can join | `src/net.rs:1081,1114` |
| Port | `DEFAULT_PORT = 5011`; the host binds `0.0.0.0:5011` (CLI `--port` overrides; the menu always uses 5011) | `src/net.rs:37,1073` |
| Capacity | `max_clients = MAX_PLAYERS - 1 = 3`; slots 1-3 | `src/net.rs:38,1079` |
| Client socket | `0.0.0.0:0` (ephemeral) | `src/net.rs:1111` |
| Client id | Unix time in milliseconds at join | `src/net.rs:1115` |
| `PROTOCOL_ID` | `0xA570B0_2` = `0x0A570B02` = 173,476,610. netcode refuses a client with a different id | `src/net.rs:34-36` |
| replicon auth | Default `AuthMethod::ProtocolCheck`: the client sends a hash of the registered replication rules and message types (type names and registration order); on mismatch the server disconnects it; on match the client entity gets `AuthorizedClient` | bevy_replicon 0.40.4 `shared.rs:138-158`, `shared/protocol.rs:116-119` |
| Discovery | None. Direct IP only; the host's menu shows its LAN IP. No relay, no NAT traversal | `src/net.rs:1051-1060` |

Neither check hashes **field layouts**: two builds whose message structs differ can connect and then fail to deserialize. `PROTOCOL_ID` has not been bumped since co-op Stage 4 (commit `1e7d52f`) although seven message types were added after it. Rules: run byte-identical exes on every machine, and bump `PROTOCOL_ID` with every wire change.

Starting a session: `start_host` (`src/net.rs:1064-1094`) creates `RenetServer` with replicon's channel configs, binds the socket, creates `NetcodeServerTransport`, inserts both and sets `NetRole::Host`. `start_join` (`src/net.rs:1097-1130`) does the same with `RenetClient`/`NetcodeClientTransport` and sets `NetRole::Client` **before** the handshake completes. `disconnect` (`src/net.rs:1133-1139`) exists but is never called, so there is no way back to Solo without restarting; a failed join cannot be retried (the menu refuses a second attempt, `src/ui/menus.rs:749-752`).

### 14.3 Channels

replicon gives every registered message its own renet channel, appended after its own replication channels in registration order. Delivery modes: `Unreliable` (unreliable, unordered), `Unordered` (reliable, unordered), `Ordered` (reliable, ordered). "Independent" (`make_message_independent`) means the message is sent immediately instead of waiting for the next `ServerTick` and the replication of the tick it was written on; without it a server message is also dropped for a client that is not yet authorized.

| # | Message | Direction | Channel | Independent | Registered |
|---|---|---|---|---|---|
| 1 | `PlayerInputMsg` | client → host | Unreliable | n/a | `src/net.rs:352` |
| 2 | `PlayerBuildMsg` | client → host | Ordered | n/a | `src/net.rs:353` |
| 3 | `AssignPlayerId` | host → client | Ordered | no | `src/net.rs:362` |
| 4 | `RunSnapMsg` | host → clients | Unordered | yes | `src/net.rs:366-367` |
| 5 | `PickupEventMsg` | host → clients | Unordered | yes | `src/net.rs:377-378` |
| 6 | `XpGrantMsg` | host → clients | Unordered | yes | `src/net.rs:379-380` |
| 7 | `LootGrantMsg` | host → one client | Unordered | yes | `src/net.rs:381-382` |
| 8 | `HazardEventMsg` | host → clients | Unordered | yes | `src/net.rs:383-384` |
| 9 | `BossSnapMsg` | host → clients | Unreliable | yes | `src/net.rs:385-386` |
| 10 | `EnemySnapMsg` | host → one client | Unreliable | yes | `src/net.rs:387-390` |

The comment at `src/net.rs:363-365` sits on `RunSnapMsg` and calls it "registered LAST"; that is stale. `EnemySnapMsg` is registered last, which is what the comment's stated intent (the crowd should starve first when the link is tight) needs.

Serialization is replicon's default: serde through postcard. Integers are varints, `f32` is 4 bytes, `Vec` has a varint length prefix, and **enums are encoded by variant index**. So the variant order of `AstronautKind` and `WeaponKind` (inside `PlayerBuildMsg`) and of `HazardEvent`/`PickupEvent` is part of the wire format. The game's own wire codes (`planet_code`, `kind_code`, `boss_code`, the pickup kind byte, `LootGrantMsg.powerup`) are explicit matches so they do not shift if an enum is reordered.

**Targets.** The host never uses `SendTargets::All`: on a listen server it also writes the message into the host's own queue, so the host would apply its own snapshot or be "assigned" a client's id. It uses `SendTargets::CLIENTS_ONLY` or `SendTargets::Single(ClientId::Client(entity))` (`src/net.rs:475-477,493-495`).

### 14.4 Replicated components

Three components replicate per entity, on every astronaut (`Replicated` is inserted by `spawn_player`, `src/player.rs:167`; rules at `src/net.rs:348-350`):

| Component | Fields | Written on the host by | Read on the client by |
|---|---|---|---|
| `PlayerId` | `u8` | `spawn_player` | `spawn_remote_rigs` (skips its own id), `adopt_my_vitals` |
| `NetTransform` | `dir: Vec3`, `height: f32`, `facing: Vec3` | `push_net_transform`, every frame (`src/net.rs:1178-1186`) | `spawn_remote_rigs`, `drive_remote_transforms` |
| `PlayerVitals` | `hp`, `max_hp`, `level: u32`, `down: bool` | `push_player_vitals`, every frame (`src/net.rs:1188-1197`) | `adopt_my_vitals` (own id only). No teammate HP display exists |

Replication is sent on replicon's `ServerTick`, which runs in `FixedPostUpdate` (64 Hz by default; the game does not change `Time<Fixed>`), and therefore stops while the host's virtual time is paused. Both push systems mark the components changed every frame, so they are re-sent every tick.

### 14.5 Host-to-client messages

| Message | Fields | Sent by / rate / target | Applied by |
|---|---|---|---|
| `AssignPlayerId(u8)` | The client's slot | `announce_player_ids`, every 500 ms, `Single` to each connected **and authorized** client (`src/net.rs:478-491`). Repeated because a one-shot send before authorization can vanish | `receive_player_id` → `MyPlayerId` (`src/net.rs:736-744`) |
| `RunSnapMsg` | `run_seed: u64`, `stage: u8`, `chain: Vec<u8>` (planet codes Moon 0, Mars 1, DarkMoon 2), `timer`, `elapsed`, `total_elapsed`, `difficulty: f32`, `kills`, `gold_collected`, `silver_run: u64`, `static_active: bool`, `static_timer: f32`, `boss_spawned`, `boss_dead`, `teleporter_open: bool`. About 40-45 bytes of postcard payload (the seed alone is a 9-10 byte varint; the comment at `src/net.rs:98` estimates about 70) | `push_run_snapshot`, every 250 ms, `CLIENTS_ONLY`, **in any AppState** (`src/net.rs:496-517`) | `apply_run_snapshot` (14.17) |
| `EnemySnapMsg` | `seq: u16`, `chunk: u8`, `chunks: u8`, `anchor: [f32; 3]`, `n_spawn`, `n_update`, `n_despawn: u16` (per chunk), `data: Vec<u8>` (hand-packed records) | `stream_enemies`, 15 Hz, `Single` per client (14.8) | `receive_enemies` |
| `BossSnapMsg` | `bosses: Vec<BossRec>`; `BossRec { id: u16 (the boss's NetId), kind: u8 (boss code), dir: [f32; 3], hp_frac: f32, phase: u8, beam_angle: f32, beam_state: u8 }` | `stream_bosses`, 20 Hz, `CLIENTS_ONLY`, sent even when empty (14.9) | `receive_bosses` |
| `HazardEventMsg` | `events: Vec<HazardEvent>` (14.10) | `stream_hazards`, one message per host frame that spawned hazards, `CLIENTS_ONLY` | `receive_hazards` |
| `PickupEventMsg` | `events: Vec<PickupEvent>` (14.11) | `stream_pickups`, one message per host frame with events, `CLIENTS_ONLY` | `receive_pickups` |
| `XpGrantMsg(f32)` | Raw XP value of a collected gem | `relay_grants`, one per gem collected by anyone, `CLIENTS_ONLY` | `apply_xp_grant` |
| `LootGrantMsg` | `gold: u64`, `heal: f32` (`-1.0` means "food"), `powerup: u8` (0 none, 1 Damage2x, 2 Magnet, 3 Speed) | `relay_grants`, `Single` to the collector's client | `apply_loot_grant` |

### 14.6 Client-to-host messages

| Message | Fields | Sent by / rate | Applied by |
|---|---|---|---|
| `PlayerInputMsg` | `wish: Vec3`, `forward: Vec3`, `jump`, `slide`, `interact: bool`; about 27 bytes | `send_local_input`, **every rendered frame**. While the client's `RunPhase` is not `Playing` it sends a neutral intent (zero `wish`, false bits) instead of the stale one (`src/net.rs:749-786`) | `apply_remote_input` (14.14) |
| `PlayerBuildMsg` | `character: AstronautKind`, `level: u32`, `stats: Stats` (all 27 fields), `weapons: Vec<(WeaponKind, u32)>`; about 110 bytes | `send_player_build`, every 500 ms of the client's virtual time (so it pauses while the client has a panel open) (`src/net.rs:601-612`) | `apply_player_build` (14.16) |

### 14.7 The local grant relay

`pickup_update` knows nothing about the network. It writes the local message `GrantOut` (`src/net.rs:189-195`): `Xp(v)` for every collected gem, and `Loot(PlayerId, PickupKind)` when the collector is not the host's `LocalPlayer` (`src/pickups.rs:216,227-232`). On the host, `relay_grants` (`src/net.rs:660-694`) turns `Xp` into a `CLIENTS_ONLY` `XpGrantMsg` and `Loot` into a `LootGrantMsg` addressed through `PeerSlots::client_for(id)`. Silver is not relayed: it is run-global and rides `RunSnapMsg.silver_run`. In Solo the relay does not run and the messages are simply dropped.

On the client (`PreUpdate`, `src/net.rs:697-733`): `apply_xp_grant` calls `gain_xp(v)` on the local sheet if alive (the joiner's own `xp_gain` applies, and its own level-up panel opens); `apply_loot_grant` adds the gold amount as received. That amount is the **raw** pile value: `pickup_update` relays the unmultiplied `PickupKind::Gold(g)` (`src/pickups.rs:230`) while `collect` applies the peer's `gold_gain` only to the host-side copy (`src/pickups.rs:254-257`), so the joiner's own gold never includes any `gold_gain` (Doug, Golden Antenna, the Golden tome). It heals 20% for food, and sets a powerup with the durations duplicated from `src/pickups.rs:278-282`. Powerups never expire on a client, because the decay lives in host-only `player_upkeep`.

### 14.8 The crowd lane (enemy streaming)

A client cannot receive 1,200+ enemies as replicated entities, so the horde travels as a custom, quantized, interest-managed batch message (`src/netenemy.rs:1-28`).

**Ids.** `assign_net_ids` (`src/netenemy.rs:240-269`) gives every `Enemy` without a `NetId` a 15-bit id from `NetEnemyIds` (starts at 1, wraps back to 1, never 0; `src/netenemy.rs:59-96`). When an enemy disappears its id goes into a quarantine for `NET_ENEMY_GRACE + 0.5 = 2.5 s` before reuse, longer than the client's 2 s proxy grace, so a surviving proxy is never retargeted onto a new enemy. Live ids are tracked only from entities that already carry `NetId` (inserts are deferred commands; tracking the queued insert once released an id that was still about to be assigned and duplicated it). Pots and bosses get ids too; pots are never streamed and bosses use their id on the boss lane.

**Cadence.** `SnapClock.acc` accumulates virtual `dt`; when it reaches `1/15 s` a snapshot is built, `acc` resets to 0 and `seq` increments (wrapping `u16`) (`src/netenemy.rs:291-298`).

**Per client** (every `ConnectedClient` with `AuthorizedClient` and a slot):

1. **Anchor** = the direction of the host's copy of that client's astronaut (`PlayerId` from `PeerSlots`). Its tangent frame `(tan, bit) = tangent_frame(anchor)` is the quantization frame. The anchor itself is sent in the message, so the client decodes in exactly the frame the host used (`src/netenemy.rs:305-313`).
2. **Selection.** Skip pots and bosses. For each enemy compute `arc = arc_dist(e.dir, anchor, R)`. **Hysteresis**: an enemy not yet resident for this client enters at `arc <= IN`; a resident one stays while `arc <= OUT` (`src/netenemy.rs:324-338`).

   | Planet | IN (m) | OUT (m) |
   |---|---|---|
   | Moon | 95 | 107 |
   | Mars | 110 | 122 |
   | Dark Moon | 82 | 93 |

   (`src/config.rs:86-87`, indexed by `planet_index`, `src/netenemy.rs:1028-1035`.)
3. **Encoding** (`src/netenemy.rs:340-344`):
   ```
   c = clamp(e.dir · anchor, -1, 1)
   s = acos(c) · R                                 arc metres from the anchor
   τ = normalize(e.dir - anchor · c)               tangent direction toward the enemy
   u = s · (τ · tan) ,  v = s · (τ · bit)          arc-metre offset in the anchor's frame
   quantize(x) = clamp( (x + 128) / 256 · 65535, 0, 65535 ) as u16          step 3.9 mm
   ```
   `NET_ENEMY_RANGE = 128` must exceed every OUT radius (`src/config.rs:77`).
4. **Record kinds** (all little-endian):

   Spawn descriptor, 8 bytes, sent the first snapshot an enemy becomes resident:

   | Bytes | Content |
   |---|---|
   | 0-1 | `u16`: id (bits 0-14) with bit 15 = **elite** |
   | 2 | `kind_code` (Shambler 0, Sprinter 1, Bruiser 2, Spitter 3, UFO 4, Burrower 5, Beamer 6, Lobber 7, Ghost 8) |
   | 3 | Scale byte: `clamp((scale - 0.70) / 2.55, 0, 1) · 255`, so scale 0.70-3.25 in steps of 0.01 |
   | 4-5 | `quantize(u)` |
   | 6-7 | `quantize(v)` |

   Update, 6 bytes:

   | Bytes | Content |
   |---|---|
   | 0-1 | `u16`: id with bit 15 = **flash** (the enemy was hit; near band only) |
   | 2-3 | `quantize(u)` |
   | 4-5 | `quantize(v)` |

   Despawn, 2 bytes: the id (bit 15 clear).

5. **Near band and far tier.** Resident enemies within `NET_ENEMY_NEAR_ARC = 50 m` get an update every snapshot. Beyond that, a round-robin slice: `stride = 8` if the record budget is already spent, else `clamp(far_count / (1200 - records) + 1, 4, 8)`; an enemy is sent when `(id + seq) % stride == 0`, until `NET_ENEMY_MAX_RECORDS = 1200` records. Far updates never carry the flash bit. The 1200 cap bounds only the far tier: spawn descriptors and near-band updates are never capped. They do count toward `records`, so they shrink the far budget, and once they alone reach 1200 no far update is sent at all (`src/netenemy.rs:346-390`).
6. **Despawns.** Every id in the client's residency set that was not selected this snapshot (it died or left interest) gets a despawn record and leaves the set; every selected id joins it (`src/netenemy.rs:392-404`).
7. **Chunking.** Records are packed per chunk as spawns, then updates, then despawns, splitting only on record boundaries, with at most `NET_ENEMY_CHUNK_BYTES = 1024` bytes of payload (under renet's 1200-byte slice, so an unreliable chunk is never fragmented). Each chunk carries its own counts, plus `chunk` and `chunks` (`src/netenemy.rs:406-465`). All chunks of a snapshot share `seq` and `anchor`.

**Client decode** (`receive_enemies`, `src/netenemy.rs:1070-1201`): for each chunk, decode with the chunk's anchor: `s = hypot(u, v)`, `heading = normalize(tan·u/s + bit·v/s)`, `dir = offset_dir(anchor, heading, s, R)` (or the anchor itself when `s < 1e-4`). Spawn: if the id is new, spawn a proxy (`Enemy` with damage 0, the kind's mesh, the elite or kind material, decoded scale, wobble hashed from the id, `NetEnemy`, `NetId`, `StageScoped`) and index it. Update: set `target`, `last_seen`, and `flash = 1` if flagged; **updates for unknown ids are ignored**. Despawn: despawn and unindex. Receive statistics count records, chunks, bytes (payload plus an estimated 29 bytes per chunk) and sequence gaps (`src/netenemy.rs:1084-1092`).

**Client drive** (`drive_proxies`, `src/netenemy.rs:1206-1266`): reap any proxy unseen for more than `NET_ENEMY_GRACE = 2 s` (virtual time); ease `shown` toward `target` along the great circle with `k = 1 - exp(-12·dt)`, snapping when the error exceeds `NET_ENEMY_SNAP_ARC = 12 m`; derive gait speed from the **chord** between smoothed positions times the local radius (angles this small quantize to zero in f32), low-passed at rate 9; decay `flash`; face the nearest astronaut (the local body or a teammate rig); animate with the same `animate_crowd` the host uses. Nothing that is not needed to draw crosses the wire: facing, wobble and stride are derived.

**What this costs.** Measured at Stage 4: 84 of 90 mobile enemies streamed, about 4.6 KB/s at 15 Hz with no sequence gaps (`PROJECT_STATUS.md:72-76`); later commits quote 3.9-4.6 KB/s. It has not been measured at the 1,200 cap. `--enemydist` showed that essentially the whole mobile horde sits within 80 m of a player (spawned at 42-58 m, steering inward), so distance culling saves little for one player; interest management pays off because each client only needs the horde near itself, pots never cross, and the far tier is thinned.

**Failure modes** (section 22): spawn descriptors are unreliable with no acknowledgement, so a lost chunk leaves an enemy invisible until it leaves and re-enters interest (M8); a host pause longer than 2 s makes the whole resident horde invisible for the same reason (H6).

### 14.9 The boss lane

Bosses ride their own lane because `spawn_boss` sets `Enemy.kind = Bruiser` (a crowd record would draw THE CRATERPILLAR as a Bruiser) and because the HUD edge marker must point at a boss anywhere on the planet, so bosses are never interest-culled (`src/net.rs:133-140`).

- Host (`stream_bosses`, `src/netenemy.rs:862-887`): every 1/20 s of virtual time, every `Boss` with its `NetId`, `boss_code` (CraterpillarJr 0, RoverGoneWrong 1, Craterpillar 2, Anubot 3), exact `dir`, `hp / max_hp`, phase, and the Anubot beam angle and state. Sent even when empty: an empty list is how a client learns a boss died.
- Client (`receive_bosses`, `src/netenemy.rs:891-999`): updates known proxies (target, `last_seen`, `Enemy.hp = hp_frac`, phase). New ids spawn a proxy whose mesh is chosen from `BossKind` (worm head for both Craterpillars, the Anubot mesh, else the generic boss mesh), with `Enemy{max_hp: 1, damage: 0}` so the unchanged boss bar divides correctly, and `Boss` with **infinite** attack timers so the ungated-looking boss code never fires on the client. A Craterpillar proxy also gets a `CraterpillarHead` and 12 segments; the kept-on-client `craterpillar_update` grows the body from the head's trail at zero wire cost. Ids absent from a message are despawned.
- `drive_boss_proxies` (`src/netenemy.rs:1002-1026`) eases proxies like crowd proxies (rate 12, snap beyond 12 m), but places them `0.8 × scale` above the ground (the spawn height) with the plain tangent-frame orientation. On the host, `enemy_move` runs bosses through `animate_crowd` (`0.6 × scale` plus bob, facing the nearest astronaut, squash and waddle) and Anubot adds its rear-up and shudder, so a client's boss floats slightly higher, never turns to face anyone and does not animate.
- The beam fields arrive but are ignored: Anubot's Verdict Beam is invisible on clients, which still take its damage (H10).

### 14.10 The hazard lane

Every enemy shot, telegraph and mortar is fully determined by its spawn values plus time, and the client has the same planet and `sphere` math, so one event lets it integrate the whole flight (`src/net.rs:227-236`).

- Host (`stream_hazards`, `src/netenemy.rs:480-516`): `Added<EnemyProjectile>`, `Added<Telegraph>` and `Added<MortarShell>` become events, so no spawn site needs to know about networking and any new hazard using these components streams automatically.

  | `HazardEvent` variant | Fields |
  |---|---|
  | `Projectile` | `dir`, `heading: [f32; 3]`, `speed`, `life: f32`, `style: u8` (1 when `speed > 30`: the Beamer railbolt, drawn with the ring material and stretched) |
  | `Telegraph` | `dir: [f32; 3]`, `radius`, `max: f32`, `ring: bool` |
  | `Mortar` | `from`, `to: [f32; 3]`, `dur: f32` |

- Client (`receive_hazards`, `src/netenemy.rs:519-582`): spawns the same components with **damage 0**, a `NetHazard` marker and `StageScoped`; the kept-on-client integrators (`enemy_projectiles`, `telegraphs`, `mortar_shells`) animate them. Reliable delivery is deliberate: a telegraph is the tell for an attack that can kill.
- Not streamed: the Beamer aim line (`AimLine`) and the Anubot beam visual (`AnubotBeamVis`), so both are invisible on clients (H10).

### 14.11 The pickup lane

An idle pickup's transform is a pure function of `(dir, time, bob)` and the fly-to-collector phase can be derived, so only spawn and despawn cross the wire (`src/net.rs:197-210`).

| `PickupEvent` variant | Fields |
|---|---|
| `Spawn` | `id: u16`, `kind: u8` (0 XP, 1 gold, 2 silver, 3 food, 4 powerup), `value: u32` (XP as `f32` bits; gold or silver amount; powerup 1 Damage2x, 2 Magnet, 3 Speed), `dir: [f32; 3]`, `bob: f32` |
| `Despawn` | `id: u16` |

- Host (`stream_pickups`, `src/netenemy.rs:602-653`): every `Pickup` without a `PickupNetId` gets the next id from `PickupIds` (wrapping `u16`, skipping 0, no collision check) and a `Spawn`; a tracked entity that no longer exists yields a `Despawn`. `gem_merge` needs no special case: it is N despawns and one spawn.
- Client (`receive_pickups`, `src/netenemy.rs:657-715`): spawns a visual `Pickup` with the right mesh and material; `animate_net_pickups` (`src/netenemy.rs:722-762`) bobs it and flies it toward any nearby astronaut purely cosmetically. Collection, XP and loot are the host's and arrive as grants (14.7). `pickup_update` must stay host-only, or a joiner would collect locally and double its XP.

### 14.12 Identity and seating

- **Slots.** `PeerSlots` maps each connected client entity to a `PlayerId` in 1-3, claiming the lowest free id; slots are reused when someone leaves (`src/net.rs:306-330`). The host's own astronaut is always 0.
- **Seating** (`seat_joining_players`, `src/net.rs:883-951`): every frame the host reconciles the set of `ConnectedClient` entities against the astronauts that exist. A new client gets a slot and a body (`spawn_player(id, run.character, is_local = false)`, the **host's** hero until the peer's first `PlayerBuildMsg`, 3.5 m from the drop point). A seated client whose body is missing (for example after the first run) is re-embodied. Seating needs `CurrentPlanet`, `RunState` and `MetaSave` to exist; it is not state-gated, so after the first run it also seats peers while the host is in menus or results.
- **Unseating** (`src/net.rs:954-976`): slots whose client entity no longer has `ConnectedClient` are freed and that `PlayerId`'s astronauts despawned. Per-client `ClientResidency` entries are not removed.
- **Identity message.** replicon 0.40 exposes no local client id, and `PlayerId` alone is ambiguous on a client: the client's own predicted astronaut is spawned as `PlayerId(0)` with `LocalPlayer`, and the host's astronaut is also `PlayerId(0)`. So the host repeats `AssignPlayerId` every 500 ms and the client latches `MyPlayerId`. Until it arrives, the client draws no teammates at all (`src/remote.rs:74-81`).
- **Invariant.** A client holds exactly one `Player` and one `PlayerState` (its own). `--netlog` prints `local_players=` and `player_states=` to check this (`src/net.rs:856-866`).

### 14.13 THE SWITCH

Commit `6367aa8` ("Co-op Stage 9") turned off every world-simulating system on clients with `.run_if(net::is_simulating)` in `src/main.rs:185-288`. The complete split:

| System | Solo/Host | Client | Why kept or gated |
|---|---|---|---|
| `rebuild_hash` | yes | yes | Cosmetic weapons (homing shots, rockets, drones) query the hash, which then holds the proxies |
| `director_spawn` | yes | **no** | The host owns the horde |
| `enemy_move` | yes | **no** | Proxies carry a real `Enemy`; local AI would fight `drive_proxies` for the transform |
| `craterpillar_update` | yes | yes | Places the streamed worm's segments; its `PlayerHitMsg` is inert on clients |
| `anubot_beam_system`, `boss_phase_system`, `burrower_emerge`, `enemy_contact`, `spitter_attack`, `beamer_attack`, `lobber_attack`, `boss_attacks` | yes | **no** | Enemy behaviour and damage are the host's |
| `mortar_shells`, `enemy_projectiles`, `telegraphs` | yes | yes | Integrate streamed hazards (damage 0) |
| `gather_local_input`, `player_input`, `player_physics`, `animate_player` | yes | yes | Own-astronaut prediction |
| `weapon_fire`, `projectile_move`, `drone_update`, `beam_update`, `aura_follow` | yes | yes | Cosmetic on clients; their `HitMsg`s are never read |
| `charge_shrines`, `interact_system` | yes | **no** | Interactions are host-resolved (a joiner's E is a no-op) |
| `pickup_update`, `kill_drops`, `gem_merge` | yes | **no** | Collection and loot are the host's |
| `run_clock` | yes | **no** | The client adopts the clock from `RunSnapMsg` |
| `levelup_trigger` | yes | yes | Each machine picks its own cards |
| `debug_spawn_boss` | yes | yes | Ungated DEV key; on a client it spawns an inert local boss |
| `comet_system`, `dust_storm_system` | yes | **no** | Local-player systems; the joiner has no comet and no storm |
| `apply_hits`, `apply_player_hits` | yes | **no** | Damage resolution is the host's |
| `player_upkeep` | yes | **no** | Regen, shields, i-frames and powerup decay are host state; the client adopts HP |
| `stage_transition` | yes | (never triggered) | Only the host sets `PendingStage` |
| `downed_watch`, `death_watch` | yes | **no** | Run end is the host's call |
| `bank_results` | yes | **no** | A client must never bank the host's run into its own save |
| HUD, panels, camera, audio, particles, faders | yes | yes | Presentation |

What a client **does** run for the network: `apply_run_snapshot`, `receive_player_id`, `apply_xp_grant`, `apply_loot_grant`, `adopt_my_vitals`, `send_local_input`, `send_player_build`, `client_follow_host_run`, the netenemy client chain and the remote-rig systems. After THE SWITCH, `--netlog` on a client reports `local_sim=0`.

### 14.14 Input routing

`InputIntent` is the seam (`src/player.rs:66-77`): on the local machine `gather_local_input` fills it from the keyboard; on the host `apply_remote_input` fills a peer's from the wire; `player_input` consumes it identically for everyone, so movement code has no networking in it.

| Step | Rule | Where |
|---|---|---|
| Client gathers | Keyboard to the local `InputIntent` (edge bits true only on the pressed frame) | `src/player.rs:382-414` |
| Client sends | `send_local_input` runs after every intent writer (`gather_local_input`, `bot_input`) and before `player_input`, every frame; neutral while not `Playing` | `src/net.rs:433-443,749-786` |
| Host receives | `apply_remote_input`: for each `FromClient<PlayerInputMsg>`, look up the slot and **assign** `wish` and `forward`, **OR** in `jump`, `slide`, `interact` (several packets can arrive in one frame) | `src/net.rs:981-1017` |
| Host moves | `player_input` for the peer's astronaut, the same frame (the seat chain runs before it) | `src/net.rs:446-452` |

**Defect H4.** Nothing ever clears the OR-ed bits on the host, so after a joiner's first Space or slide press the host copy jumps on every landing (reaching the 2.1× cap) or slides every 1.1 s forever. The neutral intent sent during panels does not help, because OR-ing `false` clears nothing. Fix: reset the edge bits on the peer's intent each host frame before OR-ing, or after `player_input` consumes them.

**No reconciliation.** The client's predicted body and the host's copy of it are never reconciled. They even start 3.5 m apart (the client spawns itself as id 0 at `Vec3::Y`, the host seats it at an offset), and drift further with host pauses and hitstop, the H4 latch, non-expiring Speed, and frame-rate differences. The host copy is what takes hits, collects pickups and anchors interest management (H5).

### 14.15 Remote teammate visuals (client)

`RemoteVisualsPlugin` (`src/remote.rs:43-57`), InRun and Client only:

- `spawn_remote_rigs` polls (rather than reacting to `Added`) for replicated astronauts without a rig whose `PlayerId` is not `MyPlayerId`, and attaches `RemoteAstronaut`, a `Transform`, `Visibility` and the astronaut rig. The hero is not on the wire, so the suit colours come from `AstronautKind::ALL[pid % 12]` (a stable palette by slot). No `StageScoped`: the entity belongs to replicon.
- `drive_remote_transforms` eases toward `NetTransform` with `k = 1 - exp(-14·dt)` (`REMOTE_SMOOTH_RATE`), snapping beyond `REMOTE_SNAP_ARC = 8 m` or when nearly antipodal; derives speed from the chord of the smoothed direction times the local radius and `vel_r` from height changes; `grounded = height <= 0.02`.
- `animate_remote_rigs` runs the same `animate_rig` as the local astronaut. Slide state is not replicated, so teammates never tuck.
- `despawn_remote_rigs` (`OnExit(InRun)`, ungated) strips the rig and the `RemoteAstronaut` but leaves the replicated root to replicon.

### 14.16 Build sync

The host spawns a peer with a fresh level-1 sheet, the joiner picks cards on its own machine, and combat rolls use `thread_rng`, so the host cannot derive the peer's numbers; the client must report them (`src/net.rs:160-185`).

`apply_player_build` (`src/net.rs:619-657`) applies **build fields only** to the peer's host-side `PlayerState`: `character`, `level`, `stats` (the derived sheet; if `max_hp` grew, the difference is added to current HP so an upgrade does not look like damage; HP is clamped to the new max), and the weapon list (levels and membership from the client, cooldowns kept from the host copy so the heartbeat does not reset firing cadence). Never hp, shield, i-frames, powerups or `dead`, which are host-live state. The host's copy of the peer's `items` is left stale on purpose: nothing in the combat path reads items, and `recompute_stats` must never run on a peer sheet on the host because it would fold in the **host's** tomes.

### 14.17 Run sync and joining

`apply_run_snapshot` (`src/net.rs:521-555`) overwrites the client's `RunState` fields listed in 14.5 (chain only if non-empty), sets `RunSync.seeded`, and, once `RunSync.world_built` is set, raises `RunSync.pending_stage` when the snapshot's stage differs from the current one. `client_follow_host_run` (`src/main.rs:354-376`) moves a seeded client from `MainMenu`/`Boot` into `InRun`; `enter_run` then builds the world from the host's seed and chain. A client never enters a run on its own seed.

Join sequence: `start_join` (role becomes Client) → netcode handshake (the client entity gets `ConnectedClient` on the host, and the host seats it as soon as a `CurrentPlanet` exists) → replicon `ProtocolCheck` → `AuthorizedClient` → the host's snapshots, identity messages and streams reach it → the first `RunSnapMsg` sets `seeded` → the client enters `InRun`.

Defects in this path (section 22): `push_run_snapshot` sends even while the host is in its menus, so a joiner who connects then builds the placeholder or previous world and never rebuilds at stage 0 (H8); there is no run-end signal, so a client stays in `InRun` on an emptied planet when the host's run ends (H9); `RunSnapMsg` is unordered with no sequence number, so an older snapshot can briefly roll the clock back.

### 14.18 Stage-follow

A client cannot reach `stage_transition` (only the host's teleporter interaction sets `PendingStage`), so the stage edge in `RunSnapMsg` is its only signal. `client_stage_transition` (`src/netenemy.rs:775-840`), first in the client chain so every later decode uses the new planet's radius:

1. Take `pending_stage`; clone the local `PlayerState` (the build carries across).
2. Despawn every `StageScoped` entity (own astronaut and every proxy included) and clear the three id-to-entity indices, otherwise `receive_*` would refuse those ids for the rest of the run.
3. Reseed `GameRng`, build the new `CurrentPlanet` and scenery (`spawn_stage`), insert `PropColliders`.
4. Respawn the local astronaut as `PlayerId(0)` with the carried sheet; spawn interactables from a fresh sheet; insert `CurrentPlanet`; set `Playing`; banner "STAGE n+1 — NAME"; log `NET client rebuilt world for stage n` (`n` is the 0-based stage index).

It deliberately mirrors only the teardown and rebuild half of `stage_transition`: the victory branch and `counters.cleared` writes stay host-only. Chest and shop panels are not closed here.

### 14.19 Session lifecycle

| Event | What happens |
|---|---|
| Host clicks HOST CO-OP | `start_host` on 5011, status line, straight to hero select (`src/ui/menus.rs:258-274`). Clicking it before any run in the process should panic (H7, *code-derived*) |
| Joiner clicks JOIN CO-OP | IP overlay; CONNECT or Enter calls `try_join`; no state change until the seed arrives (`src/ui/menus.rs:741-766`) |
| Peer leaves | Its slot is freed and its astronaut despawned; a client that loses the host has no timeout handling beyond replicon's `ClientState` (logged) |
| Host run ends | Host goes to Results; clients get no signal (H9) |
| Role change | Impossible without restarting the process (`disconnect` is never called) |
| MAX_PLAYERS 4 | Supported by the slot table and spawn scaling; 3 and 4 players have never been run |

### 14.20 Wire-change checklist

1. Add or change the type in `src/net.rs` (`#[derive(Message, Serialize, Deserialize)]`).
2. Register it in `NetPlugin::build` with `add_server_message`/`add_client_message` and a `Channel`; add `make_message_independent` for server messages that do not depend on replicated entities.
3. Send with `CLIENTS_ONLY` or `Single`, never `All`.
4. Host sender: gate `is_hosting`, or `is_simulating` + `is_networked` in `EnemyStreamPlugin`. Client receiver: `is_client`, inside the client chain **after** `client_stage_transition`, or in `PreUpdate` after `ClientSystems::Receive`.
5. Client visuals: `StageScoped`, damage 0; clear any id map in both `clear_stream_indices` and `client_stage_transition`.
6. Use explicit wire codes for enums you encode by hand.
7. **Bump `PROTOCOL_ID`** (`src/net.rs:36`).
8. Test with two instances and `--netlog`; update this section.

---

## 15. Session log format

`PlayLogPlugin` (`src/playlog.rs`) writes a plain-text file a non-developer can send back after a playtest. It deliberately bypasses Bevy's tracing layer so it survives a panic.

| Item | Value | Where |
|---|---|---|
| File | `%APPDATA%/astrobonk/logs/session-<unix seconds>.log` (`./astrobonk/logs/...` without `APPDATA`); the directory is created if needed | `src/playlog.rs:33-38,54-60` |
| Opened | In `PlayLogPlugin::build`, before the app runs. The headless harness does not add the plugin, so smokes write no log | `src/playlog.rs:52-83` |
| Writes | `playlog::line(msg)` opens the file in append mode for every line, so a crash never loses a buffered tail. The path lives in a global `Mutex<Option<PathBuf>>`, so any code (and the panic hook) can log | `src/playlog.rs:31,42-48` |
| Collision | The name has one-second resolution: two instances started in the same second on one machine append to the **same** file | |

**Header** (written once):

```
=== ASTROBONK session log ===
version : 0.1.0
started : unix 1790467258
args    : target/release/astrobonk.exe --netlog

```

**Event lines** (written as they happen):

| Line | Source |
|---|---|
| `NET hosting on port 5011` | `start_host`, `src/net.rs:1092` |
| `NET joining <ip>:<port>` | `start_join`, `src/net.rs:1128` |
| `NET client state -> <Disconnected/Connecting/Connected>` | `report_connection` on every replicon `ClientState` change, `src/net.rs:1028-1035`. The client state exists in Solo too, so solo sessions log `Disconnected` once |
| `NET peers connected: <n>` | `report_connection` on the host or in solo when the peer count changes, `src/net.rs:1036-1044` |
| `NET assigned PlayerId <n>` | `receive_player_id` on a client, `src/net.rs:740` |
| `NET client rebuilt world for stage <n>` | `client_stage_transition`, `src/netenemy.rs:839`. `<n>` is the 0-based stage index (the on-screen banner shows `n + 1`) |
| `!!! PANIC !!!` block with the panic message and location | The panic hook, which then calls the previous hook, `src/playlog.rs:73-78` |

**Health line**, every 5 real seconds (the first sample is skipped), from `health_line` (`src/playlog.rs:88-133`):

```
[   35.2s] Host state=InRun me=None players=2 enemies=187 proxies=0 fps=182 hp=86/100 lvl=4 gold=12 stage=0 timer=571 kills=41
[   35.4s] Client state=InRun me=Some(1) players=1 enemies=64 proxies=64 fps=175 hp=92/100 lvl=4 gold=3 stage=0 timer=571 kills=41
[  530.4s] Solo state=Results me=None players=0 enemies=0 proxies=0 fps=240 stage=0 timer=252 kills=1525
```

| Field | Meaning |
|---|---|
| `[t]` | Real seconds since startup, right-aligned in 7 characters (`{now:7.1}`, `src/playlog.rs:117`) |
| role | `Solo`, `Host` or `Client` |
| `state=` | `AppState` |
| `me=` | `MyPlayerId` (`None` on the host and in solo) |
| `players=` | Entities with `Player` (host: every astronaut; client: 1; 0 outside a run) |
| `enemies=` | Every `Enemy`, so **pots, bosses and client proxies are included** |
| `proxies=` | Client crowd proxies (`NetEnemy`) |
| `fps=` | `1 / delta` of that single frame, not an average |
| `hp=`, `lvl=`, `gold=` | The local `PlayerState`, only when one exists |
| `stage=`, `timer=`, `kills=` | `RunState` (always present after boot; in menus it is the placeholder or the previous run) |

**Not logged:** the seed, the layout checksums, difficulty, items and weapons, comet cash-outs, shrine and chest events, seating, host stage changes, the cause of death, and results. When reading a log, look for flatlines (a proxy count stuck while `enemies=` moves on the host) rather than ratios.

---

## 16. Rendering and asset pipeline

### 16.1 Zero-asset policy

Nothing is loaded from disk: there is no `assets/` folder. The one `AssetServer` use is the toon ink shader, compiled into the binary with `embedded_asset!` (section 16.9). Meshes come from Bevy primitives (`Mesh::from(Sphere/Cuboid/Cylinder/Cone/Torus/Capsule3d)`) or the `meshkit` compositor; materials are plain `StandardMaterial`s with no textures; every sound is synthesized into an in-memory WAV (section 17); UI text uses Bevy's built-in default font. Colour comes from material base colours, vertex colours and emissive values driven through HDR bloom.

### 16.2 Camera and post-processing

| Item | Value | Where |
|---|---|---|
| Camera | One `Camera3d` for the whole process, marked `PlayerRig` | `src/main.rs:337-346` |
| HDR and bloom | `Hdr` component, `Bloom::NATURAL` at `BLOOM_INTENSITY` 0.1 (0.04 under flash reduction or photosensitivity) with a soft prefilter threshold (`BLOOM_THRESHOLD` 0.6), so only suns, stars, glows and flashes bloom | `setup_camera` in `src/main.rs`; `fx::apply_fx_settings` |
| Tonemapping | `AcesFitted` | same |
| Toon look (P36) | `toon::camera_bundle()`: `DepthPrepass` + `NormalPrepass` (read by the ink pass), `Msaa::Off` + `Fxaa`, `ShadowFilteringMethod::Hardware2x2` (crisp cel shadows), `ColorGrading`, `ToonInk`. See 16.9 | `src/toon.rs` |
| Field of view | Default perspective (π/4); widens ×1.09 while sliding, eased at rate 10 | `src/player.rs:762-770` |
| Clear colour | `ClearColor` is the stage planet's `PlanetDef.sky`, set by `toon::apply_world_look` whenever `CurrentPlanet` changes (the Moon's sky before any run) | `src/toon.rs` |
| Chase camera | Distance 7.5, height term `1.28` (`0.4 × CAM_HEIGHT`), pitch 0.12-1.25 rad (default 0.55), eased at stiffness 14 in real time; target clamped 1.2 m above terrain; looks at `player + 1.2·up + 2·forward`; aims from the unshaken position | `src/player.rs:747-824` |
| Screenshake | Positional only, applied after aiming: `trauma² × shake_scale` × a few sines along right and up. Trauma decays 1.6/s | `src/player.rs:817-823`; `src/fx.rs:7-20` |

### 16.3 Lights

| Light | Settings | Where |
|---|---|---|
| Ambient | `GlobalAmbientLight`: the night side's only light, re-graded per world from `PlanetKind::look()` (`night`, `night_brightness` 60-70) by `toon::apply_world_look`; dim on purpose so the night side is dark and the flashlight is the read | `src/content/planets.rs`, `src/toon.rs` |
| Sun | One `DirectionalLight` per stage, planet `sun` colour, 9,000 lux, shadows on, from a **fixed** direction (-0.55, 0.35, -0.75); there is no day/night rotation. A visible unlit sun disc sits 1600 m away | `src/planet.rs:477-500` |
| Flashlight | Every astronaut rig (local, host-side peers, and client teammate rigs) carries a `SpotLight`: warm white, intensity 6,000,000, range 55 m, inner 0.22 rad, outer 0.55 rad, **shadows on** | `src/player.rs:297-311` |
| Emissive glow | Visors, crystals, beacon lights, pickups, projectiles, telegraphs, particles, stars, Earth and the sun rely on emissive colour plus bloom | Section 16.5 |

### 16.4 `meshkit`: the mesh compositor

`MeshData` (`src/meshkit.rs:13-173`) accumulates positions, normals, vertex colours and indices from transformed primitives and bakes them into **one** `Mesh`, so a detailed model is still one draw, and all enemies of a kind batch into one instanced draw call.

| Primitive | Signature | Notes |
|---|---|---|
| `add_box` | `(size, tf, color)` | 6 faces × 4 vertices, faceted normals |
| `add_sphere` | `(r, subdiv, tf, color)` | Icosphere, smooth normals |
| `add_ellipsoid` | `(radii, subdiv, tf, color)` | Squashed icosphere with corrected normals |
| `add_cylinder` | `(r, h, seg, tf, color)` | Along +Y, with caps |
| `add_cone` | `(r, h, seg, tf, color)` | Apex up, flat-shaded sides, base cap |
| `add_capsule` | `(r, body_h, tf, color)` | Cylinder plus two spheres |
| `build` | `() -> Mesh` | Position, normal, colour attributes, `u32` indices |
| `at(pos)` | helper | `Transform::from_translation` |
| `icosphere(subdiv)` | `-> (Vec<Vec3>, Vec<u32>)` | Unit icosphere, `10·4^n + 2` vertices; shared by terrain, rocks and meshkit |

**Colour convention.** Vertex colour multiplies the material's base colour: pass `Color::WHITE` for body parts (the material colour shows fully) and darker greys for accents (visors, joints, undersides), so one material per kind still reads as a detailed model (`src/meshkit.rs:5-7`; `BODY`/`DARK`/`MID` at `src/enemies.rs:220-223`).

**Winding.** Every helper winds counter-clockwise seen from outside (Bevy's front face); `build_ccw` is now the same as `build`. Until P36 the ±X/±Y box faces and every cylinder and cone triangle were clockwise, so back-face culling drew those surfaces inside-out (M12). The unit test `meshkit::tests::every_shape_winds_outward` guards it (`cargo test meshkit`).

### 16.5 Materials

| Group | Settings | Where |
|---|---|---|
| Enemy per kind | Base colour from `EnemyDef.color`, emissive 0.15×, roughness 0.8. Ghost: alpha 0.55, `AlphaMode::Blend`, emissive 1.8× | `src/enemies.rs:373-401` |
| Elite | Orange (1.0, 0.55, 0.1), emissive (1.6, 0.7, 0.05) | `src/enemies.rs:406-411` |
| Hit flash | Unlit white, emissive 4 (swapped in by `enemy_flash` via `BaseMat`) | `src/enemies.rs:412-417,1650-1663` |
| Bosses | Generic purple emissive; worm metallic grey; Anubot sandstone gold | `src/enemies.rs:418-440` |
| Anubot beam | Charging: translucent unlit orange; firing: unlit red-orange, emissive 4 | `src/enemies.rs:441-454` |
| Enemy projectiles, rings | Unlit magenta emissive 2.4; unlit alpha-blended red-orange rings (also the railbolt) | `src/enemies.rs:455-469` |
| Weapons | Per weapon: unlit, alpha-blend, emissive 3× its colour. Aura bubble: alpha 0.12, emissive 0.6×, double-sided, no culling | `src/combat.rs:71-94` |
| Pickups | Unlit emissive: gem green (big gem blue), coin gold, silver pale blue, food red, powerup purple | `src/pickups.rs:53-70` |
| Particles | 7 unlit emissive colours (×2.5) | `src/fx.rs:103-129` |
| Terrain | White base, roughness 0.95, vertex colours in `TOON_TERRAIN_BANDS` flat height steps; shading normals leaned toward the radial (`TOON_TERRAIN_NORMAL_DETAIL`) so the cel bands follow the planet's curve | `planet_mesh`, `toon_height_band`, `toon_terrain_normals` in `src/planet.rs` |
| Scenery | Rocks (`ground_low`, darker variant), emissive crystals in the planet's `enemy_tint`, metallic wrecks and beacons, unlit red beacon lights, flora pairs per style (GlowShrooms emissive), unlit stars, emissive Earth, unlit sun | `src/planet.rs:172-500` |
| Astronaut | Per rig: suit (roughness 0.7), emissive visor, backpack, metallic tool, unlit lens (5 materials) | `src/player.rs:191-296` |
| Interactables | Per spawn: an emissive shape material and a darker pedestal material in the kind's colour; teleporter unlit alpha-blend; charge rings unlit alpha-blend cyan | `src/interact.rs:198-233,275-282,310-317` |
| Storm dome | Dusty orange, alpha 0.16, unlit, double-sided | `src/events_world.rs:63-71` |

### 16.6 The astronaut rig and animation

`build_astronaut_rig` (`src/player.rs:183-313`) builds the visual as children of the astronaut root, separately from `spawn_player`, so a client can put the same look on a replicated teammate without giving it `Player`:

| Child | Content |
|---|---|
| Body joint | Torso mesh (boxes, chest panel, neck, shoulder caps) and backpack (box + two tanks) |
| Head joint (rest y = 0.7) | Helmet sphere (r 0.3) and emissive visor |
| ArmL/ArmR joints (±0.34, 0.42), splayed ±0.12 rad | Arm mesh pivoting at the shoulder, hanging down −Y |
| LegL/LegR joints (±0.15, −0.3) | Leg mesh pivoting at the hip |
| Tool | Metallic cuboid in the right hand |
| Lens | Unlit glowing cuboid |
| Flashlight | Shadowed `SpotLight` (16.3) |

Per rig: 8 meshes and 5 materials are allocated (nothing is cached) plus one shadow-casting light.

`animate_rig` (`src/player.rs:617-706`) is shared by the local astronaut (`animate_player`, drive from physics) and teammates (`animate_remote_rigs`, drive derived from replicated motion). **Rule: every joint starts from its rest pose each frame; nothing accumulates onto the live transform.** Layers:

| Layer | Rule |
|---|---|
| Gait phase | Advanced by **distance**: `stride += speed·dt / 2.1 m · 2π`, so feet never skate |
| Gait amplitude | Eases (rate 9) toward `clamp(speed / 8.5, 0, 1.15)` when grounded, 0 in the air |
| Legs | Swing `sin(phase)·0.85·amp`, anti-phase left/right, lift on the swing half; tuck while sliding |
| Arms | Swing anti-phase to the same-side leg (0.62·amp); idle sway at rest |
| Body | Bob twice per cycle, waddle roll, forward lean with speed, breathing at rest; landing squash (0.07 s compress, then ease-out-back recovery to 0.25 s, depth from impact speed) and airborne stretch; volume-preserving scale |
| Head | Counter-bob, slight pitch, slow idle scan when standing |
| Slide | Tuck blend eased at rate 14; the root also leans 0.9 rad (`src/player.rs:586-587`) |

### 16.7 Crowd and boss animation

`animate_crowd` (`src/enemies.rs:1129-1193`) moves only whole transforms, so 1,200 enemies keep batching; it is shared by the host's horde and a client's proxies. Gait phase advances by distance (`stride += speed·dt·2.3`) plus a per-enemy `wobble`; walkers bob between footfalls (Sprinters bound in big hops; Bruisers stomp lower), fliers bob on two stacked sines and bank; walkers waddle-roll (Bruiser 0.34, Sprinter 0.12, others 0.26), lean into the chase and nod with the gait; a fresh contact hit adds a lunge pitch; squash and stretch preserve volume; a hit flash pulses scale by up to 25%. Each enemy faces the nearest astronaut.

Bosses: the Craterpillar's segments ripple (`sin(3.9t - 0.8i)`, lift `0.40·scale`, roll 0.26, yaw 0.13 a quarter-phase later, squash 0.12) (`src/enemies.rs:982-1005`); Anubot rears up (pitch 0.24, scale-Y +7%) while charging and lurches and shudders while firing (`src/enemies.rs:867-885`).

### 16.8 Effects

| Effect | Implementation | Where |
|---|---|---|
| Particles | `fx::burst` spawns `count` small cubes (not pooled, despite the module comment) with random velocity plus an up-kick, life 0.35-0.7 s, gravity 18 m/s² toward the core, shrinking; `StageScoped` | `src/fx.rs:132-186` |
| Hit flash | Material swap for `flash > 0` | `src/enemies.rs:1650-1663` |
| Faders | Melee sweeps (0.14 s) and chain zaps (0.12 s) shrink and despawn | `src/combat.rs:857-872` |
| Telegraphs | Torus rings scaled from 0.1 to the radius over the timer | `src/enemies.rs:1608-1626` |
| Hitstop | Virtual time at 6% for 0.18-0.25 s | `src/fx.rs:27-44` |
| Hurt vignette | Full-screen red with alpha `min(0.55·iframes, 0.4)` | `src/ui/hud.rs:379-381` |

---

### 16.9 The toon look (P36)

Locked direction #7. Two central mechanisms in `src/toon.rs` (`ToonPlugin`, windowed app only; presentation, never simulation, so it has no net gating and no wire path), so every mesh any system spawns is toon without touching a material site:

| Part | How | Tuning |
|---|---|---|
| Cel lighting | `patch_pbr_lighting` rewrites bevy_pbr's `bevy_pbr::lighting` shader module in `Assets<Shader>` once it has loaded (Bevy's hot-reload path recompiles every pipeline that imports it). Three anchored lines: the sun's and point/spot lights' N·L go through `toon_band` (none / mid / full), specular through `toon_spec` (a hard highlight or none), the spot cone through `toon_cone` (outer ring + core). Each anchor must match exactly once or nothing is patched and a `TOON lighting NOT applied` warning is logged; success logs `TOON lighting: bevy_pbr::lighting patched (3 sites)`. Unlit materials never run it. Pure fragment ALU: crowd enemies stay one instanced draw per kind | `TOON_BAND_*`, `TOON_SPEC_EDGE`, `TOON_CONE_*` in `config.rs` |
| Ink + rim | `InkNode`, a fullscreen render-graph pass between `Node3d::Tonemapping` and `Node3d::Fxaa` (`src/toon_outline.wgsl`, embedded). Silhouettes: second difference of reverse-Z depth relative to the pixel's depth (≈0 on any plane, however grazing). Creases: normal-prepass turn against the four neighbours. Rim: pixels within 2-3 line widths inside a silhouette whose normal faces the sun (`track_sun` feeds the sun direction, colour and Devoured-Sun dimming). Faded out between `TOON_INK_FADE` metres, so the starfield and sun disc never ink. Line width scales with window height; ×`TOON_INK_HIGH_CONTRAST` in the §13 high-contrast mode | `TOON_INK_*`, `TOON_RIM_STRENGTH` |
| Per-world grade | `PlanetKind::look()` → `ToonLook { ink, night, night_brightness, saturation, exposure }`; `apply_world_look` applies it with `PlanetDef.sky` as the clear colour whenever `CurrentPlanet` changes (enter_run, every stage transition, host and client alike). Exhaustive match: a new planet must pick a look | `src/content/planets.rs` |
| Terrain | Height colours in flat steps; shading normals leaned toward the radial (16.5) | `TOON_TERRAIN_*` |

Windowed checks: `--dev --warp noon|dusk|night` sets the local astronaut (host/solo) down under a high sun, just inside the terminator, or deep in the night, facing away from the sun (`dev_warp` in `src/main.rs`).

## 17. Audio and music synthesis

### 17.1 The synthesizer

Both banks are rendered at `Startup` into 22,050 Hz, mono, 16-bit PCM WAV byte buffers (`to_wav`: RIFF header plus samples clamped to ±1 and scaled by 30,000) and registered as runtime `AudioSource { bytes }` assets (the `wav` Cargo feature decodes them) (`src/audio.rs:51-72,79-153`; `src/music.rs:223-255`).

SFX voices use `tone(buf, wave, f0, f1, dur, amp, offset)`: a linear frequency sweep from `f0` to `f1` with envelope `(1 - t)^1.8`, mixed into the buffer at `offset` seconds. Waves: sine, square, saw, and noise from a 32-bit LCG (`src/audio.rs:19-49`).

### 17.2 SFX bank (16 sounds)

| `Sfx` | Recipe (wave f0→f1 Hz, duration s) | Played on |
|---|---|---|
| `Hit` | square 220→170, 0.07 | Melee swing, chain zap |
| `Crit` | square 460→320 + sine 920→640, 0.09 | Critical hit |
| `Hurt` | saw 130→70, 0.18 + noise 0.1 | Local player hit |
| `Pickup` | sine 760→1050, 0.07 | Gem, food |
| `Coin` | sine 1180 then 1560 | Gold, silver, comet quarter ticks, shop buy, tome purchase |
| `LevelUp` | sine arpeggio 523, 659, 784 | Level-up, powerup, cage |
| `Chest` | sine 392 then 523 | Chest open and take |
| `Evolve` | square 220→880 + sine 440→1760, 0.35 | Weapon evolution |
| `BossRoar` | square 65→45, 0.4 + noise 0.3 | Boss spawns, phases, slams, boss death, THE STATIC |
| `Click` | sine 1000→900, 0.035 | UI, tutorial line |
| `Pot` | noise 0.07 + sine 300→180 | Pot break |
| `Teleport` | sine 880→120, 0.32 | Teleporter, run launch |
| `Shrine` | sine 660→700 + 990→1020 | Shrines, Moai |
| `Slide` | noise 0.2 + sine 320→140 | Slide |
| `Bhop` | square 520→940, 0.09 | Successful bunny-hop |
| `Comet` | saw 220→55, 0.5 + noise 0.35 + sine 880→1760 | Comet cash-out |

(`src/audio.rs:87-150`; enum at `src/messages.rs:63-81`.)

`play_sfx` (`src/audio.rs:161-197`) plays each `SfxMsg` as a self-despawning `AudioPlayer` at volume `volume × sfx_volume`, throttled per sound in real time: `Hit` at most every 0.05 s, `Pickup` and `Coin` every 0.04 s, others every 0.02 s. Many sounds are played only for the `LocalPlayer`'s events (`is_local` checks), so a teammate's pickups are silent.

### 17.3 Adaptive music

A 32-second loop (`src/music.rs`): 120 BPM, 2-second bars, 16 bars, A minor. Chords Am, F, C, G (`ROOTS`, `TRIADS`), arranged as an 8-bar verse (Am F C G ×2) and an 8-bar chorus (C G Am F ×2) (`src/music.rs:42-51`).

| Stem | Content | Base level | Adaptive target |
|---|---|---|---|
| Bass | Square 8ths on the chord root, octave up on off-beats | 0.30 | 0.85 |
| Pad | Triangle triads, one per bar, slow attack | 0.11 | 0.65 |
| Arp | Saw arpeggio one octave up: 8ths in the verse, 16ths in the chorus | 0.11 | `0.2 + 0.6·density` |
| Lead | Saw hook over the chorus only, doubled 0.4% sharp for thickness | 0.16 + 0.08 | 0.7 |
| Drums | Kicks on beats 1, 3 and 4 (the third kick sits at 1.5 s into the 2 s bar, which is beat 4, although the code comment calls it the "and of 3"; `src/music.rs:195-199`), snares on 2 and 4, eighth-note noise hats, a crash every 4 bars | | `0.5 + 0.5·density` |
| Static | Two detuned saws at 55 and 55.6 Hz plus constant noise | 0.09 ×2 + 0.06 | 1.0 during THE STATIC, else 0 |

`start_music` (`OnEnter(InRun)`) spawns all six as looping players at volume 0, so they stay phase-locked; `stop_music` despawns them on exit (`src/music.rs:257-275`). `update_music` (`src/music.rs:277-320`) every frame in a run:

```
density = clamp(live Enemy count / 260, 0, 1)            (pots and proxies count)
target  = stem base target (table above)
          × 0.5    for every non-Static stem during THE STATIC
          × 0.35   while the RunPhase is LevelUp, Modal or Paused
          × volume × music_volume × MUSIC_MIX (0.12)
volume += (target - volume) · (1 - e^(-2.5·dt))           real time
speed   = 0.92 during THE STATIC, else 1.0                 (detunes everything flat)
```

The same track plays on every planet; there are no per-world songs.

---

## 18. UI structure

### 18.1 Conventions

The UI is Bevy UI (`Node`, `Text`, `Button`, `BackgroundColor`, `BorderColor`, `GlobalZIndex`), built in code. Helpers in `src/ui/mod.rs`: `txt(text, size, color)` bundle; `overlay_root()` full-screen centred column; `button_node()`; font sizes `FONT_BIG` 34, `FONT_MED` 20, `FONT_SMALL` 15; colours `PANEL_BG`, `CARD_BG`, `BTN_BG`, `BTN_HOVER`. `button_hover` recolours every `Button` on `Interaction` change to `BTN_HOVER`/`BTN_BG`, which also overwrites custom button backgrounds (`src/ui/mod.rs:55-64`).

Screens are rebuilt by despawning and respawning their tree (`MenuRoot`, panel roots) rather than by diffing. **Interaction passes through overlays**: a full-screen node does not block buttons behind it, which is why `main_menu_input` ignores every button except CONNECT/CANCEL while the join overlay is open (`src/ui/menus.rs:245-250`). The settings overlay has no such guard, so clicks reach the main menu underneath.

### 18.2 Screens

| Screen | Contents | Where |
|---|---|---|
| Main menu | Title, tagline, SILVER total, LAUNCH, HOST CO-OP / JOIN CO-OP, the co-op status line, DAILY (today's name and best), TOMES / QUESTS / SETTINGS / QUIT, a scrolling side panel (tome shop with loadout toggles and +1 buttons, or the quest list with progress) | `src/ui/menus.rs:119-415` |
| Join overlay | Address being typed (digits and dots, max 15 characters), hints, CONNECT, CANCEL | `src/ui/menus.rs:665-815` |
| Hero select | 12 cards (locked ones show "???" and the unlock hint), BACK | `src/ui/menus.rs:419-505` |
| Planet select | Moon and Mars cards only (Dark Moon is reachable only through the Moon T3 chain); tier buttons 1..`max_tier`, tier N enabled once `(planet, N-1)` is cleared; the T3 route line; BACK | `src/ui/menus.rs:509-604` |
| Results | PLANET SAVED or YOU GOT BONKED; bonks, level, gold, time; silver earned; the daily score; completed quests; CONTINUE | `src/ui/menus.rs:608-662` |
| HUD | Section 18.3 | `src/ui/hud.rs:60-306` |
| Card panel | Title (red "PICK ONE TO BANISH" in banish mode), up to 4 rarity-bordered cards, REFRESH/BANISH/SKIP for level-ups | `src/ui/panels.rs:46-256` |
| Chest panel | Item card with stats and catalyst hint; TAKE (-cost) and LEAVE | `src/ui/panels.rs:260-367` |
| Shop panel | Gold, 3 item cards with BUY prices or SOLD, WALK AWAY | `src/ui/panels.rs:371-497` |
| Pause | Stat summary, RESUME, SETTINGS, ABANDON RUN | `src/ui/panels.rs:501-586` |
| Settings | Master, music, SFX, mouse sensitivity, screen shake: −/+ steppers with a text bar and value; DONE; saves on every change | `src/ui/settings.rs:85-204` |

Z-order: settings `GlobalZIndex(50)` over pause (20) over card, chest and shop panels (10) over the HUD.

### 18.3 HUD layout

| Element | Position | Content |
|---|---|---|
| Hurt vignette | Full screen | Red, alpha from i-frames |
| Edge markers | Screen edge | Pool of 24 squares (below) |
| Dust haze | Full screen | Mars storm tint, alpha up to 0.4 |
| Timer | Top centre, 44 px | `M:SS`, or `THE STATIC M:SS` |
| Comet readout | Under the timer | `☄ xN` plus a 12-cell charge bar; `☄ COMET!` flash |
| Boss bar | Under the timer, middle 50% | Name and HP fill of the boss with the largest `max_hp` |
| Counters | Top right | BONKS, GOLD, SILVER +run, active powerups with seconds left |
| Banner | 22% from top | One message at a time, 2.6 s each, fading over the last 0.5 s |
| Tutorial line | 210 px from bottom | Mission Control text |
| Prompt | 150 px from bottom | `[E] ...` for the nearest interactable |
| Bottom cluster | Bottom, middle 64% | Weapon tray (3-letter tag and level per weapon, 2-letter tag and count per item), HP bar with text, XP bar, `LV N` |

Edge markers (`src/ui/hud.rs:446-529`): targets in priority order are bosses (red, 16 px), unused interactables in query order (teleporter green 16 px, chests gold 11 px, Shady Guys purple 11 px, the cage brown 12 px; shrines, Moai and microwave are skipped), then unfinished charge rings (cyan 11 px); the first 24 get markers. A target that projects inside the viewport gets none; otherwise its camera-space direction is clamped to a rectangle 34 px inside the screen edge (flipped when behind the camera). There is no horizon occlusion test.

Damage numbers (`src/ui/numbers.rs`): a pool of 64 text nodes; a hit within 1.6 m of a live hit number merges into its running sum (so swarm fights stay readable); each lives 0.7 s, rises and fades; crits are gold with "!", heals green "+N", dodges cyan "DODGE".

### 18.4 Controls

| Input | Context | Action | Where |
|---|---|---|---|
| W A S D | Playing | Move, relative to the camera | `src/player.rs:396-409` |
| Mouse | Playing | Yaw and pitch the camera (cursor locked and hidden) | `src/player.rs:783-788,827-839` |
| Space | Playing | Jump; bunny-hop on landing | `src/player.rs:411` |
| Left Ctrl or C | Playing | Slide | `src/player.rs:412` |
| E | Playing | Use the nearest interactable | `src/interact.rs:461` |
| 1-4 or click | Card panel | Pick a card | `src/ui/panels.rs:160-169` |
| R / B / S | Level-up panel | Refresh / banish mode / skip (+10 gold) | `src/ui/panels.rs:171-188` |
| 1 or E / 2 or Esc | Chest panel | Take / leave | `src/ui/panels.rs:328-339` |
| 1-3 or click / E or Esc | Shop | Buy / walk away | `src/ui/panels.rs:450-491` |
| Esc | Playing / Paused | Pause / resume | `src/ui/panels.rs:524-576` |
| Esc | Settings, join overlay | Close / cancel | `src/ui/settings.rs:117`; `src/ui/menus.rs:782` |
| Digits, period, Backspace, Enter | Join overlay | Edit the IP / connect | `src/ui/menus.rs:786-814` |
| Space or Enter | Results | Continue | `src/ui/menus.rs:653` |
| **B** | Playing | **DEV**: summon the stage boss | `src/enemies.rs:925` |
| **T** | Playing | **DEV**: replay the tutorial | `src/tutorial.rs:43-45` |

The tutorial text says "Shift to slide", but slide is Left Ctrl or C (`src/tutorial.rs:24`). S is both "move back" and "skip" (the panel only reads it while open). There is no gamepad support and no key remapping.

---

## 19. Performance notes and budgets

### 19.1 Targets and measurements

| Item | Value | Source |
|---|---|---|
| Design target (M1 exit) | 1,200 enemies at 60 fps on a mid GPU, sim tick decoupled from render | `GDD.md:1129` |
| Co-op target (M4 exit) | 2 players, full run stable at about 800 enemies over a home connection | `GDD.md:1131` |
| Measured, solo, release build | 165-190 fps while holding the 1,200 cap (level 25, 1,525 kills) | Session log of the 2026-09-26 playtest |
| Measured, crowd stream | About 3.9-4.6 KB/s per client at roughly 90 mobile enemies | Commit messages for Stages 4-6b; `PROJECT_STATUS.md:72-76` |
| Not measured | Frame time at the cap in co-op; host streaming cost at the cap; the terrain rebuild hitch; `SpatialHash` query cost; bandwidth at the cap; 3-4 players | |

Worst case for the crowd lane, if all 1,200 enemies sat in one client's 50 m near band: 1,200 × 6 B × 15 Hz ≈ 108 KB/s plus chunk headers, per client. The far-tier stride keeps the outer band cheap, but the record cap does not bound the near band (section 14.8).

### 19.2 Budgets built into the code

| Budget | Value | Where |
|---|---|---|
| Live enemies | `floor(1200 × P)`, P = 1 / 1.75 / 2.4 / 3.0 by party size; pots and bosses count; boss add rings bypass it | `src/config.rs:30`; `src/enemies.rs:568,592` |
| XP gems | 550, merged once a second (gold, silver, food piles are uncapped) | `src/config.rs:36`; `src/pickups.rs:375-404` |
| Damage numbers | Pool of 64, merged within 1.6 m | `src/config.rs:54`; `src/ui/numbers.rs:10-11` |
| Edge markers | Pool of 24 | `src/ui/hud.rs:89` |
| Crowd chunk | 1024 B payload (renet slice is 1200) | `src/config.rs:92` |
| Crowd rate | 15 Hz; boss lane 20 Hz; run snapshot 4 Hz; identity and build 2 Hz | Section 14 |
| Separation | At most 6 neighbours per enemy | `src/enemies.rs:1101-1103` |
| Weapon slots | 4 | `src/config.rs:51` |

### 19.3 Where the time goes

| Area | Cost | Where |
|---|---|---|
| Enemy rendering | One composited mesh and one material per kind, so the whole horde of a kind is one instanced batch. Elites use a second material; a hit swaps an enemy to the flash material for a moment; ghosts are alpha-blended | `src/enemies.rs:366-401,1650-1663` |
| Crowd animation | Whole-transform only; no skeletal animation on enemies | `src/enemies.rs:1129-1193` |
| Spatial hash | Rebuilt every frame from every `Enemy` transform. `near()` probes a full cube of cells: 27 for separation (×1,200 enemies), 3,375 for each homing projectile, rocket and comet tail query (radius 14), 9,261 for a comet cash-out (radius 22). Clients pay this for cosmetic weapons too | `src/enemies.rs:180-205,473-481`; `src/combat.rs:610`; `src/comet.rs:62,97` |
| Weapon targeting | `nearest_enemy` scans every enemy per weapon per shot (40 m); chains scan per link; auras, melee and beams scan every enemy per tick | `src/combat.rs:177-194,432-451,259-281,305-325,823-835` |
| Enemy AI | Per enemy: nearest astronaut over all players, one `step_toward`, separation | `src/enemies.rs:1019-1121` |
| Host streaming | `stream_enemies` walks every enemy once per client per snapshot with an `acos`, so cost is O(clients × enemies) at 15 Hz | `src/netenemy.rs:304-369` |
| Terrain | `icosphere(7)` (327,680 triangles) displaced and normal-smoothed synchronously on every stage entry, on host and client | `src/planet.rs:98-130` |
| Scenery entities | Hundreds of rocks, 420 star cubes, flora parents with children: all separate entities | `src/planet.rs:199-458` |
| Shadows | The sun plus one shadow-casting spotlight per astronaut rig (four players means four shadowed spotlights) | `src/player.rs:298-311` |
| Rig allocation | 8 meshes and 5 materials per astronaut spawn (every stage change respawns every astronaut); nothing is cached | `src/player.rs:183-313` |
| Particles | Not pooled: each burst spawns and despawns entities | `src/fx.rs:132-162` |
| UI | `update_hud` rewrites 8 texts every frame even when unchanged | `src/ui/hud.rs:336-372` |
| Pickups | Idle pickups rewrite their transform every frame | `src/pickups.rs:197-202`; `src/netenemy.rs:755-760` |
| Replication | `push_net_transform`/`push_player_vitals` mark components changed every frame, so they are re-sent every tick | `src/net.rs:1178-1197` |
| Client upstream | One `PlayerInputMsg` per rendered frame (about 180 per second at 180 fps) | `src/net.rs:749-786` |
| Clock | Variable `dt` everywhere: simulation cost scales with frame rate, and nothing is decoupled from rendering | `src/main.rs:184-333` |

### 19.4 Practical advice

- Profile in the **release** build; the dev profile optimizes dependencies but not the game code.
- Watch `enemies=` in the session log and `SMOKE` progress lines; remember both include pots.
- If the crowd lane needs to shrink: lower `NET_ENEMY_HZ` (the proxy smoothing rate matches 15 Hz), tighten `NET_ENEMY_NEAR_ARC`, or apply the record cap to the near band.
- If frame time at the cap becomes a problem: cache the hash query results per frame for homing projectiles, restrict `near()` to a sphere, and pool particles.

---

## 20. Bevy 0.18 gotchas this project hit

| # | Gotcha | Where it bit / where it matters | What to do |
|---|---|---|---|
| 1 | **A system takes at most 16 parameters** (`impl_system_function` is implemented for 0-16 params, bevy_ecs 0.18.1 `system/function_system.rs:954`). Exceeding it is a compile error with an unhelpful trait message | At the cap: `interact_system` (16, `src/interact.rs:394-411`). Bundled with tuples to fit: `main_menu_input` (14 with a 5-tuple; it was 18 before the `MenuBtn` enum and the tuple, `DEVLOG.md:84-85`, `src/ui/menus.rs:216-238`), `netenemy::log_stream_stats` (14 with a 6-tuple, `src/netenemy.rs:1269-1292`). Near it: `choice_input`, `shop_panel`, `stage_transition`, `comet_system` (13 each) | Bundle parameters into a tuple (a tuple is one parameter and may itself hold 16), as done at `src/interact.rs:402`, `src/netenemy.rs:784`, `src/playlog.rs:93-98`; merge marker components into one enum component (`MenuBtn`); or split the system. `interact_system` must be split before it can grow |
| 2 | **A `ParamSet` holds at most 8 parameters** (`impl_param_set` for 1-8, `system/system_param.rs:748`) | `update_hud`'s text `ParamSet` has exactly 8 queries (`src/ui/hud.rs:319-328`) | Adding a ninth HUD text needs a second `ParamSet`, disjoint queries with `Without<...>` filters, or one query over an enum marker |
| 3 | **B0001 query conflicts.** Two queries in one system that can touch the same component mutably on overlapping archetypes panic at startup (Bevy error B0001) | Enemies and astronauts both have `Transform`: every enemy query with `&mut Transform` needs the player query to exclude it (`Without<Enemy>` or `Without<Player>`), e.g. `src/enemies.rs:1023-1027,1201-1203,1333-1335,1527-1528,1613-1614`; the worm needed a `ParamSet` for head `&Transform` versus segment `&mut Transform` (`src/enemies.rs:946-950`, `DEVLOG.md:266-267`); the camera query excludes the player (`src/player.rs:755-756`) | Add explicit `Without<>` filters or use a `ParamSet`. Better, snapshot the other side into a `Vec` before the mutable loop (`AstronautSnap`, `src/player.rs:44-53`), which also avoids re-querying per enemy |
| 4 | **Cursor options are a component.** In 0.18 grab mode and visibility live in a `CursorOptions` component on the window entity, not in `Window` fields | `cursor_control` (`src/player.rs:827-839`) | Query `&mut CursorOptions` with `With<PrimaryWindow>` |
| 5 | **A missing non-`Option` `Res` panics.** Parameter validation fails as "invalid" (`system/system_param.rs:780-797`) and the default error handler is `panic` (`error/handler.rs:114-125`); the game never installs another handler | `CurrentPlanet` does not exist until the first `enter_run`, so `stream_enemies` (`src/netenemy.rs:284`) can panic when hosting before any run (H7). Also: `EntityCommands::insert` on a despawned entity panics (the `stream_pickups` race) | Take `Option<Res<T>>` for anything not inserted at startup, as the client chain, seating and `play_sfx` do; or insert a default resource at startup; or gate the system on the state that guarantees it |
| 6 | **Events are "messages".** 0.18 renamed buffered events: `#[derive(Message)]`, `MessageWriter`, `MessageReader`, `app.add_message::<T>()`. They are double-buffered | `src/messages.rs`; `src/main.rs:113-118` | Readers must tolerate stale entity ids (`src/combat.rs:985-988`). Any message a system writes must be registered in both the real and the headless app |
| 7 | **Commands are deferred.** A component inserted with `Commands` is not visible to queries until the next sync point | Net id bookkeeping duplicated ids until it tracked only entities that already carry the component (`src/netenemy.rs:251-257,639-641`) | Track state from what exists, never from what you just queued |
| 8 | **State hooks run in order.** `OnExit` commands are applied before `OnEnter` of the next state | `StageScoped` astronauts are already gone in `OnEnter(Results)`, which is the cause of H1 | Copy anything needed at results time into a resource before leaving `InRun` |
| 9 | **`Res<Time>` in `Update` is virtual time, and so is `on_timer`.** Pausing `Time<Virtual>` stops them | Every net timer stops during a host panel (6.3) | Use `Res<Time<Real>>` for anything that must run while paused |
| 10 | **Audio sinks need mutable access.** `AudioSink::set_volume`/`set_speed` take `&mut self`; volumes are `Volume::Linear(f32)`; a runtime `AudioSource { bytes }` needs the decoder feature (`wav`) | `src/music.rs:283,317-318`; `Cargo.toml:7`; `DEVLOG.md:231` | Query `&mut AudioSink` |
| 11 | **Moved or renamed APIs used here** | `bevy::post_process::bloom::Bloom`, `bevy::render::view::Hdr` (`src/main.rs:29,31`); `TransformSystems::Propagate` (`src/main.rs:317`); the `GlobalAmbientLight` resource (`src/main.rs:81`); `bevy::mesh::{Indices, PrimitiveTopology}`, `bevy::asset::RenderAssetUsages` (`src/planet.rs:6-7`); `despawn_related::<Children>()` (`src/ui/hud.rs:398`); `BorderColor::all(...)` (e.g. `src/ui/hud.rs:103`) and `border_radius` inside `Node` (`src/ui/mod.rs:43-52`); `Pickable::IGNORE` (`src/ui/hud.rs:72`); `EntityCommands::insert_if` (`src/player.rs:172`) | Check the 0.18 migration guide before copying older Bevy snippets |
| 12 | **UI interaction passes through overlays.** A full-screen node does not stop `Interaction` reaching buttons underneath | Join overlay click-through made one window both host and join (`src/ui/menus.rs:245-250`); the settings overlay still lets clicks through to the main menu | Guard handlers with the overlay's open flag, or give the overlay a blocking focus policy |
| 13 | **replicon on a listen server** | `SendTargets::All` also delivers to the host's own queue; a non-independent server message to a not-yet-authorized client is dropped; there is no local-client-id API (`src/net.rs:79-87,470-477`) | Use `CLIENTS_ONLY`/`Single`; mark stateless messages independent; send identity explicitly and repeatedly |
| 14 | **`std::collections::HashMap` iteration order is random per process** (it uses `RandomState`; `bevy::platform::collections::HashMap` uses a fixed-seed hasher instead) | Not a live determinism bug today: `SpatialHash` only does keyed lookups, and the netenemy id maps are `bevy::platform` collections (`src/netenemy.rs:44`). The one std map the simulation iterates is `want_drones` in `weapon_fire` (`src/combat.rs:232,533`), which only orders drone spawns | Do not iterate a std map where order matters; use `bevy::platform` collections, a `BTreeMap` or a sorted `Vec` |
| 15 | **Debug builds check integer overflow** | The crater-width hash panicked in dev until it used `wrapping_mul` (`src/sphere.rs:97`, `DEVLOG.md:332`) | Use `wrapping_*` in every hash |

---

## 21. How to add content and systems (step by step)

General rules for every change: never rename or reorder an enum variant that is saved (section 13.4) or sent (section 14.3); add every new simulation system to **both** `src/main.rs` and `src/headless.rs`; bump `PROTOCOL_ID` (`src/net.rs:36`) whenever a wire type or an enum it carries changes; run the four smoke paths (section 4.5) before committing. In the steps below, a bare `:line` refers to the file named at the start of that step (or of the previous step).

### 21.1 Add a weapon

1. `src/content/weapons.rs`: add a `WeaponKind` variant (`:6-43`). Append; do not reorder.
2. Add its `WeaponDef` arm in `def()` (`:100-491`): `name`, `desc`, `damage`, `cooldown` (orbit weapons use 0; aura weapons use it as the tick interval), base `projectiles`, `behavior` (one of the nine `Behavior` variants, `:46-65`), `color`, `evolves_to`, `evo_item`.
3. For a base weapon, add it to `WeaponKind::BASE` (`:81-98`) and update the array length. `BASE` drives NEW-weapon cards (`src/run.rs:530-536`), catalyst hints (`src/content/weapons.rs:512-517`) and `is_evolution` (`src/content/weapons.rs:507-509`).
4. For an evolution pair, add the evolved variant too, with `evolves_to: None, evo_item: None`, and point the base weapon's `evolves_to`/`evo_item` at it and at an existing item (an item may catalyse several weapons). The Evolve card then appears on its own once the base is level 7 and the item is owned (`src/run.rs:371-384`).
5. `src/combat.rs:37-70`: add **both** variants to the list in `setup_weapon_assets`. This is required: `weapon_fire` indexes `assets.mats[&kind]` (and `aura_mats` for auras), and a missing key panics on the first shot.
6. Make it obtainable: start-unlocked by adding it to `MetaSave::default` (`src/save.rs:65-79`; `migrate` adds it to old saves), a quest `Reward::UnlockWeapon` (21.6), or a hero's starting `weapon` (starting weapons need no unlock).
7. Only if no existing behaviour fits: add a `Behavior` variant, handle it in both matches of `weapon_fire` (the early one handles `Orbit`/`Aura`; the main one at `src/combat.rs:304-500` must get an arm), put any new component and its update system into chain E (`src/main.rs:215-228`) and into `src/headless.rs:268-284`, give spawned entities an `owner` and `StageScoped`, and write `HitMsg { source: Some(owner), .. }`.
8. Co-op: `WeaponKind` travels inside `PlayerBuildMsg`, so bump `PROTOCOL_ID`. Clients fire it cosmetically with no extra work.
9. Verify: `cargo build`; `--headless 2400 --hero <a hero that starts with it>`; play it.

### 21.2 Add an item

1. `src/content/items.rs`: add an `ItemKind` variant (`:6-29`), add it to `ALL` (`:41-64`, update the length), and add its `ItemDef` arm (`:66-247`): `name`, `desc`, `rarity`, `boosts: &[(StatKind, per-stack value)]`, `max_stacks`.
2. If it only boosts existing stats, you are done: items reach gameplay only through `recompute_stats` (`src/run.rs:260-265`), and the item automatically joins level-up cards, chests, shops, the Moai and shrine blessings (they all iterate `ItemKind::ALL`).
3. For a new stat: add a `StatKind` variant (`src/stats.rs:7-35`), a `Stats` field (`:38-66`), its default (`:68-100`), an `apply` arm (`:103-134`) and a `label` arm (`:148-179`); then read the field where it should act. `Stats` travels in `PlayerBuildMsg`, so bump `PROTOCOL_ID`.
4. To make it an evolution catalyst, set a base weapon's `evo_item`; the "Evo catalyst" line appears on cards automatically (`src/run.rs:499-507`).
5. Behavioural items (procs, on-kill, conditionals) have no hook system yet. Add code where the effect happens and read `PlayerState::item_count`. On the host a peer's `items` list is stale (build sync sends derived `Stats` only), so a behavioural item needs its own field in `PlayerBuildMsg`.
6. Items are not saved, so there is no save impact.

### 21.3 Add a hero

1. `src/content/characters.rs`: add an `AstronautKind` variant (`:6-20`), add it to `ALL` (`:52-65`, update the length), add its `AstronautDef` arm (`:67-215`: `name`, `agency`, `desc`, starting `weapon`, `passive`, `passive_desc`, `suit`, `visor` colours, `unlock_desc`), a `from_name` arm for `--hero` (`:218-235`), and decide `starts_unlocked` (`:239-244`).
2. A flat passive reuses a `Passive` variant (`:23-36`). A new variant needs an arm in `recompute_stats` (`src/run.rs:247-259`; the match is exhaustive).
3. A mechanic passive is a branch keyed on `character` in the relevant `PlayerState` accessor (`attack_speed`, `damage_mult`, `crit_chance`, `aura_scale`, `effective_armor_fraction`, `move_speed_mult`, `src/run.rs:278-337`) or in the system concerned (Yuki's slide, `src/player.rs:495-497`; Fortuna's refresh, `src/ui/panels.rs:117,192-196`). The host evaluates these for peers from `PlayerBuildMsg.character`.
4. Unlock it through `starts_unlocked` (then `migrate` adds it to every old save and can never take it away) or a quest `Reward::UnlockChar`.
5. Hero select lists `ALL` automatically (`src/ui/menus.rs:426`).
6. Co-op: `AstronautKind` travels in `PlayerBuildMsg`, so bump `PROTOCOL_ID`. Teammate rig colours come from `ALL[pid % len]` (`src/remote.rs:85`) and shift when the list grows.
7. Verify: `--headless 2400 --hero <name>`.

### 21.4 Add an enemy

1. `src/content/enemies.rs`: add an `EnemyKind` variant (`:4-14`), its `EnemyDef` arm (`:40-152`: `hp`, `speed`, contact `damage`, `xp`, `scale`, `color`, `hover` (> 0 makes a flier), `standoff` (> 0 makes it hold that arc and strafe)), and add it to the `mix()` windows (`:155-167`).
2. `src/enemies.rs`: add a mesh arm in `enemy_mesh` (`:226-298`, exhaustive) and add the kind to the list in `setup_enemy_assets` (`:373-383`). This is required: `spawn_enemy` indexes `assets.meshes[&kind]` and `mats[&kind]`. Add per-kind attack components in `spawn_enemy` (`:524-535`).
3. A special attack is a new component plus a system that snapshots living astronauts first (`AstronautSnap`), targets with `nearest_astronaut`, writes `PlayerHitMsg { victim, .. }`, and spawns hazards with the existing `EnemyProjectile`, `Telegraph` or `MortarShell` components (then they stream to clients with no extra code). Register it in chain D (`src/main.rs:185-214`) with `.run_if(net::is_simulating)` and in `src/headless.rs:246-267`. A ranged attack should respect `DustStorm.player_inside`.
4. Optional: a per-kind bob and waddle in `animate_crowd` (`src/enemies.rs:1153-1177`).
5. Co-op: add it to `kind_code` (exhaustive, so the compiler reminds you) **and** `kind_from_code`, which silently falls back to Shambler (`src/netenemy.rs:1039-1064`). Keep `scale × 1.65 × 1.1 ≤ 3.25`, the range of the spawn descriptor's scale byte (`src/netenemy.rs:351,1119`). A new hazard type needs a `HazardEvent` variant plus `stream_hazards` and `receive_hazards` arms. Bump `PROTOCOL_ID`.
6. Verify: `--headless 1200 --fast-boss` (the full late-game mix) and `--headless 2400`.

### 21.5 Add a planet

1. `src/content/planets.rs`: add a `PlanetKind` variant (`:5-9`), add it to `ALL` (`:48`), and add its `PlanetDef` arm (`:50-126`): `radius`, `hill_amp`, `rugged`, `craters`, `crater_depth`, a new terrain `seed`, the ground colours, `sun`, `enemy_tint` (crystals), counts (`rocks`, `crystals`, `pots`, `flora`), `flora_style` (a new `FloraStyle` needs arms in `spawn_stage`, `src/planet.rs:346-439`), `has_earthrise`. `sky` and `meteor_showers` are not read.
2. Chains: `chain_from` (`:131-138`) and `max_tier` (`:141-146`).
3. Menus: planet select iterates a fixed `[Moon, Mars]` (`src/ui/menus.rs:515`); add it there if it can be a starting planet. The headless `--planet` parser is at `src/main.rs:62-66`.
4. Unlock through `MetaSave::default().unlocked_planets` or a quest `Reward::UnlockPlanet`; add a `ClearXxx` quest if wanted (21.6).
5. Stage boss: `run_clock` maps planets to bosses (`src/director.rs:89-92`), as does the DEV key (`src/enemies.rs:929-932`); today everything but the Moon gets Anubot.
6. A signature event follows `src/events_world.rs`: a resource plus a system that returns early unless `planet.kind` matches; register it in set G (`src/main.rs:249-254`) and in the headless list. Make any "player is inside" logic per astronaut; the storm's local-only flag is a known co-op gap.
7. Co-op: `planet_code`/`planet_from_code` (`src/net.rs:559-574`), `planet_index` (`src/netenemy.rs:1028-1035`; it maps anything unknown to 2), and both `NET_ENEMY_INTEREST_IN`/`OUT` arrays (`src/config.rs:86-87`); keep OUT below `NET_ENEMY_RANGE` (128 m) and scale interest with the radius. Bump `PROTOCOL_ID`.
8. Save: `PlanetKind` names are stored in `unlocked_planets` and `counters.cleared`; never rename.
9. Verify: `--headless 2400 --planet <new>` after adding the parser arm; two instances with `--netlog --stagenow` and matching `layout_sum`.

### 21.6 Add a quest

1. `src/content/quests.rs`: add a `QuestKind` variant (`:7-24`), add it to `ALL` (`:43-60`, update the length), and add its `QuestDef` arm (`:62-83`): `name`, `desc`, `rewards` (`Silver(n)`, `UnlockChar`, `UnlockWeapon`, `UnlockPlanet`, `TomeSlot`).
2. `src/save.rs`: add its condition in `quest_met` (`:185-206`, exhaustive) and, for a counted quest, an arm in `quest_progress` (`:225-239`) so the quest list shows "(a/b)".
3. If it needs a new lifetime counter: **first add `#[serde(default)]` to `Counters`** (`src/save.rs:14`), or every existing save fails to parse and resets. Then add the field to `Counters`, add a per-run counter to `RunState` (`src/run.rs:101-133`, initialised in `new`), increment it in the host-side system where the event happens, and bank it in `bank_results` (`src/director.rs:283-294`).
4. Rewards are applied once by `apply_reward` (`src/save.rs:208-222`). Quests are checked only when a run is banked, never on clients.
5. The quest list iterates `ALL` automatically (`src/ui/menus.rs:399-411`); newly completed quests appear on the results screen.
6. Save: `QuestKind` names are stored in `quests_done`; never rename.

### 21.7 Add a simulation system

1. Put it in the right set in `src/main.rs` (section 7.2) and chain it if it depends on order within the frame.
2. If it changes world state, deals damage, rolls loot, changes `RunState` counters or writes the save, gate it `.run_if(net::is_simulating)`.
3. Mirror it in `src/headless.rs:246-301`.
4. Iterate every astronaut; use `.single()` with `With<LocalPlayer>` only for per-machine presentation.
5. Return early when `dt <= 0` (paused).
6. Snapshot astronauts before any `&mut` loop over enemies; add `Without<>` filters for any overlapping `Transform` access.
7. Give spawned entities `StageScoped`; use `Option<Res<CurrentPlanet>>` if it can run outside a run.
8. Count its parameters (16 maximum).
9. If clients need to see what it spawns, reuse a streamed component or add a lane (14.20).

### 21.8 Add a UI element or panel

- HUD element: a marker component in `src/ui/hud.rs`, spawned in `spawn_hud` with `Pickable::IGNORE` if it is not clickable, updated by a **new** system (the `update_hud` `ParamSet` is full).
- Modal panel: a resource with an `open` flag; open it by setting `RunPhase::Modal`; a `*_panel` system in set J that builds its tree (`GlobalZIndex(10)`), handles keys and clicks, and restores `RunPhase::Playing` on close; clear it in `clear_panels` (`src/main.rs:523-536`); make `dev_autopick` and the headless bot close it; beware E and Esc being handled twice.
- Menu button: add a `MenuBtn` variant and handle it in `main_menu_input` rather than adding a query (the system is near the parameter cap).

---

## 22. Known defects index

Confirmed by reading the code at `99d012f` (an independent audit, then re-checked for this document). Items marked *live* were also seen in a real session log; the rest are *code-derived*. IDs are stable so plans and commits can refer to them, and they match `docs/KNOWN_ISSUES.md`, which has the full write-up, repro and suggested fix for each (and numbers the low items individually).

### 22.1 High

| ID | Defect | Where | Notes |
|---|---|---|---|
| H1 | `bank_results` reads the local `PlayerState` after `OnExit(InRun)` despawned it, so level falls back to 1 and gold to 0: `best_level` stays 1, Results shows LEVEL 1 / GOLD 0, the payout loses its level term, and the Level20 quest is impossible (*live*) | `src/director.rs:274-280`; `src/main.rs:141-155` | Copy level and gold into a resource before leaving `InRun` (victory, death and abandon paths) |
| H2 | Taking a chest never increments `run.chest_opens` or `run.chests_opened`: the chest price never grows and Chests10 (and its RocketPod unlock) can never complete (*live*) | `src/ui/panels.rs:341-361`; `src/run.rs:121,124` | Regression from the co-op Stage 1 split (`35db5ff`) |
| H3 | Evolving never increments `run.evolves`; EvolveWeapon can never complete | `src/run.rs:608-617`; `src/ui/panels.rs:233-238` | Same regression |
| H4 | The host latches a peer's jump, slide and interact bits forever | `src/net.rs:1011-1013` | See 14.14 |
| H5 | The client's predicted astronaut is never reconciled with the host's copy, and they start 3.5 m apart | `src/main.rs:435`; `src/netenemy.rs:809-820`; `src/net.rs:920-931` | See 14.14 |
| H6 | A host panel or pause longer than 2 s makes the joiner's current horde permanently invisible (the client reaps unseen proxies; the host still believes they are resident and sends only updates) | `src/fx.rs:47-59`; `src/netenemy.rs:291-296,1177-1185,1238-1242` | `--autopick` hid it in every test |
| H7 | Hosting before any run in the process should panic: `stream_enemies` needs a `CurrentPlanet` that does not exist yet | `src/netenemy.rs:284`; `src/main.rs:446` | *Code-derived*. Use `Option<Res<CurrentPlanet>>` |
| H8 | A joiner who connects while the host is in its menus builds the wrong world, and no rebuild triggers when the host starts at stage 0 | `src/net.rs:394-399,531-533`; `src/main.rs:373` | Gate the snapshot on `InRun`, or rebuild on a seed/chain change |
| H9 | No run-end signal reaches clients; a client stays in `InRun` on an emptied planet | `src/net.rs:99-119`; `src/main.rs:151,284-285` | |
| H10 | Anubot's Verdict Beam and the Beamer aim line are invisible on clients, which still take their damage | `src/netenemy.rs:881-882,910-917,480-516` | The beam angle and state already stream |

### 22.2 Medium

| ID | Defect | Where |
|---|---|---|
| M1 | Interactable layout RNG can desync between machines (Shady Guy stock rolls consume a sheet-dependent number of draws) | `src/interact.rs:147,241-249`; section 12.3 |
| M2 | All 8 interactables are host-only and LocalPlayer-only and read E from the keyboard; shrine blessings go only to the host; the Magnet shrine pulls every pickup to the host | `src/main.rs:232-235`; `src/interact.rs:341,375,398,404,461,490-496` |
| M3 | The teleporter never exists on a client (placed with `thread_rng` from the party centroid; only `teleporter_open` is sent) | `src/director.rs:101`; `src/interact.rs:306-309` |
| M4 | Host panels, pause and hitstop freeze the world and every net timer for everyone; a charge ring completed by a peer opens a panel on the host | `src/fx.rs:27-59`; `src/interact.rs:375-386` |
| M5 | A joiner cannot pick a hero on a fresh launch; the host seats peers with the host's hero until the first build message | `src/main.rs:386-391,435`; `src/net.rs:928` |
| M6 | A joiner's powerups never expire (Speed stays ×1.5 in prediction; Magnet stays ×40) | `src/net.rs:728-731`; `src/player.rs:864-873` |
| M7 | A joiner earns no silver, counters or quest progress from co-op (intentional for now) | `src/main.rs:148-151` |
| M8 | Crowd spawn descriptors are unreliable with no acknowledgement; a lost chunk leaves enemies invisible until they re-enter interest | `src/net.rs:387`; `src/netenemy.rs:402-404` |
| M9 | `DustStorm.player_inside` tracks only the host's local player but silences ranged enemies against everyone; joiners never see the storm | `src/events_world.rs:109-117`; `src/enemies.rs:1284,1342-1350,1456` |
| M10 | `DustStorm` is never reset; on a second Mars visit in one session storms blow with no dome drawn | `src/events_world.rs:62-82`; `src/main.rs:406-449` |
| M11 | The Comet Combo exists only for the host's local player | `src/comet.rs:42,54`; `src/main.rs:249-254` |
| M12 | meshkit box sides, cylinders and cones appear wound clockwise from outside (inside-out shading under back-face culling; unobserved) | `src/meshkit.rs:44-51,83-116,126-153` |
| M13 | `Counters` lacks `#[serde(default)]`; any schema change resets the save; the write is not atomic and keeps no backup | `src/save.rs:14,118-121,154` |
| M14 | The DEV key B (boss summon) ships ungated: a progression shortcut in solo, an inert boss on a client | `src/enemies.rs:913-936`; `src/main.rs:244` |
| M15 | A teammate downed at the teleporter stays downed on every later stage; there is no revive | `src/director.rs:183-186,220-224` |
| M16 | Spawn slope and HP scaling are far from the GDD (about 15× the GDD's spawn slope; see 11.3) | `src/enemies.rs:570-576`; `src/content/enemies.rs:238-243` |
| M17 | No stage or tier scaling: `elapsed` resets each stage, so stages 2-3 restart at 1.0× with a Shambler-only mix | `src/director.rs:197`; `src/enemies.rs:563,610` |
| M18 | `SpatialHash::near` scans full cubes of cells (3,375 per homing projectile per frame, 9,261 per comet cash-out) | `src/enemies.rs:190-204`; `src/combat.rs:607-618` |
| M19 | ABANDON RUN leaves the PAUSED overlay (buttons included) over Results and the main menu until the next run | `src/ui/panels.rs:577-585`; `src/main.rs:523-536` |
| M20 | The HOSTING / LAN-address note is written in the same click that leaves the main menu, so it is never seen | `src/ui/menus.rs:265-270,164` |
| M21 | A failed or unreachable join cannot be retried without restarting (role set before the handshake; `disconnect` never called) | `src/net.rs:1124-1126,1133`; `src/ui/menus.rs:749-752` |
| M22 | Edge markers are missing for over-the-horizon targets that still project inside the viewport (no occlusion test) | `src/ui/hud.rs:488-499` |
| M23 | Joiner feedback is thin: no damage numbers, no hurt vignette (i-frames are not copied), no boss/Static banners or roar, no proxy hit-flash material | `src/ui/hud.rs:380`; `src/net.rs:592-593`; `src/netenemy.rs:1133-1159` |

### 22.3 Low (grouped)

| Area | Defects |
|---|---|
| Movement | W+Space hopping and sliding both reach the 2.1× cap (`src/player.rs:434-453`); jump and slide are sent only on the pressed frame over an unreliable channel (`src/player.rs:411-412`; `src/net.rs:352`); `player_input` and `player_physics` are unordered; coyote time is dead logic; `InputIntent.forward` and `.interact` are never read |
| World | `PlanetDef.sky` and `meteor_showers` unused and no `ClearColor` set; an interactable can spawn inside a boulder (`src/interact.rs:124-132`); the terrain rebuilds synchronously per stage; each rig allocates 8 meshes, 5 materials and a shadowed spotlight; Craterpillar Jr looks different on host and client (`src/enemies.rs:649-655` vs `src/netenemy.rs:923-926`); boss proxies have `max_hp` 1 so the boss-bar choice is arbitrary on clients (`src/ui/hud.rs:576-580`); buried burrowers can be hit by projectiles, drones and the comet |
| Combat | Owning any cryo weapon adds +0.25 slow to every hit and the per-weapon `slow` values are unused (`src/combat.rs:277,894-902,930-932`); the aura bubble understates the damage radius (`src/combat.rs:571,851` vs `src/combat.rs:260-265`); pots and bosses use cap slots; downed astronauts still count for party scale, spawn anchors, the boss centroid and charge rings, and still absorb enemy shots; boss HP has no party scaling and add rings bypass the cap; the record cap does not bound spawn or near-band records |
| Economy | Joiner gold ignores `gold_gain`; `greed_stacks` is not in `RunSnapMsg`; chests and the shop ignore `max_stacks`; the microwave is spent when nothing fits; `roll_item`'s fallback ignores bans (`src/interact.rs:104-110`); `static_secs_best` counts only the last stage; tutorial line 6 promises a chest drop that never happens and line 4 says "Shift"; Lady Fortuna's refreshes are unlimited; S is both "back" and "skip"; Shield, EliteDamage, Knockback and SilverGain are granted by nothing; gold, silver and food piles are uncapped |
| Netcode | A pickup collected in the frame it is first streamed can leave a ghost gem or panic on insert (`src/netenemy.rs:612-648`); `RunSnapMsg` is unordered with no sequence number; after the first run the host re-seats peers in Results and menus; ABANDON on a joiner leaves it dead until the host's next stage change; LAUNCH and DAILY are not blocked for a client; there is no teammate HP display; `push_net_transform` marks changes every frame; `PlayerInputMsg` is sent every rendered frame; `ClientResidency` leaks per departed client (`src/netenemy.rs:314`); the `u16` pickup id wraps without a collision check (`src/netenemy.rs:613`); `PROTOCOL_ID` has not been bumped since Stage 4; session log names have one-second resolution and seating is not logged |
| UI | `update_weapon_row` only watches weapons, so item changes and new runs with the same weapons show a stale tray (`src/ui/hud.rs:388-396`); E and Esc can be handled twice across panels; clicks pass through the settings overlay to the main menu (tome purchases included); `Selected.daily` can leak into a later HOST CO-OP run (`src/ui/menus.rs:258-271,286`); `button_hover` overwrites custom button colours; the main menu's SILVER header is static and a remembered `MenuTab` opens with an empty side panel; on clients shrine markers never clear and there is no teleporter marker |
| Dev and debt | T replays the tutorial in shipping builds; Mars and all six recruits start unlocked (re-gating needs a removal migration); the dev CLI ships in release and re-scans arguments every frame; the headless app duplicates the system list and its summary query is unfiltered (`src/headless.rs:317`); `--fast-boss` spawns both minibosses and the boss almost together; dead code (`Joint.lag`, `sphere::surface_radius`, `net::disconnect`, unqueried markers); `MetaSave` is cloned just to call `recompute_stats`; the Dark Moon reuses Anubot and only Mars has a planet event |

---

## Appendix A: tuning constants (`src/config.rs`)

| Constant | Value | Used for |
|---|---|---|
| `PLAYER_RUN_SPEED` | 8.5 m/s | Base run speed; also the gait normaliser |
| `PLAYER_ACCEL` | 55 m/s² | Acceleration with input |
| `PLAYER_FRICTION` | 38 m/s² | Deceleration without input |
| `PLAYER_AIR_CONTROL` | 0.35 | Acceleration factor in the air |
| `PLAYER_JUMP_VEL` | 8.0 m/s | Jump velocity (× √jump_height) |
| `PLAYER_GRAVITY` | 22 m/s² | Radial gravity |
| `PLAYER_HEIGHT` | 1.7 m | Transform offset and spawn height |
| `PLAYER_RADIUS` | 0.45 m | Prop collision and contact reach |
| `SLIDE_BOOST` | 1.65 | Slide speed multiplier |
| `SLIDE_TIME` | 0.85 s | Slide duration |
| `SLIDE_COOLDOWN` | 1.1 s | Between slides |
| `BHOP_WINDOW` | 0.16 s | After landing, speed is kept and friction skipped |
| `SPEED_HARD_CAP` | 2.1 | × run speed, the absolute cap |
| `CAM_DISTANCE` | 7.5 m | Chase distance |
| `CAM_HEIGHT` | 3.2 | Height term (×0.4) |
| `CAM_STIFFNESS` | 14 | Camera easing rate |
| `REMOTE_SMOOTH_RATE` | 14 | Teammate rig easing rate |
| `REMOTE_SNAP_ARC` | 8 m | Teammate rig snap distance |
| `CAM_SENS` | 0.0032 rad per unit of mouse motion | Mouse sensitivity (× the sensitivity setting) |
| `ENEMY_CAP` | 1200 | Live enemies per party-scale unit |
| `ENEMY_SEPARATION_CELL` | 2.2 m | Spatial hash cell and separation query radius |
| `SPAWN_ARC_MIN` / `MAX` | 42 / 58 m | Spawn distance band (over the horizon) |
| `CONTACT_TICK` | 0.5 s | Between contact hits from one enemy |
| `GEM_CAP` | 550 | XP gems before merging |
| `PICKUP_BASE_RANGE` | 3.2 m | Pickup radius (× pickup_range) |
| `PICKUP_FLY_SPEED` | 26 m/s | Pickup flight (cap is ×1.8) |
| `STAGE_SECONDS` | [600, 540, 480] | Stage lengths |
| `MINIBOSS_MARKS` | [420, 120] | Timer values for minibosses |
| `BOSS_MARK` | 90 | Timer value for the stage boss |
| `XP_BASE` / `XP_PER_LEVEL` / `XP_QUAD` | 6 / 3.4 / 0.18 | XP curve |
| `CHEST_BASE_COST` / `CHEST_COST_GROWTH` | 25 / 1.75 | Chest price (growth unused, H2) |
| `WEAPON_SLOTS` | 4 | Weapon slots |
| `MAX_WEAPON_LEVEL` | 7 | Weapon level cap and evolution threshold |
| `DAMAGE_NUMBER_POOL` | 64 | Damage number pool |
| `INTERACT_RANGE` | 3.0 m | Interaction range (+1.5 in `interact_system`) |
| `WAKE_RADIUS` | 14 m | Comet tail radius |
| `COMET_MIN_TAIL` | 8 | Tail needed to charge |
| `COMET_CHARGE_GOAL` | 900 | Charge to cash out |
| `COMET_RADIUS` | 22 m | Cash-out radius |
| `COMET_GRACE` | 0.7 s | Tail dip allowed |
| `SAVE_DIR` / `SAVE_FILE` | `astrobonk` / `save.json` | Save and log location |
| `NET_ENEMY_RANGE` | 128 m | Quantization half-range |
| `NET_ENEMY_HZ` | 15 | Crowd snapshot rate |
| `NET_ENEMY_NEAR_ARC` | 50 m | Every-snapshot band |
| `NET_ENEMY_INTEREST_IN` | [95, 110, 82] m | Interest entry (Moon, Mars, Dark Moon) |
| `NET_ENEMY_INTEREST_OUT` | [107, 122, 93] m | Interest exit |
| `NET_ENEMY_MAX_RECORDS` | 1200 | Far-tier record budget |
| `NET_ENEMY_CHUNK_BYTES` | 1024 | Chunk payload limit |
| `NET_ENEMY_SMOOTH_RATE` | 12 | Proxy easing rate |
| `NET_ENEMY_SNAP_ARC` | 12 m | Proxy snap distance |
| `NET_ENEMY_GRACE` | 2 s | Unseen proxy lifetime |

Other constants live next to their code: `WORM_SEGMENTS` 12, `WORM_STRIDE` 2, `WORM_TRAIL_STEP` 0.55 (`src/enemies.rs:63-65`); `BEAM_LENGTH` 34, `BEAM_WIDTH` 2.4 (`src/enemies.rs:86-87`); storm radius 24, active 26 s, gap 20 s, drift 3.2 m/s (`src/events_world.rs:27-30`); `MUSIC_MIX` 0.12 (`src/music.rs:18`); tutorial hold 5 s (`src/tutorial.rs:17`); number merge radius 1.6 m and life 0.7 s (`src/ui/numbers.rs:10-11`); `PROTOCOL_ID`, `DEFAULT_PORT` 5011, `MAX_PLAYERS` 4 (`src/net.rs:36-38`); `EliteMods` (`src/content/enemies.rs:229-235`).

---

## Appendix B: content tables

These tables summarise the content the systems above operate on. `docs/CONTENT_CATALOG_v0.1.md` is the exhaustive catalog.

### B.1 Heroes (`src/content/characters.rs`)

| Hero (`AstronautKind`) | Weapon | Passive | Unlocked |
|---|---|---|---|
| BUZZ (`Buzz`) | Wrench | +10% damage | From the start |
| VALENTINA (`Valentina`) | Laser Pistol | +15% attack speed | From the start |
| B0-NK (`B0nk`) | Rivet Gun | +0.5% crit per level | Quest ClearMoonT1 |
| YUKI (`Yuki`) | Kunai | Slide: +30% attack speed for 3 s | Quest ClearMarsT1 |
| CHIMP-O (`ChimpO`) | Boomerang Antenna | +1 jump | Quest FreeChimp (open the Moon cage) |
| DOUG (`Doug`) | Mining Laser | +25% gold gain | Quest Kill2500 |
| DR. RETICLE (`Reticle`) | Laser Pistol | +10% crit; guaranteed-crit pulse every 1.5 s | From the start (its "500 crits" unlock text is not implemented) |
| SLIPSTREAM NOVA (`Nova`) | Cryo Vent | +15% move; +1.3 attack speed while sprinting | From the start (unlock text not implemented) |
| OLD IRONCLAD (`Ironclad`) | Rocket Pod | +90 max HP; armor ×2 below 30% HP | From the start (unlock text not implemented) |
| LADY FORTUNA (`Fortuna`) | Boomerang Antenna | +30% luck; free, unlimited refreshes (+2 refresh charges) | From the start (unlock text not implemented) |
| AURORA PRIME (`Aurora`) | Static Cling | +20% size; auras ×1.35 while sprinting | From the start (unlock text not implemented) |
| SGT. GRISTLE (`Gristle`) | Sonic Whoopee | +15% damage; ×1.4 below 50% HP | From the start (unlock text not implemented) |

### B.2 Weapons (`src/content/weapons.rs:100-491`)

| Base weapon | Dmg | CD (s) | Proj | Behaviour | Evolves to (catalyst item) | Evolution: Dmg / CD / Proj / Behaviour |
|---|---|---|---|---|---|---|
| Wrench | 14 | 1.05 | 1 | Melee arc 150°, 3.4 m | MEGA WRENCH (Protein Paste) | 46 / 0.95 / 1 / melee 360°, 4.6 m |
| Laser Pistol | 10 | 0.75 | 1 | Shot 30 m/s, pierce 0, spread 5° | GATLING LASER (Overclocked CPU) | 9 / 0.11 / 1 / shot 38, pierce 1, 9° |
| Rivet Gun | 7 | 1.1 | 4 | Shot 26, pierce 0, spread 26° | RIVETER 9000 (Laser Sight) | 11 / 0.8 / 8 / shot 30, pierce 1, 40° |
| Kunai | 12 | 0.9 | 1 | Seek 24, pierce 0 | BLADE STORM (Caffeine IV) | 16 / 0.35 / 3 / seek 32, pierce 1 |
| Boomerang Antenna | 15 | 1.5 | 1 | Boomerang 22 m/s, 14 m | SATELLITE ARRAY (Golden Antenna) | 24 / 1.1 / 3 / boomerang 28, 20 m |
| Mining Laser | 8 per tick | 2.4 | 1 | Beam 16 m × 0.9 | DEATH RAY (Heavy Payload) | 18 / 1.8 / 1 / beam 26 × 2.2 |
| Orbital Drones | 11 | 0 | 2 | Orbit 3.4 m, 190°/s | DRONE SWARM (Splitter Chip) | 16 / 0 / 6 / orbit 4.4 m, 260°/s |
| Tesla Coil | 13 | 1.3 | 1 | Chain 3 jumps, 12 m, links 5 m | STORM CORE (Extra Battery) | 22 / 0.9 / 2 / chain 6, 15 m, 7 m |
| Rocket Pod | 26 | 2.1 | 1 | Rocket 14 m/s, AoE 3.6 | MIRV POD (Cursed Moon Rock) | 30 / 1.7 / 3 / rocket 17, AoE 4.6 |
| Cryo Vent | 5 per tick | 0.5 | 1 | Aura 4.2 m | ABSOLUTE ZERO (Fish Bowl Helmet) | 12 / 0.45 / 1 / aura 6.4 m |
| Meatball Comet | 22 | 1.6 | 1 | Rocket 13, AoE 3.4 | RAGÙ RAIN (Fish Bowl Helmet) | 30 / 1.2 / 3 / rocket 16, AoE 4.6 |
| Static Cling | 6 | 0.5 | 1 | Aura 3.8 m | FULL DISCHARGE (Thorn Plating) | 13 / 0.45 / 1 / aura 5.6 m |
| Ricochet Disc | 13 | 1.25 | 1 | Chain 4, 13 m, 5.5 m | THE OMNIDISC (Lucky Meteorite) | 18 / 0.95 / 2 / chain 9, 16 m, 7 m |
| Sonic Whoopee | 13 | 1.15 | 1 | Melee arc 210°, 4.0 m | THE BROWN NOTE (Rocket Boots) | 26 / 0.95 / 1 / melee 360°, 5.2 m |
| Cosmonaut's Bell | 9 | 0.9 | 1 | Aura 4.6 m | THE ANGELUS (Star Chart) | 17 / 0.8 / 1 / aura 6.2 m |
| Yo-Yo of Damocles | 12 | 0 | 2 | Orbit 2.9 m, 240°/s | SWORD-YO (Heavy Payload) | 19 / 0 / 5 / orbit 4.6 m, 300°/s |

Unlocked from the start: Wrench, Laser Pistol, Rivet Gun, Kunai, Boomerang, Mining Laser, Meatball Comet, Static Cling, Ricochet Disc, Sonic Whoopee, Cosmonaut's Bell, Yo-Yo (`src/save.rs:65-79`). By quest: Tesla (Kill1000), Drones (Pots50), Rocket Pod (Chests10), Cryo Vent (Shrines5). Rocket Pod and Cryo Vent are also Ironclad's and Nova's starting weapons.

### B.3 Items (`src/content/items.rs:66-247`)

| Item | Rarity | Per stack | Max | Catalyst for |
|---|---|---|---|---|
| Space Borgar | Common | +20 max HP | 9 | |
| Moon Cheese | Common | +12 HP/min regen | 9 | |
| Duct Tape | Common | +8 armor | 9 | |
| Slippery Visor | Rare | +8 evasion | 6 | |
| Protein Paste | Common | +10% damage | 9 | Wrench |
| Overclocked CPU | Rare | +12% attack speed | 6 | Laser Pistol |
| Fish Bowl Helmet | Rare | +12% size | 6 | Cryo Vent, Meatball Comet |
| Rocket Boots | Common | +8% move speed | 6 | Sonic Whoopee |
| Trampoline Soles | Common | +15% jump height | 4 | |
| Magnet Boots | Common | +25% pickup range | 6 | |
| Lucky Meteorite | Epic | +12% luck | 6 | Ricochet Disc |
| Space Credit Card | Rare | -10% chest and shop cost | 4 | |
| Golden Antenna | Rare | +15% gold gain | 6 | Boomerang Antenna |
| Star Chart | Rare | +10% XP gain | 6 | Cosmonaut's Bell |
| Laser Sight | Rare | +7% crit chance | 6 | Rivet Gun |
| Heavy Payload | Epic | +0.5× crit damage | 5 | Mining Laser, Yo-Yo |
| Extra Battery | Rare | +15% duration | 5 | Tesla Coil |
| Splitter Chip | Legendary | +1 projectile | 2 | Orbital Drones |
| Cursed Moon Rock | Epic | +15% difficulty, +10% luck | 5 | Rocket Pod |
| Thorn Plating | Rare | +12 thorns | 6 | Static Cling |
| Vampire Visor | Epic | +5% lifesteal | 4 | |
| Caffeine IV | Common | +15% projectile speed | 6 | Kunai |

### B.4 Enemies (`src/content/enemies.rs:40-167`)

| Enemy | HP | Speed | Contact dmg | XP | Scale | Hover | Standoff | Joins the mix at |
|---|---|---|---|---|---|---|---|---|
| Shambler | 14 | 3.1 | 5 | 1 | 1.0 | 0 | 0 | 0 s |
| Sprinter | 9 | 6.2 | 5 | 1.2 | 0.8 | 0 | 0 | 90 s |
| Spitter | 18 | 2.6 | 8 | 2 | 1.1 | 0 | 13 m | 180 s |
| Bruiser | 70 | 2.2 | 14 | 4 | 1.7 | 0 | 0 | 270 s |
| UFO | 24 | 4.5 | 7 | 2.5 | 1.0 | 4 m | 9 m | 360 s |
| Beamer | 26 | 2.8 | 14 | 3 | 1.3 | 0 | 22 m | 360 s |
| Burrower | 30 | 5.0 | 12 | 3 | 1.2 | 0 | 0 | 450 s |
| Lobber | 45 | 1.8 | 16 | 3.5 | 1.5 | 0 | 17 m | 540 s |
| Ghost ("The Static") | 20 | 5.4 | 10 | 0 (drops silver) | 1.1 | 0.6 m | 0 | THE STATIC only |

Bosses are in section 11.6.

### B.5 Planets (`src/content/planets.rs:50-146`)

| Planet | Radius | Rocks | Crystals | Pots | Flora | Earthrise | Stage boss | Chains |
|---|---|---|---|---|---|---|---|---|
| THE MOON | 140 m | 200 | 42 | 60 | 40 mineral spires | yes | THE CRATERPILLAR | T1 Moon; T2 Moon → Mars; T3 Moon → Mars → Dark Moon |
| MARS | 160 m | 250 | 52 | 68 | 64 rust thorn shrubs | no | JUDGE ANUBOT | T1 Mars only |
| THE DARK MOON | 105 m | 120 | 85 | 38 | 55 glowing mushrooms | no | JUDGE ANUBOT (placeholder) | Reached only through Moon T3 |

Tier N of a planet is selectable once tier N-1 of it has been cleared (`src/ui/menus.rs:536-538`).

### B.6 Quests (`src/content/quests.rs:62-83`; conditions `src/save.rs:185-206`)

| Quest | Condition | Rewards |
|---|---|---|
| First Contact (`Kill100`) | 100 lifetime kills | 20 silver |
| Pest Control (`Kill1000`) | 1,000 kills | 60 silver, Tesla Coil |
| Exterminator (`Kill2500`) | 2,500 kills | 90 silver, Doug |
| Solar Defender (`Kill10000`) | 10,000 kills | 250 silver, +1 tome slot |
| Pottery Critic (`Pots50`) | 50 pots | 40 silver, Orbital Drones |
| Cache Money (`Chests10`) | 10 chests (blocked by H2) | 50 silver, Rocket Pod |
| Devout (`Shrines5`) | 5 charge rings | 50 silver, Cryo Vent |
| Space Capitalist (`Gold5000`) | 5,000 lifetime gold | 80 silver |
| Overachiever (`Level20`) | Level 20 in one run (blocked by H1) | 60 silver |
| Ascension (`EvolveWeapon`) | Evolve any weapon (blocked by H3) | 100 silver |
| Cold Case (`FreeChimp`) | Open the Moon cage | 40 silver, Chimp-O |
| Signal In The Noise (`SurviveStatic2Min`) | 120 s in THE STATIC | 150 silver, +1 tome slot |
| One Small Bonk (`ClearMoonT1`) | Clear Moon tier 1 | 50 silver, B0-NK |
| One Giant Bonk (`ClearMoonT2`) | Clear Moon tier 2 | 120 silver, Mars (already unlocked by default) |
| The Full Tour (`ClearMoonT3`) | Clear Moon tier 3 | 300 silver |
| Red Planet Standing (`ClearMarsT1`) | Clear Mars tier 1 | 120 silver, Yuki |

### B.7 Tomes (`src/content/tomes.rs:37-56`)

| Tome | Stat per level | Max level |
|---|---|---|
| Tome of Damage | +2% damage | 20 |
| Tome of Health | +6 max HP | 20 |
| Tome of Agility | +1.2% move speed | 20 |
| Tome of Cooldown | +1.2% attack speed | 20 |
| Tome of Precision | +0.6% crit chance | 20 |
| Golden Tome | +2% gold gain | 20 |
| Tome of XP | +1.5% XP gain | 20 |
| Cursed Tome | +2% difficulty | 20 |

Silver cost of the next level: `round(8 · l · √l)` with `l = current level + 1`: 8, 23, 42, 64, 89 for levels 1-5, 253 for level 10, 716 for level 20, 6,089 in total for one tome from 0 to 20. Only tomes in the loadout apply (default Damage, Health, XP; 3 slots, up to 5 through quests).

### B.8 Rarity (`src/content/mod.rs:11-58`)

| Rarity | Colour | Base roll weight (luck `l`, clamped 0-3) | Shop price |
|---|---|---|---|
| Common | grey | `max(62 - 30l, 8)` | 30 |
| Rare | blue | `26 + 8l` | 60 |
| Epic | purple | `9 + 14l` | 120 |
| Legendary | gold | `3 + 8l` | 240 |

