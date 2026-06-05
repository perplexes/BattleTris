//! bt-bot — a headless, networked BattleTris player.
//!
//! It speaks the EXACT same websocket protocol a browser does (see
//! `bt-server/src/main.rs`): it connects, announces itself "open to matches"
//! (tagged `bot:true` + a `geo` label), auto-accepts directed challenges, and
//! when a `matchStart` arrives it plays a real server-authoritative match —
//! driven by the same `bt-ai` placement search the vs-Computer Ernie uses.
//!
//! Why: it populates the lobby so a fresh visitor always has someone to play,
//! and — deployed per-fly-region over the private 6PN network — it exercises the
//! netcode under real cross-geo latency (Tokyo→sjc etc.). Two personas (see
//! [`Persona`]): aggressive **Bert** (the strong line-clearing eval + smart
//! weapons, timing board-raisers to when its spy reveals the opponent stacked high)
//! and easy-going **Ernie** (faithful placement, slower, no weapons).
//!
//! The bot keeps a LOCAL `bt-core::Game` seeded from `matchStart.seed` (the same
//! deterministic piece stream the server runs for this side) and reconciles it
//! to the authoritative `keyframe` bytes whenever one arrives — exactly the
//! prediction/reconciliation model the browser client uses. Hard-drop column
//! placement is robust to latency: the column is decided by the move/rotate
//! inputs we send (applied in order on both sides), not by where gravity has the
//! piece when the drop lands.

use std::time::Duration;

use bt_ai::{best_placement, best_placement_strong};
use bt_ai::weapons::{buy_plan, launch_choice};
use bt_core::weapons::WeaponToken;
use bt_core::Game;
use bt_replay::Input;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

/// The server's authoritative tick (`bout::TICK_MS`). We drive our local sim at
/// the same cadence.
const TICK_MS: i32 = 16;
/// Place ~one piece every this many ticks (~420ms): a steady, watchable pace
/// rather than dumping the whole piece queue at lightspeed (which would top the
/// bot out instantly and make for an unrealistic netcode test).
const PLACE_INTERVAL_TICKS: i32 = 26;
/// Tell the server we're "active" this often, so the bot counts toward the
/// lobby's players-online tally and the connection never idles out.
const ACTIVE_PING: Duration = Duration::from_secs(20);
/// If a match goes this many ticks (~2.4s) with no snapshot, treat it as over.
/// A natural top-out sends a final result snapshot, but an opponent who FORFEITS
/// by dropping their socket makes the server end the bout WITHOUT a last frame to
/// us — this timeout is how we notice and return to the lobby.
const STALE_TICKS: u32 = 150;
/// How long to wait before reconnecting after the socket drops.
const RECONNECT_DELAY: Duration = Duration::from_secs(3);
/// Try to launch a weapon about this often (~1.9s) — frequent enough to spend the
/// arsenal, spaced enough to be watchable and not waste duration weapons.
const LAUNCH_INTERVAL_TICKS: i32 = 120;
/// When `launch_choice` has nothing worth firing, re-check sooner than a full
/// interval (we may have just bought weapons or activated a spy).
const LAUNCH_RETRY_TICKS: i32 = 30;
/// A spy_board ride only on keyframes, so treat the spy (and its opponent-board
/// reveal) as "fresh" for this many ticks after each one; when it lapses without a
/// refresh, the spy has expired and we fall back to spy-less launching.
const SPY_FRESH_TICKS: u32 = 300;
/// The opponent is "high" (worth a board-raiser) once their stack fills at least
/// this fraction of the board height.
const OPP_HIGH_FRAC: f64 = 0.6;

type Out = mpsc::UnboundedSender<Message>;

/// A bot's difficulty + name. **Bert** is the aggressive adversary — the strong
/// line-clearing eval plus smart weapons, at a brisk pace. **Ernie** (the default)
/// is the easy-going one for new visitors: faithful Ernie's weaker placement, a
/// slower pace, and no weapons. (Bert & Ernie. Bert's the intense one.)
///
/// Selected by `BT_BOT_PERSONA`, else the fly process-group name
/// (`FLY_PROCESS_GROUP`), else easy-going Ernie.
#[derive(Clone, Copy)]
struct Persona {
    /// Name suffix, e.g. "Bert" → "Tokyo-Bert".
    suffix: &'static str,
    /// Use the strong line-clearing eval (vs faithful Ernie's weaker placement).
    strong: bool,
    /// Ticks between placements (lower = faster).
    place_ticks: i32,
    /// Buy + launch weapons.
    weapons: bool,
}

