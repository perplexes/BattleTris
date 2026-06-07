# Testing: the four-layer suite

BattleTris is tested in four layers, each catching a class of bug the others can't:

1. **Property-based tests (PBT)** — the engine, codec, predictor, AI, and rating
   math against generated inputs, with a hard "liveness tests must have teeth" rule.
2. **TLA+/Apalache conformance** — the netcode *design* model-checked, then its
   generated traces replayed against the real `Bout`.
3. **Playwright end-to-end** — the wasm + TypeScript front-end in a real browser.
4. **Differential / oracle + fuzz→replay** — the engine's *values* pinned to an
   independent reference and to the 1994 C++.

See also [architecture-netcode.md](architecture-netcode.md) (what the netcode
invariants *are*), [engine.md](engine.md) (the engine under test), and
[replays.md](replays.md) (the fuzz→replay bridge). The TLA+ formalism is documented
in full in [`tla/README.md`](../../tla/README.md) — this page links into it rather
than restating the models.

---

## How to run each

```sh
# Layers 1 & 4 (all Rust PBT, differential, oracle, conformance) — gated by CI:
cargo test --workspace --locked

# Lint gate (also CI):
cargo clippy --workspace --all-targets -- -D warnings

# Layer 3 (Playwright e2e) — builds wasm + TS, then runs the browser specs:
cd bt-wasm && npm test            # = build:wasm && build:ts && playwright test

# Layer 2 (TLA+) — needs Java 17+ and apalache-mc on PATH:
cd tla && bash ci-check.sh        # the fast model checks (assert each outcome)
cd tla && bash regen-traces.sh --check   # the trace fixtures aren't stale
```

The full all-fixed `Netcode.tla` check (length 14, ~6.5 min) is too slow for the PR
path; run it explicitly only when changing the model — see
[`tla/README.md`](../../tla/README.md).

---

## Layer 1 — Property-based tests (proptest)

PBT runs across nearly every crate; the headline files (under each crate's `tests/`):

| Crate | Files | What they pin |
|---|---|---|
| [`bt-core`](../bt-core/tests/) | `pbt_rotation`, `pbt_lineclear`, `pbt_codec`, `pbt_keyframe`, `pbt_versus`, `pbt_weapons`, `pbt_robustness`, `pbt` | piece rotation, line-clear/gravity, the `i64` codec, keyframe round-trip + deterministic continuation, the cross-player relay, weapon lifecycle, malformed-input robustness |
| [`bt-netcode`](../bt-netcode/tests/predictor_pbt.rs) | `predictor_pbt` | the **snap-back invariant** and friends (see below) |
| [`bt-ai`](../bt-ai/tests/) | `ai_properties`, `pbt`, `weapons_fuzz`, `weapons_lifecycle`, `weapons_funds`, `characterization`, `bot_match` | Ernie's behavior, weapon fuzzing, scoring |
| [`bt-server`](../bt-server/src/bout.rs) | (inline `proptest!`) | `apply_input` anti-cheat + the bazaar liveness spec (below) |
| [`bt-bot`](../bt-bot/src/sync.rs) | (inline `proptest!`) | the `sync::decide` FSM invariants P1–P3, P5 |
| [`bt-identity`](../bt-identity/tests/pbt.rs), [`bt-trueskill`](../bt-trueskill/tests/pbt.rs) | `pbt` | JWT round-trip, TrueSkill 2 rating |

### The snap-back invariant

The headline `bt-netcode` property
([`predictor_pbt.rs`](../bt-netcode/tests/predictor_pbt.rs)):
`unacked_inputs_survive_a_keyframe` predicts N inputs, then receives one keyframe
acking only the first `k` — and asserts the reconciled local state equals
"all N inputs applied" (the unacked tail is replayed, never dropped). It runs again
under a *stream* of rising-ack keyframes (`incremental_keyframes_keep_converging`),
the real frame-by-frame loop. `predictor_is_deterministic` is the structural basis
for browser/bot consistency: two predictors fed identical calls end identical, and
since both clients drive the *same* `Predictor`, there's no second implementation to
drift. See [architecture-netcode.md](architecture-netcode.md).

### The "teeth" rule

A liveness/"it eventually works" test that can't *fail* is worse than no test. The
project's rule (from the bazaar-deadlock work, commits `6dcdaaf`/`03197bc`/`3f70145`,
and the bot FSM `01f253e`): **every liveness property must be shown to have teeth** —
a deliberately-buggy variant must trip it. Concrete cases:

