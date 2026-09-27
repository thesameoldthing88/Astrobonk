# ASTROBONK — Build Plan to 1.0

Source of truth for WHAT to build: `GDD.md` (§14 cut line + §15 Canon Ledger). This file turns
the GDD's **1.0 scope** into ordered work packages. Each package is implemented on its own
branch, merged by an integrator that builds, runs the headless smoke matrix, reviews
adversarially, fixes, and pushes.

## Scope

**IN (GDD §14 "In 1.0, non-negotiable" + milestone exit criteria):** ~12 heroes (built), 16
weapons + evolutions (built; deepen the 6 newest), ~40 items, 23 tomes, 5 worlds with 2 arena
events each, 4-player co-op (revive, drop-in, scaling, peer interactables), daily + weekly
mutator + Ascension Depth I–X + endless + NG+ "The Copy", the quest unlock engine (~50 quests),
Codex/mastery/skins, controller parity, accessibility, per-world music.

**OUT (GDD marks post-launch, or needs a platform we cannot build/test here):** worlds 6–14,
NEW EARTH? / THE FIRST ONE finale, heroes 13–21, boss-rush, PvP *Bonk Royale*, Workshop,
cross-play, dedicated servers, matchmaking, Steam relay/NAT punch-through, online leaderboards
and global ghost replays (no backend), localization translations, per-hero voice barks.
**Deliberately not done:** a fixed-timestep rewrite of the sim — co-op shipped host-authoritative
without it, and changing the integration step without a human playtest would silently retune the
movement feel the GDD calls the skill ceiling.

## Rules every package follows
- Read the GDD sections named in the package first; the GDD is the spec, this file is the scope.
- Check what already exists before building (several systems are partial).
- Obey `CLAUDE.md` (co-op gating, headless registration, save migration, wire codes, sphere math).
- New content must be reachable in a normal game (menus/unlocks/pools), not only via flags.
- Add a headless smoke path/flag for anything testable headless; the whole smoke matrix must pass.
- Co-op: anything a client must SEE gets a wire path; anything that SIMULATES is host-only.

## Status
| Wave | Package | Status |
|---|---|---|
| 1 | P01 Core rules conformance | ✅ wave 1. Gaps: balance not human-playtested (tune with `--balance`); Tome of Banishment/Ascension are hooks only (P05); co-op client can see but not open the miniboss cache (P14); per-enemy/boss party HP scaling left to P18 |
| 1 | P02 Co-op client parity | ✅ wave 1. Gaps: version mismatch surfaces only after netcode's ~15 s timeout; a joiner's Silver (comet or pickups) goes into the host's shared pot and save, and per-player meta rewards have no owning package yet; client pause still freezes its local view |
| 2 | P03 Items 22 → ~40 | ☐ |
| 2 | P04 Accessibility & display settings | ☐ |
| 3 | P05 Tomes 8 → 23 + loadout | ☐ |
| 3 | P06 Movement techs + Antipode Blink | ☐ |
| 4 | P07 Day/night cycle, world gimmicks, diegetic difficulty | ☐ |
| 4 | P08 New enemies batch 1 | ☐ |
| 5 | P09 Planetary events (2 per world) | ☐ |
| 5 | P10 Glitched elite affixes | ☐ |
| 6 | P11 New enemies batch 2 | ☐ |
| 6 | P12 Weapon depth, evolution fanfare, hitstop canon | ☐ |
| 7 | P13 Bosses: Hollow Cosmonaut + canon phases | ☐ |
| 7 | P14 Co-op peer interactables + loot split | ☐ |
| 8 | P15 World 4: PEBBLE + THE HAND | ☐ |
| 8 | P16 Interactables & lore objects | ☐ |
| 9 | P17 World 5: BRRRR-9 + THE ZAMBONI | ☐ |
| 9 | P18 Co-op revive, scaling, drop-in, co-op ultimates | ☐ |
| 10 | P19 Branching campaign & tiers | ☐ |
| 10 | P20 Bespoke minibosses (2 per world) | ☐ |
| 11 | P21 Quests ~50 + mission wall + unlock gating | ☐ |
| 11 | P22 Audio: per-world music + meme SFX | ☐ |
| 12 | P23 Ascension Depth I–X, weekly mutator, endless | ☐ |
| 12 | P24 VFX & HUD pass | ☐ |
| 13 | P25 NG+ "The Copy" + the Static as your dead runs | ☐ |
| 13 | P26 Codex, mastery & skins, Unlock Web, daily lives | ☐ |
| 14 | P27 Gamepad parity, remapping, rumble, aim assist | ☐ |
| 14 | P28 Performance, robustness, dev gating | ☐ |
| 15 | P29 Final audit + docs | ☐ |

