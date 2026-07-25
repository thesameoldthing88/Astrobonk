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

- **Host correctness with 2 players** (Stage 3). 89 sites classified and converted:
  presentation → `With<LocalPlayer>`, simulation → per-player, enemies → nearest-by-arc.
  `PlayerHitMsg` carries a `victim`; `HitMsg` a `source`; projectiles/drones/beams/auras an
  `owner`. XP is a shared pool on individual curves, gold stays individual, horde scales
  1.0/1.75/2.4/3.0, and the run ends only when EVERY player is down.
  Repro: `--headless --coop2` (was: clock frozen, 0 kills → now level 6, 113 kills).

- **Enemy streaming — crowd lane** (Stage 4, `netenemy.rs`). Quantized, interest-managed
  batch stream: 6 bytes per enemy (position = great-circle offset from the receiving
  client's own astronaut, two u16 over ±128 m = 3.9 mm steps). Facing/wobble/stride are
  derived client-side, not sent. Measured live: 84 of 90 mobile enemies streamed, client
  draws 87 proxies, **4.6 KB/s**, zero sequence gaps.
- **Run-state replication** (Stage 5). Seed, planet chain, clock and counters at 4 Hz, so
  both machines build the *same* world. A client waits for the seed before entering the
  run. Verified by a prop-layout checksum matching exactly on both sides.

**MEASURED, and it corrects an intuition worth not re-learning:** run
`--headless --enemydist`. Excluding pots (which piggyback on `Enemy` with `speed == 0`),
essentially the ENTIRE mobile horde sits within 80 m of a player — they spawn at 42–58 m
and steer inward. There is no far-side horde to cull, so the close horizon buys much less
than it looks like it should. Interest management pays off through each client only needing
the horde near *itself*, not through distance culling per se.

**Known gaps (each deliberately left SAFE, not broken):**
- A peer gets no level-up card panel — they accrue levels and `pending_levelups` from
  shared XP, but picking a card needs a client↔host round trip. Nothing auto-picks.
- A peer cannot use interactables (chest/shop/shrine/teleporter). `interact_system` is at
  Bevy's exact 16-system-param cap, so the prompt/action split that would allow it does not
  fit; a peer pressing E is a no-op.
- **A client still runs its own local horde ALONGSIDE the streamed one** — you would
  currently see both. Stage 4 is deliberately additive. Still needed before "the switch"
  that turns the client's private simulation off: the boss/hazard lanes (bosses can't ride
  the crowd record — `spawn_boss` hardcodes a kind, and HUD edge markers need bosses
  streamed from anywhere on the planet, uncalled) and the pickups lane (or the joiner never
  gains XP). Gating before those exist leaves the joiner on an empty planet.
- The host simulates a peer's build as a **fresh level-1 sheet** — card picks never travel
  upward, so a joiner would deal level-1 damage all run. Needs a small client→host
  PlayerBuildMsg.
- A remote player standing in a dust storm is not yet hidden from ranged enemies
  (`DustStorm.player_inside` is local-only); needs a per-astronaut `InStorm` marker.

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
