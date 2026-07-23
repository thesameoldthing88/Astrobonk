# ASTROBONK — Design Document

A Megabonk-style 3D survivors roguelite on **tiny planets**: spherical worlds small enough to
run all the way around, big enough that the horizon only gently curves. Heroes are Earth
astronauts, each with a signature auto-attack. Built in **Rust / Bevy 0.18**.

---

## Part 1 — The Megabonk research digest (what we're mirroring)

Compiled 2026-07-12 from: Wikipedia (Megabonk), megabonk.wiki, megabonk.org, bonkmaster.com,
Steam guides/discussions, Destructoid, ScreenRant, gamerblurb, PCGamesN, movement-tech
writeups, and coverage of Vedinad's devlogs ("I'm Making Vampire Survivors but 3D",
Feb 2025) and PM Films' "Megabonk's WILD 11 Month Development Cycle".

### The game in one line
Third-person 3D Vampire-Survivors-like: auto-attacking weapons, hordes that scale with a
countdown timer, XP-gem leveling with randomized upgrade choices, chunky meme humor, and a
meta layer of unlocks that makes every run feed the next.

### Facts & history
- Solo dev **Vedinad** (pseudonymous; "vedinad" = "danidev" reversed, identity never
  confirmed). Unity engine. Development started **Aug 2024**, released **Sept 18, 2025**
  ($10) after a 2-week delay to dodge Silksong. **1M+ copies in 2 weeks**, peak ~117k CCU.
- Art style: low-poly meme assets; skeletal-animation cost avoided by duplicating/keyframe-
  snapping animation frames and vertex-shader tricks; enemies rendered cheap so *thousands*
  can be alive.
- Marketing: absurd name, meme trailers, demo first (itch + Steam), wishlist grind,
  cross-promo with the Brotato/VS community. The demo → viral clips → launch spike.

### Run structure (the loop we clone)
1. Pick **character** (weapon + passive) → pick **map + tier** → spawn in.
2. Stage timer counts down (**10:00** stage 1; later stages 9:00 / 8:00). Enemies spawn in
   waves that scale with time and **Difficulty** stat.
3. Kill → green **XP gems** → level up → choose **1 of 4** upgrades (weapons / items, with
   rarity rolls Common→Legendary; **Refresh / Skip / Banish** buttons, unlockable).
4. **Minibosses** spawn at the ~7:00 and ~2:00 marks. **Elites** trickle in with loot.
5. **Stage boss** near the end of the timer. Kill it before 0:00 → a **Teleporter** opens →
   next stage (higher tier = more stages chained: T1=1, T2=2, T3=3 + final arena).
6. Timer hits 0:00 with the boss alive (or you linger after winning) → **Final Swarm**:
   endless unkillable-tide ghosts; surviving pays out **Silver**; 6-min survival = unlock.
7. Death or victory → results screen → Silver + quest progress banked.

### Economy
- **Gold** — in-run only: opens chests (escalating cost, pay *after* seeing the item),
  buys from the **Shady Guy** vendor (stock rolled at stage entry; hat color = rarity).
- **Silver** — meta currency: pots (silver variant), Final Swarm, quests, results payout.
  Spent on permanent unlocks and **Tome** levels.
- **Luck** raises reward rarity everywhere (chests, shrines, level-up choices, Moai).

### The stat sheet (Megabonk's actual stat list)
Max HP (+1/level), HP Regen, Overheal, Shield (recharges after 5s), Armor (%, diminishing),
Evasion (%, diminishing), Lifesteal, Thorns, Damage (×), Crit Chance (>100% = Overcrit),
Crit Damage (×), Attack Speed, Projectile Count, Projectile Speed, Size, Duration,
Damage-to-Elites, Knockback, Move Speed, Extra Jumps, Jump Height, Luck, Difficulty
(more/faster/tankier spawns), Pickup Range, XP Gain (cap 10×), Gold Gain, Silver Gain,
Powerup drop chance/potency.

### Content scale at launch (targets to mirror proportionally)
- **21 characters** (2 free: Fox/Firestaff, Sir Oofie/Sword; rest via quests: clear tiers,
  weapon feats, hidden bosses — e.g. Monke freed from a cage, Bush/Bandit as secret fights).
