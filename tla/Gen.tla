---------------------------------- MODULE Gen ---------------------------------
(***************************************************************************)
(* A TRACE GENERATOR for the conformance harness. Unlike `Cross.tla` (which *)
(* only produces a single bazaar-crossing), `Gen` is a superset model that   *)
(* drives the SAME server semantics as `Netcode.tla`'s `ServerDeliverInput`  *)
(* across the WHOLE feature space — bazaar entry/exit, barrier crossings,     *)
(* weapon deliveries, AND reconnect/reset_ack (the snap-back path) — and      *)
(* stamps EVERY state with an explicit `lastAction` string.                  *)
(*                                                                          *)
(* Why an explicit `lastAction`: the Rust harness must map each model step to *)
(* exactly one `Bout` operation. Inferring the action by DIFFING consecutive *)
(* states (channel shrank => deliver; bazaar flipped => enter) is fragile —   *)
(* two different actions can produce the same diff, so a mis-map could pass   *)
(* silently. Emitting the action the model actually took makes the mapping    *)
(* total and unambiguous: the harness reads `lastAction` and FAILS LOUDLY on  *)
(* anything it cannot map (see `replay_itf_trace` in bout.rs).               *)
(*                                                                          *)
(* Each scenario is carved out by a TRAP invariant (a `reached*` history var, *)
(* negated in a .cfg) so Apalache emits the SHORTEST trace that reaches it.   *)
(* All knobs are held at their SHIPPED (fixed) values so the emitted trace is *)
(* the behaviour the real code must match.                                   *)
(***************************************************************************)
EXTENDS Integers, Sequences

CONSTANTS
    \* @type: Int;
    MaxSeq,
    \* @type: Int;
    MaxChan,
    \* @type: Bool;
    AckOnBarrierReject,   \* TRUE = the shipped bazaar fix (ack a barrier-rejected input)
    \* @type: Bool;
    ResetAckOnReattach    \* TRUE = the shipped reconnect fix (drop ack baseline on reattach)

VARIABLES
    \* @type: Int;
    clientSeq,            \* highest input seq the client has sent
    \* @type: Int;
    serverAck,            \* server's processed-seq cursor for this side
    \* @type: Bool;
    serverBazaar,         \* authoritative: this side in the bazaar?
    \* @type: Bool;
    connected,            \* is the client's socket up?
    \* @type: Int;
    weaponsApplied,       \* weapons the server has applied (cumulative)
    \* @type: Seq({ seq: Int, kind: Str });
    inputChan,            \* client -> server, in order. kind: "G"|"W"|"L"
    \* @type: Str;
    lastAction,          \* the action that produced THIS state (the harness reads it)
    \* @type: Int;
    bazaarVisits,        \* how many times the server has ENTERED the bazaar (trap fuel)
    \* @type: Bool;
    reachedCross,        \* a barrier-reject (G/W) delivery has happened
    \* @type: Bool;
    reachedWeaponApply,  \* a weapon was applied during normal play
    \* @type: Bool;
    reachedReconnect,    \* a Reconnect (reset_ack) has happened
    \* @type: Int;
    ackAtReconnect       \* serverAck the instant a Reconnect last fired (snap-back fuel)

vars == << clientSeq, serverAck, serverBazaar, connected, weaponsApplied,
           inputChan, lastAction, bazaarVisits,
           reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

Init ==
    /\ clientSeq = 0
    /\ serverAck = 0
    /\ serverBazaar = FALSE
    /\ connected = TRUE
    /\ weaponsApplied = 0
    /\ inputChan = << >>
    /\ lastAction = "Init"
    /\ bazaarVisits = 0
    /\ reachedCross = FALSE
    /\ reachedWeaponApply = FALSE
    /\ reachedReconnect = FALSE
    /\ ackAtReconnect = 0

\* The client sends a gameplay input (predicting ahead).
ClientSendGameplay ==
    /\ connected /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "G" ])
    /\ lastAction' = "ClientSendGameplay"
    /\ UNCHANGED << serverAck, serverBazaar, connected, weaponsApplied, bazaarVisits,
                    reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

\* The client fires a weapon (a non-shopping input, same barrier class as gameplay).
ClientFireWeapon ==
    /\ connected /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "W" ])
    /\ lastAction' = "ClientFireWeapon"
    /\ UNCHANGED << serverAck, serverBazaar, connected, weaponsApplied, bazaarVisits,
                    reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

\* The client sends a LeaveBazaar.
ClientSendLeave ==
    /\ connected /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "L" ])
    /\ lastAction' = "ClientSendLeave"
    /\ UNCHANGED << serverAck, serverBazaar, connected, weaponsApplied, bazaarVisits,
                    reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