- **`bt-bot`** `sync.rs`'s liveness property `p5_always_escapes_the_bazaar` is paired
  with `p5_harness_has_teeth_buggy_policy_trips_hazard`, which runs the same harness
  against a deliberately-buggy local-leave policy (`decide_buggy_local_leave`) and
  asserts it *fails*.
- **`bt-server`** `bout.rs`'s `the_client_always_escapes_the_bazaar` is a
  first-principles liveness spec over a generated space of adversarial schedules,
  run against the *real* `Bout::apply_input`. Reverting the ack-on-barrier-reject fix
  makes proptest fail and shrink to the minimal counterexample (`pre=0, crossing=1`)
  — a single in-flight input that crosses the barrier and is never acked. Its
  deterministic sibling `inflight_gameplay_inputs_do_not_deadlock_the_bazaar` turns
  the same freeze into a bounded test (a regression *fails*, never hangs the suite).

### Anti-cheat properties

`bout.rs` proptests pin that [`is_legal_client_input`](../bt-server/src/bout.rs) is
exactly right: every relay-internal input is *rejected* and mutates nothing
(`relay_internal_inputs_always_rejected` snapshots the full latent state — both
games plus every `Bout`-only field — and even ticks ~1500 steps to prove no *delayed*
injection surfaces at a lock); every legal client input is *accepted* outside the
bazaar (`every_legal_client_input_is_accepted_outside_bazaar`, so dropping any single
arm from the allow-list is caught); and seq monotonicity is enforced biconditionally.

---

## Layer 2 — TLA+/Apalache conformance

The netcode *design* — bazaar barrier × network delay × reload-rejoin × weapon relay
× weapon timing — is modeled in TLA+ and checked with Apalache (symbolic/SMT, with
counterexample traces). This explores the *design's* state space, which the
code-level PBT can't exhaustively cover. The models, the toggleable-fix knobs, the
"what the model taught us" narrative, and the run commands are all documented in
[`tla/README.md`](../../tla/README.md) — **not restated here**. The project dossier's
`screenshots/tla-explainer.html` is the illustrated walkthrough.

The model and the code are tied together by a **conformance harness**,
`apply_input_conforms_to_every_tla_trace` (in
[`bt-server/src/bout.rs`](../bt-server/src/bout.rs)), part of `cargo test`. It is
data-driven: it loads every `*.itf.json` in
[`bt-server/tests/traces/`](../bt-server/tests/traces/) — Apalache-generated traces
of the server semantics across the feature space (`bazaar_crossing`,
`weapon_then_cross`, `reconnect_snapback`, `two_bazaar_visits`) — drives a real
`Bout` along each, and asserts `(ack, in_bazaar, weapons-applied)` tracks the model's
`(serverAck, serverBazaar, weaponsApplied)` after every state. Each step is mapped by
the model's explicit `lastAction` string, so a step is never silently mis-mapped;
any unmapped action is a hard `panic!`, and the corpus is *required* to contain a
crossing (the teeth). The independent teeth: reverting ack-on-barrier-reject fails at
`bazaar_crossing.itf.json@3`; reverting `reset_ack` fails at
`reconnect_snapback.itf.json@4`.

---

## Layer 3 — Playwright end-to-end