fn persona() -> Persona {
    let which = std::env::var("BT_BOT_PERSONA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("FLY_PROCESS_GROUP").ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match which.as_str() {
        "bert" | "strong" | "hard" => Persona {
            suffix: "Bert",
            strong: true,
            place_ticks: PLACE_INTERVAL_TICKS,
            weapons: true,
        },
        _ => Persona {
            suffix: "Ernie",
            strong: false,
            place_ticks: 44, // ~700ms — slower, more beatable
            weapons: false,
        },
    }
}

/// Per-match local state: our predicted board plus the placement pacer and the
/// bazaar gate.
struct MatchState {
    game: Game,
    /// This bot's difficulty/name persona.
    persona: Persona,
    /// Weapons launched this match (for the end-of-match summary log).
    launched: u32,
    /// Ticks lived this match, for a periodic progress log.
    live_ticks: u32,
    /// Ticks until the next placement (paces the bot to a human-ish speed).
    cooldown: i32,
    /// Authoritative "you are in the bazaar" from the latest snapshot — while
    /// set, play is frozen server-side, so we just leave the bazaar and wait.
    in_bazaar: bool,
    /// Did we already shop (buy + LeaveBazaar) for this bazaar visit?
    bazaar_left: bool,
    /// Ticks until the next weapon-launch attempt.
    launch_cooldown: i32,
    /// Ticks of remaining "spy intel" — refreshed whenever a `spy_board` arrives;
    /// while > 0 we have a live read on the opponent's board (`opp_high`).
    spy_fresh: u32,
    /// Latest read of "opponent stacked high" from a spy reveal (false when blind).
    opp_high: bool,
    /// Set when a snapshot reports a result (win/loss) — the match is over.
    done: bool,
    /// Ticks since the last snapshot; if it crosses [`STALE_TICKS`] the opponent
    /// has gone silent (forfeit/disconnect) and we end the match our side.
    idle_ticks: u32,
}

impl MatchState {
    fn new(seed: u64, persona: Persona) -> MatchState {
        MatchState {
            game: Game::new(seed),
            persona,
            launched: 0,
            live_ticks: 0,
            cooldown: persona.place_ticks,
            in_bazaar: false,
            bazaar_left: false,
            launch_cooldown: LAUNCH_INTERVAL_TICKS,
            spy_fresh: 0,
            opp_high: false,
            done: false,
            idle_ticks: 0,
        }
    }
}

/// Resolve this bot's display name + geo label. Explicit `BT_BOT_NAME` /
/// `BT_BOT_GEO` win; otherwise derive a friendly city from fly's `FLY_REGION`
/// (auto-set per machine), so one image deployed across regions yields one
/// distinctly-named bot per region.
fn identity(p: &Persona) -> (String, String) {
    let region = std::env::var("FLY_REGION").unwrap_or_default();
    let city = match region.as_str() {
        "nrt" => "Tokyo",
        "hkg" => "HongKong",
        "syd" => "Sydney",
        "sin" => "Singapore",
        "lhr" => "London",
        "fra" => "Frankfurt",
        "ams" => "Amsterdam",
        "cdg" => "Paris",
        "gru" => "SaoPaulo",
        "scl" => "Santiago",
        "jnb" => "Johannesburg",
        "bom" => "Mumbai",
        "iad" => "Virginia",
        "ord" => "Chicago",
        "sjc" | "lax" => "California",
        "" => "Local",
        other => other,
    };
    let name = std::env::var("BT_BOT_NAME").unwrap_or_else(|_| format!("{city}-{}", p.suffix));
    let geo = std::env::var("BT_BOT_GEO").unwrap_or_else(|_| city.to_string());
    (name, geo)
}

