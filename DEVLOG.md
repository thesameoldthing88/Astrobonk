# ASTROBONK DEVLOG

## 2026-09-27 — P18 lands: the §11 co-op rules
- **P18:** a downed astronaut becomes a **Tumbling Beacon** — it rolls down the terrain's fall
  line under a planet-high flare (faded near the camera) while its **Static Meter** fills; a
  teammate standing in the ring for 3 s revives it at 50% HP and gets **Hero's Adrenaline**
  (+20% speed, 5 s); a full meter means The Static claims it until the next teleporter, where it
  rejoins at 50%. The §11 party table scales crowd and boss HP; spawns anchor on standing
  astronauts. **Friendly physics** (no friendly damage): swings nudge, blasts and Slams pop,
  cryo fields chill, Tesla arcs jolt. **Drop-in**: a peer seated 20 s+ into a run falls from
  orbit at half the squad's level and fights on autopilot while idle. **STATIC CASCADE** (two
  storm-callers 120°+ apart wrap the planet in a lightning belt) and three named duos (Deep
  Freeze Protocol, Magnet Circus, Rivet & Rescue) with a squad callout on results. Squad HUD,
  Beacon edge markers, the downed player's own panel.
- **Co-op wire:** PlayerVitals carries the down/claim/meter/revive/adrenaline/chill/grace
  fields; RunSnapMsg the cascade charge and squad tally; five hazard-lane one-shots (Revived,
  Shove, Cascade, Duo, DropIn); `PROTOCOL_ID` 0xA570B0_C.
- **Reconciled on landing:** P12's `HitMsg.weapon` and P18's `HitMsg.by` answered the same
  question; one `HitBy` remains with `HitMsg::weapon()` for P12's readers. A Beacon no longer
  lifesteals from shots in flight, regenerates, or takes hits that would reset its meters.
- **Verified:** smoke 6/6; `--revive/--cascade/--duos --coop2`, `--dropin`, `--coop4`, plus
  `--weapons base|evolve`, `--bestiary all --coop2`, `--daynight --coop2`, `--hazards`,
  `--overflow` on the merged tree; a two-instance run with `--dev --downpeer` (a revive over the
  wire, then a real party wipe at 70 s).

## 2026-09-27 — P08 lands: seven new enemies and per-world spawn tables
- **P08:** the batch-1 §9 bestiary, each with its own behaviour, silhouette and one emissive
  accent: Rollo (great-circle roller that runs away downhill and bonks on walls), Trencher
  (tunnels under a ridge, uppercuts grounded astronauts), Aegis Drone (slow-turning front shield,
  10% through it, reads BLOCK), Sunskimmer (whining kamikaze diver, shootable at altitude via its
  column hitbox), Beacon Tick (6 s tracker that pulls the horde), Mimic Chest (14% of chests,
  rolled with the layout; eats the price, shockwaves, flees, refunds the payer on death), Longshot
  Beamer Prime (leads its mark; hills block the railbolt). The director draws kinds from per-world
  spawn tables; `enemies::spawn_enemy_at` is the one entry point for adds.
- **Co-op:** kind codes 9-15, a new enemy-state lane for what a client cannot derive, five
  hazard-lane events; `PROTOCOL_ID` 0xA570B0_B (after P07/P12's bumps).
- **Reconciled on landing:** P07's night pace (`Sun::enemy_speed`) and Beacon tracking combine
  in the movers; P12 stuns hold the Rollo, Trencher, Skimmer, Tick, Mimic and Aegis (a stunned
  Skimmer mid-dive pops); P12's Bell-mark crit and the Aegis block share `apply_hits`.
- **Verified:** smoke 6/6; `--bestiary all` solo and co-op; windowed `--dev --enemies all`.
- **Found:** the UI font draws the em dash as a box ("YOU □ THE HORDE") — logged as L66.

## 2026-09-27 — P12 lands: the Tier-1 weapons play as §6 describes
- **P12:** `arsenal.rs` gives the six newest weapons their own behaviours — lobbed Meatball Comet
  (RAGÙ RAIN splits x3), Static Cling's hug field (FULL DISCHARGE's periodic nova), the Ricochet
  Disc bouncing enemy to enemy and lapping the planet (THE OMNIDISC), the Whoopee's cone shove +
  stun (THE BROWN NOTE's repulsor ring), the Bell's 4 s tolls marking foes for +crit (THE
  ANGELUS's friendly wisps), the un-hit-combo Yo-Yo (SWORD-YO's garrote) with a HUD combo. The
  §12 evolution fanfare runs on every machine; §13 hitstop only on the local player's kills,
  sparse (50/90/130 ms, evolved +20 ms); the 1.8° shake clamp holds at the camera; stunned foes
  cannot attack. `--weapons base|evo|evolve|<names>` and `--dev --evolvenow` probes.
- **Reconciled/fixed on landing:** one cryo-slow path (`HitMsg.weapon`; P30's `SlowMsg` removed);
  the camera's grade comes from the toon bundle and the fanfare dips relative to it; the melee
  swing is a see-through, shadowless crescent over the arc that hit instead of a solid orange
  slab; the weapon probe judges close-range hits only on runs that brought foes into reach.
- **Verified:** smoke 6/6; `--weapons` base/evo/evolve solo and co-op, with `--fast-boss` (nova
  134 hits, repulsor 117); windowed swings (via a temporary 2 s swing life) and the fanfare;
  co-op 60 s (client 107 proxies, `local_sim=0`).

## 2026-09-27 — P07 lands: the turning sun, night rules and the world gimmicks
- **P07:** `daynight.rs` — ONE sun (`Sun::of`) replacing the fixed `sunward`/`is_night`; the host
  turns it and eats it (Devoured Sun Shard + Cursed Δ every 60 s toward night-lock), both on
  `RunSnapMsg`, clients dead-reckon; lighting follows it everywhere (a lit crash site at the
  start); night = +15% horde pace, spawns 20% closer, +25% kill Gold, the Static fading at
  night; the Moon's Earthside/Farside (Earthlight, brighter gems, +20% elites). `gimmicks.rs` —
  Mars thorn flora snags, Dark Moon spore caps (telegraphed, hazard lane), The Crawl (Static
  massing seen through the crust via an x-ray material, then erupting as The Static rises).
  Headless `--daynight` / `--hazards` probes assert the numbers.
- **Reconciled on landing:** the toon rim now follows `daynight::SunLight` (P03's stand-in sun
  is gone); `daynight::apply_sky` is the one ambient writer and its night end is P36's toon
  night fill (P07's own night ambient fields removed); `--warp` uses the live sun.
- **Fixed on landing:** the Crawl's sites lagged the clock by a frame per level-up (a level-up
  opening mid-frame stops the `playing` systems after `run_clock`), so after a few level-ups
  they were not erupting when The Static rose — pre-Static mass is now derived from the clock.
- **Verified:** smoke 6/6; `--daynight` solo/co-op, `--hazards` on Moon/Mars/Dark Moon solo and
  co-op, `--overflow`; windowed Moon (lit start) and Dark Moon; co-op 60 s, identical layouts.

## 2026-09-27 — P36 lands: the toon art style
- **P36:** `toon.rs` patches bevy_pbr's own lighting module once it loads (3 cel steps, a hard
  specular, a two-step flashlight cone) — every material in every lane gets the look with no
  per-site changes, and crowd enemies keep one draw per kind — plus one fullscreen pass between
  tonemapping and FXAA that inks silhouettes (depth second-difference) and creases (normal
  prepass) and rims sun-facing edges. Per-world `ToonLook` (ink, night fill, saturation,
  exposure, sky); stepped terrain height colours; sparer bloom; thicker ink in high-contrast.
  `--dev --warp noon|dusk|night` gives windowed checks a lit view.
- **Fixed while landing:** charge-shrine rings stood on their edge since v0.1 (an extra
  quarter-turn on top of `frame_quat`) — they now lie flat and outline the 4.2 m zone; props now
  keep 12 m (`START_CLEAR_ARC`) clear of the drop point, so a stage never opens with the camera
  inside a boulder. The sky colour has one writer (`toon::apply_world_look`). P30's meshkit
  winding fix kept over P36's duplicate.
- **Verified:** build, smoke 6/6 (incl. the winding self-check), windowed Moon/Mars/Dark Moon at
  noon, three fresh layouts with a clear start, co-op 50 s with identical layout checksums.

## 2026-09-27 — P30 lands: the solo/save/world/combat known-issues sweep
- **Context:** the second container wipe lost the first builds of P07/P08/P10/P12/P14/P22/P30/P32
  and the partial P11/P13/P36. Rebuilt with GitHub backup branches (`claude/pensive-keller-0cood4-PXX`,
  approved by the user) and a `tools/dev/` harness in the repo. Six lanes then ran until the
  account's **weekly agent limit** (resets 2026-10-02 10:00 UTC) stopped every agent mid-build;
  their work is saved on the backup branches, and the lead now lands it directly.
- **P30:** the save counters bank again (H1: `bank_results` reads a run-long snapshot of the local
  sheet; H2/H3 were already fixed by P01); the save format tolerates new counters, renamed content
  and crashes mid-write (M13); meshkit winds outward (M12 — the astronaut's backpack and limbs no
  longer render near-black in daylight); B/T dev keys need `--dev` (M14); pots are out of the cap
  and overflow dissolves far stragglers into The Static with a per-stage backlog (L16, M16, M17);
  the spatial hash scans only reachable cells (M18); ABANDON clears the pause overlay (M19);
  per-planet skies (L6); props kept clear of interactables; cached terrain and rig assets (L8/L9).
- **Verified:** build (21 warnings, none new), smoke 6/6, `--overflow` solo + co-op (4000 ticks),
  windowed solo 32 s, two-instance co-op 50 s (client 93 proxies, `seq_gaps=0`).
- **Not done:** the independent adversarial review (the agent limit hit first); the lead reviewed
  the save-format change line by line.

## 2026-09-27 — Wave 3: P05 Tomes 8 → 23 + loadout + P06 Movement techs + Antipode Blink
- **P05:** all 15 GDD §7 tomes as stat lines folded by `recompute_stats` (so a joiner's own tomes
  reach the host in `PlayerBuildMsg`), each wired to its effect site (`tomes.rs` has the table);
  10 ranks at 100·1.6^rank Silver, a 4-slot loadout (`TOME_SLOTS_MAX` 8 via quests), `MetaSave.version`
  + `migrate()` (old 20-level tomes → ceil(L/2) ranks, slots rebuilt from quests, unowned tomes
  dropped from the loadout); the TOME LIBRARY screen (`AppState::Tomes`, scrolling card grid);
  HUD CROWD/NIGHT/MOMENTUM and a pause TOMES line; Horizon calls on the pickup lane (+ HorizonSettle),
  Nightfall's beam on `NetItemVis.lamp`; Static Silver paid once at banking; Elite excludes bosses.
- **P06:** new `techs.rs` — the Slam (air slide held → dive → host shockwave that needs a ramp:
  `player::steer` lets a wish steer above run speed but never add to it), Grind-Lines traced from
  the terrain's ridge crests (one merged rail mesh, interactables kept 2.5 m clear), the Antipode
  Blink item (Q, exact antipode, momentum turned about the camera axis, HUD dial) with Boomerang
  Insurance routed through it, F flashlight, 60% air control, Shift slide, slope-boost and slide
  plow; joiner edge presses ride `PlayerInputMsg` as wrapping counts (`EdgePresses`).
- **Integration:** conflicts in 5 files (`net.rs`, `items.rs`, `player.rs`, `main.rs`, `headless.rs`).
  Both branches took `PROTOCOL_ID` 0xA570B0_7, so the merged wire is `_8`; `NetItemVis` carries
  P05's `lamp` then P06's `blink_cd`/`antipode`; netcode notes are 2g TOMES, 2h MOVEMENT TECHS.
  Cross-package fixes: both packages wrote the flashlight's intensity (P05 every frame for
  Nightfall, P06 on the F toggle), so Nightfall would have relit a beam switched off — now
  `sync_flashlights` only keeps the switch and `tomes::apply_flashlights` (ordered after it) is the
  one writer (`FlashlightBeam.on` × Flashlight stat), and the duplicate `FLASHLIGHT_INTENSITY` is
  gone. Found in review: a Dead Man's Tether rewind (or a joiner's snap to its host copy) while
  riding a rail was pulled straight back onto the rail by the next `ride_rail`, and a Slam dive
  in the air survived the teleport — `MoveTech::cancel_moves` now runs on every teleport
  (`blink_body`, `combat::move_astronaut`, `net::reconcile_own_astronaut`). Probe hardening:
  Ricochet's fast-boss check was flaky (every shot spent its pierce before coming down), so the
  probe now brings a shot the real firing path rolled a skip for to the end of its life; the
  `--techs` darkmoon co-op Slam stage could bank a dud when the flight brushed a prop; the XP
  pipeline checks skip `--staticnow` (no kill drops XP while The Static is up); the tome probe
  lays its over-the-horizon gems only for Tome of the Horizon; `--techs` with `--tomes` is refused
  (the two probes stage conflicting scenes on the same ticks).
