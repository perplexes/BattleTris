------------------------------- MODULE Netcode -------------------------------
(***************************************************************************)
(* The full BattleTris client<->server netcode, layering three more features *)
(* onto the bazaar barrier of `Bazaar.tla`:                                 *)
(*                                                                          *)
(*   (A) RECONNECT / REATTACH — a socket drop flushes the client's in-flight *)
(*       inputs; on reconnect the client restarts its seq at 0 and the       *)
(*       server runs `reset_ack` (toggle ResetAckOnReattach). Without it the  *)
(*       fresh low seqs are all `seq <= ack`-rejected: the "snap-back".       *)
(*                                                                          *)
(*   (B) WEAPONS — a launched weapon is just another non-shopping input that  *)
(*       crosses the SAME bazaar barrier as gameplay, so the SAME ack policy  *)
(*       (AckOnBarrierReject) governs it. We also count fired vs applied to   *)
(*       assert weapons are never over-applied.                              *)
(*                                                                          *)
(*   (C) WEAPON TIMING — effects that speed up / slow down drops let the      *)
(*       client's LOCAL bazaar prediction LEAD the authoritative server. The  *)
(*       client must shop only on the AUTHORITATIVE view (toggle             *)
(*       LeaveNeedsAuth); shopping on a mere local prediction sends a         *)
(*       LeaveBazaar the server eats before it is in the bazaar — the         *)
(*       ORIGINAL predicted-leave freeze.                                    *)
(*                                                                          *)
(* Four toggles (all TRUE = the shipped, fixed system) and four invariants:  *)
(*   AckBounds          server never acks past what was sent (catches the     *)
(*                       reset_ack / snap-back bug at a reconnect).           *)
(*   WeaponsAccounted   the server never applies more weapons than were fired.*)
(*   LeaveOnlyWhenReal  no LeaveBazaar is wasted on a not-yet-bazaar server    *)
(*                       (catches the predicted-leave freeze).               *)
(*   NoDeadlock         the absorbing "stuck in the bazaar" state — by either  *)
(*                       the ack gap OR a wasted leave — is unreachable.       *)
(***************************************************************************)
EXTENDS Integers, Sequences

CONSTANTS
    \* @type: Int;
    MaxSeq,
    \* @type: Int;
    MaxChan,
    \* @type: Bool;
    AckOnBarrierReject,   \* (B/bazaar) ack a barrier-rejected input          — TRUE = fix
    \* @type: Bool;
    ResetAckOnReattach,   \* (A) drop the ack baseline on reconnect           — TRUE = fix
    \* @type: Bool;
    LeaveNeedsAuth,       \* (C) shop only on the authoritative bazaar view   — TRUE = fix
    \* @type: Bool;
    DefensiveReLeave      \* (hardening) re-send LeaveBazaar while authoritatively in our bazaar

VARIABLES
    \* @type: Int;
    clientSeq,            \* last input seq the client sent
    \* @type: Bool;
    clientBought,         \* shopped this bazaar visit?
    \* @type: Int;
    clientViewAck,        \* ack seen from the latest snapshot
    \* @type: Bool;
    clientViewBazaar,     \* AUTHORITATIVE bazaar belief (from a snapshot)
    \* @type: Bool;
    clientLocalBazaar,    \* LOCAL predicted bazaar (its own sim — can LEAD the server)
    \* @type: Int;
    serverAck,            \* server's processed-seq cursor for this side
    \* @type: Bool;
    serverBazaar,         \* authoritative: this side in the bazaar?
    \* @type: Seq({ seq: Int, kind: Str });
    inputChan,            \* client -> server, in order. kind: "G" | "W" (weapon) | "L" (leave)
    \* @type: Seq({ ack: Int, baz: Bool });
    snapChan,             \* server -> client, in order
    \* @type: Bool;
    connected,            \* is the client's socket up? (down = in the rejoin grace)
    \* @type: Int;
    weaponsFired,         \* weapons the client has launched (cumulative)
    \* @type: Int;
    weaponsApplied,       \* weapons the server has applied (cumulative)
    \* @type: Bool;
    wastedLeave           \* did the server ever eat a LeaveBazaar while NOT in the bazaar?

vars == << clientSeq, clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
           serverAck, serverBazaar, inputChan, snapChan, connected,
           weaponsFired, weaponsApplied, wastedLeave >>

Init ==
    /\ clientSeq = 0 /\ clientBought = FALSE
    /\ clientViewAck = 0 /\ clientViewBazaar = FALSE /\ clientLocalBazaar = FALSE
    /\ serverAck = 0 /\ serverBazaar = FALSE
    /\ inputChan = << >> /\ snapChan = << >>
    /\ connected = TRUE
    /\ weaponsFired = 0 /\ weaponsApplied = 0 /\ wastedLeave = FALSE

(* === Client ============================================================ *)

\* Predict ahead while it believes it is playing (neither view shows the bazaar). NOTE we
\* deliberately do NOT gate this on `clientViewAck >= clientSeq`: the real bot, once
\* `decide` returns `Play`, emits a BURST of inputs in one tick (several moves/rotates/a
\* drop, plus maybe a weapon launch) — so MULTIPLE inputs are genuinely in flight before
\* any ack arrives, and the server can enter the bazaar mid-burst. Keeping this ungated
\* (bounded only by MaxChan) is what lets the model explore those multi-input crossings —
\* the exact shape of the original freeze. (Stuck stays absorbing without a gate here: see
\* the Stuck comment — the only ESCAPE actions, ClientShop/ClientReLeave, ARE WaitAck-gated,
\* and a gameplay/weapon send can never advance serverAck while serverBazaar holds.)
ClientSendGameplay ==
    /\ connected /\ ~clientLocalBazaar /\ ~clientViewBazaar
    /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "G" ])
    /\ UNCHANGED << clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, serverBazaar, snapChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

