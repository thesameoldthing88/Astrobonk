# ASTROBONK: Known Issues

The known-issues tracker for the current build of ASTROBONK. It is written for anyone, human or AI, who picks the project up cold.

- **Build covered:** branch `main` at HEAD `99d012f` (committed 2026-07-26). Nothing has been committed since.
- **Tracker date:** 2026-09-26.
- **Status:** items carry their own Status. Items fixed on branch `claude/pensive-keller-0cood4` say `Fixed (<commit>, <package>)`; P30 (the solo/save/world/combat sweep, 2026-09-27) re-verified and closed H1-H3, M12-M14, M16-M20, M22, L1, L3-L9, L14-L16, L24-L30 and L57.
- **Ground truth:** the code in `src/`. Every item in sections 2 to 4 was confirmed by reading `src/` at `99d012f`. Items tagged LIVE were also seen in a real play session: the user's solo run of 2026-09-26, its session log and its `save.json`.

Related documents:

- `GDD.md`: the design canon.
- `DESIGN.md`: the original design and research digest.
- `docs/TECHNICAL_REFERENCE.md`: the architecture reference.
- `plan/`: the full-game plan, maintained separately.
- `PROJECT_STATUS.md` and `DEVLOG.md`: the status document and the history. Both were stale as committed at `99d012f`; 2026-09-26 rewrites were in the working tree when this tracker was written (see L64).
- The NETCODE NOTES block at the bottom of `src/net.rs`: also stale (see L63).

---

## 0. How to read this tracker

### How the list was built

1. **Subsystem reads.** Five independent readers covered world/movement, horde/combat, economy/progression, netcode and the app shell/UI. Each claim was then re-checked against the code by a separate verifier.
2. **Build and smoke run.** A build plus the four headless smokes were run (see §10).
3. **Docket audit.** The co-op plan was audited against the code (see §8).
4. **Spot re-check.** More than 50 of the items were checked against `src/` again for this document: every High and Medium item (H1-H10, M1-M23) and about 25 Low items. For M12, the box winding was re-derived by hand.

No reader claim was refuted outright. Several were re-scoped; §11 lists those corrections so they are not re-reported.

### IDs are stable

| Prefix | Meaning |
|---|---|
| `H` | High severity |
| `M` | Medium severity |
| `L` | Low severity |
| `U` | Uncertain or not reproduced (§5) |
| `D` | Design question, not a bug (§6) |
| `SH` | Ship hygiene before 1.0 (§9) |

- **Never renumber.** When an item is fixed, change its Status to `Fixed (<commit>)` and leave it in place. New items take the next free number in their band.
- **Changing severity.** If an item's severity changes, keep its ID and note the change in the item.
- **Where the numbers come from.** H1-H10 and M1-M23 match the numbering of the 2026-09-26 handoff brief. L-numbers are assigned here.

### Severity

| Severity | Meaning |
|---|---|
| **High** | It breaks progression or save data, panics, or makes co-op wrong in the planned two-PC playtest flow. |
| **Medium** | Clearly wrong behaviour a player will notice, a major co-op gap, or a latent data-loss or perf risk. |
| **Low** | Cosmetic, an edge case, minor co-op polish, dead code or debt. |

Where two verifiers disagreed about severity, the item says so. The higher rating is used unless noted.

### Area tags

`solo` · `co-op` · `UI` · `economy` · `netcode` · `perf` · `dev-hygiene`. An item can carry more than one.

### Evidence tags

| Tag | Meaning |
|---|---|
| **LIVE** | Seen in the 2026-09-26 session log or save. |
| **SMOKE** | Seen in a headless smoke run. |
| **CODE** | The mechanism was confirmed by reading code (two independent reads). It was not reproduced at runtime. |
| **COMPUTED** | Derived from constants or geometry in the code. It was not observed. |

### Line numbers

- **Format.** Code is cited as `path:line` or `path:start-end`, relative to the repo root.
- **Drift.** Lines are for HEAD `99d012f` and will drift. If a line no longer matches, search for the quoted identifier.
- **Old-commit citations.** `35db5ff^:src/...` means the file as it was one commit before `35db5ff`.

### Vocabulary

| Term | Meaning |
|---|---|
| **host** | The listen server. It simulates everything. |
| **joiner** / **client** | A connected peer that only predicts its own body. |
| **host copy** | The astronaut the host simulates on the joiner's behalf. |
| **proxy** | A client-side stand-in for a streamed enemy, boss or pickup. |
| **P** | The party-size factor. |
| **D** | `run.difficulty`. |
| **e** | Seconds elapsed in the current stage (`run.elapsed`). |
| **timer** | The stage countdown (`run.timer`). |

---

## 1. Index

| ID | Title | Severity | Area |
|---|---|---|---|
| H1 | Results and the save bank level 1 and gold 0 | High | solo, economy |
| H2 | Chest opens are never counted (Chests10 impossible, price stuck at 25) | High | economy, solo |
| H3 | Weapon evolutions are never counted (EvolveWeapon impossible) | High | economy, solo |
| H4 | Host latches a joiner's jump, slide and interact bits | High | co-op, netcode |
| H5 | Joiner's predicted body is never reconciled with the host copy | High (netcode verifier: Medium) | co-op, netcode |
| H6 | Host pause over 2 s makes the joiner's horde permanently invisible | High | co-op, netcode |
| H7 | HOST CO-OP before any run in the process should panic | High | co-op, netcode |
| H8 | Joiner who connects while the host is in menus builds the wrong world | High | co-op, netcode |
| H9 | No run-end signal reaches a joiner | High (netcode verifier: Medium) | co-op, netcode |
| H10 | Anubot beam and Beamer aim line are invisible to the joiner | High (netcode verifier: Medium) | co-op |
| M1 | Interactable layout RNG can desync between machines | Medium (High once joiners interact) | co-op, netcode |
| M2 | A joiner cannot use any interactable; shrine effects go to the host | Medium | co-op |
| M3 | The teleporter never exists on the joiner | Medium | co-op |
| M4 | Host panels, pause and hitstop freeze the world for everyone | Medium | co-op, netcode |
| M5 | A joiner cannot pick a hero | Medium | co-op |
| M6 | A joiner's powerups never expire | Medium | co-op |
| M7 | A joiner earns no meta progression from co-op (intentional) | Medium | co-op, economy |
| M8 | Lost crowd spawn descriptors leave enemies invisible | Medium | netcode |
| M9 | Dust-storm cover is computed for the host only but silences ranged enemies for all | Medium | co-op |
| M10 | DustStorm never resets: the second Mars visit has an invisible storm | Medium (shell verifier: Low) | solo |
| M11 | The Comet Combo only exists for the host's own astronaut | Medium | co-op |
| M12 | meshkit boxes, cylinders and cones have inverted winding | Medium | solo |
| M13 | Save format is fragile: one new counter wipes progress | Medium | dev-hygiene, economy |
| M14 | DEV key B (summon boss) ships ungated | Medium (two verifiers: Low) | dev-hygiene, solo, co-op |
| M15 | Downed teammates stay downed on every later stage | Medium | co-op |
| M16 | Spawn ramp is about 15x the GDD's and caps out the horde | Medium | solo, economy |
| M17 | No stage or tier scaling: stages 2 and 3 restart at minute-zero pressure | Medium | solo, economy |
| M18 | `SpatialHash::near` scans a full cube of cells | Medium | perf |
| M19 | ABANDON RUN leaks the PAUSED overlay over Results and menus | Medium | solo, UI |
| M20 | HOST CO-OP's LAN-address note is effectively never shown | Medium | co-op, UI |
| M21 | A failed or unreachable join needs a game restart | Medium | co-op, netcode, UI |
| M22 | Edge markers vanish for over-horizon targets that project on screen | Medium | UI |
| M23 | Joiner combat and boss feedback is largely missing | Medium | co-op, UI |
| L1 | Air-hopping reaches the 2.1x speed cap without sliding | Low | solo |
| L2 | Jump and slide edges are sent once, on an unreliable channel | Low | co-op, netcode |
| L3 | `player_input` and `player_physics` have no ordering | Low | solo |
| L4 | Coyote time is dead logic | Low | solo, dev-hygiene |
| L5 | `InputIntent.forward` and `.interact` are never read; comments are false | Low | dev-hygiene |
| L6 | Per-planet sky colour and `meteor_showers` are unused (gray backdrop) | Low | solo, UI |
| L7 | Interactables can spawn inside large boulders | Low | solo |
| L8 | Terrain mesh is rebuilt synchronously on every stage entry | Low | perf |
| L9 | Every astronaut rig allocates its own meshes, materials and a shadowed spotlight | Low | perf |
| L10 | Craterpillar Jr looks different on host and joiner | Low | co-op |
| L11 | Boss proxies: arbitrary boss-bar pick and fixed rotation | Low | co-op, UI |
| L12 | Buried burrowers can be hit by some weapons; the joiner draws them above ground | Low | solo, co-op |
| L13 | Teammate rigs on the joiner are coloured by slot, not by hero | Low | co-op |
| L14 | Owning a cryo weapon adds a flat slow to all of that player's hits | Low | solo |
| L15 | The aura bubble is drawn smaller than its damage radius | Low | solo, UI |
| L16 | Pots and bosses count toward the enemy cap and the `enemies=` figure | Low | solo, perf |
| L17 | Downed astronauts still anchor spawns, party scale and boss placement | Low | co-op |
| L18 | A downed astronaut's body absorbs enemy shots | Low | co-op |
| L19 | A downed astronaut keeps charging shrine rings | Low | co-op |
| L20 | Boss HP ignores party size; boss add rings bypass the cap | Low | co-op |
| L21 | `NET_ENEMY_MAX_RECORDS` is not a hard ceiling | Low | netcode, perf |
| L22 | A joiner's gold ignores its Gold Gain | Low | co-op, economy |
| L23 | `greed_stacks` is not replicated | Low | co-op, economy |
| L24 | Chests and shops can exceed an item's max stacks | Low | economy |
| L25 | The microwave is used up even when nothing fits | Low | economy |
| L26 | The `roll_item` fallback ignores bans and stack caps | Low | economy |
| L27 | `static_secs_best` only counts the final stage | Low | economy |
| L28 | Tutorial promises a boss chest; its step 5 never fires on a joiner | Low | solo, UI |
| L29 | Tutorial says Shift slides; slide is Ctrl or C | Low | UI |
| L30 | Lady Fortuna's rerolls are unlimited; her +2 refreshes are dead | Low | economy |
| L31 | S is both move-back and level-up Skip | Low | UI |
| L32 | Four stats are never granted by anything | Low | economy |
| L33 | Idle pickups dirty their Transform every frame; gold, silver and food piles are uncapped | Low | perf |
| L34 | `stream_pickups` same-frame race: ghost gem or panic | Low | netcode |
| L35 | `RunSnapMsg` is unordered and unsequenced | Low | netcode |
| L36 | Host re-seats peers while it is in Results or the menus | Low | co-op, netcode |
| L37 | ABANDON RUN on a joiner soft-locks it until the host changes stage | Low | co-op, UI |
| L38 | LAUNCH and DAILY are not blocked for a client | Low | co-op, UI |
| L39 | No teammate HP or downed indicator | Low | co-op, UI |
| L40 | A host that quits leaves the joiner frozen | Low | co-op, netcode |
| L41 | Pots the host breaks stay standing on the joiner | Low | co-op |
| L42 | Net components are marked changed every frame | Low | netcode, perf |
| L43 | `PlayerInputMsg` is sent every render frame | Low | netcode, perf |
| L44 | `ClientResidency` is never pruned when a client leaves | Low | netcode |
| L45 | u16 pickup ids wrap without a collision check | Low | netcode |
| L46 | `PROTOCOL_ID` has not been bumped since Stage 4 | Low | netcode |
| L47 | Session-log gaps: second-resolution names, no seat events, a bogus "Disconnected" | Low | dev-hygiene |
| L48 | The health line lacks the seed, checksums and loss counts | Low | dev-hygiene |
| L49 | The weapon-tray cache goes stale | Low | UI |
| L50 | E or ESC can be handled twice in one frame | Low | UI |
| L51 | Clicks pass through the settings and join overlays (can spend silver) | Low | UI, economy |
| L52 | `Selected.daily` leaks into a later HOST CO-OP run | Low | UI, co-op |
| L53 | `button_hover` overwrites custom button colours | Low | UI |
| L54 | Static SILVER header; a stale MenuTab opens an empty side panel | Low | UI |
| L55 | Joiner's edge markers: shrine markers never clear, no teleporter marker | Low | co-op, UI |
| L56 | The dev CLI is compiled into release and re-scans args every frame | Low | dev-hygiene, perf |
| L57 | The headless smoke drifts from the real app; its summary query is unfiltered | Low | dev-hygiene |
| L58 | `--fast-boss` spawns all three bosses together; the solo check passes weakly | Low | dev-hygiene |
| L59 | Dead code and no-op bindings | Low | dev-hygiene |
| L60 | Unused `ResMut`/`Res` parameters, some forcing exclusive access | Low | dev-hygiene, perf |
| L61 | `MetaSave` is cloned just to call `recompute_stats` | Low | perf |
| L62 | `update_hud` rewrites 8 texts per frame; `update_music` counts pots | Low | perf |
| L63 | Stale comments in `src/` | Low | dev-hygiene |
| L64 | Top-level docs are stale | Low | dev-hygiene |
| L65 | 35 compiler warnings, no unit tests, no CI | Low | dev-hygiene |

The uncertain items (U1-U12) are in §5, the design questions (D1-D10) in §6, and ship hygiene (SH1-SH16) in §9.

---

## 2. High

H1, H2 and H3 are solo regressions introduced by co-op Stage 1 (commit `35db5ff`), which moved the per-player numbers from `RunState` onto `PlayerState`. They affect every run the user plays today.

### H1. Results and the save bank level 1 and gold 0

- **Severity:** High · **Area:** solo, economy (also a co-op host) · **Evidence:** LIVE · **Status:** Fixed (0970452, P30)
- **Where:**
  - `src/director.rs:272-280`: `bank_results` queries `Query<&PlayerState, With<LocalPlayer>>` and falls back with `.unwrap_or((1, 0))` at :280.
  - `src/director.rs:289`: `best_level`.
  - `src/director.rs:297-298`: the payout, including the `silver_gain` fallback of 1.0.
  - `src/director.rs:336-337`: `ResultsData.level` and `.gold`.
  - `src/main.rs:141-155`: OnExit(InRun) runs `despawn_stage`, then OnEnter(Results) runs `bank_results`.
  - `src/player.rs:170`: the astronaut is `StageScoped`.
  - `src/planet.rs:91-95`: `despawn_stage`.
- **Symptom:** After any run (victory, death or ABANDON):
  - the Results screen shows LEVEL 1 and GOLD 0;
  - `save.counters.best_level` never rises above 1;
  - the silver payout loses its level term, and its `silver_gain` factor falls back to 1.0;
  - the daily score is lower than it should be;
  - the Level20 quest "Overachiever" can never complete.

  Seen live: a level-25 solo run logged `Solo state=Results ... players=0`, and the save holds `best_level: 1`.
- **Root cause:**
  - The commands that `despawn_stage` queues in OnExit(InRun) are applied before OnEnter(Results) runs. The astronaut, which carries the `PlayerState`, is `StageScoped`, so it is gone when `bank_results` looks for it. `.single()` fails and the `(1, 0)` fallback is used.
  - Before `35db5ff` this code read `run.level` and `run.gold` from `RunState`, which survives (`35db5ff^:src/director.rs`, around lines 231, 239 and 278-279).
- **Repro:** Solo, any hero. Reach level 2 or higher, then die or ESC, then ABANDON RUN. Results shows LEVEL 1 and GOLD 0, and `%APPDATA%/astrobonk/save.json` still has `best_level: 1`.
- **Suggested fix:**
  1. Snapshot the local astronaut's `level`, `gold` and `stats.silver_gain` while it still exists.
  2. The simplest way: a tiny system that runs every frame in InRun and copies them into new `RunState` fields (for example `final_level`, `final_gold`, `final_silver_gain`). This covers every exit path.
  3. Alternatively, write them at each run-end site:
     - the victory branch of `stage_transition` (`src/director.rs:165-176`);
     - `downed_watch` when it sets `Dead` (`src/director.rs:240-252`);
     - ABANDON (`src/ui/panels.rs:577-581`).
  4. `bank_results` then reads `RunState`. It needs no ordering against `despawn_stage`.
  5. Verify: a solo run to level 2 or higher, then check the Results screen and `save.json`.
- **Status:** Fixed (0970452, P30). **Resolution:** Re-verified open at the P30 base (`bank_results` still read the despawned LocalPlayer). `director::snapshot_local_sheet` copies the local sheet (level, gold, Cursed Moon Rocks, Silver gain, Static Silver) into `RunState::final_sheet` every in-run frame, and `bank_results` reads that, so every exit path banks the real numbers. The headless summary now tears the stage down the way OnExit(InRun) does and fails if the banked sheet differs from the live one.

### H2. Chest opens are never counted (Chests10 impossible, chest price stuck at 25)

- **Severity:** High · **Area:** economy, solo · **Evidence:** LIVE · **Status:** Fixed (67bf6ca, P01)
- **Where:**
  - `src/ui/panels.rs:341-361`: the chest "take" branch. It writes no counter.
  - `src/ui/panels.rs:265`: `global: Res<RunState>` is read-only.
  - `src/run.rs:121`, `:124`: the fields `chest_opens` and `chests_opened`. They are initialised at :184 and :187 and never incremented anywhere.
  - Readers: `src/interact.rs:440` (price) and `src/director.rs:285` (banking).
  - `src/config.rs:48-49`: `CHEST_BASE_COST` 25 and `CHEST_COST_GROWTH` 1.75. The growth constant is effectively dead.
  - `src/content/quests.rs:71`: Chests10 grants `UnlockWeapon(RocketPod)`.
- **Symptom:**
  - `counters.chests` stays 0.
  - "Cache Money" (Chests10: open 10 chests) never completes, so RocketPod never enters the level-up NewWeapon pool. It stays playable as Old Ironclad's starting weapon.
  - Every chest costs `25 × (1 − chest_discount)`, however many have been opened.

  Seen live: a chest was bought at about 6:58 (gold 261 to 236), and the save shows `chests: 0`.
- **Root cause:** `35db5ff` moved the take logic onto the `PlayerState` and dropped `run.chest_opens += 1; run.chests_opened += 1` (present at `35db5ff^:src/ui/panels.rs:345-346`). `chest_panel` only holds `Res<RunState>`, so it cannot write them.
- **Repro:** Solo. Open a chest with E and take it, then walk to a second chest: the price is still 25. After Results, `save.json` has `counters.chests: 0`.
- **Suggested fix:**
  - Make `global` a `ResMut<RunState>` in `chest_panel`.
  - In the take branch, after the gold check, increment both `chest_opens` and `chests_opened`.
  - Co-op note: `chest_panel` is ungated, so it also runs on clients. A client's counters are never banked (M7), and its `chest_opens` is not in `RunSnapMsg`. When docket item 5 lets joiners open chests, decide whether the chest price escalates per opener or per run (see §8, item 5).
- **Status:** Fixed (67bf6ca, P01). **Resolution:** Verified in current code: `ui::panels::chest_panel` takes `ResMut<RunState>` and advances `chest_opens` and `chests_opened` on a take; the miniboss cache (P01) also counts toward `chests_opened`.

### H3. Weapon evolutions are never counted (EvolveWeapon impossible)

- **Severity:** High · **Area:** economy, solo · **Evidence:** CODE · **Status:** Fixed (67bf6ca, P01)
- **Where:**
  - `src/run.rs:608-617`: the `Evolve` arm of `apply_upgrade` only sets `evolved = true`.
  - `src/ui/panels.rs:233-238`: the caller writes only the banner, SFX and hitstop.
  - `src/ui/panels.rs:147`: `global: Res<RunState>` in `choice_input`.
  - `src/run.rs:126`: the `evolves` field.
  - `src/director.rs:288`: banked.
  - `src/save.rs:198`: `EvolveWeapon => c.evolves >= 1`.
- **Symptom:** `counters.evolves` stays 0, so the "Ascension" quest (+100 silver) can never complete.
- **Root cause:** `35db5ff` removed `self.evolves += 1` (`35db5ff^:src/run.rs:593`) when `apply_upgrade` moved onto `PlayerState`. Nothing writes `RunState.evolves` any more.
- **Repro:**
  1. Take a weapon to max level and own its catalyst item.
  2. Pick the EVOLVE card. The "WEAPON EVOLVED" banner shows.
  3. After Results, `counters.evolves` is still 0.
- **Suggested fix:** In `choice_input`, when `apply_upgrade` returns `evolved == true` (`src/ui/panels.rs:233`), increment `RunState.evolves`. This needs `global` to become a `ResMut<RunState>`.
- **Status:** Fixed (67bf6ca, P01). **Resolution:** Verified in current code: `ui::panels::choice_input` increments `RunState::evolves` when `apply_upgrade` reports an evolution.

### H4. Host latches a joiner's jump, slide and interact bits

