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