The wasm + TypeScript front-end is tested in a real Chromium browser via Playwright
([`bt-wasm/tests/*.spec.js`](../bt-wasm/tests/)). `npm test` first builds the wasm
(`wasm-pack build . --target web --out-dir pkg --dev`) and the TS (`tsc`), then
serves `bt-wasm/` over `python3 -m http.server 4173` (the `webServer` in
[`playwright.config.js`](../bt-wasm/playwright.config.js)) so `/www/` (the page) and
`/pkg/` (the wasm it imports via `../pkg`) are reachable. Current specs:

- `weapon-deploy.spec.js` — clicking an arsenal weapon button deploys it and affects
  Ernie (the vs-Computer cross-player effect, end to end through the wasm boundary).
- `debug-picker.spec.js` — the `?debug=1` weapon-grant picker grants weapons + funds.

These cover the client paths the Rust suite can't reach (DOM, the wasm bindings, the
render loop). The vs-Computer mode is 100% client-side, so no `bt-server` is needed.

---

## Layer 4 — Differential / oracle + fuzz→replay

This layer pins the engine's *values*, not just that it "works":

- **Differential** ([`bt-core/tests/differential_lineclear.rs`](../bt-core/tests/differential_lineclear.rs)):
  real FFI against the 1994 C++ is impractical (those functions are tangled with
  X11/Motif, packet `send()`s, and display redraws), so this carries an
  **independent, deliberately naive reference** line-clear implementation on a plain
  `Vec<Vec<Option<i32>>>` grid and fuzzes the two against each other over thousands
  of random boards. Divergence in cleared counts, funds, or the resulting grid fails.
  Weapon variants get their own differential (`weapons_oracle`, `oracle`).
- **Oracle** ([`bt-core/tests/oracle.rs`](../bt-core/tests/oracle.rs)): each assertion
  pins a concrete number the original 1994 C++ produces, carrying the `file:line` it
  mirrors (e.g. the human hard-drop bonus `BT_BOARD_HGT - y_` at `BTGame.C:729`).
  These are the antidote to the "Ernie scoring drifted to a plausible-but-wrong
  value" bug class — a port that quietly drifts (28 vs 14, `value*lines` vs
  `value+lines`, bazaar at 25 vs 20) fails *loudly* here. When one fails, the
  question is "did the original really do this?" — read the cited line, don't update
  the constant.
- **Fuzz → replay** ([`bt-replay/tests/fuzz_replay.rs`](../bt-replay/tests/fuzz_replay.rs)):
  turns any weapon-fuzz seed into a faithful, scrubbable replay (via the
  [`Input::AiDrop`](../bt-replay/src/lib.rs) bridge), so a failing seed becomes
  something you can *watch* and reproduces bit-for-bit. Detailed in
  [replays.md](replays.md).

---

## What CI gates on

The GitHub Actions workflow ([`.github/workflows/deploy.yml`](../../.github/workflows/deploy.yml))
runs four jobs on every push/PR plus the deploy:

| Job | Command | Gates deploy? |
|---|---|---|
| **test** | `cargo test --workspace --locked` (layers 1 & 4 + conformance) | yes |
| **clippy** | `cargo clippy --workspace --all-targets -- -D warnings` | yes |
| **tla** | `ci-check.sh` (fast model checks — each *asserts* its expected `NoError`/`Error` outcome, so a check can never pass silently) + `regen-traces.sh --check` (fixtures not stale) | yes |
| **tla-full** | the slow all-fixed `Netcode` check (length 14) | no — `workflow_dispatch` / nightly schedule only |

The **deploy** job `needs: [test, clippy, tla]`, so a green `test`, `clippy`, and the
fast `tla` checks all gate a deploy to fly.io. The slow full-Netcode check runs only
nightly / on manual dispatch, never blocking a PR. The Playwright e2e suite is run
locally / on demand (it builds wasm + TS); see `docs/deployment.md` for the CD
topology.