- **Severity:** High · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:1011-1013`: in `apply_remote_input`, `intent.jump |= message.jump;`, and the same for `slide` and `interact`.
  - Consumers: `src/player.rs:457` (jump) and `src/player.rs:490` (slide).
  - The only reset: a fresh `InputIntent::default()` when a stage change respawns the astronaut (`src/player.rs:164`, `src/director.rs:220-224`).
- **Symptom:**
  - **Space pressed once.** After the joiner presses Space once, the host copy of the joiner re-jumps on every landing for the rest of the stage. Each re-jump lands inside `BHOP_WINDOW`, so the grounded speed cap never applies. The body reaches `SPEED_HARD_CAP`, 2.1 × run speed (`src/config.rs:16`), and plays Bhop SFX on the host.
  - **Ctrl or C pressed, never Space.** The host copy re-slides every `SLIDE_COOLDOWN` (1.1 s). It lunges along its facing even with no input, plays slide SFX and keeps Yuki's SlideFrenzy running.
  - **Both latched.** The body jumps continuously and never slides, because the jump clears `grounded` and the slide needs it.
  - **Interact.** This bit latches too, but it has no reader today. It will have one once docket item 3 lands.
  - **Why it matters.** The joiner's own screen shows none of this, because its prediction uses one-frame edges. But the host copy is the body that takes hits, collects pickups, charges rings and anchors enemy interest. It feeds H5.
- **Root cause:**
  - Edge bits are OR-ed into a component that nothing on the host ever clears for non-local astronauts.
  - `player_input` takes `&InputIntent` read-only (`src/player.rs:424`). `gather_local_input` assigns only the LocalPlayer's copy (`src/player.rs:411-413`).
  - Commit `99d012f` claims its "neutral intent while a panel is open" stops the latch. It does not: OR-ing `false` clears nothing.
- **Repro:** Run two instances: `astrobonk --host --autodrop` and `astrobonk --join 127.0.0.1`. On the joiner, press Space once, then watch the joiner's rig in the host window: it keeps hopping. This is not caught by `--botinput` (the bot never jumps or slides) or by `--headless --coop2` (peer intent is never written).
- **Suggested fix:**
  1. In `apply_remote_input`, first clear `jump`, `slide` and `interact` on every non-local astronaut, then OR in this frame's messages.
  2. The system is already ordered `.before(player_input)` (`src/net.rs:446-452`).
  3. An edge that arrives while the host is paused is dropped, which is acceptable.
  4. Optional: add a jump/slide sequence counter to `PlayerInputMsg`, so a lost unreliable packet does not lose the press (L2).
- **Status:** Open. This is pre-fix 0a in §8.

### H5. Joiner's predicted body is never reconciled with the host copy

- **Severity:** High. The world verifier rated it High, the netcode verifier Medium. It is kept High because every hit, pickup and ring charge resolves at the host copy. · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/main.rs:435` and `src/netenemy.rs:809-820`: the client always spawns its own body as id 0.
  - `src/player.rs:133-139`: id 0 spawns at `Vec3::Y`; id N spawns 3.5 m away at an angle of N × 1.7 rad.
  - `src/net.rs:920-931`: the host seats the peer with its real id. The respawn at `src/director.rs:220-224` also keeps the real id.
  - `src/net.rs:583-597`: `adopt_my_vitals` copies only `hp` and `down`.
  - `src/remote.rs:79-81`: the client skips its own replicated copy.
- **Symptom:**
  - The joiner and its host copy start every stage 3.5 m apart and drift further apart after that.
  - Enemy contact, projectile hits, pickup collection, charge-ring occupancy and enemy interest all resolve at the host copy, not where the joiner sees itself.
  - To the joiner this looks like being hit from out of reach, walking over gems that do not collect, or gems collecting at a distance.
- **Root cause:** No system writes the host's `NetTransform` for the client's own id back into its `Player`. The drift sources are:
  - the 3.5 m spawn offset;
  - host pause and hitstop, which freeze the host copy while the joiner keeps moving (M4);
  - the H4 latch;
  - a Speed powerup that never expires on the joiner (M6);
  - lost jump and slide edges (L2);
  - independent variable-dt integration on each machine (there is no fixed timestep anywhere).
- **Repro:** Two instances. The joiner stands still at spawn. The host window shows the joiner's rig about 3.5 m from where the joiner's own camera places it. The drift magnitude over time is unmeasured (U1).
- **Suggested fix:**
  1. Spawn the client's own body with its real id. The client should wait for `MyPlayerId` (from `AssignPlayerId`, 2 Hz) as well as the seed before building (`client_follow_host_run`, `src/main.rs:354-376`). Then pass that id in `enter_run` and `client_stage_transition`, so both machines use the same fan-out offset.
  2. Add soft reconciliation: each frame, ease the predicted `Player.dir` toward the replicated `NetTransform.dir` of our own id when the error passes a small threshold, and snap above a large one (for example 3 m).
  3. Remove the drift sources: H4, M6 and L2.
- **Status:** Open.

### H6. Host pause over 2 s makes the joiner's horde permanently invisible

- **Severity:** High · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/fx.rs:47-58`: `phase_time_control` pauses `Time<Virtual>` in every phase except Playing.
  - `src/netenemy.rs:291-295`: `stream_enemies` accumulates virtual-time dt, so it sends nothing while the host is paused.
  - `src/netenemy.rs:1238-1241`: the client's reaper despawns proxies unseen for `NET_ENEMY_GRACE`, which is 2.0 s (`src/config.rs:99`).
  - `src/netenemy.rs:395-404`: the host's per-client residency changes only when an id goes unseen in a snapshot it actually sends.
  - `src/netenemy.rs:1177-1185`: the client ignores update records for ids it does not know.
- **Symptom:**
  - The host spends more than 2 s in any panel (level-up, chest, shop, Moai, microwave, shrine blessing) or in the ESC pause.
  - Every enemy that was around the joiner vanishes from the joiner's screen and stays invisible, while still moving, shooting and damaging the joiner.
  - This lasts until each enemy walks out past `interest_out` and back in. `interest_out` is 107 m on the Moon, 122 m on Mars and 93 m on the Dark Moon (`src/config.rs:87`).
  - Boss proxies are unaffected, because they have no reaper.
  - Any network stall over 2 s has the same effect.
- **Root cause:**
  1. While the host's virtual clock is paused, no snapshots go out.
  2. The joiner is still Playing, so its reaper, which runs on the joiner's own virtual time, removes every proxy after 2 s.
  3. On resume, the host still lists those ids as resident, so it sends 6-byte updates instead of 8-byte spawn descriptors.
  4. The client drops those updates because the ids are no longer in `NetEnemyIndex`.
  5. The two-instance tests used `--autopick`, which closes panels instantly, so they never showed this.
- **Repro:**
  1. Run two instances without `--autopick`.
  2. On the host, level up and wait 3 s or more on the card panel, then pick.
  3. The horde near the joiner is gone from the joiner's screen, but damage keeps landing.
- **Suggested fix:** Three candidates; none has been evaluated.
  - **(a) Host clears residency after a gap.** Track the `Time<Real>` of the last snapshot sent to each client. If the gap exceeds about 0.75 × `NET_ENEMY_GRACE`, clear that client's residency before building the next snapshot, so everything is re-sent as descriptors. This is the smallest change.
  - **(b) Client reaps only on evidence.** Only reap ids that are absent from snapshots that actually arrived: keep `last_seen` as a snapshot sequence number, and do not reap when no snapshot came in.
  - **(c) Client asks for a resend.** Have the client answer unknown-id updates with a resend request. This needs a client-to-host message.
  - Also see M4: moving the host's net lanes to real time removes most gaps.
- **Status:** Open. This is pre-fix 0c in §8.

### H7. HOST CO-OP before any run in the process should panic

- **Severity:** High · **Area:** co-op, netcode · **Evidence:** CODE (the panic was not reproduced) · **Status:** Open
- **Where:**
  - `src/netenemy.rs:284`: `planet: Res<CurrentPlanet>`, a non-optional param in `stream_enemies`.
  - `src/netenemy.rs:189-194`: gated only on `is_simulating` and `is_networked`, with no AppState gate.
  - `src/main.rs:446`: the first insert of `CurrentPlanet`, in `enter_run`.
  - `src/planet.rs:59-60`: `CurrentPlanet` has no `Default`.
  - `src/ui/menus.rs:258-271`: HOST CO-OP starts the server from the main menu.
- **Symptom:**
  - On a fresh launch, clicking HOST CO-OP should crash immediately. `astrobonk --host` without `--autodrop` takes the same path.
  - Bevy 0.18's default error handler panics on a missing non-`Option` resource, and `src/` never installs a different handler.
  - The user's 2026-09-26 session hosted only after a solo run, when `CurrentPlanet` already existed, so it did not crash.
- **Root cause:** `stream_enemies` needs `CurrentPlanet` but runs whenever the role is Host, including in the menus. `CurrentPlanet` exists only after the first `enter_run`, and nothing ever removes it.
- **Repro:** Launch fresh and click HOST CO-OP, or run `astrobonk --host`. The playlog panic hook should record the panic in the session log.
- **Suggested fix:**
  - Take `Option<Res<CurrentPlanet>>` and return when it is `None`. All the other stream and receive systems already do this (for example `src/netenemy.rs:523`, `:662`, `:897`).
  - Better: gate the host lanes (`assign_net_ids`, `stream_enemies`, `stream_bosses`, `stream_hazards`, `stream_pickups`) on `in_state(AppState::InRun)`.
- **Status:** Open. This is pre-fix 0b in §8.

### H8. Joiner who connects while the host is in menus builds the wrong world

- **Severity:** High · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:394-399`: `push_run_snapshot` has only `is_hosting` and a 250 ms `on_timer`, with no AppState gate.
  - `src/net.rs:550`: `seeded = true` on any snapshot.
  - `src/main.rs:373-374`: `client_follow_host_run` pulls the client from MainMenu or Boot into InRun.
  - `src/net.rs:531-533`: a rebuild is flagged only when the stage index changes.
  - `src/ui/menus.rs:593`: the host's planet pick replaces `RunState` with a fresh seed.
  - `src/main.rs:386-393`: the boot placeholder `RunState`.
- **Symptom:**
  - The natural flow triggers it: the host clicks HOST CO-OP and picks a hero while the joiner types the IP.
  - The joiner enters InRun at once, on the host's placeholder seed (the boot `RunState`, or the host's previous run).
  - When the host launches, the new seed arrives, but the stage index is 0 on both machines, so the joiner never rebuilds.
  - For the whole stage, the rocks, pots, chests, shrines, and possibly the planet and its radius (Moon 140, Mars 160), differ from the host's. The streamed horde walks through scenery the host does not have.
  - If the host's previous run ended on stage 1 or later, the stage edge does trigger a correct rebuild.
  - On a fresh host process, H7 fires first.
- **Root cause:** The snapshot stream is not gated on the host being in a run, the client treats any snapshot as "seeded", and the rebuild trigger compares only the stage index. The comment at `src/ui/menus.rs:259-260` ("joiners arrive once it is in a run") states the intent, but nothing enforces it.
- **Repro:**
  1. Host: finish one solo run, return to the main menu, click HOST CO-OP, and stay on CharSelect.
  2. Joiner: JOIN CO-OP, type the IP, CONNECT. The joiner enters a world.
  3. Host: pick a planet.
  4. Compare the worlds. `--netlog` prints the layout and interactable checksums to the console (`info!`, which Bevy writes to stderr).
- **Suggested fix:** Do both.
  1. Add `.run_if(in_state(AppState::InRun))` to `push_run_snapshot`, so a joiner waits in its menu until the host is in a run.
  2. In `apply_run_snapshot`, flag a rebuild when `world_built` and the stage, `run_seed` or `chain` differs.
  - Until this is fixed, the joiner must connect only after the host is in the run.
- **Status:** Open. This is pre-fix 0b in §8.

### H9. No run-end signal reaches a joiner

- **Severity:** High. Two verifiers rated it High, the netcode verifier Medium. · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:99-119`: `RunSnapMsg` has no result or phase field.
  - `src/main.rs:284-285`: `downed_watch` and `death_watch` are host-only.
  - `src/main.rs:151`: `bank_results` is host-only.
  - `src/director.rs:175` and `:264`: the only two transitions to Results, both on the host.
  - `src/main.rs:354-376`: `client_follow_host_run` pulls in only from MainMenu or Boot.
  - `src/net.rs:125-130`: `RunSync.seeded` and `.world_built` are never reset.
- **Symptom:** When the host wins or the whole party dies, the host goes to Results, but the joiner stays InRun on an emptied planet:
  - the clock and kill count freeze;
  - crowd proxies vanish, most likely through a host despawn sweep (CODE, not observed). `stream_enemies` has no AppState gate. `bank_results` sets the host's phase back to Playing (`src/director.rs:344`), so the host's virtual clock runs in Results. The host also re-seats the joiner's body there on the next frame (L36). The next snapshot then finds no enemies and sends a despawn record for every resident id. Without the re-seat, `stream_enemies` would skip the client (`src/netenemy.rs:306-312`), and the client's 2 s reaper would remove the proxies instead;
  - boss proxies clear through an empty `BossSnapMsg`.

  Then, for the next run:
  - The joiner has no working way out (ABANDON soft-locks it; see L37).
  - The host re-seats the joiner as soon as it reaches Results, and that body carries into the host's next run (L36).
  - The joiner rebuilds only if the new run's stage index differs from the old one. Two consecutive tier-1 runs both sit at stage 0, so the joiner plays the new run on the previous run's world (the same mechanism as H8).
- **Root cause:** Run-end is decided on the host and never communicated. The client's state machine has no path from InRun to Results or MainMenu.
- **Repro:** Two instances. The host dies, or uses ESC then ABANDON RUN. The host shows Results; the joiner stays on the planet with a frozen timer.
- **Suggested fix:**
  1. Add a result field to `RunSnapMsg` (0 running, 1 victory, 2 death), or add a reliable `RunEndMsg`.
  2. On the client, go to Results without banking (keep `bank_results` host-only; see M7), then to the main menu, to wait for the host's next run.
  3. At run end, reset `RunSync.seeded` and `world_built`, so the next snapshot is treated as a new run.
  4. Bundle this wire change with the single `PROTOCOL_ID` bump (L46).
- **Status:** Open.

### H10. Anubot beam and Beamer aim line are invisible to the joiner

- **Severity:** High. The horde verifier rated it High, the netcode verifier Medium. · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/netenemy.rs:881-882`: `beam_angle` and `beam_state` are already sent at 20 Hz in `BossRec` (`src/net.rs:150-152`).
  - `src/netenemy.rs:910-917`: `receive_bosses` ignores both fields.
  - `src/enemies.rs:703-712`: `AnubotBeam` and `AnubotBeamVis` are added only in the host's `spawn_boss`.
  - `src/main.rs:198`: `anubot_beam_system` is host-only.
  - `src/netenemy.rs:480-516`: `stream_hazards` covers only `EnemyProjectile`, `Telegraph` and `MortarShell`.
  - `src/enemies.rs:1416-1422`: the `AimLine` spawn, inside `beamer_attack`, which is host-only (`src/main.rs:203`).
  - `src/netenemy.rs:1024`: boss proxies use a fixed rotation.
- **Symptom:**
  - On Mars and the Dark Moon (Anubot is the stage boss on both; `src/director.rs:89-91`), the joiner is hit by a sweeping beam it never sees.
  - Beamers fire their railbolt, which is streamed and visible, without the 1.1 s charge line that telegraphs it.
  - Anubot's pose tell is also lost, because proxies never turn.
- **Root cause:** The client never builds the beam visual from the fields it already receives, and aim lines never cross the wire.
- **Repro:** Two instances on Mars (for example `--stagenow` on the host, or pick Mars), with `--bossnow` to summon the stage boss early. Stand the joiner in the beam's sweep.
- **Suggested fix:** Docket item 7 (§8).
  - **7a (client-only, no wire change).** In `receive_bosses`, keep `beam_angle` and `beam_state`, and attach an `AnubotBeamVis` to Anubot proxies. Drive it with a client copy of the host's beam visual pass in `anubot_beam_system` (`src/enemies.rs:798`; the visual part is around :890-910). Extrapolate the angle between 20 Hz packets using the known spin rates, or the sweep will visibly step.
  - **7b (wire change).** Add `HazardEvent::AimLine { beamer NetId, target PlayerId, dur }`. Ship it only together with the single `PROTOCOL_ID` bump.
- **Status:** Open.

---

## 3. Medium

### M1. Interactable layout RNG can desync between machines

- **Severity:** Medium. It becomes High once joiners can interact (docket items 2-5). The netcode verifier lowered it from High. · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/interact.rs:147`: one `StdRng` seeded from `(run_seed + stage) * 0x9e37` places every interactable.
  - `src/interact.rs:241-249`: shady-guy stock is rolled from that same rng, between placements.
  - `src/interact.rs:92-111`: `roll_item` calls `Rarity::roll`, which is luck-weighted (`src/content/mod.rs:47-58`), then `candidates.choose(rng)`.
  - Callers pass different sheets:
    - `src/main.rs:436-445`: both machines pass a fresh sheet at stage 0.
    - `src/director.rs:212-216,227`: the host passes its carried sheet on stage 1 and later.
    - `src/netenemy.rs:827`: the client passes a fresh level-1 sheet of its own boot character.
- **Symptom:**
  - These can sit in different places on the host and the joiner: both shady guys, the 2 Greed and 2 Magnet shrines, the Moai, the microwave, the cage slot and all 5 charge rings.
  - Pots and chests are placed before the first stock roll, so they always match.
  - Today, the joiner sees shrines, a Moai and a microwave where the host has none, and misses charge rings that its host copy can charge.
- **Root cause:**
  - `rand` is locked at 0.8.7 (`Cargo.lock:4198-4199`). Its `choose` goes through `gen_range`, which uses rejection sampling. The number of RNG words it consumes depends on the list length.
  - The candidate list has 8, 9, 4 or 1 entries depending on the rarity rolled, which luck shifts. It also depends on the owned-at-max and banned items of the sheet passed in.
  - Different sheets therefore consume different numbers of words, and every later `place_dir` draw shifts.
  - Triggers:
    - at stage 0, when the two heroes' luck differs (Lady Fortuna has `Passive::Luck(0.30)`, `src/content/characters.rs:184`);
    - at stage 1 and later, whenever the host's carried sheet has luck, maxed items or bans.
- **Repro:**
  1. Run both instances with `--netlog`: the host plays Fortuna, the joiner Buzz.
  2. Compare the `inter_sum=` console lines.
  3. The checksum includes the Cage and the host-only Teleporter (`src/net.rs:813`, `:839-842`), so compare ring positions specifically.
- **Suggested fix:**
  - Make placement independent of any sheet. Either place everything first and roll stock afterwards, or roll stock from a second `StdRng` seeded from the same seed XOR a constant. This is pre-fix 0d in §8; it has not been evaluated.
  - Docket item 5's "pass the carried sheet" does **not** fix this. It would make the divergence routine, so do this first.
  - Also add a rings-only checksum, or exclude the Cage and Teleporter from `inter_sum`.
- **Status:** Open.

### M2. A joiner cannot use any interactable; shrine effects go to the host

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/main.rs:235`: `interact_system.run_if(net::is_simulating)`.
  - `src/interact.rs:398`, `:404`: both queries are `With<LocalPlayer>`.
  - `src/interact.rs:461`: reads `KeyCode::KeyE` directly.
  - `src/interact.rs:341`, `:375-386`: a charge-shrine blessing is built from, and applied to, the host's LocalPlayer sheet.
  - `src/interact.rs:490-496`: the Magnet shrine sets every pickup's target to the host's astronaut.
- **Symptom:**
  - A joiner pressing E at a chest, shady guy, Greed or Magnet shrine, Moai, microwave, cage or teleporter gets nothing.
  - A charge ring completed while only the joiner's host copy stands in it opens the SHRINE BLESSING panel on the host's screen, for the host's build.
  - A Magnet shrine used by the host pulls every pickup on the planet to the host. XP is a shared pool, but gold, food and powerups all go to the host.
- **Root cause:** All interactions are resolved in one host-only system that is bound to the LocalPlayer and the local keyboard. `InputIntent.interact` is sent over the wire but never read.
- **Repro:** Two instances. The joiner walks to a chest and presses E.
- **Suggested fix:** Docket items 2, 3 and 5 (§8). `interact_system` is at Bevy's 16-parameter cap, so it must be split first.
- **Status:** Open.

### M3. The teleporter never exists on the joiner

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/director.rs:99-103`: the only call to `spawn_teleporter` is inside `run_clock`, which is host-only (`src/main.rs:242`).
  - `src/interact.rs:299-309`: the position is the party centroid (`src/director.rs:66-72`) plus 18 m at a `thread_rng` angle.
  - `src/net.rs:118`: only `teleporter_open` crosses the wire.
- **Symptom:** After the stage boss dies, the joiner has no teleporter entity, no edge marker and no prompt. Only the host can take it. Even once item 3 lets the host resolve the joiner's E, the joiner cannot see where the teleporter is.
- **Root cause:** A host-only spawn at a position that cannot be reproduced from the seed, and no position on the wire.
- **Repro:** Two instances with `--bossnow`. Kill the boss; the host sees the teleporter and the joiner does not.
- **Suggested fix:** Docket item 4 (§8).
  1. Split out a direction picker and `spawn_teleporter_at(dir)`.
  2. Store `RunState.teleporter_dir`, reset it on stage change next to `teleporter_open` (`src/director.rs:203`), and send it in `RunSnapMsg`.
  3. On the client, spawn it level-triggered ("open, and no Teleporter exists"), ordered after `client_stage_transition`.
- **Status:** Open.

### M4. Host panels, pause and hitstop freeze the world for everyone

- **Severity:** Medium · **Area:** co-op, netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/fx.rs:47-58`: `phase_time_control` pauses `Time<Virtual>` in any non-Playing phase.
  - `src/main.rs:213`, `:227`, `:247`, `:253`: every simulation chain is gated `.and(playing)`.
  - `src/fx.rs:40`: hitstop sets the virtual speed to 0.06.
  - `src/net.rs:396-398`, `:408`: the `on_timer` host lanes run on virtual time. So do `stream_enemies` and `stream_bosses`, and replicon's `ServerTick`.
  - `src/interact.rs:375-386`: a completed charge ring opens a Modal on the host.
  - `src/netenemy.rs:1206-1219`: the client's `drive_proxies` uses virtual dt and returns when dt ≤ 0 (:1216-1219).
