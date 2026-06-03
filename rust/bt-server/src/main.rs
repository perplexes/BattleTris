//! BattleTris server: one binary that serves the web client (static files) AND
//! the online backend on the SAME port —
//!   * `GET /ws`        WebSocket: matchmaking (paired by TrueSkill match
//!                      quality) and the SERVER-AUTHORITATIVE match itself — the
//!                      server runs the deterministic engine (a [`bout::Bout`]),
//!                      clients send inputs and reconcile against snapshots —
//!                      plus result -> rating updates persisted to a JSON file.
//!   * everything else  static files from `STATIC_DIR` (default `bt-wasm`,
//!                      which holds `www/` and `pkg/`); `/` redirects to `/www/`.
//!
//! Serving both on one port means the browser uses a same-origin
//! `ws(s)://<host>/ws`, which works locally and behind fly.io's TLS.
//!
//! Env: `PORT` (default 8080), `STATIC_DIR` (default `bt-wasm`),
//! `RATINGS_FILE` (default `ratings.json`).
//!
//! Protocol (JSON text frames):
//!   client -> server:
//!     {"type":"queue","name":"alice","authoritative":true}
//!     {"type":"input","seq":N,"input":<bt_replay::Input>}   (a gameplay action)
//!   server -> client:
//!     {"type":"matchStart","side":"A|B","seed":N,"opponent":"bob",...}
//!     {"type":"snapshot","tick":N,"ack":N,"result":...,"you":...,"opp":...,"keyframe"?:[..]}
//!     {"type":"rating","mu":...,"sigma":...,"conservative":...,"won":true}
//!     {"type":"opponentLeft"}
//!
//! (The legacy WebRTC P2P relay — `signal`/`matched` — was removed with the
//! migration to the server-authoritative model.)

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRef, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use bt_core::versus::Side;
use bt_replay::{Input, Replay, VersusReplay};
use bt_trueskill::ts2::{rate_match, MatchOutcome, PlayerState, Ts2Params, Winner};
use bt_trueskill::{quality_1v1, Rating};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

/// Server-authoritative online match engine (the client-server migration).
mod bout;
use bout::Bout;

/// Player identity (HS256 JWT) + the `/api/identity` endpoint.
mod identity;

/// One queued input headed for an authoritative [`Bout`]: (side, action, seq).
type BoutInput = (Side, Input, u64);

/// The engine build online replays are stamped with (the `git` short SHA passed
/// at compile time via `BT_GIT_SHA`, or "dev" locally).
const ENGINE_SHA: &str = match option_env!("BT_GIT_SHA") {
    Some(s) => s,
    None => "dev",
};

/// Matchmaking/relay state (tokio mutex — held across `.await`).
type Shared = Arc<Mutex<App>>;
/// The replay index/store. `rusqlite::Connection` is `Send` but not `Sync`, so a
/// std mutex guards it; replay queries are sub-millisecond and never `.await`
/// while the guard is held, so a blocking lock on the async runtime is fine here.
type Db = Arc<std::sync::Mutex<Connection>>;

/// Combined router state. `FromRef` lets each handler extract just the piece it
/// needs — `State<Shared>` for the websocket, `State<Db>` for replay endpoints.
#[derive(Clone)]
struct AppState {
    app: Shared,
    db: Db,
}

impl FromRef<AppState> for Shared {
    fn from_ref(s: &AppState) -> Shared {
        s.app.clone()
    }
}

impl FromRef<AppState> for Db {
    fn from_ref(s: &AppState) -> Db {
        s.db.clone()
    }
}

/// Lobby presence for a named client. An UN-named connection has no status and
/// never appears in the `players` roster (the implicit 4th "anonymous" state).
///
/// A client is *Available* iff it's both challengeable (a directed challenge can
/// reach it) AND eligible for auto-pairing — "open to matches" is one switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Open to matches: challengeable and auto-pairable.
    Available,
    /// Pressed "Find Match" — actively looking (still pairable + challengeable).
    Searching,
    /// In an authoritative bout.
    InGame,
}

impl Status {
    /// The lowercase wire string the frontend expects in the `players` frame.
    fn as_str(self) -> &'static str {
        match self {
            Status::Available => "available",
            Status::Searching => "searching",
            Status::InGame => "ingame",
        }
    }

    /// "Most-engaged" ordering for de-duping a name that has multiple
    /// connections: InGame > Searching > Available (show the busiest).
    fn rank(self) -> u8 {
        match self {
            Status::Available => 0,
            Status::Searching => 1,
            Status::InGame => 2,
        }
    }
}

/// Per-connected-client state.
struct Client {
    name: String,
    tx: mpsc::UnboundedSender<Message>,
    peer: Option<u64>,
    state: PlayerState,
    /// Does this client speak the server-authoritative protocol (sends `input`,
    /// renders `snapshot`)? Announced on `queue`. Every shipped client sets it;
    /// a client that doesn't simply gets no match handoff.
    authoritative: bool,
    /// While in an authoritative match: the channel to the match's tick loop and
    /// which side this client plays. `input` messages are forwarded here.
    bout: Option<(mpsc::Sender<BoutInput>, Side)>,
    /// Lobby presence. `None` until the client establishes a name (anonymous
    /// connections don't appear in the roster). See [`Status`].
    status: Option<Status>,
}

/// Shared server state.
struct App {
    clients: HashMap<u64, Client>,
    waiting: Vec<u64>,
    /// Last time each connection pressed a gameplay button. "Players online" is
    /// the count of these within `ACTIVE_WINDOW` (set on `active`, pruned on
    /// disconnect). Only pages send `active`, so matchmaking sockets never appear.
    last_active: HashMap<u64, Instant>,
    /// Last player count we broadcast, so the decay tick only re-broadcasts on a
    /// real change.
    last_broadcast_players: usize,
    /// Persisted ratings by player name: (mu, sigma, experience).
    ratings: HashMap<String, (f64, f64, u32)>,
    /// Pairings already rated (keyed by min(id_a, id_b)).
    settled: HashSet<u64>,
    params: Ts2Params,
    /// Replay/counters DB (shared with the HTTP handlers) — backs the hit counter
    /// and the per-player stats (`players`) table.
    db: Db,
    /// Outstanding directed challenges: challenger id -> (target id, expires_at).
    /// One in-flight challenge per challenger; superseded/cleared on
    /// accept/decline/timeout/disconnect.
    challenges: HashMap<u64, (u64, Instant)>,
    /// Last `players` roster we broadcast (the de-duped name->status list), so the
    /// presence push only re-sends on a real change.
    last_players: Vec<(String, Status)>,
}

impl App {
    fn new(db: Db) -> App {
        let ratings = {
            // Seed the players table from ratings.json on first boot (idempotent),
            // then load ratings from disk as the working rating source-of-truth.
            if let Ok(conn) = db.lock() {
                migrate_ratings_into_players(&conn, &load_ratings());
            }
            load_ratings()
        };
        App {
            clients: HashMap::new(),
            waiting: Vec::new(),
            last_active: HashMap::new(),
            last_broadcast_players: 0,
            ratings,
            settled: HashSet::new(),
            params: Ts2Params::default(),
            db,
            challenges: HashMap::new(),
            last_players: Vec::new(),
        }
    }

    fn rating_for(&self, name: &str) -> PlayerState {
        match self.ratings.get(name) {
            Some(&(mu, sigma, experience)) => PlayerState { rating: Rating::new(mu, sigma), experience },
            None => PlayerState::new(self.params.base.new_rating()),
        }
    }

    fn store_rating(&mut self, name: &str, s: PlayerState) {
        self.ratings
            .insert(name.to_string(), (s.rating.mu, s.rating.sigma, s.experience));
    }
}

fn ratings_file() -> String {
    std::env::var("RATINGS_FILE").unwrap_or_else(|_| "ratings.json".to_string())
}

fn send(app: &App, id: u64, msg: &Value) {
    if let Some(c) = app.clients.get(&id) {
        let _ = c.tx.send(Message::Text(msg.to_string()));
    }
}

/// Send a message to every connected client.
fn broadcast(app: &App, msg: &Value) {
    let text = msg.to_string();
    for c in app.clients.values() {
        let _ = c.tx.send(Message::Text(text.clone()));
    }
}

/// "Players online" window: anyone who pressed a gameplay button this recently.
const ACTIVE_WINDOW: Duration = Duration::from_secs(30);

/// Number of players active within `ACTIVE_WINDOW` as of `now` (testable: `now`
/// is passed in rather than read from the clock).
fn active_count(app: &App, now: Instant) -> usize {
    app.last_active
        .values()
        .filter(|&&t| now.saturating_duration_since(t) < ACTIVE_WINDOW)
        .count()
}

fn current_hits(app: &App) -> i64 {
    let conn = app.db.lock().unwrap();
    db_hits(&conn)
}

/// The live stats frame pushed to every page: active players now + the
/// persistent total visit count.
fn stats_msg(players: usize, hits: i64) -> Value {
    json!({ "type": "stats", "players": players, "hits": hits })
}

/// Recompute the active-player count and, if it changed since the last
/// broadcast, push fresh stats to everyone. Called on activity, on disconnect,
/// and from the decay tick (so the count falls as players go idle).
fn maybe_broadcast_stats(app: &mut App) {
    let players = active_count(app, Instant::now());
    if players != app.last_broadcast_players {
        app.last_broadcast_players = players;
        let msg = stats_msg(players, current_hits(app));
        broadcast(app, &msg);
    }
}

