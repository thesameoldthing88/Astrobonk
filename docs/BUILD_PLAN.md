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

## Creative direction — LOCKED by the user on 2026-09-26 (wins over the 2026-07 GDD)
Recorded in `PROJECT_STATUS.md` §2. Where the 2026-07 GDD conflicts with it, **the direction wins**:
1. Improve on Megabonk's addictive traits in every way (match every loop, then better it).
2. **The planets are the levels** — a journey, not a map picked from a menu.
3. **ONE astronaut hero trying to get home to his PET TURTLE**; his suit **visibly upgrades planet to planet**.
4. **The 12 existing astronauts become 12 SUITS**, keeping their signature weapons and passives.
5. **Roguelite journey:** death sends you back to the **crash site**; suit upgrades and story
   **persist, Hades-style**.
6. **Cartoon-spooky tone** — funny AND frightening, E10+.
7. **Toon art style** (still zero external assets).
Carries over unchanged: tiny-planet sphere mechanics, movement as the skill, the horde + boss arc
per stage, co-op as a pillar. Packages P32–P36 implement the direction; P19 is superseded by P33.
Defects verified by the 2026-09-26 audit live in `docs/KNOWN_ISSUES.md` (stable IDs H*/M*/L*);
P30/P31 sweep them, and every package that fixes one marks it there.

## Rules every package follows
- Read the GDD sections named in the package first; the GDD is the spec, this file is the scope.
- Check what already exists before building (several systems are partial).
- Obey `CLAUDE.md` (co-op gating, headless registration, save migration, wire codes, sphere math).
- New content must be reachable in a normal game (menus/unlocks/pools), not only via flags.
- Add a headless smoke path/flag for anything testable headless; the whole smoke matrix must pass.
- Co-op: anything a client must SEE gets a wire path; anything that SIMULATES is host-only.

## Status
Packages run in parallel tracks (each on its own branch, adversarially reviewed, then merged
into `claude/pensive-keller-0cood4` by an integrator), so they are listed by ID, not by wave.

