//! TrueSkill / TrueSkill 2 ratings for 1v1 BattleTris.
//!
//! Implemented directly from:
//!   * Herbrich, Minka, Graepel, "TrueSkill(TM): A Bayesian Skill Rating
//!     System" (2007). The closed-form 1v1 update used here is the
//!     Expectation-Propagation result for the two-player factor graph.
//!   * Minka, Cleven, Zaykov, "TrueSkill 2" (2018). The model additions that
//!     apply to a 1v1, single-mode game are: individual-performance statistics
//!     (here: lines cleared), an experience offset for new players, and a quit
//!     penalty. See [`ts2`].
//!
//! A rating is a Gaussian belief over latent skill: `mu` ± `sigma`. The
//! published TrueSkill 2 paper gives the generative model but no closed-form
//! updates (Microsoft inferred them via Infer.NET / EP); for 1v1 the win/loss
//! update is the classic closed form, onto which we add the TS2 factors.
//!
//! Default scale follows the classic TrueSkill defaults (`mu=25`,
//! `sigma=25/3`). The Halo-5 paper values are not used because this is a fresh
//! 1v1 game with no historical scale to anchor to.

pub mod math;
pub mod ts2;

pub use ts2::{MatchOutcome, Ts2Params};

/// A Gaussian skill belief: `N(mu, sigma^2)`.
///
/// Skill is never observed directly, only inferred from match outcomes. The belief
/// is carried as a full distribution: `mu` is the current best estimate and
/// `sigma` is the uncertainty. A match generally
/// moves `mu` toward the result and tightens `sigma` as evidence accumulates
/// (the dynamics term can nudge `sigma` back up between games; see [`Params::tau`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rating {
    /// Mean of the skill belief, the point estimate of the player's strength.
    pub mu: f64,
    /// Standard deviation of the belief, representing uncertainty. High for new players,
    /// generally shrinking as they are measured.
    pub sigma: f64,
}

impl Rating {
    /// A rating with an explicit mean and uncertainty. Most callers want
    /// [`Params::new_rating`] (the prior) instead of hand-picking these.
    pub fn new(mu: f64, sigma: f64) -> Rating {
        Rating { mu, sigma }
    }

    /// Variance `sigma^2`. The math works in variance space (additive,
    /// closed-form) and only takes the square root back to `sigma` at the end.
    #[inline]
    pub fn variance(&self) -> f64 {
        self.sigma * self.sigma
    }

    /// A conservative skill estimate for leaderboards / display: `mu - k*sigma`
    /// (TrueSkill convention `k = 3`).
    ///
    /// Ranking by `mu` alone would let a lucky newcomer with one win and huge
    /// `sigma` outrank a proven veteran. Subtracting `k` standard deviations gives
    /// "skill we're confident the player has at least," so the leaderboard /
    /// rating display ranks by this value: a player must both perform and be
    /// well-measured (low `sigma`) to climb.
    pub fn conservative(&self, k: f64) -> f64 {
        self.mu - k * self.sigma
    }
}

/// Tunable model parameters (classic TrueSkill).
///
/// These set the shape of the rating system: where new players start, how much
/// a single game is allowed to move a rating, and how the model treats draws.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    /// Prior mean for a new player, the `mu` everyone starts at.
    pub mu0: f64,
    /// Prior standard deviation for a new player. Large by design so early games
    /// move a new player's `mu` quickly until the system has measured them.
    pub sigma0: f64,
    /// Performance standard deviation `beta`: how far a player's per-game
    /// showing can stray from their true skill (enters the math as `beta^2`).
    /// Larger `beta` means a single result is weaker evidence, so updates are
    /// gentler.
    pub beta: f64,
    /// Dynamics standard deviation `tau`. Its square `tau^2` is added to each
    /// player's variance before a match. Re-inflates `sigma` a touch each game so
    /// a long-settled rating never freezes solid and can still track a player who
    /// genuinely improves or rusts.
    pub tau: f64,
    /// Assumed probability of a draw, used to derive the draw margin `epsilon`
    /// (the performance gap below which a game counts as a tie).
    pub draw_probability: f64,
}