/// The de-duped lobby roster: one entry per *named* client, keeping the
/// most-engaged status when a name has several connections (e.g. two tabs).
/// Sorted by name so the broadcast-on-change comparison is order-stable.
fn roster(app: &App) -> Vec<(String, Status)> {
    let mut by_name: HashMap<String, Status> = HashMap::new();
    for c in app.clients.values() {
        let (name, status) = match (c.name.as_str(), c.status) {
            (n, Some(s)) if !n.is_empty() => (n.to_string(), s),
            _ => continue, // anonymous / un-named connections aren't listed
        };
        by_name
            .entry(name)
            .and_modify(|cur| {
                if status.rank() > cur.rank() {
                    *cur = status;
                }
            })
            .or_insert(status);
    }
    let mut out: Vec<(String, Status)> = by_name.into_iter().collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The `players` presence frame the lobby renders.
fn players_msg(roster: &[(String, Status)]) -> Value {
    let players: Vec<Value> = roster
        .iter()
        .map(|(name, status)| json!({ "name": name, "status": status.as_str() }))
        .collect();
    json!({ "type": "players", "players": players })
}

/// Recompute the lobby roster and, if it changed since the last push, broadcast
/// it to everyone. Mirrors [`maybe_broadcast_stats`]: collect first, then send.
fn maybe_broadcast_players(app: &mut App) {
    let next = roster(app);
    if next != app.last_players {
        app.last_players = next.clone();
        let msg = players_msg(&next);
        broadcast(app, &msg);
    }
}

fn load_ratings() -> HashMap<String, (f64, f64, u32)> {
    let mut out = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string(ratings_file()) {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(&txt) {
            for (name, v) in map {
                let mu = v.get("mu").and_then(|x| x.as_f64()).unwrap_or(25.0);
                let sigma = v.get("sigma").and_then(|x| x.as_f64()).unwrap_or(25.0 / 3.0);
                let exp = v.get("experience").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
                out.insert(name, (mu, sigma, exp));
            }
        }
    }
    out
}

fn save_ratings(ratings: &HashMap<String, (f64, f64, u32)>) {
    let obj: Value = ratings
        .iter()
        .map(|(name, &(mu, sigma, exp))| {
            (name.clone(), json!({"mu": mu, "sigma": sigma, "experience": exp}))
        })
        .collect::<serde_json::Map<_, _>>()
        .into();
    if let Ok(txt) = serde_json::to_string_pretty(&obj) {
        let _ = std::fs::write(ratings_file(), txt);
    }
}

/// Bounded capacity of a bout's input channel. A legitimate client sends a
/// handful of inputs per second; this caps memory and drains-per-tick under a
/// spam flood (excess inputs are dropped at the sender via `try_send`).
const BOUT_INPUT_CAP: usize = 256;

/// Everything the async caller needs to spawn an authoritative match's tick
/// loop, handed back by [`try_match`] when it pairs two authoritative clients.
/// Player names + rating states are captured here so the bout can settle even
/// if a client has disconnected (and been removed from `app.clients`) by the
/// time the match ends.
struct PendingBout {
    /// Globally-unique match id (from `NEXT_ID`, the same counter as connection
    /// ids, so it's disjoint from every connection id and every legacy min-id
    /// settle key). Keys this match's settlement.
    match_id: u64,
    id_a: u64,
    id_b: u64,
    seed_a: u64,
    seed_b: u64,
    name_a: String,
    name_b: String,
    state_a: PlayerState,
    state_b: PlayerState,
    tx_a: mpsc::UnboundedSender<Message>,
    tx_b: mpsc::UnboundedSender<Message>,
    input_rx: mpsc::Receiver<BoutInput>,
}

/// A per-match seed from a connection id — distinct across matches (ids are
/// monotonic) without an rng dependency, and masked to 32 bits so it round-trips
/// through the JS client's `WasmGame::new(seed: u32)` exactly (same RNG stream
/// on both sides → client prediction agrees with the authoritative sim).
fn derive_seed(id: u64) -> u64 {
    (id.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 16) & 0xFFFF_FFFF
}

/// How long a directed challenge stays open before the challenger is told the
/// target declined (no response in time).
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Find the connected client whose name == `name`, preferring one currently
/// Available (the challengeable connection) over any other tab. Returns its id.
fn find_named_available(app: &App, name: &str) -> Option<u64> {
    // First pass: an Available named connection (the one a challenge can land on).
    if let Some((&id, _)) = app
        .clients
        .iter()
        .find(|(_, c)| c.name == name && c.status == Some(Status::Available))
    {
        return Some(id);
    }
    None
}

/// Drop a challenger's pending challenge (if any). Returns the target id it was
/// pointed at, so the caller can decide whether to notify.
fn clear_challenge(app: &mut App, challenger: u64) -> Option<u64> {
    app.challenges.remove(&challenger).map(|(target, _)| target)
}

/// Is `cid` a candidate the matcher may pair `id` with right now? Not `id`
/// itself, not already in a bout, and "open to matches" — either explicitly
/// queued (in `waiting`, the legacy Find-Match path) OR lobby-Available /
/// -Searching (the unified presence switch: Available = challengeable AND
/// auto-pairable). A queued client stays a candidate even if it never announced
/// `authoritative` (legacy peer-link path); a presence candidate is by
/// definition an authoritative lobby client.
fn is_match_candidate(app: &App, id: u64, cid: u64) -> bool {
    if cid == id {
        return false;
    }
    match app.clients.get(&cid) {
        Some(c) => {
            c.bout.is_none()
                && (app.waiting.contains(&cid)
                    || matches!(c.status, Some(Status::Available) | Some(Status::Searching)))
        }
        None => false,
    }
}

/// Match `id` against the best-quality open opponent; otherwise leave it queued.
///
/// The candidate pool is everyone "open to matches" — explicitly queued
/// (`waiting`) AND lobby-Available clients — so going Available auto-pairs you
/// just like pressing Find Match. Peers are linked for whatever pair it picks
/// (the legacy result/`opponentLeft` path needs that); it returns
/// `Some(PendingBout)` only when BOTH sides are server-authoritative, which every
/// shipped client is.
fn try_match(app: &mut App, id: u64) -> Option<PendingBout> {
    let my_rating = app.clients.get(&id).map(|c| c.state.rating)?;
    // Already in a bout? Don't re-match (would clobber the live binding).
    if app.clients.get(&id).is_some_and(|c| c.bout.is_some()) {
        return None;
    }

    // Scan all open candidates (queued or Available/Searching), de-duped.
    let candidates: Vec<u64> = app
        .clients
        .keys()
        .copied()
        .filter(|&cid| is_match_candidate(app, id, cid))
        .collect();
    let mut best: Option<(u64, f64)> = None;
    for cid in candidates {
        if let Some(other) = app.clients.get(&cid) {
            let q = quality_1v1(my_rating, other.state.rating, &app.params.base);
            if best.map(|(_, bq)| q > bq).unwrap_or(true) {
                best = Some((cid, q));
            }
        }
    }

    let (opp, quality) = match best {
        Some(x) => x,
        None => {
            if !app.waiting.contains(&id) {
                app.waiting.push(id);
            }
            return None;
        }
    };

    // Link peers for whatever we picked (legacy result + opponentLeft path).
    app.waiting.retain(|&w| w != opp && w != id);
    if let Some(c) = app.clients.get_mut(&opp) {
        c.peer = Some(id);
    }
    if let Some(c) = app.clients.get_mut(&id) {
        c.peer = Some(opp);
    }

    // opp = side A (first queued/available), id = side B. start_bout returns None
    // (no handoff) unless both sides are authoritative — the peer link stands
    // either way for the legacy client-reported-result path.
    start_bout(app, opp, id, Some(quality))
}

/// Build a server-authoritative bout between two connected, authoritative
/// clients: drop them from the queue, link peers, set both `InGame`, bind the
/// input channel, send `matchStart`, and return the [`PendingBout`] the caller
/// spawns. Shared by the auto-matcher ([`try_match`]) and the directed-challenge
/// path. `quality` is the matchmaking quality if known (auto-match) else `None`
/// (a hand-picked challenge has no quality figure). Returns `None` if either
/// side vanished or isn't authoritative.
fn start_bout(app: &mut App, a: u64, b: u64, quality: Option<f64>) -> Option<PendingBout> {
    let both_authoritative = app.clients.get(&a).is_some_and(|c| c.authoritative)
        && app.clients.get(&b).is_some_and(|c| c.authoritative);
    if !both_authoritative {
        return None;
    }

    app.waiting.retain(|&w| w != a && w != b);
    // Drop any pending challenges involving either player — they're now in a
    // match, so a stale accept must not later kick off a second, unwanted bout.
    app.challenges
        .retain(|&cid, &mut (tid, _)| cid != a && cid != b && tid != a && tid != b);
    if let Some(c) = app.clients.get_mut(&a) {
        c.peer = Some(b);
    }
    if let Some(c) = app.clients.get_mut(&b) {
        c.peer = Some(a);
    }

    let (a_name, a_state) = match app.clients.get(&a) {
        Some(c) => (c.name.clone(), c.state),
        None => return None,
    };
    let (b_name, b_state) = match app.clients.get(&b) {
        Some(c) => (c.name.clone(), c.state),
        None => return None,
    };

    let (seed_a, seed_b) = (derive_seed(a), derive_seed(b));
    let (input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
    let tx_a = app.clients[&a].tx.clone();
    let tx_b = app.clients[&b].tx.clone();
    if let Some(c) = app.clients.get_mut(&a) {
        c.bout = Some((input_tx.clone(), Side::A));
        c.status = Some(Status::InGame);
    }
    if let Some(c) = app.clients.get_mut(&b) {
        c.bout = Some((input_tx, Side::B));
        c.status = Some(Status::InGame);
    }
    // quality is optional in the wire frame; a directed challenge omits it.
    let qv = quality.map(|q| json!(q)).unwrap_or(Value::Null);
    send(app, a, &json!({"type":"matchStart","side":"A","seed":seed_a,"opponent":b_name,"quality":qv}));
    send(app, b, &json!({"type":"matchStart","side":"B","seed":seed_b,"opponent":a_name,"quality":qv}));
    match quality {
        Some(q) => println!("authoritative match {a} <-> {b} (quality {q:.3})"),
        None => println!("authoritative challenge match {a} <-> {b}"),
    }
    // Roster changed (both went InGame) — let the lobby reflect it.
    maybe_broadcast_players(app);
    Some(PendingBout {
        match_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        id_a: a,
        id_b: b,
        seed_a,
        seed_b,
        name_a: a_name,
        name_b: b_name,
        state_a: a_state,
        state_b: b_state,
        tx_a,
        tx_b,
        input_rx,
    })
}

/// Settle an authoritative match from the bout's OWN captured player identities
/// (not live `app.clients`, which may have lost the loser on a forfeit
/// disconnect). Idempotent per pair; rating messages go to whoever is still
/// connected.
#[allow(clippy::too_many_arguments)]
fn settle_bout(
    app: &mut App,
    match_id: u64,
    id_a: u64,
    name_a: &str,
    state_a: PlayerState,
    id_b: u64,
    name_b: &str,
    state_b: PlayerState,
    a_won: bool,
    a_lines: u32,
    b_lines: u32,
) {
    // Key on the unique match id, not min(id), so a connection's later match
    // (same min-id) isn't wrongly skipped. run_bout calls this once per match,
    // so the guard is purely defensive.
    if !app.settled.insert(match_id) {
        return;
    }
    let outcome = MatchOutcome {
        winner: if a_won { Winner::A } else { Winner::B },
        a_lines,
        b_lines,
        a_quit: false,
        b_quit: false,
    };
    let (na, nb) = rate_match(state_a, state_b, &outcome, &app.params);
    app.store_rating(name_a, na);
    app.store_rating(name_b, nb);
    if let Some(c) = app.clients.get_mut(&id_a) {
        c.state = na;
    }
    if let Some(c) = app.clients.get_mut(&id_b) {
        c.state = nb;
    }
    save_ratings(&app.ratings);
    send(app, id_a, &rating_msg(&na, a_won));
    send(app, id_b, &rating_msg(&nb, !a_won));
    println!(
        "authoritative result: {name_a} {} {name_b}",
        if a_won { "beat" } else { "lost to" }
    );
}

/// Fold a finished bout into both players' rows in the `players` table:
/// record/streak/personal-bests/time figures. Run AFTER [`settle_bout`] so the
/// post-match mu/sigma are already in `app.ratings`. Best-effort: a DB hiccup is
/// logged-skipped rather than failing the bout. `final_a`/`final_b` are each
/// side's `(score, lines, funds)` at game-over; `ticks` is the match length.
#[allow(clippy::too_many_arguments)]
fn record_bout_player_stats(
    app: &App,
    name_a: &str,
    name_b: &str,
    a_won: bool,
    final_a: (i64, i64, i64),
    final_b: (i64, i64, i64),
    ticks: u64,
    natural: bool,
) {
    let conn = match app.db.lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let rating_of = |name: &str| {
        let base = app.params.base.new_rating();
        app.ratings.get(name).map(|&(mu, s, _)| (mu, s)).unwrap_or((base.mu, base.sigma))
    };
    for (name, won, (score, lines, funds)) in
        [(name_a, a_won, final_a), (name_b, !a_won, final_b)]
    {
        let (mu, sigma) = rating_of(name);
        let m = MatchStats { won, score, lines, funds, ticks: ticks as i64, natural };
        if let Err(e) = db_record_player_stats(&conn, name, mu, sigma, &m) {
            eprintln!("player stats write failed for {name}: {e}");
        }
    }
}

/// The per-match authoritative tick loop. Advances the deterministic engine on
/// the server's clock, broadcasts a snapshot to each client (~30Hz), and settles
/// on the natural end (or by forfeit if a client drops).
async fn run_bout(state: Shared, pb: PendingBout) {
    let PendingBout {
        match_id, id_a, id_b, seed_a, seed_b, name_a, name_b, state_a, state_b, tx_a, tx_b,
        mut input_rx,
    } = pb;
    let mut bout = Bout::new(seed_a, seed_b);
    let mut ticker = tokio::time::interval(Duration::from_millis(bout::TICK_MS as u64));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // `Some(a_won)` = settle with A as winner/loser; `None` = both gone, nothing to rate.
    let mut frame: u64 = 0;
    let mut want_keyframe = true; // send one on the very first frame
    let outcome: Option<bool> = loop {
        ticker.tick().await;
        // Drain queued inputs (the channel is bounded, so this is O(cap)).
        while let Ok((side, input, seq)) = input_rx.try_recv() {
            bout.apply_input(side, &input, seq);
        }
        bout.tick(bout::TICK_MS);
        // A cross-player effect this tick (weapon/funds/bazaar) is something the
        // clients couldn't predict — push a prompt keyframe on the next send.
        if bout.take_dirty() {
            want_keyframe = true;
        }

        if bout.is_over() {
            // Final frame carries a keyframe so both clients settle on the end state.
            let _ = tx_a.send(Message::Text(bout.snapshot_message(Side::A, true)));
            let _ = tx_b.send(Message::Text(bout.snapshot_message(Side::B, true)));
            break Some(bout.result() == 1); // 1 = A won
        }
        // Snapshots go out at ~30Hz (every other 16ms tick) — this is also where a
        // client disconnect is detected (the send fails). A reconciliation keyframe
        // rides the first frame, the ~2Hz heartbeat, and any frame after an
        // unpredictable cross-player event, so corrections are prompt.
        if frame % 2 == 0 {
            let kf = want_keyframe || frame % 32 == 0;
            if kf {
                want_keyframe = false;
            }
            let a_ok = tx_a.send(Message::Text(bout.snapshot_message(Side::A, kf))).is_ok();
            let b_ok = tx_b.send(Message::Text(bout.snapshot_message(Side::B, kf))).is_ok();
            match (a_ok, b_ok) {
                (false, false) => break None,       // both disconnected
                (true, false) => break Some(true),  // B dropped -> A wins by forfeit
                (false, true) => break Some(false), // A dropped -> B wins by forfeit
                (true, true) => {}
            }
        }
        frame += 1;
    };

    let mut app = state.lock().await;
    // Match over: both sides leave the bout and (if still connected + named) go
    // back to Available so the lobby shows them open to a new match.
    for cid in [id_a, id_b] {
        if let Some(c) = app.clients.get_mut(&cid) {
            c.bout = None;
            c.peer = None;
            if !c.name.is_empty() {
                c.status = Some(Status::Available);
            }
        }
    }
    if let Some(a_won) = outcome {
        settle_bout(
            &mut app, match_id, id_a, &name_a, state_a, id_b, &name_b, state_b,
            a_won, bout.lines(Side::A), bout.lines(Side::B),
        );
        // Real per-player stats (the `players` table) — record/streak/bests/time.
        // settle_bout has updated app.ratings; read each side's post-match rating.
        record_bout_player_stats(
            &app, &name_a, &name_b, a_won,
            (bout.score(Side::A), bout.lines(Side::A) as i64, bout.funds(Side::A)),
            (bout.score(Side::B), bout.lines(Side::B) as i64, bout.funds(Side::B)),
            bout.tick_count(),
            bout.is_over(), // natural finish only — a forfeit isn't a real top-out
        );
        // Persist the match as a deterministic, replayable VersusReplay (closes
        // D5) — but ONLY for a natural finish (a real top-out the board reaches).
        // A forfeit (a client disconnected) isn't in the seed+input stream, so its
        // playback would never latch a winner; we don't store those.
        if bout.is_over() {
            let replay = bout.to_replay(bout::TICK_MS, ENGINE_SHA);
            let json = replay.to_json();
            let id = replay_id(&json);
            let stored = app
                .db
                .lock()
                .ok()
                .and_then(|conn| db_insert_versus(&conn, &id, &replay, &json, now_secs()).ok())
                .is_some();
            if stored {
                println!("stored online replay {id} ({} ticks)", replay.tick_count);
            }
        }
    }
    // Both players went back to Available (or left) — refresh the lobby roster.
    maybe_broadcast_players(&mut app);
}

/// Settle a match result and update both players' ratings (idempotent per pair).
fn settle_result(app: &mut App, id: u64, won: bool, lines: u32, op_lines: u32) {
    let peer = match app.clients.get(&id).and_then(|c| c.peer) {
        Some(p) => p,
        None => return,
    };
    let key = id.min(peer);
    if app.settled.contains(&key) {
        return;
    }
    app.settled.insert(key);

    let (a_name, a_state) = match app.clients.get(&id) {
        Some(c) => (c.name.clone(), c.state),
        None => return,
    };
    let (b_name, b_state) = match app.clients.get(&peer) {
        Some(c) => (c.name.clone(), c.state),
        None => return,
    };

    let outcome = MatchOutcome {
        winner: if won { Winner::A } else { Winner::B },
        a_lines: lines,
        b_lines: op_lines,
        a_quit: false,
        b_quit: false,
    };
    let (na, nb) = rate_match(a_state, b_state, &outcome, &app.params);

    app.store_rating(&a_name, na);
    app.store_rating(&b_name, nb);
    if let Some(c) = app.clients.get_mut(&id) {
        c.state = na;
    }
    if let Some(c) = app.clients.get_mut(&peer) {
        c.state = nb;
    }
    save_ratings(&app.ratings);

    send(app, id, &rating_msg(&na, won));
    send(app, peer, &rating_msg(&nb, !won));
    println!(
        "result: {a_name} {} {b_name} -> {:.2}±{:.2} / {:.2}±{:.2}",
        if won { "beat" } else { "lost to" },
        na.rating.mu, na.rating.sigma, nb.rating.mu, nb.rating.sigma
    );
}

fn rating_msg(s: &PlayerState, won: bool) -> Value {
    json!({
        "type": "rating",
        "mu": s.rating.mu, "sigma": s.rating.sigma,
        "conservative": s.rating.conservative(3.0), "won": won,
    })
}

/// Resolve the name a message establishes, preferring a valid `token`'s signed
/// name over a bare `name`. A present-but-invalid token is ignored (we keep the
/// `prior` name rather than trust a forged one). With no token and no name, the
/// prior name stands. Used by `queue`/`available`/`challenge`.
fn resolve_name(app: &App, v: &Value, prior: &str) -> String {
    if let Some(tok) = v.get("token").and_then(|t| t.as_str()) {
        if let Some(signed) = identity::verify_token(tok) {
            return signed; // a validly-signed name always wins
        }
        // Bad/forged token: ignore it (don't trust the rest of this message's name).
    }
    if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
        if let Some(clean) = identity::sanitize_name(name) {
            // A bare (untokened) name is only honored for a NEW identity. Claiming
            // an already-rated name requires a valid token, so a one-line `name`
            // message can't hijack an established player's stats/rating. (Anyone
            // can still mint a token for any name via /api/identity — identity is
            // deliberately lightweight, not account-grade — but that's a step up
            // from trivially spoofing a bare name.)
            if !app.ratings.contains_key(&clean) || clean == prior {
                return clean;
            }
        }
    }
    prior.to_string()
}

async fn handle_message(state: &Shared, id: u64, text: &str) {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        // A page opened: count it as a visitor (persistent hit counter), then
        // push the current numbers to everyone so the new page is populated.
        // (It isn't an active player yet — that needs a gameplay button.)
        Some("watch") => {
            let mut app = state.lock().await;
            {
                let conn = app.db.lock().unwrap();
                let _ = db_bump_hits(&conn);
            }
            let players = active_count(&app, Instant::now());
            app.last_broadcast_players = players;
            let msg = stats_msg(players, current_hits(&app));
            broadcast(&app, &msg);
            // Send the CURRENT lobby roster to this just-connected client — the
            // periodic `players` push only fires on change, so without this a
            // client that connects after others are already online sees nobody.
            let r = roster(&app);
            send(&app, id, &players_msg(&r));
        }
        // A gameplay button was pressed: this connection is an active player for
        // the next `ACTIVE_WINDOW`. Re-broadcast if the live count went up.
        Some("active") => {
            let mut app = state.lock().await;
            app.last_active.insert(id, Instant::now());
            maybe_broadcast_stats(&mut app);
        }
        Some("queue") => {
            // Identity: prefer the token's signed name; a bare `name` is still
            // accepted (back-compat) but the token wins when both are present.
            // New clients announce the authoritative protocol; old ones omit it
            // and keep the WebRTC handoff untouched.
            let authoritative = v.get("authoritative").and_then(|b| b.as_bool()).unwrap_or(false);
            let pending = {
                let mut app = state.lock().await;
                // Ignore a re-queue from a client already in an authoritative
                // match (it would otherwise be matched into a second bout that
                // could clobber this one's binding / settle the wrong pairing).
                if app.clients.get(&id).is_some_and(|c| c.bout.is_some()) {
                    None
                } else {
                    let prior = app.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
                    let name = resolve_name(&app, &v, &prior);
                    let name = if name.is_empty() { "anon".to_string() } else { name };
                    let st = app.rating_for(&name);
                    if let Some(c) = app.clients.get_mut(&id) {
                        c.name = name;
                        c.state = st;
                        c.authoritative = authoritative;
                        c.status = Some(Status::Searching); // Find Match -> Searching
                    }
                    let pb = try_match(&mut app, id);
                    // Roster changed (this client is now Searching); push it. If a
                    // bout started, start_bout already broadcast InGame.
                    maybe_broadcast_players(&mut app);
                    pb
                }
            };
            // Spawn the match's tick loop outside the lock (it locks again only to settle).
            if let Some(pb) = pending {
                tokio::spawn(run_bout(state.clone(), pb));
            }
        }
        // Lobby presence toggle. `{"value":true}` (with an optional identity
        // `token`/`name`) marks this client "open to matches" — both
        // challengeable AND auto-pairable. `{"value":false}` leaves the roster.
        // Going Available also attempts an immediate auto-pair (a directed
        // challenge isn't required to get into a game).
        Some("available") => {
            let value = v.get("value").and_then(|b| b.as_bool()).unwrap_or(true);
            let pending = {
                let mut app = state.lock().await;
                if app.clients.get(&id).is_some_and(|c| c.bout.is_some()) {
                    None // already in a match — ignore
                } else {
                    let prior = app.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
                    let name = resolve_name(&app, &v, &prior);
                    let go_available = value && !name.is_empty();
                    let st = app.rating_for(&name);
                    if !go_available {
                        // Going unavailable / un-named -> leave the roster + queue.
                        app.waiting.retain(|&w| w != id);
                    }
                    if let Some(c) = app.clients.get_mut(&id) {
                        c.name = name;
                        c.state = st;
                        // `available` is the shipped lobby's authoritative client.
                        c.authoritative = true;
                        c.status = go_available.then_some(Status::Available);
                    }
                    // An Available, named client is eligible for auto-pairing.
                    let pb = if go_available { try_match(&mut app, id) } else { None };
                    maybe_broadcast_players(&mut app);
                    pb
                }
            };
            if let Some(pb) = pending {
                tokio::spawn(run_bout(state.clone(), pb));
            }
        }
        // Directed challenge: alice -> "I want to play <target>". Find that
        // player's Available connection and ping it with `challenged`; track the
        // pending challenge (challenger -> target) with a 30s timeout.
        Some("challenge") => {
            let mut app = state.lock().await;
            // Resolve/refresh the challenger's identity from any token/name.
            let prior = app.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
            let from = resolve_name(&app, &v, &prior);
            if let Some(c) = app.clients.get_mut(&id) {
                c.name = from.clone();
                c.authoritative = true;
            }
            let target = v.get("target").and_then(|t| t.as_str()).unwrap_or("").to_string();
            match find_named_available(&app, &target) {
                Some(tid) if tid != id => {
                    let deadline = Instant::now() + CHALLENGE_TIMEOUT;
                    app.challenges.insert(id, (tid, deadline));
                    send(&app, tid, &json!({"type":"challenged","from":from}));
                    // Schedule the timeout: if still pending after the window, tell
                    // the challenger the target "declined" (no answer). Compare the
                    // exact deadline so an earlier timer can't clear a *re-issued*
                    // challenge to the same target.
                    let state2 = state.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(CHALLENGE_TIMEOUT).await;
                        let mut app = state2.lock().await;
                        if let Some((t, d)) = app.challenges.get(&id).copied() {
                            if t == tid && d == deadline {
                                clear_challenge(&mut app, id);
                                let by = app.clients.get(&tid).map(|c| c.name.clone()).unwrap_or(target);
                                send(&app, id, &json!({"type":"challengeDeclined","by":by}));
                            }
                        }
                    });
                }
                // Target offline / busy / self — decline immediately.
                _ => {
                    send(&app, id, &json!({"type":"challengeDeclined","by":target}));
                }
            }
        }
        // bob accepts alice's challenge. If a matching pending challenge exists,
        // build a directed bout for (challenger, accepter) and start it.
        Some("challengeAccept") => {
            let from = v.get("from").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let pending = {
                let mut app = state.lock().await;
                // Find the challenger by name whose pending challenge targets THIS id.
                let challenger = app
                    .challenges
                    .iter()
                    .find(|(&cid, &(tid, _))| {
                        tid == id && app.clients.get(&cid).is_some_and(|c| c.name == from)
                    })
                    .map(|(&cid, _)| cid);
                match challenger {
                    Some(cid)
                        if app.clients.get(&cid).is_some_and(|c| c.bout.is_none())
                            && app.clients.get(&id).is_some_and(|c| c.bout.is_none()) =>
                    {
                        clear_challenge(&mut app, cid);
                        // Challenger = side A, accepter = side B (no quality figure).
                        start_bout(&mut app, cid, id, None)
                    }
                    _ => None,
                }
            };
            if let Some(pb) = pending {
                tokio::spawn(run_bout(state.clone(), pb));
            }
        }
        // bob declines alice's challenge -> tell alice, clear the pending challenge.
        Some("challengeDecline") => {
            let from = v.get("from").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let mut app = state.lock().await;
            let challenger = app
                .challenges
                .iter()
                .find(|(&cid, &(tid, _))| {
                    tid == id && app.clients.get(&cid).is_some_and(|c| c.name == from)
                })
                .map(|(&cid, _)| cid);
            if let Some(cid) = challenger {
                clear_challenge(&mut app, cid);
                let by = app.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
                send(&app, cid, &json!({"type":"challengeDeclined","by":by}));
            }
        }
        // A gameplay action in a server-authoritative match — forward it to that
        // match's tick loop, which validates + applies it. {seq, input}.
        Some("input") => {
            let seq = v.get("seq").and_then(|n| n.as_u64()).unwrap_or(0);
            let input = v
                .get("input")
                .and_then(|iv| serde_json::from_value::<Input>(iv.clone()).ok());
            if let Some(input) = input {
                let app = state.lock().await;
                if let Some((tx, side)) = app.clients.get(&id).and_then(|c| c.bout.as_ref()) {
                    // try_send: drop under flood rather than grow memory (bounded channel).
                    let _ = tx.try_send((*side, input, seq));
                }
            }
        }
        Some("result") => {
            let won = v.get("won").and_then(|b| b.as_bool()).unwrap_or(false);
            let lines = v.get("lines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let op_lines = v.get("opLines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let mut app = state.lock().await;
            // Authoritative matches are settled server-side by run_bout; ignore a
            // client-reported result while it's in a bout (untrusted + double-settle).
            if app.clients.get(&id).is_none_or(|c| c.bout.is_none()) {
                settle_result(&mut app, id, won, lines, op_lines);
            }
        }
        _ => {}
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Shared>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Shared) {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    {
        let mut a = state.lock().await;
        let st = a.rating_for("");
        a.clients.insert(
            id,
            Client { name: String::new(), tx, peer: None, state: st, authoritative: false, bout: None, status: None },
        );
    }

    // Writer task: drain the per-client channel to the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(t) => handle_message(&state, id, &t).await,
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup on disconnect: notify the peer, drop from queue/clients.
    {
        let mut a = state.lock().await;
        a.waiting.retain(|&w| w != id);
        // This connection's own pending challenge (as challenger) is cancelled.
        clear_challenge(&mut a, id);
        // Any pending challenge TARGETING this connection: the target left, so the
        // challenger should be told it was declined and the challenge cleared.
        let aimed_here: Vec<u64> = a
            .challenges
            .iter()
            .filter(|(_, &(tid, _))| tid == id)
            .map(|(&cid, _)| cid)
            .collect();
        let by = a.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        for cid in aimed_here {
            clear_challenge(&mut a, cid);
            send(&a, cid, &json!({"type":"challengeDeclined","by":by}));
        }
        let peer = a.clients.get(&id).and_then(|c| c.peer);
        if let Some(p) = peer {
            send(&a, p, &json!({"type": "opponentLeft"}));
            if let Some(c) = a.clients.get_mut(&p) {
                c.peer = None;
            }
        }
        let was_listed = a.clients.get(&id).is_some_and(|c| c.status.is_some());
        a.clients.remove(&id);
        // If a named/listed client left, the lobby roster changed — push it.
        if was_listed {
            maybe_broadcast_players(&mut a);
        }
        // If an active player's page closed, the live count may drop — recompute
        // and push it to everyone still here.
        if a.last_active.remove(&id).is_some() {
            maybe_broadcast_stats(&mut a);
        }
    }
    writer.abort();
    println!("client {id} disconnected");
}