- **29–31 weapons** with evolutions (Sword, Bow, Firestaff, Lightning Staff, Chunkers
  orbitals, Flamewalker trail, Aura, Revolver, Shotgun, Sniper (manual aim), homing rockets,
  Mines, Black Hole, Frostwalker, Poison Flask, Bananarang, Dice, Katana, Tornado, etc.)
- **77–86 items** (stat passives + jokes: Borgar, Moldy Cheese, Credit Card, Za Warudo…),
  some with stack caps; **Microwave** interactable duplicates an item.
- **23 tomes** — pre-run passive loadout leveled with Silver (Damage, Health, Agility,
  Cooldown, Precision, Golden, Silver, XP, Luck, Cursed(+difficulty), Attraction, Size,
  Quantity, Thorns, Armor, Bloody, Duration, Chaos…). Limited slots, more via quests.
- **~240 quests** double as the achievement/unlock engine ("the act of clearing the tier
  *is* the quest").
- **Maps**: Forest (bosses Lil Bark/Chadbark/Bark Vader), Desert (Anubis variants),
  Graveyard (1 tier). Interactables: locked/golden chests, Charge Shrines (15/stage),
  Greed Shrines, Magnet ("Succ") shrines, Challenge shrines, Boss-curse shrine, Moai
  (pick-a-loot + reroll), pots & silver pots, radios (music unlock), cages (need key),
  suspicious bushes (hidden miniboss), Boss Generator, wanted posters.

### Movement (the thing that makes it not-2D)
Jump (+Extra Jumps stat), **slide** (Ctrl: speed on slopes, tactical knockback), **bunny-hop**
speed preservation (build speed first, hop to keep it), air-damage upgrades, one character
climbs. Movement skill = dodging, kiting bosses, and shrine-charging under pressure.

### Why it worked (design takeaways we must not lose)
1. **Instant clarity** — you're always 5 seconds from a dopamine hit (gem, chest, level).
2. **Movement is the skill ceiling** — auto-attack frees hands/brain for traversal.
3. **Everything drips meta** — every run unlocks *something* (quest web).
4. **Meme charm is load-bearing** — names, chunky numbers, screenshake, ragdoll excess.
5. **Performance IS design** — thousands of enemies on screen is the fantasy.

---

## Part 2 — ASTROBONK: the tiny-planet twist

### Fantasy
Earth's space agencies detect a swarm devouring the solar system's small worlds. Astronauts
drop solo onto tiny planets, survive the swarm, bonk the local horror, and hop worlds by
teleporter. Ghost-cosmonauts of failed missions ARE the Final Swarm ("**The Static**").

### The planet is the map — and it fixes genre problems
- **Radius ~110–140 m** (per planet). Circumnavigate in ~90 s at run speed. From a
  third-person camera ~6 m up, the horizon sits ~35–45 m out: *slightly* rounded, exactly
  the brief.
- Enemies spawn **just beyond the horizon** — spawning is diegetic, never pop-in.
- No map edges, no infinite-tile streaming, no getting cornered — but you CAN be encircled,
  which is the real threat model (Megabonk's swarm-splitting via movement still applies).
- Terrain is an analytic noise field: `surface(dir) = R * (1 + hills(dir))` — collision,
  prop placement, and the render mesh all sample one function. No physics engine needed.

### Controls & movement (mirrored)
WASD tangent-plane movement, mouse orbit camera (up = local "radial out"), Space jump
(+Extra Jumps), Ctrl **slide** (burst + low profile, extra knockback), landing-window
**bunny-hop** keeps slide speed. No fall damage. Gravity points at planet core.

### Run structure (mirrored 1:1)
Stage countdown 10:00 → waves scale → minibosses at 7:00 & 2:00 → boss spawns at 1:30 →
kill before 0:00 → **Teleporter** → next planet (Tier = chain length: T1 Moon; T2 Moon→Mars;
T3 Moon→Mars→**Dark Moon** arena) → linger or fail = **The Static** (endless ghosts, pays
Silver, 2-min survival quest). Results screen banks Silver + quests.

### Astronauts (characters — v0.x roster of 6, expandable to 12+)
| Astronaut | Signature weapon | Passive | Unlock |
|---|---|---|---|
| **Buzz** | Wrench (melee arc bonk) | +10% Damage | start |
| **Valentina** | Laser Pistol (auto-shot nearest) | +15% Attack Speed | start |
| **B0-NK** | Rivet Gun (spread) | +0.5% Crit / level | Clear Moon T1 |
| **Yuki** | Kunai (seeks closest) | Slide grants +30% AS for 3 s | Clear Mars T1 |
| **Chimp-O** | Boomerang Antenna | +1 Extra Jump | Free the caged chimp (Moon) |
| **Doug** | Mining Laser (pierce beam) | +25% Gold Gain | 2,500 lifetime kills |

### Weapons (8 + evolutions at v0.x; grows toward ~14)
Wrench→**Mega Wrench** · Laser Pistol→**Gatling Laser** · Rivet Gun→**Riveter 9000** ·
Kunai→**Blade Storm** · Boomerang Antenna→**Satellite Array** · Mining Laser→**Death Ray** ·
Orbital Drones→**Drone Swarm** · Tesla Coil→**Storm Core** · Rocket Pod→**MIRV Pod** ·
Cryo Vent→**Absolute Zero**. Evolution = weapon at max level (7) **+** owning its paired
item, then the next level-up offers the evolution card.

### Items (~18 at v0.x)
Space Borgar (+MaxHP), Moon Cheese (+Regen), Duct Tape (+Armor), Slippery Visor (+Evasion),
Protein Paste (+Damage), Overclocked CPU (+Attack Speed), Fish Bowl Helmet (+Size), Rocket
Boots (+Move), Trampoline Soles (+Jump), Magnet Boots (+Pickup), Lucky Meteorite (+Luck),
Space Credit Card (chests cheaper), Golden Antenna (+Gold), Star Chart (+XP), Laser Sight
(+Crit), Heavy Payload (+Crit Dmg), Extra Battery (+Duration), Splitter Chip (+1 Projectile,
epic), Cursed Moon Rock (+Difficulty & +Luck), Thorn Plating (Thorns), Vampire Visor
(Lifesteal), Caffeine IV (+Projectile Speed).

### Enemies (per-planet palette swap + accents)
Shambler (walker) · Sprinter · Bruiser (tank) · Spitter (ranged lob) · UFO (hover strafer) ·
Burrower (erupts near player) · **Elites** (big, glowing, drop gold burst/chest) ·
Minibosses (Craterpillar Jr, Rover Gone Wrong) · **Bosses**: Moon = **Craterpillar**
(Lil/Mega/Emperor by tier), Mars = **Anubot** (Lil/Big/Judge) · The Static (ghost swarm).

### Planet interactables
Chests (gold, escalating, pay-after-reveal) · Shady Guy (fixed stock, hat = rarity) ·
Charge Shrine (stand in ring → pick 1 of 3) · Greed Shrine (+difficulty, +loot) · Magnet
Shrine (vacuum planet-wide) · Challenge Shrine (timed wave → chest) · Moai head (pick-a-loot)
· Microwave (duplicate one item) · Pots + **silver pots** · Cage (Chimp-O) · Suspicious Rock
(hidden miniboss) · Teleporter. Event: **Meteor Shower** (telegraph rings → impacts);
Mars: **Dust Devil** (wandering tornado that yeets you skyward).

### Tomes (pre-run loadout, Silver-leveled, 3 slots → 5 via quests)
Damage · Health · Agility · Cooldown · Precision · Golden · XP · Cursed (v0.x eight; grows).

### Quests (v0.x ~16, the unlock engine)
Lifetime kill tiers, clear each tier, break pots, charge shrines, open chests, survive The
Static 2:00, evolve a weapon, free Chimp-O, level 20 in one run, etc. Every quest pays
Silver and/or an unlock flag.

### Feel / juice checklist
Damage numbers (pooled, crit pops), hit-flash, knockback, screenshake, hitstop on big hits,
kill-poof particles, gem vacuum curve, chunky UI counters, boss HP bar, synthesized SFX
(procedural WAVs — no downloaded assets), lo-fi space-drone ambience (stretch).

---

## Part 3 — Technical architecture (Bevy 0.18.1)

- **No physics crate.** Analytic sphere terrain + custom kinematic controller (surface
  height = one noise fn). Enemies are Transform+velocity structs steered on the tangent
  plane along great circles; separation via a rebuilt-per-frame spatial hash (quantized
  world-space cells). Cap ~1,200 alive; spawn beyond horizon; cull nothing (planet occludes).
- **Instancing-friendly rendering:** one mesh + one material handle per enemy kind (Bevy
  auto-batches). Procedural low-poly meshes (icosphere/capsule/box composites), vertex-color
  planet mesh (icosphere subdiv 5–6, noise displacement), procedural starfield skybox + Earth
  sprite in the sky, `Hdr` + `Bloom::NATURAL` camera.
- **States:** `AppState { Boot, MainMenu, CharSelect, PlanetSelect, InRun, Results }` +
  `RunPhase` resource (`Playing | LevelUp | Paused | Dead | Swarm`) gating systems; level-up
  pauses virtual time.
- **Messages** (0.18 rename): `DamageMsg`, `KillMsg`, `PickupMsg`, `LevelUpMsg`, `SfxMsg`…
- **Data-driven content:** plain Rust const tables in `content/` (chars/weapons/items/
  enemies/planets/tomes/quests) — same pattern as Crown & Cinder / Fowl Play.
- **Save:** `%APPDATA%/astrobonk/save.json` (serde) — Silver, unlock flags, tome levels,
  quest counters, lifetime stats, settings.
- **Headless smoke test:** `cargo run -- --headless 1200` runs MinimalPlugins + full sim with
  a bot (auto-move, auto-pick upgrades) and asserts a clean 20-min-compressed run
  (chadkit:verify-headless compatible).
- **SFX:** tiny in-code WAV synthesizer → `Assets<AudioSource>` (square/noise blips; zero
  downloaded files).

### Module map
```
src/
  main.rs            app wiring, states, headless switch
  sphere.rs          spherical math: tangent frames, great-circle steering, terrain fn
  config.rs          tuning constants
  save.rs            meta save/load
  stats.rs           Stats struct + StatKind + stacking rules
  content/           characters, weapons, items, enemies, planets, tomes, quests (tables)
  planet.rs          mesh gen, props, interactable placement, skybox, lighting
  player.rs          controller (walk/jump/slide/bhop), camera rig, aim
  enemies.rs         spawner/director, steering+separation, elites, bosses, the Static
  combat.rs          weapon firing systems, projectiles/beams/orbitals/auras, damage,
                     crits, knockback, hit-flash, death, XP/gold drops
  pickups.rs         gems (merge), gold, food, magnets, powerups, attraction
  run.rs             run state, timer, level-ups & upgrade rolls, teleporter, results
  interact.rs        chests, shrines, shady guy, pots, cage, microwave, moai
  events_world.rs    meteor shower, dust devil
  ui/                hud, levelup cards, menus (main/char/planet/results/pause), damage numbers
  fx.rs              screenshake, hitstop, particles
  audio.rs           WAV synth + SfxMsg player
  headless.rs        bot + harness
```

### Performance budget
60 fps @ 1,200 enemies + 300 projectiles + 400 gems on mid hardware. Enemy system O(n)
steering + O(n·k) separation (k≈cell neighbors), no per-enemy children, no skeletal
animation (procedural bob/tilt in one transform write), pooled damage numbers (64), gem
merging above 500 live.

---

## Part 4 — Milestones
- **M0** scaffold: window, states, planet mesh, skybox, camera ✦
- **M1** player on sphere: controller + chase cam + slide/bhop ✦
- **M2** the horde: spawner, steering, separation, contact damage, death ✦
- **M3** combat loop: weapons, damage numbers, XP/level-up UI, gold ✦
- **M4** run shape: timer, minibosses, boss, teleporter, The Static, results ✦
- **M5** world objects: chests, shrines, shady guy, pots, events ✦
- **M6** meta: save, silver, tomes, quests, unlocks, menus ✦
- **M7** juice pass + headless smoke + balance
- **M8+** Mars/Dark Moon planets, roster→12, weapons→14, Steam-shaped polish

*Mirror rule: when in doubt, do what Megabonk does; when the sphere gives us something
better (horizon spawns, encirclement, planet-wide magnet drama), lean in.*
