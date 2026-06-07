# BattleTris — Rust + WASM port

A faithful Rust/WebAssembly port of [BattleTris](../README.md) — the 2-player
networked Tetris-battler written at Brown CS32 in 1994 — from its original
pre-standard C++/X11/Motif source (under [`usr/src/`](../usr/src)) to the
browser, on an **authoritative WebSocket server** with **TrueSkill 2**
matchmaking.

**▶ Play now: <https://battletris.fly.dev>**

The port keeps the original *game logic* faithful where it matters — board
geometry, the 18 piece shapes and rotation, the funds/[die](docs/glossary.md#die-happy-frown)/[happy](docs/glossary.md#die-happy-frown)
economy, the 20-combined-line bazaar trigger, and the 34-weapon roster — while
modernizing the *platform*: X11/Motif → HTML5 Canvas (WASM), the TCP
master/slave daemons → an authoritative WS server, the flat-file ELO DB →
TrueSkill 2. Each Rust module names the C++ class it ports (`board.rs` ⇐
`BTBoardManager`); constants are transcribed verbatim from `BTConstants.H`.

One deliberate departure: the 1994 original relayed gameplay **peer-to-peer**;
this port makes the **server authoritative** — a conscious modernization (real
anti-cheat, a totally-ordered and therefore replayable event log), not a
faithfulness goal. See [`docs/faithfulness.md`](docs/faithfulness.md) and
[`docs/architecture-netcode.md`](docs/architecture-netcode.md).

## Two-minute orientation

- **Play it** — open <https://battletris.fly.dev>, or run vs-Computer locally
  (no server needed): build the wasm with `wasm-pack`, serve `bt-wasm/` as static
  files, open `/www/`. Step-by-step: [`docs/quickstart.md`](docs/quickstart.md).
- **Hack on it** — `cargo test` for the host crates; `wasm-pack build bt-wasm
  --target web --out-dir pkg` + `npm run build:ts` for the client; `cargo run -p
  bt-server` for online play. Full toolchain + house rules:
  [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`docs/building-and-running.md`](docs/building-and-running.md).
- **Understand it** — start with [`ARCHITECTURE.md`](ARCHITECTURE.md) (crate
  graph, three data-flow paths, where to start reading).

## Documentation map

| Document | What it covers |
|----------|----------------|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Crate graph, the three data-flow paths, "where to start reading" entry points, system invariants. |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Toolchain setup, the build/test loop, and the house rules. |
| [`docs/overview.md`](docs/overview.md) | What BattleTris is and the port's thesis (faithful logic / modern platform). |
| [`docs/quickstart.md`](docs/quickstart.md) | Play online or run vs-Computer locally in under a minute. |
| [`docs/building-and-running.md`](docs/building-and-running.md) | Every run mode (Practice / vs-Computer / 2-tab / Online / region bot), the wasm + TS build, and the env-var table. |
| [`docs/architecture-netcode.md`](docs/architecture-netcode.md) | The authoritative model, the `Predictor` (prediction/reconciliation, the snap-back fix), and the bazaar barrier. |
| [`docs/weapons.md`](docs/weapons.md) | The 34-weapon system: economy, arsenal stacking, launch, and how the server resolves cross-player weapons. |
| [`docs/faithfulness.md`](docs/faithfulness.md) | What's ported verbatim, what's reimagined, the codex-audit fixes, and known gaps. |
| [`docs/engine.md`](docs/engine.md) | `bt-core` internals: determinism, the virtual tick clock, the POSIX RNG port, pieces, and line-clear/funds. |
| [`docs/frontend.md`](docs/frontend.md) | The `www/` TypeScript front-end and the wasm boundary (no bundler, plain ES modules). |
| [`docs/testing.md`](docs/testing.md) | The four-layer suite: property tests, TLA+/Apalache conformance, Playwright e2e, and differential/fuzz oracles. |
| [`docs/deployment.md`](docs/deployment.md) | Fly topology, the Dockerfile, region bots, quiesce-in-place deploys, CI, and admin/secrets. |
| [`docs/replays.md`](docs/replays.md) | The seed-replay contract, record → library → spectate, and the storage routes. |
| [`docs/glossary.md`](docs/glossary.md) | The project's vocabulary (bazaar barrier, keyframe, snap-back, op-score, idiot, Ernie…). |
| [`tla/README.md`](../tla/README.md) | The TLA+/Apalache models (Bazaar.tla, Netcode.tla) and the conformance harness. |
| `screenshots/index.html` | **The project dossier** — the long-form, illustrated write-ups (netcode, weapons codex, TLA+ explainer, the Motif redesign), served over Tailscale. |

## Workspace layout

Nine crates plus the `www/` TypeScript front-end. The shape: one dependency-free
engine at the root, pure layers on top, the two deployables (browser + server) at
the apex. Full crate-by-crate table and the dependency diagram are in
[`ARCHITECTURE.md`](ARCHITECTURE.md).

```
rust/
  bt-core/       Deterministic rules engine (no platform/UI/net deps)
  bt-ai/         "Ernie" — the BTComputer opponent port
  bt-replay/     Deterministic record/playback + the Input wire type
  bt-netcode/    The shared client Predictor (browser + bot)
  bt-wasm/       wasm-bindgen bindings + www/ TypeScript front-end
  bt-server/     axum: matchmaking, authoritative Bouts on /ws, replays, admin
  bt-bot/        Headless region bots (Bert / Ernie / The Count)
  bt-identity/   HS256 JWT player identity
  bt-trueskill/  TrueSkill 2 ratings
```

## Status

The port is **live in production** and feature-complete against the original
game: all six cross-player weapons (Swap Meet, Lazy Susan, Mirror Mirror, and the
Ames/Ace/Condor spies) ship online, the netcode is server-authoritative with
client prediction, online matches are recordable, and per-region bots keep the
lobby populated.

The test suite has four layers (see [`docs/testing.md`](docs/testing.md)):

1. **Property tests** (proptest) across the workspace — engine rotation,
   line-clear, codec, keyframe, versus, and weapons in `bt-core`; the predictor
   invariants in `bt-netcode`; plus `bt-ai`, `bt-trueskill`, `bt-identity`, and
   the bout-level liveness properties in `bt-server`.
2. **TLA+/Apalache conformance** — `Bazaar.tla` + `Netcode.tla`, with traces
   replayed against the real `apply_input` ([`tla/README.md`](../tla/README.md)).
3. **Playwright e2e** — browser tests of the wasm client
   (`bt-wasm/tests/*.spec.js`).
4. **Differential / fuzz oracles** — line-clear and weapon oracles, the
   fuzz→replay bridge.

`cargo test --workspace` runs the Rust host + server + bot tests (the engine,
netcode, AI, ratings, identity, replay, and bout suites — several hundred cases
in total); `npm run test:e2e` runs the Playwright layer; the formal layer runs
under `tla/`. CI gates deploys on the test, clippy, and fast-TLA jobs being
green. Do not infer a fixed test count from this README — `cargo test --workspace`
is the source of truth.

## License & credits

MIT. Original BattleTris © 1993–1997 **Bryan Cantrill, Charlie Hoecker, and
Mike Shapiro**, written as a Brown University CS32 final project in spring 1994;
revived several times between 1994 and 2001 and exhumed in 2026 by Adam
Leventhal. The fuller history — including the inspiration in Wesleyan Tetris — is
in Cantrill's [reunion blog post](https://bcantrill.dtrace.org/2026/05/25/a-portentous-reunion/)
and the [root README](../README.md). This Rust/WASM port preserves their game
logic; see [`docs/faithfulness.md`](docs/faithfulness.md) for exactly what is
faithful and what is reimagined.