- **Symptom:**
  - **Host panels freeze everyone.** Every level-up, chest, shop, Moai, microwave, blessing or ESC pause on the host freezes the whole co-op world: all enemies, the joiner's host copy, and every host network lane (RunSnap, AssignPlayerId, EnemySnap, BossSnap, replicated components). The joiner keeps moving locally, which feeds H5, and after 2 s it loses its horde (H6).
  - **Host hitstop slows everyone.** Host hitstop (for example 0.25 s on a boss kill) runs everyone at 6%.
  - **Rings can open panels on the host.** A ring charged by the joiner's body opens a panel on the host, even when the host is dead or across the planet.
  - **A joiner's own panel freezes only its view.** It pauses the joiner's virtual time and freezes its proxy interpolation, while the host world keeps hitting the joiner's idle host copy.
- **Root cause:** The single-player pause model (pause `Time<Virtual>` whenever the phase is not Playing) is applied unchanged to a networked host, and `RunPhase` is one global per machine.
- **Repro:** Two instances. The host opens the ESC menu; the joiner's world stops moving, but the joiner can still walk.
- **Suggested fix:** This is design question D6.
  - In co-op, stop pausing the shared world for per-player panels, and make those panels overlays. Keep the full pause for solo only.
  - Run the host's net lanes on `Time<Real>`.
  - Disable hitstop while networked.
  - At minimum, give a ring's blessing to the astronaut or astronauts who charged it.
- **Status:** Open.

### M5. A joiner cannot pick a hero

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/main.rs:386-391`: boot creates `RunState` with Buzz.
  - `src/ui/menus.rs:757-760`: `try_join` makes no state change.
  - `src/main.rs:373-374`: the joiner goes MainMenu to InRun, skipping CharSelect.
  - `src/main.rs:435`: `enter_run` spawns `run_state.character`.
  - `src/net.rs:99-119`: `RunSnapMsg` carries no character.
  - `src/net.rs:928`, `:945`: the host seats and re-seats a peer with the **host's** `run.character`.
  - `src/net.rs:607`: `PlayerBuildMsg` sends `ps.character`.
- **Symptom:**
  - The joiner always plays Buzz on a fresh launch, or whatever hero it last played in this process.
  - The host first simulates the peer as the host's own hero, then switches to Buzz when `PlayerBuildMsg` arrives (within 500 ms).
  - The host draws the peer in its own suit only from stage 2 on, when the respawn uses `ps.character`.
- **Root cause:** The join path skips hero selection, and no hero travels host-to-client or is used when the host seats a peer.
- **Repro:** A fresh joiner connects and plays Buzz.
- **Suggested fix:**
  - Let the joiner pick a hero, for example by routing JOIN through CharSelect and storing the pick in a resource that `enter_run` and `client_stage_transition` read.
  - When seating or re-seating, use the hero from `PlayerBuildMsg`, and rebuild the host-side rig when it changes.
  - Also see L13 (teammate colours).
- **Status:** Open.

### M6. A joiner's powerups never expire

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:728-731`: `apply_loot_grant` pushes `(kind, secs)`.
  - `src/player.rs:864-873`: the only decay, inside `player_upkeep`, which is host-only (`src/main.rs:279`).
  - `src/ui/hud.rs:365-372`: the HUD prints the powerup timer.
  - `src/run.rs:333-335`: Speed gives 1.5x.
  - `src/run.rs:341-343`: Magnet gives 40x pickup range.
  - `src/netenemy.rs:734-754`: `animate_net_pickups` flies gems toward the local player using the local range.
- **Symptom:** After the joiner picks up a powerup:
  - its HUD shows a frozen timer forever;
  - Speed keeps local prediction at 1.5x while the host copy decays after 15 s, which is drift (H5);
  - Magnet keeps about 128 m of range (40 × 3.2), so every streamed gem on screen visually flies at the joiner for the rest of the stage. Collection still happens on the host.
- **Root cause:** Powerup decay lives inside a host-only upkeep system.
- **Repro:** Two instances. The joiner picks up a Speed powerup and watches the HUD timer.
- **Suggested fix:** Split the powerup decay loop into its own system and run it for the LocalPlayer on clients too. Alternatively, have the host send expiry.
- **Status:** Open.

### M7. A joiner earns no meta progression from co-op (intentional)

- **Severity:** Medium · **Area:** co-op, economy · **Evidence:** CODE · **Status:** Open (by design, pending decision D5)
- **Where:** `src/main.rs:148-151`: `bank_results.run_if(net::is_simulating)`, with a comment explaining why.
- **Symptom:** A joiner's silver, counters, quests and unlocks never change from a co-op run.
- **Root cause:** This is deliberate. A client's `RunState` is adopted from the host, so banking it would write the host's kills and silver into the joiner's save. Commit `17115e9` fixed a downed client banking the host's run on a loop.
- **Suggested fix:**
  1. Decide what a joiner should bank (D5), for example its own level and gold from its `PlayerState` plus the shared run outcome.
  2. Add a client bank path that uses only client-owned numbers.
  3. Trigger it from the run-end signal added for H9.
- **Status:** Open.

### M8. Lost crowd spawn descriptors leave enemies invisible

- **Severity:** Medium · **Area:** netcode · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:387`: `EnemySnapMsg` is on `Channel::Unreliable`.
  - `src/netenemy.rs:346-357`: a spawn descriptor is sent once.
  - `src/netenemy.rs:402-404`: the id joins residency on send, with no ack.
  - `src/netenemy.rs:1177-1185`: the client drops updates for unknown ids.
- **Symptom:** Under packet loss, an enemy whose descriptor chunk was lost stays invisible to the joiner, and can still hit it, until it leaves interest and re-enters.
- **Root cause:** The host assumes delivery. The client cannot ask for a resend, and the grace reaper cannot help because no proxy was ever created.
- **Repro:** Not reproduced. It needs a lossy link (`--netlog` on both machines prints the stream stats).
- **Suggested fix:** The same family as H6:
  - periodically re-send descriptors for a rotating subset of resident near-band enemies; or
  - let the client report unknown ids; or
  - carry descriptors on a small reliable side-channel.
- **Status:** Open.

### M9. Dust-storm cover is computed for the host only but silences ranged enemies for all

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/events_world.rs:39`: `q_player` is `With<LocalPlayer>`.
  - `src/events_world.rs:109-117`: `storm.player_inside` is computed from that one astronaut.
  - `src/enemies.rs:1284`: `spitter_attack` returns early. It also drives UFOs.
  - `src/enemies.rs:1342-1350`: `beamer_attack` drops every aim line.
  - `src/enemies.rs:1456`: `lobber_attack` returns early.
  - `src/main.rs:251`: `dust_storm_system` is host-only.
  - `src/ui/hud.rs:532-543`: the haze reads a flag that never changes on a client.
- **Symptom:**
  - On Mars, while the **host** stands in the dust storm, every Spitter, UFO, Beamer and Lobber stops firing at **every** player, including a joiner in the open.
  - A joiner inside the storm while the host is outside is still shot.
  - The joiner never sees the dome or the haze.
- **Root cause:** A per-machine flag is used as a global gate on host-only AI. `DustStorm` is not replicated. The comment at `src/events_world.rs:109-111` mentions only half of the gap.
- **Repro:** Two instances on Mars. The host stands in a storm; the joiner stands outside next to a Spitter.
- **Suggested fix:**
  - Compute a per-astronaut `InStorm` marker for every `Player` on the host, and have ranged attacks skip only targets that carry it.
  - Replicate the storm's `dir`, `active` and `radius` (for example in `RunSnapMsg`), so the client can draw the dome and the haze for its own position.
- **Status:** Open.

### M10. DustStorm never resets: the second Mars visit has an invisible storm

- **Severity:** Medium (the shell verifier rated it Low) · **Area:** solo · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/events_world.rs:62`: the dome is created only if `!storm.spawned_vis && q_vis.is_empty()`.
  - `src/events_world.rs:78`: the dome is `StageScoped`.
  - `src/events_world.rs:80`: `spawned_vis = true`, never reset.
  - `src/main.rs:420`: `enter_run` resets `Comet` but not `DustStorm`.
  - `src/main.rs:107`: `DustStorm` is only ever `init_resource`'d.
- **Symptom:**
  - On the second Mars visit in the same app session (another Mars run, or a chain that reaches Mars again), storms still activate, blind ranged enemies and tint the HUD, but no dome is drawn.
  - `timer` and `dir` carry over from the previous visit. `active` carries over if the previous run ended on Mars.
- **Root cause:** A one-shot "spawned" flag outlives the `StageScoped` entity it guards.
- **Repro:** Solo. Play Mars and quit to the menu (ABANDON works). Play Mars again and wait for a storm: the haze and the silenced ranged enemies are present, but there is no dome.
- **Suggested fix:** Reset `*storm = DustStorm::default()` in `enter_run` (next to the Comet reset) and in both stage-transition systems. Alternatively, drop `spawned_vis` and rely on `q_vis.is_empty()`.
- **Status:** Open.

### M11. The Comet Combo only exists for the host's own astronaut

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/comet.rs:42`: `q_player` is `With<LocalPlayer>`.
  - `src/comet.rs:54`: `.single()`.
  - `src/main.rs:251`: `comet_system` is host-only.
  - `src/ui/hud.rs:547`: `update_comet_hud` reads the `Comet` resource, which a client never updates.
