# TLA+ models of the BattleTris netcode

Formal models of the client⇄server protocol, checked with
[Apalache](https://apalache-mc.org/) (symbolic / SMT model checking with
counterexample **traces**). These complement the Rust property tests: the PBT in
`bt-server` guards the *code* in CI; these models explore the *design's* state space
to find — and explain, via traces — bugs in the interaction of features
(bazaar barrier × network delay × reload-rejoin × weapon relay × weapon timing ×
event-channel consistency).

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

## `Netcode.tla` — the full protocol (bazaar × reconnect × weapons × timing × consistency)

`Bazaar.tla` is the teaching model. `Netcode.tla` layers on the rest of the
complexity that motivated formalising this at all, as **toggleable fixes** and
matching invariants:

| Layer | Toggle (TRUE = the shipped fix) | Invariant it guards |
|---|---|---|
| Bazaar barrier × network delay | `AckOnBarrierReject` | `NoDeadlock` — the absorbing ack-gap freeze is unreachable |
| Reload-rejoin / reattach | `ResetAckOnReattach` | `AckBounds` — `serverAck ≤ clientSeq` (a reconnect that resets seq but not ack = the **snap-back**) |
| Weapon firing | *(none — same barrier class as gameplay)* | `WeaponsAccounted` — `weaponsApplied ≤ weaponsFired` |
| Weapon-timing local prediction | `LeaveNeedsAuth` | `LeaveOnlyWhenReal` — no `LeaveBazaar` wasted on a not-yet-bazaar server (the **predicted-leave** freeze) |
| (D) keyframe restore | `KeyframeRestores` | `ResyncConvergence`: a keyframe or reattach delivery never leaves a drifted client drifted |
| (D) resync-request gating | `ResyncOnlyWhenDrifted` | `NoResyncStorm`: the client never requests a resync while already consistent |
| (D) replay guard | `EventsNotReplayed` | `EventDeliveryAccounted`: the client never applies more events than the relay emitted |
| (D) resync-grant purity | `ResyncReadOnly` | `ResultIndependence`: a resync grant never mutates server sim state |

Weapons aren't special: a launched weapon is a non-shopping input that crosses the
**same** bazaar barrier as gameplay, so the same `AckOnBarrierReject` fix covers it.
Weapon timing effects matter because they let the client's *local* bazaar prediction
lead the authoritative server — which is exactly the original predicted-leave bug, so
shopping must gate on the authoritative view (`LeaveNeedsAuth`).

### (D) the Model-B consistency layer

Trigger keyframes are rare (first frame, bazaar entry, Swap/Susan, rejoin, debug grant,
granted resync, final), so between them the relay forwards each cross-player effect
straight to the client's local sim as an event on an ordered channel (`ServerEmitEvent`,
`eventsInFlight` in flight, cumulative `eventsEmitted` / `eventsApplied`). An event
applied inside the same inter-lock window keeps the two sims in lockstep
(`ClientApplyEvent`); an event that straddles a piece lock (`ClientApplyEventLate`) is
the race the model accepts and flags, moving `clientState` from `"consistent"` to
`"drifted"`. This is deliberately an over-approximation: the real detector needs two
consecutive judged per-lock hash mismatches before it fires, since a single-lock
divergence can self-heal, so a modelled `"drifted"` state means the local sim may have
diverged, without asserting that it has. Once the detector fires, the client asks for a
resync (`ClientRequestResync`), gated by `ResyncOnlyWhenDrifted` so it never asks while
already consistent (`NoResyncStorm`), and single-flighted by `resyncPending`, the
model's abstraction of both the client's own throttle and the server's per-side grant
rate limit. The server grants it (`ServerGrantResync`) by raising `pendingKf`; the grant
must be read only (`ResyncReadOnly`) so requesting a resync never itself perturbs server
sim state (`ResultIndependence`). The next delivered keyframe, or the reattach keyframe
on `Reconnect`, then reconciles the local sim back to `"consistent"` (`KeyframeRestores`,
guarded by `ResyncConvergence`), without replaying an event the keyframe already
contains on top of it (`EventsNotReplayed`, guarded by `EventDeliveryAccounted`).

In total `Netcode.tla` has eight toggles (all TRUE = the shipped system: the four fix
toggles above, `DefensiveReLeave` (the hardening toggle described below), and the four
(D) toggles) and eight invariants folded into `AllSafe`: `AckBounds`,
`WeaponsAccounted`, `LeaveOnlyWhenReal`, `NoDeadlock`, `EventDeliveryAccounted`,
`ResyncConvergence`, `NoResyncStorm`, `ResultIndependence`.

### Run it

