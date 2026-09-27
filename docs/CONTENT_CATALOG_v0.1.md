# ASTROBONK — Content Catalog, build v0.1

This is an exhaustive catalog of every piece of game content in the **current build**, with the exact numbers read from the Rust source. It is written for anyone, human or AI, who picks up the project cold and needs to know what is in the game *today*, not what the design documents hope for.

- **The code is the source of truth.** Every number below was read from `src/` and is cited as `path:line`. Where `GDD.md` says something different, the code value is given and the disagreement is flagged with **GDD:**.
- **Design intent lives elsewhere.** `GDD.md` is the full-vision design, and `DESIGN.md` is the older design note. This catalog does not repeat their unbuilt content except to mark a disagreement.
- **Known bugs that change the numbers are called out inline** (for example, the chest price never rises). IDs such as H1–H3, M1, M9 or M17 are the stable IDs of `docs/KNOWN_ISSUES.md`, which has the full write-up, repro and suggested fix for each. §27 also uses its L-numbers for Low items; rows marked "low" there have no tracker entry of their own.
- **Derived values** (for example "Laser Pistol reach = 66 m") are computed from the cited constants and labelled *derived*. They were not measured in a running game.

---

## Contents

1. [Conventions and symbols](#1-conventions-and-symbols)
2. [Run structure and the stage clock](#2-run-structure-and-the-stage-clock)
3. [Astronauts (12)](#3-astronauts-12)
4. [Weapons (16 base + 16 evolutions)](#4-weapons-16-base--16-evolutions)
5. [Items (22)](#5-items-22)
6. [Tomes (8)](#6-tomes-8)
7. [The stat sheet (27 stats)](#7-the-stat-sheet-27-stats)
8. [Levelling: the XP curve and the level-up cards](#8-levelling-the-xp-curve-and-the-level-up-cards)
9. [Rarity and luck](#9-rarity-and-luck)
10. [Enemies (8 kinds + The Static)](#10-enemies-8-kinds--the-static)
11. [Spawning, the enemy cap and time scaling](#11-spawning-the-enemy-cap-and-time-scaling)
12. [Elites](#12-elites)
13. [Minibosses and stage bosses](#13-minibosses-and-stage-bosses)
14. [THE STATIC](#14-the-static)
15. [Planets, tiers and chains](#15-planets-tiers-and-chains)
16. [Interactables](#16-interactables)
17. [Pickups, powerups and drop tables](#17-pickups-powerups-and-drop-tables)
18. [Economy: gold, silver and the results payout](#18-economy-gold-silver-and-the-results-payout)
19. [Quests (16)](#19-quests-16)
20. [The Comet Combo](#20-the-comet-combo)
21. [The daily seed](#21-the-daily-seed)
22. [Player movement and combat-feel numbers](#22-player-movement-and-combat-feel-numbers)
23. [All `config.rs` constants](#23-all-configrs-constants)
24. [Other hard-coded constants](#24-other-hard-coded-constants)
25. [Co-op rules that change content](#25-co-op-rules-that-change-content)
26. [Code vs GDD: consolidated disagreements](#26-code-vs-gdd-consolidated-disagreements)
27. [Known bugs that distort the catalog numbers](#27-known-bugs-that-distort-the-catalog-numbers)

---

## 1. Conventions and symbols

| Symbol / term | Meaning | Source |
|---|---|---|
| **timer** | The stage countdown shown on the HUD, in seconds. It counts **down** from the stage length to 0. | `RunState.timer`, `src/run.rs:106`; decremented in `src/director.rs:61` |
| **elapsed** (`e`) | Seconds since the **current stage** began. It counts **up**, and it is **reset to 0 on every stage change** (`src/director.rs:197`). It keeps counting during The Static (`src/director.rs:52`). | `RunState.elapsed`, `src/run.rs:107` |
| **t** | `elapsed / 60`, in minutes. Used by time scaling and the spawn rate. | `src/content/enemies.rs:239`, `src/enemies.rs:574` |
| **D** | World difficulty: the **maximum** `stats.difficulty` over all astronauts, recomputed every frame. | `src/player.rs:851-852` |
| **P** | Party scale: `[1.0, 1.75, 2.4, 3.0]` for 1, 2, 3 or 4 astronauts. It counts every `Player` entity, downed ones included. | `src/enemies.rs:568` |
| **AS** | The astronaut's effective attack-speed multiplier (`PlayerState::attack_speed()`). | `src/run.rs:278-290` |
| **size** | Weapon level size factor × `stats.size`. | `src/combat.rs:252` |
| **arc** | Great-circle distance along the planet surface, in metres. | `sphere::arc_dist`, `src/sphere.rs:154-156` |
| **distance** | Straight-line 3D distance (chord). Most hit tests use this, not arc. | e.g. `src/combat.rs:188` |
| **stack** | One copy of an item. Items are stored as `(ItemKind, count)`. | `src/run.rs:147` |

All distances are metres, all speeds are metres per second, all times are seconds. Damage and HP are plain numbers. Unless noted, a multiplier stat has a base of `1.0` and an additive stat has a base of `0`.

Two different "radius" ideas appear below:
- **Planet radius** is the sphere radius in metres (105–160 m).
- **Enemy scale** is the enemy's size factor. All enemy hit and contact radii are derived from scale; see §10.

---

## 2. Run structure and the stage clock

A **run** is a chain of 1–3 **stages**. Each stage is one planet with a countdown timer. The chain is fixed by the starting planet and the **tier** (§15).

### 2.1 Stage lengths and scripted marks

| Constant | Value | Meaning | Source |
|---|---|---|---|
| `STAGE_SECONDS` | `[600, 540, 480]` | Countdown length of stage 1, 2 and 3 | `src/config.rs:40` |
| `MINIBOSS_MARKS` | `[420, 120]` | Timer values (counting down) for miniboss #1 and #2 | `src/config.rs:41` |
| `BOSS_MARK` | `90` | Timer value for the stage boss | `src/config.rs:42` |

The marks are **timer** values, so they land at different **elapsed** times on each stage:

| Event | Stage 1 (10:00) | Stage 2 (9:00) | Stage 3 (8:00) | Source |
|---|---|---|---|---|
| Stage starts | timer 10:00 / e = 0 | 9:00 / e = 0 | 8:00 / e = 0 | `src/director.rs:195-197` |
| Miniboss #1: **Craterpillar Jr** | timer 7:00 / e = 180 | 7:00 / e = 120 | 7:00 / e = 60 | `src/director.rs:75-84` |
| Miniboss #2: **Rover Gone Wrong** | timer 2:00 / e = 480 | 2:00 / e = 420 | 2:00 / e = 360 | `src/director.rs:75-84` |
| Stage boss (Craterpillar or Judge Anubot) | timer 1:30 / e = 510 | 1:30 / e = 450 | 1:30 / e = 390 | `src/director.rs:87-96` |
| Boss killed → teleporter opens | whenever the stage boss dies, **unless The Static is already active** | same | same | `src/director.rs:99-103`, `55-59` |
| **THE STATIC** begins | timer 0:00 / e = 600 | 0:00 / e = 540 | 0:00 / e = 480 | `src/director.rs:106-114` |

### 2.2 How a stage ends

- **Teleporter (next stage or victory).** Pressing E on the teleporter sets `PendingStage = stage + 1` (`src/interact.rs:545-549`).
  - If a further stage exists, the old stage is torn down, every astronaut's `PlayerState` is carried over, and a new planet is built (`src/director.rs:179-231`).
  - If the chain is finished, the run is a **Victory** (`src/director.rs:165-177`).
- **Death.** When **every** astronaut is downed, the run result becomes Death (`src/director.rs:240-252`). Results open 1.6 s of real time later (`src/director.rs:254-269`).
- **Abandon.** ABANDON RUN in the pause menu sets the result to Death (`src/ui/panels.rs:577-581`).
- **The Static never ends a stage by itself.** It keeps spawning until the party leaves by an already-open teleporter or dies.

### 2.3 What resets at each stage change

Reset (`src/director.rs:195-207`): timer, elapsed, `boss_spawned`, `boss_dead`, `minibosses_spawned`, `static_active`, `static_timer`, `teleporter_open`, `microwave_used`, the `Director` (spawn bank, elite timer back to 45 s), and `GameRng`, which is reseeded to `run_seed + stage`.

Carried: each astronaut's full `PlayerState`: level, XP, gold, weapons, items, HP, powerups, refresh and banish charges, and banned items (`src/director.rs:183-187`, `220-225`). Also carried: `greed_stacks`, kills and the other run counters.

**Consequence (M17):** the enemy mix and time scaling both read `elapsed`, so **stages 2 and 3 restart at ×1.0 enemy HP with a Shambler-only mix**. Nothing reads the stage index or the tier to make later stages harder. The only exception is the victory silver bonus (§18).
- **GDD:** "each chained world … adds a flat Difficulty step" (`GDD.md:244-245`) and scaling by chain depth `d` (`GDD.md:256-266`). Neither is built.

### 2.4 Pause model (affects every timer in this document)

Any `RunPhase` other than `Playing` (LevelUp, Modal, Paused, Dead) pauses `Time<Virtual>` (`src/fx.rs:47-58`). The main simulation chains (horde, weapons, interactables, clock, Comet, dust storm, gem merge) are also gated on `Playing` (`src/main.rs:47-49`, `185-262`). The always-on in-run set (`src/main.rs:264-287`: hit resolution, physics, `player_upkeep`, stage transition) is not gated on `Playing`, but its time-based systems see `dt = 0` while virtual time is paused. So the stage clock, spawning, cooldowns and powerup timers all **freeze** while a card panel, chest, shop or pause menu is open.

**Hitstop** runs virtual time at **×0.06** for its duration (`src/fx.rs:38-40`):

| Trigger | Hitstop | Source |
|---|---|---|
| Stage boss killed | 0.25 s | `src/combat.rs:962` |
| Comet cash-out | 0.22 s | `src/comet.rs:111` |
| Weapon evolved | 0.18 s | `src/ui/panels.rs:237` |

---

## 3. Astronauts (12)

Defined in `src/content/characters.rs:67-215`. Every astronaut starts the run with:
- **one weapon at level 1**, the "starting weapon" below (`src/run.rs:210`);
- **3 banishes and 2 refreshes** (`src/run.rs:217-218`);
- **full HP**: base 100, plus the passive, tomes and so on (`src/run.rs:226-227`).

### 3.1 Roster

| # | Name (in-game) | Agency line | Description | Starting weapon | Passive (stat) | Passive text shown | Source |
|---|---|---|---|---|---|---|---|
| 1 | **BUZZ** | NASA (retired, angry) | "Fixes problems. Percussively." | Wrench | `DamageMult(0.10)`: **+0.10 Damage** | "+10% Damage" | `characters.rs:70-81` |
| 2 | **VALENTINA** | Roscosmos legend | "First woman to bonk in orbit." | Laser Pistol | `AttackSpeed(0.15)`: **+0.15 Attack Speed** | "+15% Attack Speed" | `characters.rs:82-93` |
| 3 | **B0-NK** | Assembled from spare probes | "Beeps in a threatening manner." | Rivet Gun | `CritPerLevel(0.005)`: **+0.005 Crit Chance × level** | "+0.5% Crit Chance per level" | `characters.rs:94-105` |
| 4 | **YUKI** | JAXA special ops | "The vacuum makes no sound. She does." | Kunai | `SlideFrenzy{bonus 0.30, secs 3.0}` | "Slide grants +30% Attack Speed for 3s" | `characters.rs:106-117` |
| 5 | **CHIMP-O** | Project Mercury alumnus | "They never brought him home. He waited." | Boomerang Antenna | `ExtraJumps(1)`: **+1 Extra Jump** | "+1 Jump" | `characters.rs:118-129` |
| 6 | **DOUG** | Asteroid mining union, local 7 | "Paid by the rock." | Mining Laser | `GoldGain(0.25)`: **+0.25 Gold Gain** | "+25% Gold Gain" | `characters.rs:130-141` |
| 7 | **DR. RETICLE** | Sniper school (dishonorably retired) | "Never misses. Rarely welcome." | Laser Pistol | `CritChance(0.10)`: **+0.10 Crit Chance**, plus a mechanic | "+10% Crit; a guaranteed-crit focus pulse every ~1.5s" | `characters.rs:142-153` |
| 8 | **SLIPSTREAM NOVA** | Artemis stowaway | "Stop moving and you stop living." | Cryo Vent | `MoveSpeed(0.15)`: **+0.15 Move Speed**, plus a mechanic | "+15% Move; weapons barely cool down while sprinting" | `characters.rs:154-165` |
| 9 | **OLD IRONCLAD** | Apollo-era, welded into his suit | "Does not move. Cannot be moved." | Rocket Pod | `MaxHp(90.0)`: **+90 Max HP**, plus a mechanic | "+90 Max HP; armor doubles below 30% HP" | `characters.rs:166-177` |
| 10 | **LADY FORTUNA** | Casino heiress who bought a seat | "The house always bonks." | Boomerang Antenna | `Luck(0.30)`: **+0.30 Luck**, plus a mechanic | "+30% Luck; level-up refreshes are always free" | `characters.rs:178-189` |
| 11 | **AURORA PRIME** | Priestess of the dead sun | "Blesses the void with static." | Static Cling | `Size(0.20)`: **+0.20 Size**, plus a mechanic | "+20% Size; auras swell by +35% while she's sprinting" | `characters.rs:190-201` |
| 12 | **SGT. GRISTLE** | ESA marine, one lung, no chill | "Solves crowding with a shockwave." | Sonic Whoopee | `DamageMult(0.15)`: **+0.15 Damage**, plus a mechanic | "+15% Damage; +40% more while below half HP" | `characters.rs:202-213` |

Every flat passive is folded into the stat sheet by `recompute_stats` (`src/run.rs:246-259`). `SlideFrenzy` adds nothing to the sheet (`src/run.rs:253`); it works through `attack_speed()` (§3.2).

### 3.2 Mechanic passives (implemented outside the stat sheet)

| Hero | Mechanic | Exact rule | Source |
|---|---|---|---|
| **B0-NK** | Crit grows with level | Crit Chance gets `+0.005 × level`. It is recomputed when each level-up panel opens: +5% at level 10, +10% at level 20. | `src/run.rs:250`; `src/director.rs:138-140` |
| **Yuki** | Slide Frenzy | Starting a slide sets `frenzy_timer = 3.0` s. While it is above 0, AS gets **+0.30** (additive). | `src/player.rs:495-497`; `src/run.rs:280-284`; decay at `src/player.rs:538` |
| **Dr. Reticle** | Focus pulse | `reticle_timer` cycles 0 → 1.5 s. While it is **below 0.35 s**, crit chance gets **+1.0**. That is a guaranteed crit for 0.35 s of every 1.5 s, about 23% of the time. Over 100% crit rolls again, so base crit above 0 can double-crit during the pulse (§7.3). | `src/run.rs:305-311`; timer at `src/player.rs:854` |
| **Slipstream Nova** | Sprint cooldowns | While `fast_move` is true, AS gets **+1.3** (additive). `fast_move` means tangent speed > `8.5 × move_speed_mult × 1.08`. *Derived:* normal grounded running is clamped to exactly 1.0× run speed (`src/player.rs:446-453`), so `fast_move` is only reached while **sliding** (1.65×) or **bunny-hopping/airborne** (up to the 2.1× hard cap). | `src/run.rs:285-288`; `src/player.rs:540` |
| **Old Ironclad** | Last-stand plating | Below **30% HP**, armor is **doubled** before the diminishing curve. Base armor is 0, so this needs Duct Tape. | `src/run.rs:323-329` |
| **Lady Fortuna** | Free refreshes | On level-up panels, Refresh is **free and unlimited** (`[R]EFRESH (FREE)`). She is also given +2 refresh charges (4 total), which are therefore never spent. | `src/ui/panels.rs:190-202`, `117-121`; `src/run.rs:229-232` |
| **Aurora Prime** | Swelling auras | While `fast_move` (same rule as Nova), Aura weapon radius ×**1.35**. This applies to the damage radius and to the visual. | `src/run.rs:313-320`; `src/combat.rs:245`, `260`, `851` |
| **Sgt. Gristle** | Cornered fury | Below **50% HP**, the damage multiplier is ×**1.4**. This multiplies on top of the +0.15 and every other bonus. | `src/run.rs:292-301` |

### 3.3 Unlock conditions: text vs actual gate

The card on the character-select screen shows `unlock_desc` for locked heroes (`src/ui/menus.rs:458-462`). The actual gate is `MetaSave.unlocked_chars` (`src/ui/menus.rs:428`).

| Hero | `unlock_desc` text | What actually unlocks it | Source |
|---|---|---|---|
| Buzz | "Available from launch" | Unlocked by default | `characters.rs:239-244` |
| Valentina | "Available from launch" | Unlocked by default | same |
| B0-NK | "Clear MOON Tier 1" | Quest **ClearMoonT1** (+50 silver) | `src/content/quests.rs:78` |
| Yuki | "Clear MARS Tier 1" | Quest **ClearMarsT1** (+120 silver). Also completed by clearing Moon T2 or T3, because those runs mark (Mars, 1) as cleared (`src/director.rs:170-174`). | `src/content/quests.rs:81` |
| Chimp-O | "Find and free the cage on the MOON" | Quest **FreeChimp** (+40 silver): open the Cage (§16.9) | `src/content/quests.rs:76` |
| Doug | "Bonk 2,500 invaders (lifetime)" | Quest **Kill2500** (+90 silver) | `src/content/quests.rs:68` |
| Dr. Reticle | "Land 500 crits in one run" | **Unlocked by default.** The condition is not implemented. | `characters.rs:237-244` |
| Slipstream Nova | "Circle a planet 3x in under 40s" | **Unlocked by default.** Not implemented. | same |
| Old Ironclad | "Survive THE STATIC for 5 minutes" | **Unlocked by default.** Not implemented. | same |
| Lady Fortuna | "Open 20 Legendary chests (lifetime)" | **Unlocked by default.** Not implemented. | same |
| Aurora Prime | "Kill 1,000 enemies with an aura weapon" | **Unlocked by default.** Not implemented. | same |
| Sgt. Gristle | "Kill 3 bosses without dodging" | **Unlocked by default.** Not implemented. | same |

`MetaSave::migrate` re-inserts every default unlock on every load (`src/save.rs:130-140`). So once the recruits ship unlocked, existing saves cannot be re-locked without an explicit removal migration.

### 3.4 Astronaut disagreements with the GDD roster (`GDD.md:400-420`)

| Hero | GDD | Code |
|---|---|---|
| Sgt. Gristle | Wrench → Sledge Fist; <50% HP: +40% dmg **and +20% size**; unlock "as Buzz" | Sonic Whoopee; +15% dmg always; ×1.4 dmg below 50% HP; no size bonus |
| Dr. Reticle | Rail Needle (charge weapon); "first hit each second = guaranteed crit" | Laser Pistol; +10% crit; +100% crit for 0.35 s of every 1.5 s |
| Slipstream Nova | Cryo Vent laid as a **trail**; "no attack cooldown while above base speed" | Cryo Vent is an **aura**; AS +1.3 while above 1.08× run speed |
| Old Ironclad | "Immune to knockback; armor 2× below 30% HP" | Armor 2× below 30% HP and +90 Max HP. Nothing knocks the player back, so the immunity has nothing to act on. |
| Lady Fortuna | Boomerang → Loaded Dice; one free reroll **each level** | Boomerang (no Loaded Dice); **unlimited** free rerolls |
| Aurora Prime | Tesla Coil (aura); aura radius **scales with move speed**; unlock "1,000 Tesla kills" | Static Cling; +20% size; fixed ×1.35 aura while sprinting; unlock text "aura weapon" (not enforced) |
| The six built recruits (GDD #7 Gristle, #8 Reticle, #10 Nova, #11 Ironclad, #12 Fortuna, #14 Aurora) | Quest-gated unlocks | Start unlocked |
| The other nine GDD heroes (#9 Hexa Drone-Mother, #13 Glass Vesna, #15 Rot-9, #16 The Understudy, #17 Cascade Kaito, #18 Miss Gravity, #19 Comet Twins, #20 Warden Solongo, #21 NULL) | Canon design targets | Not built |

---

## 4. Weapons (16 base + 16 evolutions)

Defined in `src/content/weapons.rs:100-491`, fired by `weapon_fire` (`src/combat.rs:212-577`). An astronaut holds at most **`WEAPON_SLOTS` = 4** weapons (`src/config.rs:51`). Each weapon levels **1 → `MAX_WEAPON_LEVEL` = 7** (`src/config.rs:52`).

### 4.1 Per-level scaling (every weapon, evolutions included)

`WeaponKind::level_scaling(level)` (`src/content/weapons.rs:494-505`):

| Level | Damage × (`1 + 0.24·(L−1)`) | Bonus projectiles | Size × (`1 + 0.06·(L−1)`) |
|---|---|---|---|
| 1 | 1.00 | +0 | 1.00 |
| 2 | 1.24 | +0 | 1.06 |
| 3 | 1.48 | +1 | 1.12 |
| 4 | 1.72 | +1 | 1.18 |
| 5 | 1.96 | +2 | 1.24 |
| 6 | 2.20 | +2 | 1.30 |
| 7 | 2.44 | +3 | 1.36 |

**Evolving resets the weapon to level 1** (`src/run.rs:608-617`). An evolved weapon can then be levelled 1 → 7 again with **Epic**-rarity level-up cards (`src/run.rs:409-415`).

### 4.2 How the numbers combine (all behaviours)

For each weapon, every frame (`src/combat.rs:247-252`):

- **damage per hit** = `def.damage × level_dmg × damage_mult()`, where `damage_mult()` = `stats.damage`, ×2 with the Damage2x powerup, and ×1.4 for Gristle below half HP (`src/run.rs:292-302`).
- **Crit** is rolled per hit, per target (`roll_crit`, §7.3).
- **Elite multiplier:** × `stats.elite_damage` when the target is elite. Bosses count as elite.
- **count** = `def.projectiles + level_bonus + max(stats.projectiles, 0)`, minimum 1.
- **size** = `level_size × stats.size`.
- **Cooldown:** `cd -= dt × AS`; the weapon fires when `cd ≤ 0`, then `cd = def.cooldown` (`src/combat.rs:286-290`). The effective cooldown is `def.cooldown / AS`. AS has a floor of 0.1 (`src/run.rs:289`).
- **Auto-aim:** the nearest non-pot, non-buried enemy within **40 m** (straight-line). With no target, the weapon uses the astronaut's facing (`src/combat.rs:292-301`).
- **Origin:** projectiles start 0.4 m above the astronaut (`src/combat.rs:302`).

### 4.3 The nine behaviours

| Behaviour | Exact rule | Uses `count` | Uses AS | Uses `size` | Uses `proj_speed` / `duration` | Knockback | Source |
|---|---|---|---|---|---|---|---|
| **MeleeArc** `{arc_deg, range}` | One instant sweep. Hits **every** enemy (pots included, buried excluded) within straight-line `range × size`, whose tangent direction is within `arc_deg/2` of the aim. 360° = all around. | no | yes | range | no | `9 × knockback` along the hit direction | `combat.rs:305-337` |
| **Shot** `{speed, pierce, spread_deg}` | `count` straight projectiles fanned evenly across `spread_deg`. A single shot gets ±0.04 rad random jitter. Life `2.2 × duration` s. Each can hit `pierce + 1` enemies. The same target cannot be re-hit for 0.5 s. | yes | yes | hit radius, visual | speed × `proj_speed`, life × `duration` | `4 × knockback` along the heading | `combat.rs:338-365`, `668-700` |
| **Seek** `{speed, pierce}` | `count` projectiles, headings 0.35 rad apart. They home on the nearest non-pot enemy within 14 m (turn rate 10). Life `2.6 × duration` s. Pierce as for Shot. | yes | yes | hit radius, visual ×0.9 | yes | `4 × knockback` | `combat.rs:366-389`, `606-627` |
| **Boomerang** `{speed, range}` | `count` projectiles, headings 0.5 rad apart. They fly out for `range/speed` s (base speed), then steer back to the owner (turn rate 8) and vanish within 1.2 m. Life `(range/speed) × 2.4 × duration`. Pierce 999, so it hits everything on both legs (0.5 s re-hit cooldown per target). | yes | yes | hit radius, visual | yes | `4 × knockback` | `combat.rs:390-414`, `628-641` |
| **Beam** `{range, width}` | A beam fixed along the aim heading at fire time, anchored 0.6 m above the (moving) astronaut. It damages every non-buried enemy (pots included) in the corridor (`0 ≤ along ≤ range`, perpendicular < `width + scale·0.5`) **every 0.12 s**. Tick count = `floor(5 × duration) + 1`, so **6 ticks** at base duration: the first on the frame after firing, the last about 0.6 s later, and the beam despawns at about 0.72 s. | no | cooldown only (tick rate fixed) | range and width | ticks × `duration` | none | `combat.rs:415-431`, `790-838` |
| **Orbit** `{radius, deg_per_sec}` | `count` bodies orbit at `radius × size`, 0.6 m up, evenly spaced, at `deg_per_sec`. Each body hits every enemy within `0.8 + scale·0.5`, then waits **0.3 s** before it can hit again. Continuous, with no cooldown. | yes (body count) | **no** | orbit radius | no | `5` outward (fixed) | `combat.rs:255-258`, `505-554`, `738-787` |
| **Chain** `{jumps, range, link_range}` | Instant. The first target is the nearest non-pot enemy within `range` of the astronaut. Each next target is the nearest unchained enemy within `link_range` of the previous one. Up to **`jumps + count`** targets, each hit for full damage. | yes (adds targets) | yes | no | no | none | `combat.rs:432-475` |
| **Rocket** `{speed, aoe}` | `count` rockets, each with a random ±0.6 rad heading offset. They home within 14 m (turn rate 6). Life `4 × duration`. A rocket **explodes on first contact, or when its life ends**. The explosion hits every enemy (pots included) within `aoe × size + scale·0.5` for full damage, with crit rolled per target. | yes | yes | AoE radius, visual ×1.3 | yes | `7` radial (fixed) | `combat.rs:476-498`, `645-651`, `708-735` |
| **Aura** `{radius, slow}` | Every `cooldown / AS` s, hits **every** enemy (pots included, buried excluded) within `radius × aura_scale × size`. `aura_scale` is 1.35 for Aurora while sprinting, otherwise 1.0. | no | yes | radius | no | none | `combat.rs:259-282` |

**Homing is approximate:** Seek and Rocket steer toward the nearest non-pot enemy returned by `SpatialHash::near(pos, 14.0)` (`src/combat.rs:610`). That query returns whole 2.2 m cells in a cube of `2·⌈14/2.2⌉ + 1 = 15` cells per side, with no exact 14 m test, so a target up to about 17 m away along an axis (more toward the cube's corners) can be picked (`src/enemies.rs:190-204`). The Comet has the same property (§20).

**Cryo slow (all weapons):** if the shooter owns **Cryo Vent or Absolute Zero**, *every* hit from that shooter, with any weapon, adds **+0.25 slow**, capped at **0.65** (`src/combat.rs:894-902`, `930-932`). An enemy moves at `speed × (1 − slow)`, and slow decays by 0.35 per second (`src/enemies.rs:1054`, `1058`). The per-weapon `slow` values in the table (0.45 and 0.75) are **never read** (`src/combat.rs:277`).

**The aura visual** is drawn at `radius × aura_scale` without the size factor (`src/combat.rs:851`), so it understates the damage radius once size exceeds 1.

### 4.4 Base weapons (16)

"Unlocked in pool" means the weapon can be offered as a **NEW** card on level-up (§8.3). Starting weapons are always given to their hero regardless.

| Weapon | Behaviour | Parameters | Base dmg | Cooldown (s) | Projectiles | Evolves into | Catalyst item | Unlocked in pool | Source |
|---|---|---|---|---|---|---|---|---|---|
| **Wrench** — "Bonks everything in a wide arc" | MeleeArc | arc 150°, range 3.4 | 14 | 1.05 | 1 | MEGA WRENCH | Protein Paste | default | `weapons.rs:104-115` |
| **Laser Pistol** — "Zaps the nearest invader" | Shot | speed 30, pierce 0, spread 5° | 10 | 0.75 | 1 | Gatling Laser | Overclocked CPU | default | `weapons.rs:128-139` |
| **Rivet Gun** — "Sprays hot rivets in a cone" | Shot | speed 26, pierce 0, spread 26° | 7 | 1.1 | 4 | RIVETER 9000 | Laser Sight | default | `weapons.rs:152-163` |
| **Kunai** — "Smart blades hunt the closest target" | Seek | speed 24, pierce 0 | 12 | 0.9 | 1 | BLADE STORM | Caffeine IV | default | `weapons.rs:176-187` |
| **Boomerang Antenna** — "Comes back. Usually." | Boomerang | speed 22, range 14 | 15 | 1.5 | 1 | SATELLITE ARRAY | Golden Antenna | default | `weapons.rs:200-211` |
| **Mining Laser** — "Cuts a line through the swarm" | Beam | range 16, width 0.9 | 8 per tick | 2.4 | 1 | DEATH RAY | Heavy Payload | default | `weapons.rs:224-235` |
| **Orbital Drones** — "Little buddies on patrol" | Orbit | radius 3.4, 190°/s | 11 | 0 (continuous) | 2 | DRONE SWARM | Splitter Chip | quest **Pots50** | `weapons.rs:248-259` |
| **Tesla Coil** — "Lightning that likes company" | Chain | 3 jumps, range 12, link 5 | 13 | 1.3 | 1 | STORM CORE | Extra Battery | quest **Kill1000** | `weapons.rs:272-283` |
| **Rocket Pod** — "Homing hugs that explode" | Rocket | speed 14, AoE 3.6 | 26 | 2.1 | 1 | MIRV POD | Cursed Moon Rock | quest **Chests10** (blocked by bug H2); Ironclad's starter | `weapons.rs:296-307` |
| **Cryo Vent** — "A personal winter" | Aura | radius 4.2, slow 0.45 (unused) | 5 per tick | 0.5 | 1 | ABSOLUTE ZERO | Fish Bowl Helmet | quest **Shrines5**; Nova's starter | `weapons.rs:320-331` |
| **Meatball Comet** — "Frozen mortar meatball, splatters on impact" | Rocket | speed 13, AoE 3.4 | 22 | 1.6 | 1 | RAGÙ RAIN | Fish Bowl Helmet | default | `weapons.rs:346-357` |
| **Static Cling** — "A field of charge that shreds what hugs you" | Aura | radius 3.8, slow 0 | 6 per tick | 0.5 | 1 | FULL DISCHARGE | Thorn Plating | default | `weapons.rs:370-381` |
| **Ricochet Disc** — "Skips enemy to enemy; comes home" | Chain | 4 jumps, range 13, link 5.5 | 13 | 1.25 | 1 | THE OMNIDISC | Lucky Meteorite | default | `weapons.rs:394-405` |
| **Sonic Whoopee** — "A brown-note pulse that shoves the horde back" | MeleeArc | arc 210°, range 4.0 | 13 | 1.15 | 1 | THE BROWN NOTE | Rocket Boots | default | `weapons.rs:418-429` |
| **Cosmonaut's Bell** — "Tolls for the ones the vacuum kept" | Aura | radius 4.6, slow 0 | 9 per tick | 0.9 | 1 | THE ANGELUS | Star Chart | default | `weapons.rs:442-453` |
| **Yo-Yo of Damocles** — "Spiked yo-yo on a plasma cord, orbits close" | Orbit | radius 2.9, 240°/s | 12 | 0 (continuous) | 2 | SWORD-YO | Heavy Payload | default | `weapons.rs:466-477` |

The default pool is the 12 weapons listed in `MetaSave::default` (`src/save.rs:65-81`). The quest unlocks are in `src/content/quests.rs:67`, `70-72`.

### 4.5 Evolutions (16)

An evolution is offered when the base weapon is at **level ≥ 7** and the astronaut owns **≥ 1 stack** of the catalyst item (`src/run.rs:371-384`). It is always a **Legendary** card (`src/run.rs:407`). At most **2** evolution cards are placed at the front of a single roll (`src/run.rs:514-519`). There is **no per-run evolution cap**.

| Evolution | Evolved from | Behaviour | Parameters | Base dmg | Cooldown (s) | Projectiles | Description | Source |
|---|---|---|---|---|---|---|---|---|
| **MEGA WRENCH** | Wrench + Protein Paste | MeleeArc | **360°**, range 4.6 | 46 | 0.95 | 1 | "The bonk heard around the planet" | `weapons.rs:116-127` |
| **Gatling Laser** (the only evolution not named in capitals) | Laser Pistol + Overclocked CPU | Shot | speed 38, pierce 1, spread 9° | 9 | **0.11** | 1 | "The trigger is taped down" | `weapons.rs:140-151` |
| **RIVETER 9000** | Rivet Gun + Laser Sight | Shot | speed 30, pierce 1, spread 40° | 11 | 0.8 | 8 | "OSHA has left the solar system" | `weapons.rs:164-175` |
| **BLADE STORM** | Kunai + Caffeine IV | Seek | speed 32, pierce 1 | 16 | 0.35 | 3 | "A weather system of knives" | `weapons.rs:188-199` |
| **SATELLITE ARRAY** | Boomerang + Golden Antenna | Boomerang | speed 28, range 20 | 24 | 1.1 | 3 | "Full-orbit broadcast of pain" | `weapons.rs:212-223` |
| **DEATH RAY** | Mining Laser + Heavy Payload | Beam | range 26, width 2.2 | 18 per tick | 1.8 | 1 | "Strip-mines the horizon" | `weapons.rs:236-247` |
| **DRONE SWARM** | Orbital Drones + Splitter Chip | Orbit | radius 4.4, 260°/s | 16 | 0 | 6 | "The airspace is closed" | `weapons.rs:260-271` |
| **STORM CORE** | Tesla Coil + Extra Battery | Chain | 6 jumps, range 15, link 7 | 22 | 0.9 | 2 | "You are the weather now" | `weapons.rs:284-295` |
| **MIRV POD** | Rocket Pod + Cursed Moon Rock | Rocket | speed 17, AoE 4.6 | 30 | 1.7 | 3 | "One rocket, many opinions" | `weapons.rs:308-319` |
| **ABSOLUTE ZERO** | Cryo Vent + Fish Bowl Helmet | Aura | radius 6.4, slow 0.75 (unused) | 12 per tick | 0.45 | 1 | "Physics files a complaint" | `weapons.rs:332-343` |
| **RAGÙ RAIN** | Meatball Comet + Fish Bowl Helmet | Rocket | speed 16, AoE 4.6 | 30 | 1.2 | 3 | "Nonna's recipe. Survived reentry. Barely." | `weapons.rs:358-369` |
| **FULL DISCHARGE** | Static Cling + Thorn Plating | Aura | radius 5.6 | 13 per tick | 0.45 | 1 | "Ghost-cosmonauts hate this one weird trick" | `weapons.rs:382-393` |
| **THE OMNIDISC** | Ricochet Disc + Lucky Meteorite | Chain | 9 jumps, range 16, link 7 | 18 | 0.95 | 2 | "Home is wherever you're standing" | `weapons.rs:406-417` |
| **THE BROWN NOTE** | Sonic Whoopee + Rocket Boots | MeleeArc | **360°**, range 5.2 | 26 | 0.95 | 1 | "Discovered by accident. Weaponized on purpose." | `weapons.rs:430-441` |
| **THE ANGELUS** | Cosmonaut's Bell + Star Chart | Aura | radius 6.2 | 17 per tick | 0.8 | 1 | "Every toll is a mercy and a threat" | `weapons.rs:454-465` |
| **SWORD-YO** | Yo-Yo + Heavy Payload | Orbit | radius 4.6, 300°/s | 19 | 0 | 5 | "Down. Up. Existential. Down again." | `weapons.rs:478-489` |

Two items are catalysts for two weapons each: **Heavy Payload** (Mining Laser and Yo-Yo) and **Fish Bowl Helmet** (Cryo Vent and Meatball Comet). `WeaponKind::catalyst_for` lists them on item cards as "Evo catalyst: …" (`src/content/weapons.rs:512-517`; `src/run.rs:499-507`).

A base weapon is never offered while you own it **or its evolution** (`weapon_slot_free`, `src/run.rs:590-594`).

### 4.6 Derived level-1 vs level-7 numbers (base stats, AS 1, size 1, no items)

*Derived* from §4.1-4.5. Reach = speed × life. For Chain weapons the "count" column is the number of targets. The rate column is `1 / cooldown`. Because a weapon that fires resets `cd` to the full cooldown and discards the overshoot (`src/combat.rs:290`), the real rate is slightly lower at a finite frame rate; the effect is largest for the Gatling Laser, which fires every 7th frame at 60 fps (8.57 volleys/s, not 9.09).

| Weapon | Dmg L1 → L7 | Count L1 → L7 | Geometry L1 → L7 | Other |
|---|---|---|---|---|
| Wrench | 14 → 34.16 | (1 sweep) | range 3.40 → 4.62 | 0.95 sweeps/s |
| MEGA WRENCH | 46 → 112.24 | (1 sweep) | range 4.60 → 6.26 | 1.05 sweeps/s |
| Laser Pistol | 10 → 24.40 | 1 → 4 shots | reach 66 m | 1.33 volleys/s |
| Gatling Laser | 9 → 21.96 | 1 → 4 | reach 83.6 m, 2 hits each | 9.09 volleys/s |
| Rivet Gun | 7 → 17.08 | 4 → 7 | reach 57.2 m | 0.91 volleys/s |
| RIVETER 9000 | 11 → 26.84 | 8 → 11 | reach 66 m, 2 hits each | 1.25 volleys/s |
| Kunai | 12 → 29.28 | 1 → 4 | reach 62.4 m | 1.11 volleys/s |
| BLADE STORM | 16 → 39.04 | 3 → 6 | reach 83.2 m, 2 hits each | 2.86 volleys/s |
| Boomerang Antenna | 15 → 36.60 | 1 → 4 | out 0.636 s, life 1.527 s | 0.67 throws/s |
| SATELLITE ARRAY | 24 → 58.56 | 3 → 6 | out 0.714 s, life 1.714 s | 0.91 throws/s |
| Mining Laser | 8 → 19.52 per tick | 1 beam | 16 × 0.9 → 21.76 × 1.224 | 6 ticks per beam |
| DEATH RAY | 18 → 43.92 per tick | 1 beam | 26 × 2.2 → 35.36 × 2.99 | 6 ticks per beam |
| Orbital Drones | 11 → 26.84 | 2 → 5 bodies | radius 3.40 → 4.62 | 0.53 rev/s |
| DRONE SWARM | 16 → 39.04 | 6 → 9 | radius 4.40 → 5.98 | 0.72 rev/s |
| Yo-Yo | 12 → 29.28 | 2 → 5 | radius 2.90 → 3.94 | 0.67 rev/s |
| SWORD-YO | 19 → 46.36 | 5 → 8 | radius 4.60 → 6.26 | 0.83 rev/s |
| Tesla Coil | 13 → 31.72 | 4 → 7 targets | first 12 m, links 5 m | 0.77 zaps/s |
| STORM CORE | 22 → 53.68 | 8 → 11 targets | first 15 m, links 7 m | 1.11 zaps/s |
| Ricochet Disc | 13 → 31.72 | 5 → 8 targets | first 13 m, links 5.5 m | 0.80 zaps/s |
| THE OMNIDISC | 18 → 43.92 | 11 → 14 targets | first 16 m, links 7 m | 1.05 zaps/s |
| Rocket Pod | 26 → 63.44 | 1 → 4 | AoE 3.60 → 4.90, reach 56 m | 0.48 volleys/s |
| MIRV POD | 30 → 73.20 | 3 → 6 | AoE 4.60 → 6.26, reach 68 m | 0.59 volleys/s |
| Meatball Comet | 22 → 53.68 | 1 → 4 | AoE 3.40 → 4.62, reach 52 m | 0.63 volleys/s |
| RAGÙ RAIN | 30 → 73.20 | 3 → 6 | AoE 4.60 → 6.26, reach 64 m | 0.83 volleys/s |
| Cryo Vent | 5 → 12.20 | — | radius 4.20 → 5.71 | 2.00 ticks/s |
| ABSOLUTE ZERO | 12 → 29.28 | — | radius 6.40 → 8.70 | 2.22 ticks/s |
| Static Cling | 6 → 14.64 | — | radius 3.80 → 5.17 | 2.00 ticks/s |
| FULL DISCHARGE | 13 → 31.72 | — | radius 5.60 → 7.62 | 2.22 ticks/s |
| Cosmonaut's Bell | 9 → 21.96 | — | radius 4.60 → 6.26 | 1.11 ticks/s |
| THE ANGELUS | 17 → 41.48 | — | radius 6.20 → 8.43 | 1.25 ticks/s |
| Sonic Whoopee | 13 → 31.72 | (1 sweep) | range 4.00 → 5.44, 210° | 0.87 sweeps/s |
| THE BROWN NOTE | 26 → 63.44 | (1 sweep) | range 5.20 → 7.07, 360° | 1.05 sweeps/s |

The level-bonus projectiles do nothing for MeleeArc, Beam and Aura weapons. `count` is computed but unused by those behaviours.

### 4.7 Weapon disagreements with the GDD (`GDD.md:444-495`)

| Topic | GDD | Code |
|---|---|---|
| Rivet Gun catalyst | Splitter Chip | **Laser Sight** |
| Orbital Drones catalyst | Extra Battery | **Splitter Chip** |
| Tesla Coil catalyst | Laser Sight | **Extra Battery** |
| Rocket Pod catalyst | Splitter Chip | **Cursed Moon Rock** |
| Cryo Vent catalyst | Duct Tape | **Fish Bowl Helmet** |
| Evolution cap | 1 per run (+1 with Tome of Ascension) (`GDD.md:451-454`, `1220`) | No cap; up to 2 evolution cards per roll |
| Meatball Comet / Ragù Rain | Lobbed mortar over the horizon; Ragù splits into 3 on the way down | Homing **Rocket** behaviour; Ragù fires 3 rockets, with no split |
| Static Cling → Full Discharge | Evolution is a "periodic screen-clearing nova" | Bigger, faster aura |
| Ricochet Disc / Omnidisc | Travelling disc that laps the world; Omnidisc bounces infinitely | Instant **Chain** zap (5 targets; Omnidisc 11) |
| Sonic Whoopee / Brown Note | Cone knockback + **stun**; lethal repulsor ring | MeleeArc with the standard 9× knockback; no stun |
| Yo-Yo / Sword-Yo | Scales with an unhit-move combo; the cord extends at max combo | Plain Orbit weapons |
| Cosmonaut's Bell / Angelus | Tolls every 4 s, marks foes for +crit; Angelus resurrects enemies as friendly wisps | Plain Aura ticking every 0.9 s (0.8 s) |
| Not built | Duct-Tape Turret, Solar Flare Lens, Gravity Grenade, Seed Pod Launcher, Flag of Earth (and their evolutions) | — |

---

## 5. Items (22)

Defined in `src/content/items.rs:66-248`.
- Every item is a **pure stat boost**. Each stack adds its `boosts` once more (`src/run.rs:260-265`).
- Items never level. There are no proc, conditional or cursed-behaviour items.
- A stack is `(ItemKind, count)` on the astronaut's `PlayerState`.

### 5.1 Item table

| Item | Rarity | Description | Effect per stack | Max stacks | Total at max | Evolution catalyst for | Source |
|---|---|---|---|---|---|---|---|
| **Space Borgar** | Common | "Zero-g grease. Sticks to your ribs" | +20 Max HP | 9 | +180 Max HP | — | `items.rs:71-78` |
| **Moon Cheese** | Common | "Aged 4.5 billion years" | +12 Regen (HP per minute) | 9 | +108 HP/min (1.8 HP/s) | — | `items.rs:79-86` |
| **Duct Tape** | Common | "Suit patch, armor plating, life philosophy" | +8 Armor | 9 | +72 Armor → 41.9% reduction | — | `items.rs:87-94` |
| **Slippery Visor** | Rare | "They literally cannot hit you" | +8 Evasion | 6 | +48 Evasion → 32.4% dodge | — | `items.rs:95-102` |
| **Protein Paste** | Common | "Tube gains" | +0.10 Damage | 9 | +90% Damage | Wrench | `items.rs:103-110` |
| **Overclocked CPU** | Rare | "Suit firmware set to UNSAFE" | +0.12 Attack Speed | 6 | +72% AS | Laser Pistol | `items.rs:111-118` |
| **Fish Bowl Helmet** | Rare | "Bigger bubble, bigger booms" | +0.12 Size | 6 | +72% Size | Cryo Vent, Meatball Comet | `items.rs:119-126` |
| **Rocket Boots** | Common | "Walking is for Earth" | +0.08 Move Speed | 6 | +48% Move | Sonic Whoopee | `items.rs:127-134` |
| **Trampoline Soles** | Common | "Boing certified" | +0.15 Jump Height | 4 | +60% jump height | — | `items.rs:135-142` |
| **Magnet Boots** | Common | "Loot learns to love you" | +0.25 Pickup Range | 6 | +150% (3.2 m → 8.0 m) | — | `items.rs:143-150` |
| **Lucky Meteorite** | Epic | "Statistically improbable rock" | +0.12 Luck | 6 | +0.72 Luck | Ricochet Disc | `items.rs:151-158` |
| **Space Credit Card** | Rare | "Interplanetary cashback" | +0.10 Chest Discount | 4 | −40% chest/shop prices | — | `items.rs:159-166` |
| **Golden Antenna** | Rare | "Tuned to the money frequency" | +0.15 Gold Gain | 6 | +90% gold | Boomerang Antenna | `items.rs:167-174` |
| **Star Chart** | Rare | "Knowledge is XP" | +0.10 XP Gain | 6 | +60% XP | Cosmonaut's Bell | `items.rs:175-182` |
| **Laser Sight** | Rare | "Red dot of destiny" | +0.07 Crit Chance | 6 | +42% crit | Rivet Gun | `items.rs:183-190` |
| **Heavy Payload** | Epic | "Crits with extra gravity" | +0.5 Crit Damage | 5 | +2.5 → crit ×4.5 | Mining Laser, Yo-Yo | `items.rs:191-198` |
| **Extra Battery** | Rare | "Everything lasts longer" | +0.15 Duration | 5 | +75% Duration | Tesla Coil | `items.rs:199-206` |
| **Splitter Chip** | **Legendary** | "One shot becomes friends" | +1 Projectile | 2 | +2 projectiles | Orbital Drones | `items.rs:207-214` |
| **Cursed Moon Rock** | Epic | "Do NOT lick. More danger, more loot" | +0.15 Difficulty **and** +0.10 Luck | 5 | +0.75 D, +0.50 Luck | Rocket Pod | `items.rs:215-222` |
| **Thorn Plating** | Rare | "Hug at your own risk" | +12 Thorns | 6 | 72 reflected damage per hit | Static Cling | `items.rs:223-230` |
| **Vampire Visor** | Epic | "Sunproof. Ironically" | +0.05 Lifesteal | 4 | 20% chance to heal 1 HP per hit | — | `items.rs:231-238` |
| **Caffeine IV** | Common | "Projectiles share your jitters" | +0.15 Projectile Speed | 6 | +90% projectile speed | Kunai | `items.rs:239-246` |

Rarity counts: **8 Common, 9 Rare, 4 Epic, 1 Legendary.**

### 5.2 Stacking rules

- **Level-up cards** never offer an item at its max stacks (`src/run.rs:542-545`). The first copy is titled `NEW: <item>`; a later copy is titled with the plain item name, and its card body reads `<description> (n/max)`, where `n` is the count already owned (`src/run.rs:428-429`, `474-492`).
- **Chests and the Shady Guy ignore max stacks when you take or buy.** Their *roll* respects the cap (`src/interact.rs:98`), but a chest item that was rolled earlier, or duplicate shop stock, can still push an item past its max (`src/ui/panels.rs:345-349`, `465-469`).
- **The Microwave** only offers items below max stacks (`src/interact.rs:520-525`).
- **Banishing an item** on a level-up card adds it to `banned_items` for the rest of the run. Banned items leave the level-up pool, chest rolls, shop rolls, shrine rolls and Moai rolls, except for the "any item" fallback in `roll_item` (`src/interact.rs:104-110`).
- **Cursed Moon Rock** is the only item with two boosts. Its Difficulty raises spawn rate and enemy HP and damage for everyone (§11); its Luck improves rarity rolls (§9).

### 5.3 Item disagreements with the GDD (`GDD.md:525-586`)

- **Built:** the GDD's "22 built items" list matches the code one-for-one.
- **Stack caps:** the GDD says "Most items stack additively and near-infinitely"; the code caps every item at 2–9 stacks.
- **Trampoline Soles:** the GDD's movement table lists it as "+1" extra jump (`GDD.md:319`); in the code it gives **+0.15 Jump Height** per stack and no extra jump.
- **Not built:** all "do-things" items (Orbital Yo-Yo, Comet Tail, The Overheat, …, Antipode Blink), the cursed items, and the death-save stacking rule.

---

## 6. Tomes (8)

Tomes are the permanent, pre-run meta upgrades, bought with **silver** in the main menu. Defined in `src/content/tomes.rs:37-50`.

| Tome | Description | Stat | Per level | Max level | Total at max | Source |
|---|---|---|---|---|---|---|
| **Tome of Damage** | "Hit harder, forever" | Damage | +0.02 | 20 | +40% Damage | `tomes.rs:41` |
| **Tome of Health** | "Thicker suit lining" | Max HP | +6 | 20 | +120 Max HP | `tomes.rs:42` |
| **Tome of Agility** | "Lower gravity legs" | Move Speed | +0.012 | 20 | +24% Move | `tomes.rs:43` |
| **Tome of Cooldown** | "Weapons on espresso" | Attack Speed | +0.012 | 20 | +24% AS | `tomes.rs:44` |
| **Tome of Precision** | "Aim like you mean it" | Crit Chance | +0.006 | 20 | +12% Crit | `tomes.rs:45` |
| **Golden Tome** | "Coins find you cuter" | Gold Gain | +0.02 | 20 | +40% Gold | `tomes.rs:46` |
| **Tome of XP** | "Learn from the bonk" | XP Gain | +0.015 | 20 | +30% XP | `tomes.rs:47` |
| **Cursed Tome** | "More danger. More everything" | Difficulty | +0.02 | 20 | +40% Difficulty | `tomes.rs:48` |

### 6.1 Loadout and slots

- **Only equipped tomes apply.** `recompute_stats` iterates `save.tome_loadout` (`src/run.rs:238-245`). Levels bought for an unequipped tome do nothing.
- **Default loadout:** `[Damage, Health, Xp]`, with **3 slots** (`src/save.rs:88-89`).
- **Toggling:** clicking a tome name in the TOMES tab equips it (if a slot is free) or unequips it (`src/ui/menus.rs:331-342`).
- **Slots:** each `TomeSlot` quest reward adds 1 slot, up to a maximum of **5** (`src/save.rs:220`). Two quests grant one: Kill10000 and SurviveStatic2Min.
- **Cursed Tome** is the only tome that raises world difficulty. Its +Difficulty has no luck attached; nothing converts Difficulty into loot except the Cursed Moon Rock's own Luck and the Greed Shrine's Luck.

### 6.2 Cost per level

Cost in silver to go from level `c` to `c+1` is `round(8 · l · √l)` with `l = c + 1` (`src/content/tomes.rs:52-56`). Purchase is at `src/ui/menus.rs:318-330`. All 8 tomes use the same curve.

| Buy level | Cost | Cumulative | Buy level | Cost | Cumulative |
|---|---|---|---|---|---|
| 1 | 8 | 8 | 11 | 292 | 1,434 |
| 2 | 23 | 31 | 12 | 333 | 1,767 |
| 3 | 42 | 73 | 13 | 375 | 2,142 |
| 4 | 64 | 137 | 14 | 419 | 2,561 |
| 5 | 89 | 226 | 15 | 465 | 3,026 |
| 6 | 118 | 344 | 16 | 512 | 3,538 |
| 7 | 148 | 492 | 17 | 561 | 4,099 |
| 8 | 181 | 673 | 18 | 611 | 4,710 |
| 9 | 216 | 889 | 19 | 663 | 5,373 |
| 10 | 253 | 1,142 | 20 | 716 | **6,089** |

Maxing all 8 tomes costs **48,712 silver**.

**GDD** (`GDD.md:588-612`):
- **Levels and cost:** the GDD says "Each tome levels 10 ranks, cost `100 × 1.6^level`". The code has **20** ranks at `8·l^1.5`.
- **Loadout:** the GDD says "Loadout is limited to 4". The code has **3** slots, growing to 5.
- **Not built:** the 15 new tomes (Orbit, Encirclement, Ascension, and the rest).

---

## 7. The stat sheet (27 stats)

`StatKind` and `Stats` are in `src/stats.rs:7-100`. `recompute_stats` (`src/run.rs:236-275`) rebuilds the sheet from scratch, in this order:

1. `Stats::default()`.
2. Equipped tomes × level.
3. The hero's flat passive.
4. Items × stacks.
5. Greed shrines: `+0.12 Difficulty` and `+0.08 Luck` per `greed_stacks` (`src/run.rs:267-268`).
6. **+1 Max HP per level above 1** (`src/run.rs:270`).

Current HP keeps its **fraction** of max HP across a recompute (`src/run.rs:272-274`). Recompute runs when a level-up panel opens, when any upgrade is applied, and when a chest, shop or greed shrine is used.

`Stats::apply` adds values plainly, with two exceptions: `XpGain` is capped at a total of **10.0** and `ChestDiscount` at **0.6** (`src/stats.rs:103-134`).

### 7.1 All 27 stats

| # | Stat | Default | How it is applied | Granted by | Source of the effect |
|---|---|---|---|---|---|
| 1 | **MaxHp** | 100 | HP cap. Food heals 20% of it. | Space Borgar, Tome of Health, Ironclad, +1 per level | `src/run.rs:270`; `src/pickups.rs:270` |
| 2 | **Regen** | 0 | HP per **minute**: `hp += regen/60 × dt` while alive | Moon Cheese | `src/player.rs:855-858` |
| 3 | **Shield** | 0 | Absorbs damage after armor. After any hit, recharge waits 5 s (`shield_cd`), then refills at 35% of max per second. | **Nothing grants it** | `src/combat.rs:999-1004`; `src/player.rs:860-863` |
| 4 | **Armor** | 0 | Damage × `(1 − a/(a+100))`. Ironclad doubles `a` below 30% HP. | Duct Tape | `src/stats.rs:137-140`; `src/run.rs:323-329`; `src/combat.rs:997` |
| 5 | **Evasion** | 0 | Chance `e/(e+100)` to take no damage (shows "Dodge") | Slippery Visor | `src/stats.rs:141-144`; `src/combat.rs:993-996` |
| 6 | **Lifesteal** | 0 | On each landed hit, chance `min(lifesteal, 1)` to heal the **shooter** by exactly 1 HP | Vampire Visor | `src/combat.rs:941-946` |
| 7 | **Thorns** | 0 | When hit by an attacker entity (contact, Burrower eruption, Anubot beam), deal `thorns` flat damage back. Projectiles, telegraphs and worm segments have no attacker. | Thorn Plating | `src/combat.rs:1012-1017` |
| 8 | **Damage** | 1.0 | Multiplies all weapon damage. ×2 with the Damage2x powerup, ×1.4 for Gristle below 50% HP. | Protein Paste, Tome of Damage, Buzz, Gristle | `src/run.rs:292-302`; `src/combat.rs:250` |
| 9 | **CritChance** | 0.05 | Per-hit crit roll. Values over 1.0 roll again (overcrit). Reticle gets +1.0 during his pulse. | Laser Sight, Tome of Precision, Reticle, B0-NK | `src/combat.rs:196-208`; `src/run.rs:305-311` |
| 10 | **CritDamage** | 2.0 | Multiplier applied per successful crit roll | Heavy Payload | `src/combat.rs:202` |
| 11 | **AttackSpeed** | 1.0 | Cooldowns tick at `dt × AS`. Yuki's frenzy adds +0.30, Nova sprinting adds +1.3. Floor 0.1. | Overclocked CPU, Tome of Cooldown, Valentina | `src/run.rs:278-290`; `src/combat.rs:262`, `286` |
| 12 | **Projectiles** | 0 | Added to every weapon's `count` (§4.3) | Splitter Chip | `src/combat.rs:251` |
| 13 | **ProjSpeed** | 1.0 | Multiplies Shot, Seek, Boomerang and Rocket speed | Caffeine IV | `src/combat.rs:351`, `375`, `400`, `484` |
| 14 | **Size** | 1.0 | Multiplies melee range, aura radius, orbit radius, beam range and width, rocket AoE, and projectile hit radius and visuals | Fish Bowl Helmet, Aurora | `src/combat.rs:252` |
| 15 | **Duration** | 1.0 | Multiplies projectile life (Shot 2.2 s, Seek 2.6 s, Boomerang, Rocket 4 s) and beam tick count (`floor(5·dur)+1`) | Extra Battery | `src/combat.rs:354`, `378`, `403`, `423`, `487` |
| 16 | **EliteDamage** | 1.0 | Damage multiplier against `elite` enemies. Every boss and miniboss is elite. | **Nothing grants it** | `src/combat.rs:269`, `316`, `455`, `685`, `727`, `771`, `832` |
| 17 | **Knockback** | 1.0 | Multiplies melee (9), projectile (4) and comet (12) knockback. Rockets (7) and drones (5) are fixed. Bosses take 5%. | **Nothing grants it** | `src/combat.rs:322`, `691`; `src/comet.rs:105`; `src/combat.rs:928` |
| 18 | **MoveSpeed** | 1.0 | Multiplies run speed (8.5), the slide boost and the speed caps. ×1.5 with the Speed powerup. | Rocket Boots, Tome of Agility, Nova | `src/run.rs:331-337`; `src/player.rs:430-453` |
| 19 | **ExtraJumps** | 0 | Max jumps = `1 + extra_jumps` | Chimp-O | `src/player.rs:456` |
| 20 | **JumpHeight** | 1.0 | Jump velocity = `8.0 × √jump_height`, so apex height scales linearly | Trampoline Soles | `src/player.rs:478` |
| 21 | **Luck** | 0 | Rarity weights (§9). Chest, shop, shrine and Moai rolls clamp it to [0, 3]; the level-up pool clamps it to [0, 2]. | Lucky Meteorite, Cursed Moon Rock, Fortuna, Greed Shrine | `src/content/mod.rs:38-46`; `src/run.rs:579-587` |
| 22 | **Difficulty** | 0 | The world uses `D = max over astronauts`. It scales spawn rate `×(1+D)`, enemy HP `×(1+D)`, enemy damage `×(1+D/2)`, boss HP `×(1+D)` and boss damage `×(1+D/2)`. | Cursed Moon Rock, Cursed Tome, Greed Shrine | `src/player.rs:852`; `src/enemies.rs:575`, `645`, `656`; `src/content/enemies.rs:238-243` |
| 23 | **PickupRange** | 1.0 | Attraction radius = `3.2 × pickup_range` (×40 with the Magnet powerup) | Magnet Boots | `src/run.rs:339-346` |
| 24 | **XpGain** | 1.0 (cap 10) | Every XP grant × `xp_gain` | Star Chart, Tome of XP | `src/run.rs:348-349` |
| 25 | **GoldGain** | 1.0 | Each gold pickup × `gold_gain`, rounded | Golden Antenna, Golden Tome, Doug | `src/pickups.rs:254-257` |
| 26 | **SilverGain** | 1.0 | Each silver pickup × `silver_gain`, rounded; the results performance term × `silver_gain` | **Nothing grants it** | `src/pickups.rs:262-265`; `src/director.rs:298` |
| 27 | **ChestDiscount** | 0 (cap 0.6) | Chest cost and Shady Guy prices × `(1 − discount)` | Space Credit Card | `src/interact.rs:113-121`, `440-442` |

**Four stats are defined but granted by nothing:** Shield, EliteDamage, Knockback and SilverGain. They keep their defaults (0, 1.0, 1.0, 1.0) in every run.

### 7.2 Damage taken, in order

`apply_player_hits` (`src/combat.rs:973-1024`) resolves each hit in this order:

1. If `iframes > 0` or HP ≤ 0, the hit is ignored.
2. **Evasion roll.** On success, "Dodge" is shown and the hit ends.
3. **Armor:** `amount × (1 − effective_armor_fraction)`.
4. **Shield** absorbs first; `shield_cd = 5`.
5. **HP −= remainder;** **iframes = 0.4 s**.
6. **Thorns** reflect to the attacker, if there is one.
7. If HP ≤ 0, HP is set to 0 and the astronaut is **downed** (`dead = true`). A downed astronaut stops firing (`src/combat.rs:237-239`), is ignored by enemy targeting, and does not collect pickups. There is no revive.

Every hit that is not dodged sets iframes, so an astronaut takes **at most one hit per 0.4 s**, whatever the source. A dodge sets no iframes, so the next hit in the same frame is rolled again.

### 7.3 Crit and overcrit

`roll_crit(chance, crit_damage)` (`src/combat.rs:196-208`) loops while `chance > 0`:
- roll `min(chance, 1)`;
- on success, `mult *= crit_damage`;
- then `chance -= 1`.

Example: 130% crit chance with ×2 crit damage always crits once (×2) and has a 30% chance of a second crit (×4 total). Crit is rolled **per target per hit**, so an AoE or aura can crit some targets and not others.

### 7.4 The PAUSED readout

The pause menu prints `DMG xN  AS xN  CRIT N%  SPD xN / ARMOR N  EVA N  LUCK +N%  DIFF +N%` from the local sheet (`src/ui/panels.rs:539-553`). This is the only place the player sees their stats.

---

## 8. Levelling: the XP curve and the level-up cards

### 8.1 XP curve

`xp_needed(L) = XP_BASE + XP_PER_LEVEL·(L−1) + XP_QUAD·(L−1)²` = **`6 + 3.4·(L−1) + 0.18·(L−1)²`** (`src/run.rs:387-390`; constants at `src/config.rs:44-46`).

`gain_xp(v)` adds `v × xp_gain` and loops. Several levels can be gained from one gem; each adds one `pending_levelups` (`src/run.rs:348-356`). Leftover XP carries over.

| Level | XP to next | Total XP to reach | Level | XP to next | Total XP to reach |
|---|---|---|---|---|---|
| 1 | 6.00 | 0 | 21 | 146.00 | 1,210.60 |
| 2 | 9.58 | 6.00 | 22 | 156.78 | 1,356.60 |
| 3 | 13.52 | 15.58 | 23 | 167.92 | 1,513.38 |
| 4 | 17.82 | 29.10 | 24 | 179.42 | 1,681.30 |
| 5 | 22.48 | 46.92 | 25 | 191.28 | 1,860.72 |
| 6 | 27.50 | 69.40 | 26 | 203.50 | 2,052.00 |
| 7 | 32.88 | 96.90 | 27 | 216.08 | 2,255.50 |
| 8 | 38.62 | 129.78 | 28 | 229.02 | 2,471.58 |
| 9 | 44.72 | 168.40 | 29 | 242.32 | 2,700.60 |
| 10 | 51.18 | 213.12 | 30 | 255.98 | 2,942.92 |
| 11 | 58.00 | 264.30 | 31 | 270.00 | 3,198.90 |
| 12 | 65.18 | 322.30 | 32 | 284.38 | 3,468.90 |
| 13 | 72.72 | 387.48 | 33 | 299.12 | 3,753.28 |
| 14 | 80.62 | 460.20 | 34 | 314.22 | 4,052.40 |
| 15 | 88.88 | 540.82 | 35 | 329.68 | 4,366.62 |
| 16 | 97.50 | 629.70 | 36 | 345.50 | 4,696.30 |
| 17 | 106.48 | 727.20 | 37 | 361.68 | 5,041.80 |
| 18 | 115.82 | 833.68 | 38 | 378.22 | 5,403.48 |
| 19 | 125.52 | 949.50 | 39 | 395.12 | 5,781.70 |
| 20 | 135.58 | 1,075.02 | 40 | 412.38 | 6,176.82 |

XP sources, all before `xp_gain`:

| Source | XP | Source |
|---|---|---|
| Normal enemy kill | the kind's `xp` (1.0–3.5, §10) | `src/pickups.rs:334-336` |
| Elite kill | kind `xp` × 8 | `src/enemies.rs:506` |
| Any boss or miniboss kill | 50 | `src/enemies.rs:665` |
| Anything killed during The Static | 0 (silver instead) | `src/pickups.rs:329-333` |

XP is dropped as a gem and must be collected (§17). Each level also gives **+1 Max HP** (`src/run.rs:270`).

### 8.2 When the panel opens

`levelup_trigger` (`src/director.rs:118-143`) runs only in `Playing` and only for the machine's `LocalPlayer`. If `pending_levelups > 0`:
- it rolls 4 options;
- it recomputes stats, so B0-NK's per-level crit is updated;
- it sets `RunPhase::LevelUp`, which pauses the world.

After a pick, if more level-ups are pending, the panel re-rolls in place (`src/ui/panels.rs:244-256`). The title stays `LEVEL <current level>`: after a multi-level jump, every queued panel shows the same, final level number. Level-ups queue behind any open chest, shop or shrine modal.

### 8.3 How the four options are rolled

`roll_upgrades` (`src/run.rs:510-577`):

1. **Evolution cards first.** Each owned weapon at level ≥ 7 whose catalyst is owned adds one `EVOLVE` card, up to **2**.
2. **Build a weighted pool:**
   - **Weapon level-ups:** every owned weapon below level 7 is added **twice** (weight ×2). Evolved weapons count.
   - **New weapons:** only if fewer than 4 weapons are held. Adds each base weapon in `save.unlocked_weapons` that is not held and whose evolution is not held (weight 1).
   - **Items:** each item that is not banned and below max stacks gets `copies` entries (**Common 3, Rare 2, Epic 1, Legendary 1**). Each copy is **kept only if** `rng.gen_bool(rarity_pass(rarity, luck))` succeeds (§9.2). The entry is `NEW` if the count is 0, otherwise `ItemUp`.
3. **Shuffle** the pool and take entries in order, skipping duplicates, until there are 4 options.
4. **Fill** any remaining slots with **`GoldPile(15..45)`**, a uniform 15–44 gold.

Card rarity (the border colour) is cosmetic (`src/run.rs:405-419`):

| Card | Rarity shown |
|---|---|
| EVOLVE | Legendary |
| NEW weapon | Rare |
| Weapon level-up | Common (Epic if the weapon is an evolution) |
| Item | The item's rarity |
| Gold pile | Common |

### 8.4 Refresh, Banish, Skip

These three actions exist on level-up panels only. Charge-shrine, Moai and Microwave panels have no refresh, banish or skip, and must be picked from. The chest and Shady Guy use their own panels, which can be left without taking anything (§16.3, §16.4). Source: `src/ui/panels.rs:171-230`.

| Action | Key | Charges | Effect |
|---|---|---|---|
| **Refresh** | R or button | 2 per run (carried across stages); **Fortuna: free and unlimited** | Re-rolls all options (`src/ui/panels.rs:190-202`) |
| **Banish** | B, then pick a card | 3 per run | Removes the picked card from this panel. If it is an item, the item is **banned for the rest of the run**. Weapon and gold cards are only removed from this panel. (`src/ui/panels.rs:203-207`, `221-230`) |
| **Skip** | S or button | unlimited | **+10 gold**, and consumes the pending level-up (`src/ui/panels.rs:208-213`) |
| Pick | 1–4 or click | — | Applies the upgrade. An evolution plays the "WEAPON EVOLVED" banner and 0.18 s of hitstop. (`src/ui/panels.rs:232-241`) |

Key overlaps: **S** is also move-back and **B** is also the dev boss-summon key while playing (§27).

**GDD** (`GDD.md:282-295`, `1223-1224`):
- The GDD deals **3** cards; the code deals **4**.
- GDD Refresh is "2 free/run, then Gold (rising each use)". The code has no gold refresh.
- GDD Skip gives "XP boost + Gold tip". The code gives +10 gold only.
- GDD Banish deletes "permanently from this run's pool". The code does that only for items.

---

## 9. Rarity and luck

Four grades: **Common, Rare, Epic, Legendary** (`src/content/mod.rs:11-17`).

| Grade | UI colour (sRGB) |
|---|---|
| Common | (0.75, 0.78, 0.80) grey |
| Rare | (0.30, 0.65, 1.00) blue |
| Epic | (0.75, 0.35, 1.00) purple |
| Legendary | (1.00, 0.72, 0.15) gold |

Source: `src/content/mod.rs:19-28`.

There are **two separate luck models**.

### 9.1 `Rarity::weights`: chests, Shady Guy stock, charge shrines, Moai

`weights(luck)` with `l = clamp(luck, 0, 3)` (`src/content/mod.rs:38-46`):

| Grade | Weight |
|---|---|
| Common | `max(62 − 30·l, 8)` |
| Rare | `26 + 8·l` |
| Epic | `9 + 14·l` |
| Legendary | `3 + 8·l` |

`roll` picks a grade by weight (`src/content/mod.rs:47-58`). For `l ≤ 1.8` the weights always sum to exactly 100, so they are percentages. Above 1.8 Common floors at 8 and the total becomes `46 + 30·l`.

| Effective luck | Common | Rare | Epic | Legendary | Where this luck value occurs |
|---|---|---|---|---|---|
| 0.00 | 62% | 26% | 9% | 3% | Chest or shop with no luck |
| 0.15 | 57.5% | 27.2% | 11.1% | 4.2% | Moai with no luck (+0.15 bonus) |
| 0.30 | 53% | 28.4% | 13.2% | 5.4% | Charge shrine with no luck (+0.30); Fortuna's chests |
| 0.50 | 47% | 30% | 16% | 7% | |
| 1.00 | 32% | 34% | 23% | 11% | |
| 1.80 | 8% | 40.4% | 34.2% | 17.4% | Common reaches its floor |
| 3.00 (cap) | 5.9% | 36.8% | 37.5% | 19.9% | Maximum |

After the grade roll, `roll_item` picks uniformly among items of that grade that are not banned and are below max stacks. If there are none, it picks uniformly among **all** items below max stacks, ignoring bans (`src/interact.rs:92-111`).

### 9.2 `rarity_pass`: the level-up item pool

`rarity_pass(rarity, luck)` with `l = clamp(luck, 0, 2)` is the chance that each copy of an item enters the pool (`src/run.rs:579-587`):

| Grade | Formula | l = 0 | l = 0.3 | l = 1 | l = 2 (cap) |
|---|---|---|---|---|---|
| Common | 0.9 | 90% | 90% | 90% | 90% |
| Rare | 0.55 + 0.2·l | 55% | 61% | 75% | 95% |
| Epic | 0.30 + 0.25·l | 30% | 37.5% | 55% | 80% |
| Legendary | 0.12 + 0.2·l | 12% | 18% | 32% | 52% |

Combined with the copies (C 3, R 2, E 1, L 1), a Common item averages 2.7 pool entries at any luck, while the single Legendary (Splitter Chip) averages 0.12 entries at luck 0.

### 9.3 Where luck comes from

| Source | Luck |
|---|---|
| Lucky Meteorite | +0.12 per stack |
| Cursed Moon Rock | +0.10 per stack |
| Lady Fortuna | +0.30 |
| Greed Shrine | +0.08 per use (run-global stack) |
| Charge-shrine rolls | +0.30 on top of the sheet, for that roll only (`src/interact.rs:378`) |
| Moai rolls | +0.15 on top of the sheet, for that roll only (`src/interact.rs:504`) |

**GDD** (`GDD.md:533-536`) says Luck improves "elite drops, Microwave gambles, and evolution-card appearance rate". In the code it affects none of those; elite drops and the Microwave do not read luck.

---

## 10. Enemies (8 kinds + The Static)

Defined in `src/content/enemies.rs:40-152`. Systems live in `src/enemies.rs`.

### 10.1 Base stat table

| Kind | HP | Contact dmg | Speed (m/s) | XP | Scale | Hover (m) | Standoff (m) | Colour (sRGB) | Source |
|---|---|---|---|---|---|---|---|---|---|
| **Shambler** | 14 | 5 | 3.1 | 1.0 | 1.0 | 0 | 0 (melee) | (0.45, 0.75, 0.45) | `content/enemies.rs:43-54` |
| **Sprinter** | 9 | 5 | 6.2 | 1.2 | 0.8 | 0 | 0 | (0.85, 0.85, 0.30) | `content/enemies.rs:55-66` |
| **Bruiser** | 70 | 14 | 2.2 | 4.0 | 1.7 | 0 | 0 | (0.70, 0.35, 0.30) | `content/enemies.rs:67-78` |
| **Spitter** | 18 | 8 | 2.6 | 2.0 | 1.1 | 0 | **13** | (0.60, 0.45, 0.85) | `content/enemies.rs:79-90` |
| **UFO** | 24 | 7 | 4.5 | 2.5 | 1.0 | **4.0** | **9** | (0.55, 0.85, 0.95) | `content/enemies.rs:91-102` |
| **Burrower** | 30 | 12 | 5.0 | 3.0 | 1.2 | 0 | 0 | (0.75, 0.55, 0.35) | `content/enemies.rs:103-114` |
| **Beamer** | 26 | 14 | 2.8 | 3.0 | 1.3 | 0 | **22** | (1.00, 0.30, 0.55) | `content/enemies.rs:115-126` |
| **Lobber** | 45 | 16 | 1.8 | 3.5 | 1.5 | 0 | **17** | (0.55, 0.60, 0.30) | `content/enemies.rs:127-138` |
| **The Static** (`Ghost`) | 20 | 10 | 5.4 | 0 (drops silver) | 1.1 | 0.6 | 0 | (0.80, 0.85, 1.00), 55% alpha, glowing | `content/enemies.rs:139-150` |

At spawn (`spawn_enemy`, `src/enemies.rs:483-536`):

| Field | Value at spawn |
|---|---|
| HP | `hp × hp_mult` (× 8 if elite) |
| Contact damage | `damage × dmg_mult` (× 1.8 if elite) |
| Speed | `speed × U(0.9, 1.15)` |
| Scale | `scale × U(0.92, 1.1)` (× 1.65 if elite) |
| XP | `xp` (× 8 if elite) |

`hp_mult` and `dmg_mult` come from time scaling (§11.3). **Stats are frozen at spawn**: difficulty raised later does not affect enemies already alive.

### 10.2 Derived hit radii (from scale)

| Test | Formula | Shambler (1.0) | Bruiser (1.7) | Elite Bruiser (2.8) | Source |
|---|---|---|---|---|---|
| Contact reach to the astronaut | `scale·0.55 + 0.45 + 0.25` | 1.25 m | 1.64 m | 2.24 m | `src/enemies.rs:1259` |
| Separation from neighbours | `scale·0.9 + 0.6` | 1.50 m | 2.13 m | 3.12 m | `src/enemies.rs:1097` |
| Player projectile reach (size 1) | `0.45·size + 0.55 + scale·0.5` | 1.50 m | 1.85 m | 2.40 m | `src/combat.rs:669-677` |
| Drone reach | `0.8 + scale·0.5` | 1.30 m | 1.65 m | 2.20 m | `src/combat.rs:768` |
| Beam corridor half-width | `width + scale·0.5` | width + 0.5 | width + 0.85 | width + 1.4 | `src/combat.rs:830` |

Contact reach is tested as **3D distance** between centres. Walkers stand at `scale·0.6` above the ground; the astronaut's centre is 0.85 m up.

*Derived:* a UFO hovers 4.0 m up (±0.61 bob), so its centre is at least 3.1 m above a standing astronaut's, more than its 1.25 m reach. **The UFO's contact damage effectively never lands on a grounded player**; it hurts only with its bolts.

### 10.3 Movement (all kinds)

`enemy_move` (`src/enemies.rs:1019-1121`):

- **Target.** Each enemy chases the **nearest living astronaut by arc distance**.
- **Melee kinds (standoff 0)** beeline along the great circle at `speed × (1 − slow)`.
- **Ranged kinds** follow three bands:
  - beyond `standoff`: close in;
  - between `0.65·standoff` and `standoff`: **circle-strafe**;
  - inside `0.65·standoff`: **back away**.

| Kind | Standoff | Backs away inside | Strafe band |
|---|---|---|---|
| Spitter | 13 | 8.45 m | 8.45–13 m |
| UFO | 9 | 5.85 m | 5.85–9 m |
| Beamer | 22 | 14.3 m | 14.3–22 m |
| Lobber | 17 | 11.05 m | 11.05–17 m |

- **Separation:** pushes away from up to 6 neighbours at `0.35 × dt`.
- **Knockback:** a decaying tangent impulse, multiplied by `(1 − 7·dt)` each frame.
- **Slow:** decays 0.35 per second and is clamped to 0.9.
- **Props:** enemies ignore rocks and all other props. Only the player collides with them.

### 10.4 Attacks

| Kind | Attack | Numbers | Source |
|---|---|---|---|
| **All** (walking, not buried) | **Contact** | Deals its contact damage when within reach, at most once per **`CONTACT_TICK` = 0.5 s** per enemy. Pots (speed 0) never attack. | `src/enemies.rs:1237-1271`; `src/config.rs:34` |
| **Spitter** | Lobbed bolt | Fires when the target is **< 20 m** (arc). Cooldown **2.8 s** (first shot after 1–3 s). Bolt speed **11**, life **4 s** (reach 44 m), 1 m above the ground. Damage = the Spitter's contact damage. | `src/enemies.rs:1274-1323`, `524-526` |
| **UFO** | Zap bolt | Same system as the Spitter: range **< 15 m**, cooldown **2.2 s**, speed **16**, life 4 s (reach 64 m). Damage = the UFO's contact damage. | `src/enemies.rs:1303-1307` |
| **Beamer** | Railbolt with aim line | First charge after 2–4 s. When cooldown ≤ 0 and the target is **< 26 m** (arc), it **charges 1.1 s**, painting a 24 m aim line that tracks the latched target until the last **0.25 s**, then locks. It then fires a railbolt: speed **40**, life **1.4 s** (56 m), damage = the Beamer's contact damage. Cooldown **4 s**. | `src/enemies.rs:1327-1443`, `530-532` |
| **Lobber** | Mortar | First shot after 2.5–5 s. When the target is **< 24 m** (arc), it drops a **telegraph disc of radius 3.2** at the target's current position. A shell arcs for **1.6 s** (peak 9 m), and on landing everyone within **3.8 m** (radius + 0.6) of the centre takes the Lobber's contact damage. Cooldown **4.5 s**. | `src/enemies.rs:1446-1520`, `1608-1647`, `533-535` |
| **Burrower** | Ambush eruption | A director spawn places it only **9–16 m** (arc) from an astronaut, instead of 42–58 m (Burrowers in a boss add ring use the ring's 42–58 m). It stays **buried for 1.3 s**, rising under the ground; while buried it does not move and has no contact damage. On emerging it hits every astronaut within **2.6 m** for its contact damage, then chases at 5.0 m/s. | `src/enemies.rs:617-623`, `527-529`, `1196-1234` |
| **The Static** | Contact only | Standard contact | — |

**Enemy projectiles** hit the first astronaut whose centre is within **~1.05 m** (`distance² < 1.1`) and are consumed (`src/enemies.rs:1549-1556`).

**Dust storm (Mars):** while the host's local astronaut is inside the storm cell, **Spitters, UFOs and Lobbers do not fire, and Beamers drop their aim lines and hold** (`src/enemies.rs:1284`, `1342-1350`, `1456`). See §15.5 and M9.

**Buried Burrowers:** melee, aura, beam, chain and auto-aim skip them (`Without<Buried>`). Projectiles, drones, rockets and the comet can still hit them through the spatial hash (low-severity bug).

### 10.5 When each kind joins the spawn mix

`EnemyKind::mix(elapsed)` (`src/content/enemies.rs:155-166`) returns the set of kinds. The spawner picks **uniformly** from it (`src/enemies.rs:610-611`). Because `elapsed` resets each stage, every stage replays this ladder from the start and is cut off by its own clock:

| Elapsed (s) | Mix | Stage 1 timer | Stage 2 timer | Stage 3 timer | Mean base HP of mix |
|---|---|---|---|---|---|
| 0–89 | Shambler | 10:00–8:31 | 9:00–7:31 | 8:00–6:31 | 14.0 |
| 90–179 | + Sprinter | 8:30 | 7:30 | 6:30 | 11.5 |
| 180–269 | + Spitter | 7:00 | 6:00 | 5:00 | 13.7 |
| 270–359 | + Bruiser | 5:30 | 4:30 | 3:30 | 27.75 |
| 360–449 | + UFO, Beamer | 4:00 | 3:00 | 2:00 | 26.8 |
| 450–539 | + Burrower | 2:30 | 1:30 | 0:30 | 27.3 |
| 540+ | + Lobber | 1:00 | (0:00 = Static) | never | 29.5 |

- **Lobbers never come from the director on stages 2 and 3.** Stage 2 reaches e = 540 exactly when The Static replaces the mix; stage 3 ends at e = 480.
- **Boss add rings** use `mix(max(elapsed, 300))` (§13.4), so they always contain at least Shambler, Sprinter, Spitter and Bruiser. At e ≥ 540 they can include Lobbers.

**GDD** (`GDD.md:215-232`): the GDD has Sprinters at 9:00–7:30 and Bruisers and Spitters at 7:00–4:30; the code has Sprinters at 8:30, Spitters at 7:00 and Bruisers at 5:30. The GDD has Burrowers, Beamers and Lobbers at 4:30–2:30; the code has UFO and Beamer at 4:00, Burrower at 2:30 and Lobber at 1:00.

**Not built** (`GDD.md:711-727`): the 13 GDD enemies Rollo, Splitshroom, Aegis Drone, and the rest.

---

## 11. Spawning, the enemy cap and time scaling

`director_spawn` (`src/enemies.rs:539-626`). It runs only on the simulating machine (solo or host) and only while `Playing`.

### 11.1 Spawn rate

Spawns accrue into `spawn_bank` every frame. The per-second rate is:

| Phase | Rate (enemies per second) | Source |
|---|---|---|
| Before The Static | `(1 + 2.1·t) × (1 + D) × P`, with `t = elapsed/60` | `src/enemies.rs:573-576` |
| During The Static | `(10 + 0.15·static_timer) × P` (no D term) | `src/enemies.rs:570-571` |

- **Batches:** every **0.25 s** the whole-number part of the bank is spawned (`src/enemies.rs:581-590`).
- **Cap:** `cap = floor(ENEMY_CAP × P) = floor(1200 × P)`, and `room = cap − alive` (`src/enemies.rs:592-594`).
- **Overflow is discarded.** The bank is debited by the full budget even when there is no room, so no backlog bursts out later (`src/enemies.rs:590`).
- **`alive` counts every `Enemy` entity:** normal enemies, **pots** (38–68 per planet at stage start) and **bosses** (`src/enemies.rs:548`, `562`).
- **Boss spawns and boss add rings bypass the cap.**
- **Placement:** each spawn picks an anchor (the astronauts, round-robin), a random heading, and an arc of **`SPAWN_ARC_MIN`–`SPAWN_ARC_MAX` = 42–58 m** from it, over the horizon (`src/enemies.rs:595-603`; `src/config.rs:32-33`). Burrowers use 9–16 m instead.
- **Randomness:** spawns draw from `GameRng`, seeded with `run_seed + stage`, so the random stream is reproducible for a seed (`src/enemies.rs:560`). Which enemy each draw becomes still depends on frame timing (batch sizes, the `elapsed` at each batch) and on how much room the cap leaves, so two runs on one seed diverge once their timing differs.

| Party size | P | Cap |
|---|---|---|
| 1 | 1.00 | 1,200 |
| 2 | 1.75 | 2,100 |
| 3 | 2.40 | 2,880 |
| 4 | 3.00 | 3,600 |

*Derived* solo rates (D = 0, P = 1):

| Elapsed | 0:00 | 1:00 | 2:00 | 3:00 | 4:00 | 5:00 | 6:00 | 7:00 | 8:00 | 9:00 | 10:00 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Spawns/s | 1.0 | 3.1 | 5.2 | 7.3 | 9.4 | 11.5 | 13.6 | 15.7 | 17.8 | 19.9 | 22.0 |

Total spawns attempted per stage at D = 0, solo: **6,900** (stage 1, 10 min), **5,643** (stage 2, 9 min), **4,512** (stage 3, 8 min).

Static rates: **10/s** at 0:00, 14.5/s at +30 s, 19/s at +1 min, 28/s at +2 min, 55/s at +5 min (× P).

**GDD:**
- Spawn rate: the GDD says `SpawnRate = Rate_base × (1 + 0.14·t) × (1 + 0.10·d) × (1 + 0.04·Δ)` (`GDD.md:263`). The code's slope is **2.1 per minute, about 15× steeper**; the full D multiplies the rate; and there is no depth term.
- Overflow: the GDD says overflow "merges into The Static" (`GDD.md:705`, `1222`). The code **discards** it.

### 11.2 Party scale

`P = [1.0, 1.75, 2.4, 3.0][players − 1]` (`src/enemies.rs:568`). This matches the GDD's spawn-count column (`GDD.md:879-888`). The GDD's per-enemy HP and boss HP party scaling is **not built**.

### 11.3 Time scaling of enemy HP and damage

`time_scaling(elapsed, D)` (`src/content/enemies.rs:238-243`), with `t = elapsed / 60`:

```
hp_mult  = (1 + 0.11·t·max(√t, 1) + 0.35·t) × (1 + D)
dmg_mult = (1 + 0.12·t) × (1 + 0.5·D)
```

(`0.22 × … × 0.5` in the source is 0.11.) For `t < 1`, `max(√t, 1) = 1`, so `hp_mult = 1 + 0.46·t`. From `t ≥ 1` it is `1 + 0.11·t^1.5 + 0.35·t`.

| Elapsed (min) | 0 | 0.5 | 1 | 1.5 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 12 | 15 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **hp_mult** (D=0) | 1.00 | 1.23 | 1.46 | 1.73 | 2.01 | 2.62 | 3.28 | 3.98 | 4.72 | 5.49 | 6.29 | 7.12 | 7.98 | 9.77 | 12.64 |
| **dmg_mult** (D=0) | 1.00 | 1.06 | 1.12 | 1.18 | 1.24 | 1.36 | 1.48 | 1.60 | 1.72 | 1.84 | 1.96 | 2.08 | 2.20 | 2.44 | 2.80 |

*Derived* enemy stats at D = 0:

| Kind | HP when it joins (stage 1) | Contact dmg when it joins | HP at 10:00 | Dmg at 10:00 | Elite HP at 10:00 |
|---|---|---|---|---|---|
| Shambler | 14.0 (e=0) | 5.0 | 111.7 | 11.0 | 894 |
| Sprinter | 15.5 (e=90) | 5.9 | 71.8 | 11.0 | 574 |
| Spitter | 47.2 (e=180) | 10.9 | 143.6 | 17.6 | 1,149 |
| Bruiser | 253.8 (e=270) | 21.6 | 558.5 | 30.8 | 4,468 |
| UFO | 113.2 (e=360) | 12.0 | 191.5 | 15.4 | 1,532 |
| Beamer | 122.6 (e=360) | 24.1 | 207.4 | 30.8 | 1,660 |
| Burrower | 176.5 (e=450) | 22.8 | 239.4 | 26.4 | 1,915 |
| Lobber | 320.4 (e=540) | 33.3 | 359.0 | 35.2 | 2,872 |
| The Static | 159.6 (e=600) | 22.0 | grows with e (§14) | | |

D multiplies HP by `(1 + D)` and damage by `(1 + D/2)`, and it multiplies the spawn rate by `(1 + D)` too. So +0.14 D means 14% more HP per enemy *and* 14% more enemies: about 1.14 × 1.14 ≈ **+30% damage output** is needed to keep pace.

**GDD** (`GDD.md:256-269`): the GDD's HP curve is `(1 + 0.11·t)^1.35 × (1 + 0.20·d) × T × (1 + 0.06·Δ)` and its damage curve is `(1 + 0.08·t) × (1 + 0.15·d) × T × (1 + 0.05·Δ)`. The code's curves above differ in slope, exponent, the depth and planet terms, and the D weighting.

---

## 12. Elites

An elite is a normal enemy kind with flat multipliers and an orange glowing material (`src/content/enemies.rs:229-235`; material at `src/enemies.rs:406-411`).

| Modifier | Value |
|---|---|
| HP | × **8.0** |
| Contact and attack damage | × **1.8** |
| Scale | × **1.65** |
| XP | × **8.0** |

**How elites are chosen** (`src/enemies.rs:612-616`), for each director spawn outside The Static:

1. **Forced elite timer.** `Director.elite_timer` starts at **45 s** each stage (`src/enemies.rs:214-218`) and ticks down every frame. The first spawn after it reaches ≤ 0 is elite, and the timer resets to **40 s**. *Derived:* one guaranteed elite at about e = 45, 85, 125, … s. If the cap is full, the elite comes with the next spawn that fits.
2. **Random elites.** After **e > 150 s**, every spawn also has a **1.2%** chance to be elite.
3. **Add rings.** In a boss's third phase, each add has a 25% elite chance (§13.4).
4. **The Static** never spawns elites.

**Elite loot** (`src/pickups.rs:338-345`):
- the XP gem (kind XP × 8);
- **4–7 gold piles** of **4–9 gold** each;
- a **35%** chance of a random powerup.

Minibosses and stage bosses also count as elite for loot (§13.6).

**GDD** (`GDD.md:731-747`): the "Glitched" affix system (1–3 colour-telegraphed affixes such as Overclocked and Leaden) is **not built**. The GDD elite chance `min(0.35, 0.02·t + …)` (`GDD.md:264`) is replaced by the fixed 40 s timer plus 1.2%.

---

## 13. Minibosses and stage bosses

Defined in `src/content/enemies.rs:180-226`. Spawned by `spawn_boss` (`src/enemies.rs:628-714`); scheduled by `run_clock` (`src/director.rs:74-96`).

### 13.1 Boss table

| Boss | Role | When | HP | Contact dmg | Speed | Scale | Mesh | Source |
|---|---|---|---|---|---|---|---|---|
| **Craterpillar Jr** | Miniboss #1 | timer ≤ 7:00, **every planet** | 700 | 16 | 3.4 | 2.6 | Generic horned boss mesh, purple | `content/enemies.rs:184-193` |
| **Rover Gone Wrong** | Miniboss #2 | timer ≤ 2:00, **every planet** | 1,100 | 20 | 5.2 | 2.2 | Generic horned boss mesh, purple | `content/enemies.rs:194-203` |
| **THE CRATERPILLAR** | Stage boss, Moon | timer ≤ 1:30 on the Moon | 5,200 | 26 | 3.0 | 4.6 | Armoured worm head + 12 segments | `content/enemies.rs:204-213` |
| **JUDGE ANUBOT** | Stage boss, Mars **and the Dark Moon** | timer ≤ 1:30 on any planet other than the Moon | 8,200 | 32 | 3.6 | 4.2 | Jackal-headed rover | `content/enemies.rs:214-223` |

- The planet → boss mapping is `PlanetKind::Moon → Craterpillar, _ → Anubot` (`src/director.rs:89-92`), so **the Dark Moon reuses Judge Anubot**.
- The miniboss kinds are fixed per mark index, not per planet (`src/director.rs:79`).

### 13.2 Values shared by every boss (miniboss or stage boss)

| Property | Value | Source |
|---|---|---|
| Spawn position | **30 m** arc from the party centroid, random heading (`thread_rng`) | `src/enemies.rs:639-644`; `src/director.rs:63-72` |
| HP | `def.hp × (1 + D)`. **No time scaling and no party scaling.** | `src/enemies.rs:645` |
| Contact damage | `def.damage × (1 + D/2)` | `src/enemies.rs:656` |
| Entity | `Enemy{kind: Bruiser, elite: true, xp: 50}` plus `Boss` | `src/enemies.rs:659-677` |
| Movement | Beelines at `def.speed` (Bruiser kind, so standoff 0); knockback taken ×0.05 | `src/combat.rs:928` |
| Cap | Bosses and their add rings spawn even when the cap is full. Once alive they are ordinary `Enemy` entities, so they **do** count toward `alive` and shrink the room for director spawns (§11.1) | `src/enemies.rs:562`, `592-594` |

Because a boss is `elite: true`, `EliteDamage` applies to it and it drops elite loot.

### 13.3 The shared boss attack kit (all four bosses)

`boss_attacks` (`src/enemies.rs:1561-1605`) runs for **every** `Boss`, minibosses included.

| Attack | First use | Repeat | Numbers |
|---|---|---|---|
| **Slam ring** | 4.0 s after spawn | every **6.5 s** | A telegraph at the boss's feet grows over **1.4 s** to radius **7**. It then hits everyone in the **ring band** `7 × 0.35 < d < 7 + 1.0`, i.e. **2.45–8.0 m**, for **1.6 × contact damage**. Standing right under the boss is safe. Plays the boss roar. |
| **Radial burst** | 7.0 s after spawn | every **9 s** | **12** bolts evenly spaced in all directions. Speed **9**, life **5 s** (45 m), damage **0.8 × contact damage**. |

Ring hit test: `src/enemies.rs:1630-1634`.

### 13.4 Phases, enrage and add rings

`boss_phase_system` (`src/enemies.rs:720-793`):

| Phase | HP fraction | Banner (Craterpillar) | Banner (Anubot) | Banner (minibosses) |
|---|---|---|---|---|
| P1 | ≥ 66% | — | — | — |
| P2 | < 66% | "THE CRATERPILLAR — BURROW BLOOM!" | "JUDGE ANUBOT — SANDSTORM COURT!" | "<name> — ENRAGED!" |
| P3 | < 33% | "THE CRATERPILLAR — HELMET CHOIR!" | "JUDGE ANUBOT — FINAL JUDGMENT!" | "<name> — ENRAGED!" |

**On entering P2 or P3** (each transition applies once):
- **Speed ×1.28** and **damage ×1.22**. Damage is the base for contact, slam and burst, so these compound: P3 = ×1.638 speed and ×1.488 damage.
- The slam timer is cut to ≤ 1.5 s and the burst timer to ≤ 2.0 s, so both attacks follow almost at once.
- Banner, boss roar and screen shake 0.55.
- **Add ring:** `8 + 3 × phase` enemies. **P2 → 11 adds, P3 → 14 adds.**
  - They are placed evenly (±0.2 rad jitter) at **42–58 m** around the **nearest living astronaut**.
  - Kinds are drawn from `mix(max(elapsed, 300))`, so the pool is at least Shambler, Sprinter, Spitter and Bruiser.
  - Their HP and damage use the current time scaling.
  - In P3 each add has a **25%** elite chance.
  - Adds **ignore the enemy cap**. If no astronaut is alive the ring is skipped, but the enrage still happens.

If one frame of damage takes a boss from above 66% to below 33%, it jumps straight to P3: **one** enrage and **one** ring of 14.

**GDD** (`GDD.md:749-766`): the authored phase mechanics are not built:
- Craterpillar: Burrow Bloom eruptions from every compass point; Helmet Choir homing helmets.
- Anubot: Aegis Drones and a dust wall in Sandstorm Court; polar Beamer-Prime pillars in Final Judgment.

The phases today are the generic enrage plus a named banner and an add ring.

### 13.5 Boss-specific attacks

**THE CRATERPILLAR: the worm body** (`src/enemies.rs:63-65`, `686-701`, `941-1016`)

| Property | Value |
|---|---|
| Segments | **12** (`WORM_SEGMENTS`) |
| Trail | The head records its path every **0.55 m** (`WORM_TRAIL_STEP`); segment *i* sits at trail point `i × 2` (`WORM_STRIDE`) |
| Segment scale | `4.6 × max(0.85 − 0.03·i, 0.4)`: from 3.91 (first) down to 2.39 (twelfth) |
| Segment contact damage | **0.7 × the boss's contact damage at spawn** (18.2 at D = 0). It is **fixed**: enrage does not raise it. |
| Segment reach | `seg_scale × 0.6 + 0.45 + 0.25`, from about 3.05 m down to about 2.13 m |
| Hit rate | Every frame an astronaut overlaps a segment; in practice limited by the 0.4 s iframes |
| Damageable? | **No.** Segments are not `Enemy`: only the head takes damage. Segments despawn when the head dies. |

**JUDGE ANUBOT: the Verdict Beam** (`src/enemies.rs:67-87`, `798-911`)

A lighthouse beam rotates around the boss and cycles **idle → charge → fire → idle**. The phase index `ph` is 0, 1 or 2.

| State | Duration | Rotation (rad/s) | Visual | Damage |
|---|---|---|---|---|
| Idle | 2.5 s the first time, then `max(2.6 − 0.7·ph, 0.8)` = **2.6 / 1.9 / 1.2 s** | 0.25 | hidden | none |
| Charge (telegraph) | `1.3 − 0.3·ph` = **1.3 / 1.0 / 0.7 s** | 0.5 | dim amber bar, width 1.44 | none; the boss rears up |
| Fire | `3.0 + 0.6·ph` = **3.0 / 3.6 / 4.2 s** | `1.15 × (1 + 0.3·ph)` = **1.15 / 1.495 / 1.84** | bright red bar, width 4.8 | every astronaut with `0 < along < 34 m` and perpendicular `< 2.4 m` takes **1.2 × Anubot's current damage** per frame (limited by iframes) |

- Beam length **34 m** (`BEAM_LENGTH`); hit half-width **2.4 m** (`BEAM_WIDTH`).
- The beam hits **every** astronaut in the corridor, not one target.
- `attacker` is the boss, so **Thorns reflect** onto Anubot.

Anubot also uses the shared slam ring and radial burst (§13.3).

### 13.6 Boss death and rewards

- **Stage boss killed** (`src/combat.rs:947-966`, `src/pickups.rs:338-364`):
  - `boss_dead = true`, screen shake 0.8 and 0.25 s of hitstop;
  - loot: the **XP gem (50)**, **elite loot** (4–7 piles of 4–9 gold, 35% powerup chance) and **14 more gold piles of 8–19 gold** each;
  - the next `run_clock` tick opens the **teleporter**, **unless The Static is already active** (§14).
- **Miniboss killed:** it is treated as elite (`elite: e.elite || is_mini`), so it drops the XP gem (50) and elite loot. It does **not** set `boss_dead` or open anything.
- **Kill counter:** every boss kill adds 1 to `run.kills`, like any enemy.

**GDD:**
- The GDD says miniboss #1 is a "Spike → **guaranteed chest**" (`GDD.md:223`); the code drops no chest. Tutorial line 6 promises one too ("It'll drop a chest", `src/tutorial.rs:26`).
- The GDD says the teleporter "blooms at the corpse" (`GDD.md:228`); the code places it **18 m** from the party centroid in a random direction (`src/interact.rs:306-309`).

### 13.7 The DEV summon key

**B** during play summons the current planet's stage boss **30 m** (arc, random heading) from the local astronaut, through the normal `spawn_boss`, and sets `boss_spawned`, so the real boss never comes at 1:30 (`src/enemies.rs:913-936`). It is registered at `src/main.rs:244` with no dev flag and no `is_simulating` gate. In solo or on the host, killing it opens the teleporter, so it is a progression shortcut. On a co-op joiner it spawns a local boss that is never simulated and cannot be killed. It is marked "Remove before ship" (M14).

---

## 14. THE STATIC

The endgame swarm. It is `EnemyKind::Ghost`, named "The Static" (`src/content/enemies.rs:139-150`).

| Rule | Value | Source |
|---|---|---|
| Trigger | The stage timer reaches 0 → `static_active = true`, banner "THE STATIC RISES. RUN OR FARM SILVER." and the boss roar | `src/director.rs:106-114` |
| One-way | Once active, `run_clock` returns early every frame. The timer stays at 0, no further minibosses or bosses are scheduled, and **the teleporter check never runs again**. | `src/director.rs:55-59` |
| Teleporter | If the stage boss was killed **before** 0:00, the teleporter already exists and still works. If the boss is killed **during** The Static, **no teleporter ever opens** on that stage. | `src/director.rs:99-103`, `112` |
| Spawn mix | **Only Ghosts.** No elites. | `src/enemies.rs:605-608` |
| Spawn rate | `(10 + 0.15 × static_timer) × P` per second, capped by `1200 × P` alive | `src/enemies.rs:570-571` |
| Ghost stats | HP `20 × hp_mult(elapsed)`, damage `10 × dmg_mult(elapsed)`, speed 5.4, hover 0.6, scale 1.1 | `src/content/enemies.rs:139-150` |
| Scaling | `elapsed` keeps counting through The Static (`src/director.rs:52`), so ghosts keep getting tougher | — |
| Rewards | **Every** kill during The Static (ghost, leftover enemy or boss) drops **no XP gem**. Instead there is a **50%** chance of **1 silver** (× `silver_gain`, always 1.0 today). | `src/pickups.rs:329-333` |
| Survival time | `static_timer` counts seconds survived. At results, `static_secs_best = max(best, static_timer)` of the **current stage only**. | `src/director.rs:56`, `290` |
| Ending | The Static never ends by itself. Leave through an open teleporter, or die. | — |

*Derived* ghost stats, stage 1, D = 0:

| Time into The Static | e (s) | Ghost HP | Ghost dmg | Spawns/s |
|---|---|---|---|---|
| 0:00 | 600 | 159.6 | 22.0 | 10.0 |
| 0:30 | 630 | 168.4 | 22.6 | 14.5 |
| 1:00 | 660 | 177.3 | 23.2 | 19.0 |
| 2:00 | 720 | 195.5 | 24.4 | 28.0 |
| 5:00 | 900 | 252.8 | 28.0 | 55.0 |

Quests that read it:
- **SurviveStatic2Min** needs `static_secs_best ≥ 120` (+150 silver, +1 tome slot).
- Ironclad's unlock text, "Survive THE STATIC for 5 minutes", is not enforced.

**GDD** (`GDD.md:116-127`, `229`):
- "Each ghost wears the suit, name, and build silhouette of a real prior death" is **not built**; ghosts are generic.
- "every extra second pays Silver" is **not built**. Only kills pay, at 50% × 1 silver. The comment "trickle of silver for surviving" at `src/director.rs:57` has no code behind it.

---

## 15. Planets, tiers and chains

Defined in `src/content/planets.rs:50-126`. `CurrentPlanet::from_kind` (`src/planet.rs:66-81`) turns a definition into the live terrain. `crater_width` is **0.16 rad** on every planet.

### 15.1 Planet table

| Field | THE MOON | MARS | THE DARK MOON |
|---|---|---|---|
| Description | "Home turf. Well. Near-home turf." | "Red, dead, and full of teeth." | "It was not on any chart." |
| **Radius** | **140 m** | **160 m** | **105 m** |
| Circumference (*derived*) | 879.6 m | 1,005.3 m | 659.7 m |
| `hill_amp` | 0.035 (×radius ≈ 4.9 m per unit height) | 0.045 (≈ 7.2 m) | 0.03 (≈ 3.15 m) |
| `rugged` (ridged mountains) | 0.35 | 0.55 | 0.8 |
| Craters | 10 | 6 | 4 |
| Crater depth | 0.8 | 0.5 | 0.6 |
| Terrain seed | 7 | 23 | 66 |
| Ground (low / mid / high, sRGB) | (0.40, 0.40, 0.46) / (0.62, 0.62, 0.66) / (0.82, 0.82, 0.86) | (0.48, 0.24, 0.15) / (0.72, 0.42, 0.26) / (0.88, 0.60, 0.40) | (0.09, 0.07, 0.15) / (0.20, 0.16, 0.28) / (0.36, 0.28, 0.46) |
| Sun colour | (1.0, 0.98, 0.92) | (1.0, 0.85, 0.7) | (0.75, 0.55, 1.0) |
| Crystal tint (`enemy_tint`) | (0.5, 0.9, 0.6) | (0.95, 0.55, 0.35) | (0.8, 0.5, 1.0) |
| Rocks | 200 | 250 | 120 |
| Boulders (*derived*: `max(rocks/20, 4)`) | 10 | 12 | 6 |
| Crystals | 42 | 52 | 85 |
| Lander wrecks (*derived*: `rocks/45 + 3`) | 7 | 8 | 5 |
| Radio beacons (*derived*: `crystals/8 + 4`) | 9 | 10 | 14 |
| Flora | 40 × Spires (pale mineral cones) | 64 × Thorns (rust-red shrubs) | 55 × GlowShrooms (glowing fungus) |
| **Pots** | **60** | **68** | **38** |
| Earthrise | yes (Earth sphere r = 90 at 1,500 m) | no | no |
| `meteor_showers` | true (**never read**) | true (**never read**) | false |
| `sky` colour | defined, **never read**; no ClearColor is set | same | same |
| Stage boss | THE CRATERPILLAR | JUDGE ANUBOT | JUDGE ANUBOT (placeholder) |
| World event | none | **Migrating Dust Storm** (§15.5) | none |
| Source | `planets.rs:53-76` | `planets.rs:77-100` | `planets.rs:101-124` |

Every planet also has **420 stars** at 1,400–1,900 m and a fixed sun (`src/planet.rs:441-500`). The code defines `sun_dir = normalize(−0.55, 0.35, −0.75)`. The DirectionalLight (9,000 lux, shadowed, tinted by the planet's sun colour) is placed at `−sun_dir × 10` and aimed at the core, so its light travels along `+sun_dir`, and the visible sun disc (radius 45) sits at `−sun_dir × 1,600 m`, i.e. in direction (0.55, −0.35, 0.75) from the planet centre. The sun never moves, so there is no day/night sweep.

### 15.2 Terrain formula

`Terrain::height(dir)` (`src/sphere.rs:83-106`) returns a unitless height (about −1.5 to +2):

1. **Hills:** `hills(dir, seed)` sums 13 fixed sine waves over the unit sphere (frequencies 3–43; amplitude ×0.75 per wave), normalised to [−1, 1] (`src/sphere.rs:8-37`).
2. **Ridged mountains:** `+ rugged × 1.7 × max(ridge, 0) × mask`.
   - `ridge = 1 − 2·|hills₆(seed+101)|`.
   - `mask = clamp(hills₄(seed+202)·1.6 − 0.15, 0, 1)`, so only some regions are alpine.
3. **Craters:** for each of `craters` centres `hash_dir(seed+31, i)`, with width `0.16 × (0.7 … 1.3)` rad, add `crater_depth × (bowl + rim)`.
   - `bowl = −(cos(πt)·0.5 + 0.5)`.
   - `rim = sin³(πt) × 0.35`.

**Surface radius** = `radius × (1 + hill_amp × height)` (`src/sphere.rs:108-110`). Collision, props and the icosphere(7) render mesh all sample this one function (`src/planet.rs:98-130`). The terrain seed is the fixed planet seed, **not** the run seed, so every Moon has the same hills.

### 15.3 Props (per stage, from `StdRng(stage_seed ^ 0xA11CE ^ terrain_seed)`)

Source: `src/planet.rs:143-503`. Only the **player** collides with solid props; enemies pass through them.

| Prop | Count | Scale | Solid? (collider radius / height) | Source |
|---|---|---|---|---|
| Rocks (6 faceted variants) | `rocks`, Fibonacci-scattered with jitter | `U(0.4, 2.4) × (U(0.8,1.3), U(0.6,1.1), U(0.8,1.3))` | only if scale.x > 1.1: `0.75·sx` / `1.2·sy` | `planet.rs:196-221` |
| Boulders (3 variants) | `max(rocks/20, 4)` | 3–6 | always: `0.7·s` / `1.4·s` | `planet.rs:224-242` |
| Crystals (emissive cones) | `crystals` | 0.6–1.5 | `0.4·s` / `1.6·s` | `planet.rs:245-266` |
| Lander wrecks | `rocks/45 + 3` | 1.4–2.4 | `0.85·s` / `1.1·s` | `planet.rs:269-300` |
| Radio beacons (mast + blinking light) | `crystals/8 + 4` | 1 | radius 0.55 / height 2.4 | `planet.rs:303-343` |
| Flora | `flora` | 0.7–1.6 | not solid | `planet.rs:346-439` |

A prop collider is skipped once the player is higher than its `height` (jumping over it) (`src/planet.rs:37-39`).

### 15.4 Tiers and chains

`PlanetKind::chain_from(start, tier)` (`src/content/planets.rs:131-138`) and `max_tier` (`src/content/planets.rs:141-146`):

| Start | Tier | Chain | Stages | Stage lengths |
|---|---|---|---|---|
| Moon | 1 | Moon | 1 | 10:00 |
| Moon | 2 | Moon → Mars | 2 | 10:00, 9:00 |
| Moon | 3 | Moon → Mars → Dark Moon | 3 | 10:00, 9:00, 8:00 |
| Mars | 1 (max) | Mars | 1 | 10:00 |

- The planet-select screen offers only the **Moon and Mars** (`src/ui/menus.rs:515`). The Dark Moon is reachable only as stage 3 of Moon T3.
- **Tier N unlocks** when (planet, N−1) has been cleared (`src/ui/menus.rs:536-538`).
- **Mars is unlocked by default** (a dev setting, `src/save.rs:84`). It is also the reward of ClearMoonT2.
- **Victory** records `(chain[0], tier)` as cleared, plus `(p, 1)` for every later planet in the chain (`src/director.rs:168-174`). A Moon T2 or T3 win therefore also completes ClearMarsT1 (Yuki).
- **The tier does not scale difficulty.** It only sets the chain length and the victory silver bonus `30 × tier` (§18).

**GDD** (`GDD.md:680-696`): branching teleporter destinations and tiers T4, T5 and T∞ are not built.

### 15.5 World events

**Mars: Migrating Dust Storm** (`src/events_world.rs:14-130`), the only world event in the build:

| Property | Value |
|---|---|
| Storm radius | **24 m** (arc) |
| Active time | **26 s** |
| Gap between storms | **20 s** |
| First storm | **10 s** after first arriving on Mars (the timer starts at half a gap) |
| Drift | **3.2 m/s** along a great circle, random heading |
| Spawn location | a random point on the planet (`thread_rng`) |
| Effect | While the host's local astronaut is inside, `player_inside = true`: Spitters, UFOs and Lobbers hold fire, and Beamers cancel their aim lines (§10.4). The HUD fades in a dust haze of alpha 0.4 (`src/ui/hud.rs:532-544`). A translucent dome is drawn. |
| Limits | It tracks only the host's LocalPlayer, but silences ranged enemies against **every** astronaut (M9). The `DustStorm` resource is never reset, so a second Mars visit in one session runs storms with no dome drawn (M10). |

**Not built** (`GDD.md:639-658`):
- Moon: Earthside/Farside, lava-tube skylights, Earthrise Eclipse.
- Mars: thorn-flora slow, Dust Devils, and the storm's "~8m" vision limit (`GDD.md:648`); the built storm only fades in a HUD haze.
- Dark Moon: The Crawl, spore fungus, Whiteout.
- Meteor showers: the `meteor_showers` flag exists but nothing reads it.
- The day/night terminator sweep: the sun is fixed.

---

## 16. Interactables

Scattered once per stage by `spawn_interactables` (`src/interact.rs:135-296`). Used with **E** through `interact_system` (`src/interact.rs:394-551`).

### 16.1 Layout per stage

The layout comes from a **seeded** `StdRng((run_seed + stage) × 0x9e37)` (`src/interact.rs:147`). Each object is placed by `place_dir`, which tries up to 40 random directions for one at least a minimum arc from the drop point (`Vec3::Y`), then falls back to any direction (`src/interact.rs:124-132`).

Draw order is fixed: pots → chests → Shady Guys (stock, then position) → Greed → Magnet → Moai → Microwave → Cage → charge shrines.

| Object | Count per stage | Min arc from drop point | Source |
|---|---|---|---|
| Pot | planet's `pots` (Moon 60, Mars 68, Dark Moon 38) | 8 m | `interact.rs:161-193` |
| Chest | **7** | 12 m | `interact.rs:236-239` |
| Shady Guy | **2** (3 items each) | 15 m | `interact.rs:241-249` |
| Greed Shrine | **2** | 15 m | `interact.rs:251-254` |
| Magnet Shrine | **2** | 15 m | `interact.rs:255-258` |
| Moai | **1** | 20 m | `interact.rs:259-260` |
| Microwave | **1** | 20 m | `interact.rs:261-262` |
| Cage | **1 on the Moon** while Chimp-O is not yet freed; otherwise 0. The position is drawn **always**, so the RNG stream is the same either way. | 25 m | `interact.rs:263-272` |
| Charge shrine | **5** | 18 m | `interact.rs:283-295` |
| Teleporter | 1, only after the stage boss dies | 18 m from the party centroid (`thread_rng`) | `interact.rs:299-335` |

Pots are placed first; no pot lands within 8 m of the drop point unless all 40 tries fail. The Shady Guys' stock rolls share the placement RNG, so their stock can shift the positions of everything drawn after them (M1).

### 16.2 Using an interactable

| Rule | Value | Source |
|---|---|---|
| Range | Nearest unused interactable within **`INTERACT_RANGE` + 1.5 = 4.5 m** (straight-line to its transform) | `src/interact.rs:420-429` |
| Prompt | Shown on the HUD every frame | `src/interact.rs:444-459` |
| Key | **E** (`just_pressed`) | `src/interact.rs:461` |
| When | Only in `Playing` | `src/interact.rs:412-415` |
| Who | Only the machine's `LocalPlayer`, and only on the simulating machine (solo or host) | `src/main.rs:235` |

### 16.3 Chest

| Property | Value |
|---|---|
| Prompt | `[E] Open chest (N gold)` |
| Price formula | `round(25 × 1.75^chest_opens × (1 − chest_discount))` (`src/interact.rs:440-442`; `src/config.rs:48-49`) |
| Price today | **25 gold** (see the bug below). With Space Credit Card: 23 / 20 / 18 / 15 at 1–4 stacks. |
| Not enough gold | Banner "NOT ENOUGH GOLD"; no item is rolled |
| Enough gold | The item is rolled **once** with `roll_item(sheet, luck)` (§9.1) and **remembered**. Leaving and reopening shows the same item. The reveal panel pauses the world. (`src/interact.rs:466-475`) |
| Panel: TAKE (1 or E) | Pays the price shown when the panel opened, adds the item (the stack cap is **not** checked), recomputes stats and marks the chest used (`src/ui/panels.rs:341-361`) |
| Panel: LEAVE (2 or Esc) | Nothing happens; the chest stays usable |

**Bug H2:** taking a chest never increments `chest_opens` or `chests_opened`. As a result:
- the price **never rises**, so `CHEST_COST_GROWTH` is dead;
- the **Chests10** quest can never complete;
- Rocket Pod never enters the level-up pool.

The intended price ladder, with no discount, would be 25 → 44 → 77 → 134 → 234 gold.

**Design documents:**
- `DESIGN.md` (not the GDD) lists "locked/golden chests" (`DESIGN.md:73`); neither is built. Its "pay *after* seeing the item" rule (`DESIGN.md:45`, `153`) **is** how the built chest works: the item is revealed first and gold is spent only on TAKE, although opening still requires holding at least the price.
- The GDD's Mimic Chest enemy (`GDD.md:725`) is not built.
- The co-op "contested-but-generous" rule (a card for everyone, with the opener at +1 luck) is not built (`GDD.md:902`).

### 16.4 The Shady Guy (vendor)

| Property | Value | Source |
|---|---|---|
| Count | 2 per stage | `src/interact.rs:241` |
| Stock | **3 items** each, rolled **at stage entry** with `roll_item(sheet, sheet.luck)`. The stock can contain duplicates. | `src/interact.rs:242-246` |
| Sheet used for the roll | Stage 1: a **fresh** `PlayerState` for the hero (tomes and passive only) (`src/main.rs:442`). Later stages: the carried local sheet (`src/director.rs:212-216`). | — |
| Price | `round(base × (1 − chest_discount))`, frozen at stage entry | `src/interact.rs:113-121`, `245` |
| Buy | Keys 1–3 or click. Needs gold ≥ price. Adds the item (stack cap **not** checked). The offer is marked SOLD on the vendor, which stays usable for its other offers. | `src/ui/panels.rs:449-484` |
| Close | E, Esc or "[E] WALK AWAY" | `src/ui/panels.rs:486-496` |

Base prices, and prices with Space Credit Card:

| Rarity | Base price | 1 card (−10%) | 2 (−20%) | 3 (−30%) | 4 (−40%) |
|---|---|---|---|---|---|
| Common | 30 | 27 | 24 | 21 | 18 |
| Rare | 60 | 54 | 48 | 42 | 36 |
| Epic | 120 | 108 | 96 | 84 | 72 |
| Legendary | 240 | 216 | 192 | 168 | 144 |

**GDD** (`GDD.md:575-579`) says there are "1–2 per stage" and "his hat colour = the Rarity Grade". The code always has 2 vendors, and they have no hat colour.

### 16.5 Greed Shrine

| Property | Value | Source |
|---|---|---|
| Count | 2 per stage; one use each | `src/interact.rs:251-254` |
| Effect | `greed_stacks += 1` (run-global, carried across stages). Every recompute then adds **+0.12 Difficulty and +0.08 Luck per stack**. The activator's sheet is recomputed at once, and `run.difficulty` is updated. | `src/interact.rs:481-489`; `src/run.rs:267-268` |
| Banner | "GREED: +12% DIFFICULTY, +8% LUCK" | — |

`greed_stacks` is not sent to co-op clients, so a joiner's sheet never includes it.

### 16.6 Magnet Shrine

| Property | Value | Source |
|---|---|---|
| Count | 2 per stage; one use each | `src/interact.rs:255-258` |
| Effect | **Every pickup on the planet** starts flying to the activating astronaut at speed 14, then accelerates as usual (§17.2) | `src/interact.rs:490-499` |
| Banner | "THE PLANET GIVES" | — |

### 16.7 Moai

| Property | Value | Source |
|---|---|---|
| Count | 1 per stage; one use | `src/interact.rs:259-260` |
| Effect | Opens "THE MOAI SPEAKS": **3 item cards** rolled with `roll_item(sheet, luck + 0.15)`. The same item can appear twice. **One must be picked**: there is no skip, and Esc does nothing in a Modal. | `src/interact.rs:500-514` |

### 16.8 Microwave

| Property | Value | Source |
|---|---|---|
| Count | 1 per stage; **once per stage** (`microwave_used` resets on stage change) | `src/interact.rs:515-535`; `src/director.rs:204` |
| Requirement | At least one item | — |
| Effect | "MICROWAVE: DUPLICATE": up to **3** of your owned items that are below max stacks, chosen at random. Pick one to add **+1 stack**. No gold is charged. | — |
| Nothing eligible | Banner "NOTHING FITS IN THE MICROWAVE", **but the Microwave is still consumed** (low-severity bug) | `src/interact.rs:519-531` |

**GDD** (`GDD.md:581-586`) specifies "Insert one item + Gold → duplicate it at one grade lower, or gamble to upgrade it one grade". The code duplicates a stack for free, with no gamble.

### 16.9 Cage (Chimp-O)

| Property | Value | Source |
|---|---|---|
| Where | The Moon only, and only while `counters.chimp_freed` is false | `src/interact.rs:269-272` |
| Effect | Sets `save.counters.chimp_freed = true` in memory, shows "THE CAGE IS OPEN. HE REMEMBERS." with a gold burst, and marks the cage used | `src/interact.rs:536-544` |
| Persistence | Written to disk at the next `MetaSave::save()`: results banking, the pause-menu settings, or a tome button. At banking, the **FreeChimp** quest completes and grants Chimp-O and +40 silver. | `src/save.rs:199`; `src/content/quests.rs:76` |

### 16.10 Charge shrines

| Property | Value | Source |
|---|---|---|
| Count | 5 per stage; each completes once | `src/interact.rs:283-295` |
| Ring | Cyan torus, inner/outer radius 3.6/3.9 m. An astronaut is **inside** when within **4.2 m** (straight-line) of the centre. | `src/interact.rs:275`, `362` |
| Fill | `progress += dt / 8 × (astronauts inside)`: **8 s** solo, 4 s with two players | `src/interact.rs:363-365` |
| Decay | Empty rings lose `dt / 16` per second (16 s from full to empty) | `src/interact.rs:366` |
| On completion | `shrines_charged += 1`, "SHRINE CHARGED" banner, and "SHRINE BLESSING": **3 item cards** rolled with `roll_item(sheet, luck + 0.3)` for the host's local astronaut. One must be picked. | `src/interact.rs:370-386` |

The **Shrines5** quest counts completed charge rings only; Greed and Magnet shrines do not count.

### 16.11 Pots

| Property | Value | Source |
|---|---|---|
| What | A pot is an `Enemy` with HP 1, speed 0 and `Pot{broken}`. It sits in the spatial hash and **counts toward the enemy cap**. | `src/interact.rs:161-193` |
| Breaking | **Any** hit breaks it: melee, aura, projectile, rocket, drone or beam. Auto-aim, Chain, homing and the Comet ignore pots. | `src/combat.rs:904-921` |
| "Silverish" pots | 10% are drawn with a blue-silver glowing material. This is **visual only**; the loot is the same. | `src/interact.rs:163`, `188` |
| Loot | 12%: 1 Silver pile of **1–2**. 12%: 1 **Food**. 76%: **1–3 gold piles** of **2–6 gold** each. | `src/pickups.rs:308-325` |
| Counter | `pots_broken += 1` (quest Pots50) | `src/pickups.rs:309` |

### 16.12 Teleporter

| Property | Value | Source |
|---|---|---|
| Opens | On the `run_clock` tick after the **stage boss** dies, if The Static is not active. Banner "TELEPORTER ONLINE — OR STAY AND FARM". | `src/director.rs:99-103` |
| Position | **18 m** (arc) from the party centroid in a random direction (`thread_rng`). A torus with a 60 m tall light pillar. | `src/interact.rs:299-335` |
| Use | E → the next stage of the chain, or **Victory** after the last stage | `src/interact.rs:545-549`; `src/director.rs:162-177` |
| Staying | Nothing forces you to leave. The clock runs on to The Static, and the teleporter stays usable during The Static. | — |

---

## 17. Pickups, powerups and drop tables

Source: `src/pickups.rs`.

### 17.1 Pickup kinds

| Pickup | Look | Effect on collection | Source |
|---|---|---|---|
| **XP gem** `Xp(v)` | Green sphere, r = 0.22. When `v ≥ 10`: blue, ×1.6 size. | **Shared:** every living astronaut gains `v × their own xp_gain` | `src/pickups.rs:88-92`, `207-221` |
| **Gold** `Gold(g)` | Gold coin | The **collector only** gains `round(g × gold_gain)`; this also adds to `run.gold_collected` | `src/pickups.rs:254-261` |
| **Silver** `Silver(s)` | Silver-blue coin, ×1.1 | `round(s × silver_gain)` is added to `run.silver_run`, which is banked at results | `src/pickups.rs:262-268` |
| **Food** | Red cube | The collector heals **20% of max HP** (green heal number) | `src/pickups.rs:269-276` |
| **Powerup** `Powerup(k)` | Purple glowing sphere, r = 0.3 | The collector gets a timed buff (§17.3) | `src/pickups.rs:277-291` |

Each drop is scattered **0–1.2 m** from the death point (`src/pickups.rs:81-85`) and bobs on the ground until attracted.

### 17.2 Attraction and collection

| Rule | Value | Source |
|---|---|---|
| Attraction radius | `3.2 × pickup_range` (straight-line); ×**40** with the Magnet powerup | `src/run.rs:339-346`; `src/config.rs:37` |
| Target | The nearest living astronaut in range. The target is **latched** until collection. | `src/pickups.rs:157-177` |
| Flight speed | Starts at **6**, accelerates **+60/s**, capped at `PICKUP_FLY_SPEED × 1.8` = **46.8 m/s** | `src/pickups.rs:173-178`; `src/config.rs:38` |
| Collected | Within **0.8 m** | `src/pickups.rs:181` |
| Downed astronauts | Neither attract nor receive XP | `src/pickups.rs:142`, `210` |
| **Gem cap** | Once per second, if more than **`GEM_CAP` = 550** XP gems exist, the first `count − 550 + 40` gems in ECS query order (roughly the oldest; the code comment says "oldest", but no age is tracked) are merged into **one gem** carrying their total XP, dropped near their centroid | `src/pickups.rs:375-404`; `src/config.rs:36`; `src/main.rs:255-262` |
| Gold, silver and food piles | Uncapped | — |

### 17.3 Powerups

| Powerup | Banner | Duration | Effect | Source |
|---|---|---|---|---|
| **Damage2x** | "2x DAMAGE!" | **20 s** | Damage multiplier ×2 | `src/pickups.rs:279`; `src/run.rs:294-296` |
| **Magnet** | "MEGA MAGNET!" | **12 s** | Pickup range ×40 (128 m at base range) | `src/pickups.rs:280`; `src/run.rs:341-343` |
| **Speed** | "SPEED BOOST!" | **15 s** | Move speed ×1.5 | `src/pickups.rs:281`; `src/run.rs:333-335` |

Picking up a powerup you already have **resets** its timer; it does not stack (`src/pickups.rs:283-284`). Timers tick in `player_upkeep` on virtual time, so they pause with the game (`src/player.rs:864-873`).

### 17.4 Drop tables

All loot rolls use `thread_rng`, so they are **not** reproducible from the run seed. Source: `kill_drops`, `src/pickups.rs:296-372`.

| Killed | XP gem | Gold | Food | Powerup | Silver | Extra |
|---|---|---|---|---|---|---|
| **Normal enemy** | 1 gem of `xp` | 7%: 1 pile of 1–3 | 1.2% | 0.6% (uniform kind) | — | — |
| **Elite** | 1 gem of `xp × 8` | **4–7 piles of 4–9** | — | 35% (uniform kind) | — | — |
| **Miniboss** | 1 gem of 50 | 4–7 piles of 4–9 | — | 35% | — | — |
| **Stage boss** | 1 gem of 50 | 4–7 piles of 4–9, **plus 14 piles of 8–19** | — | 35% | — | Boss roar; 40-particle burst |
| **Anything, during The Static** | **none** | as its row above | as above | as above | **50%: 1 silver** | — |
| **Pot** | — | 76%: 1–3 piles of 2–6 | 12% | — | 12%: 1 pile of 1–2 | `pots_broken += 1` |

Ranges are inclusive: Rust `gen_range(a..b)` excludes `b`, and the values above already account for that.

**Expected gold per kill** (*derived*, gold_gain 1):
- normal enemy: 0.07 × 2 = **0.14**;
- elite: 5.5 × 6.5 ≈ **35.75**;
- stage boss: 35.75 + 14 × 13.5 ≈ **224.75**;
- pot: 0.76 × 2 × 4 ≈ **6.1**.

---

## 18. Economy: gold, silver and the results payout

### 18.1 Gold (in-run currency, per astronaut)

| Sources | Sinks |
|---|---|
| Kill and pot drops (§17.4) × `gold_gain` | **Chests**: 25 gold today (§16.3) |
| Level-up **gold pile** card: 15–44, flat (`src/run.rs:574`, `624`) | **Shady Guy**: 30 / 60 / 120 / 240 by rarity, before discount (§16.4) |
| Level-up **Skip**: +10, flat (`src/ui/panels.rs:209`) | |

Gold is per astronaut (`PlayerState.gold`, `src/run.rs:145`) and is **carried between stages**. It is **not** banked: the run's gold is lost at results. Only `gold_collected`, which counts pickups after `gold_gain`, feeds the lifetime `Gold5000` quest.

### 18.2 Silver (meta currency, in the save)

| Sources | Sinks |
|---|---|
| Silver pickups during the run: pots 12% × 1–2, Static kills 50% × 1 (× `silver_gain`) → `silver_run` | **Tomes** (§6.2) |
| Comet cash-out: **+peak tail count** (§20), added straight to `silver_run` | — |
| The **results payout** below | — |
| **Quest rewards** (§19), added to the save when quests complete at results | — |

### 18.3 Results payout formula

`bank_results` (`src/director.rs:272-346`) runs on entering Results, on the simulating machine only:

```
performance = floor(kills / 40) + level + (victory ? 30 × tier : 0)
payout      = silver_run + floor(performance × silver_gain)
save.silver += payout
```

- The results screen shows `SILVER EARNED: +payout` (`src/ui/menus.rs:627`).
- Quest silver is added separately, and the results screen lists it as "QUEST COMPLETE: …".
- *Example:* 1,525 kills, level 25, death: `38 + 25 = 63` performance, plus whatever silver was picked up.

**Bug H1:** `level` and `silver_gain` are read from the local `PlayerState`, but that entity has already been despawned when Results opens. So `level` falls back to **1** and `silver_gain` to 1.0 (`src/director.rs:280`, `298`). The live payout is therefore `silver_run + floor(kills/40) + 1 + victory bonus`, and the results screen shows LEVEL 1 and GOLD 0.

Victory bonus by tier: **T1 +30, T2 +60, T3 +90** (before `silver_gain`).

Counters banked on the same pass (`src/director.rs:283-294`): kills, pots, chests, shrines, gold, evolves, `best_level`, `static_secs_best`, `runs_started` (+1) and `runs_won` (+1 on victory).

**GDD** (`GDD.md:788-799`) gives `Silver = survival_seconds/6 + kills/4 + boss_kills × 150 + tier_bonus × 50 + Static_overtime_seconds, × (1 + GoldenTome × 0.05) × (1 + CursedMoonRock × 0.15)`. Only two GDD terms have a code counterpart, and both are weighted differently: kills (divisor **40**, not 4) and the tier bonus (**30 × tier, on victory only**, not `tier_bonus × 50`). Survival time, boss kills, Static overtime and the Golden Tome / Cursed Moon Rock multipliers are not in the code; the code adds the hero level and picked-up silver instead. The branching "Unlock Web" is not built.

### 18.4 Where currency lives

| Value | Scope | Persisted? | Source |
|---|---|---|---|
| `PlayerState.gold` | Per astronaut, per run | No | `src/run.rs:145` |
| `RunState.silver_run` | Per run | Banked at results | `src/run.rs:109` |
| `MetaSave.silver` | Lifetime | `%APPDATA%/astrobonk/save.json` | `src/save.rs:34`, `107-112` |

---

## 19. Quests (16)

Defined in `src/content/quests.rs:63-83`. Conditions are checked **only in `bank_results`**, against the lifetime `Counters` (`src/save.rs:168-206`). Rewards are applied at once (`src/save.rs:208-222`). The Quests tab shows `(a/b)` progress for the counter quests (`src/save.rs:225-239`).

| Quest | Name | Condition (code) | Reward | Status | Source |
|---|---|---|---|---|---|
| Kill100 | First Contact | lifetime kills ≥ 100 | 20 silver | works | `quests.rs:66` |
| Kill1000 | Pest Control | kills ≥ 1,000 | 60 silver + **unlock Tesla Coil** | works | `quests.rs:67` |
| Kill2500 | Exterminator | kills ≥ 2,500 | 90 silver + **unlock Doug** | works | `quests.rs:68` |
| Kill10000 | Solar Defender | kills ≥ 10,000 | 250 silver + **+1 tome slot** | works | `quests.rs:69` |
| Pots50 | Pottery Critic | lifetime pots ≥ 50 | 40 silver + **unlock Orbital Drones** | works | `quests.rs:70` |
| Chests10 | Cache Money | lifetime chests ≥ 10 | 50 silver + **unlock Rocket Pod** | **impossible** (bug H2: the counter never increments) | `quests.rs:71` |
| Shrines5 | Devout | lifetime **charge** shrines ≥ 5 | 50 silver + **unlock Cryo Vent** | works | `quests.rs:72` |
| Gold5000 | Space Capitalist | lifetime gold collected ≥ 5,000 | 80 silver | works | `quests.rs:73` |
| Level20 | Overachiever | `best_level ≥ 20` | 60 silver | **impossible** (bug H1: `best_level` is banked as 1) | `quests.rs:74` |
| EvolveWeapon | Ascension | lifetime evolves ≥ 1 | 100 silver | **impossible** (bug H3: `run.evolves` never increments) | `quests.rs:75` |
| FreeChimp | Cold Case | `chimp_freed` | 40 silver + **unlock Chimp-O** | works | `quests.rs:76` |
| SurviveStatic2Min | Signal In The Noise | `static_secs_best ≥ 120` (last stage of a run) | 150 silver + **+1 tome slot** | works | `quests.rs:77` |
| ClearMoonT1 | One Small Bonk | cleared (Moon, 1) | 50 silver + **unlock B0-NK** | works | `quests.rs:78` |
| ClearMoonT2 | One Giant Bonk | cleared (Moon, 2) | 120 silver + **unlock Mars** | works (Mars is already unlocked by default) | `quests.rs:79` |
| ClearMoonT3 | The Full Tour | cleared (Moon, 3) | 300 silver | works | `quests.rs:80` |
| ClearMarsT1 | Red Planet Standing | cleared (Mars, 1); also set by Moon T2 and T3 wins | 120 silver + **unlock Yuki** | works | `quests.rs:81` |

- **Total quest silver:** 1,580.
- **Tome slots:** at most +2 from quests, so 5 total (the cap).
- **Kills** count every non-pot death: normal enemies, elites, Static ghosts and bosses (`src/pickups.rs:327`).
- **Co-op:** a co-op **joiner** banks nothing (`src/main.rs:148-151`).

**GDD** (`GDD.md:801-821`) targets about 50 launch quests in six categories. Its example quests ("One Small Step", "Around The World", …) are not the built ones.

---

## 20. The Comet Combo

The scored lap-kill. Source: `src/comet.rs:33-127`; constants at `src/config.rs:58-63`. It runs only for the simulating machine's `LocalPlayer`: solo, or the host (`src/main.rs:249-254`).

| Step | Rule |
|---|---|
| **Tail count** | Every frame, count the enemies within **`WAKE_RADIUS` = 14 m** (straight-line) of the astronaut that are moving (speed > 0, so pots are excluded), at least 0.15 m away, and **not ahead**: `dot(to_enemy, move_dir) < 0.35`, which is more than about 69.5° off the heading. With no movement every nearby enemy counts. (`src/comet.rs:61-73`) |
| **Charging** | While `count ≥ COMET_MIN_TAIL` (**8**) **and** speed > **2.0 m/s**: `charge += count × speed × dt`, and `peak = max(peak, count)`. Audio ticks sound at 25%, 50% and 75%. (`src/comet.rs:75-92`) |
| **Cash-out** | When `charge ≥ COMET_CHARGE_GOAL` (**900**). (`src/comet.rs:94-119`) |
| — damage | Every moving enemy returned by the spatial hash for radius **`COMET_RADIUS` = 22 m** takes **`300 + 18 × peak`** damage. It is flagged as a crit for display, but there is no crit multiplier. Knockback is `12 × knockback` radially. `source = None`, so there is no lifesteal and no cryo. |
| — rewards | **`silver_run += peak`**; screen shake 0.8, 0.22 s of hitstop, banner "☄ COMET xN!", comet SFX, a 44-particle burst |
| — reset | The combo resets; the lifetime `fires` count goes +1 |
| **Break** | If the charging condition fails for more than **`COMET_GRACE` = 0.7 s**, the combo resets with **no payout** (`src/comet.rs:120-126`) |
| **HUD** | `☄ xN` plus a 12-segment charge bar, amber warming to white; "☄ COMET!" flashes for 0.8 s after a cash-out (`src/ui/hud.rs:547-562`) |

*Derived* examples:

| Tail | Speed | Charge rate | Time to fill | Cash-out damage | Silver |
|---|---|---|---|---|---|
| 8 (minimum) | 8.5 m/s (run) | 68/s | 13.2 s | 444 per enemy | +8 |
| 20 | 12 m/s | 240/s | 3.75 s | 660 per enemy | +20 |

**Area caveat:** the cash-out does **no exact distance test**. It hits everything in the hash's cell cube, `(2·⌈22/2.2⌉ + 1)³ = 21³` cells of 2.2 m. That reaches at least 22 m along each axis and up to about 40 m toward the cube's corners (`src/enemies.rs:190-204`; `src/comet.rs:97-98`).

**GDD** (`GDD.md:206-213`) requires completing "a full great-circle with the tail intact" and an auto-flagged highlight clip. The code has **no lap requirement** (charge is tail × speed × time) and no clip flagging.

---

## 21. The daily seed

| Piece | Rule | Source |
|---|---|---|
| Day number | `unix_seconds / 86,400` (the UTC day) | `src/run.rs:46-51` |
| Seed | splitmix64 of the day: `z = day + 0x9e3779b97f4a7c15`; `z = (z ^ z>>30)·0xbf58476d1ce4e5b9`; `z = (z ^ z>>27)·0x94d049bb133111eb`; `seed = z ^ z>>31` | `src/run.rs:54-60` |
| World name | `WORDS[seed % 8]` + `-` + hex digit `(seed>>8)%16` + hex digit `(seed>>3)%16`, e.g. "GRIEF-7B". `WORDS` = GRIEF, HUSH, EMBER, VIGIL, DROSS, WANE, SILT, PALL. | `src/run.rs:63-66` |
| Mode | The **DAILY** button forces **Moon, Tier 1**. The player still picks any unlocked hero, and the run starts right after the hero pick. | `src/ui/menus.rs:291-298`, `486-492` |
| What the seed fixes | The props (`spawn_stage`), the interactable layout and Shady Guy stock (up to M1), and the **director's random stream** (`GameRng`; the stream, not a frame-exact horde, §11.1). Terrain is the planet's fixed seed. | `src/planet.rs:152`; `src/interact.rs:147`; `src/main.rs:430-431`; `src/director.rs:206-207` |
| What it does **not** fix | Loot drops, level-up cards, chest items, shrine and Moai rolls, crits, the teleporter and boss positions, and dust storms. All of these use `thread_rng`. | — |
| Score | The run's **silver payout** (§18.3). `daily_best` is kept for the current day only; it resets when the day number changes. "NEW DAILY BEST!" shows when the payout beats it. | `src/director.rs:302-315`; `src/ui/menus.rs:628-637` |
| Tutorial | Suppressed on daily runs. A daily run does not mark the tutorial done. | `src/main.rs:423`; `src/director.rs:318-320` |
| Normal runs | Seeded from the wall clock in nanoseconds (`fresh_seed`) | `src/run.rs:38-43` |

**GDD** (`GDD.md:830-835`) adds three lives, a leaderboard and ghost replays of the top 3. None of these are built.

---

## 22. Player movement and combat-feel numbers

These are not "content" in the narrow sense, but several stats (Move Speed, Jump Height, Extra Jumps) and two hero passives (Nova, Aurora) depend on them.

### 22.1 Movement

Source: `player_input` (`src/player.rs:418-520`) and `player_physics` (`src/player.rs:523-589`).

| Rule | Value |
|---|---|
| Run speed | `8.5 × move_speed_mult` m/s |
| Ground acceleration | 55 m/s² along the input direction |
| Air acceleration | 55 × 0.35 = **19.25 m/s²** (`PLAYER_AIR_CONTROL`) |
| Friction | 38 m/s² deceleration, applied only when there is no input, the astronaut is grounded and not sliding, and it is more than 0.16 s since landing |
| Speed clamp, normal | When grounded, not sliding and outside the bhop window: **1.0× run speed** |
| Speed clamp, otherwise | Airborne, sliding or inside the bhop window: `8.5 × mult × 2.1` = **17.85 m/s** at mult 1 (`SPEED_HARD_CAP`) |
| Jump | Radial velocity `8.0 × √jump_height`, gravity 22 m/s². *Derived:* apex **1.45 m** after 0.36 s; airtime **0.73 s** at base. |
| Jump count | Max jumps = `1 + extra_jumps`. A ground jump counts as jump 1; air jumps use the rest. |
| Slide | **Left Ctrl or C**, only when grounded and the slide cooldown is ready. Lasts **0.85 s**, cooldown **1.1 s**. Velocity is set to `max(current, 8.5 × mult × 1.65 = 14.0 m/s)` along the input direction, or the facing if there is no input. It triggers Yuki's frenzy. |
| Bunny-hop | Jumping within **0.16 s** of landing, while faster than 1.05× run speed, keeps the speed (the hard cap applies in the window) and plays the bhop SFX |
| `fast_move` | Speed > 1.08× run speed; drives Nova and Aurora (§3.2) |
| Coyote time | `coyote = 0.12` s is set on landing, but it is effectively dead logic (low-severity note) |
| Props | The player collides with rock, boulder, crystal, wreck and beacon cylinders (§15.3). There is no fall damage. |
| Knockback on the player | **None.** Enemy hits never move the astronaut. |

*Derived:* holding W while hopping or sliding reaches the 2.1× hard cap. Whether this is intended is an open question.

**GDD** (`GDD.md:314-327`):
- The GDD slides with Shift; the code uses **Ctrl or C**. Tutorial line 4 also says "Shift" (`src/tutorial.rs:24`).
- The GDD has about 60% air control; the code has **35%**.
- The GDD has a "floaty ~1.2 s hang"; the code has **0.73 s**.
- The GDD's slope-boost and slide-knockback are not built.

### 22.2 Camera

Source: `camera_rig` (`src/player.rs:747-824`).

| Property | Value |
|---|---|
| Distance | 7.5 m (`CAM_DISTANCE`) |
| Height term | `3.2 × 0.4` (`CAM_HEIGHT`) |
| Stiffness | 14 (exponential chase) |
| Mouse sensitivity | 0.0032 rad per count × the settings multiplier |
| Pitch | Clamped to 0.12–1.25 rad |
| FOV | 45°; ×1.09 while sliding |
| Screenshake | Positional only, with amplitude `trauma² × shake_scale`. Trauma decays at 1.6/s (`src/fx.rs:18-20`). |

Shake sources:

| Event | Trauma |
|---|---|
| Hurt | 0.12 |
| Crit kill | 0.06 |
| Telegraph detonation | 0.22 |
| Boss phase | 0.55 |
| Boss death | 0.8 |
| Comet | 0.8 |

### 22.3 Controls that touch content

| Key | Action | Source |
|---|---|---|
| WASD | Move (camera-relative) | `src/player.rs:396-409` |
| Space | Jump | `src/player.rs:411` |
| Left Ctrl / C | Slide | `src/player.rs:412` |
| E | Interact; also TAKE in the chest panel; close the shop | `src/interact.rs:461`; `src/ui/panels.rs:328`, `486` |
| 1–4 / 1–3 | Pick a card (1–4) / buy stock (1–3) | `src/ui/panels.rs:160-164`, `450-454` |
| R / B / S | Refresh / Banish / Skip on level-up panels | `src/ui/panels.rs:171-188` |
| Esc | Pause / resume; leave a chest; close the shop | `src/ui/panels.rs:525-527`, `334`, `486` |
| **B** (while playing) | **DEV:** summon the stage boss | `src/enemies.rs:913-936` |
| **T** | **DEV:** replay the tutorial | `src/tutorial.rs:42-45` |

### 22.4 Mission Control tutorial lines

The tutorial shows on a player's first non-daily run only. Each line holds for 5 s. Source: `src/tutorial.rs:20-27`, triggers at `72-80`.

| # | Trigger | Line |
|---|---|---|
| 1 | e > 1.2 s | "Welcome to the rock. WASD to move, mouse to look. Walk it off." |
| 2 | e > 6 s or any enemy exists | "Your weapon fires itself. Your only job is not getting cornered." |
| 3 | e > 14 s | "Green gems are XP. Fill the bar, pick a card. Trust your gut." |
| 4 | Level ≥ 2 | "Shift to slide. Jump as you land to keep the speed — you'll need it." (**wrong key**: slide is Ctrl or C) |
| 5 | e > 42 s | "Run the horizon; the far side goes dark. Your light rides your gun." |
| 6 | Miniboss #1 has spawned | "Big one inbound. Kite it, don't trade. It'll drop a chest." (**no chest drops**) |

---

## 23. All `config.rs` constants

Every constant in `src/config.rs:3-99`, with where it is read. "Read at" lists code lines only; lines that merely import the constant (`use`) or name it in a comment are left out.

### 23.1 Player movement

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `PLAYER_RUN_SPEED` | 8.5 | Base run speed (m/s) | `player.rs:432`, `449`, `463`, `494`, `540`, `632`; `headless.rs:105` |
| `PLAYER_ACCEL` | 55.0 | Ground acceleration (m/s²) | `player.rs:436` |
| `PLAYER_FRICTION` | 38.0 | No-input deceleration (m/s²) | `player.rs:440` |
| `PLAYER_AIR_CONTROL` | 0.35 | Acceleration multiplier while airborne | `player.rs:434` |
| `PLAYER_JUMP_VEL` | 8.0 | Jump radial velocity (× √jump_height) | `player.rs:478` |
| `PLAYER_GRAVITY` | 22.0 | Radial gravity (m/s²) | `player.rs:543` |
| `PLAYER_HEIGHT` | 1.7 | Astronaut height; the body centre is at half of it | `player.rs:140`, `584`; `remote.rs:89`, `165` |
| `PLAYER_RADIUS` | 0.45 | Collision radius, used for props and for enemy contact reach | `player.rs:554`; `enemies.rs:1009`, `1259` |
| `SLIDE_BOOST` | 1.65 | Slide speed × run speed | `player.rs:432`, `494` |
| `SLIDE_TIME` | 0.85 | Slide duration (s) | `player.rs:491` |
| `SLIDE_COOLDOWN` | 1.1 | Slide cooldown (s) | `player.rs:492` |
| `BHOP_WINDOW` | 0.16 | Post-landing window that keeps speed (s) | `player.rs:437`, `446`, `462` |
| `SPEED_HARD_CAP` | 2.1 | Absolute speed cap × run speed | `player.rs:449` |

### 23.2 Camera

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `CAM_DISTANCE` | 7.5 | Camera boom length (m) | `player.rs:791` |
| `CAM_HEIGHT` | 3.2 | Camera lift term (× 0.4) | `player.rs:793` |
| `CAM_STIFFNESS` | 14.0 | Chase rate | `player.rs:806` |
| `CAM_SENS` | 0.0032 | Mouse sensitivity (rad per count) | `player.rs:783` |

### 23.3 Co-op teammate rendering

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `REMOTE_SMOOTH_RATE` | 14.0 | How hard a teammate's drawn pose chases its replicated snapshot | `remote.rs:126` |
| `REMOTE_SNAP_ARC` | 8.0 | Arc error (m) past which a teammate teleports instead of easing | `remote.rs:137` |

### 23.4 Horde

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `ENEMY_CAP` | 1200 | Live `Enemy` entities per party-scale unit (pots and bosses count) | `enemies.rs:592`; `headless.rs:167` (the smoke-test cap check at `ENEMY_CAP + 400`) |
| `ENEMY_SEPARATION_CELL` | 2.2 | Spatial-hash cell size (m); also the separation query radius | `enemies.rs:187`, `191`, `1091` |
| `SPAWN_ARC_MIN` | 42.0 | Minimum spawn arc from an anchor (m) | `enemies.rs:602`, `785` |
| `SPAWN_ARC_MAX` | 58.0 | Maximum spawn arc (m) | `enemies.rs:602`, `785` |
| `CONTACT_TICK` | 0.5 | Seconds between contact hits from one enemy | `enemies.rs:1147`, `1262` |

### 23.5 Pickups

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `GEM_CAP` | 550 | XP gem count above which gems are merged (§17.2) | `pickups.rs:388`, `391` |
| `PICKUP_BASE_RANGE` | 3.2 | Base attraction radius (m) | `run.rs:340` |
| `PICKUP_FLY_SPEED` | 26.0 | Flight-speed base (the cap is × 1.8) | `pickups.rs:178`; `netenemy.rs:752` |

### 23.6 Stage clock and XP

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `STAGE_SECONDS` | [600.0, 540.0, 480.0] | Stage lengths by stage index | `director.rs:196`; `run.rs:169` |
| `MINIBOSS_MARKS` | [420.0, 120.0] | Miniboss timer marks | `director.rs:75` (the `--bossnow` dev path at `main.rs:516` sets both marks done without reading the constant) |
| `BOSS_MARK` | 90.0 | Stage-boss timer mark | `director.rs:87`; `main.rs:513` |
| `XP_BASE` | 6.0 | XP curve constant term | `run.rs:389` |
| `XP_PER_LEVEL` | 3.4 | XP curve linear term | `run.rs:389` |
| `XP_QUAD` | 0.18 | XP curve quadratic term | `run.rs:389` |

### 23.7 Economy, build limits, UI and the Comet

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `CHEST_BASE_COST` | 25 | First chest price (gold) | `interact.rs:440` |
| `CHEST_COST_GROWTH` | 1.75 | Price × per chest opened. **Dead** (H2). | `interact.rs:440` |
| `WEAPON_SLOTS` | 4 | Maximum weapons held | `run.rs:530` |
| `MAX_WEAPON_LEVEL` | 7 | Weapon level cap and evolution threshold | `run.rs:374`, `457`, `524`, `605` |
| `DAMAGE_NUMBER_POOL` | 64 | Pooled floating damage numbers | `ui/numbers.rs:28` |
| `INTERACT_RANGE` | 3.0 | Interact radius (+1.5 slack = 4.5 m in use) | `interact.rs:426` |
| `WAKE_RADIUS` | 14.0 | Comet tail radius (m) | `comet.rs:62`, `69` |
| `COMET_MIN_TAIL` | 8 | Tail needed to charge | `comet.rs:75` |
| `COMET_CHARGE_GOAL` | 900.0 | Charge to cash out (Σ tail × speed × dt) | `comet.rs:28`, `94` |
| `COMET_RADIUS` | 22.0 | Cash-out query radius (m; cell cube, see §20) | `comet.rs:97` |
| `COMET_GRACE` | 0.7 | Seconds the tail may fail before the combo breaks | `comet.rs:122` |

### 23.8 Save

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `SAVE_DIR` | "astrobonk" | Folder under `%APPDATA%` (saves and session logs) | `save.rs:111`; `playlog.rs:37` |
| `SAVE_FILE` | "save.json" | Save file name | `save.rs:111` |

### 23.9 Co-op enemy streaming

| Constant | Value | Meaning | Read at |
|---|---|---|---|
| `NET_ENEMY_RANGE` | 128.0 | Half-width of the quantized position range (arc m); 16 bits gives a 3.9 mm step | `netenemy.rs:272`, `275` |
| `NET_ENEMY_HZ` | 15.0 | Crowd snapshot rate | `netenemy.rs:292` |
| `NET_ENEMY_NEAR_ARC` | 50.0 | Inside this arc, an enemy is sent every snapshot | `netenemy.rs:359` |
| `NET_ENEMY_INTEREST_IN` | [95.0, 110.0, 82.0] | Interest-enter arc per planet (Moon, Mars, Dark Moon) | `netenemy.rs:301`, `1318` |
| `NET_ENEMY_INTEREST_OUT` | [107.0, 122.0, 93.0] | Interest-leave arc per planet (hysteresis) | `netenemy.rs:302` |
| `NET_ENEMY_MAX_RECORDS` | 1200 | Records per snapshot. Called a "hard ceiling", but it is not enforced for spawn or near-band records. | `netenemy.rs:375`, `381` |
| `NET_ENEMY_CHUNK_BYTES` | 1024 | Payload bytes per chunk (under renet's 1200 slice) | `netenemy.rs:418`, `423`, `428` |
| `NET_ENEMY_SMOOTH_RATE` | 12.0 | Proxy interpolation rate | `netenemy.rs:1012`, `1221` |
| `NET_ENEMY_SNAP_ARC` | 12.0 | Arc error past which a proxy teleports | `netenemy.rs:1015`, `1246` |
| `NET_ENEMY_GRACE` | 2.0 | Seconds an unseen proxy survives before despawning | `netenemy.rs:85`, `1238` |

---

## 24. Other hard-coded constants

These tuning numbers live outside `config.rs`. The inline magic numbers in weapons, enemies, bosses and interactables are catalogued in their own sections above.

| Constant | Value | Meaning | Source |
|---|---|---|---|
| `EliteMods::HP / DMG / SCALE / XP` | 8.0 / 1.8 / 1.65 / 8.0 | Elite multipliers | `src/content/enemies.rs:231-234` |
| `WORM_SEGMENTS` | 12 | Craterpillar body segments | `src/enemies.rs:63` |
| `WORM_STRIDE` | 2 | Trail points between segments | `src/enemies.rs:64` |
| `WORM_TRAIL_STEP` | 0.55 | Metres between recorded trail points | `src/enemies.rs:65` |
| `BEAM_LENGTH` | 34.0 | Anubot Verdict Beam length (m) | `src/enemies.rs:86` |
| `BEAM_WIDTH` | 2.4 | Anubot beam hit half-width (m) | `src/enemies.rs:87` |
| `STORM_RADIUS` | 24.0 | Mars dust storm radius (m) | `src/events_world.rs:27` |
| `STORM_ACTIVE_SECS` | 26.0 | Storm duration (s) | `src/events_world.rs:28` |
| `STORM_GAP_SECS` | 20.0 | Gap between storms (s) | `src/events_world.rs:29` |
| `STORM_DRIFT` | 3.2 | Storm drift speed (m/s) | `src/events_world.rs:30` |
| `PROTOCOL_ID` | `0xA570B0_2` | Network protocol id | `src/net.rs:36` |
| `DEFAULT_PORT` | 5011 | Co-op UDP port | `src/net.rs:37` |
| `MAX_PLAYERS` | 4 | Co-op seat limit (only 2 players tested) | `src/net.rs:38` |
| `HOLD` | 5.0 | Tutorial line display time (s) | `src/tutorial.rs:17` |
| `MERGE_RADIUS` | 1.6 | Damage numbers this close coalesce (m) | `src/ui/numbers.rs:10` |
| `NUM_LIFE` | 0.7 | Damage-number lifetime (s) | `src/ui/numbers.rs:11` |
| Music `RATE / BAR / BARS / LOOP_SECS` | 22,050 Hz / 2.0 s (120 BPM) / 16 / 32 s | Adaptive music loop | `src/music.rs:14-17` |
| `MUSIC_MIX` | 0.12 | Music master level | `src/music.rs:18` |
| Audio `RATE` | 22,050 Hz | SFX synthesis rate | `src/audio.rs:8` |
| `FONT_BIG / MED / SMALL` | 34 / 20 / 15 | UI font sizes | `src/ui/mod.rs:9-11` |

The **16 SFX** (`src/messages.rs:64-81`) are Hit, Crit, Hurt, Pickup, Coin, LevelUp, Chest, Evolve, BossRoar, Click, Pot, Teleport, Shrine, Slide, Bhop and Comet.

---

## 25. Co-op rules that change content

Co-op (host-authoritative, direct-IP UDP 5011) changes these content rules. They apply on the host; the joiner receives the results.

| Rule | Behaviour | Source |
|---|---|---|
| Spawn rate and cap | × P = 1.75 / 2.4 / 3.0 for 2 / 3 / 4 astronauts. Downed astronauts still count. | `src/enemies.rs:568`, `592` |
| Spawn anchors | Spawns round-robin over every astronaut, so each gets a share over its own horizon | `src/enemies.rs:556`, `596` |
| Enemy targeting | Every enemy chases the **nearest living astronaut by arc** | `src/enemies.rs:1048` |
| Boss spawn | 30 m from the **party centroid**. Add rings form around the astronaut nearest the boss. | `src/director.rs:63-72`; `src/enemies.rs:778` |
| Boss HP | **Not** scaled by party size | `src/enemies.rs:645` |
| World difficulty | `D = max` over astronauts, so one Cursed build raises it for everyone | `src/player.rs:851-852` |
| XP | **Shared**: every gem grants its value to every living astronaut, each at its own `xp_gain` and on its own curve | `src/pickups.rs:205-221` |
| Gold, food, powerups | Go to the **collector** only | `src/pickups.rs:222-233` |
| Charge shrines | Fill `dt/8 × (astronauts inside)`. The blessing panel opens on the **host** only. | `src/interact.rs:362-386` |
| Other interactables | Usable only by the host's local astronaut; the joiner cannot use chests, the shop, shrines, Moai, Microwave, Cage or the teleporter (M2) | `src/main.rs:235` |
| Anubot beam and telegraphs | Hit every astronaut in the area | `src/enemies.rs:851-865`, `1628-1638` |
| Enemy projectiles | Consumed by the first astronaut they touch | `src/enemies.rs:1548-1556` |
| Run end | Only when **all** astronauts are downed. There is no revive, and a downed astronaut stays down on later stages (M15). | `src/director.rs:240-252` |
| Banking | A joiner banks **no** silver, counters or quests | `src/main.rs:148-151` |
| Pause | Any host panel (level-up, chest, shrine, pause) freezes the world for everyone | `src/fx.rs:47-58` |

**GDD** (`GDD.md:879-903`): per-enemy HP scaling (110–130%), boss HP scaling (165–285%), the Tumbling Beacon revive, contested chests, and killing-blow legendary drops are **not built**.

---

## 26. Code vs GDD: consolidated disagreements

Where they differ, **the code is what ships**. This table collects every disagreement noted above. `GDD.md` §15 "Current build state" and "Baseline constants" (`GDD.md:1211-1225`) are themselves stale: `GDD.md` has not changed since the first commit (`02300e3`), and 35 commits have landed since.

| Area | GDD says | Code does | See |
|---|---|---|---|
| Stage flow | Miniboss #1 gives a **guaranteed chest** | No chest drop | §13.6 |
| Teleporter | Blooms at the boss corpse | 18 m from the party centroid, random direction | §16.12 |
| Static silver | Every extra second pays Silver | Only kills: 50% × 1 silver | §14 |
| The Static | Ghosts are your real dead runs | Generic ghost enemy | §14 |
| Enemy cap overflow | Merges into The Static | Discarded | §11.1 |
| Spawn rate | `×(1 + 0.14·t)` with depth and Δ terms | `(1 + 2.1·t)(1 + D)·P`, no depth term | §11.1 |
| Enemy HP and damage scaling | `(1+0.11t)^1.35 × (1+0.2d) × T × (1+0.06Δ)`; damage `(1+0.08t)…` | `(1 + 0.11·t·max(√t,1) + 0.35·t)(1+D)`; damage `(1+0.12t)(1+D/2)`; resets each stage | §11.3 |
| Chain depth | Each chained world adds a Difficulty step | Nothing scales with stage or tier | §2.3 |
| Spawn timeline | Sprinters 9:00, Bruisers and Spitters 7:00–4:30, hazards 4:30–2:30 | Sprinter 8:30, Spitter 7:00, Bruiser 5:30, UFO and Beamer 4:00, Burrower 2:30, Lobber 1:00 (stage 1) | §10.5 |
| Elites | Glitched affix system; chance `min(0.35, 0.02t + …)` | Flat ×8 HP / ×1.8 dmg; forced every 40 s + 1.2% after 150 s | §12 |
| Boss phases | Authored mechanics (Burrow Bloom, Helmet Choir, Sandstorm Court, Final Judgment) | Generic enrage + banner + add ring | §13.4 |
| Dark Moon boss | THE HOLLOW COSMONAUT | Judge Anubot reused | §13.1 |
| Minibosses | Two bespoke per world | Craterpillar Jr and Rover Gone Wrong on every world | §13.1 |
| Co-op scaling | Per-enemy HP +10–30%, boss HP 165–285% | Spawn count only | §25 |
| Level-up | 3 cards | 4 cards | §8.3 |
| Refresh | 2 free, then gold (rising) | 2 per run, no gold option; Fortuna unlimited | §8.4 |
| Skip | XP boost + gold tip | +10 gold | §8.4 |
| Banish | Permanent removal from the run's pool | Permanent for items only | §8.4 |
| Evolution cap | 1 per run (+1 with Tome of Ascension) | No cap | §4.5 |
| Evolution catalysts | Rivet Gun: Splitter Chip; Drones: Extra Battery; Tesla: Laser Sight; Rocket Pod: Splitter Chip; Cryo Vent: Duct Tape | Laser Sight; Splitter Chip; Extra Battery; Cursed Moon Rock; Fish Bowl Helmet | §4.7 |
| Batch-1 weapon behaviours | Mortar, travelling disc, stun cone, tolling bell, combo yo-yo, nova | Reuse Rocket, Chain, MeleeArc, Aura and Orbit | §4.7 |
| Weapons | 21 base + 21 evolutions | 16 + 16 | §4 |
| Heroes | 21, quest-gated; recruit kits as in `GDD.md:400-420` | 12; the 6 recruits start unlocked with different kits | §3.4 |
| Items | Stack "near-infinitely"; proc and cursed items; Trampoline Soles = +1 jump | Capped at 2–9 stacks; pure stat items only; Trampoline Soles = +0.15 Jump Height | §5.3 |
| Tomes | 23 tomes, 10 ranks, cost `100 × 1.6^level`, 4-slot loadout | 8 tomes, 20 ranks, cost `8·l^1.5`, 3 slots growing to 5 | §6 |
| Luck | Also improves elite drops, the Microwave and evolution-card rate | Only rarity rolls and the level-up item pool | §9.3 |
| Shady Guy | 1–2 per stage; hat colour shows rarity | Always 2; no hat colour | §16.4 |
| Microwave | Item + gold → dupe one grade lower, or gamble an upgrade | Free +1 stack of an owned item | §16.8 |
| Chests | Co-op contested chests (`DESIGN.md` also lists locked and golden chests) | Plain single-player chests; price stuck at 25 | §16.3 |
| Silver formula | `survival/6 + kills/4 + boss×150 + tier×50 + overtime …` | `silver_run + kills/40 + level + 30×tier on win` | §18.3 |
| Comet Combo | Requires a full great-circle lap; highlight clip | Charge = Σ tail × speed × dt; no lap, no clip | §20 |
| Daily | Three lives, leaderboard, ghost replays | Seed + name + a local best score | §21 |
| Worlds | Gimmick, hazard and event per world; 9 more worlds; branching chains | Only Mars's dust storm; 3 worlds; fixed chains | §15 |
| Day and night | Sweeping terminator; night modifiers | Fixed sun; no modifiers | §15.1 |
| Movement | Slide on Shift; ~60% air control; ~1.2 s hang | Ctrl or C; 35%; 0.73 s | §22.1 |
| Quests | About 50 at launch, six categories | 16 | §19 |

---

## 27. Known bugs that distort the catalog numbers

These are confirmed by reading the code; H1 and H2 are also confirmed by a live save and log. Fixing them changes the numbers above. The IDs are those of `docs/KNOWN_ISSUES.md`; "low" and "design" rows have no tracker ID.

| ID | Bug | Effect on content | Where |
|---|---|---|---|
| **H1** | `bank_results` reads the local `PlayerState` after the stage entities were despawned | Results show LEVEL 1 and GOLD 0; `best_level` is banked as 1; the payout uses level 1 and silver_gain 1.0; **Level20 is impossible** | `src/director.rs:274-280`, `298`; `src/main.rs:141-155` |
| **H2** | Taking a chest never increments `chest_opens` or `chests_opened` | Chest price stuck at 25 (`CHEST_COST_GROWTH` dead); **Chests10 is impossible**; Rocket Pod never enters the level-up pool | `src/ui/panels.rs:341-361`; `src/interact.rs:440` |
| **H3** | An evolution never increments `run.evolves` | **EvolveWeapon is impossible** | `src/run.rs:608-617`; `src/ui/panels.rs:233-238` |
| M9 | The dust storm tracks only the host's LocalPlayer but silences ranged enemies against everyone | Co-op balance on Mars | `src/events_world.rs:112-117`; `src/enemies.rs:1284`, `1342`, `1456` |
| M10 | `DustStorm` is never reset | A second Mars visit in one session has storms with no dome | `src/events_world.rs:62`, `80` |
| M14 | The DEV key B summons the stage boss, ungated | Progression shortcut; B also banishes | `src/enemies.rs:913-936`; `src/main.rs:244` |
| M17 | `elapsed` resets every stage; nothing reads the stage or tier | Stages 2 and 3 are easier than their position suggests | `src/director.rs:197` |
| L14 | Owning any cryo weapon adds +0.25 slow to **every** hit; the per-weapon slow values are unused | Cryo Vent's 0.45 and Absolute Zero's 0.75 do nothing | `src/combat.rs:277`, `894-902`, `930-932` |
| L15 | The aura visual omits the size factor | Auras look smaller than their damage radius | `src/combat.rs:851` |
| L24 | Chests and the Shady Guy ignore max stacks | Items can exceed their cap | `src/ui/panels.rs:345-349`, `465-469` |
| L25 | The Microwave is consumed when "NOTHING FITS" | Lost once-per-stage use | `src/interact.rs:519` |
| L26 | The `roll_item` fallback ignores bans | A banished item can come back from a chest, shrine or shop | `src/interact.rs:104-110` |
| L30 | Fortuna's +2 refreshes are dead because her refreshes are free | None | `src/run.rs:229-232` |
| L27 | `static_secs_best` counts only the last stage of the run | SurviveStatic2Min must be met on the final stage | `src/director.rs:290` |
| L28, L29 | Tutorial lines 4 and 6: "Shift to slide" and "It'll drop a chest" | Both are wrong | `src/tutorial.rs:24`, `26` |
| L16 | Pots and bosses count toward the enemy cap | The effective cap is about 1,140 on the Moon at stage start | `src/enemies.rs:548`, `562` |
| L12 | Buried Burrowers can be hit by projectiles, drones and the comet | Free damage on ambushers | `src/enemies.rs:473-481`; `src/combat.rs:671` |
| L20 | Boss HP has no party scaling; add rings bypass the cap | Co-op balance | `src/enemies.rs:645`, `775-791` |
| low | Shield, EliteDamage, Knockback and SilverGain are granted by nothing | 4 of the 27 stats are inert | `src/stats.rs` |
| low | The six recruits' unlock texts are not enforced | They start unlocked | `src/content/characters.rs:237-244` |
| design | A boss killed during The Static opens no teleporter | Probably intended (the GDD says "The Static swallows that world") | `src/director.rs:55-59`, `112` |

---

*Generated from the source at the v0.1 build (branch `main`, commit `99d012f`; `src/` had no uncommitted changes). When a number here disagrees with the code, the code is right: fix this document.*
