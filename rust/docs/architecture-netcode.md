# Netcode architecture: server-authoritative prediction & reconciliation

BattleTris online play is **server-authoritative with client-side prediction**.
The server runs the only authoritative simulation for a matched pair of players;
each client predicts its own inputs locally so play feels instant, then reconciles
to the server's authoritative keyframes. This is the model that replaced the
faithful-but-exploitable peer-to-peer relay the 1994 original used.

This document is the engineering narrative. The *formal* models that prove the
hard parts correct live in [`tla/README.md`](../../tla/README.md) (TLA+/Apalache,
with counterexample traces) and in the project dossier
(`screenshots/netcode-writeup.html`, `screenshots/tla-explainer.html`). This page
links into those rather than restating them.

Related deep dives: [engine.md](engine.md) (the deterministic core the predictor
drives), [replays.md](replays.md) (the totally-ordered log this model makes
recordable), [testing.md](testing.md) (the four-layer suite that pins these
invariants), and `ARCHITECTURE.md` (the crate map and the three data-flow paths).

---

## Why authoritative client/server, not P2P relay

The original BattleTris ran each player's board on their own machine and exchanged
deltas peer-to-peer (`BTCommManager`). A faithful port could have kept that. The
project deliberately chose the authoritative server model instead, because two
properties fall out of it for free (see the `//!` header of
[`bt-server/src/bout.rs`](../bt-server/src/bout.rs)):

- **Anti-cheat.** A client can only send legal *inputs* — move/rotate/drop, launch
  an arsenal weapon, buy/sell, leave the bazaar. It can never inject board state,
  weapons, funds, or score. The server resolves every cross-player effect (Mirror,
  Swap, Lazy Susan, the Mondale/Keating funds taxes) itself. The gate is
  [`is_legal_client_input`](../bt-server/src/bout.rs): the relay-internal variants
  (`ReceiveWeapon`, `ReceiveOpScore`, `AddFunds`, `AiDrop`) and `SetPaused` are
  *rejected* from a client — those are how the server applies cross-player effects,
  and accepting them from a client would let it grant itself weapons/funds or stall
  the opponent's board.

- **A totally-ordered event log.** The server sees every input, in order, so an
  online match can be recorded as a [`bt_replay::VersusReplay`](../bt-replay/src/lib.rs)
  — just the two seeds plus the input stream. Because the engine is deterministic,
  replaying re-runs a `Versus` and the whole relay (weapons, taxes, bazaar, spies)
  reproduces exactly. This closed the long-standing "online games aren't replayable"
  gap (referred to in the code as **D5**). See [replays.md](replays.md).

