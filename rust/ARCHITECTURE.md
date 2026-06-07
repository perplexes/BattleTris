# Architecture

A bird's-eye map of the BattleTris Rust/WASM workspace: the crate graph, the
three ways data flows through the system, and the handful of functions to read
first. For the *why* behind the netcode and the game logic, this doc defers to
the deep dives it links.

New here? Read [`docs/overview.md`](docs/overview.md) first for what BattleTris
is and the port's thesis, then come back.

## The crate graph

Nine Rust crates plus the `www/` TypeScript front-end. The shape is a layered
diamond: one dependency-free engine at the root, a few pure layers on top of it,
and the two deployables (browser + server) at the apex.

```
                         ┌────────────────────────────────────────┐
                         │  bt-core   the deterministic engine      │
                         │  (no platform / UI / net deps —          │
                         │   custom i32 codec, no serde in core)    │
                         └───────────────┬──────────────────────────┘
                  ┌──────────────┬───────┼────────────┬─────────────┐
                  ▼              ▼        ▼            ▼             ▼
              bt-ai         bt-replay  bt-netcode  bt-trueskill  bt-identity
           (Ernie / the   (seed       (Predictor: (TrueSkill 2  (HS256 JWT
            BTComputer     replays +    shared      μ,σ rating)   player names)
            port)          Input type)  predict /
                  │              │       reconcile)
                  │              │        │
        ┌─────────┴──────┐       │   ┌────┴───────────┐
        ▼                ▼       ▼   ▼                ▼
   bt-wasm          bt-bot              bt-server
   (WasmGame /     (headless region    (axum: matchmaking, authoritative
    WasmVsComputer  bots; runs the      Bouts on /ws, leaderboard, replay
    / WasmClient;   shared Predictor)   store, admin endpoints)
    + www/ TS)
```

