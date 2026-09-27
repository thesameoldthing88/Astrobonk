# ASTROBONK — PROJECT STATUS

**This is the canonical status document. Update it and DEVLOG.md at every handoff.**

- **Last update:** 2026-09-26.
- **Code described:** `main` at **99d012f** (2026-07-26 01:23). In the working tree, src/, Cargo.toml and Cargo.lock match HEAD; only documentation has changed since. No code has been committed since 2026-07-26.
- **How this was checked:** a catch-up audit on 2026-09-26. Five subsystem readers each mapped part of src/, and a verifier re-checked every claim against the code. That day's build, headless smokes and the user's solo playtest log were also examined. Every `path:line` below was re-checked against the code at 99d012f. Where the older docs disagree with the code, the code wins.
- **Published:** the repository is public at https://github.com/thesameoldthing88/Astrobonk (`origin/main` = 99d012f).

---

## 0. The short version

1. **The solo game works end to end.** There are 12 astronauts, 16 weapons with 16 evolutions, 22 items, 8 tomes and 16 quests. There are 3 planets, played in chains of up to 3 stages (Moon → Mars → Dark Moon). Each stage has a boss arc, then THE STATIC. The build also has a daily seeded planet, a tutorial, procedural SFX and adaptive music. It uses zero external assets.
2. **2-player co-op works on one PC.** It is a host-authoritative LAN listen server on UDP 5011. It has never been verified on two machines. A joiner can move, fight, level up and follow stage changes. It **cannot** use any chest, shop, shrine or teleporter, and it **never learns that the run ended** (§5).
3. **Three solo save counters broke in co-op Stage 1 (35db5ff) and are still broken.** The best level is always saved as 1, chests are never counted, and evolutions are never counted. Three quests can never complete as a result: Overachiever (Level20), Cache Money (Chests10) and Ascension (EvolveWeapon). See §6, H1-H3.
4. **`target/release/astrobonk.exe` is stale.** It is about 4 minutes older than HEAD and lacks the fixes from 99d012f (§7). Rebuild it before handing it to anyone.
5. **A new creative direction was locked on 2026-09-26** (§2): one astronaut trying to get home to his pet turtle, the planets as the levels, 12 suits instead of 12 heroes, Hades-style persistence, a cartoon-spooky tone and a toon art style. **None of it is implemented yet.** It lives in plan/, and GDD.md is being rebuilt from it.
6. **What to do next:** follow plan/08_BUILD_ROADMAP.md. Until that roadmap orders otherwise, the audit recommends: first fix the solo regressions and rebuild release (Option A), then run the co-op pre-fixes and docket (Option B). See §9.

---

## 1. What the game is

ASTROBONK is a **third-person 3D survivors roguelite on tiny spherical planets**, in the style of Megabonk.

- **Weapons fire themselves. The skill is movement:** kiting along great circles, sliding, bunny-hopping and reading the terrain.
- **Each planet is a whole world you can run all the way around:**
  - Moon: radius 140 m (src/content/planets.rs:57).
  - Mars: radius 160 m (src/content/planets.rs:81).
  - Dark Moon: radius 105 m (src/content/planets.rs:105).
- The horizon curves visibly. The horde spawns 42-58 m away, over the horizon, and comes at you from every direction at once (src/config.rs:32-33).
- **A stage lasts 10:00, then 9:00, then 8:00** (src/config.rs:40):
  - minibosses at 7:00 and 2:00 (src/config.rs:41);
  - the stage boss at 1:30 (src/config.rs:42);
  - killing the boss opens a teleporter to the next planet in the chain (src/director.rs:98-103);
  - at 0:00, THE STATIC begins, an endless overtime swarm of `EnemyKind::Ghost` (src/content/enemies.rs:13).
- **Economy:**
  - XP gems level you up. Each level-up offers a 4-card choice with Refresh, Banish and Skip (src/run.rs:510; src/ui/panels.rs:160-183).
  - Gold buys chests and Shady Guy stock during a run.
  - Silver is the meta currency. It buys tome levels and is paid out by quests.
- **Tech:**
  - Rust 2021 on **Bevy 0.18.1** (Cargo.toml:7). Co-op uses **bevy_replicon 0.40.4 + bevy_replicon_renet 0.16.0** (Cargo.lock). These versions are pinned because the latest release of either crate pulls in Bevy 0.19, which would put two engines in one binary.
  - **Zero external assets.** Every mesh is composed from primitives in code (src/meshkit.rs). Every sound effect and the 6-stem adaptive music track are synthesized at startup (src/audio.rs, src/music.rs).
  - The target platform is PC. The GDD aims at Steam, and at Steam Deck plus controller support by 1.0.
- **Version:** `0.1.0` (Cargo.toml:3). src/ is **16,042 lines in 40 `.rs` files**.

**The game's fiction is changing.**
- In the current build and in the 2026-07 GDD, the player picks one of 12 separate Earth astronauts, each with a signature weapon and passive.
- GDD.md's golden thread is "Everything comes back around".
- §2 replaces that framing.

---

## 2. Creative direction — LOCKED by the user on 2026-09-26

This is the target for the full game. `plan/00_MASTER_PLAN.md` works it out in detail, and GDD.md is being rebuilt from plan/.
- Where the 2026-07 edition of GDD.md or DESIGN.md conflicts with this section, **this section wins.**
- **Nothing below is implemented in src/ yet.** The code still models 12 selectable heroes (src/content/characters.rs:5-20).

| # | Direction | What it means against today's build |
|---|---|---|
| 1 | **Improve on Megabonk's addictive traits in every way.** | Megabonk stays the benchmark, and every addictive loop should be matched and then bettered: the level-up dopamine, build-crafting, chest gambling, the quest unlock web, the boss-then-teleporter cadence, the final swarm and movement tech. The older "mirror rule" (DESIGN.md:242-243: do what Megabonk does unless the sphere offers better) becomes "do it better". |
| 2 | **The planets are the levels.** | Each planet is a level in a journey, not a map picked from a menu. Today a run is a Moon-started chain of up to three stages (src/content/planets.rs:131-136), picked from a planet and tier menu. |
| 3 | **ONE astronaut hero is trying to get home to his PET TURTLE.** His space suit **visibly upgrades from planet to planet.** | There is one protagonist and one emotional goal: getting home to the turtle. Progress should show on the character model, and the suit should look different on each planet. Today the rig is one hero-tier jointed astronaut tinted by the hero's `suit` and `visor` colours (src/content/characters.rs:46-47; `build_astronaut_rig`, src/player.rs:183). It has no upgrade visuals. |
| 4 | **The 12 existing astronauts become 12 SUITS**, keeping their signature weapons and passives. | The roster in §3.1 turns into a suit wardrobe for the one hero. Buzz's Wrench and +10% damage, Yuki's Kunai and slide frenzy, Nova's sprint-cooldown passive and the rest all carry over as suit kits. Unlocking a suit replaces unlocking a hero. |
| 5 | **A roguelite journey: death sends you back to the crash site. Suit upgrades and story persist (Hades-style).** | Death is not a full reset. You restart at the crash site, and your suit upgrades and story progress carry over, so each attempt moves the journey forward. Today death leads to Results. There, Silver, quest progress and unlocks are banked (src/director.rs:271-346), and the next run starts from the menus. |
| 6 | **A cartoon-spooky tone: funny AND frightening, rated E10+.** | Keep the meme humour already in the content (hero blurbs, weapons like Sonic Whoopee and Meatball Comet) and add real fright: dread, dark sides of planets, and THE STATIC. Stay inside an E10+ rating. |
| 7 | **A toon art style.** | Move from the current look toward toon rendering. Today that look is vertex-coloured primitives under PBR `StandardMaterial`, ACES tonemapping and bloom (src/main.rs:337-345). The zero-external-assets rule is not revoked by this direction. |

**What carries over unchanged from the current build:**
- the tiny-planet sphere mechanics (src/sphere.rs), and movement as the skill;
- the horde and boss arc per stage;
- co-op as a pillar. The GDD calls it "the killer differentiator" and "never the cut" (GDD.md:862, 1150).

---

## 3. Current state (code at 99d012f)

**State: v0.1. The solo core is complete and has been played by a human many times. 2-player LAN co-op works on one PC but has not been proven on two machines. The solo save counters have regressed (§6).**

The earlier status line "NOT yet human-playtested" is obsolete. The user has played the game repeatedly:
- playtests #1-#4 (DEVLOG Session 1b);
- the feedback that made the worm "read as floppy" (7a69e42);
- the first real co-op playtest on one PC, which found the host-and-join click-through (8194240);
- a solo release-build run on 2026-09-26 (§3.4).

What has never happened: a structured balance playtest, and a co-op session across two machines. A coworker playtest on two PCs was planned for 2026-07-27. Nothing in the repo shows whether it happened.

### 3.1 Content that exists in code