impl Default for Params {
    /// The classic TrueSkill defaults (`mu0 = 25`, `sigma0 = 25/3`, with
    /// `beta`/`tau` derived from `sigma0`), chosen because this is a fresh 1v1
    /// game with no historical scale to anchor to.
    fn default() -> Self {
        let sigma0 = 25.0 / 3.0;
        Params {
            mu0: 25.0,
            sigma0,
            beta: sigma0 / 2.0,
            tau: sigma0 / 100.0,
            draw_probability: 0.10,
        }
    }
}

impl Params {
    /// A fresh rating at the prior, for a never-seen player starting at the prior mean.
    pub fn new_rating(&self) -> Rating {
        Rating::new(self.mu0, self.sigma0)
    }

    /// Draw margin `epsilon` for a 1v1 (`n = 2` players):
    /// `epsilon = Phi^{-1}((p_draw + 1) / 2) * sqrt(2) * beta`.
    ///
    /// The margin by which one performance must exceed the other to count as a
    /// win rather than a draw; derived from [`draw_probability`](Self::draw_probability)
    /// so a single knob ("how often do players tie?") fixes it.
    pub fn draw_margin(&self) -> f64 {
        math::inv_cdf((self.draw_probability + 1.0) / 2.0) * std::f64::consts::SQRT_2 * self.beta
    }
}

/// Update two ratings after `winner` beats `loser` (1v1, no draw).
///
/// Returns `(new_winner, new_loser)`. The winner's `mu` rises and the loser's
/// falls, both by an amount scaled by how *surprising* the result was: beating a
/// favorite moves ratings far more than beating an underdog, because the latter
/// is what the model already expected. The result is also evidence that usually
/// tightens both beliefs, though the dynamics term can leave an already-certain
/// rating's `sigma` slightly higher. Closed-form EP update from Herbrich et al.
/// (2007).
pub fn rate_1v1(winner: Rating, loser: Rating, p: &Params) -> (Rating, Rating) {
    // Inflate variance by the dynamics factor first, so skill is allowed to have
    // drifted since the last game before we measure this one.
    let sw2 = winner.variance() + p.tau * p.tau;
    let sl2 = loser.variance() + p.tau * p.tau;

    // c normalizes the skill gap by the total uncertainty (both players' sigma
    // plus the per-game performance noise 2*beta^2); t and e are that gap and the
    // draw margin expressed on the standard-normal scale the v/w factors expect.
    let c = (2.0 * p.beta * p.beta + sw2 + sl2).sqrt();
    let eps = p.draw_margin();

    let t = (winner.mu - loser.mu) / c;
    let e = eps / c;

    // v and w are the truncated-Gaussian corrections: v drives the mean shift,
    // w the variance shrink. They encode the "surprise"; the correction is large for an upset.
    let v = math::v_win(t, e);
    let w = math::w_win(t, e);

    let new_w = update(winner.mu, sw2, c, v, w, 1.0);
    let new_l = update(loser.mu, sl2, c, v, w, -1.0);
    (new_w, new_l)
}

/// Update two ratings after a 1v1 draw. Order of `a`/`b` does not matter.
///
/// A draw is itself evidence that the two players are close, so it pulls the means
/// gently toward each other (no movement when they are already equal) and
/// usually tightens both beliefs, rather than leaving the ratings untouched (the
/// dynamics term can still raise an already-certain `sigma`).
pub fn rate_1v1_draw(a: Rating, b: Rating, p: &Params) -> (Rating, Rating) {
    let sa2 = a.variance() + p.tau * p.tau;
    let sb2 = b.variance() + p.tau * p.tau;

    let c = (2.0 * p.beta * p.beta + sa2 + sb2).sqrt();
    let eps = p.draw_margin();

    let t = (a.mu - b.mu) / c;
    let e = eps / c;

    let v = math::v_draw(t, e);
    let w = math::w_draw(t, e);

    let new_a = update(a.mu, sa2, c, v, w, 1.0);
    let new_b = update(b.mu, sb2, c, v, w, -1.0);
    (new_a, new_b)
}

