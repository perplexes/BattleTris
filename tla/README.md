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

## Roadmap

Layers to add (the rest of the complexity that motivated modelling):

- **Reload-rejoin / reattach** — socket drop, `REJOIN_GRACE` freeze, `reset_ack`,
  resume; check no state loss / no deadlock across a reconnect.
- **Weapon relay** — `receive_weapon` + fire; check every launched weapon is
  eventually delivered exactly once.
- **Weapon timing effects** — effects that speed up / slow down drops; check they
  don't interact with the bazaar/ack machinery to violate liveness.
