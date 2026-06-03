//! Per-weapon oracle — game-parameter / interface effects.
//!
//! These weapons don't (only) mutate the grid — they retune the *game*: drop
//! speed, control direction, lock latency, bazaar prices. A weapon is delivered
//! by `receive_weapon` and applied at the next piece lock (`flush_pending`), so
//! each test queues the weapon, drops a piece to flush it, then observes the
//! changed behavior through the public API.

use bt_core::game::GameEvent;
use bt_core::weapons::WeaponToken;
use bt_core::Game;

/// Hard-drop the current piece and tick until it locks (flushing any received
/// weapon via `flush_pending`).
fn lock_a_piece(g: &mut Game) {
    g.begin_drop();
    for _ in 0..600 {
        g.tick(16);
        if g.is_game_over() {
            return;
        }
        if g
            .take_events()
            .iter()
            .any(|e| matches!(e, GameEvent::Locked { .. }))
        {
            return;
        }
    }
    panic!("piece never locked");
}

/// Deliver `tok` and flush it at a lock; assert it became active.
fn activate(g: &mut Game, tok: WeaponToken) {
    g.receive_weapon(tok);
    lock_a_piece(g);
    assert!(g.board().active.is_active(tok), "{tok:?} should be active after the flush");
}

/// Rows the current piece falls under gravity over `total_ms`.
fn rows_in(g: &mut Game, total_ms: i32) -> i32 {
    let (_, y0) = g.piece_pos();
    let mut t = 0;
    while t < total_ms {
        g.tick(8);
        t += 8;
        if g.current_piece().is_none() {
            break;
        }
    }
    let (_, y1) = g.piece_pos();
    y1 - y0
}

/// Carter Years: every bazaar price doubles while active.
#[test]
fn carter_doubles_bazaar_prices() {
    let mut g = Game::new(1);
    let base = g.bazaar_price(WeaponToken::Speedy);

    activate(&mut g, WeaponToken::Carter);

    assert_eq!(
        g.bazaar_price(WeaponToken::Speedy),
        base * 2,
        "Carter doubles the displayed/charged price"
    );
}

/// Speedy Gonzales speeds gravity up; Meadow slows it down. Pin the ordering
/// (and the rough 2x) against an untouched baseline.
#[test]
fn speedy_and_meadow_change_fall_speed() {
    const WINDOW: i32 = 1024; // 2 baseline rows / 4 speedy / 1 meadow

    let base_rows = rows_in(&mut Game::new(2), WINDOW);

    let mut speedy = Game::new(2);
    activate(&mut speedy, WeaponToken::Speedy);
    let speedy_rows = rows_in(&mut speedy, WINDOW);

    let mut meadow = Game::new(2);
    activate(&mut meadow, WeaponToken::Meadow);
    let meadow_rows = rows_in(&mut meadow, WINDOW);

    assert!(base_rows >= 1, "sanity: pieces fall at the baseline ({base_rows} rows)");
    assert!(
        speedy_rows > base_rows,
        "Speedy must fall faster than baseline ({speedy_rows} vs {base_rows})"
    );
    assert!(
        meadow_rows < base_rows,
        "Meadow must fall slower than baseline ({meadow_rows} vs {base_rows})"
    );
}

/// Upbyside-down reverses the horizontal controls: "left" moves the piece right.
#[test]
fn upbyside_reverses_horizontal_controls() {
    let mut g = Game::new(3);
    activate(&mut g, WeaponToken::Upbyside);

    let (x0, _) = g.piece_pos();
    g.move_left();
    let (x1, _) = g.piece_pos();

    assert!(
        x1 > x0,
        "under Upbyside, move_left() shifts the piece right ({x0} -> {x1})"
    );
}

/// Slide Denied removes the lock-slide grace: a piece that lands under gravity
/// locks far sooner than the default `BT_SLIDE_TIME` window allows.
#[test]
fn no_slide_locks_without_the_slide_grace() {
    fn ticks_to_lock_after_landing(tok: Option<WeaponToken>) -> i32 {
        let mut g = Game::new(5);
        if let Some(t) = tok {
            activate(&mut g, t);
        }
        // Walk the piece down to the floor with soft drops (score-free, no
        // fast-drop engaged), until it can't descend any further.
        let mut last = g.piece_pos().1;
        for _ in 0..40 {
            g.soft_drop();
            let y = g.piece_pos().1;
            if y == last {
                break; // landed — sitting on the floor/stack
            }
            last = y;
        }
        // Now count ticks until it locks.
        for n in 0..40 {
            g.tick(16);
            if g
                .take_events()
                .iter()
                .any(|e| matches!(e, GameEvent::Locked { .. }))
            {
                return n + 1;
            }
        }
        i32::MAX
    }

    let default_ticks = ticks_to_lock_after_landing(None);
    let noslide_ticks = ticks_to_lock_after_landing(Some(WeaponToken::NoSlide));

    assert!(
        noslide_ticks < default_ticks,
        "NoSlide should lock sooner: {noslide_ticks} vs default {default_ticks}"
    );
}