// --- replay store -------------------------------------------------------
//
// A SQLite database (the fly volume in prod) is the single source of truth: it
// holds the full recording JSON alongside the metadata the library lists by, so
// browsing is one indexed `SELECT … ORDER BY created_at` — not a scan that opens
// and parses every file. A recording uploaded by the "report a bug" button or
// the 🔗 Share button lands here keyed by a content hash (dedup), and is fetched
// back by id. Older deployments stored replays as JSON files; those are imported
// into the DB once at startup (see `import_dir`).

/// Path to the SQLite database (`REPLAY_DB`, default `replays.db`; prod sets
/// `/data/replays.db` on the fly volume).
fn db_path() -> String {
    std::env::var("REPLAY_DB").unwrap_or_else(|_| "replays.db".to_string())
}

/// Legacy on-disk JSON directory, imported once at startup if present
/// (`REPLAYS_DIR`, default `replays`; prod's old store was `/data/replays`).
fn replays_dir() -> String {
    std::env::var("REPLAYS_DIR").unwrap_or_else(|_| "replays".to_string())
}

/// A stable, content-derived id (16 hex chars). `DefaultHasher` uses fixed keys,
/// so identical JSON always maps to the same id across runs.
fn replay_id(json: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    json.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Ids are our own hex hashes; reject anything else (defensive — the DB binds
/// ids as parameters, so this is belt-and-suspenders against malformed input).
fn valid_replay_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const SCHEMA: &str = "\
CREATE TABLE IF NOT EXISTS replays (
    id          TEXT PRIMARY KEY,
    mode        TEXT NOT NULL,
    seed        INTEGER NOT NULL,
    ai_level    INTEGER,
    tick_count  INTEGER NOT NULL,
    inputs      INTEGER NOT NULL,
    engine_sha  TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    json        TEXT NOT NULL,
    title       TEXT
);
CREATE INDEX IF NOT EXISTS idx_replays_created ON replays(created_at);
CREATE TABLE IF NOT EXISTS counters (
    name   TEXT PRIMARY KEY,
    value  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS players (
    name           TEXT PRIMARY KEY,
    mu             REAL    NOT NULL,
    sigma          REAL    NOT NULL,
    games          INTEGER NOT NULL DEFAULT 0,
    wins           INTEGER NOT NULL DEFAULT 0,
    losses         INTEGER NOT NULL DEFAULT 0,
    streak         INTEGER NOT NULL DEFAULT 0,
    streak_type    TEXT,
    high_score     INTEGER,
    high_lines     INTEGER,
    high_funds     INTEGER,
    fastest_kill   INTEGER,
    quickest_death INTEGER,
    longest_game   INTEGER
);";

fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    // Migration for DBs created before `title` existed. CREATE TABLE IF NOT
    // EXISTS won't add the column, so ALTER it in; ignore the duplicate-column
    // error when it's already there.
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN title TEXT", []);
    Ok(())
}

/// Open (creating if needed) the replay DB and ensure the schema exists. WAL +
/// `synchronous=NORMAL` suit a single always-on machine: durable across crashes,
/// readers never block the occasional writer.
fn open_db() -> Connection {
    let path = db_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let conn = Connection::open(&path).unwrap_or_else(|e| panic!("open replay db {path}: {e}"));
    // `execute_batch` ignores the row PRAGMA journal_mode returns.
    let _ = conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;");
    init_schema(&conn).expect("init replay schema");
    conn
}

/// Insert a recording (no-op if its content id already exists). Returns rows
/// affected (1 = newly stored, 0 = already present).
fn db_insert(conn: &Connection, id: &str, r: &Replay, json: &str, created_at: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO replays
            (id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            format!("{:?}", r.mode),
            r.seed,
            r.ai_level,
            r.tick_count,
            r.frames.len() as i64,
            r.engine_sha,
            created_at,
            json,
            r.title,
        ],
    )
}