\* The server's combined lines cross: this side enters the bazaar.
ServerEnterBazaar ==
    /\ connected /\ ~serverBazaar
    /\ serverBazaar' = TRUE
    /\ bazaarVisits' = bazaarVisits + 1
    /\ lastAction' = "ServerEnterBazaar"
    /\ UNCHANGED << clientSeq, serverAck, connected, weaponsApplied, inputChan,
                    reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

\* Process the next client input — EXACTLY Netcode.tla's ServerDeliverInput (and the
\* real Bout::apply_input): stale-reject; L leaves; G/W barrier-reject while in the
\* bazaar (ack iff the fix); else applied (a W counts as one delivered weapon).
ServerDeliverInput ==
    /\ connected /\ Len(inputChan) > 0
    /\ LET in == Head(inputChan) IN
        /\ inputChan' = Tail(inputChan)
        /\ lastAction' = "ServerDeliverInput"
        /\ IF in.seq <= serverAck
           THEN /\ UNCHANGED << serverAck, serverBazaar, weaponsApplied >>
                /\ UNCHANGED << reachedCross, reachedWeaponApply >>
           ELSE IF in.kind = "L"
                THEN /\ serverAck' = in.seq
                     /\ serverBazaar' = FALSE
                     /\ UNCHANGED << weaponsApplied, reachedCross, reachedWeaponApply >>
                ELSE IF serverBazaar
                     THEN \* gameplay/weapon hits the barrier: NOT applied; ack iff fix.
                          /\ serverAck' = IF AckOnBarrierReject THEN in.seq ELSE serverAck
                          /\ reachedCross' = TRUE
                          /\ UNCHANGED << serverBazaar, weaponsApplied, reachedWeaponApply >>
                     ELSE \* normal play: applied; a weapon is delivered exactly once.
                          /\ serverAck' = in.seq
                          /\ weaponsApplied' = IF in.kind = "W" THEN weaponsApplied + 1 ELSE weaponsApplied
                          /\ reachedWeaponApply' = IF in.kind = "W" THEN TRUE ELSE reachedWeaponApply
                          /\ UNCHANGED << serverBazaar, reachedCross >>
    /\ UNCHANGED << clientSeq, connected, bazaarVisits, reachedReconnect, ackAtReconnect >>

\* The socket drops: the client's in-flight inputs are flushed; the bout freezes.
Disconnect ==
    /\ connected
    /\ connected' = FALSE
    /\ inputChan' = << >>
    /\ lastAction' = "Disconnect"
    /\ UNCHANGED << clientSeq, serverAck, serverBazaar, weaponsApplied, bazaarVisits,
                    reachedCross, reachedWeaponApply, reachedReconnect, ackAtReconnect >>

\* The client reloads + reconnects: it restarts seq at 0 and the server runs reset_ack
\* (the fix) so the fresh low seqs aren't `seq <= ack`-rejected (the snap-back).
Reconnect ==
    /\ ~connected
    /\ connected' = TRUE
    /\ clientSeq' = 0
    /\ serverAck' = IF ResetAckOnReattach THEN 0 ELSE serverAck
    /\ lastAction' = "Reconnect"
    /\ reachedReconnect' = TRUE
    /\ ackAtReconnect' = serverAck   \* the ack JUST BEFORE reset_ack drops it
    /\ UNCHANGED << serverBazaar, weaponsApplied, inputChan, bazaarVisits,
                    reachedCross, reachedWeaponApply >>

Next ==
    \/ ClientSendGameplay \/ ClientFireWeapon \/ ClientSendLeave
    \/ ServerEnterBazaar  \/ ServerDeliverInput
    \/ Disconnect         \/ Reconnect

Spec == Init /\ [][Next]_vars

(* === Trap invariants (negated in the .cfgs to force a scenario) ========== *)

\* A barrier crossing happened (the ack-on-barrier-reject step).
NotCrossed == ~reachedCross

\* A weapon was applied during normal play AND a later barrier crossing happened —
\* forces a trace that delivers a weapon, then crosses the barrier with another input.
NotWeaponThenCross == ~(reachedWeaponApply /\ reachedCross)

\* A reconnect/reset_ack happened AFTER the server had acked some inputs — the exact
\* snap-back setup. `ackAtReconnect > 0` means the server had a nonzero ack baseline the
\* instant the client reconnected with a fresh seq=0; reset_ack (the fix) must then drop
\* serverAck to 0 so the fresh low seqs flow. (With the fix ON, the post-reconnect state
\* has serverAck=0 even though ackAtReconnect>0 — exactly what the harness checks.)
NotReconnectedWithHistory == ~(reachedReconnect /\ ackAtReconnect > 0)

\* The server has entered the bazaar at least twice (multiple visits in one trace).
NotTwoVisits == bazaarVisits < 2

===============================================================================