#[tokio::main]
async fn main() {
    let ws_url =
        std::env::var("BT_BOT_WS").unwrap_or_else(|_| "ws://127.0.0.1:8088/ws".to_string());
    let persona = persona();
    let (name, geo) = identity(&persona);
    println!(
        "bt-bot up: name={name:?} geo={geo:?} server={ws_url} (strong={}, weapons={})",
        persona.strong, persona.weapons
    );
    if std::env::var("BT_JWT_SECRET").map(|s| s.is_empty()).unwrap_or(true) {
        eprintln!(
            "warning: BT_JWT_SECRET is unset — the minted token will only be \
             accepted by a server sharing this process's random secret. Set the \
             SAME BT_JWT_SECRET on the server and the bots."
        );
    }
    loop {
        match run_session(&ws_url, &name, &geo, persona).await {
            Ok(()) => println!("session closed; reconnecting"),
            Err(e) => eprintln!("session error: {e}; reconnecting"),
        }
        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

/// Build the `available` frame that puts us on the roster, challengeable +
/// auto-pairable, tagged as a bot with our geo. We re-mint the token each
/// connect so it stays valid across server restarts.
fn available_msg(name: &str, geo: &str, token: &str) -> Message {
    // Announce as a bot (so two bots never auto-pair) UNLESS BT_BOT_ANNOUNCE_BOT is
    // set falsey — a testing escape hatch to run a bot as a sparring "human" that
    // DOES auto-pair against another bot (e.g. a local Bert-vs-Ernie match).
    let bot = std::env::var("BT_BOT_ANNOUNCE_BOT")
        .map(|v| !matches!(v.as_str(), "false" | "0" | "no"))
        .unwrap_or(true);
    Message::Text(
        json!({
            "type": "available", "value": true,
            "name": name, "geo": geo, "bot": bot, "token": token,
        })
        .to_string(),
    )
}

async fn run_session(
    ws_url: &str,
    name: &str,
    geo: &str,
    persona: Persona,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = bt_identity::issue_token(name);
    let (ws, _resp) = tokio_tungstenite::connect_async(ws_url).await?;
    let (mut write, mut read) = ws.split();

    // A single outbound channel drained by a writer task, so the reader loop and
    // the tick loop can both send without sharing the sink.
    let (out, mut out_rx) = mpsc::unbounded_channel::<Message>();
    let writer = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if write.send(m).await.is_err() {
                break;
            }
        }
    });

    out.send(available_msg(name, geo, &token))?;

    let mut ms: Option<MatchState> = None;
    let mut seq: u64 = 0; // monotonic across the whole connection (always > a fresh bout's ack=0)

    let mut ticker = tokio::time::interval(Duration::from_millis(TICK_MS as u64));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut active = tokio::time::interval(ACTIVE_PING);

    loop {
        tokio::select! {
            inbound = read.next() => {
                let msg = match inbound {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => { writer.abort(); return Err(e.into()); }
                    None => break, // server closed
                };
                match msg {
                    Message::Text(t) => handle_text(&t, &out, &mut ms, persona),
                    Message::Ping(p) => { let _ = out.send(Message::Pong(p)); }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                if let Some(state) = ms.as_mut() {
                    drive_tick(state, &out, &mut seq);
                    if state.done {
                        println!(
                            "match over — {name}: {} lines cleared, {} weapons launched",
                            state.game.score().lines, state.launched
                        );
                        ms = None;
                        // Back to Available for the next challenger. (The server
                        // already resets us to Available at bout end; this also
                        // refreshes the geo/bot tags.)
                        let _ = out.send(available_msg(name, geo, &token));
                    }
                }
            }
            _ = active.tick() => {
                let _ = out.send(Message::Text(json!({"type":"active"}).to_string()));
            }
        }
    }
    writer.abort();
    Ok(())
}

