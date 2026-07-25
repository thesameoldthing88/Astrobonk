# ASTROBONK — PROJECT STATUS

**Canonical status doc. Update this + DEVLOG.md at every handoff.**
Last update: 2026-07-25 (co-op stages 2a-2e: netcode de-risked, transport, replication,
input routing, remote player visuals. See "Co-op" below and the NETCODE NOTES block at the
bottom of src/net.rs, which is the detailed running log.)

## What this is
Megabonk-style 3D survivors roguelite on **tiny planets** (run all the way around;
horizon gently curved). Heroes are Earth astronauts, each with a signature auto-attack.
Rust / **Bevy 0.18.1**, zero external assets (procedural meshes + synthesized WAV sfx).
Full research digest + design: see `DESIGN.md`.

## State: v0.1 CORE COMPLETE — compiles clean, headless-smoke-tested, NOT yet human-playtested

### Works (verified by `--headless` smoke bot)
- Spherical-gravity player controller (WASD/jump/slide/bhop) + orbit chase cam
- Planet gen: icosphere terrain + analytic collision from one `sphere::Terrain` field —
  smooth hills, ridged mountain chains (mask-gated), rimmed craters; per-world flora
  (Moon spires / Mars thorns / Dark Moon glow-shrooms), rocks/crystals/stars/Earthrise/sun
- Weapon-mounted flashlight: shadowed SpotLight on a hand-tool prop; ambient dimmed so
  the planet's night side is genuinely dark (day/night texture while circumnavigating)
- Horde: director-driven spawns beyond the horizon, great-circle steering, spatial-hash
  separation, contact damage, 8 enemy types + elites, minibosses (7:00/2:00 marks),
  stage boss (1:30) with slam telegraphs + radial bursts, THE STATIC after 0:00.
  Ranged kinds (Spitter lob / UFO zap / Beamer aim-line railbolt / Lobber mortar AoE)
  use standoff AI: close to preferred range, then circle-strafe or back off
- Combat: **16 weapons + 16 evolutions** (melee arc, shots, seekers, boomerang, beam,
  orbit drones/yo-yo, chain lightning/disc, rockets/meatball, auras/bell), crits/overcrit,
  knockback, lifesteal, thorns, damage numbers, hit-flash, gem/gold/food/powerup drops, merging
- Run loop: XP → 4-card level-ups (rarities, Refresh/Banish/Skip), evolution cards,
  chests (pay-after-reveal), Shady Guy shop, charge/greed/magnet shrines, Moai,
  Microwave duplication, cage (Chimp-O unlock), pots + silver pots, teleporter →
  multi-stage chains (Moon T1/T2/T3 → Mars → Dark Moon), results banking
- Meta: silver, 16 quests (auto-grant unlock rewards), **12 astronauts** (6 founders +
  6 batch-1 recruits: Reticle/Nova/Ironclad/Fortuna/Aurora/Gristle), tome shop (8 tomes,
  loadout slots), planet/tier gating, save at `%APPDATA%/astrobonk/save.json` (auto-migrates)
- Menus: main (tomes/quests panels), char select, planet+tier select, results, pause
- Juice: screenshake, hitstop, particles, banners, boss bar, synthesized SFX

### How to run
```
cargo run                      # windowed game (dev profile is optimized enough)
cargo run -- --headless 2400   # smoke bot, ~80s sim, prints SMOKE OK/FAIL
cargo run -- --headless 1200 --fast-boss   # boss-path smoke
cargo build --release          # ship build
```
Controls: WASD move · mouse orbit · Space jump · Ctrl/C slide · E interact ·
1-4 pick cards · R refresh · B banish · Esc pause

## Co-op (in progress — host-authoritative listen server)
`bevy_replicon 0.40.4` + `bevy_replicon_renet 0.16.0`. Versions are PINNED: the latest of
either pulls Bevy 0.19 and you get two engines in one binary.

**Working and verified with two live instances:**
- Direct-IP transport, real handshake (`--host` / `--join <ip>` / `--port N`)
- Input routing: `InputIntent` is written either by the keyboard (local) or by the network
  (remote), so movement/physics/combat contain zero net code. A client predicts its own
  astronaut; the host applies the client's intent to the astronaut it owns.
