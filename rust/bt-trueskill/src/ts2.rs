//! TrueSkill 2 additions that apply to a 1v1, single-mode game.
//!
//! From Minka, Cleven, Zaykov (2018). Squad-offset and mode-correlation do not
//! apply here (1v1, one mode). The three additions that do apply:
//!
//!   * Individual statistics (eq 9). Kill/death counts correlate with
//!     performance; the BattleTris analogue is lines cleared. The paper
//!     infers this via EP over a factor graph (no reference code released). For
//!     1v1 we implement the EP-consistent special case: treat the line margin
//!     `z = winner_lines - loser_lines` as a Gaussian measurement of the latent
//!     performance margin `d = perf_w - perf_l ~ N(mu_w - mu_l, sw2+sl2+2β²)`,
//!     `z | d ~ N(λ d, R)`. We do the Gaussian measurement update first, then
//!     impose the win condition `d > ε`. This updates both mean and variance
//!     and reduces to the classic update as `R → ∞` (or `λ = 0`).
//!     (Closed form courtesy of a second-opinion derivation.)
//!   * Experience offset (eq 8). A small, outcome-independent upward `mu`
//!     bump each match that decays with experience: `bump * exp(-n / k)`.
//!     Set `experience_bump = 0` to disable. The paper's 200-parameter table
//!     needs lots of data to learn; this crate uses a simpler shape instead.
//!   * Quit penalty (eq 12-13). A quit is a surrender (loss); the quitter
//!     also gets a small extra under-performance nudge.
//!
//! Caveat (per the paper & review): raw line counts partly measure match
//! duration, so `λ`/`R` here are uncalibrated defaults. Tune against real
//! data, or prefer a duration-normalized line margin.

use crate::math::{v_win, w_win};
use crate::{quality_1v1, rate_1v1_draw, Params, Rating};

/// Who won the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winner {
    /// Player A won outright.
    A,
    /// Player B won outright.
    B,
    /// Neither side won. This covers a tie, or the case where both players quit (resolved before rating).
    Draw,
}

/// The observable result of a match: who won, plus the side signals (lines,
/// quits) that the TS2 model folds into the update.
#[derive(Clone, Copy, Debug)]
pub struct MatchOutcome {
    /// The reported winner. A quit overrides this when the match is rated.
    pub winner: Winner,
    /// Lines cleared by A and B. This is the TS2 individual-statistic signal. A wide
    /// line margin is extra evidence of a skill gap beyond the bare win bit.
    pub a_lines: u32,
    pub b_lines: u32,
    /// Whether each player quit / disconnected. A one-sided quit makes that
    /// player lose (if both quit it's a draw), and any quitter takes an extra
    /// rating penalty on top.
    pub a_quit: bool,
    pub b_quit: bool,
}

impl MatchOutcome {
    /// An A win with the given line counts and no quits, the common case.
    pub fn a_wins(a_lines: u32, b_lines: u32) -> Self {
        MatchOutcome { winner: Winner::A, a_lines, b_lines, a_quit: false, b_quit: false }
    }
    /// A B win with the given line counts and no quits.
    pub fn b_wins(a_lines: u32, b_lines: u32) -> Self {
        MatchOutcome { winner: Winner::B, a_lines, b_lines, a_quit: false, b_quit: false }
    }
}

/// A player's persistent rating plus their match experience.
///
/// Experience is carried alongside the rating because the TS2 experience offset
/// (eq 8) gives newer players a small upward nudge that decays with games
/// played, so the count must persist between matches alongside the rating.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerState {
    /// The Gaussian skill belief.
    pub rating: Rating,
    /// Number of rated matches played. This drives the decaying experience offset.
    pub experience: u32,
}

impl PlayerState {
    /// A zero-experience state around `rating`. Pass [`Params::new_rating`] for
    /// a brand-new player at the prior.
    pub fn new(rating: Rating) -> Self {
        PlayerState { rating, experience: 0 }
    }
}

/// TrueSkill 2 parameters: classic [`Params`] plus the TS2 knobs.
#[derive(Clone, Copy, Debug)]
pub struct Ts2Params {
    pub base: Params,
    /// Coupling between performance margin and line margin (`λ`). 0 disables the
    /// lines signal (pure classic TrueSkill).
    pub perf_lambda: f64,
    /// Line-margin observation noise variance (`R`); larger = weaker influence.
    pub lines_obs_var: f64,
    /// Max upward `mu` nudge for a new player after a match (eq 8). 0 disables.
    pub experience_bump: f64,
    /// Experience decay constant `k` in `bump * exp(-n/k)`.
    pub experience_k: f64,
    /// Extra `mu` penalty for a quitter, in units of their post-update sigma.
    pub quit_penalty: f64,
}