/// Store a server-recorded online (`Versus`) match. Same `replays` table — mode
/// `"Online"`, `seed` = side A's seed; the `json` holds the full [`VersusReplay`]
/// (two seeds + the ordered input stream), which the playback page detects.
fn db_insert_versus(
    conn: &Connection,
    id: &str,
    r: &VersusReplay,
    json: &str,
    created_at: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO replays
            (id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        rusqlite::params![
            id,
            "Online",
            r.seed_a,
            Option::<u32>::None,
            r.tick_count,
            r.frames.len() as i64,
            r.engine_sha,
            created_at,
            json,
            r.title,
        ],
    )
}

/// Fetch a recording's JSON by id.
fn db_get(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT json FROM replays WHERE id = ?1", [id], |row| row.get::<_, String>(0))
        .optional()
}

/// Increment the persistent hit counter (the 90s "you are visitor #N" total) and
/// return the new value. Survives restarts because it lives in the same DB.
fn db_bump_hits(conn: &Connection) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO counters (name, value) VALUES ('hits', 1)
         ON CONFLICT(name) DO UPDATE SET value = value + 1",
        [],
    )?;
    Ok(db_hits(conn))
}

/// Read the persistent hit counter (0 if never bumped).
fn db_hits(conn: &Connection) -> i64 {
    conn.query_row("SELECT value FROM counters WHERE name = 'hits'", [], |row| row.get::<_, i64>(0))
        .optional()
        .ok()
        .flatten()
        .unwrap_or(0)
}

