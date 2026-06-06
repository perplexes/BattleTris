//! The bot's client/server synchronization state machine — extracted as a pure
//! decision function so the netcode invariants can be property-tested.
//!
//! Why this exists: the bot keeps a LOCAL predicted `Game` and reconciles it to
//! the server's authoritative keyframes. The hazard is acting on local prediction
//! that has run AHEAD of the authoritative sim. The original bug was a bot that
//! PREDICTED entering the bazaar, sent a `LeaveBazaar` the server ate before it had
//! actually entered, then never re-sent — freezing the whole match in the bazaar.
//! That logic lived as a handful of booleans scattered through the driver loop;
//! here it's one total function over the observable sync state, with the safety
//! invariants pinned by proptest below (P1–P3 + a model-based no-freeze liveness
//! property, P5). The driver ([`crate::drive_tick`]) just interprets the result.

/// Everything `decide` needs, read once per tick from the live [`crate::MatchState`].
/// All fields are OBSERVABLE (no hidden coupling): three come from the authoritative
/// snapshot (`acked`, `auth_baz`, `opp_baz`), two from our local sim (`local_baz`,
/// and `last_sent`/`bought` which we maintain), so the function is trivially testable.
#[derive(Clone, Copy, Debug)]
pub struct SyncState {
    /// The match has ended (a result arrived).
    pub done: bool,
    /// No authoritative frame for too long — the opponent went silent.
    pub idle_timed_out: bool,
    /// The last input seq the SERVER has confirmed applying for us (snapshot `ack`).
    pub acked: u64,
    /// The seq of our most recently SENT input this bout. `acked < last_sent` ⇒ we
    /// have inputs in flight and MUST NOT act (that would run ahead of the server).
    pub last_sent: u64,
    /// The server says WE are in our bazaar (authoritative — the only reliable read).
    pub auth_baz: bool,
    /// The server says the OPPONENT is shopping (their bazaar freezes us too).
    pub opp_baz: bool,
    /// Our LOCAL predicted sim thinks it's in the bazaar. On its own this is flaky
    /// (the bazaar is a combined-lines mechanic a standalone `Game` can't predict);
    /// it only becomes trustworthy once a keyframe restores it, which is exactly why
    /// we require it AND `auth_baz` before buying — so the buys land in sync.
    pub local_baz: bool,
    /// We've already bought + sent the leave for THIS bazaar visit (re-armed when the
    /// server confirms we left, i.e. when `auth_baz` goes false again).
    pub bought: bool,
}

/// What the driver should do this tick. Exactly one applies — the function is total.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotAction {
    /// The bout is over (result or opponent-silent) — stop driving.
    End,
    /// Inputs are still in flight; hold until the server catches up. THE gate.
    WaitAck,
    /// A bazaar barrier is up but it isn't ours to clear (opponent shopping, or our
    /// local sim hasn't synced into our bazaar yet) — just wait it out.
    WaitBazaar,
    /// We're authoritatively AND locally in our bazaar and haven't bought yet — buy a
    /// loadout and leave (the only way to un-freeze our side), exactly once.
    Shop,
    /// Clear to advance the local sim and (subject to cooldowns) launch + place.
    Play,
}