impl Default for Ts2Params {
    /// Classic defaults plus deliberately conservative TS2 knobs. The lines/quit/
    /// experience weights are uncalibrated (see the module caveat), so they are
    /// set low enough to nudge rather than dominate the classic update.
    fn default() -> Self {
        Ts2Params {
            base: Params::default(),
            // Conservative, uncalibrated defaults (see module caveat).
            perf_lambda: 0.5,
            lines_obs_var: 256.0,
            experience_bump: 0.30,
            experience_k: 30.0,
            quit_penalty: 0.20,
        }
    }
}

impl Ts2Params {
    /// The experience offset for a player with `experience` matches (eq 8):
    /// `bump * exp(-experience / k)`.
    ///
    /// A small upward `mu` nudge that is largest for a brand-new player and
    /// decays toward zero with games played, so newcomers (who tend to be
    /// underrated early) climb a little faster without permanently inflating
    /// anyone. `experience_k <= 0` disables it.
    pub fn experience_offset(&self, experience: u32) -> f64 {
        if self.experience_k <= 0.0 {
            return 0.0;
        }
        self.experience_bump * (-(experience as f64) / self.experience_k).exp()
    }
}

/// Effective winner once quits are accounted for: a quit is a surrender (loss),
/// so it overrides whatever `winner` was reported. If both quit, it's a draw.
/// This is what stops a player from dodging a rating hit by disconnecting.
fn effective_winner(o: &MatchOutcome) -> Winner {
    match (o.a_quit, o.b_quit) {
        (true, true) => Winner::Draw,
        (true, false) => Winner::B,
        (false, true) => Winner::A,
        (false, false) => o.winner,
    }
}

/// EP-consistent 1v1 update for a decisive result, folding in the line margin
/// `z = winner_lines - loser_lines` as a Gaussian measurement of the latent
/// performance margin before imposing the win condition. Returns
/// `(new_winner, new_loser)`. With `perf_lambda == 0` this equals [`rate_1v1`].
fn rate_decisive(winner: Rating, loser: Rating, z: f64, p: &Ts2Params) -> (Rating, Rating) {
    let tau2 = p.base.tau * p.base.tau;
    let sw2 = winner.variance() + tau2;
    let sl2 = loser.variance() + tau2;
    let two_beta2 = 2.0 * p.base.beta * p.base.beta;

    let cc = sw2 + sl2 + two_beta2; // C = Var(d)
    let mu_d = winner.mu - loser.mu;
    let eps = p.base.draw_margin();

    // k = S h = [sw2, -sl2]
    let kw = sw2;
    let kl = -sl2;

    let lambda = p.perf_lambda;
    let r = p.lines_obs_var.max(1e-9);

    // --- Gaussian measurement update from the line margin ---
    let d_den = lambda * lambda * cc + r; // D
    let g = lambda * (z - lambda * mu_d) / d_den; // common scalar
    let mu_z = mu_d + cc * g;
    let mw_z = winner.mu + kw * g;
    let ml_z = loser.mu + kl * g;
    let cc_z = cc * r / d_den;
    let sw_z = sw2 - kw * kw * lambda * lambda / d_den;
    let sl_z = sl2 - kl * kl * lambda * lambda / d_den;
    let kw_z = kw * r / d_den;
    let kl_z = kl * r / d_den;

    // --- impose the win condition d > eps ---
    let sc = cc_z.sqrt();
    let t = mu_z / sc;
    let e = eps / sc;
    let v = v_win(t, e);
    let w = w_win(t, e);

    let mw_new = mw_z + (kw_z / sc) * v;
    let ml_new = ml_z + (kl_z / sc) * v;
    let sw_new = (sw_z - (kw_z * kw_z / cc_z) * w).max(1e-9);
    let sl_new = (sl_z - (kl_z * kl_z / cc_z) * w).max(1e-9);

    (
        Rating::new(mw_new, sw_new.sqrt()),
        Rating::new(ml_new, sl_new.sqrt()),
    )
}

/// Rate a 1v1 match with the TS2 model. Applies, in
/// order: the rating update (a decisive result folds in the line-margin signal;
/// a draw uses the classic draw update and ignores lines), then the quit penalty
/// and the experience offset; returns the updated states with `experience`
/// incremented.
pub fn rate_match(
    a: PlayerState,
    b: PlayerState,
    o: &MatchOutcome,
    p: &Ts2Params,
) -> (PlayerState, PlayerState) {
    let winner = effective_winner(o);

    let (mut ra, mut rb) = match winner {
        Winner::A => {
            let z = o.a_lines as f64 - o.b_lines as f64;
            rate_decisive(a.rating, b.rating, z, p)
        }
        Winner::B => {
            // rate_decisive takes (winner, loser); call it B-first and the line
            // margin from B's view, then swap the pair back to (A, B) order.
            let z = o.b_lines as f64 - o.a_lines as f64;
            let (w, l) = rate_decisive(b.rating, a.rating, z, p);
            (l, w)
        }
        Winner::Draw => rate_1v1_draw(a.rating, b.rating, &p.base),
    };

    // Quit penalty (eq 12-13): a small extra under-performance nudge.
    if o.a_quit {
        ra.mu -= p.quit_penalty * ra.sigma;
    }
    if o.b_quit {
        rb.mu -= p.quit_penalty * rb.sigma;
    }

    // Experience offset (eq 8): small upward bump, larger for newer players.
    ra.mu += p.experience_offset(a.experience);
    rb.mu += p.experience_offset(b.experience);

    (
        PlayerState { rating: ra, experience: a.experience + 1 },
        PlayerState { rating: rb, experience: b.experience + 1 },
    )
}