| Area | Count | What | Where |
|---|---|---|---|
| Astronauts (to become **suits**, §2) | 12 | **Founders:** Buzz, Valentina, B0-NK, Yuki, Chimp-O, Doug. **Recruits:** Dr. Reticle, Slipstream Nova, Old Ironclad, Lady Fortuna, Aurora Prime, Sgt. Gristle. Each has a signature weapon and a passive. **The founders' passives:** Buzz +10% damage, Valentina +15% attack speed, B0-NK +0.5% crit per level, Yuki +30% attack speed for 3 s after a slide, Chimp-O +1 jump, Doug +25% gold. **The recruits' passives were upgraded from flat stats to mechanics (Sessions 2c-2d):** Reticle has a guaranteed-crit focus pulse every ~1.5 s, Nova's weapons barely cool down while she sprints, Ironclad's armor doubles below 30% HP, Fortuna's refreshes are free, Aurora's auras swell +35% while she sprints, and Gristle gets +40% more damage below half HP. **Unlocks:** 8 start unlocked, including all 6 recruits (`starts_unlocked`, src/content/characters.rs:237-244). B0-NK, Yuki, Chimp-O and Doug are quest unlocks. | src/content/characters.rs:5-20, 66-213, 237-244 |
| Weapons | 16 base + 16 evolutions | They are built on 9 behaviours: MeleeArc, Shot, Seek, Boomerang, Beam, Orbit, Chain, Rocket and Aura. An evolution needs the weapon at level 7 (`MAX_WEAPON_LEVEL`) plus its catalyst item. No per-run evolution cap is enforced. | src/content/weapons.rs:6-43, 46-65; src/config.rs:52; src/run.rs:371-384 |
| Items | 22 | These are stat items only. There are no proc or death-save items and no cursed-item mechanic: Cursed Moon Rock is a plain stat item (+15% Difficulty, +10% Luck; src/content/items.rs:215-222). | src/content/items.rs:6-29 |
| Tomes | 8 | Damage, Health, Agility, Cooldown, Precision, Golden, Xp and Cursed. The loadout starts with 3 slots, and quests raise it to at most 5. | src/content/tomes.rs:5-14; src/save.rs:89, 220 |
| Quests | 16 | Quests grant Silver and unlocks. Three of them cannot complete today because of H1-H3: Overachiever (Level20), Cache Money (Chests10) and Ascension (EvolveWeapon) (§6). | src/content/quests.rs:66-81 |
| Enemies | 8 + The Static | Shambler, Sprinter, Bruiser, Spitter, UFO, Burrower, Beamer, Lobber and Ghost (THE STATIC). Elites run on a timer and use flat multipliers. | src/content/enemies.rs:4-14 |
| Bosses | 2 minibosses + 2 stage bosses | **Minibosses:** Craterpillar Jr and Rover Gone Wrong, on every planet. **Stage bosses:** THE CRATERPILLAR (Moon), a segmented worm; JUDGE ANUBOT (Mars), with the rotating Verdict Beam. Both have 3 phases, escalating at 66% and 33% HP. **The Dark Moon reuses Anubot** (src/director.rs:89-92). | src/content/enemies.rs:17-22; src/enemies.rs:628, 720, 798, 941 |
| Planets | 3 | Moon (140 m, terrain seed 7), Mars (160 m, seed 23) and Dark Moon (105 m, seed 66). **Chains:** T1 is Moon only; T2 is Moon → Mars; T3 is Moon → Mars → Dark Moon. Mars played on its own is a single stage. | src/content/planets.rs:57-110, 131-136 |
| World events | 1 | The migrating dust storm on Mars. Inside it, ranged enemies cannot see you. | src/events_world.rs |
| Interactables | 8 kinds + charge shrines + pots | Chest (pay after the reveal), Shady Guy shop, Greed shrine, Magnet shrine, Moai, Microwave, Cage (unlocks Chimp-O) and Teleporter. There are also 5 charge shrines per stage, plus pots and silver pots. | src/interact.rs:26-35 |

### 3.2 Systems that exist in code

- **Movement**
  - A spherical-gravity controller: WASD, jump, slide on Ctrl or C, a bunny-hop window of 0.16 s, 35% air control, and a speed hard cap of 2.1x run speed (src/config.rs:3-16; src/player.rs:382-617).
  - An orbit chase camera with a persistent, parallel-transported world-space forward vector (src/player.rs:747).
  - Terrain is analytic. One field drives the mesh, collision and spawns (src/sphere.rs), and the terrain mesh is an icosphere at subdivision 7 (src/planet.rs:98-130).
  - Props have colliders **for the player only** (`PropColliders`); enemies walk through rocks.
  - Lighting: a fixed sun (src/planet.rs:480) and a shadowed flashlight SpotLight on the rig (src/player.rs:299). The planet's far side is dark, but there is no moving day/night terminator.
- **Horde**
  - `director_spawn` (src/enemies.rs:539-626) spawns in batches every 0.25 s. The rate per second is `(1 + 2.1·t_min)·(1 + difficulty)·P`, or `(10 + 0.15·static_timer)·P` during THE STATIC (src/enemies.rs:568-576).
  - The cap is `floor(1200·P)` (src/config.rs:30). P is 1.0, 1.75, 2.4 or 3.0 for 1-4 players.
  - A spatial hash handles separation (src/enemies.rs:181).
  - Ranged enemies use standoff AI.
  - Telegraphs mark the boss slam rings and mortar landing discs.
- **Combat**
  - `weapon_fire` (src/combat.rs:212), `apply_hits` (src/combat.rs:877) and `apply_player_hits` (src/combat.rs:973).
  - Crits and overcrits, knockback, lifesteal, thorns and hit-flash.
  - A pool of 64 damage numbers that merge nearby hits (src/config.rs:54; src/ui/numbers.rs).
- **Comet Combo**, the signature scored move: run with a tail of at least 8 enemies within 14 m to build charge, then cash out in a detonation of radius 22 m (src/comet.rs; src/config.rs:58-63).
- **Pickups:** XP gems (capped at 550 and merged by `gem_merge`), gold, silver, food and powerups (src/pickups.rs).
- **Run loop and meta**
  - Level-up cards with Refresh, Banish and Skip. Evolution cards show their catalyst hint.
  - Chests, the Shady Guy, shrines, the Moai and the Microwave.
  - The teleporter chains stages, and results are banked at the end of a run.
  - Silver, quests, the tome shop, planet and tier gating.
  - The save is JSON at `%APPDATA%/astrobonk/save.json` (src/save.rs:107-112). `migrate` merges newly default-unlocked content into old saves (src/save.rs:130-145).
- **Daily seeded planet:** the same Moon T1 world for everyone that day, with a codename like "GRIEF-7B" and a best score (src/run.rs:46-63; `MenuBtn::Daily`).
- **Onboarding:** 6 diegetic radio lines from Mission Control on a new player's first normal run (src/tutorial.rs:20-27).
- **Menus**
  - The main menu has LAUNCH, DAILY, TOMES, QUESTS, SETTINGS, QUIT, **HOST CO-OP** and **JOIN CO-OP**. JOIN opens an address overlay pre-filled with 127.0.0.1 (src/ui/menus.rs:49-63, 140-163).
  - The other screens are character select, planet plus tier select, results and pause.
  - The **settings overlay** (master, music and SFX volume, mouse sensitivity, screen shake) opens from both the main menu and pause (src/ui/settings.rs).
- **HUD:** hp, XP, timer, level and gold; a weapon row; the boss bar; the comet meter; the dust-storm haze; a 24-slot pool of off-screen edge markers (src/ui/hud.rs:485); and banners.
- **Juice:** screenshake; hitstop, which slows virtual time to 0.06x (src/fx.rs:40); particles; bloom with ACES tonemapping.
- **Audio**
  - 16 synthesized SFX (src/messages.rs:64-81; src/audio.rs:89-150).
  - One 32 s, 16-bar adaptive synthwave song with 6 stems: bass, pad, arp, lead, drums and static. Stem volumes follow enemy density and the run phase, and THE STATIC detunes the mix by playing it at 0.92x speed (stems at src/music.rs:247; the Static check at :288, 318).
- **Session logs:** written to `%APPDATA%/astrobonk/logs/session-<unix>.log`, with a health line every 5 s and a panic hook (src/playlog.rs).
- **Co-op:** see §5.
- **Headless smoke harness** (src/headless.rs): see §4.3.

### 3.3 In the GDD but not built

- **Movement techs:** Antipode Blink, Orbital Slingshot and grind-lines.
- **Lighting:** a sweeping day/night terminator with night modifiers.
- **Items and enemies:**
  - proc, cursed and death-save items;
  - the Glitched elite affixes;
  - the 13 new enemies.
