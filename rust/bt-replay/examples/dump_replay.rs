//! Diagnostic tool: replay a stored versus `VersusReplay` and either dump both
//! boards at a tick, or scan every tick for an engine invariant violation. Built
//! while chasing the "piece rests in mid-air" bug (replay 75037e...) — kept as a
//! reusable debugger. See CLAUDE.md ("Debugging replays").
//!
//! Get a stored replay's JSON:
//!   curl https://battletris.fly.dev/api/replays/<id> -o /tmp/r.json
//!
//! Dump both boards at a tick (# = locked, O = falling piece):
//!   cargo run -p bt-replay --example dump_replay -- /tmp/r.json 231
//!
//! Scan ALL ticks for a position desync (game pos vs piece pos — the class of
//! bug that lets a piece lock floating):
//!   cargo run -p bt-replay --example dump_replay -- /tmp/r.json

use bt_replay::{VersusReplay, VersusReplayPlayer};

fn dump(pl: &VersusReplayPlayer, side_a: bool, label: &str, tick: u32) {
    let g = pl.game(side_a);
    let b = g.board();
    let (w, h) = (b.width, b.height);
    let mut rows: Vec<Vec<char>> = (0..h).map(|_| vec!['.'; w as usize]).collect();
    for y in 0..h {
        for x in 0..w {
            if b.get(x, y).is_some() {
                rows[y as usize][x as usize] = '#';
            }
        }
    }
    let (px, py) = g.piece_pos();
    if let Some(pc) = g.current_piece() {
        for i in 0..pc.cells.len() {
            for j in 0..pc.cells[i].len() {
                if pc.cells[i][j].is_some() {
                    let (gx, gy) = (pc.x + i as i32, pc.y + j as i32);
                    if gx >= 0 && gx < w && gy >= 0 && gy < h {
                        rows[gy as usize][gx as usize] = 'O';
                    }
                }
            }
        }
        let sync = if (px, py) == (pc.x, pc.y) { "in sync" } else { "*** DESYNC ***" };
        println!(
            "--- side {label} tick {tick} piece={:?} game=({px},{py}) piece=({},{}) [{sync}] ---",
            pc.kind, pc.x, pc.y
        );
    } else {
        println!("--- side {label} tick {tick} (no falling piece) ---");
    }
    // Print only the lower half (the stack) to keep it compact.
    for y in (h / 2)..h {
        println!("{:2} {}", y, rows[y as usize].iter().collect::<String>());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = match args.get(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: dump_replay <replay.json> [tick]");
            std::process::exit(2);
        }
    };
    let json = std::fs::read_to_string(path).expect("read replay file");
    let replay = VersusReplay::from_json(&json).expect("parse VersusReplay JSON");
    let total = replay.tick_count;

    if let Some(tick) = args.get(2).and_then(|s| s.parse::<u32>().ok()) {
        let mut pl = VersusReplayPlayer::new(replay);
        pl.seek(tick);
        dump(&pl, true, "A", tick);
        dump(&pl, false, "B", tick);
        return;
    }

    // Scan every tick for a position desync (the game's collision/lock position
    // must equal the piece's own render/land position at all times).
    let mut pl = VersusReplayPlayer::new(replay);
    let mut found = 0;
    for t in 0..=total {
        pl.seek(t); // monotonic forward -> O(total) overall
        for (label, side_a) in [("A", true), ("B", false)] {
            let g = pl.game(side_a);
            if let Some(pc) = g.current_piece() {
                let (px, py) = g.piece_pos();
                if (px, py) != (pc.x, pc.y) {
                    println!(
                        "DESYNC side {label} tick {t}: game=({px},{py}) piece=({},{}) kind={:?}",
                        pc.x, pc.y, pc.kind
                    );
                    found += 1;
                }
            }
        }
    }
    if found == 0 {
        println!("clean: no position desync across {total} ticks");
    } else {
        println!("{found} desync frame(s) found");
    }
}