/// Dispatch one server→bot text frame.
fn handle_text(text: &str, out: &Out, ms: &mut Option<MatchState>, persona: Persona) {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        // Someone challenged us — always accept.
        Some("challenged") => {
            let from = v.get("from").and_then(|f| f.as_str()).unwrap_or("");
            let _ = out.send(Message::Text(
                json!({ "type": "challengeAccept", "from": from }).to_string(),
            ));
        }
        // The match is starting: seed a fresh local sim for our side.
        Some("matchStart") => {
            let seed = v.get("seed").and_then(|s| s.as_u64()).unwrap_or(0);
            let opp = v.get("opponent").and_then(|o| o.as_str()).unwrap_or("?");
            println!("matchStart vs {opp} (seed {seed})");
            *ms = Some(MatchState::new(seed, persona));
        }
        // Authoritative frame: track result + bazaar, and reconcile on keyframes.
        Some("snapshot") => {
            if let Some(state) = ms.as_mut() {
                state.idle_ticks = 0; // the bout is alive
                if v.get("result").and_then(|r| r.as_i64()).unwrap_or(0) != 0 {
                    state.done = true;
                }
                if let Some(b) = v
                    .get("you")
                    .and_then(|y| y.get("in_bazaar"))
                    .and_then(|x| x.as_bool())
                {
                    state.in_bazaar = b;
                    if !b {
                        state.bazaar_left = false; // armed again for the next visit
                    }
                }
                // The full authoritative board rides the throttled keyframe (a
                // JSON array of bytes) — restore our local sim to it.
                if let Some(arr) = v.get("keyframe").and_then(|k| k.as_array()) {
                    let bytes: Vec<u8> =
                        arr.iter().filter_map(|n| n.as_u64().map(|x| x as u8)).collect();
                    state.game.restore_bytes(&bytes);
                }
                // Spy intel: a `spy_board` (the opponent's board, empty cells = -2)
                // rides keyframes only while one of our spies is active. Refresh the
                // "is the opponent stacked high?" read used to time board-raisers.
                if let Some(arr) = v.get("spy_board").and_then(|s| s.as_array()) {
                    let grid: Vec<i32> =
                        arr.iter().filter_map(|n| n.as_i64().map(|x| x as i32)).collect();
                    let w = state.game.board().width;
                    let h = state.game.board().height;
                    state.opp_high = opponent_is_high(&grid, w, h);
                    state.spy_fresh = SPY_FRESH_TICKS;
                }
            }
        }
        _ => {}
    }
}

/// One 16ms step of an in-progress match: handle the bazaar gate, advance the
/// local sim, and place a piece when the pacer says it's time.
fn drive_tick(state: &mut MatchState, out: &Out, seq: &mut u64) {
    if state.done {
        return;
    }
    // No authoritative frame for a while ⇒ the opponent vanished; end the match.
    state.idle_ticks += 1;
    if state.idle_ticks > STALE_TICKS {
        println!("match ended (opponent went silent)");
        state.done = true;
        return;
    }
    // Periodic progress log (~every 12s) so a long match still shows lines cleared
    // and weapons fired without waiting for a top-out.
    state.live_ticks += 1;
    if state.live_ticks % 750 == 0 {
        println!(
            "[{}] progress: {} lines, {} weapons launched",
            state.persona.suffix,
            state.game.score().lines,
            state.launched
        );
    }

    // Spy intel ages out between keyframes — once it lapses we're blind again.
    if state.spy_fresh > 0 {
        state.spy_fresh -= 1;
        if state.spy_fresh == 0 {
            state.opp_high = false;
        }
    }

    // Bazaar barrier (authoritative, or our local sim caught up to it): play is
    // frozen server-side, so shop a smart loadout once, then leave and wait.
    if state.in_bazaar || state.game.is_in_bazaar() {
        if !state.bazaar_left {
            shop_bazaar(state, out, seq);
            state.bazaar_left = true;
        }
        return;
    }

    state.game.tick(TICK_MS);
    if state.game.is_game_over() {
        return; // wait for the authoritative result to end the match
    }

    // Launch a weapon on a cadence (Ernie only) — a spy first (for intel),
    // board-raisers when the opponent is high, harassment/fund-drains otherwise.
    if state.persona.weapons {
        state.launch_cooldown -= 1;
        if state.launch_cooldown <= 0 {
            let arsenal = arsenal_of(&state.game);
            let spy_active = state.spy_fresh > 0;
            if let Some(slot) = launch_choice(&arsenal, spy_active, state.opp_high) {
                send_input(&mut state.game, out, seq, Input::LaunchWeapon(slot as u32));
                state.launched += 1;
                state.launch_cooldown = LAUNCH_INTERVAL_TICKS;
            } else {
                state.launch_cooldown = LAUNCH_RETRY_TICKS;
            }
        }
    }

    if state.cooldown > 0 {
        state.cooldown -= 1;
        return;
    }
    play_piece(&mut state.game, out, seq, state.persona.strong);
    state.cooldown = state.persona.place_ticks;
}