| Package | Status |
|---|---|
| P01 Core rules conformance | ✅ wave 1. Gaps: balance not human-playtested (tune with `--balance`); Tome of Banishment/Ascension are hooks only (P05); co-op client can see but not open the miniboss cache (P14); per-enemy/boss party HP scaling left to P18 |
| P02 Co-op client parity | ✅ wave 1. Gaps: version mismatch surfaces only after netcode's ~15 s timeout; a joiner's Silver (comet or pickups) goes into the host's shared pot and save, and per-player meta rewards have no owning package yet; client pause still freezes its local view |
| P03 Items 22 → ~40 | ✅ wave 2 (22 → 38). Gaps: Boomerang Insurance kept out of the pools until P06 fills `items::antipode_escape`; Warden Solongo death-save slot is a hook (hero #20 post-1.0); Devoured Sun Shard dims the sun as a stand-in until P07 shrinks the day side from `RunState.sun_shrink`; a joiner doesn't see a teammate's ghost/Anti-Grav volleys (teammate fire isn't streamed); grade/cursed/proc numbers not playtested |
| P04 Accessibility & display settings | ✅ wave 2. Gaps: §13 'minimum enemy-outline thickness' and 'reduce clutter' silhouette merge not built (proposed for P24); co-op clients get no damage numbers and no results screen (pre-existing), so number settings and the ASSISTED results line are host/solo only; settings panel is mouse + TAB/ESC only until P27 |
| P05 Tomes 8 → 23 + loadout | ✅ wave 3. Gaps: Nightfall reads a fixed sun (`planet::sunward`/`is_night`) until P07 turns it; Duplication's free Microwave use and Salvage's Microwave discount wait for P16's Gold price; Duplication/Salvage do nothing for joiners until P14 (host-only interactables); Golden Tome max is ×1.5 per the GDD formula (old saves were ×2.0 at L20) — design call; per-rank numbers not playtested |
| P06 Movement techs + Antipode Blink | ✅ wave 3. Gaps: Antipode Blink mastery/quest ladder is P21/P26 (MoveTech counts slams/grinds/blinks for them); antipode tell is a HUD dial until P24's threat ring; no gamepad binds (P27); §4 jump hang ~0.73 s vs GDD ~1.2 s left for a playtest; slam/rail/blink numbers not playtested; `CLAUDE.md` module map lacks `techs` and `tomes` |
| P07 Day/night cycle, world gimmicks, diegetic difficulty | ✅ (landed by the lead after the weekly agent limit, no independent review). One turning sun (`daynight::Sun`), night rules, Earthside/Farside, thorns, spore caps, The Crawl; sun phase + shrink on RunSnapMsg. Fixed on landing: toon rim/ambient reconciled with P36, the Crawl lagging the clock by a frame per level-up. Gaps: night/day balance not playtested; P08 custom movers (not landed yet) must apply `Sun::enemy_speed` |
| P08 New enemies batch 1 | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P09 Planetary events (2 per world) | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P10 Glitched elite affixes | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P11 New enemies batch 2 | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P12 Weapon depth, evolution fanfare, hitstop canon | ✅ (landed by the lead after the weekly agent limit, no independent review). Six Tier-1 weapons with their own behaviours (arsenal.rs), fanfare, sparse hitstop, stuns. Fixed on landing: slab-shaped melee swing → crescent; one cryo-slow path; fanfare relative to the toon grade. Gaps: teammates' Tier-1 weapon visuals are not streamed to other machines; numbers not playtested |
| P13 Bosses: Hollow Cosmonaut + canon phases | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P14 Co-op peer interactables + loot split | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P15 World 4: PEBBLE + THE HAND | ☐ |
| P16 Interactables & lore objects | ☐ |
| P17 World 5: BRRRR-9 + THE ZAMBONI | ☐ |
| P18 Co-op revive, scaling, drop-in, co-op ultimates | ☐ |
| P19 Branching campaign & tiers | ↪ superseded by P33 (planets are the levels) |
| P20 Bespoke minibosses (2 per world) | ☐ |
| P21 Quests ~50 + mission wall + unlock gating | ☐ |
| P22 Audio: per-world music + meme SFX | ☐ rebuilding — the first build was lost when the container was wiped before it was pushed |
| P23 Ascension Depth I–X, weekly mutator, endless | ☐ |
| P24 VFX & HUD pass | ☐ |
| P25 NG+ "The Copy" + the Static as your dead runs | ☐ |
| P26 Codex, mastery & skins, Unlock Web, daily lives | ☐ |
| P27 Gamepad parity, remapping, rumble, aim assist | ☐ |
| P28 Performance, robustness, dev gating | ☐ |
| P30 Known issues sweep — solo, save, world, combat | ✅ (landed by the lead after the weekly agent limit hit, before an independent review ran). Closed H1-H3, M12-M14, M16-M20, M22, L1, L3-L9, L14-L16, L24-L30 (see KNOWN_ISSUES). Gaps: overflow/cap numbers not human-playtested; the wrench sweep visual is an opaque flash (P12/P24) |
| P31 Known issues sweep — co-op | ☐ |
| P32 One hero, twelve suits | ☐ |
| P33 The Journey: planets as levels, crash site, Hades-style persistence | ☐ |
| P34 Suit upgrades you can see | ☐ |
| P35 Story, the turtle, and the cartoon-spooky tone | ☐ |
| P36 Toon art style | ✅ (landed by the lead after the weekly agent limit, no independent review). Cel lighting via a patch of bevy_pbr's lighting module + one ink/rim post pass; per-world ToonLook. Also fixed on landing: upright charge-shrine rings, props in the drop zone. Gaps: flash-reduction/photosensitivity interplay with bloom only eyeballed; relies on bevy_pbr 0.18.1's lighting anchors (the patch refuses whole and logs if they move) |
| P29 Final audit + docs | ☐ |

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
- Merge note (wave 3): P05 already added `planet::sunward()` and `planet::is_night(dir, sun_shrink)`
  against the fixed sun; `player_physics` sets `PlayerState.night` from it for Tome of Nightfall
  (and the headless tome probe walks to `-sunward()`). The rotating sun must REPLACE these (one
  sun, one `is_night`), not add a second test.

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

### P19 — Branching campaign & tiers (GDD §3, §8 campaign map) — SUPERSEDED by P33
*Superseded 2026-09-27: the locked direction makes the planets the levels of one journey. P33 owns the
campaign structure; the branch-at-teleporter idea below survives only as P33 chooses to keep it.*
Teleporter destination is a branch choice at each boss kill (Luck widens options), using the 5
worlds: T1 Moon; T2 Moon → {Mars | Pebble}; T3 Moon → Mars → {Dark Moon | Brrrr-9}; add T4/T5
chains using available worlds. Planet-select shows unlocked tiers/worlds; co-op: host picks, the
choice streams to clients (chain in `RunSnapMsg`). "Miss a boss → The Static swallows that world".

### P20 — Bespoke minibosses, 2 per world (GDD §9 canon TODO)
Moon keeps Craterpillar Jr + Rover Gone Wrong; author 2 each for Mars, Dark Moon, Pebble,
Brrrr-9 (and PROSPECTOR-9 as Mars alt boss per §9). Unique mesh, one signature attack each,
7:00/2:00 cadence, guaranteed chest after #1. Streamed via the boss lane.

### P21 — Quests ~50 + mission wall + unlock gating (GDD §10, §5 unlock column)
Under the locked direction quests unlock **suits** (P32), not heroes, and feed journey/story
progress (P33/P35). ~50 quests in the six categories (Milestone, Hero Trials, Build Puzzles, Planet Lore, Cursed,
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
Also (assigned at the wave-2 merge; P04 left them unbuilt): §13 readability "minimum
enemy-outline thickness" (a screen-space outline for the crowd: post-process edge pass or a
clip-space vertex-extrusion material, never per-enemy hull children) and the "reduce clutter"
mode that merges distant silhouettes. Keep the in-run panels' bands in sync with any HUD
re-layout (`hud::keep_panels_clear_of_hud` measures the bottom cluster, whose P03 item chips wrap).

### P25 — NG+ "The Copy" + the Static as your dead runs (GDD §2, §10)
Record each death locally (hero, suit colors, name/epitaph, build silhouette: weapons/evos).
Static ghosts wear those suits and names (fallback to generated names). NG+ "The Copy",
unlocked by "Devoured": the Static mirrors your prior evolved builds back at you, difficulty
derived from your own best runs. Co-op: host's records drive the Static; ghost appearance streams.

### P26 — Codex, mastery & skins, Unlock Web, daily lives (GDD §10)
Codex: a card per enemy/boss/item/weapon/hero/world with flavor + lore fragment, revealed on
encounter. Suit mastery (locked direction: heroes are suits, P32): 10 ranks per suit (kills in that suit) → palette recolors +
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
`docs/TECHNICAL_REFERENCE.md`, `docs/CONTENT_CATALOG_v0.1.md` (or a new catalog for the built
game), `docs/KNOWN_ISSUES.md` statuses, GDD §15 ledger counts (do NOT rewrite GDD.md's design — the user is
rebuilding it from `plan/`), and a README with how to build, play, host/join co-op, and controls.

### P30 — Known issues sweep: solo, save, world, combat (`docs/KNOWN_ISSUES.md`)
Re-verify EVERY solo/save/world/combat/economy item in `docs/KNOWN_ISSUES.md` against the CURRENT code
(many lines moved; some items were already fixed by P01–P12 — confirm, don't assume). Fix every item
still open unless a later package in this plan explicitly owns it (then note the owner). In scope:
H1–H3 (save counters: best level, chests, evolutions — Overachiever/Cache Money/Ascension must be
completable), M12 (meshkit winding), M13 (`#[serde(default)]` on `Counters` and every save struct —
do this FIRST), M14 (dev key B/T gating), M16/M17 (spawn ramp/stage scaling — reconcile with P01's
scaling module and the user's 2026-09-26 playtest: cap reached, kill rate collapsed; take pots out of
the cap and decide overflow → The Static per GDD §9), M18, M19 (ABANDON RUN overlay leak), M20, M22,
L1, L3–L9, L14–L16, L24–L30, and the headless harness bug (unfiltered `PlayerState` query in the
smoke summary). Update each item's status in `docs/KNOWN_ISSUES.md` (fixed-in commit / owner package).

### P31 — Known issues sweep: co-op (`docs/KNOWN_ISSUES.md`)
Same method for the co-op items: H4 (latched jump/slide/interact bits), H5 (joiner prediction never
reconciled — add a soft reconciliation toward the host's copy), H6 (host pause > 2 s blanks the
joiner's horde), H7 (HOST CO-OP before any run panics), H8 (joining while the host is in menus builds
the wrong world), H9/H10 and M3, M4, M5, M6, M8–M11, M15, M21, M23, L2, L10–L13, L17–L23 — verify
each against current code (P02/P14 already closed several), fix what is open unless P14/P18 own it.
Also M1 (Shady Guy stock rolls desync the layout RNG). Verify with coop.sh two-instance runs,
including a join while the host sits in menus and a host pause > 2 s. Update statuses in the tracker.

### P32 — One hero, twelve suits (locked direction #3, #4)
The 12 astronauts become 12 **suits** worn by ONE protagonist (the astronaut going home to his pet
turtle). Keep every suit's signature weapon + passive exactly. Rename the concept end-to-end
(character select → **Suit Wardrobe**, hero unlocks → suit unlocks, HUD/results text, quest text,
save fields with a migration that keeps existing unlocks), keep one consistent protagonist identity
(name, silhouette) while each suit changes palette/helmet/trim and the kit. Co-op: each player is
their own astronaut in their own suit (remote rigs wear the right suit — P02 replicated the kind).
Content data stays in `src/content/characters.rs` (rename types only where it clarifies; avoid churn
that breaks every other package — a type alias or staged rename is fine).

### P33 — The Journey: planets are the levels, the crash site, Hades-style persistence (direction #2, #5)
Replace "pick a planet + tier from a menu" with ONE journey home: an ordered sequence of planet
levels (Moon → Mars → Pebble → Brrrr-9 → Dark Moon → … home; use the worlds that exist and make it
data-driven so P15/P17 slot in; keep the teleporter-at-boss cadence between levels; a branch choice
at a teleporter is allowed if it serves the journey). **The crash site** is the hub you wake at: a
small diegetic space (or first-planet landing zone) where the suit wardrobe, tomes/suit upgrades,
quests and journey map live. **Death returns you to the crash site**; persistent progress = suit
upgrades, story beats, unlocks, and the furthest planet reached (with a way to resume/skip ahead
to a reached checkpoint that does not trivialize the run). Daily, co-op host/join, settings stay
reachable. Co-op: host drives the journey; a joiner's own suit/story progress persists on the
joiner's machine (decide per-player vs host-run outcomes and document it). Save migration from the
old tier/unlock data. Headless `--journey` path.

### P34 — Suit upgrades you can see (direction #3, #5)
The suit **visibly upgrades from planet to planet**: each cleared planet (and persistent suit-upgrade
purchases) adds a visible, procedural piece to the rig (e.g. reinforced pauldrons, antenna, jetpack
fins, visor glow, trim lights) with a real gameplay effect, persistent across deaths (Hades-style
mirror/upgrade layer, bought with Silver or earned per planet). Works with every suit's palette,
replicates to co-op clients (teammates see your upgrades), cached meshes/materials (don't regress L9).

### P35 — Story, the turtle, and the cartoon-spooky tone (direction #3, #5, #6)
The narrative spine: a lone astronaut crashes far from home and fights planet by planet to get back
to his **pet turtle**. Story beats per planet (Mission Control / the turtle cam / radio logs /
diegetic text — no voice), delivered at the crash site and on arrival/boss/teleporter moments, with
progress that **persists across deaths** Hades-style (new lines after deaths, reactions to how you
died). An ending: reaching home and the turtle reunion. Tone pass over player-facing text (names,
banners, lore, tutorial, results) to **cartoon-spooky, funny AND frightening, E10+** — no gore, dread
over shock. Keep the existing meme humor where it fits. Codex/lore (P26) and NG+ (P25) align with it.

### P36 — Toon art style (direction #7)
Move from vertex-coloured PBR to a **toon** look with zero external assets: cel-banded lighting
(2–3 steps), rim light, and ink outlines (post-process edge detection from depth/normals, or
inverted-hull on hero/boss meshes), tuned so 1,200 instanced enemies stay cheap. Prefer ONE
central mechanism (a toon material/extension or a post-process pass) over touching every material
site. Re-grade per-world palettes and the night side so it reads toon-spooky; keep bloom sparing;
respect P04's flash-reduction/photosensitivity settings and colorblind palettes; verify with
windowed screenshots on every world and in co-op.