/// Match quality (a `[0, 1]` balance score) for matchmaking two players. This is the
/// TS2 wrapper over [`quality_1v1`] that takes [`PlayerState`]s. Higher means a
/// more balanced, more interesting pairing.
pub fn match_quality(a: &PlayerState, b: &PlayerState, p: &Ts2Params) -> f64 {
    quality_1v1(a.rating, b.rating, &p.base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_1v1;

    /// With the lines signal disabled (`perf_lambda = 0`) and no experience/quit
    /// adjustments, `rate_match` must equal the classic `rate_1v1`.
    #[test]
    fn reduces_to_classic_when_lines_disabled() {
        let p = Ts2Params { perf_lambda: 0.0, experience_bump: 0.0, quit_penalty: 0.0, ..Default::default() };

        let r = p.base.new_rating();
        let a = PlayerState::new(r);
        let b = PlayerState::new(r);
        let (na, nb) = rate_match(a, b, &MatchOutcome::a_wins(7, 3), &p);
        let (cw, cl) = rate_1v1(r, r, &p.base);

        assert!((na.rating.mu - cw.mu).abs() < 1e-9);
        assert!((na.rating.sigma - cw.sigma).abs() < 1e-9);
        assert!((nb.rating.mu - cl.mu).abs() < 1e-9);
        assert!((nb.rating.sigma - cl.sigma).abs() < 1e-9);
    }

    #[test]
    fn quit_is_treated_as_a_loss() {
        let p = Ts2Params::default();
        let a = PlayerState::new(p.base.new_rating());
        let b = PlayerState::new(p.base.new_rating());
        let o = MatchOutcome { winner: Winner::A, a_lines: 5, b_lines: 0, a_quit: true, b_quit: false };
        let (na, nb) = rate_match(a, b, &o, &p);
        assert!(na.rating.mu < a.rating.mu, "quitter loses rating");
        assert!(nb.rating.mu > b.rating.mu, "the player who stayed gains");
    }

    #[test]
    fn decisive_win_moves_mu_more() {
        let p = Ts2Params::default();
        let a = PlayerState::new(p.base.new_rating());
        let b = PlayerState::new(p.base.new_rating());

        let narrow = rate_match(a, b, &MatchOutcome::a_wins(11, 10), &p).0;
        let blowout = rate_match(a, b, &MatchOutcome::a_wins(40, 0), &p).0;
        assert!(
            blowout.rating.mu > narrow.rating.mu,
            "a blowout should raise mu more than a squeaker"
        );
        // NB: we deliberately do NOT assert a sigma ordering here. The line
        // measurement's variance reduction is margin-independent, while the
        // binary win-bit contributes *less* variance reduction when the outcome
        // is unsurprising (a blowout), so sigma ordering is model-subtle.
    }

    #[test]
    fn experience_offset_decays_monotonically_to_zero() {
        let p = Ts2Params::default();
        assert!((p.experience_offset(0) - p.experience_bump).abs() < 1e-12);
        assert!(p.experience_offset(0) > p.experience_offset(30));
        assert!(p.experience_offset(30) > p.experience_offset(120));
        assert!(p.experience_offset(100_000) < 1e-9);
    }

    #[test]
    fn experience_is_incremented() {
        let p = Ts2Params::default();
        let a = PlayerState::new(p.base.new_rating());
        let b = PlayerState::new(p.base.new_rating());
        let (na, nb) = rate_match(a, b, &MatchOutcome::a_wins(10, 5), &p);
        assert_eq!(na.experience, 1);
        assert_eq!(nb.experience, 1);
    }

    #[test]
    fn stronger_player_ranks_higher_over_many_games() {
        let p = Ts2Params::default();
        let mut a = PlayerState::new(p.base.new_rating());
        let mut b = PlayerState::new(p.base.new_rating());
        let pattern = [true, true, true, true, false, true, true, true, true, false];
        for round in 0..50 {
            let o = if pattern[round % pattern.len()] {
                MatchOutcome::a_wins(20, 12)
            } else {
                MatchOutcome::b_wins(12, 20)
            };
            let (na, nb) = rate_match(a, b, &o, &p);
            a = na;
            b = nb;
        }
        assert!(
            a.rating.conservative(3.0) > b.rating.conservative(3.0),
            "stronger player should rank higher: a={:?} b={:?}",
            a.rating,
            b.rating
        );
    }
}
