//! Property-based tests for bt-trueskill.
//!
//! Properties are checked over random but sane rating pairs (mu in 0..50,
//! sigma in 0.1..15) using ~256 cases each.  Both the classic `rate_1v1` and
//! the full TS2 `rate_match` path are exercised.

use bt_trueskill::{
    ts2::{MatchOutcome, PlayerState, Ts2Params},
    Params, Rating,
};
use proptest::prelude::*;

/// Strategy that produces a sane Rating: mu in [0, 50], sigma in [0.1, 15].
fn sane_rating() -> impl Strategy<Value = Rating> {
    (0.0f64..50.0, 0.1f64..15.0).prop_map(|(mu, sigma)| Rating::new(mu, sigma))
}

/// Strategy for "competitive" ratings used in mu-monotonicity tests:
/// mu gap capped at 10 (outcomes are non-trivially informative) and
/// sigma >= 1.0 (so the dynamics tau inflation is small relative to the EP
/// mean shift).
///
/// Background: TrueSkill's dynamics factor adds tau^2 to variance before
/// the EP update, keeping sigma from collapsing to zero over many games.  When
/// sigma is tiny OR the mu gap is huge (the match is a near-certainty),
/// the EP mean-shift v_win → 0 and the tau inflation dominates, making
/// mu - 3*sigma decrease even for the winner.  This is intentional model
/// behaviour, not a bug.  The mu-monotonicity property is only claimed in the
/// "informative match" regime: similar skill, non-degenerate sigma.
fn competitive_rating_pair() -> impl Strategy<Value = (Rating, Rating)> {
    // Generate a base mu and a gap in [0, 10], then two sigmas in [1, 10].
    (0.0f64..40.0, 0.0f64..10.0, 1.0f64..10.0, 1.0f64..10.0).prop_map(
        |(base, gap, sw, sl)| (Rating::new(base + gap, sw), Rating::new(base, sl)),
    )
}