The migration to this model — and the consolidation of the client logic — happened
in commit `946ac3d` ("Unify client netcode in a shared Rust core; migrate www/ to
TypeScript").

---

## The shared `Predictor` (one implementation, two clients)

Before unification, the prediction/reconciliation logic existed **twice**: once in
hand-written browser JavaScript (`main.js`: `inputSeq` / `unackedInputs` /
`predict` / `applySnapshot` / `applyReprToGame`) and once in the bot's Rust. Every
painful desync bug — the snap-back, the bazaar barrier freeze, the rejoin
desync — lived in the untested JS copy.

[`bt_netcode::Predictor`](../bt-netcode/src/lib.rs) is that logic, written **once**,
so the invariants are pinned by property tests and the two clients (browser via
`bt-wasm`'s `WasmClient`, and the headless `bt-bot`) are provably consistent. The
browser and bot drive the *same* `Predictor`; there is no second reconciliation
implementation that could drift.

### `predict` — local sim + unacked queue + per-bout seq

[`Predictor::predict(input)`](../bt-netcode/src/lib.rs) applies an input to the
local [`bt_core::Game`], queues it as *unacked*, and returns the `(seq, Input)` to
send. Inputs carry a **monotonic per-bout `seq`** starting at 0 with each
`Predictor`, so it lines up with the server's per-bout `ack` baseline (a fresh bout
is never stuck waiting for an ack that can't arrive — see the cross-bout deadlock
discussion below).

`predict` returns `None` — suppressing the send — in two cases, the central gates
that keep prediction and the wire in lockstep:

- a non-shopping input while a bazaar barrier is up (the server would reject it);
- a `BuyWeapon`/`SellWeapon` the local engine rejected (insufficient funds / not
  shopping) — only an *accepted* buy is forwarded.

`LeaveBazaar` is special: it is forwarded but **not applied locally**. The bazaar is
a server-side barrier that clears (via the next keyframe) only once *both* sides are
done; leaving the local sim early would tick it out of a state the server still
holds.

### `on_snapshot` — drop acked, replay the unacked tail (the snap-back fix)

[`Predictor::on_snapshot(ack, you_bazaar, opp_bazaar, keyframe)`](../bt-netcode/src/lib.rs)
reconciles against an authoritative frame:

1. It always **drops acked inputs** (`seq <= ack`) from the unacked queue.
2. On a **keyframe** (the full authoritative state from
   [`Game::client_keyframe_bytes`](../bt-core/src/game.rs)), it **overwrites** the
   local state with the authoritative one, then **replays the still-unacked tail**
   on top.

Replaying the unacked tail is exactly what stops a not-yet-acked input from being
lost — **the snap-back**. The bug that started the whole exercise: a predicted-but-
unconfirmed move would be discarded on reconciliation, so the dropping piece
"snapped back" to where the keyframe said it was. Replaying the tail means the
predicted move survives the restore. The headline property test is
`unacked_inputs_survive_a_keyframe` in
[`bt-netcode/tests/predictor_pbt.rs`](../bt-netcode/tests/predictor_pbt.rs): a
keyframe acking only a prefix must still leave the local state equal to
"all my inputs applied".

The server only attaches a keyframe periodically (it throttles them), keeping
per-frame messages light. Between keyframes the snapshot carries just
`tick`/`ack`/`result` plus the slim
[`SelfStatus`](../bt-server/src/bout.rs)/[`OppView`](../bt-server/src/bout.rs) the
client can't derive from its own prediction (own funds, the bazaar barrier flags,
the opponent's score/lines). The client renders its own board from local
prediction.

---

## The bazaar barrier

The weapons bazaar is a **synchronized barrier**: when combined cleared lines cross
a multiple of 20 ([`BT_LINES_TIL_BAZ`](../bt-core/src/constants.rs)), both players
enter the bazaar together and the **whole match freezes server-side** until both
have left. While the barrier is up, only shopping actions
([`is_bazaar_input`](../bt-server/src/bout.rs): buy/sell/leave) are applied; gameplay
inputs are inert.

On the server, [`Bout::apply_input`](../bt-server/src/bout.rs) enforces this: while
*either* side is in the bazaar, a non-shopping action is rejected — even from the
side that already left first (e.g. a bot makes its picks instantly). On the client,
[`Predictor::barrier()`](../bt-netcode/src/lib.rs) (`you_bazaar || opp_bazaar`)
drives the predict-time gate and matches the browser's `inBazaar()`.

### The deadlock under latency, and the ack-on-barrier-reject fix

This was the marquee netcode bug, found by simulating Tokyo-class RTT (~200 ms) on
a local bot match: the match froze in the bazaar and never recovered (commit
`03197bc`).

The bug is an interaction between two state machines — the server's `apply_input`
ack policy and the client's `WaitAck` reconciliation gate. "Latency" just means
"the client's in-flight gameplay inputs reach the server *after* the bazaar barrier
comes up": the client predicts ahead, and inputs it sent before its snapshot showed
the barrier land late.

The original `apply_input` advanced `ack` only when it **applied** an input. So
those late-arriving inputs were barrier-rejected and **never acked**. The bot's
[`sync::decide`](../bt-bot/src/sync.rs) `WaitAck` gate (`acked < last_sent` ⇒ hold)
then waited forever — never reaching `Shop`, never sending `LeaveBazaar` — and the
whole match hung. (For the browser, the same un-acked input would replay on every
keyframe, drifting the board.)

The fix: **a fresh, legal input advances `ack` as soon as it is *seen* — before the
barrier check — even when the barrier then blocks *applying* it.** `ack` means "I've
processed your inputs up through seq N" (a reconciliation cursor), not "I applied
it". The input is still not applied and not recorded as a replay frame; it just lets
the client's reconciliation discard it and let `ack` catch up to `last_sent`. See
the heavily-commented `apply_input` in
[`bt-server/src/bout.rs`](../bt-server/src/bout.rs).

> Two bout proptests had originally **pinned the buggy behavior** ("ack must NOT
> advance on a bazaar-rejected input") — which is exactly why the deadlock shipped
> and why the bot's liveness proof held (it assumed the server acks what it
> receives, which the real server violated). Both were flipped to assert the
> correct behavior. This is a textbook case of tests encoding a wrong assumption.

The formal counterpart is `Bazaar.tla`'s `AckOnBarrierReject` knob (`TRUE` = fix,
`FALSE` = bug) and the `NoDeadlock` invariant — see
[`tla/README.md`](../../tla/README.md), which shows Apalache finding the deadlock in
three steps and verifying the fix.

