//! Weapons layer 4 — watchable showcase replays.
//!
//! The deterministic engine + the replay format already carry everything needed
//! to *show* a weapon: `Input::ReceiveWeapon` records an incoming weapon, and
//! the (WASM/browser) `ReplayPlayer` applies it via `apply_to_game`. So a solo
//! "showcase" replay — stack a few pieces, take the weapon, watch it land — is
//! a real, scrubbable artifact you can open in the replay library, no recorder
//! change required.
//!
//! This file builds one showcase per weapon and proves it's a faithful,
//! serializable replay (the player reproduces the live board bit-for-bit, and
//! the JSON round-trips). The `#[ignore]` generator writes them to
//! `target/weapon-showcase/` so they can be uploaded to / imported by the
//! library for visual review — the only way to vet the purely-visual weapons
//! (Gimp, Twilight) and the "feel" ones (Hatter spin, Slick slide).

use bt_core::game::GameEvent;
use bt_core::weapons::{weapon_table, WeaponToken};
use bt_core::{Board, Game};
use bt_replay::{Input, Mode, Recorder, Replay, ReplayPlayer};

fn just_locked(evs: &[GameEvent]) -> bool {
    evs.iter().any(|e| matches!(e, GameEvent::Locked { .. }))
}

/// Board as a grid of render ids (`None` = empty). Captures hidden/gimp/etc.
fn grid(b: &Board) -> Vec<Vec<Option<i32>>> {
    (0..b.height)
        .map(|y| (0..b.width).map(|x| b.get(x, y).map(|c| c.id())).collect())
        .collect()
}

/// Drive a Game + Recorder in lockstep, so the recording exactly mirrors the
/// live run: stack a low staircase of pieces, deliver `token`, flush it, settle.
fn build_showcase(token: WeaponToken) -> (Replay, Vec<Vec<Option<i32>>>) {
    let seed: u32 = 12_345;
    let mut g = Game::new(seed as u64);
    let mut rec = Recorder::new(seed, Mode::Practice, None, 16, "weapon-showcase");
    rec.set_title(weapon_table()[token.index()].name);

    let mut drop_piece = |g: &mut Game, rec: &mut Recorder, lefts: u32| {
        for _ in 0..lefts {
            rec.record(Input::MoveLeft);
            g.move_left();
        }
        rec.record(Input::BeginDrop);
        g.begin_drop();
        for _ in 0..60 {
            g.tick(16);
            rec.on_tick();
            if g.is_game_over() || just_locked(&g.take_events()) {
                break;
            }
        }
    };

    // A spread-out staircase keeps the stack low (no early top-out) but gives
    // weapons that act on existing blocks (Gimp/Twilight/Missing/Blind) targets.
    for lefts in [0, 1, 2, 3, 4, 5] {
        drop_piece(&mut g, &mut rec, lefts);
    }

    // Deliver the weapon, then flush it with one more drop.
    rec.record(Input::ReceiveWeapon(token.index() as i32));
    g.receive_weapon(token);
    drop_piece(&mut g, &mut rec, 2);

    // Let the effect settle on screen (and let timed/auto weapons act).
    for _ in 0..40 {
        g.tick(16);
        rec.on_tick();
        let _ = g.take_events();
    }

    (rec.to_replay(), grid(g.board()))
}

/// Every weapon's showcase is a faithful replay: replaying it reproduces the
/// live board exactly, and its JSON round-trips. This is what makes the
/// showcases trustworthy artifacts to watch.
#[test]
fn every_weapon_showcase_replays_faithfully() {
    for token in WeaponToken::ALL {
        let (replay, live) = build_showcase(token);

        // Replaying reproduces the live board bit-for-bit (determinism).
        let mut player = ReplayPlayer::new(replay.clone());
        player.run_to_end();
        assert_eq!(
            grid(player.player().board()),
            live,
            "{token:?}: replay diverged from the live run"
        );

        // ...and it survives a JSON round-trip (library storage format).
        let parsed = Replay::from_json(&replay.to_json()).expect("valid JSON");
        assert_eq!(parsed, replay, "{token:?}: replay JSON did not round-trip");

        // The showcase actually contains the weapon delivery.
        assert!(
            replay
                .frames
                .iter()
                .any(|f| f.input == Input::ReceiveWeapon(token.index() as i32)),
            "{token:?}: showcase is missing its ReceiveWeapon frame"
        );
    }
}

/// Sanity that the showcases aren't blank: board-mutating weapons visibly change
/// the board versus the same timeline with a no-op delivery.
#[test]
fn board_mutating_showcases_show_a_visible_effect() {
    // RiseUp adds a row, Gimp/Twilight rewrite ids, Bottle walls the neck,
    // FlipOut mirrors — all must differ from a do-nothing baseline.
    let baseline = build_showcase(WeaponToken::Mirror).1; // Mirror is a no-op in the engine
    for token in [
        WeaponToken::RiseUp,
        WeaponToken::Gimp,
        WeaponToken::Twilight,
        WeaponToken::Bottle,
        WeaponToken::FlipOut,
    ] {
        let board = build_showcase(token).1;
        assert_ne!(board, baseline, "{token:?} showcase should visibly differ from a no-op");
    }
}

/// Generator: write every weapon's showcase to `target/weapon-showcase/` as
/// `NN-slug.json` for upload to / import by the replay library. Run with:
///   cargo test -p bt-replay --test weapon_showcase -- --ignored --nocapture
#[test]
#[ignore]
fn generate_showcase_replays() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("weapon-showcase");
    std::fs::create_dir_all(&dir).expect("create showcase dir");

    let table = weapon_table();
    for (i, token) in WeaponToken::ALL.iter().enumerate() {
        let (replay, _) = build_showcase(*token);
        let slug: String = table[token.index()]
            .name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
            .collect();
        let path = dir.join(format!("{i:02}-{}.json", slug.trim_matches('-')));
        std::fs::write(&path, replay.to_json()).expect("write showcase");
        println!("wrote {}", path.display());
    }
    println!("\n{} showcase replays in {}", WeaponToken::ALL.len(), dir.display());
}