/// The whole synchronization policy, as one pure total function. Order matters: each
/// guard assumes the earlier ones failed.
///
/// Invariants (proptested below):
/// - **P1 never-ahead:** `acked < last_sent` ⇒ `WaitAck` (no action emits inputs
///   while any are unacked, so the bot never runs ahead of the authoritative sim).
/// - **P2 leave-only-when-real:** `Shop` ⇒ `auth_baz && local_baz` — a `LeaveBazaar`
///   can only be emitted when the server REALLY has us in the bazaar (never on a
///   merely predicted entry: the original freeze).
/// - **P3 barrier:** any bazaar flag set ⇒ never `Play` (we don't place/launch into
///   a frozen sim).
pub fn decide(s: &SyncState) -> BotAction {
    if s.done || s.idle_timed_out {
        BotAction::End
    } else if s.acked < s.last_sent {
        BotAction::WaitAck
    } else if s.auth_baz && s.local_baz && !s.bought {
        BotAction::Shop
    } else if s.auth_baz || s.opp_baz || s.local_baz {
        BotAction::WaitBazaar
    } else {
        BotAction::Play
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// A `SyncState` over small, adversarial ranges (acks near each other so the
    /// `acked < last_sent` boundary is exercised; all flag combinations reachable).
    fn any_sync() -> impl Strategy<Value = SyncState> {
        (
            any::<bool>(),     // done
            any::<bool>(),     // idle_timed_out
            0u64..8,           // acked
            0u64..8,           // last_sent
            any::<bool>(),     // auth_baz
            any::<bool>(),     // opp_baz
            any::<bool>(),     // local_baz
            any::<bool>(),     // bought
        )
            .prop_map(
                |(done, idle_timed_out, acked, last_sent, auth_baz, opp_baz, local_baz, bought)| {
                    SyncState {
                        done,
                        idle_timed_out,
                        acked,
                        last_sent,
                        auth_baz,
                        opp_baz,
                        local_baz,
                        bought,
                    }
                },
            )
    }

    proptest! {
        /// P1 — never act with inputs in flight. The ONLY decisions that emit inputs
        /// are `Shop` and `Play`; neither may be chosen while `acked < last_sent`
        /// (unless the bout is already ending, which emits nothing).
        #[test]
        fn p1_never_acts_ahead_of_the_server(s in any_sync()) {
            if s.acked < s.last_sent && !s.done && !s.idle_timed_out {
                let a = decide(&s);
                prop_assert!(
                    matches!(a, BotAction::WaitAck),
                    "acted ({a:?}) with inputs in flight (acked {} < last_sent {})",
                    s.acked, s.last_sent
                );
            }
        }

        /// P2 — a bazaar leave (`Shop`) only when the server REALLY has us shopping
        /// AND our local sim has synced into it. This is the direct guard against the
        /// original freeze (a leave sent on a merely predicted entry).
        #[test]
        fn p2_leaves_bazaar_only_when_authoritatively_and_locally_in_it(s in any_sync()) {
            if decide(&s) == BotAction::Shop {
                prop_assert!(s.auth_baz, "Shop without authoritative bazaar");
                prop_assert!(s.local_baz, "Shop without local bazaar sync");
                prop_assert!(!s.bought, "Shop after already buying this visit");
                prop_assert!(s.acked >= s.last_sent, "Shop with inputs still in flight");
            }
        }

        /// P3 — never place/launch (`Play`) while ANY bazaar flag is up: the sim is
        /// frozen, so the server would just reject the inputs.
        #[test]
        fn p3_never_plays_into_a_bazaar_barrier(s in any_sync()) {
            if s.auth_baz || s.opp_baz || s.local_baz {
                prop_assert_ne!(
                    decide(&s),
                    BotAction::Play,
                    "played into a bazaar barrier (auth {}, opp {}, local {})",
                    s.auth_baz, s.opp_baz, s.local_baz
                );
            }
        }

        /// `decide` is total — it always returns, for every input. (Trivially true by
        /// construction; this pins it so a future refactor that adds a fallible path
        /// has to keep it total.)
        #[test]
        fn decide_is_total(s in any_sync()) {
            let _ = decide(&s);
        }
    }

    /// P5 — the gold property: LIVENESS / no-freeze. We model a full bazaar visit with
    /// the real hazards baked in, and assert the bot always escapes back to `Play`:
    ///
    /// - **Local prediction LEADS the server.** Our local sim predicts entering the
    ///   bazaar `lead` ticks before the server authoritatively does (the bazaar is a
    ///   combined-lines mechanic we can't predict exactly). So there's a window with
    ///   `local_baz = true`, `auth_baz = false` — the exact window the original freeze
    ///   exploited.
    /// - **A leave sent before entry is EATEN.** The server only actually leaves if it
    ///   processes a `LeaveBazaar` while it considers itself in the bazaar; a leave
    ///   sent during the lead window is a no-op the server discards.
    /// - **Latency.** Acks arrive `ack_latency` ticks after we send; keyframes that
    ///   reconcile `local_baz` to the authoritative state arrive `kf_latency` ticks
    ///   after entry.
    ///
    /// Parameterized by the decision fn so the same harness can show the model has
    /// TEETH: the correct `decide` always escapes, a deliberately-buggy one freezes.
    fn run_bazaar_visit(
        decide_fn: impl Fn(&SyncState) -> BotAction,
        lead: u64,
        ack_latency: u64,
        kf_latency: u64,
    ) -> Result<(), String> {
        // ── Bot-maintained sync fields ──
        let mut seq: u64 = 3; // arbitrary nonzero start (as if mid-bout)
        let mut last_sent: u64 = seq;
        let mut bought = false;

        // ── World model ──
        let mut acked: u64 = seq;
        // Local prediction enters the bazaar at tick 0; the server enters at `lead`.
        let mut auth_baz = false;
        let mut local_baz = true;
        let mut server_entered = false;
        let mut server_left = false; // entered, then a leave actually took effect
        // A pending LeaveBazaar: (seq it rode, whether it was sent while the SERVER was
        // genuinely in the bazaar — only then is it effective rather than eaten).
        let mut pending_leave: Option<(u64, bool)> = None;
        let mut last_send_tick: u64 = 0;

        const MAX_TICKS: u64 = 5_000;
        for tick in 0..MAX_TICKS {
            // ── World advances first ──
            // The server authoritatively enters the bazaar at `lead`.
            if tick >= lead && !server_entered {
                auth_baz = true;
                server_entered = true;
            }
            // Acks reach us after `ack_latency` ticks.
            if tick >= last_send_tick + ack_latency {
                acked = last_sent;
            }
            // The server leaves ONLY when it acks a leave that was sent while it was in
            // the bazaar. A leave sent during the lead window is eaten (no effect).
            if let Some((ls, effective)) = pending_leave {
                if acked >= ls && effective && auth_baz {
                    auth_baz = false;
                    server_left = true;
                }
            }
            // Keyframes reconcile our local sim to the authoritative bazaar state after
            // `kf_latency` ticks (the server streams them even while frozen). Before the
            // server enters, our predicted `local_baz` only changes via our own leave.
            if server_entered && tick >= lead + kf_latency {
                local_baz = auth_baz;
            }
            // The snapshot handler re-arms `bought` when the server confirms we're out.
            if !auth_baz {
                bought = false;
            }

            // ── Bot decides + acts ──
            let action = decide_fn(&SyncState {
                done: false,
                idle_timed_out: false,
                acked,
                last_sent,
                auth_baz,
                opp_baz: false,
                local_baz,
                bought,
            });
            match action {
                // A genuine escape: the server completed its bazaar visit (entered AND
                // left) and we're back to clean play. A `Play` BEFORE the server has
                // entered is just normal pre-bazaar placement — keep simulating.
                BotAction::Play if server_entered && server_left => return Ok(()),
                BotAction::Play => { /* pre-entry play, or a buggy play mid-barrier */ }
                BotAction::Shop => {
                    // THE hazard guard: leaving while the server isn't authoritatively
                    // in the bazaar means the leave is eaten before entry — exactly the
                    // original freeze's seed. The correct policy (P2) never does this.
                    if !auth_baz {
                        return Err(format!(
                            "HAZARD at tick {tick}: leave sent before the server entered \
                             the bazaar (auth_baz=false) — it will be eaten"
                        ));
                    }
                    // Buy + leave: 4 inputs. Applying the leave locally drops our
                    // predicted `local_baz`; effective because `auth_baz` holds here.
                    seq += 4;
                    last_sent = seq;
                    pending_leave = Some((seq, true));
                    bought = true;
                    local_baz = false;
                    last_send_tick = tick;
                }
                BotAction::WaitAck | BotAction::WaitBazaar => { /* hold */ }
                BotAction::End => return Err(format!("unexpected End at tick {tick}")),
            }
        }
        Err(format!(
            "FREEZE: server's bazaar visit never resolved within {MAX_TICKS} ticks \
             (entered={server_entered}, left={server_left}, \
             lead={lead}, ack_latency={ack_latency}, kf_latency={kf_latency})"
        ))
    }

    /// The buggy policy the original freeze came from: leave the bazaar on the LOCAL
    /// prediction (no `auth_baz` requirement), so a leave can be sent before the server
    /// enters and get eaten. Used only to prove the liveness harness has teeth.
    fn decide_buggy_local_leave(s: &SyncState) -> BotAction {
        if s.done || s.idle_timed_out {
            BotAction::End
        } else if s.acked < s.last_sent {
            BotAction::WaitAck
        } else if s.local_baz && !s.bought {
            BotAction::Shop // ← no `auth_baz` guard: the bug
        } else if s.auth_baz || s.opp_baz || s.local_baz {
            BotAction::WaitBazaar
        } else {
            BotAction::Play
        }
    }

    proptest! {
        #[test]
        fn p5_always_escapes_the_bazaar(
            lead in 0u64..120,
            ack_latency in 0u64..200,
            kf_latency in 0u64..200,
        ) {
            let r = run_bazaar_visit(decide, lead, ack_latency, kf_latency);
            prop_assert!(r.is_ok(), "{}", r.unwrap_err());
        }
    }

    /// Teeth check: the buggy local-leave policy trips the harness's hazard guard
    /// (it leaves on local prediction during the lead window, before the server has
    /// entered — the eaten-leave seed of the original freeze) whenever local leads the
    /// server (`lead > 0`). This proves p5 above is a real constraint, not a vacuous
    /// one. (With `lead == 0` there's no pre-entry window, so even the buggy policy is
    /// safe; we require a lead here.)
    #[test]
    fn p5_harness_has_teeth_buggy_policy_trips_hazard() {
        let r = run_bazaar_visit(decide_buggy_local_leave, 8, 4, 4);
        assert!(
            r.is_err(),
            "buggy local-leave policy slipped past the hazard guard — the model has no teeth"
        );
    }

    /// The cross-bout baseline that bit during development: a fresh bout starts with
    /// the connection's seq already high (it's monotonic across the whole connection)
    /// while the server's per-bout `ack` resets to 0. Gating on a PER-BOUT `last_sent`
    /// (reset to 0 with each `MatchState`) — not the raw `seq` — is what avoids a
    /// permanent `WaitAck` deadlock here. Pin that: at bout start the bot must be free
    /// to act (not stuck waiting for an ack that can never arrive).
    #[test]
    fn fresh_bout_with_high_seq_is_not_deadlocked() {
        let s = SyncState {
            done: false,
            idle_timed_out: false,
            acked: 0,        // server's per-bout ack starts at 0
            last_sent: 0,    // ...and so does our per-bout last_sent (NOT the raw seq)
            auth_baz: false,
            opp_baz: false,
            local_baz: false,
            bought: false,
        };
        assert_eq!(decide(&s), BotAction::Play, "fresh bout must be free to act");
    }
}