- **Verified:** build clean (22 warnings, all pre-existing). Smoke 6/6 PASS. Every P05/P06 flag
  passes: `--tomes all` (solo, `--coop2`, fast-boss mars), rank-1 banishment/ascension/duplication,
  rank-3 horizon/ricochet, fast-boss elite/static, `--staticnow` (solo, fast-boss, co-op), rank-5
  co-op duplication/banishment; `--techs` on moon/mars/darkmoon-coop2/coop2/fast-boss-coop2
  (darkmoon repeated ×4), `--items new` (solo + co-op), `--deathsave` and `--deathsave --coop2
  --assist`. Windowed: solo 56 s clean; `--dev --tomes all --items antipodeblink,boomeranginsurance`
  with real keys — F turned the light off, Q blinked (blink_cd 11), ESC's pause panel fits the
  stats, the TOMES line and the new controls line; main menu and TOME LIBRARY render. Real co-op
  70 s: identical layout/interactable sums on both sides, client streams proxies with
  `local_sim=0`, `seq_gaps=0`, no panics.
- **Known gaps:** Nightfall reads a fixed sun until P07 (which must replace `planet::is_night`);
  Microwave Gold price/Salvage discount wait for P16; Duplication/Salvage are host-only until P14;
  Golden Tome max ×1.5 (GDD formula) vs the old ×2.0 is a design call; Antipode Blink mastery
  ladder P21/P26; antipode dial until P24's threat ring; no gamepad (P27); jump hang ~0.73 s vs
  GDD ~1.2 s; no tome/tech numbers playtested; `CLAUDE.md` module map lacks `techs`/`tomes`.

## 2026-09-27 — Wave 2: P03 Items 22 → 38 + P04 Accessibility & display settings
- **P03:** the 15 GDD §7 items (Orbital Yo-Yo, Comet Tail, The Overheat, Downhill Momentum,
  Second Astronaut, Encirclement Bonus, Icarus/Anti-Grav Boots, Little Black Hole, Dead Man's
  Tether, Cracked Helmet, The Static Radio, Widow's Ring, Signal Flare, Devoured Sun Shard) in
  every loot pool; per-copy Rarity Grades (+50% per rung over native) and the Cursed family; one
  `run::roll_item` for every loot source; ONE death-save resolver (Tether > Warden hook >
  Antipode/Boomerang hook > Widow's Ring); host-simulated items with `NetItemVis` + item events on
  the hazard lane; camera glides (never snaps) on a Tether rewind. Fixed: the teleporter never
  opened for a boss killed during The Static; a joiner's gold was sent before its Gold gain.
- **P04:** one tabbed settings panel (GENERAL/DISPLAY/VISION/ASSIST) from menu and pause;
  simulation-picked colorblind palettes for danger + rarity; danger outlines; flash reduction and
  photosensitivity that work through base color (unlit materials ignore emissive) with ONE
  screen-wide flash budget; UI scale 75–150% that fits 720p/Steam Deck; damage-number modes;
  0–100% screenshake; §13 assists (density, enemy damage, "one more chance") that flag the run
  ASSISTED and keep a separate daily board; telegraph rings lie flat and read by shape/motion;
  Burrower crack decals (on the hazard lane for joiners).
- **Integration:** conflicts in 10 files. `Rarity::color` takes the palette, and every palette
  gained a Cursed color (Standard keeps P03's static-magenta, which collapses to ΔE 3.6 against a
  grade for deuteranopes; picked by the same Machado-2009/CIELAB scoring: olive for deut/prot, min
  ΔE 40.7; violet for tritanopes, min ΔE 33.8); the VISION swatches show it. P04's photosensitive
  chain zaps were ported into P03's `fire_volley` (so Second Astronaut's ghost zaps share the
  budget too; an empty chain no longer spends it). `apply_player_hits`: damage = hit × assist ×
  Cracked Helmet's damage taken × armor; on a lethal hit the item resolver goes first and the
  revive token is the LAST link, as both packages documented. Chest/shop/pause take P04's
  fit-to-screen layout with P03's grade colors and graded lines. Both branches had taken
  `PROTOCOL_ID` 0xA570B0_5, so the merged wire is `_6`; HazardEvent keeps P03's variants, then
  P04's `Crack`, appended. Cross-package fixes: the in-run panels' bottom band now follows the
  weapon tray's real height (P03's wrapping item chips put P04's level-up status line on top of
  the second chip row at 150%), the item status line steps aside under panels like the tutorial
  line, the `--assist` probe expects Cracked Helmet's ×2, and `--deathsave` now scripts the whole
  chain (with `--coop2 --assist`: Tether → Widow's Ring → one more chance → down).
- **Verified:** build clean (24 warnings, all pre-existing). Smoke 6/6 PASS plain and with
  `--assist`. Every P03/P04 headless flag passes (ITEMS OK on moon/darkmoon/coop2, DEATHSAVE OK,
  Radio/Shard/Flare fast-boss co-op, ASSIST OK ×4), plus the new combined
  `--coop2 --deathsave --assist` (normal and fast-boss). Windowed: solo 56 s clean, main menu and
  VISION tab (6 swatches) clean; `--items new --assist --give stormcore,deathray --a11y
  deut,outline,flash,photo,ui=125,numbers=crits` ran to results with the ASSISTED RUN line and a
  spent token; level-up at 150% with 15 items sits clear of both chip rows. Real co-op 70 s:
  client streams proxies with `local_sim=0`; a 90 s co-op run with items + assists + outline/photo
  shows item events crossing the wire, both revive tokens spent after the item saves, and the
  joiner's HUD reading WIDOW +30% / TETHER SPENT / ONE MORE CHANCE: USED with the Widow halo.
- **Known gaps:** Boomerang Insurance out of the pools until P06; Warden hook (post-1.0); Sun Shard
  stand-in until P07; joiners don't see teammates' ghost/Anti-Grav volleys, damage numbers or a
  results screen; §13 enemy-outline thickness and clutter merge moved to P24; no playtest of grade
  steps, cursed rates, proc numbers or assist ranges. Seen, pre-existing (not wave 2): the
  results screen always shows LEVEL 1 / GOLD 0 and `counters.best_level` never passes 1, because
  `bank_results` (OnEnter Results) reads the LocalPlayer after `despawn_stage` (OnExit InRun) has
  removed it; some UI strings still use glyphs the font lacks.

## 2026-09-27 — Wave 1: P01 Core rules conformance + P02 Co-op client parity
- **P01:** new `run::scaling` is the one place enemy scaling is worked out (GDD §3 HP/DMG/spawn/
  elite over chain-wide `t`, depth `d`, planet threat `T`, Difficulty `Δ`; spawn rate follows the
  §3 run-arc beats and "breathes" around bosses). Choice economy: 2 free refreshes then
  15·1.5^n Gold (Fortuna always free), Skip pays XP + Gold, 3 Banish charges. Evolution cap 1
  (`evo_cap()` hook for Tome of Ascension). Guaranteed MINIBOSS CACHE after miniboss #1 (free
  pick of 3 items, replicated via `RunSnapMsg.reward_chest`). §10 Silver payout with an itemised
  results breakdown. Fixed never-incremented chest/evolve counters (Chests10 / EvolveWeapon
  quests were uncompletable). Pause screen shows threat + economy.
- **P02:** joiners get parity: per-astronaut Comet Combo (`NetComet`), Mars dust storm on clients
  (`InStorm` hides a peer from ranged enemies), Anubot beam + Beamer aim lines drawn on clients,
  teammates wear their own hero (`NetHero`, hero rides the connect token). Sessions outlive runs
  (`RunOverMsg`, `run_gen`), clean leave/end-session paths with readable reasons, soft client
  reconciliation, and fixes for the HOST CO-OP menu crash and the `--stagenow` co-op insert panic.
- **Integration:** conflicts in `enemies.rs` (imports), `headless.rs` (P01 `balance_probe` + P02
  `tally_xp`; P02's per-link XP-pipeline check kept, it already covers the fast-boss flake P01
  skipped) and `net.rs` (both added `RunSnapMsg` fields). Both packages had bumped `PROTOCOL_ID`
  to `0xA570B0_3` independently, so it is now `0xA570B0_4`. Cross-package fix: windowed
  `--bossnow` (P02) now also winds `total_elapsed`, which P01's scaling reads, so the boss
  arrives with the late-game horde at late-game HP as `--fast-boss`/`--minibossnow` already do.
- **Verified:** build clean (33 warnings, all pre-existing). Smoke matrix 6/6 PASS, plain and with
  `--choices`; every new P01/P02 headless flag passes (RULES OK, CHOICES, miniboss cache on
  moon/darkmoon/coop2, COMETPEER, STORMPEER, PEERHERO, fortuna `--choices` no longer hangs);
  `--fast-boss --coop2` 12/12. Windowed solo 56 s and main menu clean; level-up panel shows
  Refresh/Banish/Skip; solo, co-op host and co-op client pause menus show P01's threat lines
  with P02's session buttons. Real co-op 70 s: client streams proxies with `local_sim=0`;
  `--dev --minibossnow --stagenow` co-op shows the cache on both machines in the same second,
  client rebuilds stage 1 with a matching layout, the wipe ends the run and the session stays
  open; Mars `--bossnow` co-op draws the Anubot beam, aim lines and storm on both sides.
- **Known gaps:** balance not human-playtested (§3 curve is softer in late T1, harder in chained
  worlds; `--balance` probe); Silver income outpaces v0.1 sinks until P05/P21/P26; co-op client
  can't open the miniboss cache (P14); joiners bank nothing (host-only save; no owner yet);
  version mismatch shows only after the ~15 s netcode timeout; several pre-existing UI strings
  still use glyphs missing from the ASCII-only font.

Entries are newest first.
- From Session 2l on, **every commit has its own entry, tagged with its hash**, and dates are git commit dates (US Eastern).
- Sessions 1 to 1m carry their original pre-git dates (2026-07-13). git dates the first commit, 02300e3, to 2026-07-23; see the note in Session 1c. **The commit hashes are authoritative for ordering.**
- The current state of the project lives in PROJECT_STATUS.md. The verified issue list (the H/M/L IDs cited below) lives in docs/KNOWN_ISSUES.md.