\* Fire a weapon — a non-shopping input, same barrier class as gameplay. Ungated for the
\* same burst reason as ClientSendGameplay (a Play tick can launch a weapon AND place).
ClientFireWeapon ==
    /\ connected /\ ~clientLocalBazaar /\ ~clientViewBazaar
    /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ weaponsFired' = weaponsFired + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "W" ])
    /\ UNCHANGED << clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, serverBazaar, snapChan, connected, weaponsApplied, wastedLeave >>

\* (C) A weapon-timing effect makes the LOCAL sim predict bazaar entry ahead of
\* the authoritative server. Pure local prediction; a keyframe later corrects it.
ClientLocalEnterBazaar ==
    /\ connected /\ ~clientLocalBazaar
    /\ clientLocalBazaar' = TRUE
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar,
                    serverAck, serverBazaar, inputChan, snapChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

\* The escape: shop + LeaveBazaar. Gated on the AUTHORITATIVE view when fixed;
\* on the mere LOCAL prediction when buggy (the predicted-leave freeze).
ClientShop ==
    /\ connected
    /\ (IF LeaveNeedsAuth THEN clientViewBazaar ELSE clientLocalBazaar)
    /\ clientViewAck >= clientSeq        \* not WaitAck
    /\ ~clientBought
    /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientBought' = TRUE
    /\ clientSeq' = clientSeq + 1
    \* A leave sent WITH authoritative confirmation is a real "L"; one sent on mere
    \* local prediction (possible only when LeaveNeedsAuth = FALSE) is a "P" — the only
    \* kind that can be WASTED if the server eats it before its bazaar visit.
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> IF clientViewBazaar THEN "L" ELSE "P" ])
    /\ UNCHANGED << clientViewAck, clientViewBazaar, clientLocalBazaar, serverAck, serverBazaar,
                    snapChan, connected, weaponsFired, weaponsApplied, wastedLeave >>