/// Apply the per-player mean/variance update given the shared `v`/`w` factors.
///
/// Both players consume the same `v`/`w` (the result was one shared event); they
/// differ only in `sign` and in their own pre-game variance, so a more uncertain
/// player moves further on the same outcome. `sign` is +1 for the player favored
/// by `t` (winner / `a`), -1 otherwise.
fn update(mu: f64, var: f64, c: f64, v: f64, w: f64, sign: f64) -> Rating {
    let mean_mult = var / c;
    let var_mult = var / (c * c);
    let new_mu = mu + sign * mean_mult * v;
    let mut new_var = var * (1.0 - var_mult * w);
    if new_var < 1e-9 {
        new_var = 1e-9; // numerical floor; variance must stay strictly positive
    }
    Rating::new(new_mu, new_var.sqrt())
}

/// Match quality for matchmaking: a `[0, 1]` balance score derived from the
/// draw probability, higher meaning a more even contest. Closed form for 1v1.
///
/// The lobby prefers high-quality matchups so games are close. For a given pair
/// the score peaks when the two `mu`s coincide and falls as the gap widens; the
/// ceiling itself is below 1 unless both players are also well measured (e.g.
/// two fresh default ratings score ~0.45 because their `sigma` is large).
pub fn quality_1v1(a: Rating, b: Rating, p: &Params) -> f64 {
    let two_beta2 = 2.0 * p.beta * p.beta;
    let denom = two_beta2 + a.variance() + b.variance();
    let dmu = a.mu - b.mu;
    (two_beta2 / denom).sqrt() * (-(dmu * dmu) / (2.0 * denom)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "expected {a} ≈ {b} (tol {tol})");
    }

    #[test]
    fn default_win_matches_reference_values() {
        // Canonical trueskill-python `rate_1vs1` with default params.
        let p = Params::default();
        let r = p.new_rating();
        let (w, l) = rate_1v1(r, r, &p);
        approx(w.mu, 29.39583201999916, 1e-3);
        approx(w.sigma, 7.171475587326186, 1e-3);
        approx(l.mu, 20.604167980000835, 1e-3);
        approx(l.sigma, 7.171475587326186, 1e-3);
    }

    #[test]
    fn draw_keeps_means_but_shrinks_sigma() {
        let p = Params::default();
        let r = p.new_rating();
        let (a, b) = rate_1v1_draw(r, r, &p);
        // Equal players drawing: means unchanged, uncertainty drops.
        approx(a.mu, 25.0, 1e-6);
        approx(b.mu, 25.0, 1e-6);
        assert!(a.sigma < r.sigma);
        assert!(b.sigma < r.sigma);
    }

    #[test]
    fn quality_of_equal_default_players_is_about_0_447() {
        let p = Params::default();
        let r = p.new_rating();
        approx(quality_1v1(r, r, &p), 0.4472135955, 1e-4);
    }

    #[test]
    fn winning_raises_mu_losing_lowers_it_and_both_shrink() {
        let p = Params::default();
        let r = p.new_rating();
        let (w, l) = rate_1v1(r, r, &p);
        assert!(w.mu > r.mu);
        assert!(l.mu < r.mu);
        assert!(w.sigma < r.sigma && l.sigma < r.sigma);
    }

    #[test]
    fn beating_a_much_stronger_player_moves_mu_more() {
        let p = Params::default();
        let underdog = Rating::new(20.0, 8.0 / 3.0); // confident, lower skill
        let favorite = Rating::new(40.0, 8.0 / 3.0); // confident, higher skill
        // Expected outcome (favorite wins): small change.
        let (_w_exp, l_exp) = rate_1v1(favorite, underdog, &p);
        // Upset (underdog wins): large change.
        let (w_up, _l_up) = rate_1v1(underdog, favorite, &p);
        let expected_gain = (w_up.mu - underdog.mu).abs();
        let normal_gain = (l_exp.mu - underdog.mu).abs();
        assert!(
            expected_gain > normal_gain,
            "an upset win should move the rating more than the expected loss"
        );
    }

    #[test]
    fn quality_drops_with_skill_gap() {
        let p = Params::default();
        let a = Rating::new(25.0, 25.0 / 3.0);
        let b = Rating::new(35.0, 25.0 / 3.0);
        assert!(quality_1v1(a, b, &p) < quality_1v1(a, a, &p));
    }
}