- **Symptom:** A joiner can never charge or cash out a comet (the game's signature mechanic), and its comet HUD is blank. The same holds for any peer on the host.
- **Root cause:** `Comet` is a single resource bound to the LocalPlayer.
- **Suggested fix:** Make the comet state a per-astronaut component, run `comet_system` for every `Player` on the host, and send the joiner's comet progress (for example in `PlayerVitals`) for its HUD.
- **Status:** Open.

### M12. meshkit boxes, cylinders and cones have inverted winding

- **Severity:** Medium · **Area:** solo (visual) · **Evidence:** CODE + COMPUTED. The on-screen effect has not been looked at (U2). · **Status:** Fixed (80a6711, P30)
- **Where:**
  - `src/meshkit.rs:44-51`: the `add_box` face table; the ±X and ±Y faces are affected.
  - `src/meshkit.rs:77-116`: `add_cylinder`; all triangles are affected.
  - `src/meshkit.rs:121-153`: `add_cone`; all triangles are affected.
  - Culling is turned off only at `src/combat.rs:90` and `src/events_world.rs:68`.
- **Symptom (expected):**
  - StandardMaterial culls back faces by default, so on these faces the near side is culled and the inside of the far side is drawn.
  - Silhouettes look right, but shading is wrong (lit from behind, or too dark).
  - Affected: astronaut torso, limbs and backpack (`src/player.rs:325-376`), wrecks (`src/planet.rs:278-283`), beacons (`src/planet.rs:318-319`) and enemy parts built with meshkit.
- **Root cause:**
  - For the box's +X face, `v0 = (hx,−hy,−hz)`, `v1 = (hx,−hy,hz)` and `v2 = (hx,hy,hz)` with indices `0,1,2` give a normal of −X: clockwise seen from outside.
  - The ±Z faces and `icosphere()` are counter-clockwise, which is correct. So is Bevy's own `Cuboid`.
- **Repro:** In a windowed build, look at the astronaut from the side. Then set `cull_mode: None` on the rig material and compare.
- **Suggested fix:** Reverse the index order (`0,2,1, 0,3,2`) for the affected box faces and for the cylinder and cone triangles, then compare against Bevy's `Cuboid`.
- **Status:** Fixed (80a6711, P30). **Resolution:** Each meshkit helper now winds its own triangles outward (box faces pick their order from their normal; cylinder and cone triangles were reversed). P04's `build_ccw` workaround (which flipped every triangle, so it broke the already-correct ±Z faces and spheres) is removed. `meshkit::winding_self_check` pins it in the headless rules. U2 settled: windowed screenshots show the daylit astronaut's backpack, limbs and torso were rendering near-black and now shade white with a terminator.

### M13. Save format is fragile: one new counter wipes progress

- **Severity:** Medium · **Area:** dev-hygiene, economy · **Evidence:** CODE · **Status:** Fixed (0b58234, P30)
- **Where:**
  - `src/save.rs:14-29`: `Counters` derives `Deserialize` with no `#[serde(default)]`.
  - `src/save.rs:32`: only `MetaSave` has `#[serde(default)]`.
  - `src/save.rs:115-122`: `load()`, on any parse error, logs `warn!("save corrupt ...")` and returns `Default`.
  - `src/save.rs:147-160`: `save()` does a plain `std::fs::write` (:154), with no temp file and no backup.
  - Writers: `src/director.rs:331`, `src/ui/menus.rs:325`, `:338`, and `src/ui/settings.rs:113`, `:125`.
- **Symptom:**
  - Any of these makes `load()` fail:
    - adding a field to `Counters`;
    - renaming or removing a persisted enum variant (`AstronautKind`, `WeaponKind`, `PlanetKind`, `TomeKind`, `QuestKind` are stored by name in sets and maps).
  - The game then silently starts from a fresh save, and the next `save()`, for example touching a setting, overwrites the old file. That is a full progress wipe.
  - A crash mid-write can truncate the file.
- **Root cause:** Serde defaults are missing on the nested struct, and the failure mode is to overwrite rather than preserve.
- **Suggested fix:**
  - Add `#[serde(default)]` to `Counters` now, before any counter is added.
  - On a parse failure, rename the bad file to `save.json.bak-<unix>` instead of overwriting it.
  - Write to `save.json.tmp`, then rename.
  - Consider a `version` field and tolerant enum loading.
- **Status:** Fixed (0b58234, P30). **Resolution:** `Counters` has `#[serde(default)]` (every save struct now does); the name-keyed sets, maps, the loadout and the palette/number-mode fields load leniently (an unknown hero, weapon, planet, tome, quest or palette drops only that entry); an unreadable file is renamed to `save.json.bak-<unix secs>` before a fresh profile starts; writes go to `save.json.tmp` and are renamed into place. `MetaSave.version` exists since P05. `save::format_self_check` pins all four in the headless rules.

### M14. DEV key B (summon boss) ships ungated

- **Severity:** Medium (two verifiers rated it Low) · **Area:** dev-hygiene, solo, co-op · **Evidence:** CODE · **Status:** Fixed (b5c192f, P30; the `is_simulating` gate was P02's)
- **Where:**
  - `src/enemies.rs:913-936`: `debug_spawn_boss`. Its doc comment says "Remove before ship".
  - `src/main.rs:244`: it is registered in the in-run playing chain, with no `#[cfg]`, no dev flag and no `is_simulating` gate.
- **Symptom:**
  - **Solo or host.** Pressing B during play spawns the planet's stage boss at the player immediately (Craterpillar on the Moon, Anubot elsewhere), shows a "[DEV] ... SUMMONED" banner and sets `boss_spawned`, so the real boss never comes at 1:30. Killing it sets `boss_dead` (`src/combat.rs:959-960`) and opens the teleporter. That is a progression shortcut in release builds.
  - **Joiner.** It spawns a local boss that is never simulated and can never be killed. It stays for the stage and, with its real `max_hp`, wins the boss-bar pick over the proxies (whose `max_hp` is 1).
  - **Same key as Banish.** B is also the Banish key in the level-up panel, so players are trained to press it.
- **Root cause:** A dev tool was left in the shipping build.
- **Repro:** Solo, press B.
- **Suggested fix:** Delete it, or put it behind a dev feature or flag (see SH1), and add `.run_if(net::is_simulating)`.
- **Status:** Fixed (b5c192f, P30; the `is_simulating` gate was P02's). **Resolution:** B (summon boss) and T (replay tutorial) only work with `--dev` (`main::dev_mode`, read once). P28 still owns the wider dev-CLI gating (L56).

### M15. Downed teammates stay downed on every later stage

- **Severity:** Medium · **Area:** co-op · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/director.rs:183-186`: every `PlayerState` is cloned verbatim, including `dead` and `hp = 0`.
  - `src/director.rs:220-224`: the astronauts are respawned with those clones.
  - `src/player.rs:162`: the carried sheet is inserted unchanged.
  - `src/netenemy.rs:792`, `:819`: the client carries its own sheet the same way.
  - `src/player.rs:855`: regen runs only when `hp > 0`.
- **Symptom:** In co-op, if a teammate is down when the survivor takes the teleporter, that teammate arrives on the next planet still dead and stays dead for the rest of the run.
- **Root cause:** There is no revive path. The comment at `src/director.rs:234-239` names reviving as future work ("reviving is just clearing `dead`").
- **Suggested fix:** A revive design (GDD revive / Tumbling Beacon; D10). The minimum: on stage transition, clear `dead` and set `hp` to a fraction of max for carried sheets.
- **Status:** Open.

### M16. Spawn ramp is about 15x the GDD's and caps out the horde

- **Severity:** Medium · **Area:** solo, economy (balance) · **Evidence:** LIVE (the 2026-09-26 log matches the model) + CODE · **Status:** Fixed (6eaf5f8, P30, on top of P01's §3 curve)
- **Where:**
  - `src/enemies.rs:570-576`: the rate is `(1 + 2.1·e/60) · (1 + D) · P` per second. During The Static it is `10 + 0.15·static_timer`.
  - `src/enemies.rs:590-594`: the whole budget is withdrawn, but only `min(budget, room)` spawn. Overflow is discarded.
  - `src/content/enemies.rs:155-166`: the mix. The Bruiser joins at e = 270 s (timer 5:30).
  - `src/content/enemies.rs:238-243`: `time_scaling`; `(1 + D)` multiplies HP.
  - The GDD: `GDD.md:263` gives `SpawnRate(t) = Rate_base × (1 + 0.14·t) × ...`, and `GDD.md:705` says the cap is 1,200 with overflow merging into The Static.
- **Symptom:**
  - Solo at D = 0 spawns about 10/s at timer 5:39, 13/s at 4:12 and 22/s at 0:00, about 6,900 per stage.
  - The horde reaches the 1200 cap mid-stage. In the live run, kills fell from about 8/s to about 2/s after the cap was reached, and the player died about 5:50 into the stage at level 25.
  - Nothing culls or recycles enemies.
  - The Bruiser doubles the mix's mean base HP (13.7 to 27.75).
  - D raises both the rate and HP, so +14% D needs about +30% DPS.
  - Pots take about 57 of the cap's slots (L16).
- **Root cause:** Tuning. The slope is about 15x the GDD's, and there is no pressure valve. The code does exactly what it is written to do.
- **Suggested fix:** Decide the curve (D2), then:
  - add a far-enemy cull or recycle, or merge overflow into The Static as the GDD says;
  - take pots out of the cap.
- **Status:** Fixed (6eaf5f8, P30, on top of P01's §3 curve). **Resolution:** D2 decided by P01 (the GDD's `(1 + 0.14·t)` shape with §3 run-arc beats) and GDD §9 for the overflow. P30: the cap counts the crowd only (L16); at the cap, crowd enemies more than `STATIC_RECYCLE_ARC` (100 m) from every astronaut (never elites or buried Burrowers) dissolve into The Static, farthest first, and the budget spawns fresh over the players' horizon; what still does not fit is banked per stage (`Director::static_backlog`, max 300), told once ("THE STATIC IS GATHERING"), and pours out as extra ghosts when The Static rises. `--overflow` stages all three paths. Measured with `--headless 21000 --balance` (the bot is kept alive; the smoke is not deterministic): on the P01 curve neither the base nor P30 reached the cap during a Moon T1 stage (seeds 1-2 solo peak alive 245-563 base / 245-508 P30; `--coop2` 595 max), kills/s kept pace with spawns/s through 10:00 (about 16-23/s solo, 40-49/s co-op at 9:00-10:00). The cap is reached in The Static: a 32,000-tick run (seed 3) averaged 31.6 kills/s at 917 alive (base, pots in the cap) vs 30.3 kills/s at 994 alive with 5,583 far stragglers recycled (P30) over 750-1,050 s. No human playtest yet.

### M17. No stage or tier scaling: stages 2 and 3 restart at minute-zero pressure

- **Severity:** Medium · **Area:** solo, economy (balance) · **Evidence:** CODE · **Status:** Fixed (P01 chain-wide scaling; 6eaf5f8, P30, the mix)
- **Where:**
  - `src/director.rs:197`: `run.elapsed = 0.0` on each stage change.
  - `src/enemies.rs:563`, `:610`: `time_scaling` and `mix` read `run.elapsed`.
  - `src/content/planets.rs:22-45`: `PlanetDef` has no difficulty field.
  - `run.tier` is read only for the save and the payout (`src/director.rs:168`, `:297`).
  - `src/config.rs:40-42`: `STAGE_SECONDS` is `[600, 540, 480]`, `MINIBOSS_MARKS` is `[420, 120]` and `BOSS_MARK` is 90.
  - The GDD's difficulty shape (`GDD.md:256-264`) has chain-depth `d` and planet `T` terms.
- **Symptom:**
  - On stages 2 and 3, a carried late-game build faces a Shambler-only mix at 1.0x HP again.
  - On stage 3, which is 480 s long, miniboss #1 arrives at e = 60 while the mix is still Shambler-only.
  - Tiers 2 and 3 are not harder per stage than tier 1.
- **Root cause:** No stage or tier term exists anywhere. Greed and Cursed difficulty do carry across stages through `run.difficulty`.
- **Suggested fix:** Add a stage or tier term, for example:
  - feed `time_scaling` and `mix` with `elapsed + stage_offset`, or
  - multiply by a per-planet or per-tier factor (a new `PlanetDef` field), per the GDD.
- **Status:** Fixed (P01 chain-wide scaling; 6eaf5f8, P30, the mix). **Resolution:** P01 made HP/damage/spawn/elite scale on chain-wide run time `t`, depth `d` and planet threat `T`. The remaining reset, the spawn MIX keyed on `run.elapsed`, now reads `scaling::mix_secs`: the countdown (a shorter chained stage joins the arc mid-way) plus `MIX_DEPTH_HEAD_START_SECS` per depth, so stage 3 opens with Sprinters and Spitters and its miniboss #1 meets Bruisers.

### M18. `SpatialHash::near` scans a full cube of cells

- **Severity:** Medium · **Area:** perf · **Evidence:** CODE + COMPUTED (not measured; U7) · **Status:** Fixed (4bf90fc, P30)
- **Where:**
  - `src/enemies.rs:190-203`: `near()` uses `r = ceil(radius / 2.2)` and loops x, y and z over `−r..=r`.
  - `src/config.rs:31`: the cell is 2.2 m.
  - Callers: the homing search runs per Seek or Rocket projectile, per frame (`src/combat.rs:607-618`); the comet (`src/comet.rs:62`, `:97`).
- **Symptom:**
  - A 14 m homing query costs 15³ = 3375 HashMap probes. A 22 m comet cash-out costs 21³ = 9261.
  - With dozens of live seekers (for example BladeStorm), that is well over 100k probes per frame.
  - Clients pay this too, because their cosmetic weapons are ungated.
  - The release build held 165-190 fps at the cap in solo on the user's machine, so the cost is currently hidden.
- **Root cause:** A 3D cube scan for a population that lives on a thin spherical shell.
- **Suggested fix:** Measure first. Then:
  - visit only cells within the query sphere and near the planet surface;
  - use a coarser grid for large radii;
  - cache one nearest-target query per projectile group.
- **Status:** Fixed (4bf90fc, P30). **Resolution:** `SpatialHash::near` walks only the cells that touch the query ball (exact per-column z range) and cross the members' shell (min/max radius measured at each rebuild). `enemies::spatial_hash_self_check` compares it with brute force over a 1,200-member crowd and reports probes: 14 m 684 (was 3,375), 22 m 1,613 (was 9,261), the per-enemy 2.2 m separation query 17 (was 27). Wall-clock not profiled (U7).

### M19. ABANDON RUN leaks the PAUSED overlay over Results and menus

- **Severity:** Medium · **Area:** solo, UI · **Evidence:** CODE · **Status:** Fixed (0486601, P02 for Results/menus; b5c192f, P30 for the death beat)
- **Where:**
  - `src/ui/panels.rs:577-581`: ABANDON sets `result = Death` and `phase = Dead`.
  - `src/ui/panels.rs:584`: the `_ => {}` arm, so the Dead phase never despawns `PauseRoot`.
  - `src/ui/panels.rs:536`: `PauseRoot` is a root node with `GlobalZIndex(20)`. It is neither `StageScoped` nor under `HudRoot`.
  - `src/main.rs:141-144`: the OnExit(InRun) cleanup does not touch it.
  - `src/main.rs:305-307`: `pause_panel` runs only InRun.
- **Symptom:** After ABANDON RUN, the dimmed PAUSED panel, with its RESUME, SETTINGS and ABANDON buttons, stays on top of the Results screen and the main menu until the next run's first Playing frame. Its dead buttons block clicks on the menu buttons they overlap.
- **Root cause:** Only the Playing arm of `pause_panel` despawns the overlay.
- **Repro:** Solo. Press ESC, then ABANDON RUN.
- **Suggested fix:** Despawn `PauseRoot` in every phase except `Paused` (move the despawn loop out of the Playing arm), and also on OnExit(InRun).
- **Status:** Fixed (0486601, P02 for Results/menus; b5c192f, P30 for the death beat). **Resolution:** P02's `despawn_panels` strips the overlay on OnExit(InRun). `pause_panel` now also clears it in every phase but Paused, so it no longer lingers over the 1.6 s Dead beat after ABANDON.

### M20. HOST CO-OP's LAN-address note is effectively never shown

- **Severity:** Medium · **Area:** co-op, UI · **Evidence:** CODE · **Status:** Fixed (7334b28, P30)
- **Where:**
  - `src/ui/menus.rs:265-270`: the note is written, then `next.set(CharSelect)` runs in the same click.
  - `src/ui/menus.rs:164`: `CoopNoteText` exists only under the main menu root, which is despawned on OnExit(MainMenu) (`src/main.rs:135`).
  - `src/main.rs:176-179`: `join_panel_sync`, the text's only writer, runs only in MainMenu.
  - `src/net.rs:1092`: `start_host` logs only the port, not the IP.
- **Symptom:** The host goes straight to hero select and never sees "HOSTING — tell the other player to join: <ip>". The address is visible only after pressing BACK to the main menu. This breaks docket item 9's instructions.
- **Root cause:** The note widget lives on a screen that is torn down in the same frame the note is written.
- **Repro:** Click HOST CO-OP.
- **Suggested fix:** Show `CoopNote` on CharSelect and PlanetSelect, and on the host's in-run HUD. Also write the IP to the session log.
- **Status:** Fixed (7334b28, P30). **Resolution:** While hosting, the hero and world pickers show the hosting line, the run opens with a "HOSTING: TEAMMATES JOIN AT <ip>" banner, the host's pause menu repeats the address, and the session log records it. Seen windowed.

### M21. A failed or unreachable join needs a game restart

- **Severity:** Medium · **Area:** co-op, netcode, UI · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/net.rs:1124-1126`: `NetRole::Client` is inserted as soon as the socket exists, before any handshake.
  - `src/net.rs:1028-1035`: `ClientState` changes are only logged.
  - `src/net.rs:1133-1139`: `disconnect()` has no caller.
  - `src/ui/menus.rs:749-752`: `try_join` refuses when `role.is_networked()`.
  - `src/ui/menus.rs:252-257`: HOST and JOIN refuse with "restart to change role".
  - `src/ui/menus.rs:759-760`: the "CONNECTING" note stays up indefinitely.
- **Symptom:** A mistyped or unreachable IP shows "CONNECTING to <ip> — waiting for the host's world…" forever, and every retry says "already in a co-op session — restart to change role". There is no way to leave a session without closing the game.
- **Root cause:** The role is committed before the connection exists, and nothing reacts to a failed connection.
- **Repro:** JOIN CO-OP with an IP where no host is running.
- **Suggested fix:**
  - On the client, watch for `ClientState::Disconnected` plus a connect timeout. When either fires, call `net::disconnect`, reset `MyPlayerId` and `RunSync`, and show "could not reach host".
  - Add a LEAVE button, and allow a role change after a disconnect.
- **Status:** Open.

### M22. Edge markers vanish for over-horizon targets that project on screen

- **Severity:** Medium · **Area:** UI · **Evidence:** COMPUTED (not observed) · **Status:** Fixed (7334b28, P30)
- **Where:** `src/ui/hud.rs:488-499`. A target counts as "visible" whenever `world_to_viewport` lands inside the viewport; there is no occlusion test.
- **Symptom:** Computed for the Moon (R = 140) with the default camera (`CAM_DISTANCE` 7.5, pitch 0.55, FOV π/4): targets about 16-92° of arc ahead, roughly 40-225 m, are hidden behind the planet's limb but still project inside the viewport. They get no edge marker, and that is the main case the markers exist for: a boss, teleporter or shrine over the horizon.
- **Root cause:** A viewport-rectangle test is used as a visibility test.
- **Repro:** Not observed. Stand so that a shrine is about 60-150 m ahead over the horizon and look for its marker.
- **Suggested fix:** Treat a target as visible only if the camera-to-target segment does not intersect the planet sphere. Otherwise draw the marker clamped to the screen edge in the target's direction.
- **Status:** Fixed (7334b28, P30). **Resolution:** A target counts as in view only if the camera-to-target segment clears the planet (a ball at the mean radius, or at the target's own ground if lower, minus `EDGE_MARKER_OCCLUDER_INSET`); otherwise it gets the edge marker.

### M23. Joiner combat and boss feedback is largely missing

- **Severity:** Medium · **Area:** co-op, UI · **Evidence:** CODE · **Status:** Open
- **Where:**
  - `src/combat.rs:933`, `:994` and `src/pickups.rs:272`: the only `NumberMsg` writers, all host-only.
  - `src/ui/hud.rs:380`: the hurt vignette reads `ps.iframes`.
  - `src/net.rs:592-593`: `adopt_my_vitals` copies only `hp` and `dead`.
  - `src/combat.rs:1007-1010`: hurt shake and SFX, for the local player only.
  - Banners and BossRoar: `src/director.rs:81-111`, `src/enemies.rs:771`, `:1587`, `src/pickups.rs:360`, all host-only.
  - `src/netenemy.rs:1133-1159` and `:931-960`: proxies carry no `BaseMat`, so `enemy_flash` never flashes them (`src/enemies.rs:1650-1652`).
- **Symptom:** On the joiner:
  - no damage, heal or dodge numbers;
  - no hurt vignette or shake;
  - no crit, pot or hurt SFX;
  - no miniboss, boss, teleporter or Static banners, and no roar;
  - no hit-flash on enemies, only the scale pulse.

  The joiner does get the boss HP bar, boss edge markers, the "THE STATIC m:ss" timer, the Static music stem and the "STAGE N" banner.
- **Root cause:** Feedback is produced as a side effect of host-only simulation, and none of it is relayed.
- **Suggested fix:**
  - Add a compact per-client feedback lane. The host already knows which hits involve the joiner: `HitMsg.source` for its shots and `PlayerHitMsg.victim` for hits on it. Batch number, SFX and banner events addressed to that client.
  - Copy `iframes` into `PlayerVitals`.
  - Give proxies a `BaseMat`.
- **Status:** Open.

---

## 4. Low

Evidence is CODE unless stated otherwise. Each item carries its own Status.

### 4.1 Movement and simulation

#### L1. Air-hopping reaches the 2.1x speed cap without sliding

- **Severity:** Low (the world verifier lowered it from Medium) · **Area:** solo · **Status:** Fixed (a0e0ca9 / 889fafa, P06)
- **Where:**
  - `src/player.rs:434-453`: air acceleration is `55 × 0.35 = 19.25 m/s²`, with no per-direction limit. The hard cap applies unless the astronaut is grounded, not sliding and past `BHOP_WINDOW`.
  - `src/config.rs:4-8`, `:16`: the movement constants and `SPEED_HARD_CAP`.
- **Symptom:**
  - Holding W and chaining jumps reaches 17.85 m/s, which is 2.1x run speed, about 0.49 s into a 0.73 s jump. Re-jumping within 0.16 s of landing keeps that speed.
  - A slide with W held reaches the same cap about 0.07 s after its boost.
  - `GDD.md:321-322` says bunny-hopping preserves slide momentum and that air control is "not cheese".
- **Root cause:** Air control is uncapped up to the hard cap.
- **Repro:** Solo. Hold W and hold Space.
- **Suggested fix:** Design question D3. If it is not intended, cap in-air acceleration at the run speed and let only slide landings exceed it.
- **Resolution:** D3 decided by P06: `player::steer` lets a wish steer above run speed but never add to it, so plain hops keep run speed and only slides, slopes and rails bank more.

#### L2. Jump and slide edges are sent once, on an unreliable channel

- **Severity:** Low · **Area:** co-op, netcode · **Status:** Open
- **Where:**
  - `src/player.rs:411-412`: the edges come from `just_pressed`.
  - `src/net.rs:352`: `PlayerInputMsg` is on `Channel::Unreliable`.
  - `src/net.rs:780-784`: the edges ride only the frame of the press.
- **Symptom:** If that one packet drops, the host never jumps or slides the joiner, but the joiner's prediction already did. The positions diverge permanently (H5).
- **Root cause:** One-shot edges on a lossy lane.
- **Repro:** Needs packet loss; not reproduced.
- **Suggested fix:** Add monotonically increasing jump and slide counters to `PlayerInputMsg`. The host acts when a counter advances, so any later packet delivers the press. Combine this with the H4 fix.

#### L3. `player_input` and `player_physics` have no ordering

- **Severity:** Low · **Area:** solo · **Status:** Fixed (b5c192f, P30)
- **Where:** `src/main.rs:219` (in a chained group) and `src/main.rs:274` (in an unchained group). No `.before` or `.after` relates the two.
- **Symptom:** Whether a jump or slide is applied before or after this frame's integration and landing detection is decided by the scheduler. That can add or remove a frame of latency, or change bunny-hop timing, between runs.
- **Root cause:** Both take `&mut Player`, so they never run in parallel, but their order is unspecified.
- **Suggested fix:** Add `player_physics.after(player_input)`.
- **Resolution:** `player_physics.after(player_input)`.

#### L4. Coyote time is dead logic

- **Severity:** Low · **Area:** solo, dev-hygiene · **Status:** Resolved (b5c192f, P30)
- **Where:** `src/player.rs:458`, `:485`, `:536`, `:578`.
- **Symptom:** Coyote time never extends a jump. `coyote` is set to 0.12 only while grounded and is zeroed on a jump. Height is measured relative to the terrain, so the only way to become airborne is to jump.
- **Root cause:** Coyote time is a platformer feature with no ledges to walk off here.
- **Suggested fix:** Remove it, or document it as reserved for props with height.
- **Resolution:** No longer dead: P06's Grind-Lines ride `GRIND_RAIL_LIFT` above the ground and set `coyote`, so running off a rail's end is a real coyote window. The field's doc now says so.

#### L5. `InputIntent.forward` and `.interact` are never read; comments are false

- **Severity:** Low · **Area:** dev-hygiene · **Status:** Comments fixed (b5c192f, P30); routing E through `InputIntent` is P14's
- **Where:**
  - `src/player.rs:73`, `:76`: the fields. The comment at :73 says `forward` "drives facing/aim"; it does not.
  - `src/player.rs:380-381`: the comment says this is "the only place hardware input is read".
  - Direct hardware reads also exist at `src/interact.rs:461` (E), `src/enemies.rs:925` (B), `src/tutorial.rs:43` (T), and in the camera's mouse handling (`src/player.rs:749`, `:785-787`).
- **Symptom:** None at runtime. The comments mislead anyone routing input over the network.
- **Root cause:** Aim uses auto-target or `Player.facing` (`src/combat.rs:294-301`). Interact reads the keyboard directly.
- **Suggested fix:** Either route aim and interact through `InputIntent`, which docket item 3 needs for interact anyway, or correct the comments.
- **Resolution:** `forward` is read now (P06's Antipode Blink turns momentum about it); the comments say what the fields do and which inputs still read hardware. Routing the local E press through `InputIntent.interact` belongs to P14 (co-op peer interactables, docket items 2-3).

### 4.2 World and visuals

#### L6. Per-planet sky colour and `meteor_showers` are unused (gray backdrop)

- **Severity:** Low · **Area:** solo, UI · **Status:** Sky fixed (e4404df, P30); meteor showers are P09's
- **Where:**
  - `src/content/planets.rs:35`: `sky`.
  - `src/content/planets.rs:44`: `meteor_showers`.
  - `src/main.rs:337-346`: `setup_camera`. No `ClearColor` is set anywhere in `src/`.
- **Symptom:** Every planet's space backdrop is Bevy's default clear colour `srgb_u8(43,44,47)`, a mid gray after tonemapping, not the near-black per-planet sky. There is no meteor system.
- **Root cause:** The fields were defined but never wired up. The build's `dead_code` warning at `src/content/planets.rs:23` flags them.
- **Suggested fix:** Insert `ClearColor(planet.sky)` in `enter_run` and at stage changes (host and client). Track meteor showers as content (§7).
- **Resolution:** `planet::spawn_stage` sets `ClearColor(def.sky)` on every machine. `meteor_showers` stays unread until P09 builds the Moon's METEOR SHOWER event.

#### L7. Interactables can spawn inside large boulders

- **Severity:** Low · **Area:** solo · **Status:** Fixed (e4404df, P30)
- **Where:**
  - `src/interact.rs:124-132`: `place_dir` avoids only the player's spawn.
  - `src/interact.rs:134-144`: `spawn_interactables` never sees `PropColliders`.
  - `src/planet.rs:227-241`: boulders have scale 3-6, collider radius `0.7 × s` and height `1.4 × s`.
  - `src/interact.rs:425-426`: the use range is 4.5 m.
- **Symptom:** A chest, shrine or other interactable centred in a boulder with scale above about 5.8 cannot be reached. The push-out radius exceeds the 4.5 m use range, and the collider is taller than the roughly 1.45 m jump apex.
- **Root cause:** No overlap check between props and interactables.
- **Repro:** Rare; the frequency is unknown (U6).
- **Suggested fix:** Pass `PropColliders` into `spawn_interactables` and retry placements that fall inside a collider. The extra draws must stay deterministic, so see M1 first.
- **Resolution:** `interact::place_dir` keeps every pot and interactable `INTERACT_PROP_CLEARANCE` off each solid prop (a machine-independent stream: props come from the stage seed); the headless summary fails if any is inside one. U6 moot.

#### L8. Terrain mesh is rebuilt synchronously on every stage entry

- **Severity:** Low · **Area:** perf · **Evidence:** CODE (the hitch is unmeasured; U7) · **Status:** Fixed (e4404df, P30)
- **Where:** `src/planet.rs:98-130`; `src/meshkit.rs:182-224` (`icosphere(7)`).
- **Symptom:** Each stage entry, on host and client, builds 163,842 vertices and 327,680 triangles with a HashMap midpoint cache. That includes `Terrain::height` per vertex (about 23 `sin` calls plus crater terms) and `compute_smooth_normals`. Expect a hitch at stage transitions.
- **Root cause:** Synchronous generation inside a system.
- **Suggested fix:** Cache the mesh per planet kind, since terrain comes from a constant `PlanetDef` seed rather than the run seed. Or build it on `AsyncComputeTaskPool` during the transition.
- **Resolution:** The terrain mesh is cached per session under a `Handle::Uuid` keyed by the terrain constants, so re-entering a world (every retry, every later chain) reuses it. The first visit per world per session still builds synchronously.

#### L9. Every astronaut rig allocates its own meshes, materials and a shadowed spotlight

- **Severity:** Low · **Area:** perf · **Status:** Fixed (e4404df, P30)
- **Where:**
  - `src/player.rs:183-313`: `build_astronaut_rig` creates 8 meshes and 5 materials, plus a `SpotLight` with range 55 and `shadows_enabled: true` (:298-307).
  - It is called from `spawn_player` (`src/player.rs:174`) and from `src/remote.rs:109`.
- **Symptom:** The allocations repeat on every stage respawn, for every astronaut. With four players, four spot shadow maps render on top of the directional cascade. The NETCODE NOTES already list this as open (`src/net.rs:1247-1251`).
- **Root cause:** No shared rig assets, and shadows are on for every flashlight.
- **Suggested fix:** Build rig meshes and materials once, as a resource per hero. Disable shadows on remote flashlights.
- **Resolution:** Rig meshes and colour-keyed materials are shared `Handle::Uuid` assets; only the per-astronaut flashlight lens (its own F switch) is allocated per rig. Only the local astronaut's spotlight casts shadows.

#### L10. Craterpillar Jr looks different on host and joiner

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:** `src/enemies.rs:647-655`: on the host, `is_worm` is only `Craterpillar`, so Jr uses the generic boss mesh. `src/netenemy.rs:924-926`: on the client, `Craterpillar | CraterpillarJr` both get the worm head, and body segments are added only for Craterpillar (:967).
- **Symptom:** The host sees a generic boss blob; the joiner sees a bodiless worm head.
- **Suggested fix:** Pick one look and use the same `BossKind` match on both sides.

#### L11. Boss proxies: arbitrary boss-bar pick and fixed rotation

- **Severity:** Low · **Area:** co-op, UI · **Status:** Open
- **Where:**
  - `src/ui/hud.rs:576-580`: the pick is the "beefiest" boss by strict `max_hp >`.
  - `src/netenemy.rs:941`: every boss proxy has `max_hp: 1.0`.
  - `src/netenemy.rs:1024`: the rotation is `frame_quat(up, tangent_frame(up).0)`.
- **Symptom:**
  - Two or more bosses can be alive at once: Craterpillar Jr (timer 7:00) or Rover Gone Wrong (timer 2:00) if it has not been killed, plus the stage boss from timer 1:30 (`src/config.rs:41-42`). The joiner's boss bar then shows whichever proxy comes first in query order.
  - Boss proxies never face their heading.
- **Suggested fix:** Put the boss's absolute `max_hp` (or a priority, such as stage boss over miniboss) on the proxy. Derive the facing from the motion between snapshots.

#### L12. Buried burrowers can be hit by some weapons; the joiner draws them above ground

- **Severity:** Low · **Area:** solo, co-op · **Status:** Open
- **Where:**
  - `src/enemies.rs:473-481`: `rebuild_hash` has no `Buried` filter.
  - `src/combat.rs:587`: `projectile_move`'s enemy query has no filter either. That covers homing, collisions, explosions and drones (:610, :671, :723, :766) and the comet (`src/comet.rs:62`, `:97`).
  - By contrast, auto-aim, aura and melee (`src/combat.rs:218`, `:266`, `:308`) and beams (`:794`) exclude `Buried`.
  - `src/netenemy.rs:324-329`: `stream_enemies` has no `Buried` filter.
- **Symptom:** During the 1.3 s buried phase, projectiles, drones and the comet can kill a burrower underground. On clients, buried burrowers are drawn standing on the surface.
- **Suggested fix:** Exclude `Buried` from the hash, or from `projectile_move` and the comet. Stream a buried flag, or skip buried enemies in the crowd lane.

#### L13. Teammate rigs on the joiner are coloured by slot, not by hero

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:** `src/remote.rs:85`: `AstronautKind::ALL[(pid.0 as usize) % AstronautKind::ALL.len()]`.
- **Symptom:** On the joiner, the host and other teammates wear an arbitrary hero's suit chosen by player id. Adding a hero reshuffles the colours.
- **Root cause:** The hero is not replicated (M5).
- **Suggested fix:** Replicate the character, for example as a field in `PlayerVitals` or as its own replicated component, and build the rig from it.

### 4.3 Combat and horde

#### L14. Owning a cryo weapon adds a flat slow to all of that player's hits

- **Severity:** Low · **Area:** solo · **Status:** Fixed (ccadf44, P30)
- **Where:**
  - `src/combat.rs:277`: `let _ = slow; // applied in apply_hits via kind check`.
  - `src/combat.rs:894-902`: `has_cryo` is true if the shooter owns CryoVent or AbsoluteZero.
  - `src/combat.rs:930-932`: `e.slow = (e.slow + 0.25).min(0.65)` on every hit from that shooter, thorns included.
  - `src/content/weapons.rs:327`, `:339`: the per-weapon slows, 0.45 and 0.75, which are unused.
- **Symptom:** Every weapon of a cryo owner slows enemies by a flat 0.25. The two cryo weapons do not differ in slow.
- **Suggested fix:** Carry the slow on the `HitMsg` from the aura that produced it, and apply only that value.
- **Resolution:** The aura pulse sends `SlowMsg { target, slow }` with its weapon's slow; `apply_hits` raises the enemy's slow to at least that. Other weapons no longer slow.

#### L15. The aura bubble is drawn smaller than its damage radius

- **Severity:** Low · **Area:** solo, UI · **Status:** Fixed (ccadf44, P30)
- **Where:**
  - `src/combat.rs:252-265`: the damage radius is `radius × aura_scale × size`, where `size = (1 + 0.06 × (lvl − 1)) × stats.size`.
  - `src/combat.rs:571`, `:851`: the visual scale is `radius × aura_scale` only.
- **Symptom:** As the weapon levels up or the Size stat grows, the bubble understates the real area. At weapon level 7 the damage area is 36% wider than drawn.
- **Suggested fix:** Scale the visual by the same `size` factor.
- **Resolution:** `aura_follow` scales by the weapon level's size and the Size stat, like the damage radius.

#### L16. Pots and bosses count toward the enemy cap and the `enemies=` figure

- **Severity:** Low · **Area:** solo, perf · **Evidence:** LIVE (the log's first line: `enemies=64` = 60 pots + 4 enemies) · **Status:** Fixed (6eaf5f8, P30)
- **Where:**
  - `src/interact.rs:161-193`: a pot is `Pot` plus `Enemy { speed 0, contact_cd INF, hp 1 }`.
  - `src/enemies.rs:548`, `:562`: the cap count is `Query<(), With<Enemy>>`.
  - `src/content/planets.rs:71`, `:95`, `:119`: 60, 68 and 38 pots.
  - `src/playlog.rs:96`, `:122`: `enemies=` counts the same query.
  - `src/music.rs:286-287`: the music "density" also counts pots.
- **Symptom:** The effective horde cap is about 1140 on the Moon. The `enemies=` log figure and the music intensity include static pots, and on clients they also include proxies.
- **Suggested fix:** Count `(With<Enemy>, Without<Pot>, Without<Boss>)` for the cap, the log and the music. Consider taking pots out of `Enemy` altogether (they still need the hash for hits).
- **Resolution:** The live cap, the session log's `enemies=` and the music density count `(With<Enemy>, Without<Pot>, Without<Boss>)`. Pots stay `Enemy` (they need the hash for hits).

#### L17. Downed astronauts still anchor spawns, party scale and boss placement

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:**
  - `src/enemies.rs:547`, `:556`, `:568`: `director_spawn` uses every `Player` as an anchor and computes P from them, despite the comment "one per living astronaut" (:554).
  - `src/director.rs:66-72`: the boss spawn centroid uses every `Player`.
  - By contrast, `enemy_move` and `enemy_contact` do filter out the dead (`src/enemies.rs:1038`, `:1249`).
- **Symptom:** A dead teammate's body keeps the horde at the full party size, receives its share of spawns around it, and pulls bosses toward it.
- **Suggested fix:** Filter out dead astronauts via `PlayerState.dead` in both places.

#### L18. A downed astronaut's body absorbs enemy shots

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:** `src/enemies.rs:1527`, `:1535`, `:1549-1555`: `enemy_projectiles` tests every `Player` and despawns the shot on the first hit. `src/combat.rs:989`: `apply_player_hits` ignores victims at `hp <= 0`.
- **Symptom:** A downed teammate acts as a bullet shield.
- **Suggested fix:** Skip dead astronauts in `enemy_projectiles`.

#### L19. A downed astronaut keeps charging shrine rings

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:** `src/interact.rs:345`, `:354-362`: the occupancy query is `Query<&Transform, With<Player>>`, with no dead filter.
- **Symptom:** A dead body standing in a ring keeps charging it.
- **Suggested fix:** Filter out dead astronauts, as `src/pickups.rs:142` does.

#### L20. Boss HP ignores party size; boss add rings bypass the cap

- **Severity:** Low · **Area:** co-op · **Status:** Open
- **Where:** `src/enemies.rs:645`: `hp = def.hp × (1 + difficulty)`. `src/enemies.rs:775-791`: the phase add rings (11 and 14 adds) call `spawn_enemy` with no cap check.
- **Symptom:** Co-op bosses die P times faster than intended. Add rings can push the count above the cap.
- **Suggested fix:** Scale boss HP by a party factor (per the GDD's M5 milestone), and respect the cap for adds.

#### L21. `NET_ENEMY_MAX_RECORDS` is not a hard ceiling

- **Severity:** Low · **Area:** netcode, perf · **Status:** Open
- **Where:**
  - `src/netenemy.rs:346-365`: spawn descriptors and near-band updates are appended with no check.
  - `src/netenemy.rs:375-383`: only the far tier checks the limit.
  - `src/config.rs:88-89` documents it as a "Hard ceiling on records per snapshot", and `src/netenemy.rs:27-28` says the 1200-record ceiling "is what actually bounds the worst case".
- **Symptom:** With 2 players the cap is 2100 enemies (3600 with 4). A client re-entering a crowded area can receive more than 1200 records in one snapshot, at 15 Hz.
- **Suggested fix:** Enforce the budget for spawns too, deferring the excess to the next snapshot, or correct the documentation.

### 4.4 Economy and progression

#### L22. A joiner's gold ignores its Gold Gain

- **Severity:** Low (Medium once joiners can spend gold, after docket item 5) · **Area:** co-op, economy · **Status:** Open
- **Where:**
  - `src/pickups.rs:255-257`: the host credits `round(g × gold_gain)` to its copy of the peer.
  - `src/net.rs:676`: it relays the raw `g` in `LootGrantMsg`.
  - `src/net.rs:717`: the client adds `m.gold` unmultiplied.
  - `src/run.rs:624`: GoldPile cards add only to the client's sheet.
- **Symptom:** The joiner's HUD gold is lower than it should be with Doug (+25%), the Golden Antenna or the Golden Tome. There is no other effect today, because joiners cannot spend gold.
- **Suggested fix:** Relay the multiplied amount, or apply `gold_gain` on the client. Decide which sheet owns gold before item 5.

#### L23. `greed_stacks` is not replicated

- **Severity:** Low · **Area:** co-op, economy · **Status:** Open
- **Where:**
  - `src/net.rs:99-119`: `RunSnapMsg` has no `greed_stacks`.
  - `src/interact.rs:483`: the only increment, which is host-only.
  - `src/run.rs:267-268`: each stack gives +0.12 difficulty and +0.08 luck.
  - Client recompute sites: `src/director.rs:139-140` and `src/ui/panels.rs:233`, `:351`, `:471`.
- **Symptom:** The joiner's derived stats never include greed luck, and the host adopts those stats from `PlayerBuildMsg`. World difficulty is barely affected, because it is the maximum over players (`src/player.rs:852`).
- **Suggested fix:** Docket item 4: add the field to `RunSnapMsg`, and recompute the LocalPlayer's sheet when it changes.

#### L24. Chests and shops can exceed an item's max stacks

- **Severity:** Low · **Area:** economy · **Status:** Fixed (3e8660c, P03; ccadf44, P30)
- **Where:**
  - `src/ui/panels.rs:345-349` (chest) and `:463-469` (shop): the item is pushed or incremented with no cap check.
  - `src/interact.rs:241-249`: shop stock is rolled once at stage entry, and duplicates are possible.
  - `src/interact.rs:471`: the chest item is memoised on first open.
- **Symptom:** An item can go past its cap. For example, SplitterChip can reach 3/2, giving +3 projectiles.
- **Suggested fix:** Check `item_count < max_stacks` at purchase and take time; show the offer as "MAXED".
- **Resolution:** P03 guards the shop purchase and re-rolls a chest item that became unavailable; P30 shows a capped offer as MAXED with no BUY.

#### L25. The microwave is used up even when nothing fits

- **Severity:** Low · **Area:** economy · **Status:** Fixed (73dec7c)
- **Where:** `src/interact.rs:516-531`: `run.microwave_used = true` is set at :519, before the "NOTHING FITS" early return at :528-530.
- **Symptom:** Using the microwave while every owned item is maxed shows "NOTHING FITS IN THE MICROWAVE" and still consumes it for the stage.
- **Suggested fix:** Set `microwave_used` only after at least one option exists.
- **Resolution:** Verified: the use is spent only after at least one option exists.

#### L26. The `roll_item` fallback ignores bans and stack caps

- **Severity:** Low · **Area:** economy · **Status:** Fixed (3e8660c, P03)
- **Where:** `src/interact.rs:104-110`: the fallback filters only on `item_count < max_stacks`, not `banned_items`, and then `unwrap_or(&ItemKind::SpaceBorgar)` with no stack check.
- **Symptom:** Banished items can come back from chests, shops, shrines or the Moai. When everything is maxed, SpaceBorgar is offered past its cap.
- **Suggested fix:** Apply the ban filter in the fallback. If nothing is available, offer gold or silver instead.
- **Resolution:** Verified: `run::roll_item` filters bans and caps (`item_available`); its last resort is the uncapped Space Borgar. Residual edge: a banished Borgar can still be that last resort when every other item is capped or banished.

#### L27. `static_secs_best` only counts the final stage

- **Severity:** Low · **Area:** economy · **Status:** Fixed (0970452, P30)
- **Where:** `src/director.rs:202`: `static_timer` is zeroed on each non-final stage change. `src/director.rs:290`: only the final value is banked.
- **Symptom:** Time survived in The Static on earlier stages never counts toward SurviveStatic2Min.
- **Suggested fix:** Track a per-run maximum in `RunState` that `stage_transition` does not reset.
- **Resolution:** `RunState::static_secs_peak` keeps the longest Static stretch on any stage; banking takes the max of it and the final stage's timer.

#### L28. Tutorial promises a boss chest; its step 5 never fires on a joiner

- **Severity:** Low · **Area:** solo, UI · **Status:** Fixed (ccadf44, P30; the chest is P01's cache)
- **Where:**
  - `src/tutorial.rs:26`: "It'll drop a chest."
  - `src/pickups.rs:338-364`: minibosses give elite drops, and stage bosses drop 14 gold piles. Nothing drops a chest.
  - `src/tutorial.rs:78`: step 5 waits on `run.minibosses_spawned[0]`, which is not in `RunSnapMsg`.
- **Symptom:** The tutorial's last line is false. On a joiner with a fresh save, the tutorial stops before that line.
- **Suggested fix:** Change the text, or make minibosses drop a chest. Add `minibosses_spawned` to `RunSnapMsg`, or key the step on the boss bar.
- **Resolution:** P01's guaranteed miniboss cache makes "It'll drop a chest" true. Step 5 now waits for any `Boss` on the field (the host's or a joiner's streamed proxy) instead of host-only `minibosses_spawned`.

#### L29. Tutorial says Shift slides; slide is Ctrl or C

- **Severity:** Low · **Area:** UI · **Status:** Fixed (a0e0ca9, P06)
- **Where:** `src/tutorial.rs:24` ("Shift to slide"); `src/player.rs:412` (`ControlLeft` or `KeyC`). Nothing binds Shift.
- **Symptom:** New players are told the wrong key.
- **Suggested fix:** Fix the text, or bind Shift as well. See also SH13.
- **Resolution:** Shift (and Ctrl, C) slide.

#### L30. Lady Fortuna's rerolls are unlimited; her +2 refreshes are dead

- **Severity:** Low · **Area:** economy · **Status:** Fixed (ccadf44, P30)
- **Where:**
  - `src/run.rs:229-232`: Fortuna starts with +2 refreshes.
  - `src/ui/panels.rs:190-196`: Fortuna never spends a refresh and may reroll without limit.
  - `src/ui/panels.rs:117-118`: the button says "(FREE)".
  - `GDD.md:411`: "Free level-up reroll each level".
- **Symptom:** Fortuna can reroll every level-up without limit, and the +2 refreshes are never used.
- **Suggested fix:** Design question D8. One free reroll per level-up per the GDD, then paid refreshes.
- **Resolution:** D8 decided per GDD §5: one free reroll per level-up hand (`PlayerState::level_reroll`), spent before the run's free refreshes, then Gold. Pinned by the rules self-check and `--choices`.

#### L31. S is both move-back and level-up Skip

- **Severity:** Low · **Area:** UI · **Status:** Open
- **Where:** `src/ui/panels.rs:183` (Skip on `just_pressed(KeyS)`) and `:208-212` (+10 gold, the level is consumed); `src/player.rs:400` (S is backward movement).
- **Symptom:** Pressing S while backing away as a panel appears throws away the level-up for 10 gold.
- **Suggested fix:** Bind Skip to a key that is not used for movement, or require a short delay after the panel opens before keys count.

#### L32. Four stats are never granted by anything

- **Severity:** Low · **Area:** economy · **Status:** Open
- **Where:**
  - `src/stats.rs`: Shield, EliteDamage, Knockback and SilverGain, with defaults 0.0, 1.0, 1.0 and 1.0.
  - Their consumers exist:
    - shield: `src/player.rs:861-862`, `src/run.rs:228`;
    - elite damage: `src/combat.rs:269` and others;
    - knockback: `src/combat.rs:322`, `src/comet.rs:105`;
    - silver gain: `src/pickups.rs:263`, `src/director.rs:298`.
- **Symptom:** No item, tome, passive or shrine raises these stats, so their code paths never matter.
- **Suggested fix:** Add content that grants them (see `plan/`), or remove them.

#### L33. Idle pickups dirty their Transform every frame; gold, silver and food piles are uncapped

- **Severity:** Low · **Area:** perf · **Status:** Open
- **Where:**
  - `src/pickups.rs:197-201`: the idle bob and spin assign `Transform` every frame.
  - `src/pickups.rs:381-390`: `gem_merge` merges XP only (`GEM_CAP` 550, `src/config.rs:36`).
  - Static ghosts drop Silver(1) 50% of the time (`src/pickups.rs:329-333`).
- **Symptom:** Change detection and transform propagation run for every idle pickup every frame. Gold, silver and food can pile up without bound within a stage (they are `StageScoped`).
- **Suggested fix:** Animate the pickups' visual child only, or use a shader. Merge or cap non-XP piles.

### 4.5 Netcode and session flow

#### L34. `stream_pickups` same-frame race: ghost gem or panic

- **Severity:** Low · **Area:** netcode · **Evidence:** CODE (which outcome actually happens is unknown; U3) · **Status:** Open
- **Where:**
  - `src/netenemy.rs:612-648`: the `PickupNetId` insert is deferred (:615), and `known` is rebuilt only from entities that already carry the id (:642-648).
  - `src/pickups.rs:172-195`: collection within 0.8 m despawns the pickup in the same frame (:181, :194). The two systems are not ordered.
  - `assign_net_ids` has the same shape (`src/netenemy.rs:266-268`).
- **Symptom:** A gem that drops within about 0.8 m of an astronaut and is collected in the same frame it is first streamed goes one of two ways:
  - if the insert applies before the despawn, the entity never enters `known` and no Despawn is sent, so the joiner sees a ghost gem until the stage ends;
  - if the despawn applies first, `insert` on a dead entity panics under Bevy's default error handler.
- **Suggested fix:** Use `try_insert`. Treat the Spawn event just sent as known, so that its disappearance on the next frame emits a Despawn. Do the same in `assign_net_ids`.

#### L35. `RunSnapMsg` is unordered and unsequenced

- **Severity:** Low · **Area:** netcode · **Status:** Open
- **Where:**
  - `src/net.rs:366`: `Channel::Unordered`, which is reliable but unordered. There is no sequence field.
  - `src/net.rs:531-534`: any stage mismatch sets `pending_stage`.
  - `src/netenemy.rs:800-802`: a rebuild clears the proxy indices.
- **Symptom:** A retransmitted older snapshot can arrive after a newer one. The timer and kill count jump backwards. At a stage boundary, the joiner does an extra full rebuild back to the old stage and then forward again, and the cleared indices then cause the H6 invisible-horde effect.
- **Root cause:** It needs a loss plus a retransmission across a stage boundary, so it is rare.
- **Suggested fix:** Add a `u32` sequence to `RunSnapMsg` and drop stale snapshots. This is a wire change, so bundle it with the protocol bump (L46).

#### L36. Host re-seats peers while it is in Results or the menus

- **Severity:** Low · **Area:** co-op, netcode · **Status:** Open
- **Where:**
  - `src/net.rs:446-452`: the seat, unseat and apply chain is gated only on `is_hosting`.
  - `src/net.rs:898-900`: "not in a run yet" is inferred from a missing `CurrentPlanet`, which is never removed after the first run.
  - `src/net.rs:936-949`: the respawn.
- **Symptom:** After the host's first run, `despawn_stage` removes the peer's astronaut on OnExit(InRun), and the next frame re-spawns it (with rig and shadowed spotlight) on the stale planet while the host sits in Results or the menus. That body, with a fresh sheet and the host's hero, carries into the next run.
- **Suggested fix:** Gate the seat chain on `in_state(AppState::InRun)`, and seat peers at `enter_run`. Remove `CurrentPlanet` on OnExit(InRun), which also fixes H7's class of bug.

#### L37. ABANDON RUN on a joiner soft-locks it until the host changes stage

- **Severity:** Low · **Area:** co-op, UI · **Status:** Open
- **Where:**
  - `src/ui/panels.rs:577-581`: ABANDON sets `phase = Dead`.
  - `src/main.rs:285`: `death_watch` is host-only.
  - `src/ui/panels.rs:584`: the pause panel ignores the Dead phase.
  - `src/netenemy.rs:832`: `client_stage_transition` sets `Playing` on the next host stage change.
- **Symptom:** A joiner who picks ABANDON RUN is frozen (virtual time paused, pause overlay stuck, M19) until the host teleports, or indefinitely if the host never does.
- **Suggested fix:** On a client, ABANDON should disconnect and return to the main menu (needs M21's disconnect path), or be hidden.

#### L38. LAUNCH and DAILY are not blocked for a client

- **Severity:** Low · **Area:** co-op, UI · **Status:** Open
- **Where:** `src/ui/menus.rs:285-299` (no role check); `src/main.rs:373` (the client is pulled in only from MainMenu or Boot); `src/ui/menus.rs:593` (a fresh seed).
- **Symptom:** A client with a pending or failed join can click LAUNCH or DAILY. It then enters its own world while `NetRole::Client` disables all simulation, so it plays a dead world. With a live host, it gets the streamed horde over the wrong props. The window is small once connected: a client in the main menu is pulled into InRun as soon as its first snapshot lands (snapshots go out every 250 ms), or at once if it was already seeded, because `RunSync.seeded` is never reset (`src/main.rs:364-375`).
- **Suggested fix:** Disable LAUNCH and DAILY while the role is Client.

#### L39. No teammate HP or downed indicator

- **Severity:** Low · **Area:** co-op, UI · **Status:** Open
- **Where:** `src/net.rs:583-597`: `adopt_my_vitals`, the only reader of `PlayerVitals`, reads only our own id. `max_hp` and `level` are never read, and there is no teammate UI in `src/ui/`.
- **Symptom:** Nobody can see a teammate's health or whether they are down.
- **Suggested fix:** Docket item 8 (§8).

#### L40. A host that quits leaves the joiner frozen

- **Severity:** Low · **Area:** co-op, netcode · **Status:** Open
- **Where:**
  - `src/net.rs:1028-1035`: `ClientState::Disconnected` is only logged.
  - `src/net.rs:1133`: `disconnect()` has no caller.
  - replicon's client reset clears its maps but despawns nothing.
- **Symptom:** When the host closes the game, the joiner keeps its stale teammate rigs and boss proxies, and its `RunState` stops updating. Crowd proxies are reaped after 2 s. The joiner has to restart.
- **Suggested fix:** Handle Disconnected on the client: return to the main menu and reset the net state (the same path as M21).

#### L41. Pots the host breaks stay standing on the joiner

- **Severity:** Low (lowered from Medium: loot still streams, and the joiner's shots do break the host twin) · **Area:** co-op · **Status:** Open
- **Where:** `src/netenemy.rs:327-329`: `stream_enemies` skips pots. `src/main.rs:267`: `apply_hits`, the only pot-break path, is host-only.
- **Symptom:** On the joiner, pots broken by anyone stay standing, with their loot popping out beside them. The joiner's own shots never break its local copies.
- **Suggested fix:** Docket item 6 (`client_pot_break`) covers the joiner's own shots. Full parity needs a pot-break event, which is a wire change.

#### L42. Net components are marked changed every frame

- **Severity:** Low · **Area:** netcode, perf · **Status:** Open
- **Where:** `src/net.rs:1178-1197`: `push_net_transform` and `push_player_vitals` assign through `Mut` unconditionally.
- **Symptom:** replicon sends mutations for every astronaut on every tick, even when nothing changed. That is at most 4 entities.
- **Suggested fix:** Use `set_if_neq`.

#### L43. `PlayerInputMsg` is sent every render frame

- **Severity:** Low · **Area:** netcode, perf · **Status:** Open
- **Where:** `src/net.rs:433-443` (in Update, no timer) and `:749-786` (`send_local_input` writes one message per frame, including neutral ones).
- **Symptom:** The upstream packet rate scales with the client's fps: about 180 messages per second at 180 fps.
- **Suggested fix:** Send at a fixed rate (30-60 Hz), and send edges immediately (see L2).

#### L44. `ClientResidency` is never pruned when a client leaves

- **Severity:** Low · **Area:** netcode · **Status:** Open
- **Where:** `src/netenemy.rs:314`: the only writer, `entry(client).or_default()`. `src/net.rs:954-976`: `unseat_leaving_players` does not touch it.
- **Symptom:** A small memory leak per departed client.
- **Suggested fix:** Remove the client's entry on unseat.

#### L45. u16 pickup ids wrap without a collision check

- **Severity:** Low · **Area:** netcode · **Status:** Open
- **Where:** `src/netenemy.rs:613-614`: `ids.next.wrapping_add(1).max(1)`. `PickupIds` is never reset. `src/netenemy.rs:669-671`: the client skips a Spawn for an id it already has.
- **Symptom:** After 65,535 pickups in a session, a new id can collide with a pickup that is still alive in the same stage. The joiner then misses the new gem, and the next Despawn removes the wrong one. This is unlikely, because the client index is cleared per stage.
- **Suggested fix:** Reset `PickupIds` on stage change, and skip ids that are still live.

#### L46. `PROTOCOL_ID` has not been bumped since Stage 4

- **Severity:** Low · **Area:** netcode · **Status:** Open
- **Where:** `src/net.rs:34-36`: `0xA570B0_2`, last changed in commit `1e7d52f`.
- **Symptom:** Seven messages were added since the last bump: six server messages (RunSnap in Stage 5, HazardEvent and BossSnap in Stage 6, PickupEvent, XpGrant and LootGrant in Stage 7) and one client message (PlayerBuild in Stage 8). The documented rule "Bumped whenever the wire format changes" was not followed. replicon's `ProtocolCheck` hashes the registered message types but not their field layouts, so two builds that differ only in a message's fields would connect and misparse.
- **Suggested fix:** Bump once with the next wire change (docket item 4), and every time after. Also see SH7, a build-hash handshake.

#### L47. Session-log gaps: second-resolution names, no seat events, a bogus "Disconnected"

- **Severity:** Low · **Area:** dev-hygiene · **Evidence:** LIVE (a "NET client state -> Disconnected" line appears in a Solo session) · **Status:** Open
- **Where:**
  - `src/playlog.rs:56-60`: `session-<unix secs>.log`, opened in append mode.
  - `src/net.rs:932`, `:949`, `:974`: seat, re-seat and leave go to `info!` only.
  - `src/net.rs:1028-1035`: `report_connection` logs `ClientState` on every role.
- **Symptom:** Two instances started in the same second share one file. Peer seat events are missing from the file a tester hands back. Solo and host logs contain a misleading client-state line.
- **Suggested fix:** Use millisecond timestamps or add the pid. Mirror the seat events to `playlog::line`. Log `ClientState` only when the role is Client.

#### L48. The health line lacks the seed, checksums and loss counts

- **Severity:** Low · **Area:** dev-hygiene · **Status:** Open
- **Where:** `src/playlog.rs:115-131`: the 5 s health line. `src/net.rs:843-854`: seed, `layout_sum` and `inter_sum` are printed only under `--netlog`, to the console (Bevy's `info!` writes to stderr).
- **Symptom:**
  - A seed desync (H8, M1) or packet loss cannot be seen from session logs alone, and a double-clicked exe has no console.
  - `enemies=` includes pots, and on a client it also includes proxies (L16).
- **Suggested fix:** Add `seed`, `layout_sum`, `inter_sum`, stream sequence gaps and rx/tx counts to the health line.

### 4.6 UI

#### L49. The weapon-tray cache goes stale

- **Severity:** Low · **Area:** UI · **Status:** Open
- **Where:** `src/ui/hud.rs:388-396`: the `Local` cache is keyed on weapons only. `src/ui/hud.rs:421-437`: item chips are built in the same rebuild.
- **Symptom:**
  - Item chips do not refresh when items change without a weapon change (a chest, the shop or an item card).
  - A new run that starts with the same loadout the last one ended with shows an empty tray until the first weapon change, because the `Local` survives the HUD being despawned.
- **Suggested fix:** Include items in the cache key, and clear the cache on OnEnter(InRun).

#### L50. E or ESC can be handled twice in one frame

- **Severity:** Low · **Area:** UI · **Evidence:** CODE (order-dependent; not observed) · **Status:** Open
- **Where:**
  - `src/ui/panels.rs:486-495`: the shop closes on E or ESC and sets Playing.
  - `src/interact.rs:461`, `:476-479`: `interact_system` reads the raw E press and reopens the shop without marking the vendor used.
  - `src/ui/panels.rs:334`, `:525-527`: the chest's ESC and the pause panel's ESC.
  - These systems sit in unordered groups (`src/main.rs:235` vs `:302-305`).
- **Symptom:** Closing the shop with E can reopen it in the same frame. ESC out of a chest or the shop can also open the pause menu.
- **Suggested fix:** Order the panel systems before `interact_system` and `pause_panel`, and consume the key (for example `ButtonInput::clear_just_pressed`) once handled.

#### L51. Clicks pass through the settings and join overlays (can spend silver)

- **Severity:** Low · **Area:** UI, economy · **Status:** Open
- **Where:**
  - `src/ui/settings.rs:137-143`: the settings overlay root has no `FocusPolicy::Block`; Bevy's default for a `Node` is `Pass`.
  - `src/ui/menus.rs:248`: `main_menu_input` guards menu buttons only on `join_open`.
  - `src/ui/menus.rs:318-342`: the tome +1 and equip loops are unguarded.
- **Symptom:** Clicking empty parts of the SETTINGS overlay can press LAUNCH, HOST CO-OP or a tome's +1 underneath, spending silver. Under the JOIN overlay, only the tome buttons are exposed.
- **Suggested fix:** Give overlay roots `FocusPolicy::Block`, and early-return in `main_menu_input` while any overlay is open.

#### L52. `Selected.daily` leaks into a later HOST CO-OP run

- **Severity:** Low · **Area:** UI, co-op · **Status:** Open
- **Where:** `src/ui/menus.rs:286`: only LAUNCH resets `daily`. `src/ui/menus.rs:258-271`: HOST CO-OP does not. `src/ui/menus.rs:486-492`: CharSelect builds a daily `RunState` when the flag is set.
- **Symptom:** After a DAILY run, HOST CO-OP followed by a hero pick starts a daily-seeded co-op run and skips planet select.
- **Suggested fix:** Reset `selected.daily` in the HOST CO-OP arm.

#### L53. `button_hover` overwrites custom button colours

- **Severity:** Low · **Area:** UI · **Status:** Open
- **Where:**
  - `src/ui/mod.rs:55-64`: on any `Changed<Interaction>`, it writes `BTN_BG` or `BTN_HOVER`.
  - `Button` requires `Interaction`, so a new button matches `Changed` on its first frame.
- **Symptom:** These colours are effectively never shown:
  - the equipped-tome green (`src/ui/menus.rs:371`);
  - `CARD_BG` on hero cards (`src/ui/menus.rs:440`);
  - choice cards (`src/ui/panels.rs:100`);
  - shop cards (`src/ui/panels.rs:419`).
- **Suggested fix:** Store each button's base colour in a component and restore that instead of `BTN_BG`.

#### L54. Static SILVER header; a stale MenuTab opens an empty side panel

- **Severity:** Low · **Area:** UI · **Status:** Open
- **Where:**
  - `src/ui/menus.rs:129`: the SILVER text is spawned once with no marker.
  - `src/ui/menus.rs:344-347`: the `dirty` flag is never set to true.
  - `src/main.rs:110`: `MenuTab` is a global that is never reset.
  - `src/ui/menus.rs:194-205`, `:309`: the side panel is spawned empty, and the next click toggles it off.
- **Symptom:** The silver total does not update after buying tomes. On returning to the main menu with Tomes or Quests selected, the side panel is empty, and the first click on that tab closes it.
- **Suggested fix:** Give the SILVER text a marker and update it. Reset `MenuTab` in `spawn_main_menu`, or render the selected tab immediately.

#### L55. Joiner's edge markers: shrine markers never clear, no teleporter marker

- **Severity:** Low · **Area:** co-op, UI · **Status:** Open
- **Where:**
  - `src/ui/hud.rs:464-481`: markers are drawn for interactables that are not `used` and for rings that are not `done`.
  - `src/interact.rs:371`: `ChargeShrine.done` is set only by the host-only `charge_shrines`.
  - The client has no teleporter entity (M3), and its chest `used` flags are per machine.
- **Symptom:** On the joiner, completed rings keep their markers, there is never a teleporter marker, and chests the host opened keep theirs.
- **Suggested fix:** Replicate ring completion and chest use (planned as out of scope for the playtest; see §8), and add the teleporter via docket item 4.

#### L66. The UI font has no em dash (or other non-ASCII glyphs)

- **Severity:** Low · **Area:** UI · **Evidence:** SCREENSHOT (windowed run, 2026-09-27) · **Status:** Open (owner: P24/P35)
- **Where:** Any UI string with a character outside the bundled font's range, e.g. the `YOU — THE HORDE` stage banner.
- **Symptom:** The em dash draws as a tofu box: "YOU □ THE HORDE". Other typographic characters (curly quotes, ellipsis, accented letters such as RAGÙ) are at risk the same way.
- **Suggested fix:** Audit `src/ui/` and `src/content/` strings for non-ASCII characters; either ship a font with the glyphs (generated in code, per the zero-asset rule) or substitute ASCII (" - ", "...") at the text-building boundary.

### 4.7 Dev tooling and debt

#### L56. The dev CLI is compiled into release and re-scans args every frame

- **Severity:** Low · **Area:** dev-hygiene, perf · **Status:** Open
- **Where:**
  - `src/main.rs:52-69`: `--headless`, `--fast-boss`, `--hero`, `--planet`, `--seed`.
  - `src/main.rs:159-171`: `dev_stage_now` and `dev_autopick` run conditions call `std::env::args()` inside the closure.
  - `src/main.rs:385`, `:398-399`: `--stagenow`, `--join`, `--autodrop` in `boot`.
  - `src/main.rs:451-520`: the dev systems (`--autopick`, `--stagenow`, `--bossnow`).
  - `src/net.rs:1141-1172` (`apply_cli_net`): `--host`, `--join`, `--port`, `--botinput`, `--netlog`.
  - `src/headless.rs`: `--coop2`, `--enemydist`.
  - There is no `#[cfg]` anywhere in `src/`.
- **Symptom:**
  - Shipped builds accept flags that skip stages, force tier 3, give free upgrades or summon bosses.
  - Bevy evaluates every run condition every frame without short-circuiting, so the two `args()` scans run each frame in every AppState.
- **Suggested fix:** SH5: parse the args once into a `DevFlags` resource at startup, and compile the dev flags only with a `dev` Cargo feature.

#### L57. The headless smoke drifts from the real app; its summary query is unfiltered

- **Severity:** Low · **Area:** dev-hygiene · **Evidence:** SMOKE · **Status:** Fixed (0486601, P02)
- **Where:**
  - `src/headless.rs:190-301`: a hand-copied system list with no `NetPlugin`, no `NetRole`, no `phase_time_control`, no `gather_local_input` or `player_input`, and no `aura_follow`, `enemy_flash`, `gem_merge`, `interact_system` or `death_watch`.
  - `src/headless.rs:105-106`: the bot writes `vel_t` directly.
  - `src/headless.rs:317`: the summary reads `world.query::<&PlayerState>().iter(world).next()`, with no `LocalPlayer` filter.
  - `src/headless.rs:342`: the "XP pipeline dead" check uses that value.
  - Stale docs: `src/headless.rs:20-21` says the bot presses E (it does not), and `src/headless.rs:112`'s "Fail-fast sanity checks" comment sits on `enemy_distance_probe` instead of `bot_watchdog` (:164).
- **Symptom:**
  - `--headless 1200 --fast-boss --coop2` fails about 1 run in 3 with "FAIL: XP pipeline dead". The summary reports the peer, which died at level 1, instead of the local astronaut. In one failing run the watchdog printed lvl=4 and the summary level=1.
  - The smoke cannot exercise the wire path, the pause model or `InputIntent`, so H4, H6 and the other wire bugs are invisible to it.
- **Suggested fix:** Add `With<LocalPlayer>` to the :317 query; this is untested (U4). Longer term, build both apps from shared system-set functions.
- **Resolution:** The summary and the watchdog read the LocalPlayer (U4 settled by P02's 12/12 fast-boss co-op runs). P30 also makes `--balance` print the local build and moves the misplaced "fail-fast" doc comment onto `bot_watchdog`. Building both apps from shared system sets remains open.

#### L58. `--fast-boss` spawns all three bosses together; the solo check passes weakly

- **Severity:** Low · **Area:** dev-hygiene · **Evidence:** SMOKE · **Status:** Open
- **Where:**
  - `src/headless.rs:213-216`: the timer is set to 95, below both `MINIBOSS_MARKS` of 420 and 120 (`src/config.rs:41`), without marking the minibosses spawned. `--bossnow` does mark them (`src/main.rs:516`).
  - `src/headless.rs:338-341`: the zero-kills check is skipped for `--fast-boss`.
- **Symptom:**
  - CraterpillarJr and RoverGoneWrong spawn on the first tick, and the stage boss about 5 s later.
  - The solo fast-boss smoke died at level 1 with 0 kills and still printed SMOKE OK. It proves the boss spawns, but not that combat works.
- **Suggested fix:** Mark `minibosses_spawned = [true, true]` in `--fast-boss`, and assert on some kills or boss damage.

#### L59. Dead code and no-op bindings

- **Severity:** Low · **Area:** dev-hygiene · **Status:** Open
- **Where:**
  - `src/net.rs:1133`: `disconnect` (never called).
  - `src/player.rs:100`: `Joint.lag` (never read).
  - `sphere::surface_radius` (no callers).
  - `src/player.rs:27-32`, `:612-742`: `Player` duplicates the `RigAnim` fields and copies them back and forth. The animator docs promise "lean-into-acceleration" and "head drag", which do not exist.
  - `src/meshkit.rs:87`, `:95`: `let b …; let _ = b;`.
  - `src/main.rs:428`: `let _ = &role;`.
  - `src/pickups.rs:151`: `let _ = &attractors;`.
  - `src/pickups.rs:222-223`: an owner lookup that is then discarded.
  - `src/pickups.rs:247-253`: an unreachable Xp arm in `collect()`.
  - `src/netenemy.rs:1307`: `let _ = e;`.
  - `src/interact.rs:387`: `let _ = &save;`.
  - `src/player.rs:519`: `let _ = planet;`.
  - `src/combat.rs:502`: `let _ = is_local;`.
  - `src/enemies.rs:887`: `let _ = up;`.
  - `src/enemies.rs:1117`: an unused `let up`.
  - `src/ui/menus.rs:131-132`: an empty `buttons` array.
  - `src/ui/menus.rs:33-44`: the marker components `LaunchBtn`, `TomesBtn`, `QuestsBtn`, `QuitBtn`, `SettingsBtn` and `DailyBtn` are inserted but never queried.
  - `src/headless.rs:109`: `let _ = &planet;`.
- **Suggested fix:** Delete them, or use them. See L65 for the compiler's list.

#### L60. Unused `ResMut`/`Res` parameters, some forcing exclusive access

- **Severity:** Low · **Area:** dev-hygiene, perf · **Status:** Open
- **Where:**
  - `src/combat.rs:975`, `:978`: `apply_player_hits` takes `ResMut<RunState>` and `ResMut<RunPhase>` and never uses either. The `ResMut`s still serialise it against other systems.
  - Unused `run: Res<RunState>`: `anubot_beam_system` (`src/enemies.rs:801`), `craterpillar_update` (:945), `enemy_contact` (:1239), `enemy_projectiles` (:1526).
  - `src/combat.rs:216`: `weapon_fire`'s `_planet`.
  - `src/pickups.rs:302`: `kill_drops`'s `q_ps`.
  - `src/interact.rs:344`: `charge_shrines`'s `save`.
  - `src/run.rs:164`: `RunState::new`'s `_save`.
  - `src/player.rs:122`: `spawn_player`'s `run`.
- **Suggested fix:** Remove them. This frees scheduler parallelism and parameter budget (`interact_system` is at the 16-param cap).

#### L61. `MetaSave` is cloned just to call `recompute_stats`

- **Severity:** Low · **Area:** perf · **Status:** Open
- **Where:** `src/director.rs:139-140`, `src/interact.rs:484`, `src/ui/panels.rs:350`, `:470`.
- **Symptom:** The whole save, with its HashMaps and HashSets, is cloned on every level-up, greed shrine, chest take and shop buy. The impact is negligible, since these are rare events.
- **Suggested fix:** Pass `&save` directly; it compiles, because the sheet comes from a separate `Query`.

#### L62. `update_hud` rewrites 8 texts per frame; `update_music` counts pots

- **Severity:** Low · **Area:** perf · **Status:** Open
- **Where:** `src/ui/hud.rs:336-372`: eight unconditional text writes (`t.0 = …`, six of them `format!`, :340-371) through the `ParamSet` at `:319-328`, which is at Bevy's cap of 8. `src/music.rs:286-287`: counts every `Enemy`, pots included.
- **Symptom:** Text change detection and re-layout run every frame. The music "density" is inflated by static pots.
- **Suggested fix:** Compare before assigning, or use `set_if_neq`. Exclude pots (L16).

#### L63. Stale comments in `src/`

- **Severity:** Low · **Area:** dev-hygiene · **Status:** Open
- **Where:**
  - `src/fx.rs:1`: says particles are "pooled". They are spawned and despawned one entity each (:132-176).
  - `src/config.rs:88` calls the record cap a "Hard ceiling", and `src/netenemy.rs:27-28` says it bounds the worst case. Neither holds (L21).
  - `src/events_world.rs:109-111`: mentions only half of the storm gap (M9).
  - `src/player.rs:73`, `:380-381`: `InputIntent` claims (L5).
  - `src/tutorial.rs:24`: Shift (L29).
  - `src/enemies.rs:142-144`: says ring telegraphs hurt "only near the edge band". They hit from 0.35R to R+1 (:1631).
  - `src/enemies.rs:716-718`: says enrage makes bosses "attack more often". It only pulls the next slam and burst in once (:753-754); the cadence resets at :1577 and :1590.
  - `src/net.rs:4` and `:12-15`: the header predates `PlayerBuildMsg` and streaming.
  - `src/net.rs:34-36`: the bump rule is not followed (L46).
  - `src/net.rs:172`: describes on-change sends plus a 2 s heartbeat. The code sends every 500 ms.
  - `src/net.rs:363-366`: "Registered LAST" sits on `RunSnapMsg`; `EnemySnapMsg` is actually last (:387).
  - `src/net.rs:805-806`: calls remote visuals the "next step", but `remote.rs` exists.
  - `src/net.rs:901-905`: says the stage transition "respawns only player 0". `src/director.rs:218-226` respawns everyone.
  - `src/net.rs:1199-1262`: the NETCODE NOTES block stops at Stage 3. Its "remaining work, in order" is stale: items 1-2c are done, and item 3 is done without the planned far-side aggregation. It lacks Stages 4-10 and the current gaps (H4-H10, M2-M6).
- **Suggested fix:** Correct each comment. Move the running netcode log into `docs/` and leave a pointer.

#### L64. Top-level docs are stale

- **Severity:** Low · **Area:** dev-hygiene · **Status:** Open. The first two bullets describe the files as committed at `99d012f` (read them with `git show 99d012f:PROJECT_STATUS.md`). When this tracker was written, 2026-09-26 rewrites of both files were in the working tree, uncommitted. Once those rewrites are committed, mark the first two bullets fixed with that commit.
- **Where:**
  - `PROJECT_STATUS.md` at `99d012f`: the "Last update" line (:4-6) describes co-op stages 2a-2e. The body was last edited at Stage 5 (commit `99e768d`) and still says "NOT yet human-playtested" (:14).
  - `DEVLOG.md` at `99d012f`: ends at the 2026-07-24 session (Session 2k, commit `82bce4b`). It has no entries for the 24 commits after that: `7a69e42`, then the 23 co-op commits `35db5ff`..`99d012f`. It still carries "UNCOMMITTED" notes (:88, :104, :128) for work that has since been committed.
  - `GDD.md`: last changed in `02300e3`, so its "Current build state" section (`GDD.md:1211`) predates every later commit.
  - `DESIGN.md`: `:157-158` and `:216` describe a Meteor Shower and a Dust Devil that do not exist.
- **Suggested fix:** Commit the refreshed `PROJECT_STATUS.md` and `DEVLOG.md`, and keep refreshing them at every handoff, which is the project rule. Point readers to this file and to `plan/`.

#### L65. 35 compiler warnings, no unit tests, no CI

- **Severity:** Low · **Area:** dev-hygiene · **Evidence:** SMOKE (the build output) · **Status:** Open
- **Where:** The whole crate.
  - `cargo build` gives 0 errors and 35 warnings: 14 unused variables or imports (12 variables, 2 imports), 3 unneeded `mut`, 18 dead-code items (for example `src/combat.rs:4`, `src/enemies.rs:9`, `src/net.rs:1133`, `src/player.rs:100`, `src/content/planets.rs:23`, `src/playlog.rs:27`).
  - There are 0 `#[test]` functions and no CI configuration.
- **Symptom:** Real warnings hide in the noise. The only regression check is the headless smoke, which has its own bugs (L57, L58) and cannot exercise the wire path.
- **Suggested fix:**
  - Triage the warnings to zero.
  - Add unit tests for the pure logic: `roll_item` placement determinism (M1), save round-trips (M13), the spawn-rate formula, and `quantize`/`decode`.
  - Add a CI job that runs `cargo build`, `cargo test` and the four smokes.

---

## 5. Uncertain or not reproduced

These are open questions. They are not confirmed defects. Do not file them as bugs until the check in the last column has been done.

| ID | What is uncertain | Related | How to settle it |
|---|---|---|---|
| U1 | The runtime magnitude of the code-derived co-op bugs: how far the joiner drifts (H5); whether H7 really panics (it should, under Bevy 0.18's default handler); how often H8 happens in the real flow; the size of M1's ring displacement; which targets lose their markers in M22. Nothing in co-op was run live in the 2026-09-26 session. | H5-H8, M1, M22 | Run two instances with `--netlog` and follow each item's Repro. |
| U2 | Whether M12's inverted winding is visible on screen. The winding itself is confirmed by computing the cross products. | M12 | Look at the rig in a windowed build, with and without `cull_mode: None`. **Settled (P30):** visible; see M12. |
| U3 | In the `stream_pickups` race, whether the ghost gem or the panic happens in practice. It depends on command-buffer apply order. | L34 | Force a pickup to spawn inside the collect radius and watch for both outcomes. |
| U4 | Whether the headless summary query in the `--fast-boss --coop2` flake always returns the peer or varies by run. Whether adding `With<LocalPlayer>` fixes the flake is untested. | L57 | Apply the filter, then run the smoke 20 times. **Settled (P02).** |
| U5 | Whether air-hopping to 2.1x without sliding is intended tuning. | L1, D3 | The owner decides. |
| U6 | How often an interactable lands inside a boulder with scale above 5.8. About 6.7% of boulders are that large; how often an interactable is placed inside one is unknown. | L7 | Sample `spawn_interactables` against `PropColliders` over many seeds. **Moot (P30):** placement now avoids props (L7). |
| U7 | Perf magnitudes: the terrain rebuild hitch (L8), the `SpatialHash::near` cost (M18), and host streaming cost at the cap. Bandwidth figures (about 3.9-4.6 KB/s) come from old commit messages and were not re-measured. | L8, M18, L21 | Profile a release build at the 1200 cap, solo and hosting. **Partly measured (P30):** probe counts in M18; the terrain rebuild is now cached (L8). |
| U8 | Cross-OS determinism. The terrain, props and interactable layout depend on `f32` `sin`/`cos`/`acos` and on `rand` sequences. Whether a Windows host and a Linux or macOS joiner build identical worlds is unverified. | H8, M1 | Compare `--netlog` `layout_sum` across OSes. |
| U9 | `local_ip()` routes a UDP socket toward 8.8.8.8 to find the LAN address (`src/net.rs:1051-1060`). Behind a VPN it can report the wrong interface. | M20 | Test on a VPN'd machine. |
| U10 | The source of the difficulty rise at about timer 372 in the 2026-09-26 run. Spawns plus kills ran about 1.15x the D = 0 prediction right after the level 18-19 pick, which fits a Cursed Moon Rock (+0.15 D, `src/content/items.rs:215-221`) or a Greed Shrine (+0.12). The log records no difficulty, items or shrine events. | M16, L48 | Add D and the item list to the health line. |
| U11 | `shrines=0` in the user's save is not necessarily a bug. Only completed charge rings count (`src/interact.rs:372`), and the session log never records shrine events. | L48 | Complete a ring, then check the counter after Results. |
| U12 | Three- and four-player sessions (`MAX_PLAYERS = 4`, `src/net.rs:38`) have never been run. The party scales 2.4 and 3.0, caps of 2880 and 3600, 3-4 spotlight shadow maps and 3-4 client streams at the cap are all unexercised. | L9, L21 | Run a four-instance test. |

Also unknown: whether the planned 2026-07-27 two-PC coworker playtest happened. There are no July logs or saves on the development machine, and nothing was committed afterwards.

---

## 6. Design questions (not bugs)

The owner must decide these. They are listed so that nobody "fixes" them unilaterally.

| ID | Question | Code | Canon or context |
|---|---|---|---|
| D1 | A stage boss killed after The Static begins never opens the teleporter, because `run_clock` returns before the check. Is that intended, and does a late kill pay anything? | `src/director.rs:55-58` returns before `:99-103`. The comment at `:112` says "if it was never beaten the teleporter never opens". `boss_dead` is still set (`src/combat.rs:960`). | Almost certainly intended. `GDD.md:117` and `GDD.md:694` say that if you miss a boss, The Static swallows that world. |
| D2 | The spawn curve: keep the code's `(1 + 2.1·t)` or move toward the GDD's `(1 + 0.14·t)`? Add a cull, or merge overflow into The Static? Exclude pots from the cap? | M16, L16 | `GDD.md:263`, `GDD.md:705` **Decided:** P01 (GDD §3 curve) + P30 (overflow valve into The Static; pots out of the cap). See M16. |
| D3 | Is air-hopping to 2.1x without sliding intended? | L1 | `GDD.md:321-322` **Decided (P06):** a hop steers but never adds speed. See L1. |
| D4 | The co-op chest rule: opener-only (the docket's model) or the GDD's "contested-but-generous" (a card for everyone, the opener rolls at +1 Luck)? | Docket item 5 | `GDD.md:902` |
| D5 | Should joiners bank any meta progress from co-op, and which numbers? | M7 | Not decided. |
| D6 | The co-op pause model: which panels may pause the shared world, and what happens to an astronaut whose player is in a panel? | M4, H6 | Not decided. |
| D7 | The level-up card count. The GDD says three; the code deals four. | `src/run.rs:566`, `:573` (`opts.len() >= 4`, `while opts.len() < 4`) | `GDD.md:283` |
| D8 | Lady Fortuna: one free reroll per level (the GDD) or unlimited (the code)? | L30 | `GDD.md:411` **Decided (P30):** the GDD: one free reroll per level. See L30. |
| D9 | When should the ship-hygiene items go (the B and T keys, Mars and recruit unlocks, the dev CLI)? Re-gating unlocks needs an explicit removal migration. | §9 | Not decided. |
| D10 | Revive design, and whether a stage change revives the downed. | M15 | The GDD's M5 milestone (revives), the Tumbling Beacon. |

---

## 7. Known content gaps (not bugs)

These are features the GDD or DESIGN describe that are not built. They are tracked in `plan/`, not here, but a newcomer should know they are missing and not broken.

- **Bosses.** The Dark Moon's stage boss is Anubot (the `_ => BossKind::Anubot` arm at `src/director.rs:89-91`), and Mars uses Anubot as well. The canon Dark Moon boss is THE HOLLOW COSMONAUT (`GDD.md:657`, `:761`, `:1241`).
- **World events.** Only one exists: the Mars dust storm (`src/events_world.rs`). DESIGN's Meteor Shower and Dust Devil are unbuilt, and `PlanetDef.meteor_showers` is unread (L6).
- **Recruit unlocks.** The unlock conditions for the six recruits (for example "Land 500 crits in one run", "Open 20 Legendary chests") are shown in `unlock_desc` (`src/content/characters.rs:152-212`) but not implemented. The recruits simply start unlocked (SH4).
- **Stats.** Shield, EliteDamage, Knockback and SilverGain have no content that grants them (L32).
- **Networking.** It is direct-IP UDP on port 5011 only: no Steam relay, no lobby UI beyond HOST/JOIN, no drop-in, no revive, and no fixed timestep.

---

## 8. Co-op docket: remaining work

### 8.1 Background

The last working session (2026-07-26) ended with an approved ten-item plan, the "Director's docket". Its goal was a two-PC playtest in which a second person, on another machine, joins, plays a full stage and hands back logs.

- **Default plan:** items 1-6 first, with items 7-8 allowed to slip. After the test, "let the log decide", leaning toward solo content and balance.
- **Deliberately out of scope for the playtest:**
  - charge-ring progress replication (the joiner getting its own blessing);
  - Steam relay;
  - headless determinism;
  - any InteractUsedMsg or InteractReqMsg protocol;
  - shop "sold" sync.
- **Unknown:** whether the playtest, planned for 2026-07-27, ever happened.

**Item 1 is done** (commit `99d012f`). It contained three changes:

- the shrine blessing query gained `With<LocalPlayer>` (`src/interact.rs:341`);
- the client sends a neutral intent while a panel is open (`src/net.rs:763-772`);
- the cage's `place_dir` draw became unconditional, so the ring layout no longer depends on `chimp_freed` (`src/interact.rs:269`).

Four caveats remain:

1. The planned check, "two saves with opposite `chimp_freed` land the five rings in the same places", was never run. Both instances shared one save file. The `inter_sum` checksum cannot prove it either, because it includes the Cage and the host-only Teleporter (`src/net.rs:813`, `:839-842`).
2. The commit message's claim that the neutral intent "stops the interact bit latching" is false: OR-ing `false` clears nothing (H4).
3. `inter` and `inter_sum` go to the console only (`info!`, stderr), under `--netlog`, so they are not in session logs.
4. `target/release/astrobonk.exe` predates the commit (SH6).

**Items 2-10 are not started.** None of `LocalInteractTarget`, `interact_world`, `spawn_teleporter_at`, `teleporter_dir`, `client_pot_break` or a teammate HUD exists in `src/`.

### 8.2 Pre-fixes the audit adds before items 2-10

These are outside the original docket. The audit found that H4, H6 and H8 would wreck a two-PC test run as planned.

| Step | Do | Why |
|---|---|---|
| 0 | Fix the solo regressions **H1, H2, H3**, plus **M19** (the ABANDON overlay) and the **L57** headless filter (`src/headless.rs:317`, add `With<LocalPlayer>`). | Small and confirmed, and they hit every run the user plays today. |
| 0a | **H4:** in `apply_remote_input`, clear jump, slide and interact on every remote astronaut each host frame before OR-ing that frame's messages (`src/net.rs:981-1017`). | Fixes the live bunny-hop and slide drift. It also gives item 3 the consume-once semantics it needs. |
| 0b | **H8:** gate `push_run_snapshot` on `in_state(AppState::InRun)` (`src/net.rs:394-399`), and/or rebuild on a seed or chain change (`src/net.rs:528-534`). **H7:** make `stream_enemies` take `Option<Res<CurrentPlanet>>`, or gate the host lanes on InRun. **M20:** make the LAN address visible after HOST CO-OP. | Otherwise the joiner can build the wrong world, the host can crash, and the host cannot read its own address. |
| 0c | **H6:** fix the invisible horde after a host pause. Two unevaluated candidates: key the client's reaper to snapshot arrival rather than virtual seconds, or have the host clear residency and re-send descriptors after a gap. | Any level-up over 2 s on the host otherwise blinds the joiner for the rest of the stage. |
| 0d | **M1:** isolate the shady-guy stock rolls from the placement RNG (a suggestion, not evaluated). | Item 5 would otherwise make the layout divergence routine. |

### 8.3 Items 2-10

#### Item 2: split `interact_system` into target, local and world systems

- **Goal:** Get under Bevy's 16-parameter cap, and separate three jobs:
  - *which interactable am I facing* (the prompt), which runs on every machine;
  - *local kinds* (Chest, ShadyGuy, Moai, Microwave, Cage), which act on the opener's own sheet on the opener's machine;
  - *world kinds* (Greed and Magnet shrines, Teleporter), which the host resolves.
- **Status:** Not started.
- **Plan claims checked:**
  - The system does have exactly 16 params, tuple-collapsed (`src/interact.rs:394-411`).
  - It is host-only today (`src/main.rs:235`).
  - It reads E from the keyboard (`src/interact.rs:461`).
- **Corrections:**
  - "Chest, ShadyGuy, Moai, Microwave and Cage each touch only the opener's sheet" is only roughly true:
    - the Chest reads `RunState.chest_opens` (`src/interact.rs:440`);
    - the Microwave reads and writes `RunState.microwave_used` (`:451`, `:516`, `:519`);
    - the Cage writes `MetaSave.counters.chimp_freed` (`:538`).
  - All of these are per-machine, so the design still works.
- **Touch sites:**
  - `src/interact.rs:392-551`: the split.
  - `src/interact.rs:55-57`: add a `LocalInteractTarget` resource next to `InteractPrompt`.
  - `src/main.rs:100-102`: init the new resource.
  - `src/main.rs:229-248`: registration. Target and local run on all machines with InRun and playing; world keeps `.run_if(net::is_simulating)`.
  - Optional: `src/headless.rs:268-284`. Headless has no `NetRole`, so do not add `is_simulating` there.
- **Risks and notes:**
  - **Stale world-kind state on the client.** The client's `used` bits for world kinds never flip, because the host resolves them and nothing syncs them. The joiner keeps a stale prompt and marker for a consumed shrine. `interact_world` should search **world kinds only**, so the host never resolves a different entity than the joiner's prompt showed.
  - **One E press can do two things.** On the client, pressing E at a chest opens it locally and also sends `interact = true`. Keep the kind partitions disjoint.
  - **A client Cage unlock is lost unless saved.** A client Cage sets `chimp_freed` in memory only, because clients never bank (M7). Call `save.save()` after it, or the unlock is lost.
- **Done when:**
  - A joiner can open a chest and use the shop, Moai and microwave for its own sheet.
  - Solo and the host behave exactly as before.
  - All four smokes pass.

#### Item 3: the host resolves world interactions for every astronaut

- **Goal:** `interact_world` on the host iterates every astronaut whose `InputIntent.interact` is set and resolves Greed, Magnet and Teleporter for that actor.
- **Status:** Not started.
- **Plan claims checked:** `apply_remote_input` ORs `interact` and nothing clears it (`src/net.rs:1013`). The host's own astronaut can use the same path, because `gather_local_input` sets the bit every frame (`src/player.rs:413`).
- **Corrections:**
  - **The latch is not limited to interact.** Jump and slide latch too, and that is a live bug today (H4). Do pre-fix 0a first.
  - **Never recompute a peer's sheet on the host for Greed.** That would fold the host's meta tomes into it, which `src/net.rs:643-646` explicitly forbids, and it would set `run.difficulty` from the peer's stats (`src/interact.rs:486`). Recompute only the host's LocalPlayer; peers pick up greed through item 4.
  - **Resolution is gated by the host's phase** (`src/main.rs:247`). While the host is in a panel, a joiner's E press is dropped once 0a clears bits each frame. That is simpler than resolving stale presses late.
- **Touch sites:**
  - `src/net.rs:981-1017`: the reset.
  - The Greed (`src/interact.rs:481-489`), Magnet (`:490-499`) and Teleporter (`:545-549`) arms move into `interact_world`, with `Query<(Entity, &Transform, &mut InputIntent), With<Player>>`.
  - `PendingStage` (`src/director.rs:17-19`) is unchanged.
  - Keep the astronaut query `With<Player>` and the interactable query `Without<Player>`, so they stay disjoint.
- **Done when:** A joiner can use Greed and Magnet shrines and take the teleporter (after item 4 makes it visible), and the host's own use is unchanged.

#### Item 4: replicate the teleporter and `greed_stacks`

- **Goal:** The joiner can see and take the teleporter, and its sheet includes greed.
- **Status:** Not started.
- **Plan claims checked, all confirmed:**
  - `spawn_teleporter`'s only call site is in the host-only `run_clock` (`src/director.rs:101`, `src/main.rs:242`).
  - The position uses `thread_rng` plus the party centroid (`src/interact.rs:306-308`, `src/director.rs:66-72`).
  - The joiner has no teleporter entity at all.
  - `RunSnapMsg` lacks `teleporter_dir` and `greed_stacks` (`src/net.rs:100-119`).
  - Every client `recompute_stats` passes `greed_stacks`, which is always 0 on a joiner.
  - The client spawn must be ordered after `client_stage_transition`.
- **Corrections:**
  - **Replicating `greed_stacks` is not enough.** The client recomputes its sheet only on its own picks (`src/ui/panels.rs:233`, `:351`, `:471`; `src/director.rs:140`). Also recompute the LocalPlayer when `greed_stacks` changes.
  - **Spawn the client teleporter level-triggered**, as "`teleporter_open` and no Teleporter exists", not on a `Local<bool>` rising edge. This self-heals after a rebuild or rejoin.
  - **`RunSnapMsg` is unordered** (L35), so a late old snapshot could flip `teleporter_open` or the stage backwards. Consider adding the sequence number in the same bump.
  - **The `PROTOCOL_ID` bump is overdue anyway** (L46). Make it here, once, together with any other wire change you can bundle: H9's result field, L35's sequence, M9's storm fields, 7b's AimLine.
- **Touch sites:**
  - `src/interact.rs:298-335`: split into a direction picker and `spawn_teleporter_at(dir)`.
  - `src/director.rs:99-103`: store `run.teleporter_dir`.
  - `src/director.rs:203`: reset it on stage change.
  - `src/run.rs:102-133` and `:170-190`: the field and its init.
  - `src/net.rs:36`: the bump.
  - `src/net.rs:100-119`, `:496-517`, `:521-555`: the message fields, push and apply.
  - `src/netenemy.rs:200-214`: a new `client_spawn_teleporter` after `client_stage_transition`.

#### Item 5: chests, shop, Moai and microwave for the joiner (plus `client_stage_transition` fixes)

- **Goal:** After item 2, the local kinds work on the joiner against its own sheet and gold. The results reach the host through `PlayerBuildMsg`'s derived stats.
- **Status:** Not started.
- **Plan claims checked:**
  - The panels are ungated and query `With<LocalPlayer>` (`src/main.rs:301-304`; `src/ui/panels.rs:50`, `:146`, `:264`, `:375`).
  - They spend local gold and call `recompute_stats` (`src/ui/panels.rs:344`, `:351`, `:464`, `:471`; `src/run.rs:626`).
  - The host adopts the stats (`src/net.rs:601-657`).
  - `client_stage_transition` force-sets Playing with no panel guard (`src/netenemy.rs:832`), and it passes a fresh level-1 sheet into `spawn_interactables` (`src/netenemy.rs:827`).
- **Corrections:**
  - **Clone `carried` before it moves.** It is moved into `spawn_player` at `src/netenemy.rs:819`.
  - **Reset `microwave_used` on the client.** The client never resets it on a stage change: it is not in `RunSnapMsg`, and `client_stage_transition` takes `Res<RunState>`. The host does this at `src/director.rs:204`.
  - **Client panels freeze the joiner's view.** They pause the client's `Time<Virtual>`, which freezes `drive_proxies`, `drive_boss_proxies`, `animate_net_pickups` and `drive_remote_transforms`, while the host world keeps hitting the joiner's idle body. This already happens on level-up; chests and the shop will make it frequent (M4, D6).
  - **"Pass the carried sheet" does not fix M1.** It makes M1 routine. The audit's own note that "the layout is safe: `roll_item` makes a fixed two draws" is **wrong**: `choose` consumes a length-dependent number of words (M1). Do pre-fix 0d first.
  - **L22 becomes Medium once the joiner can spend gold.** Decide who owns the joiner's gold: its own sheet (the client) or the host copy.
  - **H2 is adjacent.** Fix the chest counters first, and decide whether the chest price escalates per opener or per run.
  - **The chest rule is a design question** (D4): opener-only, or "contested-but-generous" per `GDD.md:902`.
- **Touch sites:**
  - `src/netenemy.rs:775-788`: add `ResMut<ChestPanel>`, `ResMut<ShopPanel>` and `ResMut<ChoicePanel>`, and make `RunState` a `ResMut`.
  - `src/netenemy.rs:792`, `:809-830`: the carried-sheet clone.
  - `src/netenemy.rs:832`: the phase reset.
  - `src/main.rs:522-536`: `clear_panels`, the model to copy.
  - `src/director.rs:204`: the host's `microwave_used` reset.

#### Item 6: `client_pot_break`

- **Goal:** Pots the joiner shoots break on the joiner's screen, as a visual and SFX only.
- **Status:** Not started.
- **Plan claims checked:** The client already produces the `HitMsg`, because `rebuild_hash`, `weapon_fire` and `projectile_move` are ungated (`src/main.rs:188`, `:220-221`) and pots carry `Enemy`. The client drops it because `apply_hits`, the only `HitMsg` reader (`src/combat.rs:879`), is host-only (`src/main.rs:267`).
- **Corrections:**
  - "Loot stays the host's: its twin pot breaks under the same shot" is **not guaranteed**. The host simulates the joiner's weapons from the host copy, which is elsewhere (H5), with `thread_rng`.
  - Pots broken by the host player never break on the joiner's screen (L41), because pots never cross the wire (`src/netenemy.rs:327`). Full parity needs a pot-break event, which is a wire change.
  - A `KillMsg` written on the client is inert (its only reader, `kill_drops`, is host-only), so omitting it is right.
- **Touch sites:**
  - `src/combat.rs:876-921`: the pot branch of `apply_hits`, the model to follow.
  - `src/pickups.rs:321-323`: the visual reference (`fx::burst` Gold, 8, 5.0), plus `Sfx::Pot`, which the host writes in `apply_hits` (`src/combat.rs:918`).
  - `src/main.rs:264-288`: register it with `.run_if(net::is_client)`.

#### Item 7: the two invisible attack tells (H10)

- **Goal:** The joiner sees Anubot's Verdict Beam and the Beamer's aim line.
- **Status:** **Partial.** The data for 7a is already on the wire.
- **Corrections:**
  - The plan said the beam "needs low-rate angle and phase state added". That is wrong: `BossRec.beam_angle` and `beam_state` (`src/net.rs:150-152`) are already sent at 20 Hz (`src/netenemy.rs:881-882`). Only the client consumer and visual are missing, so **7a needs no wire change**.
  - The aim line does need the latched target, sent as a `PlayerId` (`Beamer.target`, `src/enemies.rs:1413`).
- **7a (client only):**
  - In `receive_bosses` (`src/netenemy.rs:891-999`), keep `beam_angle` and `beam_state`, and give Anubot proxies an `AnubotBeamVis`.
  - Drive it with a client copy of the host's visual pass (in `anubot_beam_system`, `src/enemies.rs:798`, around :890-910).
  - `BossSnapMsg` is unreliable at 20 Hz (`src/net.rs:385`), so extrapolate the angle between packets using the known spin rates, or the sweep will step.
- **7b (wire change):**
  - Add `HazardEvent::AimLine { beamer NetId, target PlayerId, dur }` (`src/net.rs:238-258`).
  - Emit it in `stream_hazards` (`src/netenemy.rs:480-516`), and resolve it in `receive_hazards` (`:519-582`) against `NetEnemyIndex` and `MyPlayerId` or the `RemoteAstronaut`.
  - The client stretcher mirrors `src/enemies.rs:1426-1440`, and must track the target's **drawn** position.
  - The beamer proxy may be interest-culled when the target is the host.
  - Ship 7b only if it shares item 4's single `PROTOCOL_ID` bump; otherwise defer it until after the test.

#### Item 8: teammate HUD (L39)

- **Goal:** Show each teammate's HP, level and downed state.
- **Status:** Not started.
- **Plan claims checked:**
  - `PlayerVitals { hp, max_hp, level, down }` is replicated for every astronaut (`src/net.rs:350`; spawned at `src/player.rs:166-167`; filled at `src/net.rs:1188-1197`).
  - `update_hud`'s `ParamSet` is at the cap of 8 (`src/ui/hud.rs:319-328`). `update_hud` itself has only 6 of its 16 params, so a second `ParamSet` would also work, but a separate system is cleaner.
- **Corrections:**
  - "`PlayerVitals` is read by nobody" is wrong: `adopt_my_vitals` reads it for the client's own id (`src/net.rs:583-597`).
  - "Filter by `pid != MyPlayerId`, not `Without<LocalPlayer>`" is incomplete. The client's own predicted body is `PlayerId(0)` with **zeroed** `PlayerVitals` (`src/main.rs:435`, `src/player.rs:163-166`). So the filter must be `Without<Player>` (or `Without<LocalPlayer>`) **and** `pid != MyPlayerId`, as `adopt_my_vitals` does. On the host `MyPlayerId` is `None`, so `Without<LocalPlayer>` alone is enough.
- **Touch sites:**
  - `src/ui/hud.rs:60-306`: `spawn_hud`, to add the rows.
  - A new `update_teammate_hud` in `src/ui/hud.rs`, registered in `src/main.rs:290-308`.
  - Optional: teammate arrows in `update_edge_markers` (`src/ui/hud.rs:446-529`), from peer Transforms on the host and `RemoteAstronaut` Transforms on the client. Remote rigs exist only on clients (`src/remote.rs:47-53`).

#### Item 9: two-machine dry run (process)

- **Goal:** A real two-PC session, with logs from both machines.
- **Status:** Not started, and possibly never done (see §5).
- **Corrections and prerequisites:**
  1. **Rebuild the release exe.** `target/release/astrobonk.exe` (69,395,968 B, about 66 MiB, a single file with zero external assets) was built 4 minutes before HEAD and lacks at least part of `99d012f` (SH6). Run `cargo build --release` only when no instance of the game is running; on Windows the build cannot replace a running exe.
  2. **Both PCs need the byte-identical exe.** `PROTOCOL_ID` was not bumped for Stages 5-8, so mismatched builds would connect and misparse (L46).
  3. **The host cannot read its LAN address off the menu** (M20). Workarounds: press BACK from CharSelect, or run `ipconfig`. On a VPN'd machine, `local_ip()` may be wrong (U9).
  4. **The joiner must connect after the host is in the run** (H8), unless pre-fix 0b is done. A fresh host process must not click HOST CO-OP before playing a run (H7), unless 0b is done.
  5. **The joiner will be Buzz** (M5). A fresh save on the second PC also means its tutorial is active and it sees the Moon cage.
  6. **Launch both from a console with `--netlog`.** The checksums (`seed`, `layout_sum`, `inter_sum`) go to the console only (`info!`, stderr).
  7. **The host needs a firewall rule** for UDP port 5011 (an admin shell).
- **Expect:** H4, H6, M2-M4 and M23 to show up unless their fixes landed first.

#### Item 10: triage the logs (process)

- **Goal:** Collect `%APPDATA%/astrobonk/logs/session-*.log` from both machines, plus any `--netlog` console captures, and fix what broke.
- **Status:** Not started.
- **Corrections:**
  - **The health line lacks what you need** (L48): the seed, `layout_sum`, `inter_sum`, sequence gaps and rx/tx counts. A seed desync or packet loss is invisible from session logs alone.
  - **`enemies=` is misleading.** It includes pots, and on a client it also includes crowd and boss proxies (L16). Interest management means proxies legitimately cover only part of the horde. **Look for flatlines, not ratios.**
  - **A cage on one machine only** makes `inter` and `inter_sum` differ legitimately.

### 8.4 Recommended order

1. **Pre-fix 0 (solo):** H1, H2, H3, M19, and the L57 filter.
2. **Pre-fix 0a:** H4, the input latch.
3. **Pre-fix 0b:** H8 (snapshot gate and/or seed-change rebuild), H7 (Option or InRun gate), M20 (visible LAN address).
4. **Pre-fix 0c:** H6, the invisible horde after a host pause.
5. **Pre-fix 0d:** M1, isolating the stock RNG from placement.
6. **Item 2**, then **item 3** (world kinds only, no peer recompute on Greed), then **item 5** (clear the panels, clone and pass the carried sheet, reset `microwave_used`).
7. **Item 4**, with the **single `PROTOCOL_ID` bump**. Bundle in any other wire changes that are ready: H9's result field, L35's sequence, 7b.
8. **Item 6**, then **7a**, then **item 8**, then **7b** only if it can share that bump.
9. **Rebuild release** from the final HEAD. Optionally add `seed`, `layout_sum` and `inter_sum` to the playlog health line (L48). Re-run all four smokes.
10. **Item 9** (both PCs on the identical exe, launched from a console with `--netlog`, the joiner connecting after the host is in the run), then **item 10**.

---

## 9. Ship hygiene before 1.0

These are the things that must not ship in a public build. Each one is confirmed in code at `99d012f`. D9 (when to remove them) is the owner's call; until then they stay, because playtesting relies on several of them.

| ID | Item | Where | What to do |
|---|---|---|---|
| SH1 | **DEV key B summons the stage boss.** It is ungated in release and on clients, is a progression shortcut, and shares the Banish key. See M14. | `src/enemies.rs:913-936` (doc: "Remove before ship"); `src/main.rs:244` | Delete it, or compile it only with a `dev` Cargo feature. If kept for dev builds, add `.run_if(net::is_simulating)`. **Done (b5c192f, P30):** B needs `--dev`. |
| SH2 | **DEV key T replays the tutorial** in any run. | `src/tutorial.rs:42-45` (comment: "DEV: press T to replay"); registered at `src/main.rs:299` | Remove it, or move it to a "Replay tutorial" button in Settings. |
| SH3 | **Mars is unlocked by default** ("dev: Mars selectable for playtesting"). `migrate` folds every default unlock into every loaded save, so every save regains Mars on load. As a result, ClearMoonT2's `UnlockPlanet(Mars)` reward and the locked Mars card ("Clear MOON Tier 2 to chart this world") are dead, and Yuki (unlocked by ClearMarsT1) is reachable early. | `src/save.rs:84`; `migrate` at `src/save.rs:130-145`; `src/content/quests.rs:79`; `src/ui/menus.rs:566-568` | Removing line 84 stops new saves from getting Mars, but **existing saves keep it** (it is persisted in `unlocked_planets`). Re-gating needs an explicit removal migration, for example a save `version` bump that removes Mars unless ClearMoonT2 is in `quests_done`. Decide whether to grandfather playtesters. |
| SH4 | **All six recruits start unlocked**, and their unlock conditions are not implemented. `starts_unlocked` excludes only B0nk, Yuki, ChimpO and Doug. Conditions such as "Land 500 crits in one run", "Circle a planet 3x in under 40s" and "Open 20 Legendary chests" have no counters or quests (`src/save.rs:15-29`, `src/content/quests.rs:7-24`). | `src/content/characters.rs:237-244`; `unlock_desc` at `src/content/characters.rs:152-212` | Implement the counters and quests, change `starts_unlocked`, and add a removal migration as for SH3. `migrate` only ever adds. |
| SH5 | **The dev CLI and headless harness are compiled into release** (L56): `--headless`, `--fast-boss`, `--hero`, `--planet`, `--seed`, `--autodrop`, `--autopick` (free upgrades), `--stagenow` (skips stages, forces tier 3), `--bossnow`, `--botinput`, `--netlog`, `--coop2`, `--enemydist`, and the multiplayer `--host`, `--join` and `--port`. Two run conditions re-scan `std::env::args()` every frame. | `src/main.rs:52-69`, `:159-171`, `:385`, `:398-399`, `:451-520`; `src/net.rs:1141-1172`; `src/headless.rs` | Parse args once into a resource. Put the dev flags and the headless app behind a `dev` feature. Keep only the player-facing `--host`, `--join` and `--port` if wanted. |
| SH6 | **The release exe in `target/` is stale.** `target/release/astrobonk.exe` (01:19) predates HEAD `99d012f` (01:23). It lacks the `inter_sum=` string that commit added, so it very likely lacks all three of the commit's fixes. The 2026-09-26 playtest ran on it. | Build artifact, not code | Before any playtest or release, run `cargo build --release` from a clean tree with the game closed, then run the four smokes against **that** binary. Never hand out an exe you did not just build from a known commit. Record the commit hash in the build (see SH7). |
| SH7 | **There is no build identity check between peers.** `PROTOCOL_ID` has not been bumped since Stage 4, and replicon's `ProtocolCheck` does not detect field-layout changes (L46). | `src/net.rs:34-36` | Bump `PROTOCOL_ID` on every wire change. Better, derive it from a hash of the wire types or the git commit at build time (a `build.rs`), and show the version on the main menu and in the session log. |
| SH8 | **The save can be wiped by a schema change**, and writes are not atomic (M13). | `src/save.rs:14`, `:115-122`, `:147-160` | Add `#[serde(default)]` on `Counters`, back up on a parse failure, write to a temp file then rename, and add a `version` field. Do this before adding any counter, and before SH3 or SH4. **Done (0b58234, P30):** see M13. |
| SH9 | **A console window opens alongside the game on Windows.** There is no `#![windows_subsystem = "windows"]` in `src/main.rs`, so the release exe is a console-subsystem binary. | `src/main.rs:1` | Add `#![cfg_attr(not(feature = "dev"), windows_subsystem = "windows")]`. Note that `--netlog` output and all `info!` logging go to the console (Bevy writes them to stderr), so keep the console in dev builds, or mirror what matters to the session log (L48). |
| SH10 | **The save and log paths fall back to `.` when `APPDATA` is unset**, which is every non-Windows OS. A Linux or macOS build would write `./astrobonk/save.json` and `./astrobonk/logs/` relative to the working directory. | `src/save.rs:107-111`; `src/playlog.rs:33-37` | Use the platform data directory (for example with the `directories` crate). |
| SH11 | **Session logs are never rotated.** Every launch creates `session-<unix>.log`, and nothing deletes old ones. | `src/playlog.rs:56-60`; there is no `remove_file` or `read_dir` in `src/` | Keep the last N logs, or cap their total size. |
| SH12 | **Netcode is unauthenticated and unencrypted.** `ServerAuthentication::Unsecure` and `ClientAuthentication::Unsecure`; `client_id` is the join time in milliseconds. Anyone who can reach UDP port 5011 can join, up to three joiners (`max_clients: MAX_PLAYERS - 1`). This is fine for LAN playtests. | `src/net.rs:1081`, `:1114-1115`, `:1079`, `:37-38` | For internet play use secure connect tokens or a relay (Steam), and add a host-side accept or kick list. |
| SH13 | **Player-facing text errors:** "Shift to slide" (L29); "It'll drop a chest" (L28); the "[DEV] … SUMMONED" banner (`src/enemies.rs:935`); "already in a co-op session — restart to change role" (M21). | `src/tutorial.rs:24`, `:26`; `src/enemies.rs:935`; `src/ui/menus.rs:252-257`, `:749-752` | Fix them with their items. Do a full text pass before 1.0. **P30:** "Shift to slide" is true since P06 (L29); the chest line is true since P01's cache (L28); the [DEV] banner needs `--dev` (M14). The full text pass is still open. |
| SH14 | **35 compiler warnings, no unit tests, no CI** (L65). | Crate-wide | Get to zero warnings, add tests for the save, placement determinism and wire codecs, and add CI running the build, tests and smokes. |
| SH15 | **No LICENSE file** in the public GitHub repository (`git ls-files` shows none). | Repo root | The owner should choose a license, or state "all rights reserved", before inviting contributions. |
| SH16 | **Leftover dev markers in code.** | `src/enemies.rs:913-935`; `src/tutorial.rs:42`; `src/save.rs:84`; `src/main.rs:498`, `:517` (DEV log lines) | Before 1.0, a search for `DEV`, `dev:` and `Remove before ship` in `src/` should return nothing that ships. |

---

## 10. Build and test baseline at HEAD `99d012f`

This is the state as measured on 2026-09-26. It is context for anyone verifying a fix.

### Toolchain and dependencies

Pinned in `Cargo.lock`:

- Bevy 0.18.1
- bevy_replicon 0.40.4
- bevy_replicon_renet 0.16.0
- rand 0.8.7, which is the version the game code uses; 0.9.5 is also in the tree

The profiles are in `Cargo.toml`:

- dev: `opt-level = 1`, with dependencies at 3;
- release: `lto = "thin"`, `codegen-units = 1`.

### Build

`cargo build` was a no-op: 0 errors, 35 warnings (L65). The debug exe matches HEAD. The release exe is stale (SH6).

### Smokes (debug exe)

| Command | Result |
|---|---|
| `astrobonk --headless 2400` | PASS: level 7, 155 kills. |
| `astrobonk --headless --coop2` | PASS. This form runs 1500 ticks, because the tick count must directly follow `--headless` (`src/main.rs:54`). Write `--headless 2400 --coop2` to get 2400. |
| `astrobonk --headless 1200 --fast-boss` | PASS, weakly: the bot died at level 1 with 0 kills, and the zero-kills check is skipped for this mode (L58). |
| `astrobonk --headless 1200 --fast-boss --coop2` | 1 of 3 runs passed. Both failures were "FAIL: XP pipeline dead", caused by the unfiltered summary query (L57). Commit `17115e9` documents the same flake on earlier commits. |

What the smokes can and cannot catch:

- **Checks.** A smoke fails when:
  - the enemy count exceeds `ENEMY_CAP + 400`;
  - the run state goes non-finite;
  - the run ends stuck in a panel;
  - there are 0 kills (not in `--fast-boss`);
  - level < 2 with more than 50 kills;
  - there are 0 enemies with the boss not dead;
  - `--fast-boss` never spawns the boss.

  See `src/headless.rs:164-172` and `:333-358`.
- **Blind spots.** They cannot catch wire-path, pause-model or `InputIntent` bugs (L57).
- **Tests.** There are no `#[test]` functions.

### Runtime

- The live solo run on the stale release exe held 165-190 fps at the 1200-enemy cap.
- Host streaming cost at the cap has not been measured (U7).

---

## 11. Corrections already applied (do not re-report)

The catch-up analysis re-checked these claims, from earlier docs, readers or the docket, and found them wrong or overstated. They are recorded here so they are not filed again.

### Movement

- "A slide is slower than hopping" is wrong. Both reach the 2.1x cap (L1).
- With Space latched, the host copy jumps continuously and never slides (H4).

### The Static

- "A boss killed during The Static never opens the teleporter" is a design question, most likely intended, not a High bug (D1).

### Layout RNG

- "Interactable positions are unaffected by the sheet" and the docket audit's "`roll_item` makes a fixed two draws, so the layout is safe" are both wrong (M1).

### Reading the 2026-09-26 log

- The kill bursts did not stop at timer 344. There was another +63 at timer 257.
- The cap count does not include client proxies: `director_spawn` runs only on the host, which has none.
- "About 56-57 pots" is not a constant. Pots are broken during the run, which weakens any pot-residual estimate (U10).

### Unlocks

- RocketPod and CryoVent are not unobtainable. They are the starting weapons of Old Ironclad and Slipstream Nova. Only their entry into the level-up pool is blocked (H2 for RocketPod).
- Mars and the recruits stay unlocked on existing saves because the save persists them, not because `migrate` re-adds them. `migrate` only re-adds while the defaults still include them (SH3, SH4).
- `chimp_freed` persists through any `save()` call (Settings, tome purchases), not only through `bank_results`.

### Severity changes

- The pots gap was lowered to Low (L41): the joiner's host copy does break the host twin, and the loot streams.
- Joiner `gold_gain` was lowered to Low for now (L22).
- M1 was lowered from High to Medium until joiners can interact.
- L1 was lowered from Medium to Low.

### Session flow

- A joiner who ABANDONs is released by the host's next stage change (`src/netenemy.rs:832`) (L37).
- A peer's host-side rig uses the peer's real hero from stage 2 onward (M5).
- How the joiner's crowd proxies go at run end (H9). One reader said a host sweep despawns them; its verifier said the host sends nothing, because its astronauts are gone, so the 2 s reaper does it. Neither is the whole picture: the host re-seats the joiner's body in Results (L36) with its virtual clock running, and its next snapshot is then a despawn sweep. The 2 s reaper is the fallback. This was re-derived from code at `99d012f`, not observed.
- "The client gets no boss warning" is overstated. It has the HP bar, edge markers, the Static timer and the Static music stem; only the banners and the roar are missing (M23).

### Commit and docket claims

- `99d012f`'s claim that the neutral intent "stops the interact latch" is false (H4).
- Docket item 7: the beam data is already on the wire (H10, §8).
- Docket item 8: `PlayerVitals` is already read, by `adopt_my_vitals` for the client's own id (§8).
- The docket audit's claim, in item 5, that the layout is safe is wrong (M1).

### Code facts corrected

- Weapon fire, projectiles, drones and beams do run on clients, as cosmetics whose `HitMsg`s are dropped. They are not host-only.
- Far-tier crowd updates carry no flash bit; only the near band does.
- `BossRoar` also comes from `boss_phase_system`, `boss_attacks` and `kill_drops`, not only `run_clock`. All of them are host-only.

