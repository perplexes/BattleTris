# Contributing to BattleTris

This is a faithful Rust + WebAssembly port of the 1994 Brown CS networked
Tetris-battler. The game *logic* is ported verbatim from the original C++; the
*platform* (X11/Motif → Canvas, TCP daemons → an authoritative websocket server)
is reimplemented. That split, plus a few hard-won correctness rules, shapes how
we work. Read the **House rules** below before sending a change.

## Toolchain

| Tool | For |
|------|-----|
| Rust (stable) | the whole workspace |
| `wasm-pack` + `wasm32-unknown-unknown` target | the browser client (`bt-wasm`) |
| Node + `tsc` | the `www/*.ts` browser front-end |
| Python 3 | the static dev/e2e server |
| Playwright | the browser e2e tests |
| Apalache + Java 17 (optional) | the TLA+ model checks ([`tla/README.md`](../tla/README.md)) |

There's no toolchain pin; CI uses `dtolnay/rust-toolchain@stable`. See
[docs/building-and-running.md](docs/building-and-running.md) for install
commands.

## The build / test loop

From `rust/`:

```sh
cargo test --workspace                          # the host test suite
cargo clippy --workspace --all-targets -- -D warnings   # warnings are errors
```

`bt-wasm` is wasm-only and excluded from the default workspace members, so the
above runs without the wasm toolchain. For the browser client + its e2e:

```sh
cd bt-wasm
npm install
npm run build            # wasm-pack (--dev) + tsc
npm run test:e2e         # Playwright (vs-Computer is client-side — no server)
```

The TLA+ conformance checks live under `tla/` and are run with `bash ci-check.sh`
(and `bash regen-traces.sh --check`) — see [`tla/README.md`](../tla/README.md).

## House rules

These are the project's distinctive conventions. They exist because each one
caught a real bug or hid a real failure. Follow them.

### 1. `bt-core` stays dependency-free

The engine crate (`bt-core`) has **no third-party dependencies** — no `serde`,
no `rand`. It uses a hand-written i32 codec for serialization and a POSIX-LCG
RNG port. Keep it that way: the determinism and replay guarantees rest on the
engine being a closed, auditable system. New dependencies belong in the crates
*above* core (`bt-server`, `bt-wasm`, `bt-bot`), never in core.

### 2. No silent fallbacks — fail loud

Zero fallbacks of anything: fonts, code paths, config. If something doesn't
work it must **fail instantly and visibly**, never degrade silently to a
"good enough" path that hides the breakage.

- **Code:** avoid `unwrap_or(default)`, swallowed `catch {}`, or "best-effort"
  paths that paper over a real error. Surface it (panic / throw / log loudly).
- **Config/env:** don't quietly substitute a default for a *required* value —
  error out. (The token secret, for example, is logged loudly when missing.)
- **Fonts/UI:** use the one intended family with no fallback list, self-hosted,
  so a load failure visibly breaks rather than rendering an OK-looking substitute.

The reasoning: a fallback masks whether the *real* thing works. We want the
single intended thing, or an obvious failure.

### 3. Each Rust module names the C++ class it ports

Every ported module's `//!` header states which original C++ class it mirrors,
e.g. `board.rs` is "the faithful analogue of `BTBoardManager`
(`usr/src/game/BTBoardManager.{H,C}`)" and `game.rs` ports `BTGame`. Constants
are transcribed verbatim from `BTConstants.H`. When you add or change ported
logic, keep that pointer accurate and check the original C++ under `usr/src/`
(see [../PORTING.md](../PORTING.md) for the class map). For faithful-port
*visuals* (cell glyphs etc.), follow the original `usr/src/game/*.C` rather than
improvising.

### 4. Model sync/protocol logic as an explicit FSM + property tests

When the logic is a synchronization or protocol-state problem (client-vs-server
prediction, reconnect/pause phases, the bazaar barrier handshake), model it as
an **explicit state machine with property-based tests**, not a tangle of ad-hoc
booleans scattered through a loop. Two real bugs — a bot leaving a bazaar it had
only *predicted* entering, and a cross-bout ack-baseline deadlock — were classic
"implicit FSM via booleans" failures.

How to apply:

- Extract the per-tick decision into a **pure, total function** over an
  observable state struct (the model: `bt-bot/src/sync.rs`'s
  `decide(SyncState) -> BotAction`), and keep side effects in the driver.
- Pin invariants with proptest: safety properties (never act ahead of the
  server's ack; never leave a merely-predicted bazaar) **and** a model-based
  liveness/no-freeze property that simulates the server's responses.
- A liveness/model test is only worth keeping if it has **teeth**: run a
  deliberately-buggy variant through the same harness and assert it fails, so
  the model can't pass vacuously.

(Compile-time typestate was considered and deliberately rejected here: the tokio
loop mutates `&mut MatchState` each tick, so typestate's ownership-move
transitions would just reintroduce a runtime enum match at the loop boundary.)

### 5. Visual sign-off before any UI change

For **any** visual change — rendering, layout, colors, glyphs, CSS — render it,
screenshot it, and get explicit human sign-off **before** committing or
deploying. Screenshots are not a reliable self-review for visual correctness;
only a person looking at the result can confirm it. For faithful-port visuals,
also check the original C++ (`usr/src/game/*.C`, e.g. `BTBox.C` for cell glyphs)
and match it closely.

### 6. Frame work by complexity, not time

In PRs, plans, commit bodies, and handoff notes, describe work by its
*complexity and shape* ("small change to one file", "new schema + migration +
handlers + UI", "a foundation + outbox + UI signal — three pieces"), never by a
time estimate ("~half a day", "30 min"). Strip any `~Nh / ~Nd` phrasing in
review.

### 7. Prefer `ast-grep` for code search

For searching **code** structures (function definitions, call sites, specific
shapes), prefer `ast-grep -p '<pattern>' -l rust <path>` (with metavars like
`$A`, `$$$`) over text `grep` — structural search gives precise, low-noise
matches. Plain text grep is fine for non-code (config values, logs, prose).
Note: ast-grep's walker doesn't recognize the original `.C` extension, so feed
those via stdin: `ast-grep -p '<pat>' --lang cpp --stdin < usr/src/game/BTGame.C`.

## CI & PR expectations

CI ([`.github/workflows/deploy.yml`](../.github/workflows/deploy.yml)) runs on every
push and PR to `main`. Three jobs **gate** a deploy and must be green:

- **test** — `cargo test --workspace --locked`
- **clippy** — `cargo clippy --workspace --all-targets -- -D warnings`
- **tla** — the fast Apalache model checks + a trace-freshness check

The slow, full Netcode TLA+ check (`tla-full`) runs nightly / on manual dispatch
only and never gates a PR. A green push to `main` deploys to fly.io.

Before opening a PR: run `cargo test --workspace` and the clippy line above
locally, and run the e2e suite if you touched the client. Get visual sign-off
for any UI change (rule 5).

## Commit messages

The repo's commit messages are unusually descriptive — they carry the *why*, not
just the *what* (browse `git log` for the house style; e.g. "netupdate, plyupdate
leave NetManager in limbo when calling fatalErr"). Match that: lead with the
component, state the change, and explain the reasoning when it isn't obvious.

## Where to read next

- [docs/building-and-running.md](docs/building-and-running.md) — all build/run modes + env vars.
- [ARCHITECTURE.md](ARCHITECTURE.md) — the crate map and data-flow paths.
- [tla/README.md](../tla/README.md) — the TLA+/Apalache conformance harness.
- The project dossier (`screenshots/*.html`, served over Tailscale) — the
  long-form architecture / netcode / weapons write-ups.