\* HARDENING (mirrors bt-bot's WaitBazaar arm): while the server AUTHORITATIVELY still
\* has us in the bazaar and we've already shopped, keep (idempotently) re-sending
\* LeaveBazaar. Escaping the bazaar then never depends on the `bought` re-arm observing
\* an out-of-bazaar snapshot (the latent assumption the model surfaced). Gated on
\* clientViewBazaar (authoritative), never local prediction — so it is always a real "L".
ClientReLeave ==
    /\ DefensiveReLeave
    /\ connected /\ clientViewBazaar /\ clientBought
    /\ clientViewAck >= clientSeq        \* not WaitAck
    /\ clientSeq < MaxSeq /\ Len(inputChan) < MaxChan
    /\ clientSeq' = clientSeq + 1
    /\ inputChan' = Append(inputChan, [ seq |-> clientSeq + 1, kind |-> "L" ])
    /\ UNCHANGED << clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, serverBazaar, snapChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

\* Receive a snapshot: a keyframe resyncs BOTH views (authoritative + local) and
\* re-arms `bought` whenever the server confirms we are out of the bazaar.
ClientDeliverSnapshot ==
    /\ connected /\ Len(snapChan) > 0
    /\ LET s == Head(snapChan) IN
        /\ snapChan' = Tail(snapChan)
        /\ clientViewAck' = s.ack
        /\ clientViewBazaar' = s.baz
        /\ clientLocalBazaar' = s.baz
        /\ clientBought' = IF s.baz THEN clientBought ELSE FALSE
    /\ UNCHANGED << clientSeq, serverAck, serverBazaar, inputChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

(* === Server ============================================================ *)

\* The opponent's lines cross the threshold: this side enters the bazaar.
ServerEnterBazaar ==
    /\ connected /\ ~serverBazaar
    /\ serverBazaar' = TRUE
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, inputChan, snapChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

\* Process the next client input — the heart of the protocol (Bout::apply_input).
ServerDeliverInput ==
    /\ connected /\ Len(inputChan) > 0
    /\ LET in == Head(inputChan) IN
        /\ inputChan' = Tail(inputChan)
        /\ IF in.seq <= serverAck
           THEN \* stale / replayed (e.g. a fresh client's low seq vs a NOT-reset ack):
                \* rejected, nothing changes — this is the snap-back when reset_ack is off.
                UNCHANGED << serverAck, serverBazaar, weaponsApplied, wastedLeave >>
           ELSE IF in.kind \in {"L", "P"}
                THEN \* LeaveBazaar (bazaar-legal; ack advances; leaving when not in the
                     \* bazaar is a harmless no-op). WASTED only if it was a PREDICTED leave
                     \* ("P", no authoritative confirmation) eaten before the server's bazaar
                     \* visit — the predicted-leave freeze. A duplicate/stale real "L" (a
                     \* defensive re-leave arriving after we already left) is benign.
                     /\ serverAck' = in.seq
                     /\ serverBazaar' = FALSE
                     /\ wastedLeave' = IF (in.kind = "P" /\ ~serverBazaar) THEN TRUE ELSE wastedLeave
                     /\ UNCHANGED weaponsApplied
                ELSE IF serverBazaar
                     THEN \* gameplay / weapon hits the barrier: NOT applied. Ack advances
                          \* iff AckOnBarrierReject (the fix) — else the gap never closes.
                          /\ serverAck' = IF AckOnBarrierReject THEN in.seq ELSE serverAck
                          /\ UNCHANGED << serverBazaar, weaponsApplied, wastedLeave >>
                     ELSE \* normal play: applied; a weapon counts as delivered exactly once.
                          /\ serverAck' = in.seq
                          /\ weaponsApplied' = IF in.kind = "W" THEN weaponsApplied + 1 ELSE weaponsApplied
                          /\ UNCHANGED << serverBazaar, wastedLeave >>
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    snapChan, connected, weaponsFired >>

\* Emit an authoritative snapshot.
ServerSendSnapshot ==
    /\ connected /\ Len(snapChan) < MaxChan
    /\ snapChan' = Append(snapChan, [ ack |-> serverAck, baz |-> serverBazaar ])
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, serverBazaar, inputChan, connected,
                    weaponsFired, weaponsApplied, wastedLeave >>

(* === Reconnect / reattach ============================================== *)

\* The socket drops: the client's in-flight inputs and any undelivered snapshots
\* are gone; the server freezes the bout (no further actions until reconnect).
Disconnect ==
    /\ connected
    /\ connected' = FALSE
    /\ inputChan' = << >>
    /\ snapChan' = << >>
    /\ UNCHANGED << clientSeq, clientBought, clientViewAck, clientViewBazaar, clientLocalBazaar,
                    serverAck, serverBazaar, weaponsFired, weaponsApplied, wastedLeave >>

\* The client reloads + reconnects: it restarts seq at 0, a keyframe resyncs its
\* views to the authoritative state, and `reset_ack` (the fix) drops the server's
\* ack baseline so the fresh low seqs are accepted instead of `seq <= ack`-rejected.
Reconnect ==
    /\ ~connected
    /\ connected' = TRUE
    /\ clientSeq' = 0
    /\ clientViewAck' = 0
    /\ clientBought' = FALSE
    /\ clientViewBazaar' = serverBazaar
    /\ clientLocalBazaar' = serverBazaar
    /\ serverAck' = IF ResetAckOnReattach THEN 0 ELSE serverAck
    /\ UNCHANGED << serverBazaar, inputChan, snapChan, weaponsFired, weaponsApplied, wastedLeave >>

Next ==
    \/ ClientSendGameplay \/ ClientFireWeapon \/ ClientLocalEnterBazaar
    \/ ClientShop         \/ ClientReLeave     \/ ClientDeliverSnapshot
    \/ ServerEnterBazaar  \/ ServerDeliverInput \/ ServerSendSnapshot
    \/ Disconnect         \/ Reconnect

Spec == Init /\ [][Next]_vars

(* === Invariants ======================================================== *)

\* (A) The server never claims to have acked more than the client has sent. A
\* reconnect that resets the client's seq but NOT the ack (reset_ack off) breaks
\* this immediately — that is the snap-back.
AckBounds == serverAck <= clientSeq

\* (B) Weapons are never over-applied (delivered at most once each).
WeaponsAccounted == weaponsApplied <= weaponsFired

\* (C) No LeaveBazaar is ever wasted on a server that is not in the bazaar.
LeaveOnlyWhenReal == ~wastedLeave

\* The "frozen in the bazaar" state: the server is in the bazaar, the ack gap is open (a
\* barrier-crossing input was never acked), and nothing is in flight to close it.
\*
\* SOUNDNESS of this as a single-state predicate. The gap closes only if serverAck rises,
\* which happens only in ServerDeliverInput (or a Reconnect — see below). In a Stuck state
\* inputChan is empty, so ServerDeliverInput is disabled until the client sends. The client
\* CAN still send gameplay/weapon inputs (it may not yet know it is in the bazaar — that is
\* the whole point), but those can NEVER advance serverAck while serverBazaar holds: under
\* the bug a barrier-rejected input is not acked, and a fresh non-bazaar apply is impossible
\* while serverBazaar = TRUE. The only delivery that WOULD advance serverAck-and-leave is an
\* "L"/"P", emitted solely by ClientShop / ClientReLeave — both of which ARE WaitAck-gated
\* (`clientViewAck >= clientSeq`). Since clientViewAck can never exceed serverAck (snapshots
\* carry serverAck) and serverAck < clientSeq here, those escape actions are DISABLED. So no
\* sequence of in-bout actions can close the gap — the match is genuinely frozen. (Note it
\* is the ESCAPE actions' existing gate, not a gate on gameplay sends, that makes Stuck
\* absorbing — so gameplay/weapon sends stay ungated and the model still explores the
\* multi-input-in-flight bursts that produce the crossing.)
\*
\* The ONE thing that can leave a Stuck state is Disconnect+Reconnect — i.e. the player
\* RELOADS THE PAGE. That is not an in-bout recovery; it is precisely the user's
\* manual escape hatch from the freeze (and the snap-back era's "just refresh" workaround).
\* So flagging Stuck is correct: it is the freeze a human had to reload out of. With the
\* ack-on-barrier-reject fix every delivered input advances serverAck, so once the channel
\* empties serverAck == clientSeq and Stuck is unreachable (the all-fixed check is NoError).
\*
\* (The other freeze mechanism — a wasted predicted-leave — is caught soundly by the
\* safety invariant LeaveOnlyWhenReal. We deliberately do NOT add a `bought`-latched
\* disjunct here: whether such a state is absorbing depends on whether a future
\* baz=FALSE snapshot re-arms `bought`, which the in-order channel + always-sent
\* snapshots guarantee in reality but the abstraction does not — so it is not a sound
\* single-state characterization of a deadlock. See README.)
Stuck ==
    /\ connected
    /\ serverBazaar
    /\ serverAck < clientSeq
    /\ Len(inputChan) = 0
NoDeadlock == ~Stuck

AllSafe == AckBounds /\ WeaponsAccounted /\ LeaveOnlyWhenReal /\ NoDeadlock

=============================================================================