/// The local arsenal as the ten protocol token indices (-1 = empty slot).
fn arsenal_of(game: &Game) -> [i32; 10] {
    let mut a = [-1i32; 10];
    for (i, slot) in a.iter_mut().enumerate() {
        *slot = game.arsenal_token(i);
    }
    a
}

/// In the bazaar, buy a smart loadout ([`buy_plan`]) within our funds — each buy
/// applied locally (prediction) AND sent to the server — then leave so the barrier
/// lifts. Only buys the engine accepts are forwarded, keeping the sim in sync and
/// avoiding a stream of rejected inputs.
fn shop_bazaar(state: &mut MatchState, out: &Out, seq: &mut u64) {
    if state.game.is_in_bazaar() && state.persona.weapons {
        let funds = state.game.score().funds;
        let arsenal = arsenal_of(&state.game);
        let carter = state.game.weapon_active(WeaponToken::Carter);
        for tok in buy_plan(funds, &arsenal, carter) {
            if state.game.buy_weapon(tok) {
                *seq += 1;
                let iv = serde_json::to_value(Input::BuyWeapon(tok.index() as i32))
                    .unwrap_or(Value::Null);
                let _ = out.send(Message::Text(
                    json!({ "type": "input", "seq": *seq, "input": iv }).to_string(),
                ));
            }
        }
    }
    *seq += 1;
    let _ = out.send(Message::Text(
        json!({ "type": "input", "seq": *seq, "input": "LeaveBazaar" }).to_string(),
    ));
    if state.game.is_in_bazaar() {
        Input::LeaveBazaar.apply_to_game(&mut state.game);
    }
}

/// Whether the opponent's spied board (`render_ids`, empty cells = -2, row-major
/// `w`×`h`) is stacked tall enough to be worth a board-raiser — its highest column
/// fills at least [`OPP_HIGH_FRAC`] of the board.
fn opponent_is_high(grid: &[i32], w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 || grid.len() < (w * h) as usize {
        return false;
    }
    let mut min_top = h; // smallest filled-row index; h = empty column everywhere
    for x in 0..w {
        for y in 0..h {
            if grid[(y * w + x) as usize] >= 0 {
                if y < min_top {
                    min_top = y;
                }
                break;
            }
        }
    }
    (h - min_top) as f64 >= OPP_HIGH_FRAC * h as f64
}

/// Rotate + slide the current piece to `bt-ai`'s best placement, then hard-drop
/// — sending each move as a legal `input` AND mirroring it on the local sim.
/// Mirrors `bt_ai::Computer::take_turn` but emits the moves over the wire
/// instead of only mutating locally. All loops are bounded, so a blocked piece
/// just drops where it is rather than spinning.
fn play_piece(game: &mut Game, out: &Out, seq: &mut u64, strong: bool) {
    let (target_x, target_or, rot_cap) = match game.current_piece() {
        Some(p) => {
            let pl = if strong {
                best_placement_strong(game.board(), p)
            } else {
                best_placement(game.board(), p)
            };
            (pl.x, pl.orientation, p.orientations.max(1))
        }
        None => return,
    };

    // Rotate to the target orientation (first, like take_turn).
    for _ in 0..rot_cap {
        match game.current_piece() {
            Some(p) if p.orientation != target_or => send_input(game, out, seq, Input::Rotate),
            _ => break,
        }
    }
    // Slide to the target column.
    let move_cap = game.board().width * 2;
    for _ in 0..move_cap {
        match game.current_piece().map(|p| p.x) {
            Some(px) if px < target_x => send_input(game, out, seq, Input::MoveRight),
            Some(px) if px > target_x => send_input(game, out, seq, Input::MoveLeft),
            _ => break,
        }
    }
    // Slam it home.
    send_input(game, out, seq, Input::BeginDrop);
}

/// Apply an input to the local sim (prediction) and forward it to the server
/// with the next sequence number.
fn send_input(game: &mut Game, out: &Out, seq: &mut u64, input: Input) {
    input.apply_to_game(game);
    *seq += 1;
    let iv = serde_json::to_value(&input).unwrap_or(Value::Null);
    let _ = out.send(Message::Text(
        json!({ "type": "input", "seq": *seq, "input": iv }).to_string(),
    ));
}