---

## The bot's `sync::decide` — a pure-FSM policy layer

The `Predictor` owns the *mechanics* (when to apply, queue, replay). It does not
decide *when* to predict — that is policy, and for the headless bots it lives in
[`bt-bot`'s `sync::decide`](../bt-bot/src/sync.rs): one **total pure function** over
the observable sync state, returning exactly one [`BotAction`] (`End`, `WaitAck`,
`WaitBazaar`, `Shop`, `Play`).

The hazard `decide` guards against: acting on local prediction that has run *ahead*
of the authoritative sim. The original freeze was a bot that *predicted* entering
the bazaar, sent a `LeaveBazaar` the server ate before it had actually entered, then
never re-sent — freezing the match (commit `01f253e`, observed live as "The Count
frozen in bazaar"). The invariants, proptested in the same file:

- **P1 — never-ahead:** `acked < last_sent` ⇒ `WaitAck`. No action emits inputs
  while any are unacked, so the bot never runs ahead of the server.
- **P2 — leave-only-when-real:** `Shop` ⇒ `auth_baz && local_baz`. A `LeaveBazaar`
  is only emitted when the server *really* has us in the bazaar (never on a merely
  predicted entry), and the local sim has been keyframe-synced into it.
- **P3 — barrier:** any bazaar flag set ⇒ never `Play`.
- **P5 — liveness/no-freeze:** a model-based property that simulates a full bazaar
  visit with the real hazards baked in (local prediction leads the server, leaves
  sent before entry are eaten, ack/keyframe latency) and asserts the bot always
  escapes back to `Play`. It has **teeth**: a deliberately-buggy local-leave policy
  (`decide_buggy_local_leave`) trips the harness's hazard guard.

`last_sent` is **per-bout** (reset with each `MatchState`), not the connection's
monotonic `seq` — so a fresh bout (where the server's `ack` resets to 0) doesn't
deadlock waiting for an ack that can never arrive. The regression for this is
`fresh_bout_with_high_seq_is_not_deadlocked`.

A later hardening (commit `ef98d0c`, prompted by the TLA+ model — see below): the
`WaitBazaar` arm now **idempotently re-sends `LeaveBazaar`** while the server
authoritatively still has us in the bazaar (`DefensiveReLeave`), gated on the
authoritative flag and never local prediction, so escaping no longer depends on the
`bought` re-arm ever observing an out-of-bazaar snapshot.

---

## Reconnect, rejoin-grace, and quiesce

These three interlock to keep a live game alive across drops and deploys. The
operational side is documented in `docs/deployment.md`; the netcode-relevant
mechanics:

### Reconnect snap-back (`reset_ack`)

When a client reattaches to a live bout (a refresh, or a socket drop during a server
redeploy), it restarts its input `seq` at 0 — but the bout still holds the
disconnected client's high `ack`. Without intervention, every reconnected input
would be `seq <= ack`, get rejected, and the player's piece would snap back for the
rest of the match (the bug a mid-match redeploy triggered, commit `57cc52b`).

[`Bout::reset_ack(side)`](../bt-server/src/bout.rs) drops that side's baseline to 0
on reattach (the old in-flight inputs are gone — the socket closed and the input
channel was drained ticks ago), so `seq` 1, 2, 3… flow again. The `rejoin` handler
in [`bt-server/src/main.rs`](../bt-server/src/main.rs)'s `run_bout` calls it *before*
sending the resync keyframe, so the client sees `ack: 0`. The formal counterpart is
`Netcode.tla`'s `ResetAckOnReattach` knob and the `AckBounds` invariant
(`serverAck ≤ clientSeq`) — see [`tla/README.md`](../../tla/README.md).

### Rejoin grace

When a *human* side's socket drops mid-bout, `run_bout` freezes the simulation and
starts a grace window (`REJOIN_GRACE`); the still-connected side gets an
`opponentReconnecting` message with the countdown. If the absent player reconnects
in time (`BoutControl::Reattach`), the freeze lifts, both boards are resynced with
keyframes, and both clients get `opponentResumed`. On grace expiry the absent side
forfeits. A dropped **bot** does not get grace — it forfeits immediately. The bot
mirrors this with its own `idle_timed_out` → `BotAction::End` after the grace
window (commit `f10be33`).

### Quiesce-in-place

Because the server is stateful on a single attached fly volume (`replays.db` +
ratings), two machines can't share state, so there is no blue/green. Instead a
deploy uses a quiesce-in-place drain: `POST /admin/drain` pauses new matchmaking at
the single `start_bout` chokepoint while letting in-flight bouts finish and keeping
`rejoin` allowed; the deploy script waits (uncapped) for the active-bout count to
hit zero, then swaps the machine in place. Details and the admin token model are in
`docs/deployment.md`.

---

## What the TLA+ models taught the design

The formal models did not just confirm the code — they **changed** it. From
[`tla/README.md`](../../tla/README.md)'s "What the model taught us":

- The first all-fixed run flagged a false-positive `NoDeadlock` violation, exposing
  a **latent assumption in the client**: the bot's `bought` flag re-arms only on
  seeing a `baz=FALSE` snapshot between visits — which the in-order channel +
  always-sent snapshots guarantee in production but the abstraction did not. Rather
  than rely on it, the code was hardened (the `DefensiveReLeave` re-send above).
- The model and the implementation are tied together by a **conformance harness**:
  `apply_input_conforms_to_every_tla_trace` (in
  [`bt-server/src/bout.rs`](../bt-server/src/bout.rs)) replays a corpus of
  Apalache-generated `*.itf.json` traces against the *real* `Bout` and asserts
  `(ack, in_bazaar, weapons-applied)` tracks the model's
  `(serverAck, serverBazaar, weaponsApplied)` after every step. The teeth: revert
  the ack-on-barrier-reject fix → fails at `bazaar_crossing.itf.json@3`; revert
  `reset_ack` → fails at `reconnect_snapback.itf.json@4`. See
  [testing.md](testing.md) and [`tla/README.md`](../../tla/README.md).

---

## Wire protocol summary

- **Client → server:** input frames built by the one shared
  [`bt_netcode::input_frame`](../bt-netcode/src/lib.rs):
  `{"type":"input","seq":N,"input":<Input serde form>}`. Building it in one place
  means the browser and bot can never disagree on the wire (the hand-rolled JS
  reprs that had to be kept in sync with serde by hand are gone).
- **Server → client:** `{"type":"snapshot", ...}` carrying
  [`Snapshot`](../bt-server/src/bout.rs); plus `matchStart`, `opponentReconnecting`,
  `opponentResumed`, `opponentLeft`, `rejoinFailed`, `heartbeat`, `draining`, and
  the read-only `spectate` two-board view.
- The reconciliation **keyframe** rides the snapshot's `keyframe` field on a
  throttle (first frame, ~2 Hz heartbeat, and any frame after an unpredictable
  cross-player event — driven by [`Versus::take_dirty`](../bt-core/src/versus.rs)).
  It is the byte form of [`Game::client_keyframe_bytes`](../bt-core/src/game.rs),
  with `op_funds` **redacted** so a client can't learn the opponent's funds
  (faithful to the original, where funds are spy-revealed only).