## 2026-09-26 — Session 4: two months on — verified catch-up audit, the NEW CREATIVE DIRECTION, the full plan + docs, and publication on GitHub

- **Where it stood:**
  - There had been no commits since 99d012f (2026-07-26 01:23), and the working tree was clean.
  - Whether the planned 2026-07-27 two-PC coworker playtest happened is unknown. No July logs exist on this machine, and the save is fresh.
- **The user's own playtest today (solo, Moon T1, release exe):**
  - Reached **level 25 with 1,525 kills** and died about 5:50 into the stage.
  - The enemy count sat at the **1,200 cap** at about 164-187 fps.
  - Then the user clicked HOST CO-OP; the log ends hosting at CharSelect.
  - The release exe turned out to be stale (see below). None of 99d012f's fixes touch a solo run on a fresh save, so the run's data still stands.
- **Verified catch-up audit.**
  - **Method:**
    - Five subsystem readers (world, horde, economy, netcode, shell) mapped all of src/. **A verifier then re-checked every claim against the code.**
    - Separately: a build and smoke pass, a docs and history digest, and an audit of the co-op docket.
  - **Every reader's bug claim came back confirmed in code.** Several were re-scoped, and the corrections are recorded in docs/KNOWN_ISSUES.md.
  - **Build:** `cargo build` was a no-op, with **0 errors and 35 warnings**. The debug exe matches HEAD.
  - **Smokes:**
    - `--headless 2400`: pass.
    - `--headless --coop2`: pass. It ran the default 1,500 ticks, because the tick count must come directly after `--headless`.
    - `--headless 1200 --fast-boss`: pass, but weakly. The bot died at level 1 with 0 kills; the 0-kills check is skipped for fast-boss.
    - `--headless 1200 --fast-boss --coop2`: **1 of 3 passed.** The root cause is a harness bug, not only non-determinism: the summary at src/headless.rs:317 reads an unfiltered `PlayerState` query and can pick up the peer.
  - **`target/release/astrobonk.exe` is stale.** It was built at 01:19, before HEAD at 01:23, and it lacks the `inter_sum=` string that 99d012f added. Today's playtest ran on it. Rebuild before handing it to anyone.
  - **Three solo save regressions from co-op Stage 1 (35db5ff).** They hit every run played today.
    - **H1 (live):** best_level is stuck at 1. `bank_results` reads the LocalPlayer after `despawn_stage` has removed it, and the Results health line shows `players=0`.
    - **H2 (live):** chests are never counted. The save shows chests=0 although a chest was bought at about 6:58.
    - **H3:** evolutions are never counted.
    - As a result, Overachiever, Cache Money and Ascension can never complete, and the chest price never grows.
  - **Co-op findings, all derived from code:**
    - **H4:** the host latches a peer's jump, slide and interact bits forever.
    - **H5:** the joiner's predicted body is never reconciled with the host's copy.
    - **H6:** a host panel or pause longer than 2 s makes the joiner's horde permanently invisible.
    - **H7:** HOST CO-OP before any run in the process should panic.
    - **H8:** joining while the host is in its menus builds the wrong world.
    - **H9:** no run-end signal reaches a client.
    - **H10:** the Anubot beam and the Beamer aim line are invisible on the joiner.
    - **M1:** the interactable layout RNG can still desync across machines through the Shady Guy stock rolls. This contradicts the docket's "fixed two draws, so the layout is safe".
  - **Playtest signal:**
    - The spawn ramp in code, `(1 + 2.1·t/min)`, is about 15x the GDD's `(1 + 0.14t)`.
    - Overflow is discarded and nothing culls.
    - Bruisers at 5:30 double the mix's mean HP.
    - Pots occupy cap slots.
    - The run reached the cap, and the kill rate collapsed from about 8/s to about 2/s. The code does exactly what it says, so whether this curve is the design is the user's call.
  - **Corrections to the record:**
    - 99d012f's claim that the neutral intent "also stops the interact bit latching" is false, because ORing `false` clears nothing.
    - 17115e9 blamed the `--fast-boss --coop2` flake on non-determinism alone; the harness query is the real cause.
- **NEW CREATIVE DIRECTION, locked by the user.** It is recorded in PROJECT_STATUS.md §2, and nothing in src/ implements it yet:
  - Improve on Megabonk's addictive traits in every way.
  - **The planets are the levels.**
  - **One astronaut hero is trying to get home to his PET TURTLE.** His space suit **visibly upgrades from planet to planet.**
  - **The 12 existing astronauts become 12 SUITS**, keeping their signature weapons and passives.
  - **A roguelite journey:** death sends you back to the crash site, and **suit upgrades and story persist, Hades-style.**
  - **A cartoon-spooky tone**, funny AND frightening, rated E10+.
  - **A toon art style.**
- **Plan and docs, written this session:**
  - `plan/00_MASTER_PLAN.md` is the full-game plan and end goals under the new direction.
  - `plan/08_BUILD_ROADMAP.md` is the ordered roadmap from today's code to the full game.
  - GDD.md is being rebuilt from plan/ into the complete GDD.
  - `docs/TECHNICAL_REFERENCE.md`, `docs/CONTENT_CATALOG_v0.1.md` and `docs/KNOWN_ISSUES.md` capture the code as it stands.
  - README.md is the front door for the public repo.
  - **PROJECT_STATUS.md was rewritten** as the canonical status. It was frozen at co-op Stage 5 (99e768d) and still claimed "NOT yet human-playtested", a client-local horde and no PlayerBuildMsg.
  - **This DEVLOG was backfilled:**
    - it now has entries for 7a69e42 and every commit 35db5ff..99d012f (23 commits);
    - the four stale "UNCOMMITTED" notes now name the commits that landed them;
    - the stale numbers are annotated: the Craterpillar tuning, the fixed-timestep claim, MUSIC_MIX and the line count.
- **Published:** the repository is now public at https://github.com/thesameoldthing88/Astrobonk (`main`, which is 99d012f for the code). It contains all source, Cargo.toml / Cargo.lock and the design docs. `target/` is git-ignored, so the repo holds no exe.
- **Nothing in src/ changed this session, and nothing was rebuilt.** The release exe is still stale.
- **NEXT:**
  - Follow plan/08_BUILD_ROADMAP.md.
  - Until it orders otherwise, the audit recommends **Option A**: fix H1-H3, M19 and M10 plus the headless filter, gate the B and T dev keys, add `#[serde(default)]` to `Counters`, rebuild release and re-run the smokes.
  - Then **Option B**: the co-op pre-fixes H4/H7/H8/H6/M1, then docket items 2-10.
  - The details and the user's open decisions are in PROJECT_STATUS.md §9.

## 2026-07-26 01:23 — Session 3 · docket item 1: two live co-op fixes + a save-independent interactable layout (99d012f)

- **This was the first item of the Director's 10-item co-op docket**, the approved plan for getting ready for a two-PC playtest.
- **Charge shrines were silently dead in co-op.**
  - `charge_shrines` did `q_ps.single()` over every `PlayerState`. With two astronauts that returns `Err(MultipleEntities)`, so the blessing panel was skipped.
  - Meanwhile the shrine still set `done`, counted `shrines_charged`, and played the banner and SFX.
  - Fix: the query is now `With<LocalPlayer>` (src/interact.rs:341). Solo is unchanged.
- **A client re-sent its last input forever while a panel was open.**
  - `gather_local_input` stops during a panel, but `send_local_input` had no phase gate, and the host *assigns* `wish`. A joiner who was holding W when their level-up opened kept sprinting on the host.
  - Fix: the client now sends a neutral intent whenever `RunPhase != Playing` (src/net.rs:763-772).
- **The interactable layout depended on each machine's save.**
  - The cage's `place_dir` draw sat inside `if !save.counters.chimp_freed`, and `place_dir` consumes a variable number of RNG draws.
  - Skipping it shifted every later draw and moved all five charge shrines, so two players with different saves would stand in different rings.
  - Fix: the draw is now unconditional (src/interact.rs:269). The cage itself stays per-machine.
- **Added an interactable-layout checksum (`inter`, `inter_sum`) to `--netlog`**, next to the prop checksum. It reads equal across two instances (inter=20, inter_sum=-75632).
- Smoke paths pass.
- **Later, from the 2026-09-26 audit:**
  - The claim that the neutral intent "also stops the interact bit latching" is **false**. The host ORs the bits (src/net.rs:1011-1013), so a later `false` clears nothing. Jump and slide latch the same way (H4).
  - The layout fix was verified only with **one shared save**, since both instances ran on one PC.
  - `inter_sum` includes the Cage and the host-only Teleporter, so it legitimately differs between opposite `chimp_freed` saves.
  - Shady Guy stock rolls still share the placement RNG and can shift the layout (M1).
  - The release exe was built about 4 minutes **before** this commit and does not contain it.

## 2026-07-26 01:13 — Session 3 · the host shows its LAN address (bdb7cc6)

- **Why:** two PCs on one network cannot use 127.0.0.1. The joiner needs the host's LAN address.
- **What:** clicking HOST CO-OP now writes "HOSTING — tell the other player to join: <ip> (port 5011)".
- **How `local_ip()` works** (src/net.rs:1051): it uses the standard UDP trick. Connecting a datagram socket sends nothing; it only makes the OS pick the interface it would route through. It falls back to loopback when there is no route.
- **Later, from the 2026-09-26 audit (M20):** the note is written in the same click that switches to CharSelect (src/ui/menus.rs:265-270), so it is effectively never seen. `local_ip()` can also pick the wrong interface behind a VPN.

## 2026-07-26 01:02 — Session 3 · session logging for playtests (ca38187)

- **Why:** a tester who double-clicks the exe has no console, so stdout logging gives them nothing to hand back.
- **What:** `src/playlog.rs` writes plain text to `%APPDATA%/astrobonk/logs/session-<unix>.log`. It contains:
  - the version and the exact command line;
  - co-op events: hosting or joining, client state transitions, the assigned PlayerId, peer count, client stage rebuilds;
  - a **health line every 5 s**: role, state, PlayerId, players, enemies, proxies, fps, hp, level, gold, stage, timer and kills;
  - **panics**, through a panic hook.
- **Design choices:**
  - Each line opens the file afresh, so a crash never loses the tail.
  - It deliberately does not use tracing or LogPlugin.
- **Verified:** a joiner's log shows the full arc, from "NET joining" through PlayerId assignment to proxies climbing 16 → 27 → 41.
- **Later, from the 2026-09-26 audit:**
  - The health line lacks the seed, `layout_sum`, `inter_sum`, loss counts, difficulty, items, comet fires, shrine events and the cause of death.
  - `enemies=` includes pots.
  - File names have only one-second resolution.
  - Today's solo playtest log is the only one on disk.

## 2026-07-26 00:54 — Session 3 · the co-op join UI, and the click-through that made one machine host AND join (8194240)

- **Hosting and joining had been CLI-only.** The main menu now has:
  - **HOST CO-OP** and **JOIN CO-OP**;
  - an address entry pre-filled with 127.0.0.1, which accepts digits and dots only (ENTER connects, ESC cancels);
  - a status line, because a failed host on a taken port used to be completely silent.
- **The bug came from the first real (one-PC) playtest.** The logs showed one window both joining and hosting.
  - Cause: a full-screen Bevy overlay does not stop the buttons behind it from receiving `Interaction`. Pressing CONNECT also pressed HOST CO-OP underneath, and `NetRole::Client` was overwritten by `Host`.
  - Fix, in two parts: buttons outside the overlay are ignored while it is open, and HOST/JOIN refuse outright when already networked, on both the button path and the ENTER path.