- **Bosses:**
  - authored per-phase boss mechanics (today's phases are a generic enrage, a banner and an add ring);
  - THE HOLLOW COSMONAUT (the Dark Moon boss);
  - the finale and superboss.
- **Interactables:** golden chests, the challenge shrine, the boss-curse shrine, the Suspicious Rock and radios. The radio beacons are props only.
- **Worlds:**
  - worlds 4-12;
  - signature events for the Moon and the Dark Moon;
  - per-world music.
- **Meta:**
  - the Unlock Web;
  - Ascension Depth, NG+, mastery and skins, and weekly mutators;
  - the Codex.
- **Co-op:**
  - party HP scaling (only the spawn count scales today);
  - down and revive (the Tumbling Beacon);
  - drop-in;
  - Steam relay;
  - 3-4 player testing (`MAX_PLAYERS` is 4 at src/net.rs:38, but it is untested).
- **Settings and input:** gamepad, rebinding and accessibility modes.
- **Foundations:** a fixed timestep (there is no `FixedUpdate` anywhere in src/) and deterministic combat RNG (`thread_rng` is used throughout combat and loot).
- **Where this is tracked:**
  - [docs/CONTENT_CATALOG_v0.1.md](docs/CONTENT_CATALOG_v0.1.md) catalogues what exists.
  - plan/ describes what the full game should contain under the §2 direction.

### 3.4 The last playtest (2026-09-26, solo, Moon T1, stale release exe)

The session log shows one run on a fresh save:
- **Result:**
  - **Level 25 and 1,525 kills.** The player died about 5:50 into the stage, around timer 4:12.
  - **Performance:** about 164-187 fps with the enemy count at the **1,200 cap**.
  - Afterwards the user clicked HOST CO-OP ("NET hosting on port 5011", state CharSelect), and the log ends there.
- **What it showed:**
  - **Save regressions, observed live.**
    - The save recorded `best_level: 1`, which is H1; the Results line logs `players=0`.
    - It recorded `chests: 0` although a chest was bought at about 6:58, which is H2.
    - It recorded `evolves: 0`; that counter can never rise, which is H3.
    - Silver was 521, which is 24 short, because the level term dropped out.
  - **The spawn curve is a death spiral.** This is how the code is written, not a spawn bug.
    - The ramp `(1 + 2.1·t/min)` is about 15x the GDD's `(1 + 0.14t)` (GDD.md:263).
    - Overflow above the cap is discarded, not merged into THE STATIC.
    - Nothing culls or recycles enemies.
    - Bruisers join at timer 5:30 (4:30 elapsed; src/content/enemies.rs:161) and double the mix's mean HP.
    - Pots take about 57 cap slots.
    - Between timers 5:39 and 4:12, about 1,156 enemies spawned against 177 kills. The count reached the cap, and the kill rate collapsed from about 8/s to about 2/s.
  - **Solo level-up panels pause the spawner.** Virtual time is paused and `director_spawn` returns at dt = 0 (src/enemies.rs:550-553).
- **Decision needed:** whether this curve is the intended design is the user's call (§9.3).

### 3.5 Build and smoke results (2026-09-26, debug exe, which matches HEAD)

- **`cargo build`:** a no-op, with **0 errors and 35 warnings**:
  - 13 unused variables or imports;
  - 3 unneeded `mut`;
  - 19 dead-code items, for example `net::disconnect` at src/net.rs:1133, `Joint.lag` at src/player.rs:100, and `PlanetDef.sky` / `meteor_showers` at src/content/planets.rs:35, 44.
- **Toolchain:** the build was checked with rustc 1.96.0. The repo pins no toolchain.

| Smoke | Result | Note |
|---|---|---|
| `--headless 2400` | PASS | Reached level 7 with 155 kills and 2 comets. |
| `--headless --coop2` | PASS | Ran 1,500 ticks, because `--coop2` is not a tick count (see §4.3). Reached level 6 with 84 kills. |
| `--headless 1200 --fast-boss` | PASS, **weak** | The bot died at level 1 with 0 kills. It passes only because `--fast-boss` skips the 0-kills check (src/headless.rs:338) and the boss did spawn. |
| `--headless 1200 --fast-boss --coop2` | **1 of 3 passed** | **The root cause is a harness bug:** the summary query at src/headless.rs:317 is an unfiltered `world.query::<&PlayerState>()`. With two astronauts it can read the peer, which may have died at level 1, and then "FAIL: XP pipeline dead" fires. The likely fix is to add `With<LocalPlayer>`; it is untested. Commit 17115e9 blamed this on non-determinism alone. |

---

## 4. How to run

### 4.1 Build and play

**Requirements:**
- A current stable Rust toolchain. The repo pins none; it was verified with rustc 1.96.0.
- A GPU that Bevy 0.18 supports.
- There are no asset files to fetch.
- `target/` is git-ignored, so a fresh clone contains **no exe**. You must build.

```
cargo run                      # windowed game; the dev profile is playable (own code opt-level 1, deps opt-level 3; Cargo.toml:15-19)
cargo run --release            # full-speed build (thin LTO, codegen-units 1; Cargo.toml:21-23)
cargo build --release          # produces target/release/astrobonk.exe (~69 MB), the file handed to playtesters
```

**Windows notes:**
- `cargo build --release` cannot replace `astrobonk.exe` while the game is running. Close the game first.
- **Never kill it by image name**: the user may be playing.

### 4.2 Controls

| Input | Action | Source |
|---|---|---|
| W A S D | Move, relative to the camera | src/player.rs:397-406 |
| Mouse | Orbit the camera. The cursor is locked while playing and freed in panels and menus | src/player.rs:747, 827-839 |
| Space | Jump. Jumping again within 0.16 s of landing keeps slide speed (bunny-hop) | src/player.rs:411; src/config.rs:15 |
| Left Ctrl or C | Slide | src/player.rs:412 |
| E | Interact: chest, shop, shrine, Moai, microwave, cage, teleporter | src/interact.rs:461 |
| 1-4 | Pick a level-up card. On the chest panel, 1 takes and 2 leaves; in the shop, 1-3 buy | src/ui/panels.rs:160, 328-334, 450 |
| R / B / S | Refresh / Banish / Skip on the level-up panel. **S is also move-back** | src/ui/panels.rs:171-183 |
| Esc | Pause and resume (Settings and Abandon Run are in the pause menu); also closes panels | src/ui/panels.rs:525-526, 568 |
| **B (DEV)** | **Summons the current planet's stage boss** during play. It shares the key with Banish. This ships ungated (M14) | src/enemies.rs:913-936 |
| **T (DEV)** | Replays the tutorial | src/tutorial.rs:42-45 |

**Known text error:** tutorial line 4 says "Shift to slide" (src/tutorial.rs:24). Slide is actually Ctrl or C.

### 4.3 Headless smoke tests (no window)

```
cargo run -- --headless 2400                     # solo smoke, ~80 s of sim time, prints SMOKE OK / FAIL
cargo run -- --headless 2400 --coop2             # two astronauts on one host (use this order, see below)
cargo run -- --headless 1200 --fast-boss         # boss path (late-game mix + veteran tome loadout)
cargo run -- --headless 1200 --fast-boss --coop2 # 2-player boss path (flaky: see §3.5)
cargo run -- --headless 2400 --planet mars       # also: --planet darkmoon, --hero <name>, --seed <u64>
cargo run -- --headless 2400 --enemydist         # histogram of how far the horde is from the players
```

**How the harness works:**
- It runs MinimalPlugins at a **fixed 33 ms per tick**, with a movement bot and a watchdog (src/headless.rs).
- `--hero`, `--planet`, `--seed` and `--fast-boss` are parsed only under `--headless` (src/main.rs:52-69).
- `--coop2` and `--enemydist` are read inside src/headless.rs (:377, :292).

**It fails when:**
- enemies exceed `ENEMY_CAP + 400`;
- run state becomes non-finite;
- the run ends stuck in a panel;
- there are 0 kills (this check is skipped with `--fast-boss`);
- the level is below 2 while kills exceed 50;
- there are 0 enemies and the boss is not dead;
- or, with `--fast-boss`, the boss never spawns (src/headless.rs:163-172, 333-358).

**Gotchas:**
- **The tick count must come directly after `--headless`** (src/main.rs:54). `--headless --coop2 2400` silently runs the default 1,500 ticks.
- **The smoke is not deterministic, even with `--seed`.** Two runs of the same build diverge, so it cannot serve as a before/after oracle for a refactor.
- **The headless app is a hand-copied duplicate** of the real system list. It has no NetPlugin and no `phase_time_control` (src/headless.rs:190-301). A system added to src/main.rs is **not** smoke-tested unless it is added there too. Messages that a shared system writes must also be registered there; see c5914ca for the `GrantOut` panic.
- **It cannot exercise** the wire path, `phase_time_control` or `InputIntent` from a real keyboard.

### 4.4 Co-op, the way a player does it (menus)

The host is a **listen server**: it plays as well. Both machines must run a **byte-identical build**, because `PROTOCOL_ID` has not been bumped since Stage 4 (src/net.rs:36). Mismatched builds would connect and then misdecode.

1. **Host:**
   - Allow **UDP 5011** inbound through the firewall.
   - Click **HOST CO-OP**, pick a hero and planet, and start the run.
   - **Caveat (M20):** the "HOSTING — tell the other player to join: <LAN IP>" note is written in the same click that switches to character select, so it is effectively never seen (src/ui/menus.rs:265-270). Get the IP from `ipconfig` instead, or press BACK to see the note.
   - **Caveat (H7):** if HOST CO-OP is clicked before any run has been played in this process, the host should panic (code-derived, not reproduced). `stream_enemies` takes a non-optional `Res<CurrentPlanet>` and runs as soon as the role is Host (src/netenemy.rs:189-194, 284). **Workaround:** play or abandon one solo run first, or launch with `--host --autodrop` (§4.5).
2. **Joiner:** click **JOIN CO-OP**, type the host's address (digits and dots only; it is pre-filled with 127.0.0.1), and press ENTER.
   - **Connect only after the host is in the run.** A joiner who connects while the host is still in its menus builds the wrong world (H8).
   - A failed join cannot be retried without restarting the game (M21).
   - The joiner does not pick a hero. It plays Buzz on a fresh launch (M5).
3. **After the run ends,** restart both games. The joiner is never told the run ended (H9).

**On one PC:** start two instances and join 127.0.0.1. Both instances share one save file.

### 4.5 Co-op and dev command-line flags (windowed game)

```
astrobonk.exe --host --autodrop --botinput --netlog        # host: skip menus, bot walks, per-second net dump
astrobonk.exe --join 127.0.0.1 --autodrop --botinput --netlog
```
(With `cargo`, use `cargo run -- <flags>`.)

| Flag | Effect | Source |
|---|---|---|
| `--host` | Start listening on `--port` (default 5011) at startup | src/net.rs:1148-1157 |
| `--join <ip>` | Connect at startup. `--autodrop` is ignored for a joiner, which waits for the host's seed | src/net.rs:1158-1171; src/main.rs:398-400 |
| `--port <n>` | Port for `--host` / `--join` | src/net.rs:1151, 1165 |
| `--autodrop` | Skip the menus and drop straight into Moon T1 as Buzz | src/main.rs:399 |
| `--botinput` | A bot walks the astronaut (no keyboard needed) | src/net.rs:1145 |
| `--netlog` | Per-second pose and invariant dump to **stdout**, including the prop checksum (`layout_sum`) and the interactable checksum (`inter`, `inter_sum`). Launch from a console to see it | src/net.rs:1146 |
| `--autopick` | Take option 1 of every card panel automatically | src/main.rs:166-171, 455-480 |
| `--bossnow` | Wind the clock to just before the boss mark; the minibosses are skipped | src/main.rs:504-520 |
| `--stagenow` | Host advances to stage 2 about 20 s in. Also forces tier 3 at boot | src/main.rs:161-165, 385, 484-500 |

**Two-instance verification signals used in the commits:**
- `local_sim=0` means a client simulates no enemies of its own.
- `proxies=N` climbing means the stream is alive.
- Identical `layout_sum` and `inter_sum` on both machines mean the same world was built.

### 4.6 Where files live

- **Save:** `%APPDATA%/astrobonk/save.json` (src/save.rs:107-112). It is written non-atomically, with no backup.
- **Session logs:** `%APPDATA%/astrobonk/logs/session-<unix>.log` (src/playlog.rs:33-60). Each log records:
  - the version and command line;
  - co-op events;
  - a health line every 5 s: role, state, PlayerId, players, enemies (**pots included**), proxies, fps, hp, level, gold, stage, timer and kills;
  - panics.

  The log does **not** record the seed, `layout_sum`, `inter_sum`, difficulty, items, comet fires, shrine events or the cause of death.

---

## 5. Co-op status

**In one line:** 2-player co-op works end to end **on one PC**. The joiner can fight and level up, but it cannot interact with anything, and it is not told when the run ends. It has **never been verified on two machines**. 3-4 players are untested.

### 5.1 Architecture

- **Topology:**
  - A **host-authoritative listen server** over direct-IP UDP, using renet's netcode transport.
  - The default port is **5011** (src/net.rs:37), with at most 4 players (src/net.rs:38).
  - Solo is `NetRole::Solo` and opens no sockets.
  - `net::is_simulating` is true for Solo and Host (src/net.rs:460-462). Everything that mutates the world is gated on it (src/main.rs:185-288, "THE SWITCH").
- **Input:**
  - Clients send only **input intent**: `PlayerInputMsg`, on an unreliable channel, every render frame.
  - Hardware and network both write the same `InputIntent` component (src/player.rs:71-77), so movement, physics and combat contain no net code.
- **Astronauts:** replicated per entity through replicon, as `PlayerId`, `NetTransform` and `PlayerVitals` (src/net.rs:348-350).
- **Enemies:**
  - The horde **never** replicates per entity. It streams through custom lanes in src/netenemy.rs.
  - The crowd lane uses quantized, interest-managed batches: each position is a great-circle offset from the receiving client's own astronaut, stored as two u16s over ±128 m.
  - Bosses have their own lane. Hazards and pickups use event lanes.
- **Builds:** each player picks its own cards. The client reports its **derived** stats upward in `PlayerBuildMsg` at 2 Hz, and the host simulates the peer with them (src/net.rs:175).

### 5.2 What a joiner CAN do today

- Connect through **JOIN CO-OP** (or `--join <ip>`) to the host's IP on port 5011.
- Build **the identical world** from the host's seed and planet chain (`RunSnapMsg`) — but only if it joins **after** the host is in a run (H8). Verified by matching `layout_sum` and, since 99d012f, `inter_sum`.
- Move its own **locally predicted** astronaut.
- See:
  - the streamed **horde** (interest-managed proxies, animated by the same `animate_crowd` as the host);
  - the **bosses**, with the HP bar and edge markers, and the Craterpillar's 12 body segments rebuilt locally at zero bytes;
  - the streamed **hazards**: spitter shots, beamer railbolts, mortars and slam rings;
  - the **pickups**;
  - its **teammates**, drawn as animated rigs.
- Receive **shared XP**, level up and pick its own cards on its own screen. The host then simulates the joiner's real build.
- Receive the gold, food and powerups addressed to it. Take hp and death from the host (`adopt_my_vitals`).
- **Follow the host through stage changes.** On a new `stage` in `RunSnapMsg`, `client_stage_transition` tears down and rebuilds the world, carrying the joiner's sheet (src/netenemy.rs:775-840).

### 5.3 What a joiner CANNOT do or see

| Gap | Detail | Ref |
|---|---|---|
| **Any interactable** | No chest, shop, greed/magnet shrine, Moai, microwave or cage. **No teleporter:** it does not exist on the client at all. `interact_system` is host-only and reads the local keyboard, and charge-shrine blessings go to the host only | M2, M3; src/main.rs:232-235; src/interact.rs:461; src/director.rs:101 |
| **Run end** | No run-end signal reaches the client. It stays InRun on an emptied planet | H9 |
| **Some attacks** | The Anubot Verdict Beam and the Beamer's 1.1 s aim line are invisible on the joiner, which still takes their damage | H10 |
| Pots | Pots the host breaks stay standing; the joiner cannot break pots locally | L41 |
| World feedback | No comet, no dust storm or haze, no damage numbers, no hurt vignette, no banners and no teammate HP HUD | M9, M11, M23, L39 |
| Hero | Always Buzz on a fresh launch. There is no hero pick for a joiner | M5 |
| Progress | No meta progression. The joiner banks no Silver, counters or quests from co-op (intended, so the host's numbers are not banked) | M7 |
| Powerups | They never expire on the joiner (Speed stays at 1.5x, Magnet at x40) | M6 |
| Session | No leave or retry, no revive, and a failed join needs a restart | M15, M21 |
| Position | The client's predicted body is never reconciled with the host's copy, and the two start 3.5 m apart | H5 |

### 5.4 What crosses the wire

| Direction | Message / component | Channel and rate | Content |
|---|---|---|---|
| host → client | `PlayerId`, `NetTransform`, `PlayerVitals` (replicated) | replicon `ServerTick` in FixedPostUpdate, 64 Hz by default | pose (`dir`, `height`, `facing`), hp, max_hp, level, down |
| host → client | `AssignPlayerId` | Ordered, repeated at 2 Hz | "you are player N". replicon 0.40 has no local-client-id API, and `PlayerId(0)` is ambiguous on a client |
| host → client | `RunSnapMsg` (src/net.rs:100-119) | Unordered, 4 Hz, **no sequence number** | seed, stage, chain, clock, difficulty, counters, Static/boss/teleporter flags. **It carries no `greed_stacks`** |
| host → client | `EnemySnapMsg` (src/net.rs:273) | Unreliable, 15 Hz, per client, chunks of at most 1024 B | spawn records of 8 bytes, updates of 6 bytes, despawns of 2 bytes. Interest hysteresis per planet is 95/107 m (Moon), 110/122 m (Mars) and 82/93 m (Dark Moon) (src/config.rs:66-99) |
| host → client | `BossSnapMsg` (src/net.rs:156) | Unreliable, 20 Hz | exact f32 positions, HP fraction, Anubot beam state |
| host → client | `HazardEventMsg`, `PickupEventMsg` | Unordered (reliable) events | a spawn event per hazard or pickup; the client integrates the flight or animation itself |
| host → client | `XpGrantMsg` / `LootGrantMsg` | Unordered (reliable) | XP broadcast to all (shared pool); gold, food and powerups to the collector only |
| client → host | `PlayerInputMsg` (src/net.rs:294) | Unreliable, every render frame | wish, forward, and the jump/slide/interact bits. It is neutral while a panel is open (99d012f) |
| client → host | `PlayerBuildMsg` (src/net.rs:175) | Ordered, 2 Hz | character, level, derived `Stats`, (weapon, level) pairs. The host never adopts items or gold from it |

**Bandwidth:** the last measured figure was about 3.9-4.6 KB/s at roughly 90 streamed enemies (commits 1e7d52f and 70dac72). It has not been re-measured at the cap.

**What the client still simulates:**
- its own body (prediction);
- cosmetic weapon fire (nothing consumes its HitMsgs);
- the hazard integrators (damage 0);
- the worm body;
- level-up panels, HUD, audio and camera.

**What the client never simulates:**
- AI, spawning, damage and loot;
- pickup collection;
- the clock and interactions;
- upkeep and powerup decay;
- run end and banking.

### 5.5 How co-op was built (2026-07-25 to 07-26; see DEVLOG for each)

| Stage | Commit | What it did |
|---|---|---|
| 1 | 35db5ff | Split the global `RunState` into a run-global `RunState` plus a per-astronaut `PlayerState` component (182 compile errors fixed down to 0). **This introduced H1-H3.** |
| 2a-2e | a0924d2, 3a1b357, 31dd23b, 6321938, bb1c3bd | Pinned the replicon versions; added `PlayerId`/`LocalPlayer`, the replication skeleton, the direct-IP transport, `InputIntent` routing, and teammate rigs (remote.rs) |
| 3 | f0790a1 | Made the host correct with 2 players (89 `.single()` sites reclassified). Added the co-op rules from the GDD and the `--coop2` smoke |
| 4 | 1e7d52f | Crowd lane (quantized, interest-managed). Added `--enemydist`. **PROTOCOL_ID was last bumped here** |
| 5 | e2987e9 | `RunSnapMsg`: both machines build the same world (layout checksum) |
| 6 / 6b | 4b20500, 70dac72 | Boss and hazard lanes; the worm body rebuilt on the client; `--bossnow` |
| 7 | c5914ca | Pickup lane; shared-XP broadcast; collector-only loot |
| 8 | 9b954f1 | `PlayerBuildMsg` build sync; `--autopick` |
| 9 | 6367aa8 | **THE SWITCH:** clients stop simulating their own world (`local_sim=0`) |
| audit | 17115e9 | Fixed a downed client banking the host's run into its own save in a loop, stale net indices after Results, and `gem_merge` eating streamed gems |
| 10 | 1ce86dc | Clients follow the host through stage changes; `--stagenow` |
| UI | 8194240, bdb7cc6 | HOST CO-OP / JOIN CO-OP menu and address overlay (and the click-through fix); the LAN-IP note |
| logs | ca38187 | Session logs with panic capture |
| docket 1 | 99d012f | Charge-shrine blessing `With<LocalPlayer>`; neutral input while a panel is open; interactable layout made save-independent; `inter_sum` checksum |

### 5.6 The co-op docket (the approved pre-playtest plan, items 1-10)

| # | Item | Status |
|---|---|---|
| 1 | The two live fixes plus interactable-layout parity | **Done** (99d012f). Never verified with genuinely different saves: the checksum covers the Cage and the host-only Teleporter, which differ legitimately (src/net.rs:813, 839-842) |
| 2 | Split `interact_system` (at the 16-param cap, src/interact.rs:394-411) into target / local / world | Not started |
| 3 | The host resolves world interactions (Teleporter, Greed, Magnet) for every astronaut | Not started. It must also reset the latched jump/slide/interact bits (H4) |
| 4 | Replicate the teleporter and `greed_stacks`, with one PROTOCOL_ID bump | Not started |
| 5 | Chests, shop, Moai and microwave for the joiner | Not started. Blocked in practice by M1: isolate the Shady Guy stock RNG first |
| 6 | `client_pot_break` (visual only) | Not started |
| 7 | Anubot beam (7a: client-only, since the data already streams) and Beamer aim line (7b: a wire change) | Partial: the data for 7a is on the wire and unused |
| 8 | Teammate HUD from `PlayerVitals` (filter `pid != MyPlayerId`) | Not started |
| 9 | Two-machine dry run | Not started. Rebuild release first (§7) |
| 10 | Log triage | Not started |

None of `LocalInteractTarget`, `interact_world`, `spawn_teleporter_at`, `teleporter_dir` or `client_pot_break` exists in src/.
- The recommended order, with the pre-fixes the audit added, is in §9.
- [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md) has the full reasoning.
- plan/08_BUILD_ROADMAP.md places co-op within the full-game roadmap.

**The NETCODE NOTES block at src/net.rs:1199-1262 is stale.** It was last edited in Stage 3 (bf1dae0):
- Items 3 and 4 are mostly done.
- Stages 4-10 are not mentioned.
- The module header at src/net.rs:1-15 predates `PlayerBuildMsg` and streaming.
- The comment "bumped whenever the wire changes" (src/net.rs:34-36) is not being followed.

For the current netcode picture, trust this section and docs/TECHNICAL_REFERENCE.md over those comments.

---

## 6. Verified top issues

**[docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md) is the full list** (high, medium and low, with failure scenarios and fix notes). The IDs below follow the 2026-09-26 audit.
- Every item was confirmed by reading the code.
- Items marked *live* were also seen in the user's 2026-09-26 log or save.
- The rest are derived from code and were not reproduced at runtime.

### High

| ID | Issue | Where | Consequence |
|---|---|---|---|
| **H1** | `bank_results` reads the local `PlayerState` **after** `despawn_stage` has removed it (the player is `StageScoped`). It therefore falls back to level 1 and gold 0 | src/director.rs:274, 280; src/main.rs:141-155; src/player.rs:170 | *Live*: best_level is stuck at 1, Results shows LEVEL 1 / GOLD 0, the payout and daily score lose the level term, and the "Overachiever" (Level20) quest can never complete. **A regression from 35db5ff.** Fix: copy level and gold into RunState or ResultsData before leaving InRun, in the victory, downed/death and Abandon paths |
| **H2** | Taking a chest never increments `chest_opens` or `chests_opened` | src/ui/panels.rs:341-361; src/run.rs:121, 124 | *Live*: saved chests stay 0, so "Cache Money" (Chests10) can never complete and Rocket Pod never enters the level-up pool (it is still Ironclad's starter). The chest price is stuck at 25 because `CHEST_COST_GROWTH` is dead (src/interact.rs:440). **Regression from 35db5ff** |
| **H3** | An evolution never increments `run.evolves` | src/run.rs:608-617; src/ui/panels.rs:233-238 | "Ascension" (EvolveWeapon) can never complete. **Regression from 35db5ff** |
| **H4** | The host latches a remote peer's jump, slide and interact bits forever (OR-assigned, never cleared) | src/net.rs:1011-1013 (consumed at src/player.rs:457, 490) | After one Space press, the host's copy of the joiner re-jumps on every landing at up to 2.1x speed. Ctrl or C re-slides every 1.1 s. The bits clear only at a stage-change respawn. 99d012f's neutral intent does **not** clear them |
| **H5** | The client's predicted body is never reconciled with the host's copy, and they spawn 3.5 m apart | src/main.rs:435; src/netenemy.rs:809-820; src/net.rs:920-931 | The host copy takes the hits, collects the pickups and anchors interest. Drift comes from host pause and hitstop, H4, non-expiring Speed, and differing dt |
| **H6** | A host panel or pause longer than 2 s makes the joiner's horde **permanently invisible** | src/fx.rs:47-58; src/netenemy.rs:291-295; src/config.rs:99 | The stream stops while virtual time is paused. The client reaps its proxies after 2 s, but the host still lists them as resident, so later updates are dropped. The enemies stay invisible but still deal damage until they leave interest and re-enter it. `--autopick` hid this in every test |
| **H7** | Clicking HOST CO-OP before any run in the process should panic | src/netenemy.rs:189-194, 284 | `stream_enemies` takes a non-optional `Res<CurrentPlanet>`. `--host` without `--autodrop` hits the same path |
| **H8** | A joiner who connects while the host is in its menus builds the wrong world | src/net.rs:394-399; src/main.rs:373; src/ui/menus.rs:593 | `push_run_snapshot` has no state gate, and at stage 0 the new seed never triggers a rebuild |
| **H9** | No run-end signal reaches a client | src/net.rs:99-119; src/main.rs:151, 284-285, 373 | The joiner stays InRun on an emptied planet |
| **H10** | The Anubot Verdict Beam and the Beamer aim line are invisible on the joiner, which still takes damage from them | src/netenemy.rs:881-882, 910-917 | Docket item 7 |

### Selected medium issues

| ID | Issue | Where |
|---|---|---|
| M1 | **The interactable layout RNG can desync across machines.** Shady Guy stock rolls share the placement `StdRng`, and rand 0.8.7's `choose` consumes a variable number of words that depends on each machine's sheet (luck, items, bans). The shady guys, shrines, Moai, microwave, cage and all 5 charge rings can move; pots and chests are safe. This becomes high once joiners can interact | src/interact.rs:147, 241-249; src/director.rs:212-216 |
| M4 | Host panels, pause and hitstop freeze the world and every host net timer **for everyone**. A peer completing a charge shrine opens a Modal on the host | src/fx.rs:47-58; src/interact.rs:375-386 |
| M9 | `DustStorm.player_inside` tracks only one astronaut, yet it silences every ranged enemy against every target | src/events_world.rs:109-117; src/enemies.rs:1284 |
| M10 | `DustStorm` is never reset. On a second Mars visit in the same session, storms run and blind enemies but no dome is drawn (this shows in solo) | src/events_world.rs:62, 80 |
| M13 | `Counters` has no `#[serde(default)]`. Adding a field or renaming a persisted enum variant makes `load()` fall back to Default, and the next save overwrites the player's progress | src/save.rs:14, 118-121 |
| M14 | DEV key **B** (boss summon, "Remove before ship") ships ungated. In solo it is a progression shortcut, because killing the summoned boss opens the teleporter | src/enemies.rs:913-936 |
| M16-M17 | Spawn and scaling are tuning outliers against the GDD, and there is no stage or tier scaling: `stage_transition` resets `elapsed` to 0, so stages 2-3 restart at minute-zero time scaling (only `difficulty` carries over) with a Shambler-only mix | src/enemies.rs:563-576; src/director.rs:197; src/content/enemies.rs:155-166, 238-243 |
| M19 | ABANDON RUN leaves the PAUSED overlay, blocking buttons included, over Results and the main menu until the next run (this shows in solo) | src/ui/panels.rs:536, 584 |
| M20 | The HOSTING / LAN-IP note is effectively never seen | src/ui/menus.rs:265-270 |

**Adjacent debt:**
- **PROTOCOL_ID has not been bumped since Stage 4** (src/net.rs:36; L46), although 7 message types have been added since: `RunSnapMsg`, `BossSnapMsg`, `HazardEventMsg`, `PickupEventMsg`, `XpGrantMsg`, `LootGrantMsg` and `PlayerBuildMsg`.
- The headless summary query is unfiltered (src/headless.rs:317; §3.5; L57).

---

## 7. The stale release exe

**`target/release/astrobonk.exe` is stale. Do not hand it to a tester.**
- **Timing:** it was built at 2026-07-26 01:19, and HEAD (99d012f) was committed at 01:23. The two files that commit changed were saved after the build: src/interact.rs at 01:20:58 and src/net.rs at 01:21:41.
- **Proof:**
  - A byte search finds the string `inter_sum=`, which 99d012f added, **zero times** in the release exe and once in the debug exe.
  - The mtimes say it very likely lacks all three 99d012f fixes as well.
  - It does include bdb7cc6, the LAN-IP note.
- **By contrast,** `target/debug/astrobonk.exe` (01:22) matches HEAD. `cargo build` found nothing to recompile.
- **The user's 2026-09-26 playtest ran on the stale release exe.** None of 99d012f's fixes affect a solo run on a fresh save, so that run's data is still valid.
- **Fix:** close the game, then run `cargo build --release`. Re-run the four smokes (§4.3) before shipping it.
- For a two-PC test, copy the **same** freshly built exe to both machines.
- `target/` is git-ignored, so the GitHub repo contains no exe at all.

---

## 8. Docs map

| Document | What it is | Status |
|---|---|---|
| [README.md](README.md) | The front door: what ASTROBONK is, how to build and run it, and where to read next | Being written for the public repo |
| **PROJECT_STATUS.md** (this file) | The canonical current status: what works, what is broken, and what is next | Current as of 2026-09-26 |
| [GDD.md](GDD.md) | **The complete game design document** | **Being rebuilt from plan/** under the §2 creative direction. The 2026-07 edition's "Current build state" (GDD.md:36, 1211-1235) predates every commit after 02300e3 and does not mention co-op |
| [plan/00_MASTER_PLAN.md](plan/00_MASTER_PLAN.md) | **The full-game plan**: the end goals and the complete design under the locked 2026-09-26 direction | Being written |
| [plan/08_BUILD_ROADMAP.md](plan/08_BUILD_ROADMAP.md) | The ordered build roadmap from today's code to the full game | Being written. **This is where "what next" lives** |
| [docs/TECHNICAL_REFERENCE.md](docs/TECHNICAL_REFERENCE.md) | Architecture, systems, the schedule and role gating, netcode and wire formats, save format, and engine traps | Being written alongside this update (2026-09-26). It supersedes the stale NETCODE NOTES comments in src/net.rs |
| [docs/CONTENT_CATALOG_v0.1.md](docs/CONTENT_CATALOG_v0.1.md) | Every hero/suit, weapon, evolution, item, tome, quest, enemy, boss, planet and interactable that exists in v0.1, with its numbers | Being written alongside this update (2026-09-26) |
| [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md) | The verified issue list (high, medium, low, uncertain) | Being written alongside this update (2026-09-26) |
| [DEVLOG.md](DEVLOG.md) | Session-by-session history, newest first, including every commit | Backfilled through 99d012f plus the 2026-09-26 entry |
| [DESIGN.md](DESIGN.md) | The original 2026-07 Megabonk research digest and v0.1 design | Historical. Its module map (DESIGN.md:199-221) claims meteor showers and dust devils, which do not exist |
| src/net.rs:1199-1262 | NETCODE NOTES, the in-code running log from the co-op build | **Stale since Stage 3**; see §5.6 |

**Precedence when documents disagree:**
- **For what exists today:**
  1. The code.
  2. This file and DEVLOG.md.
  3. docs/.
- **For what the game should become:**
  1. The §2 creative direction.
  2. plan/.
  3. The rebuilt GDD.md.
- **DESIGN.md and the 2026-07 GDD text are historical.**

---

## 9. Next steps

### 9.1 Direction and roadmap

- **The creative direction in §2 is locked** (2026-09-26). It is the target for everything that follows:
  - Megabonk's addictive traits, bettered;
  - the planets as the levels;
  - one astronaut going home to his pet turtle, with a suit that visibly upgrades each planet;
  - the 12 astronauts turned into 12 suits;
  - death returning you to the crash site, with Hades-style persistence;
  - a cartoon-spooky tone at E10+;
  - the toon art style.
- **The ordered build plan is [plan/08_BUILD_ROADMAP.md](plan/08_BUILD_ROADMAP.md).** The full-game plan it serves is [plan/00_MASTER_PLAN.md](plan/00_MASTER_PLAN.md). When the roadmap and this section disagree, the roadmap wins; update this file to match.
- None of the direction is implemented yet. The largest changes it implies against today's code:
  - Hero selection becomes suit selection (`AstronautKind`, src/content/characters.rs).
  - The Results-and-menu loop becomes a persistent journey that returns you to the crash site (src/director.rs `bank_results`; src/save.rs).
  - The planet/tier menu becomes a sequence of planet levels (src/content/planets.rs `chain_from`; src/ui/menus.rs).
  - A per-planet suit-upgrade visual appears on the rig (src/player.rs `build_astronaut_rig`).
  - A toon rendering pass replaces the current PBR look (src/main.rs:337-345; src/meshkit.rs vertex colours).

### 9.2 Immediate engineering (the audit's recommendation, pending the roadmap's ordering)

**Option A: stabilize the solo loop first.** It is small, it is independent of co-op, and it affects every run played today.
1. Fix H1, H2 and H3 (the save counters), M19 (the Abandon overlay), M10 (the DustStorm reset), and the unfiltered query at src/headless.rs:317 (add `With<LocalPlayer>`).
2. Gate or remove the DEV keys B (M14) and T.
3. Fix the tutorial's "Shift to slide" (src/tutorial.rs:24).
4. **Add `#[serde(default)]` to `Counters` (src/save.rs:14) before any new save field lands** (M13). The Hades-style persistence in §2 will add save data, and without this attribute a new field wipes existing saves.
5. Close the game, run `cargo build --release`, and re-run all four smokes (§4.3).
6. Refresh this file and DEVLOG.md.

**Option B: resume the co-op docket toward a two-PC test.**
1. **Pre-fixes the audit added:**
   - 0a: reset the latched bits in `apply_remote_input` (H4).
   - 0b: gate `push_run_snapshot` on InRun, or rebuild on a seed or chain change (H8); use `Option<Res<CurrentPlanet>>` in `stream_enemies` (H7); make the LAN note visible (M20).
   - 0c: fix the invisible horde after a host pause (H6). One candidate keys the client reaper to snapshot arrival; the other has the host re-send spawn descriptors after a pause. Neither has been evaluated.
   - 0d: isolate the Shady Guy stock rolls from the placement RNG (M1).
2. **Then the docket items:** 2, 3, 5, then 4 with the single PROTOCOL_ID bump, then 6, 7a and 8 (7b only if it can share the bump).
3. **Then:** rebuild release, run item 9 (the joiner connects **after** the host is in the run, both launched from a console with `--netlog`), then item 10.

**Option C: balance and content**, driven by the 2026-09-26 log.
- Retune the spawn slope.
- Add stage and tier scaling (M17).
- Take pots out of the cap.
- Add a cull, or merge overflow into THE STATIC.

**The audit's recommendation is A, then B.** Option C waits on the spawn-curve decision below.

### 9.3 Decisions only the user can make

1. Did the 2026-07-27 two-PC coworker playtest happen? If so, what broke, and where are that PC's `%APPDATA%/astrobonk/logs`?
2. Which comes next: co-op readiness (B), or solo balance and content (C)? And how does each sit inside plan/08_BUILD_ROADMAP.md?
3. The spawn curve:
   - keep the code's `(1 + 2.1·t/min)` or move toward the GDD's `(1 + 0.14t)`?
   - add a cull, or merge-into-Static?
   - exclude pots from the cap?
4. Confirm that a boss killed during THE STATIC opens no teleporter. Today it cannot: once the Static is active, `run_clock` returns before the teleporter check (src/director.rs:55-59, 98-103, 112).
5. The co-op chest rule: opener-only (the docket) or the GDD's "contested-but-generous" (GDD.md:902)?
6. Should joiners bank any meta progress? This now also means: does a joiner's suit or story progress persist under §2?
7. When do the ship-hygiene items go: the B and T keys, Mars unlocked by default (src/save.rs:84), and all 6 recruits unlocked (src/content/characters.rs:237-244)? Re-gating either unlock needs an explicit removal migration, because existing saves persist it.
8. Is air-hopping to 2.1x without sliding intended?
9. Commits: the user authorizes commits explicitly ("commit it").

---

## 10. Architecture map (src/, 16,042 lines in 40 files)

| File | Lines | Role |
|---|---|---|
| main.rs | 536 | The `AppState` enum (Boot, MainMenu, CharSelect, PlanetSelect, InRun, Results); the `playing` run condition (:47-49); the CLI branch to headless (:51-70). **Every system registration and role gate ("THE SWITCH", :185-288)**. Also `client_follow_host_run` (:354-376), `boot` (:379-404), `enter_run` (:406-449) and the dev systems (:451-520) |
| headless.rs | 382 | The smoke app: a fixed 33 ms tick, the movement bot, the watchdog, `--coop2`, `--enemydist`. Its system list is a **hand-copied duplicate** |
| sphere.rs | 176 | Analytic terrain (hills, ridges, craters); tangent frames; `advance`, `step_toward`, `arc_dist`, `offset_dir`, `fib_sphere` |
| planet.rs | 517 | `StageScoped`; `PropColliders`; `CurrentPlanet`; the icosphere(7) terrain mesh; `spawn_stage` prop scatter; `despawn_stage`; sun and sky props |
| player.rs | 875 | `Player`, `PlayerId`, `LocalPlayer`, `InputIntent` (:71); `spawn_player` (:117); `build_astronaut_rig` (:183); `gather_local_input` (:382); `player_input` (:418); `player_physics` (:523); `animate_rig` (:617); `camera_rig` (:747); `player_upkeep` (:842, host-only) |
| meshkit.rs | 224 | The primitive compositor. It bakes box, sphere, cylinder, cone and capsule shapes into one vertex-coloured mesh; also `icosphere()` |
| config.rs | 99 | **Every tuning constant**: movement, camera, the enemy cap, spawn arcs, stage marks, comet, net streaming (:66-99) |
| comet.rs | 127 | Comet Combo (host's LocalPlayer only) |
| events_world.rs | 130 | The Mars migrating dust storm, the only world event |
| enemies.rs | 1663 | `Enemy`/`Boss`, `EnemyAssets`, `SpatialHash` (:181), `Director`; `director_spawn` (:539); `spawn_boss` (:628); `boss_phase_system` (:720); `anubot_beam_system` (:798); `debug_spawn_boss` (:915); `craterpillar_update` (:941); movement, attack and telegraph systems |
| combat.rs | 1024 | `weapon_fire` (:212, 9 behaviours); projectiles, drones, beams, auras; `apply_hits` (:877); `apply_player_hits` (:973) |
| director.rs | 346 | `run_clock` (marks, teleporter, The Static); `levelup_trigger`; `stage_transition`; `downed_watch` / `death_watch`; `bank_results` (:272) |
| fx.rs | 186 | Shake; hitstop (virtual time at 0.06x); `phase_time_control` (:47-58: every non-Playing phase **pauses `Time<Virtual>`**); particles (not actually pooled, despite :1) |
| run.rs | 640 | `RunState` (run-global resource); `PlayerState` (per-astronaut component); `RunPhase`; `GameRng` and the daily seed (:46-63); `recompute_stats`; `evolvable` (:371); `roll_upgrades` (:510); `apply_upgrade`; `ChoicePanel` |
| stats.rs | 180 | The 27-stat sheet |
| pickups.rs | 404 | XP, gold, silver, food and powerups; `pickup_update` (shared XP; the `GrantOut` relay); `kill_drops`; `gem_merge` |
| interact.rs | 551 | `InteractKind` (:26-35); `Pot`; charge shrines; seeded `spawn_interactables`; `spawn_teleporter` (:299); `interact_system` (:394-551, **at the 16-param cap**) |
| save.rs | 240 | `MetaSave` / `Counters` stored as JSON; `save_path` (:107); `load`; `migrate` (:130); `save`; `check_quests` (:168) |
| tutorial.rs | 88 | 6 Mission Control radio lines; DEV key T |
| messages.rs | 84 | Local-only messages: `HitMsg`, `PlayerHitMsg`, `KillMsg`, `NumberMsg`, `BannerMsg`, `SfxMsg`, and the `Sfx` enum (:64). **None of them is networked** |
| content/*.rs | 1,602 total | Static tables: characters (245), weapons (518), items (249), enemies (243, with `mix` and `time_scaling`), planets (147, with `chain_from`), quests (84), tomes (57), mod (59, rarity) |
| net.rs | 1262 | `NetRole`; every wire type; replicon/renet plugin setup (:343-458); identity; input routing (`apply_remote_input` :981-1017); `RunSnapMsg`; build sync; grants; seat/unseat; transport (`start_host`, `start_join`, `local_ip` :1051); CLI `apply_cli_net` (:1142); `--netlog` checksums; the **stale** NETCODE NOTES (:1199-1262) |
| netenemy.rs | 1386 | Host lanes for crowd, boss, hazards and pickups; the ordered client proxy chain (:200-214); `client_stage_transition` (:775-840) |
| remote.rs | 209 | Client-only teammate rigs driven by `NetTransform` (the palette is chosen by slot, :82-85) |
| playlog.rs | 133 | Session log file, a health line every 5 s, and a panic hook |
| audio.rs | 197 | 16 synthesized SFX; `SfxThrottle` |
| music.rs | 320 | A 6-stem, 32 s adaptive synthwave track; `MUSIC_MIX` 0.12 (:18) |
| ui/mod.rs, hud.rs, menus.rs, panels.rs, numbers.rs, settings.rs | 64 / 629 / 815 / 586 / 163 / 204 | `button_hover`; the HUD (`update_hud`'s ParamSet is at the cap of 8; edge markers); menus including HOST/JOIN and the address overlay; modal panels (level-up, chest, shop, pause); the damage-number pool; the settings overlay |

**Rules of this codebase. Each was learned the hard way; do not relearn them.**
- **`InputIntent` decouples input from movement.**
  - Anything that writes it must be ordered `.before(send_local_input)`.
  - `player_input` reads only `wish`, `jump` and `slide`. `forward` and `interact` are never read, and the comments at src/player.rs:73 and :380-381 are wrong.
- **Remote astronauts never get `Player`, `PlayerState` or `StageScoped`.**
  - Otherwise about 30 `.single()` sites silently return `Err(MultipleEntities)`, and `player_physics` stomps the replicated transform (see the header of src/remote.rs).
  - Every per-machine `.single()` needs `With<LocalPlayer>`, because the host holds N PlayerStates.
- **`PlayerId(0)` is ambiguous on a client.** Both its own predicted body and the host's body carry it. Only `MyPlayerId` tells them apart.
- **Role gating:** anything that mutates the world, damage, loot, RunState counters or the save must be `.run_if(net::is_simulating)`. Visual and local systems stay ungated.
- **A client never banks progress or saves for a stage it did not simulate** (17115e9, 1ce86dc).
- **Track net ids only from entities that already carry the component.** `commands.insert` is deferred (1e7d52f, c5914ca).
- **Never use `SendTargets::All` from the host.** It loops back into the host's own queue (e2987e9).
- **Send derived `Stats`, not items, in `PlayerBuildMsg`.** The host must never `recompute_stats` a peer, because that folds in the host's meta tomes (src/net.rs:643-646).
- **Draw order in the seeded streams is a cross-machine contract.** No draw may depend on per-machine state (99d012f; M1 still breaks this).
- **Bevy 0.18 limits:** at most 16 system params and at most 8 per ParamSet. A missing non-`Option` `Res` **panics** by default.
- **Enum variant names are persisted in the save.** Renaming one wipes saves until M13 is fixed.

---

## 11. Known quirks

- **The headless smoke is not deterministic, even with a fixed `--seed`.** It cannot be a before/after oracle for refactors; verify by inspection or targeted checks.
  - All simulation uses variable dt, and combat and loot use `thread_rng`.
  - Only world generation and director spawns are seeded:
    - terrain comes from PlanetDef seeds 7, 23 and 66;
    - props from `StdRng(seed ^ 0xA11CE ^ terrain.seed)`;
    - interactables from `StdRng((run_seed + stage) * 0x9e37)` (src/interact.rs:147);
    - enemy spawns from `GameRng`.
- **`--headless`'s tick count must directly follow the flag** (src/main.rs:54).
- **`--fast-boss --coop2` flakes** because of the unfiltered query at src/headless.rs:317, not only because of non-determinism.
- **Pots are `Enemy` entities** (`Pot` + `Enemy{speed 0, contact_cd INF, hp 1}`).
  - They sit in the spatial hash, count toward the 1,200 cap (60 are placed on the Moon, src/content/planets.rs:71; about 57 were still standing in the 2026-09-26 run) and toward the `enemies=` log figure, and break on any hit.
  - They are excluded from steering and streaming via `Without<Pot>` and pot checks.
- **Bosses are `Enemy{kind: Bruiser}` + `Boss`.** Their visuals must key off `BossKind`.
- **Worm segments are not `Enemy`.** They are decoration placed from the head's trail.
- **The Dark Moon's stage boss is Anubot** (src/director.rs:89-92). THE HOLLOW COSMONAUT is not built.
- **"Boss is a scaled capsule" is long obsolete.** There is a generic boss mesh (67a734b), the Craterpillar worm and Judge Anubot, each with 3 phases.
- **Charge-shrine loot, the Moai and the Microwave reuse the level-up `ChoicePanel`** with `is_levelup: false` (src/interact.rs:385, 511, 532).
- **Z-layers:** card, chest and shop panels at `GlobalZIndex(10)`, pause at 20, settings at 50 (src/ui/panels.rs:69, 286, 401, 536; src/ui/settings.rs:142).
- **Full-screen Bevy overlays do not block the buttons behind them.** This caused the host-and-join bug (8194240). It still lets clicks pass through the settings overlay to the main menu, which can spend Silver on tomes.
- **Every non-Playing `RunPhase` pauses `Time<Virtual>`** (src/fx.rs:47-58). In co-op, a host panel freezes the world **and every host net timer** for everyone (M4, H6).
- **`CurrentPlanet` and `DustStorm` are never removed or reset** between runs. `Option<Res<CurrentPlanet>>` is not an "in a run" test.
- **Level-up offers 4 cards.** GDD canon says 3 (GDD.md:283).
- **No evolution cap is enforced.** GDD canon is 1 per run, +1 with a Tome of Ascension.
- **Air-hopping** (W + Space) reaches the same 2.1x cap as sliding.
- **The sky is Bevy's default gray.** `PlanetDef.sky` and `meteor_showers` are never read, and no `ClearColor` is set.
- **Enemies ignore props.** The horde walks through rocks; this was deliberate (DEVLOG Session 2i).
- **Dev leftovers ship in release:**
  - the B boss summon;
  - T to replay the tutorial;
  - Mars unlocked by default;
  - all 6 recruits unlocked;
  - the dev CLI flags compiled in. Their run conditions re-scan the args every frame (src/main.rs:159-171).
- **On Windows, a running game locks `target/release/astrobonk.exe`.** Close it before `cargo build --release`, and never kill it by image name.
- **DEVLOG dates:** Sessions 1-1m are dated 2026-07-13, but git's first commit (02300e3) is dated 2026-07-23. The commit hashes are authoritative for ordering.
- **The repository has no LICENSE file.**

---

## 12. History at a glance

| Date (git) | Milestone | Commits |
|---|---|---|
| 2026-07-23 | v0.1 and the GDD; content batch 1 (12 heroes, 16+16 weapons) | 02300e3, efc9ef4 |
| 2026-07-24 | Visual pass (meshkit); Moon vertical slice (Craterpillar, camera, Comet Combo, music, settings); Mars identity (dust storm, Judge Anubot); Quality & Clarity; boss phases; hero passives; world seed and Daily; tutorial; jointed astronaut rig and prop collision; horde animation; exaggeration pass | 67a734b through 82bce4b |
| 2026-07-25 | Craterpillar de-flop; **co-op Stages 1-10** | 7a69e42; 35db5ff through 1ce86dc |
| 2026-07-26 | Co-op menu UI, session logs, LAN IP, docket item 1 | 8194240, ca38187, bdb7cc6, **99d012f (HEAD)** |
| 2026-09-26 | Verified catch-up audit; new creative direction locked; plan/ and docs/ written; published to GitHub | (documentation) |

Session-by-session detail is in [DEVLOG.md](DEVLOG.md).

---

## 11. Build-out in progress (branch `claude/pensive-keller-0cood4`, PR #1)

Since 2026-09-27 the rest of the game is being built package by package on
`claude/pensive-keller-0cood4`. The plan and live status table are in
[docs/BUILD_PLAN.md](docs/BUILD_PLAN.md); per-wave notes are at the top of DEVLOG.md.
Sections 1-10 above describe `main` at 99d012f. They will be rewritten against the
finished code in the final documentation pass (BUILD_PLAN P29). The locked creative
direction in §2 is folded into the plan. Where the 2026-07 GDD and §2 disagree, §2 wins.

Behaviour changes already merged on the branch that differ from §1-§10:
- **Silver income vs sinks (since P01).** Banking follows the GDD §10 formula. A T1 run that
   clears and then farms ~1:30 of The Static now banks about 4,700 Silver (about 2,300 from
   the formula, mostly `kills / 4`, plus about 2,400 picked up). The v0.1 formula paid about
   2,650 for the same run, and its performance part was about 250. The sinks are still v0.1
   prices: tomes cost `8·l^1.5` over 20 ranks and quests pay 20-250 Silver. P05 moves tomes
   to the §7 price (`100 × 1.6^level`, 10 ranks), P21 re-prices quests and P26 adds the
   Unlock Web (1,200-15,000 per node). Until they land, meta progress runs fast.
- **Enemy HP curve (since P01).** HP follows GDD §3, `(1 + 0.11·t)^1.35`, with `t` counted
   across the whole chain, and is anchored so the cold-open Shambler still dies to one
   starter hit (`config::SCALE_HP_BASE`). Late T1 is softer than the v0.1 curve (x2.7 at
   10:00 vs x8). Chained worlds start far harder, because v0.1 reset to x1 at every
   teleporter; a T3 finale now reaches about x11. Tune against `--headless 21000 --balance`,
   which keeps the bot alive and prints spawns/s against kills/s and the live count every
   30 s. Don't tune against bot deaths: the bot dies to contact damage around minute 3-4
   whatever the curve.
- **Co-op sessions (since P02).** A session outlives the host's runs: at run end the host
   sends `RunOverMsg` and joiners return to the menu still connected, then follow the host into
   its next run (`RunSnapMsg.run_gen` drops stragglers). Only the host banks; a joiner's Silver
   lands in the shared run pot and the host's save. Per-player meta rewards for joiners have no
   owning package yet. The pattern for new co-op features is a host-only SIM system plus an
   ungated VISUALS system (see NETCODE NOTES 2e in `src/net.rs`); `--netlog` prints NETPARITY.