Edges are "depends on". `bt-netcode`'s `Predictor` is the deliberate sharing
point: it is run by **both** the browser (through `bt-wasm`'s `WasmClient`) and
the headless `bt-bot`, so the prediction/reconciliation logic exists exactly
once and is property-tested once.

Crate-by-crate (paraphrasing each crate's `//!` header — read those for the full
story):

| Crate | Role |
|-------|------|
| [`bt-core`](docs/engine.md) | The faithful, deterministic rules engine ported from `usr/src/game/`. Modules mirror the C++ classes (`board` ⇐ `BTBoardManager`, `piece` ⇐ `BTPiece`, …). Dependency-free — a custom i32 codec, no serde. Consumed by everything. |
| `bt-ai` | The computer opponent "Ernie": `eval_board` (the `BTCBoard::eval` heuristic) + `best_placement` (orientation × column search) + `Computer`, which drives a `bt_core::Game` turn by turn. A faithful `BTComputer`/`BTCBoard` port (plus a stronger eval used only by bots). |
| [`bt-replay`](docs/replays.md) | Deterministic record/playback and the wire-level `Input` type. A recording is just `{seed, mode, ai_level, dt_ms, engine_sha, frames}`; re-running the engine reproduces the game bit-for-bit (same engine build). |
| [`bt-netcode`](docs/architecture-netcode.md) | The shared `Predictor`: client-side prediction (`predict`) + reconciliation against authoritative keyframes (`on_snapshot`, restore-then-replay the unacked tail). Run by both browser and bot. |
| [`bt-trueskill`](docs/glossary.md#trueskill-2) | TrueSkill / TrueSkill 2 ratings for 1v1 (a Gaussian `μ ± σ`), implemented from the Herbrich-Minka-Graepel and Minka-Cleven-Zaykov papers. |
| `bt-identity` | A tiny self-contained HS256 JWT so the server can trust a *signed* player name (`BT_JWT_SECRET`), rather than pull in `jsonwebtoken`. |
| [`bt-wasm`](docs/frontend.md) | wasm-bindgen bindings — `WasmGame`, `WasmVsComputer`, `WasmClient`, plus the replay players — and the `www/` TypeScript front-end (Canvas renderer, protocol, no bundler). |
| [`bt-server`](docs/deployment.md) | The axum server: matchmaking by TrueSkill quality, the authoritative online `Bout`s on `/ws`, leaderboard, the SQLite replay store, metrics, and admin endpoints. Serves the static client on the same port. |
| `bt-bot` | Headless, networked players ([Bert](docs/glossary.md#bert) / [Ernie](docs/glossary.md#ernie) / [The Count](docs/glossary.md#the-count)) that speak the same `/ws` protocol a browser does, deployed per-fly-region over 6PN. |

`cargo test` runs the host crates (`bt-core`, `bt-trueskill`, `bt-ai`,
`bt-replay`, `bt-identity`, `bt-netcode`); `bt-wasm` is wasm32-only and is built
with `wasm-pack`, while `bt-server`/`bt-bot` join in under `--workspace`. The
workspace also ships [`tla/`](../tla/README.md) (TLA+/Apalache models), `usr/src/`
(the original 1994 C++), and the fly.io / CI config.

## The three data-flow paths

There are three distinct ways a game runs. They share `bt-core`; they differ in
*who owns the simulation*.

### 1 — vs-Computer (100% client-side, no server)

The fastest thing to stand up: there is no network. `WasmVsComputer` owns both
the player's `Game` and Ernie's, ticked locally.

```
  keyboard ──▶ WasmVsComputer ──▶ bt_core::Game (player)
                      │
                      └──────────▶ bt_ai::VsComputer ──▶ bt_core::Game (Ernie)
                      │
                      └──────────▶ render.ts ──▶ Canvas
```

### 2 — Online (server-authoritative, the real netcode)

The server runs the only simulation. Each client predicts locally and reconciles.

```
  browser A                          bt-server                         browser B
  ─────────                          ─────────                         ─────────
  WasmClient.predict(input)
     │ apply locally (instant)
     │ send {seq, input} ──▶  /ws  ──▶ Bout::is_legal_client_input ──▶ Versus.tick
                                            │ (authoritative engine)
            ◀── snapshot {tick, ack, ◀──────┤ broadcast each side's
                you, opp, keyframe?}        │ frame ~30 Hz          ──▶ (same to B)
  WasmClient.on_snapshot(...)
     │ drop acked inputs
     │ on keyframe: restore + replay unacked tail   ← the snap-back fix
```

The server resolves every cross-player effect (Mirror, Swap, taxes, spies), so a
client can only ever send *legal inputs* — that is the anti-cheat property, and
the totally-ordered input log is what makes online games recordable.

### 3 — Bot (the same Predictor, headless, over 6PN)

A region bot is just path 2 with no browser: it keeps a local `bt_core::Game`,
runs the **same** `bt_netcode::Predictor`, and layers a pure decision FSM
(`bt-bot`'s `sync::decide`) on top to decide *when* to predict. The placement
itself comes from the same `bt-ai` search the vs-Computer Ernie uses.

```
  bt_ai placement ──▶ sync::decide (pure FSM) ──▶ Predictor.predict ──▶ /ws ──▶ Bout
                                                        ▲                          │
                                                        └──── on_snapshot ◀── snapshot
```

## Where to start reading

Four entry points, in dependency order:

1. **`bt-core/src/game.rs` → `Game::tick(dt_ms)`** — the heart of the engine: the
   explicit virtual clock (spawn → fall → slide → lock → clear → spawn → top-out)
   that replaced the X11 timeout loop. (Two boards at once:
   `bt-core/src/versus.rs` → `Versus::tick`.) See [`docs/engine.md`](docs/engine.md).
2. **`bt-server/src/bout.rs` → `is_legal_client_input`** — the anti-cheat gate
   and the server side of authoritative play; the file's `//!` header explains
   why authoritative-over-P2P. See [`docs/architecture-netcode.md`](docs/architecture-netcode.md).
3. **`bt-netcode/src/lib.rs` → `Predictor::predict` / `Predictor::on_snapshot`** —
   the shared prediction/reconciliation core; `on_snapshot`'s restore-then-replay
   is the [snap-back](docs/glossary.md#snap-back) fix.
4. **`bt-wasm/www/main.ts`** — the browser entry: wires keyboard → wasm → Canvas,
   and drives `WasmClient` for online play. See [`docs/frontend.md`](docs/frontend.md).

## System invariants

Three properties hold across the whole system; each has a deep dive.

- **Determinism.** Same seed + same inputs ⇒ same game, bit-for-bit, on the same
  engine build. The engine is seeded (a faithful `drand48`/`lrand48` LCG) and
  ticked by a fixed timestep. → [`docs/engine.md`](docs/engine.md).
- **The seed-replay contract.** A game *is* `{seed, mode, ai_level, dt_ms,
  engine_sha, frames}`; everything else regenerates. → [`docs/replays.md`](docs/replays.md).
- **The bazaar barrier.** While either side shops, the match freezes for both;
  getting this right under latency (the deadlock + the snap-back) is the crux of
  the netcode, and it is checked by property tests *and* TLA+/Apalache. →
  [`docs/architecture-netcode.md`](docs/architecture-netcode.md),
  [`tla/README.md`](../tla/README.md), and the project dossier
  (`screenshots/netcode-writeup.html`, `screenshots/tla-explainer.html`).