```sh
cd tla

# THE SHIPPED SYSTEM — every fix on; all eight invariants hold (about 5 min, length 14):
apalache-mc check --length=14 --config=Netcode.cfg Netcode.tla        # -> NoError

# EACH FIX IS NECESSARY — flip one off, its invariant breaks (fast, with a trace):
apalache-mc check --length=10 --config=NetcodeBugAck.cfg         Netcode.tla # NoDeadlock             violated
apalache-mc check --length=10 --config=NetcodeBugReset.cfg       Netcode.tla # AckBounds              violated
apalache-mc check --length=10 --config=NetcodeBugLeave.cfg       Netcode.tla # LeaveOnlyWhenReal       violated
apalache-mc check --length=10 --config=NetcodeBugKfRestore.cfg   Netcode.tla # ResyncConvergence       violated
apalache-mc check --length=10 --config=NetcodeBugStorm.cfg       Netcode.tla # NoResyncStorm           violated
apalache-mc check --length=10 --config=NetcodeBugReplay.cfg      Netcode.tla # EventDeliveryAccounted  violated
apalache-mc check --length=10 --config=NetcodeBugResyncWrite.cfg Netcode.tla # ResultIndependence      violated
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

A later review pass closed one more soundness gap: for `Stuck` to be a genuinely
*absorbing* single-state predicate, the modelled client must be unable to make progress
out of it. `ClientSendGameplay`/`ClientFireWeapon` originally lacked the WaitAck gate the
real bot has (`bt-bot`'s `decide` holds **all** sends while `acked < last_sent`), so the
model was *more permissive* than the code — and a `Stuck`-labelled state could still send.
Adding the `clientViewAck >= clientSeq` conjunct to those actions makes the model faithful
to the bot **and** makes `Stuck` absorbing: with the gap open and nothing in flight, every
client send is gated off and no in-bout action can close it. The only thing that leaves a
`Stuck` state is a `Disconnect`+`Reconnect` — i.e. the player **reloading the page**, which
is precisely the manual escape hatch from the freeze, not an in-bout recovery. So flagging
`Stuck` is correct. (Teeth preserved: each `NetcodeBug*` cfg still violates its invariant;
the all-fixed run is still `NoError`.)

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
  (bazaar entry/exit, barrier crossings, weapon delivery, reconnect/`reset_ack`, and an
  abstract client-consistency layer: `EmitEvent`, `ClientApplyEvent`,
  `ClientApplyEventLate`, `HashMismatch`, `ResyncKeyframe`). It stamps every state with
  an **explicit `lastAction` string** so the Rust harness maps each step to exactly one
  `Bout`/`Predictor` op by NAME, not by guessing from a state diff (two different
  actions can produce the same diff — an explicit action makes the mapping total and
  unambiguous, and any unmapped action is a hard failure). Each scenario is carved out
  by its own trap invariant in a `Gen*.cfg`:

  | Fixture | cfg / trap | What it exercises |
  |---|---|---|
  | `bazaar_crossing.itf.json` | `GenCross` / `NotCrossed` | a weapon crosses the barrier (ack-on-reject) |
  | `weapon_then_cross.itf.json` | `GenWeaponCross` / `NotWeaponThenCross` | a weapon delivered in play, then a later crossing |
  | `reconnect_snapback.itf.json` | `GenReconnect` / `NotReconnectedWithHistory` | the disconnect → reconnect → `reset_ack` snap-back path |
  | `two_bazaar_visits.itf.json` | `GenTwoVisits` / `NotTwoVisits` | the server enters the bazaar twice in one trace |
  | `event_then_resync.itf.json` | `GenEventResync` / `NotEventThenResync` | a cross-player event applied late (drift), then a resync keyframe restores it |
  | `drift_rejoin.itf.json` | `GenDriftRejoin` / `NotDriftRejoin` | the client reconnects while drifted, so the reattach keyframe is what restores it |

### The harness

**`apply_input_conforms_to_every_tla_trace`** (in `bt-server/src/bout.rs`) is
data-driven: it loads EVERY `*.itf.json` in `rust/bt-server/tests/traces/` (a floor of
six fixtures is asserted; a corpus of five or fewer fails the test), drives a real
`Bout` along each (`force_into_bazaar` on `ServerEnterBazaar`, `apply_input` on
`ServerDeliverInput`, `reset_ack` on `Reconnect`), and asserts `(ack, in_bazaar,
weapons-applied)` conformance after every state. It panics loudly on any `lastAction` it
can't map, no silent skips.

Stage 5 also drives a real `bt_netcode::Predictor` alongside the `Bout`, seeded and
stocked to match, and checks it against the model's client-consistency fields at every
state:

- **Seq-stream conformance on sends**: `ClientSendGameplay` / `ClientFireWeapon` /
  `ClientSendLeave` each call `predictor.predict(...)` and assert the returned seq
  equals the model's `clientSeq`.
- **Real event emission**: `EmitEvent` is realized by actually granting and launching a
  weapon for the opponent (`Side::B`) and ticking both the `Bout` and the `Predictor` in
  lockstep until the relay flushes an event to side A (`take_events_for`), so the events
  applied later are genuine cross-player weapon launches.
- **Held-event and applied-event tracking**: a local `held` queue and an
  `events_applied` counter are asserted equal to the model's `eventsInFlight` /
  `eventsApplied` after every state, for both `ClientApplyEvent` and
  `ClientApplyEventLate`.
- **Real keyframe restores**: `ResyncKeyframe` and `Reconnect` each pull a real keyframe
  off `Bout::snapshot_for` and feed it to `predictor.on_snapshot`, then assert the
  predictor's `lock_seq` / `lock_hash` now equal the `Bout`'s, so the restore actually
  reconciled the two sims.
- **Consistent-implies-equal-lock-pair, the over-approximation direction**: the model's
  `"drifted"` means the local sim MAY have diverged, so the harness only asserts real
  lockstep where the model claims `"consistent"`: after every such state the
  predictor's `(lock_seq, lock_hash)` must equal the `Bout`'s. A `"drifted"` state where
  the two real sims happen to still be in lockstep is not a conformance failure; the
  check never runs the other direction.

The corpus-level test also requires non-vacuity, so a regen that stopped exercising a
path fails loudly instead of rotting silently: at least one trace with a barrier
crossing, one with a `Reconnect`, one with a weapon delivered in normal play
(`weaponsApplied > 0`), one with a `ClientApplyEventLate`, one with a `ResyncKeyframe`,
and one with a `Reconnect` while the previous state's `clientState` was `"drifted"`.

The teeth are real and independent:
- revert the ack-on-barrier-reject fix → fails at `bazaar_crossing.itf.json@3`
  (`ACK DIVERGED from the model (1)`);
- revert `reset_ack` → fails at `reconnect_snapback.itf.json@4`;
- strip every late-applied event from the corpus → `any_late_event` fails ("the drift
  teeth are gone");
- strip every resync keyframe from the corpus → `any_resync_kf` fails ("the
  resync-convergence teeth are gone");
- strip every drifted reconnect from the corpus → `any_drift_rejoin` fails ("the
  drifted-reconnect teeth are gone").

### Regenerating the fixtures

`regen-traces.sh` regenerates all six committed fixtures from the `Gen*.cfg` traps
(normalizing Apalache's per-run timestamp so the committed files are stable):

```sh
APALACHE_MC=/path/to/apalache-mc ./regen-traces.sh           # overwrite the fixtures
APALACHE_MC=/path/to/apalache-mc ./regen-traces.sh --check   # CI: fail if any is stale
```

`--check` compares the trace **schema** (the `vars` list, their ITF `varTypes`, and the
config `params`) against a fresh regen, NOT the full state sequence. That is deliberate:
Apalache's Z3 backend can return a *different but equally-short and equally-valid* trap
trace across runs, so a whole-trace byte-diff would be flaky. The schema is what actually
goes stale when the model changes (a renamed/retyped/added variable the harness can no
longer map), and per-state conformance of the committed trace itself is checked — with
teeth — by the Rust `apply_input_conforms_to_every_tla_trace`.

## CI

Two checked-in scripts back the GitHub Actions `tla` job (see `.github/workflows/deploy.yml`):

- **`ci-check.sh`** runs the FAST model checks and **asserts the expected outcome of
  each** — a buggy cfg MUST report `Error` (its invariant violated), a fixed cfg MUST
  report `NoError`; anything else fails the build, so a check can never pass silently.
  Covers `Bazaar` buggy/fixed, the `Cross` trap, the seven `NetcodeBug*` teeth
  (`NetcodeBugAck`, `NetcodeBugReset`, `NetcodeBugLeave`, `NetcodeBugKfRestore`,
  `NetcodeBugStorm`, `NetcodeBugReplay`, `NetcodeBugResyncWrite`), and a reduced-length
  all-fixed `Netcode` run: 11 checks total, about half a minute for the fast suite.
- **`regen-traces.sh --check`** asserts the committed trace fixtures (six of them)
  aren't stale.

The full all-fixed `Netcode` check (length 14, about 5 min) is too slow for the PR path; it
runs only in the `tla-full` job, gated on `workflow_dispatch` / the nightly schedule.