/// Strategy for line counts used in TS2 outcomes (0..=40).
fn line_count() -> impl Strategy<Value = u32> {
    0u32..=40u32
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // -----------------------------------------------------------------------
    // (a) Monotonicity: after rate_1v1 in the competitive regime (mu gap <= 10,
    //     sigma >= 1), the winner's mean mu does NOT decrease and the loser's
    //     does NOT increase.
    //
    //     We check mu (not conservative mu - 3*sigma) because conservative
    //     skill can decrease even for the winner when sigma is small or the gap
    //     is large: tau inflation always adds variance, and when the match is
    //     a near-certainty the EP mean-shift is tiny.  mu itself is always
    //     updated in the expected direction by the EP equations.
    // -----------------------------------------------------------------------
    #[test]
    fn winner_mu_does_not_decrease(
        (w, l) in competitive_rating_pair(),
    ) {
        let p = Params::default();
        let (nw, nl) = bt_trueskill::rate_1v1(w, l, &p);

        prop_assert!(
            nw.mu >= w.mu - 1e-9,
            "winner mu decreased: before={} after={}",
            w.mu, nw.mu
        );
        prop_assert!(
            nl.mu <= l.mu + 1e-9,
            "loser mu increased: before={} after={}",
            l.mu, nl.mu
        );
    }

    // -----------------------------------------------------------------------
    // (b) Sigma health: stays > 0 and is finite (no NaN/inf/explosion) for
    //     both players after rate_1v1.
    // -----------------------------------------------------------------------
    #[test]
    fn sigma_stays_positive_and_finite_classic(
        w in sane_rating(),
        l in sane_rating(),
    ) {
        let p = Params::default();
        let (nw, nl) = bt_trueskill::rate_1v1(w, l, &p);

        prop_assert!(nw.sigma > 0.0, "winner sigma <= 0: {}", nw.sigma);
        prop_assert!(nw.sigma.is_finite(), "winner sigma is not finite: {}", nw.sigma);
        prop_assert!(nl.sigma > 0.0, "loser sigma <= 0: {}", nl.sigma);
        prop_assert!(nl.sigma.is_finite(), "loser sigma is not finite: {}", nl.sigma);
        prop_assert!(nw.mu.is_finite(), "winner mu is not finite: {}", nw.mu);
        prop_assert!(nl.mu.is_finite(), "loser mu is not finite: {}", nl.mu);
    }

    // -----------------------------------------------------------------------
    // (c) No panics: rate_1v1 never panics for any sane input.
    //     (proptest itself catches panics and reports them as failures)
    // -----------------------------------------------------------------------
    #[test]
    fn rate_1v1_never_panics(w in sane_rating(), l in sane_rating()) {
        let p = Params::default();
        let _ = bt_trueskill::rate_1v1(w, l, &p);
    }

    // -----------------------------------------------------------------------
    // TS2 rate_match — same three properties over the full TS2 path.
    // Winner is always A in these checks so we can reason clearly about
    // direction; a separate test checks B-wins symmetry.
    // -----------------------------------------------------------------------

    /// (a-ts2) Monotonicity: in the competitive regime (mu gap <= 10, sigma >= 1),
    ///         with the lines signal disabled (perf_lambda = 0), the winner's mu
    ///         does NOT decrease and the loser's does NOT increase.
    ///
    ///         When the lines signal is active, the TS2 model can intentionally
    ///         lower the winner's mu if the winner cleared far fewer lines than
    ///         the loser (contradictory evidence about underlying performance).
    ///         That is correct TS2 behaviour, not a bug — so we disable it here.
    #[test]
    fn ts2_winner_mu_does_not_decrease_no_lines(
        (w, l) in competitive_rating_pair(),
        w_lines in line_count(),
        l_lines in line_count(),
    ) {
        // disable lines signal (perf_lambda) — pure win/loss
        let p = Ts2Params { experience_bump: 0.0, quit_penalty: 0.0, perf_lambda: 0.0, ..Default::default() };

        let a = PlayerState::new(w);
        let b = PlayerState::new(l);

        let (na, nb) = bt_trueskill::ts2::rate_match(a, b, &MatchOutcome::a_wins(w_lines, l_lines), &p);

        prop_assert!(
            na.rating.mu >= w.mu - 1e-9,
            "TS2 winner mu decreased: before={} after={}",
            w.mu, na.rating.mu
        );
        prop_assert!(
            nb.rating.mu <= l.mu + 1e-9,
            "TS2 loser mu increased: before={} after={}",
            l.mu, nb.rating.mu
        );
    }

    /// (b-ts2) Sigma stays positive and finite through rate_match.
    #[test]
    fn ts2_sigma_stays_positive_and_finite(
        a_r in sane_rating(),
        b_r in sane_rating(),
        a_lines in line_count(),
        b_lines in line_count(),
    ) {
        let p = Ts2Params::default();
        let a = PlayerState::new(a_r);
        let b = PlayerState::new(b_r);
        let (na, nb) = bt_trueskill::ts2::rate_match(a, b, &MatchOutcome::a_wins(a_lines, b_lines), &p);

        prop_assert!(na.rating.sigma > 0.0, "TS2 a sigma <= 0: {}", na.rating.sigma);
        prop_assert!(na.rating.sigma.is_finite(), "TS2 a sigma not finite: {}", na.rating.sigma);
        prop_assert!(nb.rating.sigma > 0.0, "TS2 b sigma <= 0: {}", nb.rating.sigma);
        prop_assert!(nb.rating.sigma.is_finite(), "TS2 b sigma not finite: {}", nb.rating.sigma);
        prop_assert!(na.rating.mu.is_finite(), "TS2 a mu not finite: {}", na.rating.mu);
        prop_assert!(nb.rating.mu.is_finite(), "TS2 b mu not finite: {}", nb.rating.mu);
    }

    /// (c-ts2) rate_match never panics for any sane input with any winner.
    #[test]
    fn ts2_rate_match_never_panics(
        a_r in sane_rating(),
        b_r in sane_rating(),
        a_lines in line_count(),
        b_lines in line_count(),
        // 0 = A wins, 1 = B wins
        winner in 0u8..2,
    ) {
        let p = Ts2Params::default();
        let a = PlayerState::new(a_r);
        let b = PlayerState::new(b_r);
        let outcome = if winner == 0 {
            MatchOutcome::a_wins(a_lines, b_lines)
        } else {
            MatchOutcome::b_wins(a_lines, b_lines)
        };
        let _ = bt_trueskill::ts2::rate_match(a, b, &outcome, &p);
    }

    /// draw variant also must not NaN/inf.
    #[test]
    fn rate_1v1_draw_finite(a in sane_rating(), b in sane_rating()) {
        let p = Params::default();
        let (na, nb) = bt_trueskill::rate_1v1_draw(a, b, &p);
        prop_assert!(na.sigma > 0.0 && na.sigma.is_finite());
        prop_assert!(nb.sigma > 0.0 && nb.sigma.is_finite());
        prop_assert!(na.mu.is_finite());
        prop_assert!(nb.mu.is_finite());
    }
}