---

## Packages

### P01 — Core rules conformance (GDD §3, §10, §15)
Bring the run's rules to canon.
- Difficulty scaling: implement the §3 formula shapes for enemy HP/damage/spawn rate/elite chance
  over run-time `t`, chain depth `d`, planet multiplier `T`, Difficulty stat `Δ`. Centralize in
  one module (e.g. `run::scaling`) so Ascension/mutators/co-op scaling can multiply in later.
- Choice economy (§3 table): Refresh = 2 free/run then Gold rising per use; Skip = small XP boost
  + Gold tip; Banish = 3 charges/run (+ hooks for items/Tome of Banishment). Keep Fortuna's
  free refreshes. Card count: keep the current count unless the GDD ledger says otherwise.
- Evolution cap: 1 evolved weapon per run, +1 with Tome of Ascension (expose a hook, `evo_cap()`).
- Guaranteed chest after miniboss #1 (§3 run arc). Silver payout = §10 formula (keep existing
  quest/Static payouts consistent; results screen shows the breakdown).
- Files: `run.rs`, `director.rs`, `enemies.rs` (director_spawn, elite roll), `ui/panels.rs`.

### P02 — Co-op client parity (GDD §11; gaps found in code audit)
- Joiner Comet Combo: host computes the combo for EVERY astronaut (per-player state on
  `PlayerState` or a component), cash-out applies to that player; client HUD shows its own combo
  (stream the minimal state).
- Mars dust storm on clients: stream storm dir/radius/active; per-astronaut `InStorm` marker so
  ranged enemies ignore ANY astronaut inside the storm (not just the host).
- Anubot verdict beam visible on clients (BossRec already carries angle/state — render it);
  Beamer aim lines visible on clients (hazard/event lane).
- Remote astronauts wear their own hero: replicate `AstronautKind` (and slide state) so
  `remote.rs` builds the right suit and tucks on slide; host spawns peers with THEIR character.
- Leave/disconnect: pause-menu "Leave session"; client returns to main menu cleanly when the
  host disappears; host keeps running when a client leaves. Use `net::disconnect`.
- Version safety: bump `PROTOCOL_ID`; mismatched builds must refuse, with a readable status line.
- Gate `enemies::debug_spawn_boss` to non-clients (and later `--dev`, see P28).
- Files: `net.rs`, `netenemy.rs`, `remote.rs`, `comet.rs`, `events_world.rs`, `ui/menus.rs`,
  `ui/panels.rs` (pause).

### P03 — Items 22 → ~40 (GDD §7)
- Add every "New items" entry from §7 EXCEPT Antipode Blink (P06 owns it): Orbital Yo-Yo, Comet
  Tail, The Overheat, Downhill Momentum, Second Astronaut, Encirclement Bonus, Icarus Boots,
  Anti-Grav Boots, Little Black Hole, Dead Man's Tether, Cracked Helmet, The Static Radio,
  Widow's Ring, Signal Flare, Devoured Sun Shard (+ Boomerang Insurance routed as an item whose
  effect calls a blink hook P06 will fill; stub it safely).
- Rarity grades scale the same effect; stack caps (Legendaries/marked usually 1–2); Cursed grade.
- Death-save stacking rule (§7): at most ONE death-save per would-be-death, priority Dead Man's
  Tether → (Warden hook) → Boomerang Insurance/Antipode → Widow's Ring. One central resolver.
- Items with a visual (orbiting debris, trail, black-hole pull, ghost co-pilot) must be visible to
  co-op clients (use existing lanes) and simulate host-side only.
- Devoured Sun Shard exposes a "sun shrink" value P07 consumes (store on RunState).
- All items enter the level-up/chest/shop pools; catalyst lines stay correct.
- Files: `content/items.rs`, `stats.rs`, `run.rs`, `combat.rs`, `player.rs`, `pickups.rs`.

