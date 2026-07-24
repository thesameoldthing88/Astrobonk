# ASTROBONK DEVLOG

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
  testable without surviving 8+ min). REMOVE BEFORE SHIP.
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
- First git commit of the project made this session.

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
- Headless smoke bot (`--headless`, `--fast-boss`) validates the sim: kills, XP,
  level-ups, panel resolution, boss spawn, death. Both variants SMOKE OK.
- Balance nudge post-smoke: spawn curve 2.2+2.4t → 1.5+2.2t /s, shambler dmg 6→5.
- Bevy 0.18 gotchas found (recorded in chadkit bevy-018-reference): cursor options are
  a `CursorOptions` component now (not `Window` fields); B0001 needs `Without<Player>`
  on every mut-Transform enemy query; `Volume::Linear`, runtime `AudioSource { bytes }`
  synth works; `despawn_related::<Children>()`; `BorderColor::all()`.
- NEXT: human playtest (see PROJECT_STATUS.md).
