-------------------------------- MODULE Bazaar --------------------------------
(***************************************************************************)
(* A TLA+ model of the BattleTris bazaar barrier under network delay — the *)
(* protocol whose deadlock real ~Tokyo latency surfaced. Checked with       *)
(* Apalache (symbolic / SMT model checking + trace output).                 *)
(*                                                                          *)
(* The system is the interaction of two state machines over async, in-order *)
(* channels (TCP/WebSocket), abstracting the game to just the facts that    *)
(* drive the freeze:                                                        *)
(*                                                                          *)
(*   CLIENT (the bot / browser): predicts ahead and sends inputs tagged with *)
(*     a monotonic seq; learns the authoritative state only from delayed     *)
(*     snapshots. Its reconciliation gate (bt-bot's `sync::decide`):         *)
(*       - while its seen ack < what it has sent  -> WaitAck (send nothing)  *)
(*       - once it knows it's in the bazaar & caught up -> Shop (LeaveBazaar)*)
(*                                                                          *)
(*   SERVER (bt-server's Bout::apply_input): the authoritative sim. While it  *)
(*     is in the bazaar, a non-shopping input is REJECTED (the barrier). The  *)
(*     modelled policy knob is whether a barrier-rejected input still         *)
(*     advances `ack` — AckOnBarrierReject = TRUE is the FIX, FALSE the bug.  *)
(*                                                                          *)
(* "Network delay" is just: the client's in-flight gameplay inputs can reach *)
(* the server AFTER it has entered the bazaar (driven by the opponent's      *)
(* lines, modelled as the nondeterministic ServerEnterBazaar event). The     *)
(* barrier then rejects them; if they aren't acked, the client's ack never   *)
(* catches up, its WaitAck gate holds forever, it never shops, and the match *)
(* is stuck in the bazaar — an ABSORBING state. NoDeadlock asserts it is     *)
(* unreachable; with the buggy policy Apalache finds it and prints the trace.*)
(***************************************************************************)
EXTENDS Integers, Sequences

CONSTANTS
    \* @type: Int;
    MaxSeq,             \* bound on how many inputs the client may send
    \* @type: Int;
    MaxChan,            \* bound on each channel's length (keeps the model finite)
    \* @type: Bool;
    AckOnBarrierReject  \* TRUE = the fix (ack a barrier-rejected input); FALSE = the bug

VARIABLES
    \* @type: Int;
    clientSeq,          \* highest input seq the client has sent (= last_sent)
    \* @type: Bool;
    clientBought,       \* has the client shopped this bazaar visit?
    \* @type: Int;
    clientViewAck,      \* the ack the client has seen (from the latest snapshot)
    \* @type: Bool;
    clientViewBazaar,   \* the client's belief: am I in the bazaar? (from a snapshot)
    \* @type: Int;
    serverAck,          \* the server's ack for this side (last seq it processed)
    \* @type: Bool;
    serverBazaar,       \* authoritative: is this side in the bazaar?
    \* @type: Seq({ seq: Int, kind: Str });
    inputChan,          \* client -> server, in order. kind: "G" gameplay | "L" LeaveBazaar
    \* @type: Seq({ ack: Int, baz: Bool });
    snapChan            \* server -> client, in order: (ack, in_bazaar)

vars == << clientSeq, clientBought, clientViewAck, clientViewBazaar,
           serverAck, serverBazaar, inputChan, snapChan >>

Init ==
    /\ clientSeq = 0
    /\ clientBought = FALSE
    /\ clientViewAck = 0
    /\ clientViewBazaar = FALSE
    /\ serverAck = 0
    /\ serverBazaar = FALSE
    /\ inputChan = << >>
    /\ snapChan = << >>

(* --- Client actions --------------------------------------------------- *)

\* The client predicts ahead: while it BELIEVES it is playing (its last snapshot
\* did not show the bazaar), it may send a gameplay input. This is the source of
\* the in-flight inputs that cross the barrier under delay.
ClientSendGameplay ==
    /\ ~clientViewBazaar
    /\ clientSeq < MaxSeq
    /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "G" ])
    /\ UNCHANGED << clientBought, clientViewAck, clientViewBazaar,
                    serverAck, serverBazaar, snapChan >>

\* The reconciliation gate's escape: once the client KNOWS it is in the bazaar AND
\* the server has acked everything it sent (so it is not WaitAck-gated), it shops —
\* modelled as sending a single LeaveBazaar. (WaitAck is implicit: when
\* clientViewBazaar and clientViewAck < clientSeq, no client action is enabled.)
ClientShop ==
    /\ clientViewBazaar
    /\ clientViewAck >= clientSeq    \* NOT WaitAck: the server caught up to our sends
    /\ ~clientBought
    /\ clientSeq < MaxSeq
    /\ Len(inputChan) < MaxChan
    /\ clientBought' = TRUE
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "L" ])
    /\ UNCHANGED << clientViewAck, clientViewBazaar, serverAck, serverBazaar, snapChan >>

\* The client receives a snapshot: it updates its view of the ack + bazaar, and
\* re-arms `bought` whenever the server confirms it is out of the bazaar.
ClientDeliverSnapshot ==
    /\ Len(snapChan) > 0
    /\ LET s == Head(snapChan) IN
        /\ snapChan' = Tail(snapChan)
        /\ clientViewAck' = s.ack
        /\ clientViewBazaar' = s.baz
        /\ clientBought' = IF s.baz THEN clientBought ELSE FALSE
    /\ UNCHANGED << clientSeq, serverAck, serverBazaar, inputChan >>

(* --- Server actions --------------------------------------------------- *)

\* The server's combined lines cross the threshold (driven by the OPPONENT): it
\* enters this side's bazaar. Independent of this client's inputs — that is exactly
\* why the client's in-flight inputs can arrive on the wrong side of the barrier.
ServerEnterBazaar ==
    /\ ~serverBazaar
    /\ serverBazaar' = TRUE
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar,
                    serverAck, inputChan, snapChan >>

\* The server processes the next client input (Bout::apply_input):
\*   - LeaveBazaar: bazaar-legal -> applied; leaves the bazaar; advances ack.
\*   - gameplay while in the bazaar: BARRIER-REJECTED -> not applied. Whether it
\*       advances ack is the policy knob (AckOnBarrierReject) — the whole bug.
\*   - gameplay while not in the bazaar: applied; advances ack.
ServerDeliverInput ==
    /\ Len(inputChan) > 0
    /\ LET in == Head(inputChan) IN
        /\ inputChan' = Tail(inputChan)
        /\ IF in.kind = "L"
           THEN /\ serverBazaar' = FALSE
                /\ serverAck' = in.seq
           ELSE IF serverBazaar
                THEN /\ serverBazaar' = serverBazaar
                     /\ serverAck' = IF AckOnBarrierReject THEN in.seq ELSE serverAck
                ELSE /\ serverBazaar' = serverBazaar
                     /\ serverAck' = in.seq
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar, snapChan >>

\* The server emits an authoritative snapshot (every tick): the current ack + bazaar.
ServerSendSnapshot ==
    /\ Len(snapChan) < MaxChan
    /\ snapChan' = Append(snapChan, [ ack |-> serverAck, baz |-> serverBazaar ])
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar,
                    serverAck, serverBazaar, inputChan >>

Next ==
    \/ ClientSendGameplay
    \/ ClientShop
    \/ ClientDeliverSnapshot
    \/ ServerEnterBazaar
    \/ ServerDeliverInput
    \/ ServerSendSnapshot

Spec == Init /\ [][Next]_vars

(* --- Invariants ------------------------------------------------------- *)

\* Sanity safety (holds under BOTH policies): the server never acks past what was
\* sent, and the client's view of the ack never exceeds its own seq.
AckBounds ==
    /\ serverAck <= clientSeq
    /\ clientViewAck <= clientSeq

\* THE property. An ABSORBING stuck state: the server is in the bazaar, has acked
\* less than the client sent, and nothing is in flight that could ever advance the
\* ack — so the client can never satisfy `clientViewAck >= clientSeq`, never shops,
\* never sends LeaveBazaar, and serverBazaar stays TRUE forever. With the fix every
\* delivered input advances ack, so once inputChan empties serverAck == clientSeq and
\* this state is unreachable; with the bug, a barrier-crossing gameplay input leaves
\* serverAck behind forever.
Stuck ==
    /\ serverBazaar
    /\ serverAck < clientSeq
    /\ Len(inputChan) = 0

NoDeadlock == ~Stuck

=============================================================================