- Replication down: `PlayerId`, `NetTransform{dir,height,facing}`, `PlayerVitals`
- Identity: host sends `AssignPlayerId` (Ordered, repeated 500ms, Single-target)
- Remote player visuals (`remote.rs`): teammates get the real rig, eased toward the
  replicated pose, animated by the same `animate_rig` the local player uses

**Not working yet — co-op looks right on the JOINER and is broken on the HOST.**
MEASURED: with one client joined the host holds `local_players=2 player_states=2`, and
~30 systems find the player with `.single()`, so they return `Err(MultipleEntities)` and
silently early-return — HUD, weapon fire, pickups, interaction, level-up panels, enemy
targeting. Two fail quieter still: Anubot's verdict beam and Craterpillar contact damage
just stop dealing damage. **This is the next co-op ticket.** The client is unaffected by
design (it keeps exactly one `Player`; remotes are visual only).

Still open after that: enemy streaming with interest management (1,200 enemies cannot
replicate per-entity — this is the real perf risk and could still change the design),
shared XP / revives / per-player-count scaling, Steam relay for click-to-join, and a lobby
UI. Nothing above needs port forwarding to TEST locally, but direct-IP does in the wild.

**Co-op dev harness:** `--autodrop` (skip menus), `--botinput` (walk without a keyboard),
`--netlog` (per-second pose/invariant dump). Two-instance repro:
```
astrobonk.exe --host --autodrop --botinput --netlog
astrobonk.exe --join 127.0.0.1 --autodrop --botinput --netlog
```

## Next steps (priority order)
1. **HUMAN PLAYTEST** — balance is smoke-bot-calibrated only. Watch: early spawn
   pressure (softened once already), wrench feel, camera pitch limits, gem drought.
2. Damage-number/UI pass on ultrawide + small windows.
3. The 4 unbuilt Megabonk-isms: golden chests, boss-curse shrine, challenge shrine
   (wave → chest), radios/music. (Challenge shrine component exists conceptually only.)
4. Music: lo-fi space drone loop (synth like sfx, or license-free track).
5. More content per DESIGN.md targets: +6 astronauts, +4 weapons, ~40 more items,
   more quests, Mars dust-devil event, Dark Moon final-boss uniqueness.
6. Perf audit at 1,200 enemies in release build (batching held up in dev profile).
7. Steam-shaped polish: settings (volume/sens), rebinds, gamepad, leaderboard-ish
   post-run stats.

## Architecture map (src/)
`sphere.rs` math · `planet.rs` worldgen · `player.rs` controller+cam · `enemies.rs`
horde/bosses · `combat.rs` weapons/damage · `pickups.rs` drops/magnet · `run.rs`
run-state/upgrade-rolls · `director.rs` timer/stages/results · `interact.rs`
chests/shrines/vendors · `content/` all data tables · `save.rs` meta ·
`ui/` hud/panels/menus/numbers · `fx.rs` shake/hitstop/particles · `audio.rs` synth ·
`headless.rs` smoke bot · `config.rs` tuning constants · `net.rs` co-op transport/
replication/input routing (+ NETCODE NOTES running log) · `remote.rs` drawing teammates

## Known quirks
- **The headless smoke is NOT deterministic, even with a fixed `--seed`.** Two runs of the
  same build diverge, so it CANNOT be used as a before/after oracle for a refactor. Verify
  refactors by inspection or targeted checks instead. (Determinism is worth fixing — co-op
  and the daily planet both want it.)
- Remote astronauts must NEVER get `Player`/`PlayerState` — see the header comment in
  `remote.rs` for why (silent `.single()` failures, stomped transforms).
- Pots piggyback on the `Enemy` component (hash membership) with `speed == 0` and are
  excluded from steering/targeting via `Without<Pot>` / pot checks — see combat.rs.
- Charge-shrine loot + Moai + Microwave reuse the level-up ChoicePanel (`is_levelup: false`).
- Boss is a scaled capsule; per-boss meshes/attacks are v0.2 work.
- `GlobalZIndex(10/20)` layers panels over HUD; menus rebuild side panel on click.