// --- per-player stats (the `players` table) -----------------------------
//
// One row per named player, accumulated at each settled bout: rating
// (mu/sigma), record (games/wins/losses), a running win/loss streak, the
// player's personal bests (high score/lines/funds), and three match-tick figures
// (fastest_kill / quickest_death / longest_game). `ratings.json` stays the
// rating source-of-truth for matchmaking; this table mirrors mu/sigma so the
// per-player profile endpoint is one indexed lookup.

/// A player's stored stats. `None` figures are "never recorded yet".
#[derive(Debug, Clone, PartialEq)]
struct PlayerStats {
    name: String,
    mu: f64,
    sigma: f64,
    games: i64,
    wins: i64,
    losses: i64,
    streak: i64,
    streak_type: Option<String>,
    high_score: Option<i64>,
    high_lines: Option<i64>,
    high_funds: Option<i64>,
    fastest_kill: Option<i64>,
    quickest_death: Option<i64>,
    longest_game: Option<i64>,
}

impl PlayerStats {
    /// A fresh, never-played record for `name` at the new-player rating.
    fn fresh(name: &str) -> PlayerStats {
        let base = Ts2Params::default().base.new_rating();
        PlayerStats {
            name: name.to_string(),
            mu: base.mu,
            sigma: base.sigma,
            games: 0,
            wins: 0,
            losses: 0,
            streak: 0,
            streak_type: None,
            high_score: None,
            high_lines: None,
            high_funds: None,
            fastest_kill: None,
            quickest_death: None,
            longest_game: None,
        }
    }
}

/// The per-match figures a settlement feeds into a player's row.
struct MatchStats {
    won: bool,
    score: i64,
    lines: i64,
    funds: i64,
    /// Match length in ticks (drives longest_game / fastest_kill / quickest_death).
    ticks: i64,
    /// True only for a natural finish (a real top-out). A forfeit/disconnect must
    /// not pollute the kill/death *timing* records with its arbitrary length.
    natural: bool,
}

/// `max(existing, v)` where an absent existing value takes `v`.
fn opt_max(existing: Option<i64>, v: i64) -> Option<i64> {
    Some(existing.map_or(v, |e| e.max(v)))
}

/// `min(existing, v)` where an absent existing value takes `v`.
fn opt_min(existing: Option<i64>, v: i64) -> Option<i64> {
    Some(existing.map_or(v, |e| e.min(v)))
}

/// Fetch a player's row, or `None` if they've never been recorded.
fn db_get_player(conn: &Connection, name: &str) -> rusqlite::Result<Option<PlayerStats>> {
    conn.query_row(
        "SELECT name, mu, sigma, games, wins, losses, streak, streak_type,
                high_score, high_lines, high_funds, fastest_kill, quickest_death, longest_game
         FROM players WHERE name = ?1",
        [name],
        |row| {
            Ok(PlayerStats {
                name: row.get(0)?,
                mu: row.get(1)?,
                sigma: row.get(2)?,
                games: row.get(3)?,
                wins: row.get(4)?,
                losses: row.get(5)?,
                streak: row.get(6)?,
                streak_type: row.get(7)?,
                high_score: row.get(8)?,
                high_lines: row.get(9)?,
                high_funds: row.get(10)?,
                fastest_kill: row.get(11)?,
                quickest_death: row.get(12)?,
                longest_game: row.get(13)?,
            })
        },
    )
    .optional()
}

/// Upsert a whole player row.
fn db_put_player(conn: &Connection, p: &PlayerStats) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO players
            (name, mu, sigma, games, wins, losses, streak, streak_type,
             high_score, high_lines, high_funds, fastest_kill, quickest_death, longest_game)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(name) DO UPDATE SET
            mu=?2, sigma=?3, games=?4, wins=?5, losses=?6, streak=?7, streak_type=?8,
            high_score=?9, high_lines=?10, high_funds=?11,
            fastest_kill=?12, quickest_death=?13, longest_game=?14",
        rusqlite::params![
            p.name, p.mu, p.sigma, p.games, p.wins, p.losses, p.streak, p.streak_type,
            p.high_score, p.high_lines, p.high_funds, p.fastest_kill, p.quickest_death, p.longest_game,
        ],
    )?;
    Ok(())
}

/// Fold one settled match into a player's row (loading the prior row or starting
/// fresh), updating record / streak / personal-bests / time figures, and persist.
/// `mu`/`sigma` are the player's POST-match rating (kept in sync with ratings.json).
fn db_record_player_stats(
    conn: &Connection,
    name: &str,
    mu: f64,
    sigma: f64,
    m: &MatchStats,
) -> rusqlite::Result<()> {
    let mut p = db_get_player(conn, name)?.unwrap_or_else(|| PlayerStats::fresh(name));
    p.mu = mu;
    p.sigma = sigma;
    p.games += 1;
    if m.won {
        p.wins += 1;
    } else {
        p.losses += 1;
    }
    // Streak: extend a same-result run, else start a new one of length 1.
    let this_type = if m.won { "wins" } else { "losses" };
    if p.streak_type.as_deref() == Some(this_type) {
        p.streak += 1;
    } else {
        p.streak = 1;
        p.streak_type = Some(this_type.to_string());
    }
    p.high_score = opt_max(p.high_score, m.score);
    p.high_lines = opt_max(p.high_lines, m.lines);
    p.high_funds = opt_max(p.high_funds, m.funds);
    p.longest_game = opt_max(p.longest_game, m.ticks);
    // Kill/death *timing* records only count real top-outs — a forfeit's length is
    // arbitrary and would otherwise log a bogus "fastest kill" / "quickest death".
    if m.natural {
        if m.won {
            p.fastest_kill = opt_min(p.fastest_kill, m.ticks);
        } else {
            p.quickest_death = opt_min(p.quickest_death, m.ticks);
        }
    }
    db_put_player(conn, &p)
}

/// One-time seed of the `players` table from `ratings.json` — only when the table
/// is empty, so it never clobbers accumulated stats. mu/sigma/experience map to
/// mu/sigma/games (the rating's experience IS its games-played count).
fn migrate_ratings_into_players(conn: &Connection, ratings: &HashMap<String, (f64, f64, u32)>) {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM players", [], |r| r.get(0))
        .unwrap_or(0);
    if count > 0 || ratings.is_empty() {
        return;
    }
    for (name, &(mu, sigma, exp)) in ratings {
        let mut p = PlayerStats::fresh(name);
        p.mu = mu;
        p.sigma = sigma;
        p.games = exp as i64;
        let _ = db_put_player(conn, &p);
    }
    println!("seeded players table from {} rating(s)", ratings.len());
}

/// The `GET /api/player/:name` JSON: stats + an Elo-styled figure from the
/// conservative rating (μ−3σ). An unknown player yields a fresh, zeroed record.
fn player_record_json(p: &PlayerStats) -> Value {
    let conservative = p.mu - 3.0 * p.sigma;
    json!({
        "name": p.name,
        "elo": elo_styled(conservative),
        "mu": p.mu,
        "sigma": p.sigma,
        "games": p.games,
        "wins": p.wins,
        "losses": p.losses,
        "streak": p.streak,
        "streak_type": p.streak_type,
        "high_score": p.high_score,
        "high_lines": p.high_lines,
        "high_funds": p.high_funds,
        "fastest_kill": p.fastest_kill,
        "quickest_death": p.quickest_death,
        "longest_game": p.longest_game,
    })
}

/// List recordings newest-first (capped) with just the metadata the library
/// browse page needs.
fn db_list(conn: &Connection, limit: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, title
         FROM replays ORDER BY created_at DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        let ai_level: Option<i64> = row.get(3)?;
        let title: Option<String> = row.get(8)?;
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "mode": row.get::<_, String>(1)?,
            "seed": row.get::<_, i64>(2)?,
            "ai_level": ai_level,
            "tick_count": row.get::<_, i64>(4)?,
            "inputs": row.get::<_, i64>(5)?,
            "engine_sha": row.get::<_, String>(6)?,
            "mtime": row.get::<_, i64>(7)?,
            "title": title,
        }))
    })?;
    rows.collect()
}