- **A joiner deliberately does not pick a character or planet.** It waits for the host's seed.
- **Verified:** the roles are mutually exclusive, and the client streams normally (proxies=52, local_sim=0).
- **Later, from the 2026-09-26 audit:**
  - H7: HOST CO-OP before any run in the process should panic.
  - H8: joining while the host is in its menus builds the wrong world.
  - M21: a failed join cannot be retried without a restart.
  - M5: the joiner always plays Buzz.
  - Click-through remains a problem elsewhere: the settings overlay over the main menu can still spend Silver on tomes.

## 2026-07-25 23:47 — Session 3 · Co-op Stage 10: clients follow the host through stage changes (1ce86dc)

- **The problem:** `director::stage_transition` can never run on a client, because its only trigger is the host-only teleporter.
  - So a joiner kept the entire old planet while its HUD described the new one.
  - Its `CurrentPlanet` was never replaced, so every streamed enemy decoded against the wrong radius (Moon 140 m vs Mars 160 m).
- **The fix:**
  - `apply_run_snapshot` compares the incoming `stage` before assigning it and raises `RunSync::pending_stage`.
  - `client_stage_transition` (src/netenemy.rs:775-840) then mirrors only the teardown-and-rebuild half: the StageScoped sweep, reseed, `spawn_stage`, `spawn_interactables`, swapping `CurrentPlanet`, and respawning the local astronaut **carrying its sheet**.
  - The victory and `counters.cleared` writes stay host-only.
- **Ordering is the whole trick.** All client stream systems form one explicit chain with `client_stage_transition` first (src/netenemy.rs:200-214), and it clears the three id→entity maps in the same system.
- **Verified across a live Moon → Mars hop:**
  - host and client both show stage=1, props=259, layout_sum=9066677;
  - the client shows proxies=59 and local_sim=0;
  - the client kept lvl=2, xp=9, hp=66.
- **Harness:** `--stagenow` advances a stage about 20 s in and forces tier 3 at boot. The tier-1 chain is Moon only, so advancing from it hit the victory branch.

## 2026-07-25 18:40 — Session 3 · three breaks-co-op bugs from an adversarial audit of the switch (17115e9)

- **1. A downed client banked a full run to disk on a ~1.6 s loop.**
  - A client holds exactly one `PlayerState`, so `downed_watch`'s `all(dead)` passed the instant the joiner went down.
  - `bank_results` then wrote the host's kills and silver into the joiner's save and ran `check_quests`. `client_follow_host_run` pulled it straight back InRun, and the loop repeated.
  - Fix: `downed_watch`, `death_watch` and `bank_results` are host-only, and `client_follow_host_run` only pulls in from MainMenu or Boot.
- **2. Any trip to Results permanently emptied the client's world.**
  - `NetEnemyIndex`, `NetPickupIndex` and `NetBossIndex` survived the teardown while pointing at dead entities, so `receive_*` refused to respawn those ids ever again.
  - Fix: clear all three on `OnExit(InRun)`, and make despawns tolerate entities that are already gone.
- **3. `gem_merge` ate streamed gems.** It orphaned index entries and left a merged mega-gem that nothing could collect. Fix: `gem_merge` is host-only.
- **Verified live:** local_sim=0, proxies climbing 84 → 91, hp adopted from the host, no panics.
- **Still open at that point:** clients did not follow stage changes. Stage 10 fixed that.
- **Correction, from the 2026-09-26 audit:** this commit blamed the `--fast-boss --coop2` smoke's ~1-in-3 failure on "documented headless non-determinism". The actual root cause is the unfiltered `PlayerState` query at src/headless.rs:317, which can read the peer.

## 2026-07-25 18:22 — Session 3 · Co-op Stage 9: THE SWITCH — clients stop simulating their own world (6367aa8)

- **Until this commit, a joiner ran a full private horde alongside the streamed one.** Every lane it needed now existed, so the client's private simulation was turned off.
- **Measured:**
  - client `local_sim=0` (it simulates no enemies itself);
  - `proxies=55` (everything it draws comes from the host).
- **Gated on `net::is_simulating`** (src/main.rs:185-288): `director_spawn`, `enemy_move`, `enemy_contact`, `burrower_emerge`, the spitter/beamer/lobber attacks, `charge_shrines`, `interact_system`, `pickup_update`, `kill_drops`, `player_upkeep`, `comet_system` and `dust_storm_system`.
  - Why `enemy_move` must be gated: proxies carry a real `Enemy`, so on a client it would steer them with local AI.
- **Deliberately left running on clients:**
  - `craterpillar_update`, which places the streamed worm's segments;
  - `enemy_projectiles`, `telegraphs` and `mortar_shells`, which integrate streamed hazards;
  - `rebuild_hash`, because the client's own weapons still need a spatial index.
- **Added:**
  - `adopt_my_vitals`: the client's hp mirrors the replicated `PlayerVitals`. Verified hp tracked 92 → 81 → 75 → 69 → 58 from damage applied on the host.
  - `animate_net_pickups`: cosmetic bob and spin for streamed gems.
- **Interest management visibly paid off:** `in_interest[p0:95 p1:48]` of mobile=100.
- **Later, from the 2026-09-26 audit:**
  - With the comet and dust storm gated host-side, the joiner has no comet HUD (M11) and never sees the storm (M9).
  - Nothing reconciles the client's own predicted position with the host's copy (H5).

## 2026-07-25 16:39 — Session 3 · Co-op Stage 8: build sync — the host stops simulating a level-1 peer (9b954f1)

- **The problem:** the host spawned peers with `carried: None`, a fresh level-1 sheet built from the HOST's save and character. Card picks happen on the client and never travelled up.
  - Measured before the fix: the host held p1 at "lvl2 dmg1 weps1" indefinitely.
- **The fix: `PlayerBuildMsg`** (src/net.rs:175; Ordered, 2 Hz, ~130 B). It carries the character, level, **derived `Stats`** and (weapon, level) pairs.
  - It is the only client-to-host traffic besides input, and it is unavoidable. Combat uses `thread_rng`, so no lockstep scheme could let the host recompute the peer's numbers.
  - **It sends derived Stats, not items**, because `recompute_stats` folds in the local machine's meta tomes. The host's copy of a peer's `items` is deliberately left stale (commented at src/net.rs:643-646).
- **Three things an adversarial pass caught:**
  - **The character is not cosmetic.** It gates six hero branches the host evaluates every frame: Nova, Gristle, Reticle, Aurora, Ironclad and Yuki.
  - **Max-HP growth must add to current hp.** The client preserves hp as a fraction of max.
  - **Cooldowns are not sent.** Each existing cd is preserved by kind, or a 2 Hz heartbeat would reset them into free bursts.
  - The apply touches only client-owned fields. hp, shield, iframes, powerups and dead stay host-authoritative.
- **Added `--autopick`.** Without it, a bot-driven client stalls on its first level-up panel.
- **Verified:** the host's view of the joiner became "lvl5 dmg1.10 hp104 weps2". In another run the two players held different weapon counts (host 2, joiner 3). All smokes pass.

## 2026-07-25 15:39 — Session 3 · Co-op Stage 7: the pickups lane and the shared XP economy (c5914ca)

- **The problem:** until this commit, the joiner never gained XP and never picked a card.
- **Pickup lane:** an event lane. Only spawn and despawn cross the wire; the idle bob and the fly-to-collector motion are derived locally.
  - `gem_merge` reads as N despawns plus one spawn.
  - The lane is reliable (Unordered), because a lost despawn would leave a ghost gem forever.
- **Grants:**
  - `pickups.rs` emits a local `GrantOut`, and a relay in net.rs turns it into wire messages.
  - **XP is a shared pool**, per GDD co-op canon. It is broadcast, and each machine applies it to its own `PlayerState`, so each player still picks their own cards on their own screen.
  - **Gold, food and powerups go to the collector only** (`SendTargets::Single` via `PeerSlots::client_for`).
- **Avoided the deferred-command trap** again: pickup wire ids are tracked only from entities that actually carry the component.
- **Verified:**
  - The joiner reached level 2, with xp 5 → 8 → 9 driven purely by the host's kills.
  - Grants matched exactly: 6 emitted, 6 received.
- **Regression fixed in the same commit:** headless.rs builds its own App without NetPlugin, so `GrantOut` had to be registered there too. Otherwise every smoke panicked. This is a standing hazard of the duplicated system list.

## 2026-07-25 13:48 — Session 3 · Co-op Stage 6b: the worm gets its body back on clients (70dac72)

- **The Craterpillar had streamed as a lone head.** Its 12 segments now appear on the joiner for **zero extra bytes.**
- **How:**
  - Segments carry no `Enemy` or `Boss` and are not in the spatial hash; they are pure decoration placed from the head's trail.
  - The client attaches a `CraterpillarHead` to the boss proxy and spawns the segments. The existing `craterpillar_update` then animates them with the tuned math from 7a69e42 (whip 0.015 per segment, wave speed 3.9).
  - Segments carry damage 0, since the host resolves contact damage. They self-clean when the head goes.
- **Verified:**
  - worm_segs=12 for the whole fight, with a body spread of 7-11 m (the theoretical maximum is 13.2 m).
  - Bandwidth unchanged at **3.9-4.5 KB/s**.

## 2026-07-25 13:17 — Session 3 · Co-op Stage 6: boss and hazard lanes (4b20500)

- **Boss lane:**
  - Bosses cannot ride the crowd record. `spawn_boss` sets `Enemy.kind = Bruiser` for all four `BossKind`s and picks the mesh from `BossKind`.
  - Bosses are never interest-culled, because the HUD edge marker must point at a boss from the far side of the planet.
  - The lane uses exact f32 positions (`BossSnapMsg`, Unreliable, 20 Hz).
  - The proxy carries `Enemy{hp = streamed fraction, max_hp = 1.0}`, so `update_boss_bar` and `update_edge_markers` work unchanged.
- **Hazard lane:** an **event** lane. Each shot, ring and mortar is fully determined by its spawn conditions plus time.
  - It is reliable (Unordered), because a telegraph is the tell for the attack that kills you.
  - The host emits through `Added<T>`, so no future hazard can bypass it.
  - Client visuals carry damage 0.
- **Correctness fixes:**
  - The boss proxy's attack timers had been set to 0, which would fire every frame; they are now `INFINITY`.
  - `boss_phase_system`, `boss_attacks` and `anubot_beam_system` are host-only.
  - **`apply_hits` and `apply_player_hits` are host-only.** The client had been running its own damage against proxies whose hp is a 0..1 fraction. One local hit "killed" a boss the host still considered alive.
- **Added:** `--bossnow` (it marks the minibosses done), and boss and hazard counters in `--netlog`.
- **Verified live:** a Craterpillar proxy with the correct mesh, `bosses=1` held for the whole fight, and streamed hazards drawn on the client.
- **Not yet done at that point:**
  - the worm segments (done in 6b);
  - the Anubot Verdict Beam overlay;
  - the Beamer aim line, which needs the latched target id.
- **Later, from the 2026-09-26 audit (H10):** the boss record carries `beam_angle` and `beam_state` (src/netenemy.rs:881-882), but the client ignores them (src/netenemy.rs:910-917). The beam and the aim line are still invisible on the joiner.

## 2026-07-25 12:37 — Session 3 · docs: record Stages 4-5 and the enemy-distribution measurement (99e768d)

