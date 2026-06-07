--------------------------------- MODULE Cross --------------------------------
(***************************************************************************)
(* A tiny generator model whose only job is to emit a TRACE that exercises  *)
(* the bazaar-crossing step — a gameplay input delivered while the server is *)
(* in the bazaar — so a Rust conformance harness can replay it against the   *)
(* real `Bout::apply_input` and check the implementation tracks the model.   *)
(*                                                                          *)
(* `crossed` is a history/monitor variable set the moment a barrier-reject   *)
(* delivery happens; the config's trap invariant `~crossed` forces Apalache  *)
(* to produce the shortest trace that performs one. (Same `apply_input` ack  *)
(* policy knob as Bazaar.tla; we keep it TRUE so the trace is the SHIPPED     *)
(* behaviour the real code must match.)                                     *)
(***************************************************************************)
EXTENDS Integers, Sequences

CONSTANTS
    \* @type: Int;
    MaxSeq,
    \* @type: Int;
    MaxChan,
    \* @type: Bool;
    AckOnBarrierReject

VARIABLES
    \* @type: Int;
    clientSeq,
    \* @type: Int;
    serverAck,
    \* @type: Bool;
    serverBazaar,
    \* @type: Seq({ seq: Int, kind: Str });
    inputChan,
    \* @type: Bool;
    crossed

vars == << clientSeq, serverAck, serverBazaar, inputChan, crossed >>

Init ==
    /\ clientSeq = 0
    /\ serverAck = 0
    /\ serverBazaar = FALSE
    /\ inputChan = << >>
    /\ crossed = FALSE

ClientSend ==
    /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "G" ])
    /\ UNCHANGED << serverAck, serverBazaar, crossed >>

ServerEnter ==
    /\ ~serverBazaar
    /\ serverBazaar' = TRUE
    /\ UNCHANGED << clientSeq, serverAck, inputChan, crossed >>

ServerDeliver ==
    /\ Len(inputChan) > 0
    /\ LET in == Head(inputChan) IN
        /\ inputChan' = Tail(inputChan)
        /\ IF serverBazaar
           THEN \* barrier-reject: not applied; ack advances iff the fix is on.
                /\ serverAck' = IF AckOnBarrierReject THEN in.seq ELSE serverAck
                /\ crossed' = TRUE
                /\ UNCHANGED serverBazaar
           ELSE \* normal play: applied.
                /\ serverAck' = in.seq
                /\ UNCHANGED << serverBazaar, crossed >>
    /\ UNCHANGED clientSeq

Next == ClientSend \/ ServerEnter \/ ServerDeliver

NotCrossed == ~crossed   \* trap: Apalache finds the shortest trace that crosses

=============================================================================
