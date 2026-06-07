# TLA+ models of the BattleTris netcode

Formal models of the client⇄server protocol, checked with
[Apalache](https://apalache-mc.org/) (symbolic / SMT model checking with
counterexample **traces**). These complement the Rust property tests: the PBT in
`bt-server` guards the *code* in CI; these models explore the *design's* state space
to find — and explain, via traces — bugs in the interaction of features
(bazaar barrier × network delay × reload-rejoin × weapon relay × weapon timing).

## `Bazaar.tla` — the bazaar barrier under network delay

Models the deadlock that real ~Tokyo latency surfaced (fixed in
`bt-server/src/bout.rs` `apply_input`, and pinned by the PBT
`the_client_always_escapes_the_bazaar`). Two state machines over async, in-order
channels:

- **Client** (bot/browser): predicts ahead, sends seq-tagged inputs, learns the
  authoritative state only from delayed snapshots; its `WaitAck` gate holds while its
  seen `ack` trails what it sent.
- **Server** (`Bout::apply_input`): authoritative; rejects non-shopping inputs while
  in the bazaar. The modelled policy knob `AckOnBarrierReject` is the whole bug:
  `TRUE` = the fix (a barrier-rejected input still advances `ack`), `FALSE` = the bug.

"Network delay" = the client's in-flight gameplay inputs can reach the server *after*
it enters the bazaar (modelled by the independent `ServerEnterBazaar`). `NoDeadlock`
asserts the **absorbing stuck state** (`serverBazaar ∧ serverAck < clientSeq ∧ nothing
in flight to advance ack`) is unreachable.

### Run it

```sh
# needs Java 17+ and apalache-mc on PATH (https://github.com/apalache-mc/apalache/releases)
cd tla

# THE BUG — Apalache finds the deadlock and writes a counterexample trace:
apalache-mc check --length=10 --config=BazaarBuggy.cfg Bazaar.tla
#   -> NoDeadlock violated at State 3; trace in _apalache-out/.../violation1.{tla,itf.json}

# THE FIX — Apalache verifies no deadlock is reachable up to the bound:
apalache-mc check --length=12 --config=Bazaar.cfg Bazaar.tla
#   -> The outcome is: NoError
```

The buggy counterexample is the real bug in 3 steps:

| Step | Transition | Result |
|---|---|---|
| 1 | `ServerEnterBazaar` | `serverBazaar=TRUE` (opponent's lines cross) |
| 2 | `ClientSendGameplay` | `clientSeq=1`, input `G#1` in flight, client still thinks it's playing |
| 3 | `ServerDeliverInput` | barrier-rejects `G#1`; **buggy: `serverAck` stays 0** ⇒ `ack(0) < sent(1)`, nothing in flight = stuck forever |

## `Netcode.tla` — the full protocol (bazaar × reconnect × weapons × timing)

`Bazaar.tla` is the teaching model. `Netcode.tla` layers on the rest of the
complexity that motivated formalising this at all, as **toggleable fixes** and
matching invariants:

| Layer | Toggle (TRUE = the shipped fix) | Invariant it guards |
|---|---|---|
| Bazaar barrier × network delay | `AckOnBarrierReject` | `NoDeadlock` — the absorbing ack-gap freeze is unreachable |
| Reload-rejoin / reattach | `ResetAckOnReattach` | `AckBounds` — `serverAck ≤ clientSeq` (a reconnect that resets seq but not ack = the **snap-back**) |
| Weapon firing | *(none — same barrier class as gameplay)* | `WeaponsAccounted` — `weaponsApplied ≤ weaponsFired` |
| Weapon-timing local prediction | `LeaveNeedsAuth` | `LeaveOnlyWhenReal` — no `LeaveBazaar` wasted on a not-yet-bazaar server (the **predicted-leave** freeze) |

Weapons aren't special: a launched weapon is a non-shopping input that crosses the
**same** bazaar barrier as gameplay, so the same `AckOnBarrierReject` fix covers it.
Weapon timing effects matter because they let the client's *local* bazaar prediction
lead the authoritative server — which is exactly the original predicted-leave bug, so
shopping must gate on the authoritative view (`LeaveNeedsAuth`).

### Run it

```sh
cd tla

# THE SHIPPED SYSTEM — every fix on; all four invariants hold (≈6.5 min, length 14):
apalache-mc check --length=14 --config=Netcode.cfg Netcode.tla        # -> NoError

# EACH FIX IS NECESSARY — flip one off, its invariant breaks (each ≈2 s, with a trace):
apalache-mc check --length=10 --config=NetcodeBugAck.cfg   Netcode.tla # NoDeadlock        violated
apalache-mc check --length=10 --config=NetcodeBugReset.cfg Netcode.tla # AckBounds         violated
apalache-mc check --length=10 --config=NetcodeBugLeave.cfg Netcode.tla # LeaveOnlyWhenReal violated
```

### What the model taught us

Modelling disciplined the spec. The first all-fixed run reported a `NoDeadlock`
violation — but the trace showed a *false positive*: the client finishes a bazaar
visit cleanly, the server re-enters the bazaar, and a too-broad `Stuck` predicate
flagged the stale `clientBought`/`clientViewBazaar` as absorbing. It isn't — a
`baz=FALSE` snapshot re-arms it. The real lesson is a **latent assumption in the
client**: `bought` re-arms only on seeing a `baz=FALSE` snapshot between visits, which
the **in-order channel + always-sent snapshots** guarantee in production but the
abstraction did not. `Stuck` was tightened to the sound ack-gap predicate; the
predicted-leave freeze is caught by the safety invariant `LeaveOnlyWhenReal` instead.
That's the loop: the checker forces you to say exactly what you mean.

### …and what we hardened because of it

The latent re-arm assumption was real even if production satisfies it, so we closed it
in the code: `bt-bot`'s `WaitBazaar` arm now **idempotently re-sends `LeaveBazaar` while
the server authoritatively still has us in the bazaar** (`DefensiveReLeave`), gated on
the authoritative flag — never local prediction — so escaping a bazaar we're in no
longer depends on the `bought` re-arm ever seeing an out-of-bazaar snapshot. The model
mirrors it (`ClientReLeave`), which forced one more refinement: a re-leave can produce a
*benign* stale `LeaveBazaar`, so leaves are now tagged **real `L`** (sent with
authoritative confirmation) vs **predicted `P`** (local-only) — only a wasted `P` trips
`LeaveOnlyWhenReal`. With the hardening on, all four invariants still hold; each fix off
still breaks its invariant.

## Closing the loop: TLA⁺ traces → the real Rust (conformance)

The model and the implementation can drift. To tie them together — the
[modelator](https://github.com/informalsystems/modelator) idea — we generate traces
from Apalache and **replay them against the real `Bout`**, asserting the
implementation's `(ack, in_bazaar, weapons-applied)` tracks the model's
`(serverAck, serverBazaar, weaponsApplied)` after every step.

### The generators

- **`Cross.tla`** is the minimal teaching generator: a history variable `crossed` + the
  trap `INVARIANT NotCrossed` force Apalache to emit the shortest trace that performs a
  single bazaar *crossing* (a gameplay input the barrier rejects).
- **`Gen.tla`** is the production generator — a superset model driving the SAME server
  semantics as `Netcode.tla`'s `ServerDeliverInput` across the whole feature space
  (bazaar entry/exit, barrier crossings, weapon delivery, reconnect/`reset_ack`). It
  stamps every state with an **explicit `lastAction` string** so the Rust harness maps
  each step to exactly one `Bout` op by NAME, not by guessing from a state diff (two
  different actions can produce the same diff — an explicit action makes the mapping
  total and unambiguous, and any unmapped action is a hard failure). Each scenario is
  carved out by its own trap invariant in a `Gen*.cfg`:

  | Fixture | cfg / trap | What it exercises |
  |---|---|---|
  | `bazaar_crossing.itf.json` | `GenCross` / `NotCrossed` | a weapon crosses the barrier (ack-on-reject) |
  | `weapon_then_cross.itf.json` | `GenWeaponCross` / `NotWeaponThenCross` | a weapon delivered in play, then a later crossing |
  | `reconnect_snapback.itf.json` | `GenReconnect` / `NotReconnectedWithHistory` | the disconnect → reconnect → `reset_ack` snap-back path |
  | `two_bazaar_visits.itf.json` | `GenTwoVisits` / `NotTwoVisits` | the server enters the bazaar twice in one trace |

### The harness

**`apply_input_conforms_to_every_tla_trace`** (in `bt-server/src/bout.rs`) is
data-driven: it loads EVERY `*.itf.json` in `rust/bt-server/tests/traces/`, drives a
real `Bout` along each (`force_into_bazaar` on `ServerEnterBazaar`, `apply_input` on
`ServerDeliverInput`, `reset_ack` on `Reconnect`), and asserts `(ack, in_bazaar,
weapons-applied)` conformance after every state. It panics loudly on any `lastAction` it
can't map — no silent skips — and requires the corpus to contain a crossing (the teeth).

The teeth are real and independent:
- revert the ack-on-barrier-reject fix → fails at `bazaar_crossing.itf.json@3`
  (`ACK DIVERGED from the model (1)`);
- revert `reset_ack` → fails at `reconnect_snapback.itf.json@4`.

### Regenerating the fixtures

`regen-traces.sh` regenerates all four committed fixtures from the `Gen*.cfg` traps
(normalizing Apalache's per-run timestamp so the committed files are stable):

```sh
APALACHE_MC=/path/to/apalache-mc ./regen-traces.sh           # overwrite the fixtures
APALACHE_MC=/path/to/apalache-mc ./regen-traces.sh --check   # CI: fail if any is stale
```

## CI

Two checked-in scripts back the GitHub Actions `tla` job (see `.github/workflows/deploy.yml`):

- **`ci-check.sh`** runs the FAST model checks and **asserts the expected outcome of
  each** — a buggy cfg MUST report `Error` (its invariant violated), a fixed cfg MUST
  report `NoError`; anything else fails the build, so a check can never pass silently.
  Covers `Bazaar` buggy/fixed, the `Cross` trap, the three `NetcodeBug*` teeth, and a
  reduced-length all-fixed `Netcode` run.
- **`regen-traces.sh --check`** asserts the committed trace fixtures aren't stale.

The full all-fixed `Netcode` check (length 14, ~6.5 min) is too slow for the PR path; it
runs only in the `tla-full` job, gated on `workflow_dispatch` / the nightly schedule.