- Recorded the `--enemydist` finding in PROJECT_STATUS.md: excluding pots, the entire mobile horde sits within 80 m of a player, so there is no far-side horde to cull.
- **This was the last edit to PROJECT_STATUS.md until the 2026-09-26 rewrite.**

## 2026-07-25 12:37 — Session 3 · Co-op Stage 5: replicate the run so both machines build the same world (e2987e9)

- **The problem:** a joiner generated its world from its own `fresh_seed()`. Terrain matched, because it comes from the fixed per-planet seed, but every rock, pot, chest and shrine landed elsewhere.
- **`RunSnapMsg`** (src/net.rs:100-119; Unordered, 4 Hz, ~70 B, about 0.3 KB/s) carries:
  - the seed;
  - the **planet chain**, because it comes from each machine's own menu picks;
  - the clock and the shared counters.
  - It is sent `CLIENTS_ONLY`, never `All`, because `All` loops back into the host's own queue.
- **A client no longer self-starts.** `--autodrop` is ignored with `--join`, and `client_follow_host_run` enters InRun only after the seed lands.
  - The first attempt instead deferred the world build inside InRun. That panicked at once: Bevy 0.18 fails system-param validation on a missing `CurrentPlanet`.
- **`run_clock` is host-only.** A client adopts the clock instead of running a drifting copy.
- **Verified:**
  - HOST and CLIENT both showed seed=1784997367443551800, stage=0, props=196, layout_sum=14562500.
  - The clocks tracked within one snapshot.
- **Later, from the 2026-09-26 audit:**
  - `RunSnapMsg` has no sequence number and omits `greed_stacks`.
  - `push_run_snapshot` has no state gate, so a joiner who connects during the host's menus builds the wrong world (H8).
  - There is still no run-end signal (H9).

## 2026-07-25 12:03 — Session 3 · Co-op Stage 4: stream the horde to clients — the crowd lane (1e7d52f)

- **Why a custom lane:** enemies cannot replicate as entities. The cap is 1,200 per player-scale, up to 3,600 for a party of four. So the crowd rides a custom quantized, interest-managed batch message (`EnemySnapMsg`, defined in src/net.rs:273; the lane that builds and decodes it is the new src/netenemy.rs), while astronauts stay on replicon.
- **The sphere is the compression.**
  - Each record is the enemy's great-circle offset from the **receiving** client's astronaut, in arc metres, stored as two u16s over ±128 m (a 3.9 mm step).
  - An update record is 6 bytes. Decoding uses `sphere::offset_dir`, the exact inverse of the encode.
  - Facing, wobble, stride, hp and speed are derived client-side, not sent.
  - Proxies run the same `animate_crowd`, which was extracted from `enemy_move` in this commit.
- **Measured first, and it corrected the premise.** `--enemydist` showed that, excluding pots, essentially the whole mobile horde sits within 80 m of a player: enemies spawn at 42-58 m and steer inward. So the wins come from per-client locality, from pots never crossing the wire, and from a far-tier round-robin, not from far-side culling.
- **Two bugs found by measurement:**
  - **Duplicate NetIds.** Ids were recorded at insert time, but `commands.insert` is deferred, so an id was freed and handed out twice. 37 enemies were in range and only 8 streamed. Ids are now tracked only from entities that actually carry a NetId.
  - **Record counts were written on chunk 0 only**, so every later chunk was undecodable. Counts are now per chunk, and chunks split on record boundaries.
- **Verified live:**
  - The host streamed 84 of 90 mobile enemies and the client drew 87 proxies, with zero sequence gaps.
  - Bandwidth was **4.6 KB/s**.
- **PROTOCOL_ID was bumped to `0xA570B0_2`. This is the last bump to date** (src/net.rs:36), although Stages 5-8 all changed the wire.
- **At that point the client still ran its own horde alongside the streamed one** (the change was additive). THE SWITCH in Stage 9 removed it.

## 2026-07-25 10:29 — Session 3 · docs: record Stage 3 and the gaps it deliberately leaves (bf1dae0)

- Updated PROJECT_STATUS.md and the NETCODE NOTES in src/net.rs. It named the three known gaps:
  - peers get no level-up panel (closed by Stages 7-8);
  - peers cannot use interactables (**still open**);
  - the client runs its own horde (closed by Stage 9).
- **This was the last edit to the NETCODE NOTES block (src/net.rs:1199-1262).** It has been stale ever since.

## 2026-07-25 10:28 — Session 3 · Co-op Stage 3: make the HOST correct with two players (f0790a1)

- **The problem:** about 30 systems found "the player" with `.single()`. On a 2-player host that returns `Err(MultipleEntities)` and silently early-returns.
- **A classification pass found 89 affected sites.**
  - Presentation (camera, HUD, panels, tutorial, comet) now uses `With<LocalPlayer>`.
  - Per-player simulation (weapon fire, damage, pickups) now iterates.
  - Enemy targeting uses the nearest astronaut by **great-circle arc**, not `Vec3::distance`.
- **The repro came first:** `--headless --coop2`.
  - Before: timer frozen at 600, 0 kills, FAIL.
  - After: level 6, 113 kills, SMOKE OK.
- **Bugs found beyond missing damage:**
  - `run_clock`'s `.single()` sat above the clock tick, which froze the entire run.
  - `burrower_emerge`, `enemy_projectiles` and `telegraphs` leaked entities.
  - The dust-storm dome never spawned on a 2-player host.
  - `boss_phase_system` never reached its phases.
  - **A pre-existing solo bug:** `stage_transition` respawned only `PlayerId(0)` and read its sheet after the StageScoped sweep. This is why the shop rolled at zero luck and Results always showed level 1.
- **Message shapes:**
  - `PlayerHitMsg` gains a required `victim`.
  - `HitMsg` gains `source`.
  - Projectiles, drones, beams and auras gain `owner`.
  - The Beamer and pickups gain a latched `target`.
- **Co-op rules, taken from the GDD:**
  - XP is a shared pool on individual curves (GDD.md:899-900).
  - Gold, food and powerups stay individual (gold: GDD.md:901).
  - The horde scales 1.0 / 1.75 / 2.4 / 3.0 by party size.
  - Shrines charge faster with more players in them.
  - **The run ends only when every astronaut is down.**