/// One-time migration: import any pre-existing on-disk replay JSON (the old file
/// store / a fly volume from before the DB) into the DB. Idempotent (`INSERT OR
/// IGNORE` by content id), best-effort — malformed files are skipped, and the
/// file's mtime is preserved as `created_at` so historical ordering survives.
fn import_dir(conn: &Connection, dir: &str) -> usize {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let mut imported = 0;
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let txt = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let r = match Replay::from_json(&txt) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let created_at = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_else(now_secs);
        if let Ok(n) = db_insert(conn, &replay_id(&txt), &r, &txt, created_at) {
            imported += n;
        }
    }
    imported
}

/// `POST /api/replays` — store a recording, return `{"id": "..."}`. Validates
/// it parses as a [`Replay`] first so we never persist junk.
async fn post_replay(State(db): State<Db>, body: String) -> impl IntoResponse {
    let replay = match Replay::from_json(&body) {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid replay").into_response(),
    };
    let id = replay_id(&body);
    let stored = {
        let conn = db.lock().unwrap();
        db_insert(&conn, &id, &replay, &body, now_secs())
    };
    match stored {
        Ok(_) => (StatusCode::OK, Json(json!({ "id": id }))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "store unavailable").into_response(),
    }
}

/// `GET /api/replays/:id` — fetch a stored recording as JSON.
async fn get_replay(State(db): State<Db>, Path(id): Path<String>) -> impl IntoResponse {
    if !valid_replay_id(&id) {
        return (StatusCode::BAD_REQUEST, "bad id").into_response();
    }
    let found = {
        let conn = db.lock().unwrap();
        db_get(&conn, &id).ok().flatten()
    };
    match found {
        Some(txt) => ([(header::CONTENT_TYPE, "application/json")], txt).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `GET /replay/:id` — the pretty, shareable playback link. Redirects to the
/// static player page, which fetches the recording from `/api/replays/:id`.
async fn replay_page(Path(id): Path<String>) -> impl IntoResponse {
    if !valid_replay_id(&id) {
        return (StatusCode::BAD_REQUEST, "bad id").into_response();
    }
    Redirect::temporary(&format!("/www/replay.html?id={id}")).into_response()
}

/// `GET /api/replays` — list stored recordings (newest first, capped) with just
/// enough metadata for the library browse page. Backs `library.html`.
async fn list_replays(State(db): State<Db>) -> impl IntoResponse {
    let replays = {
        let conn = db.lock().unwrap();
        db_list(&conn, 200).unwrap_or_default()
    };
    Json(json!({ "replays": replays })).into_response()
}

// --- leaderboard --------------------------------------------------------
//
// TrueSkill stays the rating engine; the leaderboard just presents it. Players
// rank by conservative skill (μ−3σ, the same number matchmaking trusts), shown
// as an Elo-styled figure — a cosmetic linear transform so the board reads like
// a familiar ladder rather than raw TrueSkill units.

/// Map conservative TrueSkill (μ−3σ, ~0 for a new player, rising as σ shrinks
/// with games played) onto an Elo-styled scale (new ≈ 1000, strong ≈ 1900).
fn elo_styled(conservative: f64) -> i64 {
    (1000.0 + conservative * 30.0).round().max(100.0) as i64
}

/// Rank players by conservative rating (descending), capped, as the JSON the
/// leaderboard page renders.
fn rank_players(ratings: &HashMap<String, (f64, f64, u32)>) -> Vec<Value> {
    let mut rows: Vec<(f64, Value)> = ratings
        .iter()
        .map(|(name, &(mu, sigma, games))| {
            let conservative = mu - 3.0 * sigma;
            (
                conservative,
                json!({
                    "name": name,
                    "elo": elo_styled(conservative),
                    "mu": mu,
                    "sigma": sigma,
                    "games": games,
                }),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    rows.into_iter().take(200).map(|(_, v)| v).collect()
}

/// `GET /api/leaderboard` — players ranked by conservative TrueSkill, Elo-styled.
/// Backs `leaderboard.html`.
async fn leaderboard(State(state): State<Shared>) -> impl IntoResponse {
    let players = {
        let app = state.lock().await;
        rank_players(&app.ratings)
    };
    Json(json!({ "players": players })).into_response()
}

/// `POST /api/identity` with `{"name":"<str>"}` — mints an HS256 identity token
/// `{"token":"<jwt>"}` carrying the sanitized name. Empty/whitespace names are
/// rejected; over-long names are capped to [`identity::MAX_NAME_LEN`].
async fn post_identity(body: String) -> impl IntoResponse {
    let name = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string));
    let name = match name.as_deref().and_then(identity::sanitize_name) {
        Some(n) => n,
        None => return (StatusCode::BAD_REQUEST, "name required").into_response(),
    };
    let token = identity::issue_token(&name);
    Json(json!({ "token": token })).into_response()
}

/// `GET /api/player/:name` — a player's stats. An unknown player returns a fresh,
/// zeroed record (200, not 404) so the lobby can show a brand-new player.
async fn player_profile(State(db): State<Db>, Path(name): Path<String>) -> impl IntoResponse {
    let stats = {
        let conn = db.lock().unwrap();
        db_get_player(&conn, &name).ok().flatten()
    }
    .unwrap_or_else(|| PlayerStats::fresh(&name));
    Json(player_record_json(&stats)).into_response()
}

#[tokio::main]
async fn main() {
    let conn = open_db();
    let imported = import_dir(&conn, &replays_dir());
    if imported > 0 {
        println!("imported {imported} replay(s) from {} into {}", replays_dir(), db_path());
    }
    // One DB handle, shared by the matchmaking/stats websocket (App) and the HTTP
    // replay/leaderboard handlers (AppState.db).
    let db: Db = Arc::new(std::sync::Mutex::new(conn));
    let state = AppState {
        app: Arc::new(Mutex::new(App::new(db.clone()))),
        db,
    };

    // Materialize the identity-token secret once now (so the per-process-random
    // fallback is fixed for the run, and a missing BT_JWT_SECRET is logged).
    let _ = identity::secret();
    if std::env::var("BT_JWT_SECRET").is_err() {
        println!("BT_JWT_SECRET unset — using a per-process-random token secret");
    }

    // Decay tick: re-broadcast the "players online" count as players go idle past
    // the 30s window (no client message triggers that, so the server must).
    {
        let app = state.app.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            loop {
                tick.tick().await;
                maybe_broadcast_stats(&mut *app.lock().await);
            }
        });
    }

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "bt-wasm".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/replays", post(post_replay).get(list_replays))
        .route("/api/replays/:id", get(get_replay))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/identity", post(post_identity))
        .route("/api/player/:name", get(player_profile))
        .route("/replay/:id", get(replay_page))
        .route("/", get(|| async { Redirect::permanent("/www/") }))
        .fallback_service(ServeDir::new(&static_dir))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("BattleTris server on http://{addr}  (static: {static_dir}, ws: /ws)");
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settling_a_win_raises_winner_lowers_loser() {
        let p = Ts2Params::default();
        let a = PlayerState::new(p.base.new_rating());
        let b = PlayerState::new(p.base.new_rating());
        let outcome = MatchOutcome { winner: Winner::A, a_lines: 30, b_lines: 12, a_quit: false, b_quit: false };
        let (na, nb) = rate_match(a, b, &outcome, &p);
        assert!(na.rating.conservative(3.0) > a.rating.conservative(3.0));
        assert!(nb.rating.mu < b.rating.mu);
        assert_eq!(na.experience, 1);
    }

    // --- matchmaking / settlement glue (the "between-the-game" layer) --------

    fn test_app() -> App {
        App {
            clients: HashMap::new(),
            waiting: Vec::new(),
            last_active: HashMap::new(),
            last_broadcast_players: 0,
            ratings: HashMap::new(),
            settled: HashSet::new(),
            params: Ts2Params::default(),
            db: std::sync::Arc::new(std::sync::Mutex::new(mem_db())),
            challenges: HashMap::new(),
            last_players: Vec::new(),
        }
    }

    /// Add a named client with NO lobby status (the pre-presence default): it
    /// only becomes a match candidate via the explicit `waiting` queue, so the
    /// `queue`-based matchmaking tests keep their old semantics. New presence
    /// tests set `status` explicitly.
    fn add_client(app: &mut App, id: u64, name: &str) -> mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        let state = app.rating_for(name);
        app.clients.insert(
            id,
            Client { name: name.to_string(), tx, peer: None, state, authoritative: false, bout: None, status: None },
        );
        rx
    }

    fn drained_types(rx: &mut mpsc::UnboundedReceiver<Message>) -> Vec<String> {
        let mut out = Vec::new();
        while let Ok(Message::Text(t)) = rx.try_recv() {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                if let Some(ty) = v.get("type").and_then(|x| x.as_str()) {
                    out.push(ty.to_string());
                }
            }
        }
        out
    }

    #[test]
    fn a_lone_player_is_queued_not_matched() {
        let mut app = test_app();
        let _rx = add_client(&mut app, 1, "solo");
        try_match(&mut app, 1);
        assert_eq!(app.waiting, vec![1], "queued while waiting for an opponent");
        assert_eq!(app.clients[&1].peer, None);
    }

    #[test]
    fn authoritative_pair_starts_a_bout_instead_of_webrtc() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, 1, "alice");
        let mut rx_b = add_client(&mut app, 2, "bob");
        app.clients.get_mut(&1).unwrap().authoritative = true;
        app.clients.get_mut(&2).unwrap().authoritative = true;

        assert!(try_match(&mut app, 1).is_none(), "alice queues, no opponent yet");
        let pending = try_match(&mut app, 2).expect("bob matches alice -> a hosted bout");

        assert_eq!((pending.id_a, pending.id_b), (1, 2), "first-queued is side A");
        assert!(
            app.clients[&1].bout.is_some() && app.clients[&2].bout.is_some(),
            "both clients are bound to the bout's input channel"
        );
        // Authoritative clients get `matchStart` (with a seed), NOT WebRTC `matched`.
        let ta = drained_types(&mut rx_a);
        let tb = drained_types(&mut rx_b);
        assert!(ta.contains(&"matchStart".to_string()) && !ta.contains(&"matched".to_string()));
        assert!(tb.contains(&"matchStart".to_string()));
    }


    #[tokio::test]
    async fn run_bout_ticks_and_broadcasts_snapshots_to_both_sides() {
        use std::sync::Arc;
        let (tx_a, mut rx_a) = mpsc::unbounded_channel::<Message>();
        let (tx_b, mut rx_b) = mpsc::unbounded_channel::<Message>();
        let (input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
        let app0 = test_app();
        let (state_a, state_b) = (app0.rating_for("alice"), app0.rating_for("bob"));
        let pb = PendingBout {
            match_id: 99, id_a: 1, id_b: 2, seed_a: 11, seed_b: 22,
            name_a: "alice".into(), name_b: "bob".into(), state_a, state_b,
            tx_a, tx_b, input_rx,
        };
        let shared: Shared = Arc::new(Mutex::new(test_app()));
        let handle = tokio::spawn(run_bout(shared, pb));

        // A legal input is accepted by the loop (no panic).
        let _ = input_tx.try_send((Side::A, Input::MoveLeft, 1));
        // Let several 16ms ticks elapse, then confirm snapshots reached both sides.
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(drained_types(&mut rx_a).iter().any(|t| t == "snapshot"), "A got snapshots");
        assert!(drained_types(&mut rx_b).iter().any(|t| t == "snapshot"), "B got snapshots");

        // Dropping both receivers ends the loop (snapshot sends fail).
        drop(rx_a);
        drop(rx_b);
        let _ = tokio::time::timeout(Duration::from_millis(300), handle).await;
    }

    #[test]
    fn settle_bout_rates_from_captured_identities_even_if_the_loser_left() {
        let mut app = test_app();
        // Only the winner is still connected; the loser already disconnected.
        let mut rx_win = add_client(&mut app, 1, "alice");
        let state_a = app.clients[&1].state;
        let state_b = app.rating_for("bob"); // bob is gone from app.clients

        settle_bout(&mut app, 100, 1, "alice", state_a, 2, "bob", state_b, true, 30, 12);

        assert!(drained_types(&mut rx_win).contains(&"rating".to_string()), "winner got a rating");
        assert!(
            app.ratings.contains_key("alice") && app.ratings.contains_key("bob"),
            "both ratings persisted by name despite the loser being gone"
        );
        // Idempotent per pair: a duplicate settle is a no-op.
        let before = app.ratings.get("alice").copied();
        settle_bout(&mut app, 100, 1, "alice", state_a, 2, "bob", state_b, true, 30, 12);
        assert_eq!(app.ratings.get("alice").copied(), before, "settle is idempotent");
    }

    #[test]
    fn settle_rates_once_notifies_both_and_is_idempotent() {
        std::env::set_var("RATINGS_FILE", std::env::temp_dir().join("bt_glue_ratings.json"));
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, 1, "alice");
        let mut rx_b = add_client(&mut app, 2, "bob");
        try_match(&mut app, 1);
        try_match(&mut app, 2);
        let _ = drained_types(&mut rx_a); // clear the 'matched' frames
        let _ = drained_types(&mut rx_b);

        let exp_before = app.clients[&1].state.experience;
        settle_result(&mut app, 1, true, 30, 12); // alice beats bob

        assert_eq!(app.clients[&1].state.experience, exp_before + 1, "rated exactly once");
        assert!(app.clients[&1].state.rating.conservative(3.0) > 0.0);
        assert!(drained_types(&mut rx_a).contains(&"rating".to_string()), "winner notified");
        assert!(drained_types(&mut rx_b).contains(&"rating".to_string()), "loser notified");

        // Settling the same pairing again must be a no-op (the dedup the relay
        // depends on when both clients report the result).
        let exp = app.clients[&1].state.experience;
        settle_result(&mut app, 2, false, 12, 30);
        assert_eq!(app.clients[&1].state.experience, exp, "second settle changes nothing");
    }

    #[test]
    fn ratings_round_trip_through_json() {
        let mut m = HashMap::new();
        m.insert("alice".to_string(), (29.4_f64, 7.1_f64, 3u32));
        let obj: Value = m
            .iter()
            .map(|(n, &(mu, s, e))| (n.clone(), json!({"mu": mu, "sigma": s, "experience": e})))
            .collect::<serde_json::Map<_, _>>()
            .into();
        let txt = serde_json::to_string(&obj).unwrap();
        let parsed: Value = serde_json::from_str(&txt).unwrap();
        let mu = parsed["alice"]["mu"].as_f64().unwrap();
        assert!((mu - 29.4).abs() < 1e-9);
    }

    #[test]
    fn replay_id_is_stable_and_content_addressed() {
        let a = r#"{"seed":1,"frames":[]}"#;
        let b = r#"{"seed":2,"frames":[]}"#;
        assert_eq!(replay_id(a), replay_id(a), "same content -> same id");
        assert_ne!(replay_id(a), replay_id(b), "different content -> different id");
        assert_eq!(replay_id(a).len(), 16);
    }

    #[test]
    fn replay_id_validation_blocks_path_traversal() {
        assert!(valid_replay_id("0123abcdef9876ff"));
        assert!(!valid_replay_id("../../etc/passwd"));
        assert!(!valid_replay_id("foo/bar"));
        assert!(!valid_replay_id(""));
        assert!(!valid_replay_id("zzzz")); // not hex
    }

    fn sample_replay(seed: u32, mode: bt_replay::Mode, ai_level: Option<u32>) -> Replay {
        Replay {
            version: bt_replay::REPLAY_VERSION,
            seed,
            mode,
            ai_level,
            dt_ms: 16,
            engine_sha: "abc1234".to_string(),
            tick_count: 100,
            frames: vec![bt_replay::Frame { tick: 5, input: bt_replay::Input::MoveLeft }],
            title: None,
        }
    }

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn replay_title_round_trips_through_the_db() {
        let conn = mem_db();
        let mut r = sample_replay(7, bt_replay::Mode::Practice, None);
        r.title = Some("The Gimp".to_string());
        let json = r.to_json();
        let id = replay_id(&json);
        db_insert(&conn, &id, &r, &json, 1234).unwrap();
        let list = db_list(&conn, 10).unwrap();
        assert_eq!(list[0]["title"], "The Gimp", "the library listing surfaces the title");
    }

    #[test]
    fn replay_db_insert_list_get_round_trip() {
        let conn = mem_db();
        let r = sample_replay(42, bt_replay::Mode::VsComputer, Some(7));
        let json = r.to_json();
        let id = replay_id(&json);

        assert_eq!(db_insert(&conn, &id, &r, &json, 1000).unwrap(), 1, "first insert stores a row");

        // get returns the exact JSON we stored.
        let got = db_get(&conn, &id).unwrap().expect("row present");
        assert_eq!(got, json);
        assert_eq!(Replay::from_json(&got).unwrap(), r, "round-trips back to the same Replay");

        // list surfaces the metadata the library page reads.
        let list = db_list(&conn, 200).unwrap();
        assert_eq!(list.len(), 1);
        let item = &list[0];
        assert_eq!(item["id"], id);
        assert_eq!(item["mode"], "VsComputer");
        assert_eq!(item["seed"], 42);
        assert_eq!(item["ai_level"], 7);
        assert_eq!(item["tick_count"], 100);
        assert_eq!(item["inputs"], 1);
        assert_eq!(item["engine_sha"], "abc1234");
        assert_eq!(item["mtime"], 1000);
    }

    #[test]
    fn replay_db_dedups_by_content_id() {
        let conn = mem_db();
        let r = sample_replay(1, bt_replay::Mode::Practice, None);
        let json = r.to_json();
        let id = replay_id(&json);
        assert_eq!(db_insert(&conn, &id, &r, &json, 10).unwrap(), 1);
        assert_eq!(db_insert(&conn, &id, &r, &json, 20).unwrap(), 0, "same content -> ignored");
        assert_eq!(db_list(&conn, 200).unwrap().len(), 1, "stored once");

        // Practice mode has no Ernie level -> ai_level surfaces as JSON null.
        assert!(db_list(&conn, 200).unwrap()[0]["ai_level"].is_null());
    }

    #[test]
    fn active_count_respects_30s_window() {
        let db: Db = Arc::new(std::sync::Mutex::new(mem_db()));
        let mut app = App::new(db);
        // Build instants relative to a base so adding durations never underflows.
        let base = Instant::now();
        let now = base + Duration::from_secs(100);
        app.last_active.insert(1, base + Duration::from_secs(99)); // 1s ago  -> active
        app.last_active.insert(2, base + Duration::from_secs(75)); // 25s ago -> active
        app.last_active.insert(3, base + Duration::from_secs(69)); // 31s ago -> idle
        app.last_active.insert(4, base); //                           100s ago -> idle
        assert_eq!(active_count(&app, now), 2, "only the two within 30s count");
    }

    #[test]
    fn hit_counter_persists_and_increments() {
        let conn = mem_db();
        assert_eq!(db_hits(&conn), 0, "no hits before any visit");
        assert_eq!(db_bump_hits(&conn).unwrap(), 1);
        assert_eq!(db_bump_hits(&conn).unwrap(), 2);
        assert_eq!(db_bump_hits(&conn).unwrap(), 3);
        assert_eq!(db_hits(&conn), 3, "reads back the running total");
    }

    #[test]
    fn leaderboard_ranks_by_conservative_descending() {
        let mut r = HashMap::new();
        r.insert("strong".to_string(), (30.0, 2.0, 50)); // μ−3σ = 24
        r.insert("rookie".to_string(), (25.0, 25.0 / 3.0, 0)); // μ−3σ = 0
        r.insert("mid".to_string(), (28.0, 5.0, 10)); // μ−3σ = 13
        let p = rank_players(&r);
        let names: Vec<&str> = p.iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["strong", "mid", "rookie"], "sorted by conservative skill");
        assert!(
            p[0]["elo"].as_i64().unwrap() > p[2]["elo"].as_i64().unwrap(),
            "stronger player has higher Elo"
        );
        assert_eq!(p[2]["elo"].as_i64().unwrap(), 1000, "a fresh player sits at ~1000");
        assert_eq!(p[0]["games"], 50);
    }

    #[test]
    fn replay_db_lists_newest_first() {
        let conn = mem_db();
        for (seed, created) in [(1u32, 100i64), (2, 300), (3, 200)] {
            let r = sample_replay(seed, bt_replay::Mode::Practice, None);
            let json = r.to_json();
            db_insert(&conn, &replay_id(&json), &r, &json, created).unwrap();
        }
        let list = db_list(&conn, 200).unwrap();
        let seeds: Vec<i64> = list.iter().map(|v| v["seed"].as_i64().unwrap()).collect();
        assert_eq!(seeds, vec![2, 3, 1], "ordered by created_at descending");
    }

    // --- presence / roster ---------------------------------------------------

    /// Mark a client authoritative + give it a lobby status (the presence path).
    fn set_present(app: &mut App, id: u64, status: Status) {
        let c = app.clients.get_mut(&id).unwrap();
        c.authoritative = true;
        c.status = Some(status);
    }

    #[test]
    fn roster_lists_only_named_clients_lowercase_and_sorted() {
        let mut app = test_app();
        let _r1 = add_client(&mut app, 1, "bob");
        let _r2 = add_client(&mut app, 2, "alice");
        let _r3 = add_client(&mut app, 3, ""); // anonymous -> never listed
        set_present(&mut app, 1, Status::Searching);
        set_present(&mut app, 2, Status::Available);

        let frame = players_msg(&roster(&app));
        let players = frame["players"].as_array().unwrap();
        assert_eq!(players.len(), 2, "the anonymous connection isn't listed");
        // Sorted by name -> alice before bob; statuses are lowercase wire strings.
        assert_eq!(players[0]["name"], "alice");
        assert_eq!(players[0]["status"], "available");
        assert_eq!(players[1]["name"], "bob");
        assert_eq!(players[1]["status"], "searching");
    }

    #[test]
    fn roster_dedupes_a_name_keeping_the_most_engaged_status() {
        let mut app = test_app();
        // Same name on two connections: one Available, one InGame -> show InGame.
        let _r1 = add_client(&mut app, 1, "dup");
        let _r2 = add_client(&mut app, 2, "dup");
        set_present(&mut app, 1, Status::Available);
        set_present(&mut app, 2, Status::InGame);
        let r = roster(&app);
        assert_eq!(r.len(), 1, "de-duped by name");
        assert_eq!(r[0], ("dup".to_string(), Status::InGame), "the busiest status wins");
    }

    #[test]
    fn maybe_broadcast_players_only_pushes_on_a_real_change() {
        let mut app = test_app();
        let mut rx = add_client(&mut app, 1, "alice");
        set_present(&mut app, 1, Status::Available);
        maybe_broadcast_players(&mut app);
        assert!(drained_types(&mut rx).contains(&"players".to_string()), "first roster pushed");
        // No change -> no re-broadcast.
        maybe_broadcast_players(&mut app);
        assert!(!drained_types(&mut rx).contains(&"players".to_string()), "no spurious re-push");
        // A status change -> pushed again.
        set_present(&mut app, 1, Status::Searching);
        maybe_broadcast_players(&mut app);
        assert!(drained_types(&mut rx).contains(&"players".to_string()), "change re-pushed");
    }

    // --- unified auto-pairing (Available is challengeable AND auto-pairable) ---

    #[test]
    fn two_available_clients_auto_pair_without_an_explicit_queue() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, 1, "alice");
        let mut rx_b = add_client(&mut app, 2, "bob");
        set_present(&mut app, 1, Status::Available);
        set_present(&mut app, 2, Status::Available);

        // alice goes through the matcher with NO one queued — bob is Available, so
        // they pair purely on presence.
        let pending = try_match(&mut app, 1).expect("two Available clients pair");
        assert_eq!((pending.id_a, pending.id_b), (2, 1), "bob (the candidate) is side A");
        assert_eq!(app.clients[&1].status, Some(Status::InGame));
        assert_eq!(app.clients[&2].status, Some(Status::InGame));
        assert!(drained_types(&mut rx_a).contains(&"matchStart".to_string()));
        assert!(drained_types(&mut rx_b).contains(&"matchStart".to_string()));
    }

    // --- directed challenge --------------------------------------------------

    #[test]
    fn challenge_accept_builds_a_directed_bout_and_sets_both_ingame() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, 1, "alice");
        let mut rx_b = add_client(&mut app, 2, "bob");
        set_present(&mut app, 1, Status::Available);
        set_present(&mut app, 2, Status::Available);

        // alice challenges bob (record the pending challenge as the handler would).
        let tid = find_named_available(&app, "bob").expect("bob is challengeable");
        assert_eq!(tid, 2);
        app.challenges.insert(1, (2, Instant::now() + CHALLENGE_TIMEOUT));

        // bob accepts -> a directed bout for (alice=A, bob=B); challenge cleared.
        let pending = start_bout(&mut app, 1, 2, None).expect("directed bout built");
        clear_challenge(&mut app, 1);
        assert_eq!((pending.id_a, pending.id_b), (1, 2), "challenger is side A");
        assert!(app.challenges.is_empty(), "pending challenge cleared on accept");
        assert_eq!(app.clients[&1].status, Some(Status::InGame));
        assert_eq!(app.clients[&2].status, Some(Status::InGame));
        // No quality figure for a hand-picked challenge.
        let ma: Vec<Value> = std::iter::from_fn(|| rx_a.try_recv().ok())
            .filter_map(|m| match m {
                Message::Text(t) => serde_json::from_str(&t).ok(),
                _ => None,
            })
            .collect();
        let start = ma.iter().find(|v| v["type"] == "matchStart").expect("matchStart");
        assert!(start["quality"].is_null(), "a challenge match carries no quality");
        let _ = &mut rx_b;
    }

    #[test]
    fn find_named_available_ignores_busy_or_offline_targets() {
        let mut app = test_app();
        let _r = add_client(&mut app, 1, "busy");
        set_present(&mut app, 1, Status::InGame); // in a match -> not challengeable
        assert!(find_named_available(&app, "busy").is_none(), "InGame isn't available");
        assert!(find_named_available(&app, "ghost").is_none(), "offline isn't available");
        set_present(&mut app, 1, Status::Available);
        assert_eq!(find_named_available(&app, "busy"), Some(1), "Available is challengeable");
    }

    // --- per-player stats (the players table) --------------------------------

    #[test]
    fn player_settlement_folds_record_streak_bests_and_times() {
        let conn = mem_db();
        // alice wins a 500-tick match with score 1200, 30 lines, 400 funds.
        let win = MatchStats { won: true, score: 1200, lines: 30, funds: 400, ticks: 500, natural: true };
        db_record_player_stats(&conn, "alice", 28.0, 5.0, &win).unwrap();
        let p = db_get_player(&conn, "alice").unwrap().unwrap();
        assert_eq!((p.games, p.wins, p.losses), (1, 1, 0));
        assert_eq!((p.streak, p.streak_type.as_deref()), (1, Some("wins")));
        assert_eq!(p.high_score, Some(1200));
        assert_eq!(p.high_lines, Some(30));
        assert_eq!(p.high_funds, Some(400));
        assert_eq!(p.longest_game, Some(500));
        assert_eq!(p.fastest_kill, Some(500), "a win sets fastest_kill");
        assert_eq!(p.quickest_death, None, "no loss yet");

        // A second win extends the streak; a faster kill + lower score don't regress
        // the bests; longest_game takes the max.
        let win2 = MatchStats { won: true, score: 800, lines: 50, funds: 100, ticks: 300, natural: true };
        db_record_player_stats(&conn, "alice", 29.0, 4.5, &win2).unwrap();
        let p = db_get_player(&conn, "alice").unwrap().unwrap();
        assert_eq!((p.streak, p.streak_type.as_deref()), (2, Some("wins")), "streak grows");
        assert_eq!(p.high_score, Some(1200), "high_score keeps the max");
        assert_eq!(p.high_lines, Some(50), "high_lines takes the new max");
        assert_eq!(p.fastest_kill, Some(300), "fastest_kill takes the min");
        assert_eq!(p.longest_game, Some(500), "longest_game keeps the max");
        assert!((p.mu - 29.0).abs() < 1e-9, "mu kept in sync");

        // A loss resets the streak to a fresh losing run and sets quickest_death.
        let loss = MatchStats { won: false, score: 50, lines: 2, funds: 0, ticks: 120, natural: true };
        db_record_player_stats(&conn, "alice", 27.5, 4.4, &loss).unwrap();
        let p = db_get_player(&conn, "alice").unwrap().unwrap();
        assert_eq!((p.games, p.wins, p.losses), (3, 2, 1));
        assert_eq!((p.streak, p.streak_type.as_deref()), (1, Some("losses")), "streak resets");
        assert_eq!(p.quickest_death, Some(120), "a loss sets quickest_death");
        assert_eq!(p.fastest_kill, Some(300), "fastest_kill untouched by a loss");
    }

    #[test]
    fn a_forfeit_records_the_result_but_not_kill_death_timing() {
        let conn = mem_db();
        // A forfeit win (natural=false) counts as a win + game, but its arbitrary
        // length must NOT set fastest_kill.
        let ff_win = MatchStats { won: true, score: 10, lines: 1, funds: 0, ticks: 7, natural: false };
        db_record_player_stats(&conn, "carol", 25.0, 8.0, &ff_win).unwrap();
        let p = db_get_player(&conn, "carol").unwrap().unwrap();
        assert_eq!((p.games, p.wins), (1, 1), "forfeit still counts as a win");
        assert_eq!(p.longest_game, Some(7), "length still feeds longest_game");
        assert_eq!(p.fastest_kill, None, "a forfeit must not set fastest_kill");

        // A forfeit loss likewise leaves quickest_death untouched.
        let ff_loss = MatchStats { won: false, score: 0, lines: 0, funds: 0, ticks: 9, natural: false };
        db_record_player_stats(&conn, "carol", 24.0, 8.0, &ff_loss).unwrap();
        let p = db_get_player(&conn, "carol").unwrap().unwrap();
        assert_eq!((p.wins, p.losses), (1, 1));
        assert_eq!(p.quickest_death, None, "a forfeit must not set quickest_death");
    }

    #[test]
    fn ratings_json_migrates_into_an_empty_players_table_once() {
        let conn = mem_db();
        let mut ratings = HashMap::new();
        ratings.insert("veteran".to_string(), (30.0_f64, 2.0_f64, 42u32));
        ratings.insert("rookie".to_string(), (25.0, 25.0 / 3.0, 0));

        migrate_ratings_into_players(&conn, &ratings);
        let vet = db_get_player(&conn, "veteran").unwrap().unwrap();
        assert!((vet.mu - 30.0).abs() < 1e-9 && (vet.sigma - 2.0).abs() < 1e-9);
        assert_eq!(vet.games, 42, "experience maps to games");
        assert_eq!(vet.wins, 0, "migration brings ratings, not a win/loss split");

        // A second migration is a no-op (table no longer empty) — it must not
        // clobber accumulated stats.
        let win = MatchStats { won: true, score: 1, lines: 1, funds: 1, ticks: 10, natural: true };
        db_record_player_stats(&conn, "veteran", 31.0, 1.9, &win).unwrap();
        migrate_ratings_into_players(&conn, &ratings);
        let vet = db_get_player(&conn, "veteran").unwrap().unwrap();
        assert_eq!(vet.games, 43, "migration didn't overwrite the post-game row");
        assert!((vet.mu - 31.0).abs() < 1e-9, "post-game rating preserved");
    }

    #[test]
    fn player_record_json_defaults_for_an_unknown_player() {
        let p = PlayerStats::fresh("nobody");
        let v = player_record_json(&p);
        assert_eq!(v["name"], "nobody");
        assert_eq!(v["games"], 0);
        assert_eq!(v["wins"], 0);
        assert_eq!(v["elo"], 1000, "a fresh player reads ~1000 Elo");
        assert!(v["high_score"].is_null(), "never-recorded bests are null");
        assert!(v["fastest_kill"].is_null());
        assert!(v["streak_type"].is_null());
    }
}