### P04 — Accessibility & display settings (GDD §13)
- Settings (persisted in `MetaSave`, reachable from main menu AND pause): colorblind palettes
  (Deuteranopia/Protanopia/Tritanopia) applied to danger/rarity colors; high-contrast "danger =
  white outline" telegraph mode; flash-reduction toggle (evolution flash, bloom clamp);
  photosensitivity mode (throttle strobe/flicker < 3/s); UI scale 75–150% (`UiScale`);
  damage numbers mode Full / Merged-only / Crits-only / Off + size; screenshake slider (exists).
- "Difficulty as options": enemy density, enemy damage sliders + "one more chance" revive token;
  a run using them is flagged (`RunState.assisted`) and shown on results.
- Telegraphs must read by SHAPE + motion, not color alone (pulsing ring, dashed aim line).
- Files: `ui/settings.rs`, `save.rs`, `ui/numbers.rs`, `fx.rs`, `ui/hud.rs`, `enemies.rs` (telegraph visuals only).

### P05 — Tomes 8 → 23 + loadout (GDD §7 Tomes)
- Add the 15 new tomes with real effects: Orbit, Encirclement, Nightfall, Gravity, Swarm, Salvage,
  Ricochet, Momentum, Vampirism, Elite, Banishment, Duplication, Horizon, Static, Ascension
  (+1 evolution slot via P01's `evo_cap()`).
- Each tome 10 ranks, cost `100 × 1.6^level` Silver; loadout = 4 slots (quests widen it later —
  expose the slot count on the save). Old saves migrate without losing tome levels.
- Tome menu redesign to fit 23 (scrolling/grid), shows rank, effect, next cost, equipped state.
- In co-op each machine's tomes fold into its own sheet (already true via build sync — verify).
- Files: `content/tomes.rs`, `save.rs`, `run.rs`, `ui/menus.rs`, effect sites in `combat.rs`/`pickups.rs`.

### P06 — Movement techs + Antipode Blink + flashlight (GDD §4)
- Orbital Slingshot: hold-drop **Slam** mid-air converts horizontal speed into a radial AoE
  shockwave on landing (damage scales with speed; host-simulated).
- Grind-Lines: ridged-mountain crests expose grindable spines (derive from `sphere::Terrain`
  ridge mask); slide onto one to rail at +50% speed with locked footing; visible spine mesh.
- Antipode Blink: item (Rare) + mechanic — teleport to the exact opposite point (cooldown);
  HUD tell showing enemy density at the antipode; Boomerang Insurance auto-blink at 20% HP (fill
  P03's hook). Must work for co-op peers (host executes, clients snap via REMOTE_SNAP_ARC).
- Flashlight toggle (F) per §13; air control per §4 table (don't retune ground feel).
- Slide on Shift as well as Ctrl/C (§4 table). Camera laws hold.
- Files: `player.rs`, `sphere.rs`, `planet.rs` (spine visuals), `ui/hud.rs`, `content/items.rs`.

### P07 — Day/night cycle, world gimmicks, diegetic difficulty (GDD §4, §8, §12)
- The planet's sun direction rotates so the terminator sweeps the surface over the stage;
  helper `is_night(dir)` / `daylight(dir)` used by gameplay.
- Night risk/reward (§4): enemies +15% move speed & spawn 20% closer on the night side; kills at
  night drop +25% Gold.
- Moon Earthside/Farside: Farside = lower ambient, gems glow brighter, elite rate +20%.
- Mars: thorn flora slows on contact. Dark Moon: fungus detonates spore clouds (telegraphed);
  "The Crawl" — Static massing visible through the ground as markers before it erupts.
- Diegetic difficulty (§3): Cursed Δ and the Devoured Sun Shard (P03) shrink the day side every
  60 s toward night-lock.
- Co-op: sun phase in `RunSnapMsg` so both machines light identically; hazards host-simulated.
- Files: `planet.rs`, new `daynight.rs`, `events_world.rs`, `enemies.rs` (spawn/speed hooks), `pickups.rs`.

### P08 — New enemies batch 1 (GDD §9)
Rollo (rolls the great circle, accelerates downhill, can't turn sharply), Trencher (burrows a
visible ridge then uppercut-launches), Aegis Drone (front shield cone — flank it), Sunskimmer
(kamikaze diver, audible whine), Beacon Tick (touch plants a 6 s tracker that paths enemies to
you), Mimic Chest (disguised loot; 360° shockwave then flees), Longshot Beamer Prime (leads
target, fires over the curve). Distinct silhouette + one emissive accent each (meshkit), per-world
spawn tables, mix timing in the director. Stream kinds to clients (`kind_code`), hazards via the
event lane. Files: `content/enemies.rs`, `enemies.rs`, `netenemy.rs`, `interact.rs` (mimic).

### P09 — Planetary events, 2 per world (GDD §8, M3)
Event scheduler (fires in the 4:30–2:30 "hazards live" window, readable at horizon range).
Moon: EARTHRISE ECLIPSE (25 s, flashlight + ambient die, rim-lit silhouettes) and METEOR SHOWER
(telegraph rings → impacts). Mars: MIGRATING DUST STORM (exists) and DUST DEVILS (roaming
tornadoes fling you/enemies into low-gravity float). Dark Moon: WHITEOUT (15 s static-snow, lost
astronauts walk across the sky) and SPORE BLOOM. Banner + audio cue per event. Stream event
state to clients. Headless `--event <name>` forces one. Files: `events_world.rs`, `net.rs`,
`netenemy.rs` (event lane), `ui/hud.rs`, `planet.rs`.

### P10 — Glitched elite affix system (GDD §9)
Elites roll 1–3 stacked affixes scaling with Difficulty/Cursed: Overclocked, Leaden, Warden,
Contagious, Magnetar, Nightborne, Meteoric, Cursed-Touched — each with its aura color, effect and
counterplay as specified. Guaranteed loot; Cursed-Touched drops a Legendary + mini-Static. Stream
affix bits in the enemy spawn descriptor so clients draw the auras. Files: `enemies.rs`,
`content/enemies.rs`, `combat.rs` (hit hooks: Magnetar, Warden shield), `netenemy.rs`, `pickups.rs`.

### P11 — New enemies batch 2 (GDD §9)
Splitshroom (+3 Sporelings on death), Chorus Node (buried summoner pulsing adds every 4 s),
Orbiter Wisp (orbits at fixed radius firing inward), Graviton (local gravity well dragging the
player toward the horde — player physics hook, host-side), Tidewalkers (move only at night,
sunrise petrifies them into destructible cover — uses P07), Static Herald (giant slow cosmonaut
that mutes SFX cues near it), Glowspore Horror (night-only Dark Moon elite, loot piñata). Same
streaming/silhouette rules as P08.

### P12 — Weapon depth, evolution fanfare, hitstop canon (GDD §6, §12, §13)
- Make the six newest weapons play as §6 describes (they currently reuse generic behaviors):
  Meatball Comet lobbed mortar (+Ragù Rain splits into 3), Static Cling hug aura (+Full
  Discharge periodic nova), Ricochet Disc bounces enemy-to-enemy and can lap the planet
  (+Omnidisc infinite bounces), Sonic Whoopee cone knockback + stun (+Brown Note repulsor ring),
  Cosmonaut's Bell tolls every 4 s and marks foes for +crit (+Angelus resurrects slain as
  friendly wisps), Yo-Yo of Damocles scales with un-hit move combo (+Sword-Yo garrote).
- Evolution fanfare (§12): desaturate, gold shockwave ring, assembly pop, brass sting hook.
- Hitstop canon (§13): only on the player's kills — 50 ms, elites 90, bosses 130 + 0.15 s
  dilation to 0.85×, evolved weapons +20 ms. Shake budget table (§13).
- Files: `combat.rs`, `content/weapons.rs`, `fx.rs`, `audio.rs`.

### P13 — Bosses: Hollow Cosmonaut + canon phases (GDD §9)
- THE HOLLOW COSMONAUT (Dark Moon stage boss, unique mesh): P1 Long Walk (only fightable on
  the night side), P2 Static Bloom (rift drags The Static in early), P3 Void Collapse (gravity
  inverts on his beat, pulling toward the antipode).
- Craterpillar P3 HELMET CHOIR: segments detach into homing helmets.
- Judge Anubot P2 SANDSTORM COURT (Aegis Drones + wrapping dust wall), P3 FINAL JUDGMENT
  (Beamer-Prime pillars at the poles cross-firing the equator).
- Telegraphs on the terrain, red reserved for danger. Everything visible on clients (boss lane +
  event lane). Headless `--fast-boss --planet darkmoon` exercises it.

### P14 — Co-op peer interactables + loot split (GDD §11)
- Peers can use chests, Shady Guy, charge shrines, Moai, Microwave, pots, cage, teleporter:
  client sends an interact request; host validates range/cost against THAT player's sheet and
  resolves; choice panels are shown on the PEER's screen and the pick travels back.
  Split `interact_system` (prompt vs action) — it is at Bevy's 16-param limit.
- Loot split (§11): chest opening deals a card to EVERY player (opener rolls at +1 Luck); gold
  individual; charge-shrine blessing for whoever charged it.
- Headless `--coop2` bot exercises a peer opening a chest.

### P15 — World 4: PEBBLE + THE HAND (GDD §8)
Marble-tiny white-dwarf-lit world: pick a radius where the horde still reads (lap in well under
20 s); spawning/interest/director distances must scale with radius (they assume ~140 m now).
Gimmick: your piercing/boomerang shots lap the world and hit foes behind you. Boss THE HAND
(tries to pick the planet up — unique mesh + 3 phases). Two events. Palette, flora, props, music
hook. Extend every planet-indexed table (see CLAUDE.md). `--planet pebble`.

### P16 — Interactables & lore objects (GDD §2, §7, DESIGN.md Megabonk-isms)
Golden chests (Legendary-weighted), boss-curse shrine (summon a boss early for loot),
challenge shrine (timed wave → chest), radios (looped final transmissions as text; unlock a
music track), suspicious rock (hidden miniboss), Microwave per §7 spec (duplicate one grade lower
OR gamble an upgrade; capped Legendary blocked; Tome of Duplication free use), Shady Guy per spec
(1–2 per stage, hat color = rarity, stock rolled at stage entry), wrecks with AXIOM memos, and the
9 Dark Moon lore fragments (collectibles feeding P21's "Devoured" quest). All placed from the
seeded stream unconditionally (CLAUDE.md rule 5). Co-op usable (P14's request path).

### P17 — World 5: BRRRR-9 + THE ZAMBONI (GDD §8)
Cyan ice world: frictionless movement (glide, never stop — world-specific physics params, not a
global retune), aurora sky. Boss THE ZAMBONI (resurfaces lanes, leaving slick/damaging tracks;
3 phases). Events: AURORA CURTAIN (descending damaging light-wall) + one more. Two-world content
rules as P15. `--planet brrrr9`.

### P18 — Co-op revive, scaling, drop-in, co-op ultimates (GDD §11)
- Down & revive: at 0 HP become a Tumbling Beacon (rolls downhill along the terrain gradient,
  vertical flare visible planet-wide); teammate stands in radius 3 s to revive; Static Meter
  fills while down (cap → claimed until next teleporter, rejoin at 50%); no self-revive; Hero's
  Adrenaline (+20% move 5 s) for the rescuer. Solo unchanged (or uses P04's revive token).
- Scaling table (§11): spawn 100/175/240/300% (exists), per-enemy HP 100/110/120/130%, boss HP
  100/165/225/285%. Friendly fire off for damage, ON for knockback physics.
- Drop-in: a client may join mid-run; lands at half the squad's average level with 30 s autopilot
  grace. Up to 4 players end-to-end.
- Opposite-pole ultimates: STATIC CASCADE (two Tesla/Storm Core owners on opposite hemispheres
  wrap the sphere in lightning) + the named duo combos surfaced on the results screen.

### P19 — Branching campaign & tiers (GDD §3, §8 campaign map)
Teleporter destination is a branch choice at each boss kill (Luck widens options), using the 5
worlds: T1 Moon; T2 Moon → {Mars | Pebble}; T3 Moon → Mars → {Dark Moon | Brrrr-9}; add T4/T5
chains using available worlds. Planet-select shows unlocked tiers/worlds; co-op: host picks, the
choice streams to clients (chain in `RunSnapMsg`). "Miss a boss → The Static swallows that world".

### P20 — Bespoke minibosses, 2 per world (GDD §9 canon TODO)
Moon keeps Craterpillar Jr + Rover Gone Wrong; author 2 each for Mars, Dark Moon, Pebble,
Brrrr-9 (and PROSPECTOR-9 as Mars alt boss per §9). Unique mesh, one signature attack each,
7:00/2:00 cadence, guaranteed chest after #1. Streamed via the boss lane.

### P21 — Quests ~50 + mission wall + unlock gating (GDD §10, §5 unlock column)
~50 quests in the six categories (Milestone, Hero Trials, Build Puzzles, Planet Lore, Cursed,
Hidden) including the §10 named examples; counters tracked from data already in the sim; hero
unlocks per the §5 unlock column for the 6 recruits (existing saves keep what they have);
"Devoured" (9 fragments) unlocks NG+; loadout-slot rewards for tomes. Mission-wall UI with
categories, progress bars, hidden entries revealed on completion. Client never banks quests for
the host's run — each machine counts its OWN player's feats (define which counters are per-player).

### P22 — Audio: per-world music + meme SFX (GDD §12)
Music per world (Moon synthwave exists; Mars desert-rock/industrial; Dark Moon dark-ambient
dungeon-synth; Pebble; Brrrr-9) with the adaptive stem system; boss/miniboss combat layer;
Static heartbeat sub-bass. SFX: hero quip-blip on spawn, cartoon BONK on Buzz wrench crit,
Shady-Guy jingle, "ba-DUMP" level-up, 4-note evolution brass sting, rarity riser; voice cap +
priority ducking. All synthesized in code.

### P23 — Ascension Depth I–X, weekly mutator, endless (GDD §10)
Ascension Depth I–X stacking (Thin Air, Long Night, Hungry Horizon, Iron Ghosts, Shrinking World,
Gold Drought, Two Bosses, Gravity Sickness, The Watching, Absolute Zero Hour), unlocked
progressively, each clear = permanent star + Silver multiplier. Weekly mutator from a pool
(Boomerang Week, Everything's On Fire, Big Head Enemies, Reverse Gravity Tuesdays, …) chosen by
week seed, shown on the main menu. Endless "Forever Alone": decline the teleporter, Static
intensifies, wave counter. Co-op: host's settings stream. `--ascension N`, `--mutator <name>`.

### P24 — VFX & HUD pass (GDD §12, §13)
Deaths pop into faceted shards + puff; elites burst into a firework + gem fountain; gems streak
on a curved arc; full-planet gem vacuum on level-up; crit hit-flash yellow; Static identity
(film grain, scanline flicker, edge desaturation overlay); level-up cards with rarity glow and
evolution-ready glow; HUD layout per §13 (timer + threat tier top-center, HP + XP bottom-left,
loadout with level pips bottom-right, off-screen threat arrows incl. Beamer aim).

### P25 — NG+ "The Copy" + the Static as your dead runs (GDD §2, §10)
Record each death locally (hero, suit colors, name/epitaph, build silhouette: weapons/evos).
Static ghosts wear those suits and names (fallback to generated names). NG+ "The Copy",
unlocked by "Devoured": the Static mirrors your prior evolved builds back at you, difficulty
derived from your own best runs. Co-op: host's records drive the Static; ghost appearance streams.

### P26 — Codex, mastery & skins, Unlock Web, daily lives (GDD §10)
Codex: a card per enemy/boss/item/weapon/hero/world with flavor + lore fragment, revealed on
encounter. Character mastery: 10 ranks per hero (kills as that hero) → palette recolors +
procedural hats + rank-10 Mastery Perk. Unlock Web: Silver-bought branching node map as the
cosmetic/unlock Silver sink. Daily: three lives + local best board.

### P27 — Gamepad parity, remapping, rumble, aim assist (GDD §13)
Full pad support per the §13 control map (move, camera, jump, slide, interact, flashlight,
card pick/banish/refresh/skip, pause, menus navigable by D-pad/stick); full remapping for KBM
and pad persisted in settings; rumble per §13 (hit low-freq, pickups high-freq, boss-slam ramp);
soft aim assist for pad (default on, adjustable); optional 40 fps cap setting (Deck).

### P28 — Performance, robustness, dev gating (GDD §14 M1/M8)
Enemy overflow above the cap merges into The Static; cheap far-hemisphere AI (LOD-by-horizon);
perf overlay (fps, frame ms, enemy/projectile counts) behind a key; dev keys/flags (B boss, T
tutorial) gated behind `--dev`; clean the compiler warnings; soak: several long headless runs
across every world/ascension without panic.

### P29 — Final audit + docs
Whole-game audit against GDD 1.0 scope; fix gaps; update `PROJECT_STATUS.md`, `DEVLOG.md`,
GDD §15 ledger counts, and a README with how to build, play, host/join co-op, and controls.
