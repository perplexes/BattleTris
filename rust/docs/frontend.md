# Frontend: the browser client

The browser client lives in [`rust/bt-wasm/www/`](../bt-wasm/www/). It is plain,
strict **TypeScript** compiled by `tsc` (no bundler) talking to the Rust engine
through a small **wasm-bindgen** boundary. There is no framework — the DOM is built
and updated directly.

This page maps the `www/` files, the wasm boundary, the build, and the `?debug=1`
developer surface. The wire protocol it speaks is documented from the server side in
[`architecture-netcode.md`](architecture-netcode.md); the engine behind the wasm
wrappers is [`engine.md`](engine.md).

## `www/` layout

The page is one window that swaps between a `#lobbyScreen` and a `#gameScreen`
(faithful to the original `BTStartup.C` show/hiding `BTChallenge` and `BTGame`).

| File | Role |
|------|------|
| `index.html` | The single game page: lobby + game screens, the bazaar overlay, the debug overlay element. |
| `main.ts` | The application. Owns the lobby, all four play modes, the persistent websocket (lobby *and* match), client-side prediction wiring, the bazaar, the HUD, and the `?debug=1` tools. By far the largest file. |
| `render.ts` | Shared Canvas board drawing (`drawBoard`, `CELL_SIZE`). The *same* draw path is used by the live game and every replay/spectate viewer, so playback is pixel-identical. Faithful to `BTBox.C`. |
| `protocol.ts` | The websocket wire types — server frames as a **discriminated union** on `type` (see below). |
| `sound.ts` | Synthesized Web Audio blips (the port's answer to `BTSoundManager`; no sampled assets) with a localStorage on/off toggle. |
| `replay.ts` | The `/replay/:id` playback page — drives `WasmReplayPlayer` (solo / vs-Computer) or `WasmVersusReplayPlayer` (online) with play/pause/seek/speed, plus the `?debug=1` step controls. |
| `spectate.ts` | The live-match spectator (a debug view): subscribes to a bout via `{type:"spectate",match_id}` and renders the server's read-only two-board frames. No prediction, no input. |
| `leaderboard.ts` | The `/leaderboard` page — ranks players by Elo-styled TrueSkill from `GET /api/leaderboard`. |
| `library.ts` | The replay library index — lists stored games (`GET /api/replays`) linking to `/replay/:id`. |
| `update-gag.ts` | The UPDATE button gag. The 1994 client's UPDATE *pulled* the roster; ours is *pushed* live, so the button has no work — it tells an escalating joke instead. Extracted so it's unit-testable (`update-gag.test.ts`). |
| `dom-util.ts` | Shared DOM helpers (`escapeHtml`, etc.). |
| `motif-scroll.ts` | The custom `XmScrolledList` scrollbar widget (gutter + sunken trough + embossed triangles) — native browser bars can't reproduce the Motif look. |
| `motif.css` | Loaded *after* `style.css`; overrides bevel **colors** only (computed gray-on-gray from OSF/Motif `Xm/Color.c`), never layout. See [`faithfulness.md`](faithfulness.md). |
| `style.css` | Layout + the faithful palette. |
| `assets/` | Sprites (the B/T shield, the gimp image for cell id 23, etc.). |

The `*.test.ts` files run under Node's type-stripping (`npm run test:unit`), not the
browser build.

## The wasm boundary

The bindings are in [`rust/bt-wasm/src/lib.rs`](../bt-wasm/src/lib.rs). Each exported
`#[wasm_bindgen]` struct wraps an engine type and exposes a JS-facing API. The
read/render methods are deliberately **mirrored** across the wrappers, so the
front-end draws and shows the HUD for any of them with one code path.

| Wrapper | Wraps | Used for |
|---------|-------|----------|
| `WasmGame` | `bt_core::Game` | Practice / 2-player single-board play. Owns a `Recorder` so any game exports a replay. |
| `WasmVsComputer` | `bt_ai::VsComputer` | vs-Computer (Ernie). Owns the player + AI match (bazaar barrier, difficulty, relay) and records the human's inputs. **100% client-side — no server needed.** |
| `WasmClient` | `bt_netcode::Predictor` | Online play. The thin wasm face over the **same** prediction/reconciliation core the bot runs. |
| `WasmReplayPlayer` | `bt_replay::ReplayPlayer` | Solo / vs-Computer replay playback. |
| `WasmVersusReplayPlayer` | `bt_replay::VersusReplayPlayer` | Online (two-board) replay playback. |

A few patterns worth knowing:

- **Fixed timestep.** `fixed_dt()` returns `FIXED_DT_MS = 16`; the front-end runs an
  accumulator loop at that rate so play (and every recording) is deterministic
  regardless of `requestAnimationFrame` jitter. Recordings replay bit-exact only when
  stepped at this same rate.
- **Online inputs go through `predict_*`.** Unlike `WasmGame`'s direct
  `move_left()`/etc., `WasmClient`'s inputs (`predict_move_left`, `predict_launch`,
  `predict_buy`, …) apply the move locally **and return the ready-to-send wire
  frame** (or `""` when the input was gated by the bazaar barrier or rejected). The
  client sends the string verbatim. Server frames come back through
  `on_snapshot(ack, you_bazaar, opp_bazaar, keyframe)`, which reconciles. The
  seq/ack/keyframe-replay invariants are the proptested ones in `bt-netcode`, not
  hand-rolled JS — there's only one reconciliation implementation, shared with the
  bot.