- **Verified:** 1p and 2p smokes, both `--fast-boss` paths, and a live two-instance run.
- **Left in a safe state at that point:** no level-up panel for peers, and no interactables for peers (`interact_system` is at Bevy's 16-param cap).

## 2026-07-25 09:23 — Session 3 · docs: bring PROJECT_STATUS.md current with Stages 2a-2e (8d8ed56)

- PROJECT_STATUS.md was 12 days stale and did not mention co-op.
- It now records what was verified working, and states plainly that co-op looked right on the joiner but was **broken on the host**. The measurement: `local_players=2` kills about 30 `.single()` queries.
- It also recorded two traps:
  - the pinned replicon versions;
  - the headless smoke is non-deterministic even with a fixed seed.

## 2026-07-25 09:22 — Session 3 · Co-op Stage 2e: remote player visuals — teammates are drawn and animated (bb1c3bd)

- **New `src/remote.rs`.** Replicated astronauts arrive as bare entities (`PlayerId` + `NetTransform` + `PlayerVitals`). Each one gets the real rig, eased toward the replicated pose and animated by the same `animate_rig` as the local player.
- **THE CENTRAL RULE: a remote astronaut never gets `Player` or `PlayerState`.**
  - Otherwise `player_physics` stomps its transform.
  - It would also make about 30 `.single()` sites silently return `Err(MultipleEntities)`. Two of them fail quieter still: the Anubot beam and Craterpillar contact damage simply stop hurting you.
- **Behaviour-preserving refactors:**
  - `build_astronaut_rig()` split out of `spawn_player`;
  - `animate_rig(RigAnim, RigDrive, …)` split out of `animate_player`;
  - `camera_rig` now filters `With<LocalPlayer>`;
  - seating reconciles against missing bodies, so peers survive a stage change.
- **Identity:**
  - replicon 0.40 has no local-client-id API, and both the client's predicted body and the host's body are `PlayerId(0)`.
  - So the host sends `AssignPlayerId` on an Ordered channel every 500 ms (unauthorized clients drop messages). It is sent Single, never All.
- **Two speed-derivation traps, both silent under-estimates:**
  - integrate with the **local** surface radius, not the nominal one;
  - measure the **chord**, not `angle_between`, because f32 `acos` near 1 quantizes.
  - Derived speed now matches the host's: 8.49 vs 8.50.
- **Verified:**
  - The client builds exactly one rig, with no ghost twin: replicated=2, local_players=1, player_states=1.
  - Measured the next ticket: the host reports `local_players=2` (fixed in Stage 3).
- **Recorded:** the headless smoke is not deterministic even with `--seed`, which was confirmed by diffing two runs.
- **Still open:** remotes use a palette by slot, not their real suit (src/remote.rs:82-85), and they have no slide pose. Each rig also allocates its own meshes and a shadowed SpotLight.

## 2026-07-25 08:54 — Session 3 · Co-op Stage 2d: input routing — the client drives its astronaut on the host (6321938)

- **New `InputIntent`** (src/player.rs:71). It is written either by the keyboard (`gather_local_input`) or by the network (`apply_remote_input`). Movement, physics and combat consume it identically, so the simulation contains no net code.
- **Seating:** the host seats each connected client with a `PlayerId` and an astronaut, and routes that client's `PlayerInputMsg` onto it. Clients send intent every frame and predict locally.
- **Three bugs found while verifying:**
  - **Send ordering.** The send ran after the keyboard gather but not after the bot harness, so it shipped an empty wish. **Rule: anything that writes intent must run `.before(send_local_input)`.**
  - `player_physics` and `animate_player` were still `single_mut()`, so remote bodies never moved.
  - Seating used `Added<ConnectedClient>`, which is lost if a client connects while the host is in a menu. Seating is now state-reconciling.
- **Verified:** a bot-driven client walks a circle, and the host's copy of it traces the same path about one sample late. Solo smoke OK.
- **Added:** `--autodrop`, `--botinput` and `--netlog`.
- **Later, from the 2026-09-26 audit (H4):** `apply_remote_input` ORs the jump, slide and interact bits into the host's copy and never clears them (src/net.rs:1011-1013). After one press, the joiner's host-side body re-jumps or re-slides indefinitely.

## 2026-07-25 08:26 — Session 3 · Co-op Stage 2c: working direct-IP transport (31dd23b)

- **Host and join over UDP through renet's netcode transport**, built from the backend's bundled example rather than guessed APIs.
  - `start_host()` is a **listen server**: the host plays too.
  - Also added: `start_join()`, `disconnect()`, and the CLI flags `--host`, `--join <ip>` and `--port N`.
- **`report_connection`** logs client state transitions and the host's peer count, so a connection is verified rather than assumed.
- **Verified:** two live instances on one machine. The client went Connecting → Connected, and the host reported "peers connected: 1".
- Solo stays the default and opens no sockets.
- **Later:** `disconnect()` is still never called (a dead-code warning). A failed join cannot be retried without a restart (M21).

## 2026-07-25 08:17 — Session 3 · Co-op Stage 2b: replication layer — host-authoritative skeleton (3a1b357)

- **New `src/net.rs`:**
  - `NetRole` (Solo / Host / Client; Solo is today's behaviour with no sockets);
  - a deliberately small replication set: `PlayerId`, `NetTransform` (sphere dir + height + facing) and `PlayerVitals` (hp, level, down);
  - `PlayerInputMsg`, defined as the client-to-host intent channel.
- **A player's build stays local.** Only its visible effects cross the wire.
- Host and Solo push sim state into the replicated components every frame, so the host path is always exercised.
- Remaining work was documented in-file; that became the NETCODE NOTES.

## 2026-07-25 08:09 — Session 3 · Co-op Stage 2a: the netcode dependency de-risked + player identity (a0924d2)

- **Checked crate compatibility before building on it.**
  - bevy_replicon 0.41 pulls in Bevy 0.19, which would put two engines in one binary.
  - 0.40 targets Bevy 0.18 cleanly.
  - **Pinned `bevy_replicon = "0.40.0"`** (Cargo.lock resolves it to 0.40.4). Stage 2b added `bevy_replicon_renet = "0.16.0"` on the same reasoning.
- **Added player identity:**
  - `PlayerId(u8)`, where 0 is local/host and 1 and up are peers;
  - the `LocalPlayer` marker.
  - `spawn_player` now takes an id, a character and `is_local`, and fans extra players out around the drop point.

## 2026-07-25 05:12 — Session 3 · Co-op Stage 1: split RunState into a run-global RunState + a per-player PlayerState (35db5ff)

- **The blocker for co-op:** "there is exactly one player" was baked into 53 call sites across 8 files, and one global `RunState` mixed the clock, stage and boss flags with HP, build, level and gold.
- **After the split:**
  - `RunState` is run-global: timer, stage, chain, boss flags, THE STATIC, shared counters, seed, and aggregated difficulty (the maximum across players).
  - **`PlayerState` is a per-entity component:** hp, stats, weapons, items, level and xp, gold, powerups, and hero-passive timers. This is the GDD's co-op model, where each player owns their own build.
- **Also:**
  - i-frame checks were centralized in `apply_player_hits`;
  - `spawn_interactables` and `spawn_player` take the player's state;
  - two system tuples were split at Bevy's caps.
- **Verified:**
  - 182 compile errors went down to 0;
  - moon and mars-boss smokes are green;
  - a seeded run confirmed that XP, level, gold and item HP still accrue;
  - windowed play OK.
- **Later, from the 2026-09-26 audit: this commit introduced three save-counter regressions that are still live.**
  - **H1:** `bank_results` now reads level and gold from the LocalPlayer's `PlayerState`, which `despawn_stage` has already removed on OnExit(InRun). The save gets best_level=1.
  - **H2:** taking a chest no longer increments `chest_opens` or `chests_opened`.
  - **H3:** an evolution no longer increments `run.evolves`. The increment used to be at run.rs:593 before this commit; `apply_upgrade` now runs on `PlayerState` and never touches `RunState`.
  - Two of these were seen live in the user's save on 2026-09-26: best_level 1 and chests 0.

## 2026-07-25 00:55 — Session 2l: tune the Craterpillar — kill the floppiness (7a69e42)

- **Playtest note:** the worm read as floppy.
  - The root cause was the whip growth from Session 2k: amplitude **increasing** down the body is right for a loose rope and wrong for a muscular worm.
- **New values** (partly reverting 2k's worm numbers):
  - whip 0.06 → **0.015** per segment, so the body holds its shape;
  - lift 0.75 → **0.40**;
  - roll 0.5 → **0.26**;
  - serpentine yaw 0.28 → **0.13**;
  - squash 0.22 → **0.12**;
  - wave speed 4.6 → **3.9**, since a slower wave reads as heavier.
- **Crowd gaits stayed at 2k's exaggerated values**, because those played well.
- These worm values are the ones Stage 6b later reused for the client-side worm body.

## 2026-07-24 — Session 2k: exaggeration pass (the skill's own 1.5x rule, applied)
- Crowd gaits pushed ~2x: waddle 0.13→0.26 (Bruiser 0.20→0.34, heavy rock), inter-footfall
  bob 0.10→0.20, Sprinter hop 0.30→0.55 (a real bound), flier bank 0.10→0.22 + bigger bob,
  chase-lean 0.18→0.30 plus a new fore-aft NOD on the gait, lunge pitch 0.45→0.70 and
  lunge squash 0.12→0.22 (the attack tell now reads across a crowded screen).
- Craterpillar: lift 0.28→0.75 with a whip falloff that GROWS toward the tail, roll
  0.22→0.5, squash 0.10→0.22, and a new quarter-phase-offset YAW so the body serpentines
  side-to-side instead of only bobbing. Reads as a snake now, not a bouncing train.
  **SUPERSEDED about an hour later by 7a69e42 (Session 2l).** The worm read as floppy, so whip
  0.06→0.015/segment, lift 0.75→0.40, roll 0.5→0.26, yaw 0.28→0.13, squash 0.22→0.12,
  wave 4.6→3.9. The crowd-gait and player-gait numbers in this entry still stand.
- Player gait matched so he isn't stiff beside a livelier horde: leg swing 0.55→0.85,
  foot lift 0.09→0.15, arm swing 0.38→0.62, body bob 0.055→0.09, waddle 0.045→0.075.
- Check + fast-boss smoke green.

## 2026-07-24 — Session 2j: the HORDE and BOSSES come alive (masterclass, part 2)
- **Enemies (crowd tier — whole-transform only, so 1200 still batch to one draw call/kind):**
  new `Enemy.stride` advanced by DISTANCE travelled → waddle roll + inter-footfall bob that
  always matches real movement. Per-kind character: Sprinters *bound* (hop arc twice/stride),
  Bruisers rock heavily (0.20 waddle), fliers bank + use stacked sines instead of a gait.
  Lean into the chase; **lunge pitch + squash on a fresh contact hit** (reads as "it just
  swung at you" — an actual gameplay tell). Volume-preserved squash/stretch throughout.
- **Craterpillar:** segments now UNDULATE — a phase-lagged wave travels down the body
  (recipe R6): per-segment lift, roll into the wave, and squash on the down-beat. It ripples
  like a worm instead of sliding like a flat train.
- **Judge Anubot:** hero-tier body tell layered over enemy_move — he REARS UP as the Verdict
  Beam charges (anticipation, scaling with the windup) and lurches forward + shudders while
  it fires. His pose announces the attack, not just the light.
- All 3 smoke variants green (moon worm / mars anubot / normal), windowed render OK.

## 2026-07-24 — Session 2i: PROP COLLISION (bug: player walked through rocks)
- User-reported: props were pure visuals — only the terrain surface was solid, so the
  player walked straight through rocks/boulders/wrecks/beacons/crystals.
- Fix: new `PropCollider { dir, radius, height }` + `PropColliders` resource. `spawn_stage`
  now RETURNS the colliders it placed (rocks >1.1 scale, all boulders, wrecks, beacon
  masts, crystals); inserted as a resource at every call site (main/director/headless).
- `PropColliders::resolve()` does the push-out on the sphere's tangent plane (great-circle
  offset from the prop centre); `player_physics` applies it after the advance step and
  removes ONLY the inward velocity component, so you slide along a rock instead of sticking.
- Height-aware: props you've jumped above don't collide, so you can hop onto/over them.
- Smoke + windowed check green.
- KNOWN GAP (deliberate): enemies still ignore props — the horde flows through rocks. Fine
  for now (1200 enemies × N props is the expensive case; would need the spatial hash), but
  noted for a later pass if it reads badly.

## 2026-07-24 — Session 2h: CODE-ART MASTERCLASS + the astronaut comes alive
- Built a new chadkit skill `code-art-animation` (SKILL.md + recipes.md) — the reference
  Chad asked for: shape-language rules for composed-primitive models, animation-principles
  checklist, math toolkit (easing / springs / framerate-independent smoothing / sine
  stacking / phase waves / quaternion craft incl. our parallel-transport camera lesson /
  2-bone IK / gait / impact), three rig tiers by perf budget, review checklist, and 12
  copy-paste rigs. Mirrored into the plugin cache; memory updated.
  NOTE: chadkit is NOT a git repo — skill is on disk only (offer `git init`).
- **Proved it on ASTROBONK.** Rebuilt the astronaut from one baked static mesh into a
  HERO-TIER JOINT RIG: torso+backpack under a Body joint, helmet+visor under a Head joint,
  and 4 limb joints (arms pivot at shoulders, legs at hips) with meshes authored to hang
  from their pivots. New `Joint { limb, rest, lag }` component.
- New `animate_player` system layering the recipes, always composing from REST:
  R2 walk cycle driven by DISTANCE (stride_len 2.1m, so feet never skate) with anti-phase
  arms; body bob twice/cycle + waddle roll + run lean; R1 breathing idle (fades in as the
  gait fades out) + slow head scan at rest; R5 landing squash scaled by impact speed with
  ease-out-back settle + airborne stretch, volume-preserved; slide tuck; head counter-bob
  (drag layer). Gait amplitude smoothed with 1-exp(-rate·dt).
- Smoke green + windowed render OK.

## 2026-07-24 — Session 2g: ONBOARDING (first-run Mission-Control tutorial)
- New `tutorial.rs`: 6 diegetic radio lines from Mission Control, triggered by what the
  player is doing (move → auto-weapons → gems/level-up → slide → day/night+flashlight →
  miniboss tell). Cyan lower-third line, fade in/out, ~5s each. No modal, no wall of text.
- Fires ONLY on a brand-new player's first non-daily run (save.tutorial_done); veterans
  (any prior run) auto-skip via migrate(); marked done after the first completed run.
- DEV key T replays it (veterans can test). (Still present at 99d012f: src/tutorial.rs:42-45.) Regression smoke green, windowed render OK.
- This closes the last "first 20 minutes" gap — the game now teaches itself for a stranger,
  the prerequisite for a public demo.

## 2026-07-24 — Session 2f: DAILY SEEDED PLANET (cashing the determinism payoff)
- Course-correction (told user): combat-RNG determinism (step 1b) isn't needed for our
  host-authoritative co-op plan or a fair daily; fixed-timestep is co-op-build-time. So the
  smart continuation of the foundations work is its visible payoff — the daily planet.
  (LATER: co-op Stages 1-10 were built with NO fixed timestep. There is still no
  `FixedUpdate` anywhere in src/. Host authority, streamed state and PlayerBuildMsg made it
  unnecessary; see 9b954f1.)
- New: run::today()/daily_seed()/daily_name() (splitmix64 day-seed → "GRIEF-7B" codename).
  Main-menu DAILY button shows today's world + best; picks a hero, then drops onto a fixed
  Moon T1 built from today's SHARED seed (same world + waves for everyone). run.is_daily flag.
- Results screen shows daily score + NEW DAILY BEST; save tracks daily_day/daily_best
  (serde(default) migrates old saves).
- Refactor: collapsed 6 main-menu button queries into one MenuBtn enum query to stay under
  Bevy's 16-system-param cap (adding daily+selected pushed main_menu_input to 18).
- Validated: seeded smokes green, windowed menu renders. First feature built ON the seed
  foundation — proves it end-to-end.
- Committed in a61b927, together with Session 2e's deterministic world seed.

## 2026-07-24 — Session 2e: FOUNDATIONS step 1 — deterministic world seed (path to co-op)
- User chose "the path to co-op". Step 1 of the foundations pass: a seeded, reproducible
  world. New `run::GameRng(StdRng)` resource + `RunState.run_seed` (fresh time-seed per run,
  overridable). Reseeded per stage (run_seed + stage) in enter_run / stage_transition /
  headless_enter.
- World gen is now deterministic BY CONSTRUCTION from the seed: `spawn_stage` (prop scatter),
  `spawn_interactables` (chest/shrine layout + Shady Guy stock; LATER: placement and stock rolls
  sharing one stream is audit issue M1), and `director_spawn` (the
  whole enemy spawn stream: kinds, elite timing) all draw from seeded StdRng instead of
  thread_rng. Terrain was already seed-driven.
- Headless `--seed N` flag added; seeded + normal + Mars smokes green; windowed boot OK.
- This unlocks the DAILY SEEDED PLANET (same world for everyone) as a near-free next feature,
  and is the determinism co-op needs.
- NOT YET deterministic: in-combat rolls (crit/lifesteal/drops) still thread_rng, and dt is
  variable — full run-reproducibility needs step 1b (combat RNG) + step 2 (fixed timestep).
- Committed in a61b927, together with Session 2f's daily planet.

## 2026-07-24 — Session 2d: hero mechanic-passives, batch 2 (roster complete)
- Final three flat heroes now play by mechanic:
  - **Dr. Reticle** — +10% crit + a guaranteed-crit "focus pulse" every ~1.5s (run.reticle_timer
    ticked in upkeep; new run.crit_chance() getter; all 8 combat roll_crit sites routed through it).
  - **Lady Fortuna** — +30% luck + level-up refreshes are ALWAYS FREE (choice_input skips the
    charge for her; button shows "FREE").
  - **Aurora Prime** — +20% size + auras swell +35% while sprinting (run.aura_scale() applied in
    weapon_fire aura branch + aura_follow visual).
- All 12 heroes now have a real signature (6 built + Nova/Gristle/Ironclad + these 3).
- Smoke green for all three. Committed in a697c42 (the prior checkpoint, Sessions 2a-2c, is
  62af260). Since 35db5ff the hero-passive fields and getters (`reticle_timer`, `fast_move`,
  `crit_chance()`, `aura_scale()`) live on the per-player `PlayerState`, not `RunState`.

## 2026-07-24 — Session 2c: hero mechanic-passives (Tier 3 #8, first batch)
- Three flat-stat recruits now play differently, not just with different numbers:
  - **Slipstream Nova** — +15% move, and weapons barely cool down while she's above base
    speed (+1.3 attack speed via a new `run.fast_move` flag set in player_physics). The USP
    hero: keep sprinting to keep firing.
  - **Sgt. Gristle** — +15% dmg, and +40% MORE while below half HP (dynamic in damage_mult).
  - **Old Ironclad** — +90 HP, and armor DOUBLES below 30% HP (new effective_armor_fraction,
    used in apply_player_hits).
- Implemented as conditional layers in the effective-stat getters (attack_speed/damage_mult/
  effective_armor_fraction) rather than static recompute — clean, no per-frame recompute.
- All three smoke green. (Reticle/Fortuna/Aurora still flat for now — next batch.)
- Committed in 62af260, together with Sessions 2a (Quality & Clarity) and 2b (boss phases).

## 2026-07-24 — Session 2b: BOSS PHASES (Tier 2 #7)
- Bosses now escalate on HP thresholds (66% / 33%) via `Boss.phase` + `boss_phase_system`:
  each phase-up ENRAGES (speed ×1.28, damage ×1.22, faster attacks), fires a named banner
  (Craterpillar: BURROW BLOOM → HELMET CHOIR; Anubot: SANDSTORM COURT → FINAL JUDGMENT) +
  roar + shake, and erupts an **encirclement ring of adds** around the player (8→14, elites
  in P3). The fight visibly changes shape as you win.
- Anubot's Verdict Beam reads the phase: faster sweep, shorter telegraph, longer live
  window, shorter rest as he enrages.
- Both fast-boss smokes green, no query conflict (boss_phase_system &mut Enemy+&mut Boss
  disjoint from the read-only beam/other systems). Phase transitions validated in-game
  (bot too weak to threshold a boss).

## 2026-07-24 — Session 2a: QUALITY & CLARITY pass (lead's punch list, tier 1+2)
- **Evolution discovery:** weapon cards now show their evolution + catalyst item
  ("Evolves: MEGA WRENCH (with Protein Paste)" / "(catalyst owned!)"); item cards, chest
  reveals, and Shady Guy stock show "Evo catalyst: <weapons>" (new WeaponKind::catalyst_for
  + run::catalyst_line). The build-chase is finally legible in-game.
- **Off-screen edge markers:** 24-slot pool of colored squares hugging the screen edge,
  pointing at bosses (red), teleporter (green), chests (gold), Shady Guy (purple), charge
  shrines (cyan), the cage (brown). View-space projection w/ behind-camera flip; hidden
  when target is on-screen. On a sphere everything lives below the horizon — now findable.
- **Movement feedback:** slide = whoosh SFX + dust kick + ~9% FOV punch (smoothed);
  successful bunny-hop = "kept it!" chirp + cyan sparkle. New Sfx::Slide/Bhop.
- **Comet juice:** dedicated Sfx::Comet boom-sweep on cash-out (no more borrowed evolve
  sting), rising quarter-charge ticks, and a gold particle tail while the combo is alive.
- **Damage-number merging:** hits within 1.6m fold into one running sum (crit gold wins,
  font grows with the number, life refreshes while feeding). Swarm fights readable.
- Validated: Moon + Mars-boss smokes green, windowed render check alive.

## 2026-07-13 — Session 1m: JUDGE ANUBOT — Mars gets a unique boss
- Mars's stage boss is no longer the generic hulk. **Judge Anubot** = a jackal-headed
  rover-god (tracked chassis, riser neck, elongated jackal head + snout + pointed ears,
  glowing eyes) with a signature mechanic: the **VERDICT BEAM** — a rotating lighthouse
  railbeam. Idle → charge (dim, slow-rotating telegraph) → fire (bright, faster sweep,
  damaging) → idle. Dodge by reading the rotation and using terrain/distance. Runs on top
  of the existing boss slam/burst for a busy, distinct fight.
- New AnubotBeam + AnubotBeamVis components, anubot_beam_system (3 Transform queries made
  disjoint with explicit Without filters — no B0001), unique anubot_mesh + beam mats.
- spawn_boss now branches: worm (Craterpillar) / beam-god (Anubot) / generic hulk.
- Validated: `--headless --planet mars --fast-boss` spawns Anubot + runs the beam, no panic.
- Two worlds now genuinely differ: Moon (worm boss, craters, Earthrise) vs Mars (beam boss,
  dust storm, artillery mix). The "content variety" thesis is proven on a second rock.

## 2026-07-13 — Session 1l: MARS becomes a real world — the migrating dust storm
- Director's call (user-approved): stop polishing the Moon, prove world #2 feels different.
  Committed the whole Moon slice first (e8bd263; git dates it 2026-07-24).
- New `events_world.rs` planetary-event system. First event: Mars **MIGRATING DUST STORM** —
  a storm-cell that wanders the surface on a cycle (arrives ~10s, ~26s active, ~20s gap),
  drifting along a great circle. A translucent dust dome marks it; a screen haze fades in
  while you're inside. **Inside it, ranged enemies can't see you** — Spitters/UFOs/Lobbers
  hold fire and Beamers drop their aim lines. A mobile stealth bubble to ride or flee.
- Wired the storm-blind check into spitter/beamer/lobber attack systems; DustStorm resource
  required, so added it to headless too (+ the system) and a `--planet` smoke flag.
- Dev: Mars unlocked in default + migrate so it's selectable for playtesting. (Still unlocked
  by default at 99d012f, src/save.rs:84. Re-gating needs an explicit removal migration, because
  existing saves persist the unlock.)
- Validated: `--headless --planet mars` shows storm spawned+active, no panic; Moon unregressed.
- NEXT for Mars identity: a unique Anubot boss + maybe dust-devils; then more worlds.

## 2026-07-13 — Session 1k: balance pass v1 (weapon audit + opening pacing)
- Weapon DPS audit (all 16, level-1 single-target): arsenal is HEALTHY — single-target
  hitters (~13-17) trade crowd coverage; aura/chain/spread weapons (~10-12) trade raw DPS.
  No trap picks, no god weapon. Only clear outlier: Sonic Whoopee (low dmg AND low
  single-target while knockback doesn't kill) → damage 10→13 to join the pack.
- Opening pacing: spawn rate (1.5 + 2.1t) → (1.0 + 2.1t) so a level-1 player can learn the
  first ~2 min; mid/late ramp unchanged. Reversible if it now reads too easy early.
- Smoke bonus: runs now show comets=1 (Comet Combo cash-out confirmed end-to-end — the
  detonation screen-clears ~70 enemies). Bot survives ~70s (was ~60s).
- Balance is felt, not spreadsheet'd — this is v1; real tuning needs the player's feedback.

## 2026-07-13 — Session 1j: music → a real song (verse/chorus/beat)
- Feedback: the 8s loop felt like a clip on repeat. Rebuilt as a **32s / 16-bar** piece:
  8-bar VERSE (Am F C G ×2) then 8-bar CHORUS (C G Am F ×2) with a **lead-melody hook**
  (A-minor pentatonic, detuned-saw for thickness) — a proper song that loops every 32s.
- Added a **real beat**: kick (1, 3, +and-of-3), **snare** on 2 & 4, hats, and a crash on
  each 4-bar phrase. Drums now have a baseline (0.5 + 0.5·density) so there's ALWAYS a beat,
  harder as the swarm heats up. Arp goes 8ths→16ths in the chorus.
- New Lead stem (6 total). Adaptive mix + Static detune-flat behavior unchanged.
- Validated: 9s windowed run — synthesizes + plays clean.

## 2026-07-13 — Session 1i: settings menu + aura transparency bugfix
- **Bugfix (user-reported):** aura weapons (Cryo/Cosmonaut's Bell/Static Cling/etc.) drew a
  fully-opaque glowing sphere around the player — blinding you after picking one. Added
  faint see-through `aura_mats` (alpha 0.12, double-sided, dim emissive) used for the aura
  bubble; the projectile/impact mats stay bright.
- **Settings overlay** (`ui/settings.rs`) reachable from the main menu AND the pause menu
  (stepper rows): Master / Music / SFX volume, Mouse Sensitivity, Screen Shake. Persists to
  MetaSave on every change and applies live — sensitivity → camera, shake → shake budget,
  volumes → music/sfx. So you can now tune camera feel yourself (pause → Settings).
- MetaSave gained music_volume/sfx_volume/sensitivity/shake_scale with `#[serde(default)]`
  on the struct so older saves load without wiping progress.
- ESC ordering: settings_panel runs `.after(pause_panel)` + a guard so one ESC closes
  settings without also resuming the run.
- Smoke green; menu renders. Slice remaining: the balance pass.

## 2026-07-13 — Session 1h: adaptive music (zero-asset synthwave)
- New `music.rs` — 5 procedurally-synthesized, phase-locked looping stems (bass/pad/arp/
  drums/static), A-minor synthwave, built in-code like the SFX (no downloaded assets).
- **Adaptive:** each stem's volume is driven live by on-screen enemy density + run phase.
  Arp/drums fade in as the horde tightens; on THE STATIC the melodic stems duck under a
  noise+detuned-drone wash and everything drops to 0.92× speed ("beautiful becoming broken").
  Ducks to 35% under menus/level-up panels. Respects save.volume × MUSIC_MIX(0.55).
  (LATER: MUSIC_MIX is now 0.12, src/music.rs:18, because 100% on the slider was too loud.)
- Wired: bank built at Startup; start/stop on InRun enter/exit; update in Update.
  Bevy 0.18 note: AudioSink.set_volume/set_speed need `&mut AudioSink` in the query.
- Validated via a 10s windowed run (music inits + runs, no panic).
- NEXT: settings menu (music/SFX/sensitivity sliders) then the balance pass to finish the slice.

## 2026-07-13 — Session 1g: THE COMET COMBO (signature scored mechanic)
- Director's call (user-approved): add the game's identity-defining move. New `comet.rs` +
  `Comet` resource. Run with a horde in your wake (enemies within 14m and behind your
  heading) and a charge builds as `Σ(tail · speed · dt)`; fill it before the tail breaks
  (0.7s grace) → **cash out**: AoE detonation (300 + peak·18 dmg in 22m, big knockback),
  bonus Silver = peak, screenshake + hitstop + gold particle burst + "☄ COMET x N!" banner.
- HUD: a warm-amber "☄ xN + charge bar" under the timer that heats to white as it fills,
  then flashes on cash-out.
- Wired into game + headless schedules; reset on run start. Headless `comets=` counter added.
  Smoke green (no panic over 3000 ticks); the flee-happy test bot never sustains a tail long
  enough to fire one — a human easily will (~7s of kiting ~15 enemies). Live-play validation.
- Tuning knobs in config.rs: WAKE_RADIUS, COMET_MIN_TAIL, COMET_CHARGE_GOAL, COMET_RADIUS, COMET_GRACE.

## 2026-07-13 — Session 1f (cont): camera rework — kill the drift/flip/pop
- Playtest: camera felt unsmooth and "clicked into place, not where I aimed." Root causes:
  (1) camera forward was rebuilt each frame from `tangent_frame(p.dir)` + a yaw scalar —
  that basis twists as you move (aim drifts) and hard-flips near the poles (the "click");
  (2) ground-clearance was a hard radial snap applied AFTER the position lerp, tripped
  constantly by the new bumpier terrain.
- Fix: CamRig now stores a **persistent world-space `forward`** vector, parallel-transported
  onto the tangent plane each frame and rotated only by mouse input (no per-frame basis,
  no pole flip). player_input uses the same forward so movement matches the view. Ground
  clamp moved PRE-lerp so clearance eases in instead of popping.

## 2026-07-13 — Session 1f: MOON VERTICAL SLICE begins — the Craterpillar boss
- PM call (user-approved): stop widening, deepen the Moon T1 slice to demo quality.
  Sequence: unique boss → balance pass → foundations (determinism/fixed-step) → music+settings.
- **The Craterpillar** — the Moon stage boss is now an actual segmented worm that laps
  the tiny planet. Head = the damageable Boss (existing HP/hit systems); 12 tapering body
  segments follow the head's distance-based trail (framerate-independent) and deal contact
  damage; body vanishes when the head dies. New armored worm-head + body-segment meshes.
- Fixed a B0001 query-conflict panic (head `&Transform` vs segment `&mut Transform`) with a
  ParamSet + explicit Without filters. Logged in bevy-018-reference.
- **DEV key `B`** summons the current planet's boss on demand (so the Craterpillar is
  testable without surviving 8+ min). REMOVE BEFORE SHIP. (Still present and ungated at
  99d012f: src/enemies.rs:913-936, audit issue M14.)
- Both smoke paths green (fast-boss spawns + runs the worm; normal path unregressed).
- NEXT in the slice: balance pass (spawn curve + 16-weapon DPS parity).

## 2026-07-13 — Session 1e: visual quality pass — detailed models + richer terrain
- **New `meshkit.rs` mesh compositor** — bakes multiple primitives (box/sphere/ellipsoid/
  cylinder/cone/capsule) into ONE mesh with per-vertex color multipliers, so a detailed
  model still renders as a single instanced draw call. Moved the shared `icosphere` here.
- **Enemies** — every kind rebuilt from parts (head/limbs/plates/accents) instead of a
  bald primitive: Shambler gets arms+legs+visor, Bruiser gets shoulder plates + fists,
  UFO gets a dome + underside light ring, Lobber a mortar barrel + legs, etc. Vertex
  colors keep one readable signal color + dark accents. Still 1 mesh/1 material per kind.
- **Astronaut** — full suit mesh (torso, chest panel, shoulders, 2 arms, 2 legs, boots) +
  richer backpack (main + 2 O2 tanks); helmet/visor/tool/flashlight kept separate.
- **Bosses** — imposing generic mesh (plated shoulders, horns, back spikes, fists).
- **Terrain** — planet mesh subdiv 6→7 (4× resolution, crisper mountains/craters); 3 finer
  noise octaves added to the height field (small amplitude, movement-safe).
- **Props** — rocks now 6 angular variants w/ double-noise + non-uniform scale + a darker
  material mix; added big **boulders**, and lore props: crashed-lander **wrecks**
  (half-sunk, some are your dead prints) and radio **beacons** (mast + dish + blinking light).
- Validated: headless SMOKE OK + an 8s windowed run confirms all new meshes render.

## 2026-07-13 — Session 1d: content batch — roster 6→12 heroes, arsenal 10→16 weapons
- **6 new heroes** (all start unlocked for now; quest-gating is a follow-up): Dr. Reticle
  (+20% crit), Slipstream Nova (+20% move), Old Ironclad (+90 HP), Lady Fortuna (+30%
  luck, +2 refreshes), Aurora Prime (+25% size, sig Static Cling), Sgt. Gristle (+25%
  dmg, sig Sonic Whoopee). Added flat-stat Passive variants (CritChance/MoveSpeed/MaxHp/
  Luck/Size) — the GDD's mechanic-bending passives (poison, low-HP rage, no-cooldown-
  while-moving) stay on the backlog as new-system work.
- **6 new weapons + 6 evolutions** mapped onto existing behaviors so they ship today:
  Meatball Comet→Ragù Rain (Rocket), Static Cling→Full Discharge (Aura), Ricochet
  Disc→Omnidisc (Chain), Sonic Whoopee→Brown Note (MeleeArc), Cosmonaut's Bell→Angelus
  (Aura), Yo-Yo of Damocles→Sword-Yo (Orbit). All added to the level-up pool.
- Wired: material map (combat.rs), default unlocks (save.rs), passive→stat (run.rs).
- **Save migration on load** folds newly-default-unlocked heroes/weapons into existing
  saves so returning players get the new content without a wipe.
- Headless `--hero <name>` flag added; smoke-tested aurora/gristle/reticle/nova → all
  SMOKE OK (Reticle's crit build notably out-killed the field).

## 2026-07-13 — Session 1c: the GDD (creativity-to-a-billion pass)
- Authored **GDD.md** — the canonical design bible. Built via a 14-specialist design
  workflow + a creative-director critique pass, then synthesized into one voice.
  Golden thread: **"Everything comes back around."** Subtitle locked.
- Reconciled all canon (Tier = chain length only; Ascension Depth; Rarity Grade; single
  endgame antagonist THE FIRST ONE / superboss THE DEVOURER / Dark Moon boss THE HOLLOW
  COSMONAUT; one death-save-at-a-time rule; Antipode Blink consolidation; Gold/Silver
  only — no "Bonk Bucks"). §15 Canon Ledger holds the reconciled counts + build baseline.
- 8 "make it legendary" upgrades adopted: Comet Combo (scored lap-kill), The Static = your
  real dead runs, NG+ "The Copy," The Gardener archetype, diegetic difficulty, Antipode
  Blink, co-op opposite-pole ultimates, the Daily "GRIEF-7b."
- First git commit of the project made this session. (DATE NOTE: git dates that first commit,
  02300e3, to 2026-07-23, and e8bd263 to 2026-07-24, while Sessions 1-1m are logged as
  2026-07-13. The mismatch is unresolved; the commit hashes are authoritative.)

## 2026-07-13 — Session 1b: first playtest feedback ("looks very good") → terrain + flashlight pass
- **Flashlight**: hand-tool prop + glowing lens on every astronaut, real shadowed
  `SpotLight` (55m, warm white) aimed with facing; global ambient dimmed 140→80 so the
  night side is properly dark and the beam matters.
- **Terrain 2.0**: new `sphere::Terrain` — smooth hills + mask-gated ridged mountain
  chains + craters with raised rims (deterministic per seed). Per-planet `rugged`,
  `craters`, `crater_depth`. Same analytic field still drives mesh + collision + spawns.
- **Bigger worlds**: Moon 115→140m, Mars 130→160m, DarkMoon 90→105m; prop counts scaled,
  chests 5→7, charge shrines 4→5.
- **Flora**: per-world plant archetypes from shared primitives — Moon pale mineral
  spires, Mars rust thorn shrubs, Dark Moon glowing fungus trees.
- Fixed a debug-build integer-overflow panic in the crater-width hash (`wrapping_mul`).
- Smoke re-passed on rugged terrain (bot: 29 kills, lvl 3).
- Playtest #2: mouse-X and A/D were inverted — the sphere tangent frame's positive yaw
  turns LEFT, so mouse-right must subtract yaw; D is `+right` (fwd × up), A is `-right`.
  Handedness now verified against the frame algebra, not vibes.
- Playtest #4 request: enemies that shoot back. Added **Beamer** (long-range sniper:
  tracking aim-line for 1.1s, locks 0.25s before release, fires a 40 m/s railbolt;
  standoff 22m) and **Lobber** (artillery: mortars the player's position with a 1.6s
  full-disc landing telegraph + arcing shell visual; standoff 17m). Gave **UFOs** their
  missing zap (they previously just hovered). Generalized standoff AI: ranged kinds
  approach to preferred range, then circle-strafe / back off. Telegraph gained a
  `ring` flag (boss slam = ring band, mortar = full disc). Mix: Beamers join at 6:00
  elapsed, Lobbers at 9:00. Fast-boss smoke now uses a veteran tome loadout + late-game
  mix so the new attack paths actually run under test.
- Playtest #3: "camera freaks out when enemies get close" — contact hits pinned shake
  trauma at max AND the camera re-aimed from the shaken position, converting positional
  shake into violent rotational jitter. Fix: aim from the unshaken position (rotation is
  now rock-stable), apply shake as a small translation-only offset afterward, guard
  against degenerate look vectors, per-hit trauma 0.25→0.12, slam trauma 0.35→0.22,
  camera terrain clearance 0.6→1.2 for the new mountains.

## 2026-07-13 — Session 1: research deep-dive + full v0.1 build
- Deep research on Megabonk (Vedinad, Unity, Sept 2025, 1M+ sales/2wks): run structure
  (10:00 stages, minibosses at 7:00/2:00, boss → teleporter, Final Swarm at 0:00),
  economy (gold in-run / silver meta), full stat sheet, 21 chars / 29+ weapons /
  77+ items / 23 tomes / ~240 quests, chest & Shady Guy mechanics, movement tech
  (slide/bhop). Digest lives in DESIGN.md Part 1. (The linked devlog's YouTube CC was
  API-blocked from every route; facts sourced from wikis/Wikipedia/guides instead.)
- Designed the tiny-planet twist: analytic sphere terrain (one noise fn = collision +
  mesh + props), horizon-distance spawning, encirclement as core threat.
- Built the whole v0.1 in Bevy 0.18.1, no external assets. ~6.5k lines across 20 modules.
  (That was v0.1. At 99d012f, src/ is 16,042 lines across 40 files.)
- Headless smoke bot (`--headless`, `--fast-boss`) validates the sim: kills, XP,
  level-ups, panel resolution, boss spawn, death. Both variants SMOKE OK.
- Balance nudge post-smoke: spawn curve 2.2+2.4t → 1.5+2.2t /s, shambler dmg 6→5.
- Bevy 0.18 gotchas found (recorded in chadkit bevy-018-reference): cursor options are
  a `CursorOptions` component now (not `Window` fields); B0001 needs `Without<Player>`
  on every mut-Transform enemy query; `Volume::Linear`, runtime `AudioSource { bytes }`
  synth works; `despawn_related::<Children>()`; `BorderColor::all()`.
- NEXT: human playtest (see PROJECT_STATUS.md).
