# ASTROBONK
### *Everything Comes Back Around*
**Game Design Document — the canonical design bible**

> Marketing tagline: **"Run around your own apocalypse."**
> This GDD supersedes the old design notes as the source of truth. The original Megabonk
> research digest lives on in [DESIGN.md](DESIGN.md) as the foundational reference; the
> living **CANON LEDGER** (§15) records every reconciled number and the current build state.

---

## The Golden Thread

**Everything comes back around.** On a world this small there are no corners and no escape.
The enemies you outrun crest the horizon *ahead* of you. The astronauts who died before you
return as **The Static**. The players who bounce at week three are pulled back by a loop
designed to close. Every system in this document — great-circle kiting, encirclement, the
scanner-copy twist, the anti-churn meta, the daily seed — is the same idea at a different
scale: *you are always the center of a small, dying, curving world, and it always laps back
to you.* The mechanic (the ground curves), the story (the copy that keeps copying), and the
business (the day-21 return) are one sentence. **Hammer it on every page.**

## TL;DR — the one-page pitch

You are a **dead astronaut**, freshly re-printed, dropped onto a planet the size of a soccer
field at the edge of a solar system that something already ate. Your weapon fires itself; your
only job is to **move** — dodge, kite, and stitch together a build absurd enough to survive ten
minutes of 360° encirclement before the boss falls and the teleporter yanks you to the next
dying rock. It's Vampire Survivors' dopamine and Risk of Rain 2's escalation welded to one
idea nobody else has: **the arena is a tiny sphere, and the horizon is where the horror comes
from.** Megabonk proved a billion-dollar appetite and left three doors open — more worlds,
co-op, and a reason to stay. We walk through all three and lock them behind a mechanic Megabonk
can't retrofit: *the ground is round.*

**Engine:** Rust + Bevy 0.18, zero external art/audio (procedural meshes, in-code WAV-synth SFX).
**Status:** complete playable v0.1 skeleton, ~40% of Megabonk's content scale, smoke-tested.

---

# 1 · Vision, Pillars & The Thesis

### The Core Fantasy
It feels like being small. You crest a hill and the whole planet falls away beneath your boots —
you can *see* the swarm cresting the far horizon in every direction at once, tiny at first, then
not. There's no wall to back into, no corner to hold; on a sphere, "retreat" is just "the long
way around," and every enemy you outran is waiting for you when you close the loop. It's tense,
it's funny (your guy is an out-of-shape NASA retiree swinging a **wrench**), and underneath the
dopamine there's a quiet dread: the sky is wrong, Earth hangs in it like a memory, and you are
the last living thing on a world that is running out of night. You are alone, you are
outnumbered, and for exactly ten glorious minutes you are *winning anyway.*

### The Six Design Pillars

1. **THE PLANET IS THE GAME.** Curvature, verticality, and a real day/night terminator are
   mechanics, not backdrop. *So we always design around encirclement and the run-around loop;
   nothing plays the same as a flat arena.*
2. **MOVEMENT IS THE SKILL.** Weapons are automatic; mastery is positioning, dodging, jumping,
   route-planning. *So we always make the ground you choose to stand on matter more than any
   button.*
3. **DOPAMINE FIRST, DREAD UNDERNEATH.** Chunky and stupidly satisfying on top; cool and a
   little sad below. *So we always pair a dumb grin (Space Borgar) with a cold fact (these
   astronauts are dead) — the melancholy makes the meme land harder.*
4. **EVERY RUN BRANCHES.** Heroes × weapon evolutions × tomes × planet-chains recombine into
   builds we didn't fully predict. *So we always ship synergies over stat-sticks — a card earns
   its slot only if it changes how you move or what you chase.*
5. **NO TWO ROCKS FEEL ALIKE.** Each world has a signature hazard, flora, gravity feel, and
   event that could not be swapped onto another. *Content variety is the product.*
6. **THE HOOK OUTLASTS THE HONEYMOON.** Retention is a designed system. *We measure every
   feature by "does this pull someone back on day 21," not "is this fun on day 1."*

### Why We Beat Megabonk

| Their weakness | Our answer |
|---|---|
| **Only 2 real maps at launch** (most-cited flaw) | Distinct worlds *now*, and a pipeline where the tiny-sphere USP makes every planet feel like a new game. Variety is the headline, not the patch note. |
| **Single-player only** | **Co-op.** Encirclement on a sphere is *built* for a squad watching each other's horizons — two players splitting to cover opposite poles is something Megabonk structurally cannot do. |
| **Flat, interchangeable arenas** | The **tiny planet**: curvature, verticality, a day/night terminator, planet-as-arena events. There is no flat-map version of ASTROBONK. |
| **~60% drop-off by week 3** | A deliberate **anti-churn retention layer** — quests as the unlock engine, tome meta, ascension, dailies, and NG+ designed for the day-21 return. |
| **Hit-or-miss humor** | Meme charm is **authored and load-bearing** — curated bits with a cool-sad spine so comedy has weight instead of noise. |

**Thesis in one line:** *Megabonk proved the appetite; it did not fill it.* We take the exact
three things players begged for and weld them to a mechanic that can't be copied — the ground
curves.