- **`render_grid()`** returns a flat `width*height` array of cell ids (the piece
  overlaid; `EMPTY = -2`) which `render.ts` draws.
- **`drain_events()`** returns flat `[tag, a, b, c]` quads (locked/scored/weapon-
  launched/bazaar/idiot/airslide/game-over/funds-stolen) that drive sound and HUD.
- **Weapon catalog** for the bazaar UI comes from free functions: `weapon_name`,
  `weapon_description`, `weapon_price`, `weapon_duration`, `max_weapons`.

### `protocol.ts` discriminated unions

Server-to-client frames are a TypeScript discriminated union keyed on `type`, mirror
of the JSON `bt-server` emits (`bt-server/src/main.rs` + the `Snapshot` struct in
`bout.rs`). Because the union is exhaustive, the message handler (`onSignalMessage`
in `main.ts`) is compile-checked — a new server message type, or a renamed field, is
a **type error**, not a silent `undefined` at runtime. Input frames are *not*
modelled here: they're built in Rust (`bt-netcode::input_frame`, surfaced via
`WasmClient.predict_*`), so the client only constructs the lobby/control frames in
`ClientMessage`.

## Build: `tsc` only, no bundler

The page is served as plain ES modules — there is no bundler, no transpile step
beyond `tsc`.

- `npm run build:wasm` → `wasm-pack build . --target web --out-dir pkg` (generates
  `pkg/bt_wasm.js` + `bt_wasm.d.ts`).
- `npm run build:ts` → `tsc -p tsconfig.json` compiles each `www/*.ts` into a
  `www/*.js` **beside it** (no `outDir` — setting it to the source dir would make tsc
  exclude the sources). The page's `<script type="module">` and the relative
  `../pkg/bt_wasm.js` import resolve exactly as authored.
- The emitted `*.js` / `*.js.map` are gitignored (`www/.gitignore`); the Dockerfile
  runs `npm ci && npm run build:ts` (after `wasm-pack`, so the generated
  `pkg/bt_wasm.d.ts` exists to type the import). `noEmitOnError` is set, so a type
  error fails the build.
- The wasm boundary is **typed for free** by the generated `pkg/bt_wasm.d.ts`.

The TS config is strict (`strict`, `noUnusedLocals`/`Parameters`,
`exactOptionalPropertyTypes`, `noPropertyAccessFromIndexSignature`, `types: []` so no
Node globals leak into the browser surface).

## The `?debug=1` developer surface

Two diagnostic tools, both off by default and gated behind `?debug=1` in the URL or
the **backtick (`` ` ``) key** toggle:

- **In-game debug overlay** — a green monospace HUD over the playfield
  (`pointer-events:none`) showing mode/seed/match_id, the input seq + unacked count,
  local-vs-server state with opponent-score drift, and active weapons with their
  line countdowns (via `weapon_remaining`).
- **Weapon-grant picker** (vs-Computer only) — a funds drop plus one-click grant of
  any of the 34 weapons straight into the arsenal (`WasmVsComputer::grant_weapon`),
  for testing weapons without playing to the bazaar. *Not* recorded into the replay
  (it's a debug mutation), so granted weapons won't reproduce on playback.

`main.ts` also honors a `?screen=bazaar` preview param (force the bazaar overlay
open). The **replay page** has its own `?debug=1` controls: step, jump-to-tick,
copy-state, and a panel showing the raw input stream at the current tick
(`WasmVersusReplayPlayer::inputs_at_tick`).

For the online server-side grant counterpart (`POST /admin/grant`) and the
matchmaking/lobby endpoints, see [`deployment.md`](deployment.md).

## Related

- [`engine.md`](engine.md) — the `bt-core` engine behind the wasm wrappers.
- [`architecture-netcode.md`](architecture-netcode.md) — the prediction/reconciliation
  the `WasmClient` realizes.
- [`replays.md`](replays.md) — the record/library/spectate pipeline the viewers serve.
- [`faithfulness.md`](faithfulness.md) — the native-geometry and Motif rules the
  render path honors.
