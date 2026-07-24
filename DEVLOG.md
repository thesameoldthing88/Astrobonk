# ASTROBONK DEVLOG

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
