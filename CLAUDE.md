# ASTROBONK — working notes for Claude

Megabonk-style 3D survivors roguelite on tiny spherical planets. Rust + **Bevy 0.18.1**, zero
external assets (procedural meshes in `meshkit.rs`, in-code WAV synth in `audio.rs`/`music.rs`).
Design bible: `GDD.md` (§15 Canon Ledger wins on conflicts). Build plan: `docs/BUILD_PLAN.md`.
Status/history: `PROJECT_STATUS.md`, `DEVLOG.md`, and the NETCODE NOTES at the bottom of `src/net.rs`.

## Build & verify
- `cargo build` — must stay free of **errors**; don't add new warnings in code you touch.
- Headless smoke (no window; prints `SMOKE OK`/`SMOKE FAIL`):
  `cargo run -- --headless 2400`, `--headless 1200 --fast-boss`, `--headless 2400 --coop2`,
  `--headless 1200 --fast-boss --planet mars|darkmoon`, `--headless 1200 --fast-boss --coop2`.
  Useful flags: `--hero <name>`, `--seed N`, `--planet <name>`. Add a flag when a new feature
  needs a smoke path (e.g. `--event <name>`, `--ascension N`).
- The smoke is NOT deterministic even with `--seed`; never use it as a before/after oracle.
- Set `APPDATA` to a temp dir when running tests, or logs/saves land in `./astrobonk/`.
- Two-instance co-op (needs a display): `--host --autodrop --botinput --netlog` and
  `--join 127.0.0.1 --autodrop --botinput --netlog`. Also `--autopick`, `--bossnow`, `--stagenow`.

## Hard rules (each one has already cost a debug cycle)
1. **`headless.rs` builds its own App with a DUPLICATED system/resource/message list.** Every new
   system, resource and message must be registered in BOTH `main.rs` and `headless.rs`
   (headless omits rendering/UI-only systems). A missing message registration panics every
   smoke path.
2. **Bevy limits:** a system may take at most 16 params — group extras into tuples
   `(ResMut<A>, ResMut<B>)` or a `#[derive(SystemParam)]`. `add_systems` tuples max out at 20
   elements — split into another `add_systems` call. Two queries touching the same component
   with `&mut` need disjoint `Without<…>` filters or a `ParamSet` (B0001 panic otherwise).
3. **Bevy 0.18 API:** buffered events are `Message` (`#[derive(Message)]`, `add_message`,
   `MessageWriter::write`, `MessageReader::read`). Cursor options are a `CursorOptions`
   component. `despawn_related::<Children>()`, `BorderColor::all()`, `Volume::Linear`.
   `AudioSink` setters need `&mut AudioSink`.
4. **Co-op model = host-authoritative listen server** (`net::NetRole`: Solo | Host | Client).
   Solo behaves as its own host (`is_simulating` is true). Any system that SIMULATES (spawns,
   damages, rolls loot, advances timers, resolves interactables) must be gated
   `.run_if(net::is_simulating)`. Clients only draw what the host streams.
   - Anything new that a client must SEE needs a wire path: crowd enemies ride
     `netenemy.rs` (6-byte records; new `EnemyKind`s need `kind_code`/`kind_from_code`),
     bosses the boss lane, one-shot attacks the hazard EVENT lane, loot the pickup lane,
     run-global state `RunSnapMsg` (4 Hz). Wire codes are explicit and must never be reordered.
     Bump `net::PROTOCOL_ID` when the wire format changes.
   - Per-player state lives on the astronaut as `run::PlayerState`; run-global state is the
     `run::RunState` resource. Presentation queries use `With<LocalPlayer>`; simulation
     iterates ALL astronauts; enemies target the nearest astronaut by great-circle arc
     (`player::nearest_astronaut`). Never use `.single()` on `Player`/`PlayerState` in
     simulation code — it silently fails with 2+ players.
   - Remote astronauts on a client NEVER get `Player`/`PlayerState` (see `remote.rs` header).
   - Things indexed by planet (`PlanetKind::ALL`, `NET_ENEMY_INTEREST_IN/OUT`, `planet_code`,
     `netenemy::planet_index`) must all be extended together when a planet is added.
   - A client must never write the save (`bank_results` etc. are host/solo only).
5. **Determinism of the world:** world gen draws from `run::GameRng` (seeded per stage). Every
   draw must be UNCONDITIONAL on per-machine state (save progress, settings), or the two
   machines' layouts diverge (the cage bug). Put per-machine differences after the draws.
6. **Sphere math:** positions are unit `dir` + height; distances are great-circle arcs
   (`sphere::arc_dist`), never `Vec3::distance`, for gameplay ranges.
7. **Save compatibility:** `save::MetaSave` uses `#[serde(default)]`; new fields must default
   sensibly and `migrate()` must fold new default unlocks into old saves.
8. Content is data: add heroes/weapons/items/tomes/enemies/planets/quests in `src/content/`
   tables; tuning numbers go in `config.rs`.
9. Camera law (GDD §4/§13): never snap, aim from the unshaken position, total shake offset is
   clamped; the player is never knocked back by chip damage.
10. Dev-only keys/flags must not ship active in a normal build (gate behind `--dev`).

## Module map
`sphere` math · `planet` worldgen/props/lighting · `player` controller+camera+rig ·
`enemies` horde/bosses/hazards · `combat` weapons/damage · `pickups` drops/magnet ·
`run` run+player state, upgrade/loot rolls, item grades · `items` what the §7 items DO (procs,
cursed levers, the one death-save resolver) · `director` clock/stages/results · `interact`
chests/shrines/vendors · `events_world` planetary events · `comet` Comet Combo · `content/`
data tables · `save` meta save · `ui/` hud/panels/menus/settings/numbers · `fx` shake/hitstop/
particles · `audio`/`music` synth · `net` transport/replication · `netenemy` horde/boss/hazard/
pickup streaming · `remote` teammate rigs · `playlog` session log · `headless` smoke bot.

## Git
Work on the branch you are told to. Commit messages: imperative subject, a body explaining
WHY, and end with the attribution lines given in the task prompt. Never commit `target/`,
`.astrobonk-bin`, or `astrobonk/` (test logs).