### Audience & Platforms
**Primary:** Vampire Survivors / Megabonk / Brotato players who hit the content wall and want
the *next* one, plus Risk of Rain 2 fans who want build depth and co-op in a lower-commitment
package. **Secondary:** streamers/clip-culture (the sphere makes instantly-legible "look at
this horizon" moments) and the meme-game crowd who buy on a grin. **Platforms:** PC-first
(Steam), KBM + full controller, Steam Deck verified at launch; consoles post-launch.

### Taglines
- **"Run around your own apocalypse."** *(lead marketing — sells the clip)*
- "The horizon is where they come from."
- "Small world. No corners."
- "Dead astronauts. Doomed worlds. Excellent loot."

---

# 2 · Narrative & Worldbuilding — The Bonkiverse

### The Premise: a solar system being eaten
It started as a signal. In 2029 every deep-space array on Earth caught the same thing at once —
not a message, but an *appetite*. Astronomers named it **THE SWARM**: a lightless tide of
chittering geometry that peels planets like fruit. Neptune went dark first, then the belt, then
Mars started *thinning*. The Swarm has no throne to bomb, no queen to shoot — it's closer to
weather, or hunger with a physics degree. The load-bearing line, scrawled on a Mission Control
whiteboard: **"It's not invading. It's grazing. We are the field."**

Humanity's answer was absurd because every serious one had failed: send **astronauts**, one at a
time, on foot, to the tiny captured moonlets the Swarm hasn't finished digesting, and have them
*hold the ground long enough to matter.* Not win. Hold. Each planet is small enough to run all
the way around — which means there is nowhere the horde isn't. You are always surrounded because
you are always the center of a very small world.

### What The Static really is *(and the "make it legendary" upgrade)*
When the ten-minute window closes and the teleporter's been missed, the horizon fills with **THE
STATIC**: a soundless, endless swarm of **ghost-cosmonauts** — spacesuits with nobody home,
visors full of grey snow. These are the failed missions. The Swarm doesn't just eat mass; it
eats the *attempt.* The Static is what's left of trying: motion without a person, still walking
the loops they died running.

**Canon feature — the Static is literally your dead runs.** Each ghost wears the suit, name, and
*build silhouette* of a real prior death: yours locally, and (online) a friend's or a stranger's
from that day's seed. *"One of the ghosts is wearing your suit. It waves. You wave back. Neither
of you stops shooting."* Killing them pays Silver — the quietly horrible joke of the economy is
that you fund the next hero by putting down the last one.

### The fiction behind the systems
- **Why do runs reset?** You don't come back — a *copy* does. Mission Control runs the **RE-BOOT
  Program**: every astronaut is print-scanned before launch and re-instantiated from that scan
  after they fall. Death is a data event; the you that dies joins the Static, the you that wakes
  in the launch bay remembers nothing but "the mission continues."
- **Why Silver?** It's salvage — Earth's treasuries melted into one machinable alloy to keep the
  printers warm. A civilization liquidating itself.
- **Why is Gold in-run only?** It's Swarm-glitched matter that evaporates the instant you leave
  the planet. Worthless to bank, priceless for the next ten minutes. The Shady Guy knows this.
  That's why he's always smiling.
- **Who sends you?** The **LAST STAND COALITION** — the skeleton crews of every agency sharing
  one launch pad and a very cracked coffee machine.

### The agencies
| Faction | Vibe | Sends |
|---|---|---|
| **NASA** | Bureaucratic warmth; nostalgia and duct tape | Buzz |
| **Roscosmos** | Grim, brilliant, fatalistic; lost the most | Valentina |
| **JAXA** | Precise, quiet, honorable; treats each print like a person | Yuki |
| **Belt Miners' Union (Local 404)** | Blue-collar, unionized, allergic to management | Doug |
| **AXIOM Dynamics** | *The sinister one.* A private "continuity contractor" that owns the RE-BOOT patents | (nobody official) |

**AXIOM Dynamics** is the rot under the floorboards — they didn't invent the print-scanner to
save astronauts, they invented it to *own* them, licensing each re-instantiation back to the
Coalition at a per-death fee. Their memos call the Swarm a **"market condition."** They know more
than they should. B0-NK keeps finding AXIOM serial numbers on its own parts and does not like
what that implies.

### The campaign arc & the Dark Moon twist
- **THE MOON** — home turf, the tutorial-world, the place you defend. Earthrise still blue on the
  horizon. Barely.
- **MARS** — a graveyard already half-eaten. You're not saving it, you're reading its autopsy.
- **THE DARK MOON** — the twist. It isn't uncharted; it's Earth's *second* moon that the Swarm
  already finished — devoured, hollowed, rebuilt from the inside as a lure. Its glowing fungus is
  the Swarm's own tissue mistaking itself for flora. The deeper you go, the more its geometry
  matches the RE-BOOT print-bay. **The revelation:** the Swarm didn't come from the stars. **It
  came from the scanner.** AXIOM's very first re-instantiation, in 2029, printed something that
  kept printing. We are being eaten by our own copy — and every reset feeds it one more.

This is why the endgame antagonist is **THE FIRST ONE** (the original copy), whose true
devouring form is fought as the superboss **THE DEVOURER ("It Ate The Sun")**, and why NG+ is
called **THE COPY** (§10) — the Static learns and mirrors *your* builds back at you. The horror
of fighting your own optimized loadout is the endgame Megabonk can't touch.

### Environmental storytelling
**Radios** (looped final transmissions from prior missions), **wrecks** (crashed landers — some
are your *own* previous prints, still gripping a wrench), and **the caged Chimp-O** (the Mercury
program's forgotten passenger, left in orbit in 1961, alive somehow, rightfully furious —
unlocking him is finding out the cage was never locked from the outside).

> **AXIOM MEMO 7734 (Mars wreck):** "Re: astronaut morale. Recommend we stop telling them it's
> the last run. It is never the last run. — Continuity Dept."
> **RADIO, looping (Moon):** "…Houston, the horizon's full. Tell my print I said don't bother.
> …Tell it anyway."
> **BUZZ, launch-bay bark:** "I don't remember dying. That's the *good* news."

Funny on top. Grazed underneath.

---

# 3 · Core Gameplay Loop & Run Structure

### The 30-second loop — "Kill, Glow, Grow, Go"
1. **KILL** (auto) — your weapon fires itself; you aim your *body*, arcing your run so the
   auto-shot sweeps the densest part of the encircling ring.
2. **GLOW → GEMS** — kills burst into XP gems (the chunky "bonk" pop + hitstop is the drug); your
   Magnet radius vacuums them as you pass.
3. **GROW** — fill the bar → **LEVEL-UP.** Time freezes, cards fan out.
4. **CHOOSE** — one click. Build math shifts. Dopamine spike #2.
5. **GO → REPOSITION** — unfreeze; the ring closed while you shopped. Sprint the horizon: crest a
   hill for LOS, drop behind a ridge to break a Beamer's aim-line, or run a full lap to *wind the
   whole horde into a comet-tail behind you* — then turn and let auto-fire mow the queue.

The loop's genius is the **planet as pressure valve**: on a flat map you backpedal into a corner
and die; here there's no corner, only *keep moving around a globe*, which converts panic into
rhythm.

### ★ The COMET COMBO — the signature scored mechanic
We formalize the lap-kill into the single most marketable verb in the game. As you run with a
horde strung out behind you, a **tail-counter** grows per enemy in your wake. Complete a full
great-circle with the tail intact and you **cash out**: a scored screen-clear detonation scaling
with tail length, a "COMET ×N" banner, bonus Silver, and an auto-flagged highlight clip. This
converts the USP from a *feeling* into a **scored skill expression** — the thing streamers chase
and the thing our trailer opens on. Everything comes back around; the Comet Combo is that,
weaponized.

### The 10-minute run arc
A stage is a **10:00 countdown**. Pressure, spawn rate, and reward richness all ride the clock —
tuned so tension breathes (inhale/build, hold/miniboss, exhale/loot), never a flat grind.

| Time | Beat | Spawns | Tension |
|---|---|---|---|
| **10:00** | Cold open | Shambler trickle over the horizon | "I have room." First lap, first level by ~9:20. |
| **9:00–7:30** | First escalation | Sprinters join, ring tightens | Movement stops being optional. |
| **7:00** | **MINIBOSS #1** | Craterpillar Jr / Rover Gone Wrong + elites | Spike → **guaranteed chest** (first build fork). |
| **7:00–4:30** | Density ramp | Bruisers, Spitters lob from range | The horizon *fills* — every direction is a threat. |
| **4:30–2:30** | Hazards live | Burrowers erupt, Beamers paint aim-lines, Lobbers drop AoE; a **planetary event** may fire | Multi-axis: ground, horizon, sky. |
| **2:00** | **MINIBOSS #2** | Harder, elite-flavored, Static whispers | Endgame is near. |
| **1:30** | **STAGE BOSS** | Craterpillar / Judge Anubot / world boss; adds keep pouring | Peak focus. |
| **~0:30** | Boss down → **TELEPORTER** | Portal blooms at the corpse | Relief + decision: dive on, or linger to farm? |
| **0:00** | **THE STATIC** | Endless ghost-cosmonauts, scaling forever | Greed tax — every extra second pays Silver but never stops accelerating. Get out or get devoured. |

The **7:00 / 2:00 / 1:30** cadence front-loads a reward after each spike — pain is always
monetized within seconds.

### Tier / chain structure
A run is a **planet chain** whose length = **Tier** (see §15 glossary; "Tier" means *chain
length* and nothing else). Beat the boss, take the teleporter, carry your build onward at
escalated difficulty.

| Tier | Chain | Length |
|---|---|---|
| **T1** | [Moon] | 1 stage |
| **T2** | [Moon → Mars] | 2 stages |
| **T3** | [Moon → Mars → Dark Moon] | 3 stages (finale) |

Each chained world inherits your level/gear but adds a flat Difficulty step + its own hazard
flavor. Later worlds and higher tiers appear on the select screen as quests unlock them.

### Win & loss states
- **WIN (Chain Clear):** kill the final boss of the chosen tier and exit → results, banked Silver
  (boss + clear bonus), quest progress, first-clear unlocks.
- **WIN-ish (Static Survivor):** skip the teleporter and farm The Static — escalating Silver, no
  victory jingle, eventual death. Pure greed.
- **LOSS (KO):** HP hits 0. You still bank all Silver and quest ticks earned. **Death is never a
  zero** — a "bad" run still moves the meta bar, so the next launch is one tome-level stronger.

### Difficulty scaling model
Pressure scales on run-time `t` (min), chain depth `d`, planet multiplier `T`, and your
**Difficulty stat `Δ`** (raised by Cursed Moon Rock, Cursed Tome, higher tiers). Formula *shape*:

```
EnemyHP(t)     = HP_base   × (1 + 0.11·t)^1.35 × (1 + 0.20·d) × T × (1 + 0.06·Δ)
EnemyDMG(t)    = DMG_base  × (1 + 0.08·t)      × (1 + 0.15·d) × T × (1 + 0.05·Δ)
SpawnRate(t)   = Rate_base × (1 + 0.14·t)      × (1 + 0.10·d)       × (1 + 0.04·Δ)
EliteChance(t) = min(0.35, 0.02·t + 0.05·d + 0.03·Δ)
```

HP scales **super-linearly** (exponent > 1) so the late game demands *build multiplication*, not
raw fire — this is what makes evolutions feel mandatory. Damage scales gentler so skilled
movement can always out-dodge. `Δ` is the voluntary risk knob; the Cursed items are the honeypot.

### ★ Diegetic difficulty — the world visibly dies
Difficulty is *rendered*, not just numeric. Cranking Cursed / equipping the **Devoured Sun
Shard** literally shrinks the day side each 60s — the more danger you invite, the more the sun
goes out, until you fight by flashlight on a night-locked world. Difficulty **is** the
melancholy, on-screen, always. A thesis-level marriage of dopamine and dread.

### The "always 5 seconds from a dopamine hit" cadence
Guaranteed positive stimulus roughly every 5s, staggered so troughs never overlap: kill-pop
(~1–2s), gem-chime threshold (~5–8s), level-up (~20–40s), chest/elite/Shady-Guy ping (~60s),
scripted spikes (7:00/2:00/1:30). *Two dry seconds with nothing popping is a bug, not a lull.*

### Choice economy — Refresh / Skip / Banish *(canon, reconciled)*
Level-up pauses the game. Three cards deal up; you steer the RNG toward getting a weapon to **max
level 7 while owning its paired item**, so its **evolution card** appears (marked with a golden
antenna-arc the instant it's ready).

| Action | Cost | Effect |
|---|---|---|
| **Refresh** | 2 free/run, then Gold (rising each use) | Reroll all cards. |
| **Skip** | Free | Take none; bank a small **XP** boost + a **Gold** tip. *(There is no "Bonk Bucks" — the only currencies are Gold and Silver.)* |
| **Banish** | 3 charges/run (+via items/Tome of Banishment) | Delete a card *permanently* from this run's pool — the surgeon's tool for build purity. |

Banish protects the plan, Refresh chases it, Skip funds it. The whole minute-to-minute is quietly
a puzzle of steering RNG toward one god-tier evolved build before the 1:30 boss — and the tiny
planet keeps you alive long enough to solve it.

---

# 4 · Movement, Camera & The Sphere (Signature Systems)

Every clone puts you on a flat plane and asks you to draw circles. ASTROBONK puts you on a
*planet* and asks you to draw **great circles** while gravity glues your boots to the crust. The
horizon is a curved lip ~40 m out, and the horde crests it from *every* compass point at once.
There is no back of the map, no corner. That single geometric fact rewrites the genre.

### Why it beats 2D — in feel
In Vampire Survivors you retreat forever in one direction and the swarm becomes a safe comet-tail.
On a tiny planet, retreating "away" means **the horde you fled reappears in front of you** —
you lapped the world and rear-ended your own kill-zone. The sphere converts the genre's flattest
tactic into a live decision: *which way around the ball, and do I have time to close the loop
before the far side fills in?* You can *see* danger sink below the horizon as you flee it, then
dread it rising back up ahead. That rise-and-set rhythm is the game's heartbeat.

### The movement toolkit
| Tech | Input | Effect |
|---|---|---|
| **Run** | WASD / stick | Base surface-tangent speed; camera auto-aligns "down" to planet center |
| **Jump** | Space | 1 baseline; low gravity gives a floaty ~1.2s hang |
| **Extra jumps** | Space (air) | Chimp-O +1, Trampoline Soles +1, stackable |
| **Slide** | Shift while moving | Crouch-slide; **accelerates downhill** (slope-boost), **knocks back** enemies you plow through |
| **Bunny-hop** | jump on slide-landing | Land a slide, hit Space in the ~0.15s window → preserve momentum; chain indefinitely |
| **Air control** | stick in air | ~60% authority — curve over a crater lip, not cheese a full 180 |
| **No fall damage** | — | Peaks and cliffs are *toys*, never punishments |

**Slide is the skill fulcrum** — Yuki's whole identity (slide → +30% attack speed 3s), but every
hero weaponizes it: slope-accel turns craters into slingshots; knockback carves a breathing-hole
in a wall of Shamblers.

### The skill ceiling
A novice runs circles and jumps to dodge Burrowers. A **master** never touches the ground at full
speed: slide *down* a crater's inner wall to bank momentum, bunny-hop *up* the far rim, launch
off the lip, glide a quarter-planet in one combo — outrunning The Static without ever giving the
swarm a stationary target. Mastery is reading terrain as a *momentum map*: mountains are ramps,
craters are half-pipes, ridgelines are highways. The ceiling is high because the same analytic
noise that draws the mesh draws the collision — every bump is *real* and can be surfed.

### Encirclement, not a wall
- **Splitting the swarm:** sprint a tight great-circle → the horde stretches into a thin ring;
  slide-knockback punches a gap → one mob becomes two thin arcs you AoE-clear separately.
- **Great-circle kiting:** the optimal kite is a *closed loop* that keeps everything at max weapon
  range and lets orbitals (DRONE SWARM) sweep a continuous band.
- **Terrain as tool:** duck behind a peak to break Beamer aim-lines (LOS); stand *on* a peak so
  Lobber mortars overshoot into the valley; let Burrowers erupt into the crater you just left.

### Verticality & air-damage builds
Peaks are **vantage economy** — the tallest ridge grants a brief camera pull-back and a clean
firing line down both slopes (Doug's DEATH RAY throne). We seed an **air-damage family**: the
*Icarus Boots* item (+40% damage airborne, −10% grounded) and a Tesla that chains harder mid-air.
Chimp-O's stacked jumps become an offensive rotation — hang-time *is* his DPS window.

### Day/night & the flashlight as a mechanic
Each planet rotates; the terminator sweeps the surface over the 10:00 run. **Night is a biome, not
a filter.** Ambient drops so the dark side is genuinely black beyond your weapon-mounted spotlight.
- **Night risk:** enemies +15% move speed, spawn 20% closer (they crest the horizon *inside* your
  vision); The Static is near-invisible — you hear it before you see it.
- **Night reward:** kills drop +25% Gold; a night-only elite (the **Glowspore Horror**, Dark Moon)
  is a loot piñata only trackable *by* your flashlight sweep.
- **The cone is a targeting choice:** the light is mounted to your *weapon*, so where you aim your
  build is where you can see. The evolved *Searchlight Rig* widens the cone and briefly
  **blinds/staggers** enemies caught in a hard sweep.

### Three new movement-techs Megabonk can't reach
1. **Orbital Slingshot (Slide-Jump-Slam):** slide down a steep slope to redline momentum, jump at
   the lip, then **Slam** (hold-drop) to convert horizontal speed into a radial AoE shockwave on
   landing. Whiff the ramp, whiff the bomb.
2. **★ Antipode Blink — THE signature high-skill verb.** A traversal skill/item that teleports you
   to the planet's *exact opposite point* — the one place the encircling horde is thinnest. The
   skill isn't the button; it's **earning the read** on whether the far side is safe or already a
   Static wall. It has a HUD tell (the off-screen threat ring shows antipode density) and a whole
   mastery/quest ladder. One legible, teachable, deep skill only a sphere can host. *(All other
   antipode effects — e.g. Boomerang Insurance's panic escape — route through this one mechanic;
   see §15.)*
3. **Grind-Lines (Ridge Surfing):** ridged-mountain crests expose a thin grindable spine — slide
   on and **rail** it at +50% speed with locked footing, a highway across the globe. Fall off and
   you eat the crater. Pure expression, pure risk, impossible on a flat map.

### Camera rules — the shake lesson (canon from playtests)
Per-hit screenshake on a curved-horizon camera made testers nauseous within minutes. Law, now:
1. **Stability is sacred** — the camera trails behind/above, its up-vector slerping toward the
   anti-gravity normal at a fixed max rate; it *never snaps* rounding the globe.
2. **Shake is rationed** — hitstop/micro-shake only on *big* events (elite death, boss slam,
   evolution), never chip damage. Aim from the *unshaken* position so shake never becomes
   rotational jitter. **Hard clamp: total camera offset ≤ 1.8°** — the horizon line is sacred.
3. **Horizon-lock** — soft roll-damping keeps the curve from tilting drunkenly.

---

# 5 · The Astronaut Roster (21 Heroes)

Twenty-one astronauts answered the call — most already dead when they picked up the wrench. Each
carries one signature weapon and one deep **Passive** that bends the whole run around it. Design
intent: **no two heroes want the same items**, so 21 heroes = 21 distinct build economies (the
anti-churn engine Megabonk never built).

> **Launch scope:** the 6 founders + ~6 recruits ship by 1.0; the rest arrive in free hero packs
> (see §14/§15). All 21 are canon design targets.

| # | Hero | Origin | Signature weapon | Passive (playstyle-bender) | Identity | Unlock |
|---|---|---|---|---|---|---|
| 1 | **Buzz** | NASA retiree who never stopped grinding | Wrench (melee) | +10% damage | Reliable bruiser | **Starter** |
| 2 | **Valentina** | Roscosmos ace, calm in any orbit | Laser Pistol | +15% attack speed | Rapid gunner | **Starter** |
| 3 | **B0-NK** | Robot from spare probes | Rivet Gun (spread) | +0.5% crit / level | Scaling crit-bot | Clear Moon T1 |
| 4 | **Yuki** | JAXA special ops | Kunai (seeker) | Slide → +30% atk speed 3s | Momentum knife | Clear Mars T1 |
| 5 | **Chimp-O** | Mercury chimp left in orbit '61 | Boomerang Antenna | +1 jump | Airborne trickster | Free the caged Chimp (Moon) |
| 6 | **Doug** | Belt miner, hates management | Mining Laser (pierce) | +25% gold | Economy digger | 2,500 lifetime kills |
| 7 | **Sergeant Gristle** | ESA marine, one lung, no chill | Wrench → **Sledge Fist** | <50% HP: +40% dmg, +20% size | Melee bruiser | Kill 3 bosses no-dodge as Buzz |
| 8 | **Dr. Reticle** | Disgraced sniper professor | Rail Needle (charge) | First hit each second = guaranteed crit | Crit-sniper | Land 500 crits in one run |
| 9 | **Hexa Drone-Mother** | AI hive, one satellite → many | Orbital Drones | Every Splitter Chip also spawns a mini-drone | Summoner | Own 8 drones at once |
| 10 | **Slipstream Nova** | Skateboarder, Artemis stowaway | Cryo Vent (trail) | No attack cooldown while above base speed | Speed-demon | Circle a planet 3× in <40s |
| 11 | **Old Ironclad** | Apollo pilot, welded into his suit | Rocket Pod | Immune to knockback; armor 2× below 30% HP | Immovable tank | Survive The Static 5 min |
| 12 | **Lady Fortuna** | Casino heiress who bought a seat | Boomerang → **Loaded Dice** | Free level-up reroll each level; +30% luck | Luck/economy gambler | Open 20 Legendary chests (lifetime) |
| 13 | **Glass Vesna** | Cosmonaut revived at 0.5× mass | Laser Pistol | +80% damage, max HP capped at 40 | Glass-cannon | Beat Dark Moon without healing |
| 14 | **Aurora Prime** | Solar-sail priestess of the dead sun | Tesla Coil (aura) | Damage-aura radius scales with move speed | Aura priest | 1,000 Tesla kills |
| 15 | **Rot-9** | Medical probe that weaponized decay | Kunai → **Plague Fang** | All hits stack poison; kills spread it | Status/DoT plaguebringer | Poison-kill an elite |
| 16 | **The Understudy** | A suit nobody remembers filling | Mining Laser | Starts each run as a *random other hero's* kit | Wildcard meme | Play as all 6 founders |
| 17 | **Cascade Kaito** | Storm-chaser who chased wrong | Tesla Coil | Every 6th hit chains to all on-screen | Chain-lightning aura | Chain-kill 15 at once |
| 18 | **Miss Gravity** | Physicist who deleted her weight | Cryo Vent → **Singularity Vent** | Can walk the planet's *underside*; pulls loot | Gravity gimmick | Fall off a ledge 10× (yes, really) |
| 19 | **Comet Twins (Cas & Pol)** | Two kids in one suit, arguing | Rivet Gun | Fire two weapons at once, each 60% power | Dual-wield hybrid | Reach hero level 20 |
| 20 | **Warden Solongo** | Warden of the frozen astronaut morgue | Rocket Pod → **MIRV Choir** | Revives once as a ghost with +100% fire rate | Second-wind glass tank | Die 50 times (lifetime) |
| 21 | **NULL** | The Static, wearing a suit, pretending | ??? (steals last enemy's attack) | Enemies +50% faster; all rewards +50% rarity | Hard-mode expert | Full-clear a T3 chain on Cursed |

### The heroes you'll brag about
- **Slipstream Nova (#10)** — the USP made flesh. Never stop circling: lay a ring of ice around
  the *whole planet* and let the horde chase you into your own slick. On a small enough world she
  hits her own back, catching enemies between the fresh trail and yesterday's lap.
- **Miss Gravity (#18)** — the only hero who plays both sides of the sphere, curling under the
  horizon to shake pursuers entirely; the Singularity Vent folds terrain into a pinhole that sucks
  the encirclement into one cryo-cluster. A love letter to "the arena is a ball."
- **Rot-9 (#15)** — the anti-boss. Poison spreads on kill, so The Static's endless swarm becomes
  fuel: one seeded corpse cascades a green wave across the night side. You kite; you don't burst.
- **The Understudy (#16)** — boots each run as a random other hero's kit. The "let fate decide"
  streamer bait, secretly a mastery test (own it = learn all 20 kits). A suit no one remembers
  filling, wearing everyone else's ghost — peak ASTROBONK.
- **NULL (#21)** — the Static that learned to walk, unlocked only by a Cursed T3 full-clear. The
  narrative flex: the swarm you've fled all game was, briefly, *playable.*

**Archetype coverage:** bruiser (Gristle), crit-sniper (Reticle), summoner (Hexa, Twins),
speed (Nova), tank (Ironclad, Warden), luck/economy (Fortuna, Doug), glass-cannon (Vesna),
aura/AoE (Aurora, Cascade), DoT (Rot-9), meme/gimmick (Understudy, Miss Gravity), hard-mode
(NULL). Every lane filled, and each unlock condition doubles as a mastery quest.

---

# 6 · Arsenal & Evolution Web (21 base → 21 evolutions)

Every weapon fires itself; the player aims their *feet.* On a sphere you circle in ~14 seconds,
an attack's **geometry is its personality**: a beam that misses on a flat plane sails over the
curve here; an orbital that would trail behind you wraps the whole horizon and bites the crowd
chasing your tail. Curvature is the second designer in every weapon.

**Evolution rule (canon):** a weapon at **max level 7** + owning its **paired item** → an
evolution card appears at the next level-up. **Baseline: 1 evolved weapon per run**; **Tome of
Ascension** raises the cap to 2. A single item may be the paired trigger for more than one weapon
— that is intentional (see §15), and with Ascension it lets one pickup arm two evo paths.

### Tier 0 — The Built Ten (canon)
| Base | Archetype | Evolution | Paired item |
|---|---|---|---|
| Wrench | Melee arc (Buzz) | MEGA WRENCH | Protein Paste |
| Laser Pistol | Auto-shot (Valentina) | GATLING LASER | Overclocked CPU |
| Rivet Gun | Spread (B0-NK) | RIVETER 9000 | Splitter Chip |
| Kunai | Seeker (Yuki) | BLADE STORM | Caffeine IV |
| Boomerang Antenna | Returning throw (Chimp-O) | SATELLITE ARRAY | Golden Antenna |
| Mining Laser | Pierce beam (Doug) | DEATH RAY | Heavy Payload |
| Orbital Drones | Orbital | DRONE SWARM | Extra Battery |
| Tesla Coil | Chain lightning | STORM CORE | Laser Sight |
| Rocket Pod | Homing salvo | MIRV POD | Splitter Chip |
| Cryo Vent | Slow aura | ABSOLUTE ZERO | Duct Tape |

### Tier 1 — The New Eleven
- **Meatball Comet** — lobbed mortar; splatters AoE over the horizon. **Evo: RAGÙ RAIN** (+Fish
  Bowl Helmet) — splits into 3 on the way down, blanketing the ring. *"Nonna's recipe. Survived
  reentry. Barely."*
- **Duct-Tape Turret** — deployed summon. **Evo: THE MANUFACTORY** (+Duct Tape) — turrets are
  permanent and *planet-locked*, so a lap past your own turrets is a gauntlet you built.
- **Solar Flare Lens** — rotating lighthouse beam, day-side only. **Evo: CORONA** (+Extra Battery)
  — ignites the *ground*, laying burning arcs along your trail. *"The sun remembers every planet
  it outlived."*
- **Gravity Grenade** — trap that yanks the horde into one clump. **Evo: EVENT HORIZON** (+Magnet
  Boots) — the clump also sucks in loot/XP/gold across the hemisphere.
- **Ricochet Disc** — bounces enemy-to-enemy; on a small sphere it laps the world and hits your
  pursuers *from in front*. **Evo: THE OMNIDISC** (+Lucky Meteorite) — infinite bounces.
- **Static Cling** — melee-hug damage aura. **Evo: FULL DISCHARGE** (+Thorn Plating) — periodic
  screen-clearing nova. *"Ghost-cosmonauts hate this one weird trick."*
- **Seed Pod Launcher** — plants explosive spires. **Evo: THE GREENHOUSE** (+Cursed Moon Rock) —
  untriggered spires grow into permanent turrets; terrain becomes your farm.
- **Sonic Whoopee** — cone knockback + stun; the best "oh god I'm surrounded" panic button.
  **Evo: THE BROWN NOTE** (+Rocket Boots) — the shove becomes a lethal repulsor ring.
- **Yo-Yo of Damocles** — close melee tether-orbit; scales with your unhit-move combo. **Evo:
  SWORD-YO** (+Heavy Payload) — the cord extends to garrote the whole ring at max combo.
- **Cosmonaut's Bell** — tolls every 4s, damaging + marking foes for +crit. **Evo: THE ANGELUS**
  (+Star Chart) — each toll resurrects a slain enemy as a friendly Static-wisp.
- **Flag of Earth** — plant a banner that buffs you and slows enemies in a growing radius. **Evo:
  MANIFEST DESTINY** (+Space Credit Card) — the zone expands to wrap the entire planet. *"We came
  in peace. That was optimistic."*

### Synergy & evolution philosophy
The addiction engine is **multiplicative stacking across four layers leveled at four different
times**: pre-run **Tome** loadout (Silver), **hero passive** (fixed), in-run **weapon** (L1–7),
in-run **items** (the pairing that unlocks evolution). No single layer is a build; the *product*
is. When a run "pops," a screen that held 40 enemies now holds 400 corpses — that spike is what
we sell. Keep three multiplier types legible so players *feel* the math: **additive** (the floor),
**rate** (attack speed / projectiles — the accelerator), **conditional/triggered** (crit,
lifesteal, on-hit/on-kill — the exponent, because each trigger fires *per projectile per enemy*
and encirclement guarantees a wall of targets).

**The sphere is the secret fifth multiplier:** orbitals wrap the horizon; beams/boomerangs return
from the far side; trails and traps (Corona, Manufactory, Greenhouse) let you **load your own
planet like a magazine** and run laps through it.

### ★ Named build engines *(THE GARDENER promoted to a headline archetype)*
- **THE LAWNMOWER** — Chimp-O + Satellite Array + Drone Swarm + Agility/Cooldown. Everything
  orbits; sprint one way forever and the equatorial band is a blender.
- **THE CRITSHOW** — Valentina + Gatling Laser + Laser Sight + Heavy Payload + Precision/Cursed +
  Cosmonaut's Bell crit-mark. Every rate-up becomes a crit-up; fragile, ludicrous, screen-shaking.
- **THE GARDENER (headline)** — Doug + Greenhouse + Manufactory + Space Credit Card + Golden/**Tome
  of the Horizon**. *Terraform a killzone into the planet, then run laps through your own gun.* No
  flat survivors game can host "make the world hostile and then leave, letting your map kill for
  you." This is the purest expression of the golden thread and a top-billed marketing shot.
- **THE BLACK HOLE / WHIRLPOOL** — Gravity Grenade → Event Horizon + Ragù Rain + Cursed Moon Rock.
  You don't chase the horde, you *summon* it into the kill.

---

# 7 · Items, Tomes & Buildcraft

Items are the connective tissue. They drop from chests, the Shady Guy, elite corpses, and boss
teleporters, and they **never level** — power is fixed at pickup, so your levers are **Rarity
Grade** and **stacking**. On a planet where you can physically outrun a stat check, items turn "I
dodged everything" into "I deleted the horizon."

**Rarity Grades:** Common / Rare / Epic / Legendary. Higher grade = a bigger roll of the *same*
effect. Legendaries and marked items are **stack-capped** (usually 1–2). Most items stack
additively and near-infinitely — diminishing returns come from opportunity cost, not soft caps.
**Luck** raises grade rolls *everywhere* (chests, vendor, elite drops, Microwave gambles, and
evolution-card appearance rate) — the one stat every build quietly wants.

### The 22 built items (canon)
Space Borgar (+HP), Moon Cheese (+regen), Duct Tape (+armor), Slippery Visor (+evasion), Protein
Paste (+dmg), Overclocked CPU (+atk speed), Fish Bowl Helmet (+size), Rocket Boots (+move),
Trampoline Soles (+jump), Magnet Boots (+pickup), Lucky Meteorite (+luck), Space Credit Card
(chest discount), Golden Antenna (+gold), Star Chart (+xp), Laser Sight (+crit), Heavy Payload
(+crit dmg), Extra Battery (+duration), Splitter Chip (+1 projectile), Cursed Moon Rock
(+difficulty +luck), Thorn Plating (thorns), Vampire Visor (lifesteal), Caffeine IV (+proj speed).

### New items — the ones that *do things*
**Proc & conditional (Rare–Epic):**
| Item | Grade | Effect |
|---|---|---|
| **Orbital Yo-Yo** | Rare | Every 4s a debris chunk orbits you once (40 dmg). *Scales with planet radius* — bigger world, wider swing |
| **Comet Tail** | Epic | Burning trail while moving; stand still 2s to ignite it around you (kite-to-win) |
| **The Overheat** | Epic | +40% atk speed; every 10th shot jams 1s. Punishes greed, loves crit |
| **Downhill Momentum** | Epic | +1% dmg per meter of elevation *descended* this second — run down craters to nuke |
| **Second Astronaut** | Epic | A ghost co-pilot mirrors one random weapon at 50% — literal co-op flavor |
| **Encirclement Bonus** | Epic | +2% dmg per unique compass direction enemies surround you from (max +16%). The USP, weaponized |
| **Icarus Boots** | Epic | +40% dmg airborne, −10% grounded (air-build enabler) |
| **Anti-Grav Boots** | Legendary | Hold jump to hover 2s; while airborne all weapons fire in a full 360° ring |
| **Little Black Hole** | Legendary (cap 1) | Pulls all enemies within 8m to a point every 6s. It's already a singularity |
| **Dead Man's Tether** | Legendary (cap 1) | When you'd die, "rewind" 3s to your last position at 1 HP. Once/run. *(A death-save — see the stacking rule below.)* |

**Cursed / high-risk:**
| Item | Grade | Effect |
|---|---|---|
| **Cracked Helmet** | Cursed | +50% dmg, +50% crit; you take +100% damage |
| **The Static Radio** | Cursed | +1 Rarity Grade to loot everywhere; The Static arrives 90s early and angrier |
| **Widow's Ring** | Cursed | +30% all stats while at exactly 1 HP; −20% max HP *(a death-save enabler)* |
| **Signal Flare** | Cursed | Enemies always know where you are; +40% gold, +40% xp |
| **Devoured Sun Shard** | Cursed (cap 1) | +100% dmg. Every 60s the day side shrinks toward total night — the diegetic-difficulty lever |
| **Antipode Blink** | Rare | Active/auto teleport to the planet's opposite pole. *Boomerang Insurance* (auto-blink at 20% HP) routes through this one mechanic |

> **Death-save stacking rule (canon):** at most **one** death-save resolves per would-be-death
> (Dead Man's Tether → Warden Solongo's ghost-revive → Boomerang Insurance/Antipode escape →
> Widow's-Ring 1-HP survival, in that priority). They never compound into immortality.

### The Shady Guy vendor *(spec — filling a canon gap)*
A greasy, always-smiling figure who spawns 1–2 per stage. **Stock is rolled at stage entry**
(Luck applies at entry, not on approach) and **his hat color = the Rarity Grade** of his wares —
never miss a gold hat. He sells items for **Gold**; stock refreshes each stage. Diegetically he's
an AXIOM contractor selling you Swarm-glitched matter he knows evaporates — the house always wins.

### The Microwave *(spec — wiring the orphaned system)*
A glowing appliance half-buried in a crater; spawns once per stage, **interact with E**. Insert
one item + Gold → **duplicate** it at one grade lower, *or* gamble to **upgrade** it one grade
(Luck-weighted). Duplicating a capped Legendary is blocked (it beeps sadly). This is our answer to
Megabonk's reroll grind: a physical, planet-object risk toy. **Tome of Duplication** grants one
free use per stage.

### Tomes — meta loadout & exponential anchors
Pre-run passives bought with **Silver**, slotted before you drop. Keep the **8 built** (Damage,
Health, Agility, Cooldown, Precision, Golden, XP, Cursed) and add **15 new** for **23 total**:

| New tome | Effect (max) |
|---|---|
| Orbit | +size & speed of orbiting/return weapons |
| Encirclement | +dmg scaling with nearby enemy count |
| Nightfall | Stronger flashlight, +dmg on night side |
| Gravity | Faster fall, +jump (air builds) |
| Swarm | +proc frequency on all "every X seconds" items |
| Salvage | Chests/Microwave cost less Gold |
| Ricochet | Projectiles bounce off terrain once |
| Momentum | +dmg the longer you keep moving |
| Vampirism | Baseline lifesteal, boosts Vampire Visor |
| Elite | Elites drop more; you take more elite dmg |
| Banishment | +1 Banish charge, +1 Refresh |
| Duplication | Free Microwave use per stage |
| **Horizon** | XP orbs auto-collect over the horizon *(the Gardener's tome)* |
| Static | The Static pays double Silver; it also hits harder |
| **Ascension** | +1 evolution slot — enables two evolved weapons |

Loadout is **limited to 4 of the pool** (widened by quests), so the grind is a *build-identity*
choice, not raw power creep. Each tome levels 10 ranks, cost `100 × 1.6^level` Silver — the
safety-valve grind: even a bad run funds a fraction of a tier.

### The signature exponential combos (our "Holy Trinity")
- **THE DEATH SPIRAL** — *Cursed × Static × Devoured Sun Shard.* Crank difficulty and rarity into
  the red; the world goes dark, loot goes Legendary, you farm The Static forever.
- **THE SNOWBALL** — *XP × Golden × Duplication.* Level faster → more Gold → more dupes → more
  items → level faster. Exponential economy.
- **THE WHIRLPOOL** — *Orbit × Encirclement × Little Black Hole.* Suck the horde into one point,
  grind them on orbiting death. The smaller the world, the tighter the blender.

### Build archetypes players chase
Kiter · Glass Nova (1-HP god) · Orbital Blender · Proc Machinegun · Night Stalker · Air
Superiority · The Banker · **The Gardener** (load-the-planet). Every archetype has a hero, a
weapon path, an item core, and a tome anchor — the recombination space is the replay engine.

---

# 8 · Worlds, Biomes & Planetary Events

A world is not a skin; it is a **rule that rewrites how you run.** Terrain stays canon — one
analytic noise function (hills + ridged mountains + craters) drives mesh, collision, and props
together; each world just re-weights it and adds **one gimmick system.** This is how we bury
Megabonk's two-map problem.

> **Launch scope:** the 3 built worlds + 2–4 new ship by 1.0; the rest are roadmapped free drops
> (§14). All are canon design targets.

### The Founding Three (deepened — canon)
**THE MOON** — pale regolith, Earthrise fixed over one hemisphere (a soft "north"). *Gimmick —
Earthside/Farside:* the Earth-facing hemisphere is calm and dim-lit; cross the terminator to
Farside and ambient drops, gems glow brighter, elite rate +20%. *Hazard:* collapsing lava-tube
skylights (fall-through pits to a brief cavern lane). *Event — EARTHRISE ECLIPSE:* Earth slides
in front of the sun for 25s, killing flashlight and ambient both; you fight by rim-lit silhouette.
*Boss:* **THE CRATERPILLAR.**

**MARS** — rust dunes, thorn flora, bruise sky. *Gimmick — MIGRATING DUST STORM:* a physical
storm-cell walks the surface; inside, vision is ~8m and ranged enemies can't see you either — a
mobile stealth bubble to ride or flee. *Hazard:* thorn-flora slows on contact. *Event — DUST
DEVILS:* roaming tornadoes fling you (and enemies) into low-gravity float. *Boss:* **JUDGE
ANUBOT.**

**THE DARK MOON** — purple crust, glowing-fungus flora, no sky, just The Static bleeding at the
edges. *Gimmick — THE CRAWL:* the planet is faintly translucent, so you *see The Static massing
through the ground* on the far side and know where it'll erupt. *Hazard:* fungus that detonates
spore-clouds. *Event — WHITEOUT:* for 15s the horizon fills with static-snow and every astronaut
you've lost this run walks once across the sky. *Boss:* **THE HOLLOW COSMONAUT** *(the Dark Moon's
stage boss — distinct from the campaign superboss, see §15)*.

### Nine new worlds (design targets)
| # | World | Feel | Signature gimmick | Boss |
|---|---|---|---|---|
| 4 | **PEBBLE** | Marble-tiny, white-dwarf lit | Lap it in ~6s — your own piercing/boomerang shots come back and hit foes behind you | THE HAND (tries to pick the planet up) |
| 5 | **THE SUN'S PIMPLE** | Blinding molten | You fight standing **on a tiny sun**; flashlight useless, solar flares as expanding rings | SOLARIS (a face in the plasma) |
| 6 | **HOLLOW** | Cave-glow teal | Drop through the crust to the **inner surface** (gravity points outward) | THE INNER TWIN (mirror of your hero) |
| 7 | **SATURN JR.** | Gold gas-deck | No ground — walk **sinking condensation islands** | THE EYE (a Great-Red-Spot storm) |
| 8 | **THE SHARD** | Cracked obsidian | A **shattered ring-world** of floating arcs + jump-pads (Chimp-O's paradise) | THE KEYSTONE |
| 9 | **BRRRR-9** | Cyan ice, aurora | **Frictionless** — you glide, never stop; positioning is chess | THE ZAMBONI |
| 10 | **THE COMPOST** | Sickly green | A **living planet** — killing enemies grows terrain in real time; over-farm and you wall yourself in | THE ROOT (the world is the boss) |
| 11 | **RINGWORM** | Chrome + debris haze | A debris ring **rains telegraphed meteor-lines**; read the shadows | THE COLLECTOR |
| 12 | **NEW EARTH?** *(secret)* | Fake-blue idyllic | Looks exactly like home for 3 minutes — then the sky pixelates into Static: it was **The First One** wearing home as a mask | → the finale |

Signature event highlights: **CORONAL LOOP** (Sun's Pimple lifts a stripe of terrain + enemies off
the surface), **CORE PULSE** (Hollow nulls gravity on both surfaces), **REALIGNMENT** (Shard arcs
rotate into new layouts mid-run), **AURORA CURTAIN** (Brrrr-9's lights descend as a damaging
light-wall), **BLOOM** (Compost flowers into healing pods + pollinator swarm). Plus **THE PALE
DOT** — Voyager's carried-off "pale blue dot," a one-screen pinprick reached only via a Legendary
teleporter roll; brutal density, best Silver/min in the game.

### Campaign map — how worlds chain
Teleporter destination is a **branch** at each boss kill (Luck widens options). Rule:
**hotter/weirder deeper** — the chain walks from the calm home Moon toward the devoured heart, so
the melancholy compounds exactly as the dopamine spikes.

```
T1  Moon
T2  Moon → { Mars | Pebble }
T3  Moon → Mars → { Dark Moon | Hollow | Brrrr-9 }
T4  Moon → Mars → Shard → { Sun's Pimple | Saturn Jr. }
T5  Moon → Mars → Compost → Ringworm → Dark Moon
T∞  any → NEW EARTH? (secret) → THE FIRST ONE
```

Miss a boss anywhere and **The Static swallows that world** instead (canon). **NEW EARTH?** is
unlocked by finishing a run with every dead-astronaut cosmetic collected — the cool-sad payload of
the whole game.

---

# 9 · Enemies, Elites & Bosses

On a sphere there is no back line — threats crest the horizon from **every azimuth at once.** The
bestiary is designed around reading a full 360° encirclement: each enemy answers "where is it safe
to stand *right now*?" differently. Death is almost always the player getting **pinched between two
arcs of the circle.** Canonical live enemy cap: **1,200** (overflow merges into The Static).

### The built eight (canon backbone)
Shambler · Sprinter · Bruiser · Spitter (ranged lob) · UFO (hover zapper) · Burrower (erupts under
you) · Beamer (long-range aim-line railbolt) · Lobber (mortar AoE telegraph) — plus **The Static.**

### New enemies (behavior-first)
| Enemy | World | How it abuses the sphere |
|---|---|---|
| **Rollo** | Moon | Curls into a ball and *rolls the great-circle* toward you, accelerating downhill; can't turn sharply — bait it into a crater wall |
| **Splitshroom** | Dark Moon | Bursts into 3 Sporelings on death that reseed the ring faster than you can clear it |
| **Aegis Drone** | Mars | Front shield-cone toward you; run the long way around the curve to flank |
| **Chorus Node** | Dark Moon | Buried summoner that pulses adds on a 4s beat; a priority target hiding below the horizon line |
| **Sunskimmer** | Mars | Kamikaze diver, visible ~1.5s as it crests; at night you hear the whine first |
| **Orbiter Wisp** | Dark Moon | Never approaches — *orbits you at fixed radius* like a tiny moon, firing inward |
| **Graviton** | Dark Moon | Warps local gravity into a downhill well that drags you toward the horde. The purest USP enemy |
| **Beacon Tick** | Any | Harmless-looking; a touch plants a tracker that paths off-screen enemies to you for 6s |
| **Longshot Beamer Prime** | Mars | Elite sniper that leads its target and fires *over the curve* — cover is a hill's shadow, not distance |
| **Trencher** | Moon | Burrows a visible ridge toward you, then an uppercut launch — punishes staying grounded |
| **Tidewalkers** | Any | Slow giants that move only on the night side; sunrise petrifies them into destructible cover |
| **Mimic Chest** | Any | Disguised as loot; opening triggers a 360° shockwave and it flees. Teaches greed-punishment |
| **Static Herald** | Static | A giant slow cosmonaut that *emits silence*, muting your SFX cues so you fight the swarm deaf |

> **Miniboss gap (canon TODO):** the 7:00/2:00 cadence needs **two minibosses per world**; new
> worlds inherit Craterpillar Jr / Rover Gone Wrong until bespoke pairs are authored (see §15).

### Elites — the "Glitched" affix system
Elites are larger, glowing, loot-guaranteed variants rolled with **1–3 stacked affixes** (scaling
with Difficulty/Cursed), each **telegraphed by aura color**:

| Affix | Aura | Effect | Counterplay |
|---|---|---|---|
| Overclocked | white | +60% move & atk speed | Kite, don't trade |
| Leaden | brown | Huge HP, gravity-well on death | Bait the well away |
| Warden | cyan | Regenerating front-shield | Flank the curve |
| Contagious | green | Splits into 2 on hit-thresholds | Burst, don't chip |
| Magnetar | violet | Pulls your projectiles off-course | Use seekers/piercers |
| Nightborne | deep blue | Invulnerable by day, doubled at night | Fight it at dawn |
| Meteoric | orange | Hurls itself skyward, slams a ring | Watch the shadow |
| Cursed-Touched | black-gold | Drops a Legendary, summons a mini-Static on death | High risk, high loot |

Two-affix combos breed nightmares — *Overclocked Meteoric* ("the pogo goblin"), *Warden Magnetar*
("the unhittable turtle") — giving veterans build-test puzzles.

### Boss design philosophy
Bosses are **multi-phase, hard-telegraphed, and arena-aware on a sphere**: every signature move
exploits that you can't leave the planet and the boss can see all the way around it. Phases
escalate by **shrinking safe geometry** — pole to equator, day to night, whole surface to one lit
ring. Telegraphs rise over the horizon (light columns, ground-cracks that wrap the globe).

- **THE CRATERPILLAR (Moon)** — P1 Segmented Chase (a moving wall you outrun/hop), P2 Burrow Bloom
  (erupts from all compass points), P3 Helmet Choir (segments detach into homing helmets). Its
  body can wrap the whole planet, so "run away" loops you into its tail.
- **JUDGE ANUBOT (Mars)** — P1 Verdict Beams (rotating lighthouse railbeam; hide in a hill's
  shadow), P2 Sandstorm Court (Aegis Drones + a wrapping dust wall), P3 Final Judgment
  (Beamer-Prime pillars at the poles cross-firing the equator).
- **THE HOLLOW COSMONAUT (Dark Moon)** — the first human to reach the Dark Moon, still transmitting
  to a dead Houston. P1 Long Walk (fight only on the night side), P2 Static Bloom (tears a rift
  dragging The Static in early), P3 Void Collapse (gravity inverts on his beat, pulling you toward
  the antipode mid-fight).
- **PROSPECTOR-9 (Mars alt)** — Doug's lost union brother; drills shafts that vent lava rings, P2
  flips to the far side so you chase across the pole.

### The finale & the secret superboss
- **THE FIRST ONE** — the campaign-ending antagonist (the original 2029 copy that kept printing),
  reached via **NEW EARTH?**; you fight it *from the outside of your own planet.*
- **THE DEVOURER ("It Ate The Sun")** — the superboss, The First One's true devouring form, a
  planet-sized maw the arena orbits *inside.* Unlocked by clearing Dark Moon T3 with Cursed Tome
  active and surviving 2 minutes of The Static without leaving the lit ring. Three phases: **Tide**
  (the horizon rises as a wall of teeth), **Famine** (it eats your gold and half the terrain,
  shrinking walkable surface), **Last Light** (day/night stops; you fight on one flickering ring as
  everything you've ever unlocked spawns at once). Kill it and the epilogue is quiet: the sun, for
  one screen, comes back on.

---

# 10 · Meta-Progression, Economy & Retention (Anti-Churn)

**The one rule: no run ever pays out zero.** Win, die, or dissolve into The Static, you always bank
Silver, tick a quest, and reveal a Codex entry. Megabonk lost ~60% of players by week 3 because
runs 40–200 felt identical to run 39. Our engine keeps the dopamine schedule from flatlining —
small guaranteed drips, medium surprise spikes, long-horizon mastery goals, all layered.

### The Silver economy & Unlock Web
**Gold** stays in-run; **Silver** is the meta spine, earned via a transparent formula so the next
unlock always feels close:
```
Silver = (survival_seconds / 6) + (kills / 4) + (boss_kills × 150)
       + (tier_bonus × 50) + Static_overtime_seconds
       × (1 + GoldenTome×0.05) × (1 + CursedMoonRock×0.15)
```
Silver flows into a **branching Unlock Web** drawn as the dead solar system you "reclaim" — buying
a node reveals its neighbors, so there's always a visible next thing (e.g. *Second Rocket* 1,200 →
co-op; *Chimp Recovery* 2,500 → Chimp-O; *Deep Signal* 9,000 → Dark Moon chaining; *Ascension
Relay* 15,000 → the difficulty ladder).

### Quests — the unlock engine (Megabonk's genius stroke)
**Clearing the quest IS the content unlock.** Six categories on a cosmonaut-graffiti "mission
wall": Milestone, Hero Trials, Build Puzzles, Planet Lore, Cursed (biggest payouts), Hidden
(revealed only on completion). **Launch target ~50 quests; long-tail 120+.** Named examples:

| Quest | Requirement | Reward |
|---|---|---|
| *One Small Step* | Finish any T1 run | 300 Silver |
| *Around The World* | Circle a planet fully without stopping | Rocket Boots unlock |
| *Encircled & Fine* | Survive a 100-enemy full-horizon ring 30s | 800 Silver |
| *Comet ×50* | Land a 50-tail Comet Combo | Cosmetic comet-trail |
| *Nightshift* | Kill 300 on the dark side | Flashlight glow skins |
| *Anubot Humbled* | Beat Judge Anubot no-hit | Doug skin: *Bad Day Miner* |
| *Chimp Ascendant* | Chimp-O reaches +6 jumps | Boomerang evolution early |
| *Devoured* | Read all 9 Dark Moon lore fragments | **The Copy (NG+)** unlock |
| *Glass Rocket* | Win at 1-HP with no armor items | Legendary chest key |
| *The Long Dark* | Endless mode wave 50 | Ascension Depth V key |
| *Ghosts Remembered* | Read every dead-cosmonaut nameplate | Codex 100% badge |

*(Note: the true ending / NG+ has a **single** trigger — the 9 Dark Moon fragments via* Devoured.
*Codex 100% is a separate completionist badge, not the ending — see §15.)*

### The Retention Layer
1. **Ascension — "The Descent Ladder"** (10 stacking tiers, renamed **Ascension Depth I–X** to keep
   "Tier" meaning only chain length): *Thin Air* (+25% enemy speed) → *Long Night* (2× nights) →
   *Hungry Horizon* (spawn closer) → *Iron Ghosts* → *Shrinking World* (radius −15%) → *Gold
   Drought* → *Two Bosses* → *Gravity Sickness* → *The Watching* (an invincible orbiting stalker) →
   *Absolute Zero Hour* (all of it + hidden timer). Each clear = a permanent star + Silver
   multiplier. The hardcore endgame that kept RoR2 alive.
2. **★ Daily Seeded Planet — "GRIEF-7b, Tuesday"** *(promoted to a headline retention primitive).*
   One shared procedurally-named world per day, fixed seed, three lives, leaderboard by Silver.
   Everyone runs the *same tiny sphere* — same crater rim, same north-pole pileup at 7:00 — so it
   generates a specific, shared water-cooler conversation the flat-map competition structurally
   cannot. Ghost-replays of the top 3 orbit as translucent racers.
3. **Endless — "Forever Alone."** Decline the teleporter and stay; The Static intensifies
   infinitely, Silver flows, a wave counter drives one-more-run. Lore hook: you *chose* not to go
   home.
4. **★ NG+ — "THE COPY"** *(the scanner-twist paid off as a mechanic).* After the true ending, the
   Static **learns and mirrors your evolved builds back at you** — every ghost runs a real prior
   loadout, difficulty derived from *your own best runs.* Fighting your own optimized build is the
   endgame Megabonk can't touch. Everything comes back around — literally, at you.
5. **Character Mastery & Skins.** 10 mastery ranks per hero (kills-as-that-hero) → recolors,
   procedural hat meshes, and a rank-10 **Mastery Perk** (e.g. Valentina's laser gains a free
   ricochet). All in-code mesh/palette swaps — **zero asset cost.**
6. **Weekly Mutator — "This Week On A Dying Star."** A rotating global rule from a pool (*Boomerang
   Week*, *Everything's On Fire*, *Big Head Enemies*, *Reverse Gravity Tuesdays*) — free feed
   refresh via flag toggles.
7. **Codex & Collection.** Every enemy/boss/item/weapon/hero/world gets a card — flavor on front,
   **cool-sad lore fragment on back**, revealed by encountering it. Each Static ghost has a real
   name and a one-line epitaph. The quiet, non-competitive path for lore-hunters.

### Psychological hooks & anti-burnout
Variable-ratio dopamine (guaranteed Silver + surprise Legendary/quest pops); goal-gradient (the
Web always shows a half-filled next node); sunk-cost softener (loss still banks progress); identity
investment ("my Chimp-O"). **Dev anti-burnout is load-bearing:** every retention system reuses
*existing procedural systems* — skins are palette swaps, dailies are seeds, mutators are flags,
quests are counters on data already tracked. The whole long tail ships as **configuration, not new
assets** — how ASTROBONK outlasts week 3 where Megabonk didn't.

---

# 11 · Co-op & Multiplayer (The Killer Differentiator)

Megabonk is a solo experience. **ASTROBONK is a place you go with three friends to watch the
universe end together.** That's the whole moat, and the tiny-planet USP is a co-op *gift*: on a
sphere the size of a soccer field, four players are **always visible to each other** — you see
Valentina's laser strobing over the north pole, watch Doug get swarmed on the dark side and have
to *decide* whether the run around the planet is worth abandoning your quarter. Distance is
dramatic because the world is round and small. No flat survivors game can copy it.

### Why the sphere makes co-op sing
- **Convergence & split are real tactics** — spread to the four poles and the encircling horde is
  quartered; stack up and you concentrate firepower but share one ring of death.
- **You can literally see the plan** — no minimap; glance across the curve for your squad's health
  bars and flashlight cones.
- **Planet events hit everyone differently** — being on the wrong side of a dust storm or the night
  terminator is now a *social* problem.

### Scaling
| Players | Spawn count | Per-enemy HP | Boss HP | XP |
|---|---|---|---|---|
| 1 | 100% | 100% | 100% | 100% |
| 2 | 175% | 110% | 165% | shared pool |
| 3 | 240% | 120% | 225% | shared pool |
| 4 | 300% | 130% | 285% | shared pool |

More players means the encirclement ring is *fuller*, not just faster; boss HP scales sub-linearly
so a coordinated squad still gets a fast-kill dopamine hit.

### Down & revive on a sphere
At 0 HP you become a **Tumbling Beacon** — a ragdoll that rolls "downhill" along the terrain
gradient (that same noise function now drives your corpse) and fires a vertical distress flare
visible from anywhere on the planet. A teammate stands in your radius 3s to revive (Rocket/Trampoline
builds make the cross-world rescue viable). A **Static Meter** fills while down; if it caps first,
The Static claims you until the next teleporter (rejoin at 50%). **You cannot self-revive** — co-op
is load-bearing by design. Reviving grants the rescuer +20% move speed for 5s ("Hero's Adrenaline").

### Loot split — deliberately asymmetric
- **XP: shared pool, individual level curve** — everyone's gems feed one bucket; each picks their
  own cards. Kills the kill-steal resentment; keeps builds personal.
- **Gold: individual** — encourages spreading out to grab the perimeter.
- **Chests: contested-but-generous** — a card drops for *everyone*, but the opener rolls at +1 Luck.
- **Legendary elite drops** go to the killing blow — a funny bragging-rights economy.

### Friendly fire: **off for damage, ON for physics**
You can't hurt teammates, but knockback, Trampoline pads, Cryo slow-fields, and Tesla arcs *do*
affect them — accidentally boop a friend off a spire or freeze them mid-dodge. Chaos without
griefing. A Cursed Moon Rock owner broadcasts their +difficulty aura to the whole squad, so
bringing the cursed build is a *group-consent* meme.

### ★ Opposite-pole ultimates — authored co-op set-pieces
One signature two-pole ultimate per weapon family, surfaced with screen-wide VFX and a results
callout — the coordinated money shot streamers chase. The flagship: **STATIC CASCADE** — two Tesla
owners on opposite poles chain their STORM CORE arcs toward each other, briefly wrapping the *whole
sphere* in a lightning belt. Plus named duo combos surfaced on the results screen: **Deep Freeze
Protocol** (Yuki's frost + Doug's DEATH RAY shatters chilled ranks), **Magnet Circus** (Chimp-O
herds with SATELLITE ARRAY, Valentina melts the knot), **Rivet & Rescue** (B0-NK suppresses, Buzz
tanks with thorns). These make co-op *aspirational*, not just "more players."

### Netcode — authoritative host + client prediction
Lockstep stalls the squad on one player's hitch; rollback re-simulating 1,200 enemies on mispredict
is a CPU fire — both wrong for us. We ship **authoritative host** (peer-hosted; dedicated servers
post-1.0). The host simulates all enemy AI, spawns, and the noise-driven world; clients send only
*input intent* and locally **predict their own astronaut** with reconciliation — the one thing that
must feel crisp. Because auto-attacks are automatic, the classic "did my shot hit" latency pain
mostly evaporates. Enemies stream as **compact quantized batches with interest management** — full
fidelity for the horde on *your* arc, coarse updates for the far side you can barely see. *The
sphere is a bandwidth gift: the far horizon is naturally low-detail.*

### Onboarding, phasing & drop-in *(filling a canon gap)*
The **teleporter lobby** between chained planets is the natural join/shop/heal/re-spec screen and
the co-op join point. First co-op session gets a designed moment: a friend's Beacon **drops from
orbit** onto your dead solar system at half the squad's average level; a joined-mid-stage player's
astronaut auto-fights on autopilot for 30s (bathroom-break grace). **Realistic phasing:** 2-player
at Early Access, **4-player by 1.0**; dedicated servers, cross-play, and matchmaking post-1.0.

### Stretch — VERSUS: *Bonk Royale*
Two squads, one planet, opposite poles, the *same* shared horde. Killing enemies feeds a **Static
Cannon** you aim at the enemy team — dump your horde onto their side and watch them drown in the
swarm you sent. Last team standing at The Static wins. Post-1.0, cosmetic stakes only — the PvE is
the heart; this is the meme tournament mode.

---

# 12 · Art Direction, VFX & Audio

ASTROBONK looks like **a chunky diorama toy that happens to be a dying solar system.**

### Visual pillars
1. **Chunky low-poly meets cosmic scale** — faceted, flat-shaded heroes/props (a wrench is
   *readably* a wrench at 3m and 300m) standing on the shoulder of something enormous and sad
   (curving horizon, star-dense sky, Earthrise, a distant devoured sun). *The contrast is the art
   direction.*
2. **Silhouette-first, always** — at 1,200 enemies the screen is a churning ring; nothing survives
   but **silhouette + one color signal.** Every enemy owns a distinct body-shape and a single
   emissive accent; heroes get an exaggerated shape + a **hero-color rim light** so you're never
   lost in your own swarm.
3. **Vertex-color biomes, procedural as a STYLE** — zero external textures is the aesthetic, not a
   constraint we hide. Terrain color bakes per-vertex from the *same* noise that drives mesh +
   collision (elevation/slope/crater-mask → per-world gradient). Reads as **handmade papercraft
   planet**, costs nothing.
4. **Everything juices, nothing clutters** — a particle that doesn't tell the player something
   (I hit that / that hit me / that's about to explode) doesn't spawn.

### Color & lighting per world
| World | Palette | Key light | Night | Signature |
|---|---|---|---|---|
| **Moon** | Bone-white, ash-grey, warm tan | Hard white sun, crisp shadows | Blue-black, high stars | **Earthrise** casts cyan fill on the night side |
| **Mars** | Rust, dried-blood red, ochre | Dim amber haze | Muddy brown, low stars | **Dust storms** desaturate the frame to sepia, cut draw distance |
| **Dark Moon** | Bruise-purple, void-black, teal/magenta | Almost none | Near-black; the fungus IS the light | Glowing flora/accents are the *only* readable color |

**Day/night + flashlight:** ambient is deliberately dim so night is *dark* — on the Dark Moon the
flashlight cone is the primary light source 70%+ of the time. The darkness is a difficulty knob and
a mood, simultaneously. Muzzle flashes briefly light terrain (pooled, hard-capped point lights).

### VFX language — "big signals for the player, tiny signals for the horde"
- **Hit-flash:** every damaged enemy flashes flat white ~60ms via a vertex-color override — one
  uniform, no particle, scales to thousands for free. *The single most important readability tool.*
- **Crits:** flash goes *yellow* + hitstop micro-freeze + a chunky floating number (white normal /
  gold crit / cyan DoT), pooled and size-capped.
- **Deaths:** pop into 3–5 faceted shards + a puff, despawn instantly. No corpses (readability +
  perf). Elites burst into a small firework and a gem fountain.
- **Gem vacuum:** the dopamine hose — gems streak in on a curved arc with a rising sparkle; a
  full-planet vacuum on level-up sucks the whole visible ring in at once.
- **Evolution fanfare:** the highest-value moment — screen desaturates, a gold shockwave ring
  expands, the model swaps with an "assembly" pop and a bass hit.
- **Boss telegraphs:** authored **on the terrain as red decals** (mortar rings, aim-lines,
  slam-zones) — because they're on the sphere they curve over the horizon, so you see danger arcing
  toward you. **Red is reserved exclusively for danger.**
- **The Static's own identity:** film-grain static, scanline flicker, and desaturation creeping in
  from the edges — the world literally degrading into signal noise.

### UI/HUD identity — "chunky mission-control retro"
Thick rounded panels, NASA-patch motif, big legible mono-ish numerals. Rarity color-coded
everywhere (grey/blue/purple/gold) with matching card borders + a rarity-riser sound. The level-up
screen is the visual anthem: three fat cards, hover-tilt, rarity glow, weapon/item rendered as the
actual chunky 3D model on a turntable. HUD stays cornered mid-run — the planet is the star.

### Audio
**Procedural-synth SFX (canon):** all SFX are in-code WAV synthesis — a coherent **toy-laser /
retro-future** identity (square-wave lasers, noise-burst impacts, a pitched "BONK" on melee — the
title is a promise). Every SFX is parameterized so rarity/crit/evolution reuse the same synth with
shifted pitch/brightness. Hard voice-cap + priority ducking keeps 1,200 enemies from becoming white
noise.

**Music (per world):** Moon = warm nostalgic **synthwave** ("we came from here"); Mars = driving
**desert-rock / industrial**; Dark Moon = **dark-ambient dungeon-synth** (droning, arrhythmic,
wrong). **Adaptive:** stem-layered, intensity rising with on-screen enemy density — the horde
literally scores itself; miniboss/boss triggers a combat layer; when **The Static** hits, the music
degrades in lockstep with the VFX (pitch-drops, detunes flat, a heartbeat sub-bass takes over —
beautiful becoming broken).

**Meme-charm touches:** a hero quip-blip on spawn (Chimp-O screech, B0-NK dial-up chirp), a cartoon
BONK on Buzz's wrench crit, a greasy Shady-Guy jingle, a faintly stupid **"ba-DUMP"** level-up you'll
crave, and a four-note evolution brass sting that's 60% heroic, 40% joke. Sad in the music; dumb
grins in the SFX.

---

# 13 · UX, Game Feel & Accessibility

Game feel is our moat — chunkier than Megabonk while staying legible when forty enemies crest the
horizon at once.

### The screenshake budget
Screenshake is a currency, hard-clamped so a Static swarm can never blur the horizon into vomit.
Every frame has a budget of **1.0**; contributions stack and clamp. `offset = trauma² × maxOffset`,
so low trauma is nearly invisible and only real threats punch.

| Source | Trauma | Notes |
|---|---|---|
| Basic weapon fire | **0.00** | Auto-attacks NEVER shake |
| Enemy hit landing on you | 0.12 | The one that matters |
| Elite/boss slam | 0.22–0.35 | Directional kick toward impact |
| Evolution unlocked | 0.25 | Celebratory, one-shot |
| Craterpillar burrow-erupt | 0.40 | Warns it's coming through the crust |

**Hard clamp: total camera offset ≤ 1.8° pitch/yaw** — lose the curve and you lose the planet.

### Hitstop
**Only on *your* kills, never chip damage.** Killing blow freezes both actors **50ms** (elites 90,
bosses 130 + a 0.15s dilation to 0.85×). Evolved weapons get **+20ms** so a MEGA WRENCH kill feels
categorically heavier than a Wrench kill.

### Damage numbers — readouts, not confetti
Normal (small white), crit (1.6× **gold**, bold/shape-coded so colorblind players read it without
color), big-crit (a one-frame flash on the number, not the screen). **Number merging:** at high
counts, numbers within 0.3m/0.1s **coalesce into a running sum** — the single biggest readability
win over the genre's number-blizzard. DoT/thorns desaturated and drifting sideways. Toggle: Full /
Merged-only / Crits-only / Off.

### Knockback & the camera-stability law (canon from playtests)
Enemies take gentle knockback; **the player is never knocked back by chip damage** — involuntary
displacement toward the horizon is disorienting and unfair. Only boss slams push you (capped 1.5m,
always *tangent*, never toward the poles). The camera's up-vector slerps toward the surface normal
at a fixed max rate, decoupled from velocity; terrain pitch smoothed over several frames. **The
world curves; the frame does not lurch.**

### Controller rumble
Mirrors the shake budget on separate motors: low-freq = getting hit, high-freq = pickups/level-up.
Boss telegraphs give a **rising rumble ramp** 0.4s before a slam — a haptic tell that reads even
when the boss is on the planet's night side. Default 70%, full slider.

### HUD & the level-up flow
HUD hugs the edges so the center — where encirclement happens — stays clean: **top-center** timer +
threat tier; **bottom-left** HP orb + XP bar; **bottom-right** loadout icons with level pips;
**off-screen threat arrows** ring the edge showing where Beamers aim and where the boss is below the
horizon (essential on a sphere). Level-up **pauses**; cards show **evolution-ready glow** the instant
a weapon hits L7 with its paired item — no menu-digging.

### Onboarding — learn by doing
No wall of text. First run auto-selects **Buzz on the Moon**, taught through diegetic radio-crackle
from a dead-serious Mission Control handler: "You're on a rock, Buzz. Walk it off." (movement) →
"Your wrench swings itself. Just don't get cornered." (auto-weapons) → first level-up soft-arrow
(cards) → "It gets dark on the far side. Bring a light." (day/night + flashlight) → 7:00 miniboss
with highlighted threat arrows. Every deeper system unlocks via a quest with a one-line tooltip.

### Input & control map (KBM / gamepad / Steam Deck)
| Action | KBM | Pad |
|---|---|---|
| Move | WASD | Left stick |
| Camera | Mouse | Right stick |
| Jump (+extra) | Space | A / South |
| Slide / Dash | Shift/Ctrl | RB / R1 |
| Interact (chest, teleporter, Shady Guy, Microwave) | E | X / West |
| Toggle flashlight | F | D-pad Up |
| Card: pick / banish / refresh / skip | LMB / 1 / 2 / Esc | A / X / Y / B |
| Pause | Esc | Start |

Steam Deck ships at a locked 40fps target with trackpad-camera fallback and D-pad card nav. Aim
assist (soft camera magnetism toward nearest threat) default-on for pad, off for mouse, adjustable.

### Accessibility — humane by default
Because our aesthetic leans on bloom, glowing elites, and a genuinely dark night side,
photosensitivity and legibility get first-class treatment:
- **Colorblind-safe telegraphs** — every danger is *shape + motion + color*, never color alone
  (mortar = pulsing ring, aim-line = animated dashes, Burrower = cracking decal); Deut/Prot/Trit
  palettes + a high-contrast "danger = white outline" mode.
- **Screenshake slider** (0–100%) and an **independent flash-reduction toggle** (kills the evolution
  white-flash, clamps bloom).
- **Photosensitivity mode** — disables Storm Core strobe, softens Death Ray bloom, throttles Static
  particle flicker to <3 flashes/sec (WCAG-informed).
- **Full remap** on every input including the cards.
- **Difficulty as options, not menus** — independent sliders (enemy density, enemy damage, a "one
  more chance" revive-token toggle) layered *on top of* the canon Difficulty/Cursed systems, so
  accessibility never touches leaderboard integrity (accessibility runs flagged separately, still
  earn Silver).
- **Readability** — UI scale 75–150%, damage-number size, minimum enemy-outline thickness, a
  "reduce clutter" mode that merges distant silhouettes.

Every setting persists per-profile and is reachable **mid-run from the pause menu** — the most
humane accessibility feature is not making someone quit their run to fix the thing hurting their
eyes.

---

# 14 · Production Roadmap & Launch Strategy

### The two boulders
Everything orbits two masses heavy enough to sink the project: **content scale** (~40% of
Megabonk's volume today — and "too little" was Megabonk's most-cited flaw, so 40% is a grave) and
**co-op** (the headline differentiator and the single largest engineering risk in a Bevy horde
game). We sequence so these are **never in flight at once** — content burnout and netcode burnout
would compound.

### Milestone roadmap: v0.1 → 1.0
| # | Milestone | Codename | Focus | Exit criteria |
|---|---|---|---|---|
| M1 | Foundations Hardening | *Solid Ground* | ECS refactor, fixed-timestep sim, deterministic RNG, perf instrumentation | **1,200 enemies @ 60fps** on mid GPU; sim tick decoupled from render |
| M2 | Content Scale I | *Fill the Sky* | **6→12 heroes**, 10→16 base weapons, 22→~40 items, 3→5 worlds | Every hero has a paired weapon + evolution; no dead build slots |
| M3 | Planet-as-Arena Events | *Bad Sky Day* | Meteor showers, eclipses, gravity flips, terminator hazards | 2 signature events per world; all readable at horizon range |
| M4 | Co-op Vertical Slice | *Two Astronauts* | 2-player online, host-authoritative, one world | 2p full run stable at ~800 enemies over a home connection |
| M5 | Co-op Scale-Out | *Full Crew* | 4-player, all worlds, shared-XP tuning, revive, drop-in, opposite-pole ults | 4p run to boss; no desync on the Static |
| M6 | Content Scale II | *Deep Space* | worlds 6–7, elite/miniboss variety, **quests → ~50** | Meta unlock curve spans ~25h to full launch roster |
| M7 | Retention Layer | *Come Back Tomorrow* | daily seed, weekly mutator, Ascension ladder, NG+ "The Copy," cosmetic Silver sink | Anti-churn loop live and instrumented |
| M8 | Polish & LC | *Wax On* | juice pass, onboarding, accessibility, localization, Deck verified | RC stable across a 200-run soak; crash-free ≥99.5% |

M1–M3 are the "beat Megabonk on content and spectacle" arc → the **free demo.** M4–M5 are the
co-op boulder, walled off after foundations are proven. M6–M7 are the "why it retains" arc. M8 is
the runway.

### Scope / cut line
- **In 1.0 (non-negotiable):** ~12 heroes, 16 weapons + evolutions, 5 worlds with arena events,
  4-player co-op, daily/weekly retention loop + NG+, the quest unlock engine, Steam Deck, controller
  parity.
- **Post-launch (free, publicly promised):** worlds 6–14 (incl. Sun's Pimple, Hollow, Shard,
  Compost, NEW EARTH?, Pale Dot), hero packs to 21, boss-rush, PvP *Bonk Royale*, Workshop mods,
  cross-play, seasonal cosmetic events.
- **Cut without mercy if slipping:** per-hero voice barks (text is fine), the gravity-flip event
  (highest bug surface), any world beyond #5. **Co-op is never the cut** — losing it makes us
  Megabonk-*minus*; losing world #5 makes us Megabonk-*plus.*

### Risk register
| Risk | Sev | Mitigation |
|---|---|---|
| **Perf at 1,200+ enemies** | High | ECS archetype batching, sphere-surface spatial hash, GPU-instanced enemy meshes, LOD-by-horizon (off-camera hemisphere runs cheap AI), hard cap with "merge into Static" overflow valve. Budget enforced from M1. |
| **Co-op netcode** | High | Host-authoritative + client prediction, deterministic fixed tick (built M1 *to enable this*), local-player reconciliation only. Ship 2p before 4p. Fallback: degrade to lockstep for ≤4 on the small map — movement, not aim, is the skill, so latency is survivable. |
| **Content burnout** | Med-High | Procedural-first pipeline is the moat — a new world is a noise function + palette + one boss, not an asset order. Content batched (M2, M6) so creative and engineering gears don't grind continuously. |
| **Scope creep** | Med | The cut line is contractual; new ideas file as post-1.0 cards by default. |
| **Humor misses** (Megabonk's own flaw) | Med | Names/flavor are *data, not code* — cheap to iterate; A/B community voting on names during the demo. |

### Launch strategy (Vedinad playbook, adapted)
1. **Steam page up early** (end of M3) with a wishlist button; the **co-op reveal held in reserve**
   as a mid-campaign spike.
2. **Free demo** = M1–M3 build (3 heroes, 3 worlds, arena events, single-player). The demo's job is
   the *screenshot*, not the full game.
3. **Meme trailer**, vertical/clip-native first. Cold open: an astronaut sprinting a full lap with a
   **comet-tail of 900 enemies wrapping the horizon** → cut to eclipse-darkness + one flashlight
   cone + a perfect Death Ray lap-kill → the sad-lore stinger.
4. **Wishlist grind** timed to Steam Next Fest (demo live *for* the Fest).
5. **Co-op announce** ~2 weeks pre-launch: *"Now bring a friend to the dead solar system"* — the
   differentiator Megabonk cannot answer without a rebuild.
6. **Launch timing:** avoid a Megabonk-sequel window; target a quiet Steam week, **$9.99–$12.99**,
   10% launch discount.

### Why the footage is inherently clip-able
The **Comet Combo lap-kill** (a self-contained 6-second clip with a punchline); **horizon
encirclement** (the wall-of-enemies-curving-over-a-tiny-world shot that exists nowhere else);
**terminator-line drama** (bright day → pitch-black night + one flashlight cone); **physics gags**
(Chimp-O boomeranging an antenna around the whole planet into his own face); **co-op chaos** (four
astronauts on a marble the size of a house — a screenshot that explains the game with zero text).
Every one is a thumb-stopping vertical clip with no editing skill required — the USP does the
framing.

### Post-launch cadence & Workshop
Weekly seeded mutator; monthly balance + one hero *or* weapon-evo; quarterly free content drop
(world/boss/hero pack, telegraphed to fight the week-3 cliff). **Workshop:** because worlds are
*already just data* (noise function + palette + spawn table), mod support is a serialization task,
not an engine rewrite — a structural advantage over asset-heavy competitors.

### Success metrics
| Metric | Floor | Target | Stretch |
|---|---|---|---|
| Wishlists at launch | 25k | 75k | 200k |
| Week-1 sales | 15k | 60k | 250k |
| **Week-3 retention** (Megabonk lost ~60%) | 45% | **55%** | 65% |
| Median session | 25 min | 40 min | 55 min |
| Co-op run share | 20% | 35% | 50% |
| Positive review rate | 85% | 92% | 96% |
| Daily-run participation | 10% DAU | 20% DAU | 30% DAU |

**The single metric that decides beat-vs-match: week-3 retention.** Content scale gets players in;
the tiny-planet spectacle and co-op are what make week three feel different from week one.

---

# 15 · Canon Ledger & Glossary

*The reconciled source of truth. When sections above and this ledger disagree, the ledger wins.*

### Current build state (v0.1, real & shipping)
6 astronauts (Buzz + Valentina start; B0-NK, Yuki, Chimp-O, Doug unlock) · 10 weapons + 10
evolutions · 22 items · 8 tomes · 3 worlds (Moon/Mars/Dark Moon) · 8 enemies + The Static ·
bosses Craterpillar & Judge Anubot + minibosses Craterpillar Jr / Rover Gone Wrong · 16 quests ·
Gold/Silver economy · save at `%APPDATA%/astrobonk`. Rust/Bevy 0.18, procedural assets. Playtested
& smoke-tested; balance eyeball-only.

### Baseline constants (from code — anchor the scaling formulas here)
- Base **Max HP 100**; base **crit chance 5%**, **crit damage ×2.0**; run speed ≈ **8.5 m/s**.
- **Weapon max level 7**; **evolution cap 1** per run (**+1 with Tome of Ascension**).
- Stage length **10:00**; miniboss marks **7:00 / 2:00**; boss **1:30**; **canonical enemy cap
  1,200** (overflow merges into The Static).
- Level-up economy: **Refresh** 2 free/run then Gold · **Banish** 3 charges/run · **Skip** free
  (small XP + Gold tip). *There is no "Bonk Bucks."* Currencies are **Gold (in-run)** and
  **Silver (meta)** only.

### Reconciled counts (design targets vs 1.0 scope)
| Content | Built | 1.0 target | Full vision |
|---|---|---|---|
| Astronauts | 6 | ~12 | 21 |
| Weapons (+evos) | 10 (+10) | 16 (+16) | 21 (+21) |
| Items | 22 | ~40 | 80+ |
| Tomes | 8 | 23 | 23 |
| Worlds | 3 | 5 | 14 (incl. 2 secret) |
| Quests | 16 | ~50 | 120+ |

### Naming canon (resolving the critique)
- **"Tier"** means **chain length** (T1–T3) *only.* The difficulty ladder is **Ascension Depth
  (I–X)**. Loot rarity is **Rarity Grade** (Common/Rare/Epic/Legendary). The select-screen pick is
  "planet + tier," consistent with chain length.
- **Endgame antagonist canon:** the Dark Moon *stage boss* is **THE HOLLOW COSMONAUT**. The
  *campaign-final* antagonist is **THE FIRST ONE** (the 2029 scanner copy). Its true devouring form
  / **secret superboss** is **THE DEVOURER ("It Ate The Sun")**. NG+ = **THE COPY**.
- **True-ending / NG+ trigger:** the single canon trigger is the **9 Dark Moon lore fragments**
  (quest *Devoured*). **Codex 100%** is a separate completionist badge, *not* the ending.
- **Antipode:** one mechanic — **Antipode Blink.** All other "opposite-pole" effects (e.g.
  Boomerang Insurance) route through it.
- **Death-saves** do **not** stack: at most one resolves per would-be-death, priority Dead Man's
  Tether → Warden Solongo revive → Antipode/Boomerang escape → Widow's Ring 1-HP.
- **Paired evo items may serve multiple weapons** *by design* (e.g. Splitter Chip arms both
  Riveter 9000 and MIRV Pod); with Tome of Ascension this lets one item enable two evo paths.

### The eight "make it legendary" upgrades (adopted, cross-referenced)
1. **Comet Combo** — scored lap-kill mechanic (§3, §14 trailer).
2. **The Static = your real dead runs & friends'/daily** (§2, §10).
3. **NG+ "The Copy"** — the Static mirrors your builds (§10).
4. **The Gardener** — load-the-planet archetype promoted to headline (§6).
5. **Diegetic difficulty** — the world visibly dies as Cursed rises (§3, §12).
6. **Antipode Blink** — the signature high-skill verb + mastery ladder (§4).
7. **Opposite-pole co-op ultimates** — authored set-pieces (§11).
8. **Daily "GRIEF-7b"** — promoted as the water-cooler primitive (§10).

### Open design TODOs (tracked debt)
- Author **2 bespoke minibosses per new world** (new worlds inherit Jr/Rover until then).
- Design the **teleporter lobby** between chained stages (co-op join + shop/heal/re-spec).
- **Leaderboard integrity:** host-authoritative validation for Daily/Ascension; accessibility runs
  flagged separately.
- Trim the **overlapping death-saves / traversal items** per the rules above during content passes.

---

*ASTROBONK — everything comes back around.*
*Living document. Update alongside [PROJECT_STATUS.md](PROJECT_STATUS.md) and
[DEVLOG.md](DEVLOG.md) at every milestone.*
