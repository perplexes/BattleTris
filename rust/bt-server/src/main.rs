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
//!     {"type":"queue","token":"<jwt>"}      (Find Match; or {"name":"alice"})
//!     {"type":"available","value":true,"token":"<jwt>"}   (open to matches)
//!     {"type":"challenge","target":"bob"}   (directed challenge)
//!     {"type":"input","seq":N,"input":<bt_replay::Input>}   (a gameplay action)
//!     {"type":"rejoin","match_id":N,"token":"<jwt>"}   (reattach after a refresh)
//!     {"type":"leaveMatch"}                 (intentional leave -> immediate forfeit)
//!   server -> client:
//!     {"type":"matchStart","side":"A|B","seed":N,"opponent":"bob","match_id":N,...}
//!     {"type":"snapshot","tick":N,"ack":N,"result":...,"you":...,"opp":...,"keyframe"?:[..]}
//!     {"type":"rating","mu":...,"sigma":...,"conservative":...,"won":true}
//!     {"type":"players","players":[{"name":..,"status":"available|searching|ingame"}]}
//!     {"type":"opponentLeft"}
//!     {"type":"opponentReconnecting"}       (opponent dropped; bout frozen, grace)
//!     {"type":"opponentResumed"}            (opponent reattached; play resumes)
//!     {"type":"rejoinFailed"}               (no such live bout for this identity)
//!
//! Every connected client speaks the server-authoritative protocol (sends
//! `input`, renders `snapshot`); the server runs the only simulation. (The legacy
//! WebRTC P2P relay — `signal`/`matched` — was removed with that migration.)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRef, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use bt_core::versus::Side;
use bt_replay::{Input, Replay, ReplayPlayer, VersusReplay, VersusReplayPlayer};
use bt_trueskill::ts2::{rate_match, MatchOutcome, PlayerState, Ts2Params, Winner};
use bt_trueskill::{quality_1v1, Rating};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

/// Server-authoritative online match engine (the client-server migration).
mod bout;
mod metrics;
use bout::Bout;

/// Player identity (HS256 JWT) — now a shared crate so the bots can mint the
/// same tokens. Aliased to `identity` so existing `identity::…` call sites stand.
use bt_identity as identity;

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
    /// The opponent connection while in a match — used to send `opponentLeft` if
    /// this client disconnects mid-bout. Set in [`start_bout`], cleared at end.
    peer: Option<String>,
    state: PlayerState,
    /// While in a match: the channel to the match's tick loop and which side this
    /// client plays. `input` messages are forwarded here.
    bout: Option<(mpsc::Sender<BoutInput>, Side)>,
    /// The `match_id` of the bout this client is in (keys [`App::bouts`]), so an
    /// intentional `leaveMatch` can forfeit exactly this client's own bout. `None`
    /// outside a match. Set with `bout`, cleared when the bout ends.
    match_id: Option<String>,
    /// Lobby presence. `None` until the client establishes a name (anonymous
    /// connections don't appear in the roster). See [`Status`].
    status: Option<Status>,
    /// Optional geo label (e.g. "Tokyo") shown next to the name in the roster.
    /// Set from the `available` message; the region bots announce theirs.
    geo: Option<String>,
    /// True for a headless practice bot (announced via `available`'s `bot` flag).
    /// A bot is human-challengeable and human-auto-pairable, but two bots never
    /// auto-pair with EACH OTHER — otherwise they'd drain the lobby playing
    /// themselves and leave nobody for a visitor to challenge.
    is_bot: bool,
    /// Lobby presence captured at the start of the current bout, restored when it
    /// ends (see [`post_bout_status`]). `None` outside a bout.
    prev_status: Option<Status>,
    /// When the last ws Ping was sent to this client (to measure round-trip time on
    /// the matching Pong). `None` between a Pong and the next Ping.
    ping_sent_at: Option<Instant>,
    /// Last measured round-trip time, in ms — shown in the lobby roster (replacing
    /// the old geo label). `None` until the first Pong comes back.
    ping_ms: Option<u32>,
}

/// Presence to restore when a bout ends. A player who had ANY lobby presence
/// before the match (Available/Searching) returns to Available; a one-off
/// directed CHALLENGER (no prior presence) leaves the roster instead of being
/// forced Available — otherwise the moment the match ends they'd be auto-paired
/// straight back into another one (challenge a bot, top out, get re-matched with
/// a bot forever), which reads as the board "resetting" mid-session.
fn post_bout_status(prev: Option<Status>) -> Option<Status> {
    prev.map(|_| Status::Available)
}

/// Shared server state.
struct App {
    clients: HashMap<String, Client>,
    waiting: Vec<String>,
    /// Last time each connection pressed a gameplay button. "Players online" is
    /// the count of these within `ACTIVE_WINDOW` (set on `active`, pruned on
    /// disconnect). Only pages send `active`, so matchmaking sockets never appear.
    last_active: HashMap<String, Instant>,
    /// Last player count we broadcast, so the decay tick only re-broadcasts on a
    /// real change.
    last_broadcast_players: usize,
    /// Persisted ratings by player name: (mu, sigma, experience).
    ratings: HashMap<String, (f64, f64, u32)>,
    /// Pairings already rated (keyed by `match_id`).
    settled: HashSet<String>,
    params: Ts2Params,
    /// Replay/counters DB (shared with the HTTP handlers) — backs the hit counter
    /// and the per-player stats (`players`) table.
    db: Db,
    /// Outstanding directed challenges: challenger id -> (target id, expires_at).
    /// One in-flight challenge per challenger; superseded/cleared on
    /// accept/decline/timeout/disconnect.
    challenges: HashMap<String, (String, Instant)>,
    /// Last `players` roster we broadcast (the de-duped name/status/geo list), so
    /// the presence push only re-sends on a real change.
    last_players: Vec<RosterEntry>,
    /// Live authoritative bouts, keyed by `match_id`, so a reconnecting player's
    /// `rejoin` can reattach to the running tick loop. Inserted in [`start_bout`],
    /// removed when [`run_bout`] ends. See [`BoutHandle`].
    bouts: HashMap<String, BoutHandle>,
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
            bouts: HashMap::new(),
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

fn send(app: &App, id: &str, msg: &Value) {
    if let Some(c) = app.clients.get(id) {
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

/// One de-duped lobby row: (name, busiest status, optional ping RTT in ms, is_bot).
/// The ping is bucketed to 10ms so normal jitter doesn't spam roster re-broadcasts.
type RosterEntry = (String, Status, Option<u32>, bool);

/// The de-duped lobby roster: one entry per *named* client, keeping the
/// most-engaged status when a name has several connections (e.g. two tabs).
/// Sorted by name so the broadcast-on-change comparison is order-stable.
fn roster(app: &App) -> Vec<RosterEntry> {
    let bucket = |p: Option<u32>| p.map(|ms| (ms + 5) / 10 * 10);
    let mut by_name: HashMap<String, (Status, Option<u32>, bool)> = HashMap::new();
    for c in app.clients.values() {
        let (name, status) = match (c.name.as_str(), c.status) {
            (n, Some(s)) if !n.is_empty() => (n.to_string(), s),
            _ => continue, // anonymous / un-named connections aren't listed
        };
        by_name
            .entry(name)
            .and_modify(|cur| {
                // Keep the busiest connection's status; carry its ping + bot flag.
                if status.rank() > cur.0.rank() {
                    *cur = (status, bucket(c.ping_ms), c.is_bot);
                }
            })
            .or_insert((status, bucket(c.ping_ms), c.is_bot));
    }
    let mut out: Vec<RosterEntry> = by_name
        .into_iter()
        .map(|(name, (status, ping, bot))| (name, status, ping, bot))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The `players` presence frame the lobby renders. Each entry carries an optional
/// `ping` (round-trip latency in ms — shown next to the name) and, for bots, a
/// `bot:true` flag (the lobby ignores it; the roaming Count uses it to prefer
/// challenging humans).
fn players_msg(roster: &[RosterEntry]) -> Value {
    let players: Vec<Value> = roster
        .iter()
        .map(|(name, status, ping, bot)| {
            let mut o = json!({ "name": name, "status": status.as_str() });
            if let Some(p) = ping {
                o["ping"] = json!(p);
            }
            if *bot {
                o["bot"] = json!(true);
            }
            o
        })
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

/// How long a bout stays FROZEN after a human side's socket drops, waiting for that
/// player to reconnect (an accidental browser refresh) before the match is
/// forfeited. Long enough for a page reload (wasm re-init + ws reconnect), short
/// enough not to over-stall a genuine quit. A *bot* drop never waits — it forfeits
/// at once (bots don't refresh).
const REJOIN_GRACE: Duration = Duration::from_secs(12);

/// An out-of-band control message to a running bout's tick loop, separate from the
/// per-frame `input` stream. Sent by the `rejoin` handler when a dropped player
/// reconnects on a brand-new connection.
enum BoutControl {
    /// Reattach `side` to a freshly-connected client: swap in its writer `tx` and
    /// adopt its new connection `id` (the disconnected one was already removed from
    /// `app.clients`, so the loop must retarget settle/status by the live id).
    Reattach { side: Side, new_id: String, tx: mpsc::UnboundedSender<Message> },
    /// `side` intentionally left the match (the in-app "Leave game" button) — end
    /// the bout NOW, no reconnect grace, with the other side winning by forfeit.
    Forfeit { side: Side },
    /// A read-only spectator wants the live two-board stream (the debug live-match
    /// view). The loop adds `tx` to its spectator list; the spectator sends no
    /// inputs, so there's no anti-cheat surface.
    AddSpectator { tx: mpsc::UnboundedSender<Message> },
}

/// A handle to a live bout, kept in [`App::bouts`] keyed by `match_id` so the
/// `rejoin` handler can hand a reconnecting client straight back into the running
/// loop. Dropped (removed) when the bout ends.
struct BoutHandle {
    /// Deliver a [`BoutControl`] to the tick loop (reattach).
    control: mpsc::Sender<BoutControl>,
    /// A clone of the bout's input channel, given to a reconnecting client so its
    /// inputs flow to the same loop.
    input_tx: mpsc::Sender<BoutInput>,
    name_a: String,
    name_b: String,
}

/// Everything the async caller needs to spawn an authoritative match's tick
/// loop, handed back by [`try_match`] when it pairs two authoritative clients.
/// Player names + rating states are captured here so the bout can settle even
/// if a client has disconnected (and been removed from `app.clients`) by the
/// time the match ends.
struct PendingBout {
    /// Tagged-UUID match id (`match-<uuid>`) — unguessable + unique across restarts.
    /// Keys this match's settlement and [`App::bouts`].
    match_id: String,
    id_a: String,
    id_b: String,
    seed_a: u64,
    seed_b: u64,
    name_a: String,
    name_b: String,
    state_a: PlayerState,
    state_b: PlayerState,
    tx_a: mpsc::UnboundedSender<Message>,
    tx_b: mpsc::UnboundedSender<Message>,
    input_rx: mpsc::Receiver<BoutInput>,
    /// Out-of-band reattach messages from the `rejoin` handler (see [`BoutControl`]).
    control_rx: mpsc::Receiver<BoutControl>,
    /// Per side: is this a human (eligible for the reconnect grace freeze)? A bot
    /// drop forfeits at once. Index by side (A = 0, B = 1).
    human: [bool; 2],
}

/// A per-match seed from a connection id — distinct per connection (ids are random
/// uuids) without an rng dependency, and masked to 32 bits so it round-trips through
/// the JS client's `WasmGame::new(seed: u32)` exactly (same RNG stream on both sides
/// → client prediction agrees with the authoritative sim).
fn derive_seed(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    h.finish() & 0xFFFF_FFFF
}

/// How long a directed challenge stays open before the challenger is told the
/// target declined (no response in time).
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Find the connected client whose name == `name`, preferring one currently
/// Available (the challengeable connection) over any other tab. Returns its id.
fn find_named_available(app: &App, name: &str) -> Option<String> {
    // First pass: an Available named connection (the one a challenge can land on).
    if let Some((id, _)) = app
        .clients
        .iter()
        .find(|(_, c)| c.name == name && c.status == Some(Status::Available))
    {
        return Some(id.clone());
    }
    None
}

/// Drop a challenger's pending challenge (if any). Returns the target id it was
/// pointed at, so the caller can decide whether to notify.
fn clear_challenge(app: &mut App, challenger: &str) -> Option<String> {
    app.challenges.remove(challenger).map(|(target, _)| target)
}

/// Is `cid` a candidate the matcher may pair `id` with right now? Not `id`
/// itself, not already in a bout, and "open to matches" — either explicitly
/// queued (in `waiting`, the Find-Match path) OR lobby-Available / -Searching
/// (the unified presence switch: Available = challengeable AND auto-pairable).
fn is_match_candidate(app: &App, id: &str, cid: &str) -> bool {
    if cid == id {
        return false;
    }
    // The auto-matcher pairs only two HUMANS who are both open to play. Bots are
    // passive: a human reaches a regional bot by CHALLENGING it, and The Count
    // reaches humans via its own roaming challenges — neither side auto-pairs INTO a
    // bot. (So an open human stays available for The Count to challenge instead of
    // being instantly thrown into a regional bot.)
    if app.clients.get(id).is_some_and(|c| c.is_bot)
        || app.clients.get(cid).is_some_and(|c| c.is_bot)
    {
        return false;
    }
    match app.clients.get(cid) {
        Some(c) => {
            c.bout.is_none()
                && (app.waiting.iter().any(|w| w == cid)
                    || matches!(c.status, Some(Status::Available) | Some(Status::Searching)))
        }
        None => false,
    }
}

/// Match `id` against the best-quality open opponent; otherwise leave it queued.
///
/// The candidate pool is everyone "open to matches" — explicitly queued
/// (`waiting`) AND lobby-Available clients — so going Available auto-pairs you
/// just like pressing Find Match. Returns `Some(PendingBout)` (which the async
/// caller spawns) when it finds an opponent.
fn try_match(app: &mut App, id: &str) -> Option<PendingBout> {
    let my_rating = app.clients.get(id).map(|c| c.state.rating)?;
    // Already in a bout? Don't re-match (would clobber the live binding).
    if app.clients.get(id).is_some_and(|c| c.bout.is_some()) {
        return None;
    }

    // Scan all open candidates (queued or Available/Searching), de-duped.
    let candidates: Vec<String> = app
        .clients
        .keys()
        .cloned()
        .filter(|cid| is_match_candidate(app, id, cid))
        .collect();
    let mut best: Option<(String, f64)> = None;
    for cid in candidates {
        if let Some(other) = app.clients.get(&cid) {
            let q = quality_1v1(my_rating, other.state.rating, &app.params.base);
            if best.as_ref().map(|(_, bq)| q > *bq).unwrap_or(true) {
                best = Some((cid, q));
            }
        }
    }

    let (opp, quality) = match best {
        Some(x) => x,
        None => {
            if !app.waiting.iter().any(|w| w == id) {
                app.waiting.push(id.to_string());
            }
            return None;
        }
    };

    // opp = side A (first queued/available), id = side B.
    start_bout(app, &opp, id, Some(quality))
}

/// Build a server-authoritative bout between two connected clients: drop them
/// from the queue, link peers, set both `InGame`, bind the input channel, send
/// `matchStart`, and return the [`PendingBout`] the caller spawns. Shared by the
/// auto-matcher ([`try_match`]) and the directed-challenge path. `quality` is the
/// matchmaking quality if known (auto-match) else `None` (a hand-picked challenge
/// has no quality figure). Returns `None` only if a client vanished.
fn start_bout(app: &mut App, a: &str, b: &str, quality: Option<f64>) -> Option<PendingBout> {
    app.waiting.retain(|w| w != a && w != b);
    // Drop any pending challenges involving either player — they're now in a
    // match, so a stale accept must not later kick off a second, unwanted bout.
    app.challenges
        .retain(|cid, (tid, _)| cid != a && cid != b && tid != a && tid != b);
    if let Some(c) = app.clients.get_mut(a) {
        c.peer = Some(b.to_string());
    }
    if let Some(c) = app.clients.get_mut(b) {
        c.peer = Some(a.to_string());
    }

    let (a_name, a_state) = match app.clients.get(a) {
        Some(c) => (c.name.clone(), c.state),
        None => return None,
    };
    let (b_name, b_state) = match app.clients.get(b) {
        Some(c) => (c.name.clone(), c.state),
        None => return None,
    };

    // Mint the match id BEFORE the matchStart sends so each side learns it up front
    // (the client parks it in its URL as `?match=<id>` for rejoin-on-refresh).
    let match_id = new_id("match");
    // Which sides are human? A bot drop forfeits instantly (no reconnect grace).
    let human_a = !app.clients.get(a).is_some_and(|c| c.is_bot);
    let human_b = !app.clients.get(b).is_some_and(|c| c.is_bot);

    let (seed_a, seed_b) = (derive_seed(a), derive_seed(b));
    let (input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
    // The reattach control channel: the `rejoin` handler delivers a fresh socket to
    // the running tick loop through here (see [`BoutControl`]).
    let (control_tx, control_rx) = mpsc::channel::<BoutControl>(4);
    let tx_a = app.clients[a].tx.clone();
    let tx_b = app.clients[b].tx.clone();
    if let Some(c) = app.clients.get_mut(a) {
        c.bout = Some((input_tx.clone(), Side::A));
        c.match_id = Some(match_id.clone());
        c.prev_status = c.status; // remember presence to restore at bout end
        c.status = Some(Status::InGame);
    }
    if let Some(c) = app.clients.get_mut(b) {
        c.bout = Some((input_tx.clone(), Side::B));
        c.match_id = Some(match_id.clone());
        c.prev_status = c.status;
        c.status = Some(Status::InGame);
    }
    // Register the live bout so a reconnecting player can find it by `match_id`.
    app.bouts.insert(
        match_id.clone(),
        BoutHandle { control: control_tx, input_tx, name_a: a_name.clone(), name_b: b_name.clone() },
    );
    // quality is optional in the wire frame; a directed challenge omits it.
    let qv = quality.map(|q| json!(q)).unwrap_or(Value::Null);
    // Each side's matchStart carries the OPPONENT's Elo, so a rating-matched bot
    // (The Count) can dial its difficulty to the player it's facing.
    let a_elo = elo_styled(a_state.rating.conservative(3.0));
    let b_elo = elo_styled(b_state.rating.conservative(3.0));
    send(app, a, &json!({"type":"matchStart","side":"A","seed":seed_a,"opponent":b_name,"opp_elo":b_elo,"quality":qv,"match_id":match_id.clone()}));
    send(app, b, &json!({"type":"matchStart","side":"B","seed":seed_b,"opponent":a_name,"opp_elo":a_elo,"quality":qv,"match_id":match_id.clone()}));
    metrics::METRICS.matches.inc();
    match quality {
        Some(q) => println!("authoritative match {a} <-> {b} (quality {q:.3})"),
        None => println!("authoritative challenge match {a} <-> {b}"),
    }
    // Roster changed (both went InGame) — let the lobby reflect it.
    maybe_broadcast_players(app);
    Some(PendingBout {
        match_id,
        id_a: a.to_string(),
        id_b: b.to_string(),
        seed_a,
        seed_b,
        name_a: a_name,
        name_b: b_name,
        state_a: a_state,
        state_b: b_state,
        tx_a,
        tx_b,
        input_rx,
        control_rx,
        human: [human_a, human_b],
    })
}

/// Settle an authoritative match from the bout's OWN captured player identities
/// (not live `app.clients`, which may have lost the loser on a forfeit
/// disconnect). Idempotent per pair; rating messages go to whoever is still
/// connected.
#[allow(clippy::too_many_arguments)]
fn settle_bout(
    app: &mut App,
    match_id: &str,
    id_a: &str,
    name_a: &str,
    state_a: PlayerState,
    id_b: &str,
    name_b: &str,
    state_b: PlayerState,
    a_won: bool,
    a_lines: u32,
    b_lines: u32,
) {
    // Key on the unique match id, not min(id), so a connection's later match
    // (same min-id) isn't wrongly skipped. run_bout calls this once per match,
    // so the guard is purely defensive.
    if !app.settled.insert(match_id.to_string()) {
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
    if let Some(c) = app.clients.get_mut(id_a) {
        c.state = na;
    }
    if let Some(c) = app.clients.get_mut(id_b) {
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

/// Map a [`Side`] to a 0/1 index for per-side arrays (A = 0, B = 1).
#[inline]
fn sidx(s: Side) -> usize {
    match s {
        Side::A => 0,
        Side::B => 1,
    }
}

/// The per-match authoritative tick loop. Advances the deterministic engine on
/// the server's clock, broadcasts a snapshot to each client (~30Hz), and settles
/// on the natural end.
///
/// **Reconnect grace (pause-both).** A *human* side's socket dropping no longer
/// forfeits instantly: the whole bout FREEZES (sim + inputs halt) for up to
/// [`REJOIN_GRACE`] while we wait for that player to reconnect (an accidental
/// refresh) and reattach via [`BoutControl::Reattach`] (the `rejoin` handler). The
/// still-connected side is told `opponentReconnecting`; on reattach both get a
/// keyframe + `opponentResumed` and play resumes from the exact frozen state. If
/// the grace expires the absent side forfeits. A *bot* side dropping still
/// forfeits at once (bots don't refresh).
async fn run_bout(state: Shared, pb: PendingBout) {
    let PendingBout {
        match_id, mut id_a, mut id_b, seed_a, seed_b, name_a, name_b, state_a, state_b,
        mut tx_a, mut tx_b, mut input_rx, mut control_rx, human,
    } = pb;
    let mut bout = Bout::new(seed_a, seed_b);
    let mut ticker = tokio::time::interval(Duration::from_millis(bout::TICK_MS as u64));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // `Some(a_won)` = settle with A as winner/loser; `None` = both gone, nothing to rate.
    let mut frame: u64 = 0;
    let mut want_keyframe = true; // send one on the very first frame
    let mut connected = [true, true]; // is each side's socket currently attached?
    let mut spectators: Vec<mpsc::UnboundedSender<Message>> = Vec::new(); // live-view watchers
    let mut grace_until: Option<Instant> = None; // freeze deadline while a human reconnects
    let outcome: Option<bool> = loop {
        ticker.tick().await;

        // ── Control: reattach a reconnecting player, or an intentional leave ─────
        let mut forfeit_by: Option<Side> = None;
        while let Ok(ctrl) = control_rx.try_recv() {
            match ctrl {
                BoutControl::Reattach { side, new_id, tx } => {
                    println!("bout {match_id}: reattach side {side:?} new_id {new_id}");
                    match side {
                        Side::A => { tx_a = tx.clone(); id_a = new_id; }
                        Side::B => { tx_b = tx.clone(); id_b = new_id; }
                    }
                    connected[sidx(side)] = true;
                    // The fresh client needs the match handoff, THEN a keyframe to resync.
                    let (opp, seed, side_str) = match side {
                        Side::A => (&name_b, seed_a, "A"),
                        Side::B => (&name_a, seed_b, "B"),
                    };
                    let _ = tx.send(Message::Text(
                        json!({"type":"matchStart","side":side_str,"seed":seed,"opponent":opp,"match_id":match_id}).to_string(),
                    ));
                    let _ = tx.send(Message::Text(bout.snapshot_message(side, true)));
                    // Both back? Lift the freeze, resync both boards, tell both to resume.
                    if connected[0] && connected[1] {
                        grace_until = None;
                        let resumed = json!({"type":"opponentResumed"}).to_string();
                        let _ = tx_a.send(Message::Text(bout.snapshot_message(Side::A, true)));
                        let _ = tx_a.send(Message::Text(resumed.clone()));
                        let _ = tx_b.send(Message::Text(bout.snapshot_message(Side::B, true)));
                        let _ = tx_b.send(Message::Text(resumed));
                        want_keyframe = true;
                    }
                }
                BoutControl::Forfeit { side } => forfeit_by = Some(side),
                BoutControl::AddSpectator { tx } => {
                    // Send the current state immediately so the watcher isn't blank
                    // until the next broadcast, then keep them in the stream.
                    if tx.send(Message::Text(bout.spectator_message(&name_a, &name_b))).is_ok() {
                        spectators.push(tx);
                    }
                }
            }
        }
        if let Some(side) = forfeit_by {
            // Intentional leave: the leaver loses now, no grace. The settle section
            // tells the still-attached side `opponentLeft` (shared with the grace-
            // expiry path), so we just end the loop here.
            break Some(side == Side::B); // A left -> B wins (false); B left -> A wins (true)
        }

        // ── Frozen (a human side is mid-reconnect): hold the sim, watch the clock ─
        if !(connected[0] && connected[1]) {
            // `grace_until` is always set here (only a human drop pauses; a bot drop
            // forfeits). On expiry the absent side forfeits to whoever remains.
            if grace_until.is_some_and(|d| Instant::now() >= d) {
                println!("bout {match_id}: grace expired (connected={connected:?}) -> forfeit");
                break match (connected[0], connected[1]) {
                    (true, false) => Some(true),  // A stayed -> A wins
                    (false, true) => Some(false), // B stayed -> B wins
                    _ => None,                    // nobody came back
                };
            }
            // Probe any still-connected side (~2Hz) so the expiry verdict stays
            // accurate if it ALSO drops while we wait (the client ignores this).
            if frame % 32 == 0 {
                let hb = json!({"type":"heartbeat"}).to_string();
                if connected[0] { connected[0] = tx_a.send(Message::Text(hb.clone())).is_ok(); }
                if connected[1] { connected[1] = tx_b.send(Message::Text(hb)).is_ok(); }
            }
            frame = frame.wrapping_add(1);
            continue;
        }

        // ── Normal tick (both sides attached) ────────────────────────────────────
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
            connected = [a_ok, b_ok];
            if !a_ok || !b_ok {
                // A *bot* dropping (no human is down) forfeits immediately. A human
                // dropping starts the freeze and waits for a reconnect.
                let human_down = (!a_ok && human[0]) || (!b_ok && human[1]);
                if !human_down {
                    // No human is down — only a bot dropped (incl. a bot-vs-bot match,
                    // e.g. The Count vs a regional bot). Forfeit immediately; the side
                    // still attached wins. (If A is down, a_ok=false -> Some(false)=B won.)
                    break Some(a_ok);
                }
                if grace_until.is_none() {
                    grace_until = Some(Instant::now() + REJOIN_GRACE);
                    println!("bout {match_id}: human drop (connected={connected:?}) -> freeze {}s", REJOIN_GRACE.as_secs());
                }
                // Carry the grace length so the connected side can show a countdown
                // to the forfeit (the server stays the authority on the actual end).
                let msg = json!({
                    "type": "opponentReconnecting",
                    "grace_secs": REJOIN_GRACE.as_secs(),
                })
                .to_string();
                if a_ok { let _ = tx_a.send(Message::Text(msg.clone())); }
                if b_ok { let _ = tx_b.send(Message::Text(msg)); }
                // Next iteration sees `connected` partial and enters the freeze.
            }
        }
        // Stream the read-only two-board view to spectators (~15Hz). A failed send
        // means the watcher closed the tab — drop them. No effect on the players.
        if frame % 4 == 0 && !spectators.is_empty() {
            let msg = bout.spectator_message(&name_a, &name_b);
            spectators.retain(|tx| tx.send(Message::Text(msg.clone())).is_ok());
        }
        frame += 1;
    };

    let mut app = state.lock().await;
    // The bout is over — drop its reattach registry entry so a late `rejoin` for
    // this match cleanly fails (`rejoinFailed`) instead of finding a dead loop.
    app.bouts.remove(&match_id);
    // Match over: both sides leave the bout and return to their PRE-match
    // presence (see [`post_bout_status`]) — a player who was "open to matches"
    // goes back to Available; a one-off directed challenger leaves the roster
    // rather than being force-Available and instantly auto-rematched.
    for cid in [&id_a, &id_b] {
        if let Some(c) = app.clients.get_mut(cid) {
            c.bout = None;
            c.match_id = None;
            c.peer = None;
            if !c.name.is_empty() {
                c.status = post_bout_status(c.prev_status);
            }
            c.prev_status = None;
        }
    }
    if let Some(a_won) = outcome {
        // A forfeit (an intentional leave OR a grace-window expiry — anything that
        // ISN'T a natural top-out) doesn't latch a game-over on the winner's client
        // by itself, and the winner may be sitting on the "opponent reconnecting"
        // freeze. Tell whoever's still attached the opponent left. (The loser's tx
        // is dead on a disconnect → a harmless no-op; on an intentional leave the
        // leaver already went to the lobby and ignores it.)
        if !bout.is_over() {
            let left = json!({ "type": "opponentLeft" }).to_string();
            let _ = tx_a.send(Message::Text(left.clone()));
            let _ = tx_b.send(Message::Text(left));
        }
        settle_bout(
            &mut app, &match_id, &id_a, &name_a, state_a, &id_b, &name_b, state_b,
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
                .and_then(|conn| {
                    let total_lines = bout.lines(Side::A) as i64 + bout.lines(Side::B) as i64;
                    db_insert_versus(&conn, &id, &replay, &json, now_secs(), &name_a, &name_b, total_lines).ok()
                })
                .is_some();
            if stored {
                println!("stored online replay {id} ({} ticks)", replay.tick_count);
            }
            // Tell both clients the replay id so the game-over screen can offer a
            // "Watch replay" button (best-effort — a disconnected client ignores it).
            let msg = json!({ "type": "matchReplay", "id": id }).to_string();
            let _ = tx_a.send(Message::Text(msg.clone()));
            let _ = tx_b.send(Message::Text(msg));
        }
    }
    // Both players went back to Available (or left) — refresh the lobby roster.
    maybe_broadcast_players(&mut app);
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

async fn handle_message(state: &Shared, id: &str, text: &str) {
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
            app.last_active.insert(id.to_string(), Instant::now());
            maybe_broadcast_stats(&mut app);
        }
        Some("queue") => {
            // Identity: prefer the token's signed name; a bare `name` is still
            // accepted (back-compat) but the token wins when both are present.
            let pending = {
                let mut app = state.lock().await;
                // Ignore a re-queue from a client already in a match (it would
                // otherwise be matched into a second bout that could clobber this
                // one's binding / settle the wrong pairing).
                if app.clients.get(id).is_some_and(|c| c.bout.is_some()) {
                    None
                } else {
                    let prior = app.clients.get(id).map(|c| c.name.clone()).unwrap_or_default();
                    let name = resolve_name(&app, &v, &prior);
                    let name = if name.is_empty() { "anon".to_string() } else { name };
                    let st = app.rating_for(&name);
                    if let Some(c) = app.clients.get_mut(id) {
                        c.name = name;
                        c.state = st;
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
                if app.clients.get(id).is_some_and(|c| c.bout.is_some()) {
                    None // already in a match — ignore
                } else {
                    let prior = app.clients.get(id).map(|c| c.name.clone()).unwrap_or_default();
                    let name = resolve_name(&app, &v, &prior);
                    let go_available = value && !name.is_empty();
                    let st = app.rating_for(&name);
                    // Optional presence decoration: a geo label and a bot flag (the
                    // region bots set both). geo shows in the roster; the bot flag
                    // keeps two bots from auto-pairing each other.
                    let geo = v.get("geo").and_then(|g| g.as_str())
                        .map(|s| s.chars().take(24).collect::<String>());
                    let is_bot = v.get("bot").and_then(|b| b.as_bool()).unwrap_or(false);
                    if !go_available {
                        // Going unavailable / un-named -> leave the roster + queue.
                        app.waiting.retain(|w| w != id);
                    }
                    if let Some(c) = app.clients.get_mut(id) {
                        c.name = name;
                        c.state = st;
                        c.status = go_available.then_some(Status::Available);
                        if geo.is_some() {
                            c.geo = geo;
                        }
                        c.is_bot = is_bot;
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
            let prior = app.clients.get(id).map(|c| c.name.clone()).unwrap_or_default();
            let from = resolve_name(&app, &v, &prior);
            if let Some(c) = app.clients.get_mut(id) {
                c.name = from.clone();
            }
            let target = v.get("target").and_then(|t| t.as_str()).unwrap_or("").to_string();
            match find_named_available(&app, &target) {
                // NB bots CAN be challenged (The Count duels the regional bots when it
                // gets bored of humans); only auto-pairing two bots is blocked.
                Some(tid) if tid != id => {
                    let deadline = Instant::now() + CHALLENGE_TIMEOUT;
                    app.challenges.insert(id.to_string(), (tid.clone(), deadline));
                    send(&app, &tid, &json!({"type":"challenged","from":from}));
                    // Schedule the timeout: if still pending after the window, tell
                    // the challenger the target "declined" (no answer). Compare the
                    // exact deadline so an earlier timer can't clear a *re-issued*
                    // challenge to the same target. The spawned task is 'static, so
                    // capture OWNED ids (the handler's `id` is a borrow).
                    let state2 = state.clone();
                    let id_owned = id.to_string();
                    tokio::spawn(async move {
                        tokio::time::sleep(CHALLENGE_TIMEOUT).await;
                        let mut app = state2.lock().await;
                        if let Some((t, d)) = app.challenges.get(&id_owned).cloned() {
                            if t == tid && d == deadline {
                                clear_challenge(&mut app, &id_owned);
                                let by = app.clients.get(&tid).map(|c| c.name.clone()).unwrap_or(target);
                                send(&app, &id_owned, &json!({"type":"challengeDeclined","by":by}));
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
                let challenger = app.challenges.iter().find_map(|(cid, (tid, _))| {
                    (tid.as_str() == id && app.clients.get(cid).is_some_and(|c| c.name == from))
                        .then(|| cid.clone())
                });
                match challenger {
                    Some(cid)
                        if app.clients.get(&cid).is_some_and(|c| c.bout.is_none())
                            && app.clients.get(id).is_some_and(|c| c.bout.is_none()) =>
                    {
                        clear_challenge(&mut app, &cid);
                        // Challenger = side A, accepter = side B (no quality figure).
                        start_bout(&mut app, &cid, id, None)
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
            let challenger = app.challenges.iter().find_map(|(cid, (tid, _))| {
                (tid.as_str() == id && app.clients.get(cid).is_some_and(|c| c.name == from))
                    .then(|| cid.clone())
            });
            if let Some(cid) = challenger {
                clear_challenge(&mut app, &cid);
                let by = app.clients.get(id).map(|c| c.name.clone()).unwrap_or_default();
                send(&app, &cid, &json!({"type":"challengeDeclined","by":by}));
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
                if let Some((tx, side)) = app.clients.get(id).and_then(|c| c.bout.as_ref()) {
                    // try_send: drop under flood rather than grow memory (bounded channel).
                    let _ = tx.try_send((*side, input, seq));
                }
            }
        }
        // Reconnect after an accidental refresh: the client parked the match in its
        // URL (`?match=<id>`) and now reattaches to the still-running, frozen bout.
        // Requires a signed token whose name is one of the two participants — so a
        // shared/stale link can't hijack someone else's match (it just fails).
        Some("rejoin") => {
            // The match id is a tagged-UUID string (`match-<uuid>`), carried in the
            // client's `?match=<id>` URL.
            let mid = v.get("match_id").and_then(|m| m.as_str()).map(|s| s.to_string());
            let mut app = state.lock().await;
            let prior = app.clients.get(id).map(|c| c.name.clone()).unwrap_or_default();
            let name = resolve_name(&app, &v, &prior);
            // Find the live bout and which side this identity plays.
            let found = mid.clone().and_then(|mid| {
                app.bouts.get(&mid).and_then(|h| {
                    let side = if !name.is_empty() && h.name_a == name {
                        Some(Side::A)
                    } else if !name.is_empty() && h.name_b == name {
                        Some(Side::B)
                    } else {
                        None
                    };
                    side.map(|s| (s, h.control.clone(), h.input_tx.clone()))
                })
            });
            let ok = match found {
                Some((side, control, input_tx)) => {
                    // Hand this fresh socket to the running loop (it sends the
                    // matchStart handoff + a keyframe). try_send (not await) so we
                    // never hold the lock across a send; capacity 4 vs a one-shot
                    // reattach means it only fails if the loop already ended.
                    let sent = match app.clients.get(id).map(|c| c.tx.clone()) {
                        Some(tx) => {
                            control.try_send(BoutControl::Reattach { side, new_id: id.to_string(), tx }).is_ok()
                        }
                        None => false,
                    };
                    if sent {
                        let st = app.rating_for(&name);
                        if let Some(c) = app.clients.get_mut(id) {
                            c.name = name;
                            c.state = st;
                            c.prev_status = c.status; // restore presence at bout end
                            c.status = Some(Status::InGame);
                            c.bout = Some((input_tx, side));
                            c.match_id = mid.clone(); // mid is Some here (found matched)
                        }
                        maybe_broadcast_players(&mut app); // back to InGame in the roster
                    }
                    sent
                }
                None => false,
            };
            println!("rejoin id={id} match_id={mid:?} -> {}", if ok { "ok" } else { "failed" });
            if !ok {
                // No such live bout / not a participant / the loop just ended — fail
                // loudly so the client clears the URL and returns to the lobby.
                send(&app, id, &json!({"type":"rejoinFailed"}));
            }
        }
        // Intentional in-app "Leave game": forfeit THIS client's own bout right away
        // (no reconnect grace — that's only for an accidental socket drop). We use
        // the server-side `match_id`/`side` binding (not anything client-supplied),
        // so a client can only forfeit the match it's actually in.
        Some("leaveMatch") => {
            let app = state.lock().await;
            let target = app.clients.get(id).and_then(|c| match (c.match_id.as_ref(), c.bout.as_ref()) {
                (Some(mid), Some((_, side))) => Some((mid.clone(), *side)),
                _ => None,
            });
            if let Some((mid, side)) = target {
                if let Some(h) = app.bouts.get(&mid) {
                    let _ = h.control.try_send(BoutControl::Forfeit { side });
                }
            }
        }
        // Read-only spectate (the live-match debug view): attach this socket to a
        // bout's spectator stream. No identity/token needed — spectators send no
        // inputs, so there's nothing to forge.
        Some("spectate") => {
            let mid = v.get("match_id").and_then(|m| m.as_str()).map(|s| s.to_string());
            let app = state.lock().await;
            let tx = app.clients.get(id).map(|c| c.tx.clone());
            let sent = match (mid.as_deref(), tx) {
                (Some(m), Some(tx)) => app
                    .bouts
                    .get(m)
                    .map(|h| h.control.try_send(BoutControl::AddSpectator { tx }).is_ok())
                    .unwrap_or(false),
                _ => false,
            };
            if !sent {
                send(&app, id, &json!({"type":"spectateFailed"}));
            }
        }
        _ => {}
    }
}

/// A fresh tagged-UUID id: `<kind>-<uuid v4>` (e.g. `client-7f3a…`, `match-1b9c…`).
/// The kind prefix makes ids self-describing in logs/URLs; the v4 uuid makes them
/// unguessable and unique across server restarts.
fn new_id(kind: &str) -> String {
    format!("{kind}-{}", uuid::Uuid::new_v4())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Shared>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Shared) {
    let id = new_id("client");
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let tx_ping = tx.clone();

    {
        let mut a = state.lock().await;
        let st = a.rating_for("");
        a.clients.insert(
            id.clone(),
            Client {
                name: String::new(), tx, peer: None, state: st, bout: None,
                match_id: None, status: None, geo: None, is_bot: false,
                prev_status: None, ping_sent_at: None, ping_ms: None,
            },
        );
    }

    metrics::METRICS.ws_connections.inc();

    // Writer task: drain the per-client channel to the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if sender.send(msg).await.is_err() {
                break;
            }
            metrics::ws_out();
        }
    });

    // Ping task: every 5s, stamp the send time and ws-Ping the client. Browsers and
    // the bot auto-Pong, and the read loop turns the Pong into a round-trip time —
    // the lobby's per-player latency. Stops when the client is gone.
    let ping_state = state.clone();
    let ping_id = id.clone();
    let ping_task = tokio::spawn(async move {
        let mut iv = tokio::time::interval(Duration::from_secs(5));
        iv.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        iv.tick().await; // consume the immediate first tick
        loop {
            iv.tick().await;
            {
                let mut a = ping_state.lock().await;
                match a.clients.get_mut(&ping_id) {
                    Some(c) => c.ping_sent_at = Some(Instant::now()),
                    None => break,
                }
            }
            if tx_ping.send(Message::Ping(Vec::new())).is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = receiver.next().await {
        metrics::ws_in();
        match msg {
            Message::Text(t) => handle_message(&state, &id, &t).await,
            Message::Pong(_) => {
                let mut a = state.lock().await;
                if let Some(c) = a.clients.get_mut(&id) {
                    if let Some(sent) = c.ping_sent_at.take() {
                        let rtt = sent.elapsed().as_millis() as u32;
                        c.ping_ms = Some(rtt);
                        metrics::METRICS.ws_ping_ms.observe(rtt as f64);
                    }
                }
                // Roster carries the (bucketed) ping, so refresh if it changed.
                maybe_broadcast_players(&mut a);
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup on disconnect: notify the peer, drop from queue/clients.
    {
        let mut a = state.lock().await;
        a.waiting.retain(|w| w != &id);
        // This connection's own pending challenge (as challenger) is cancelled.
        clear_challenge(&mut a, &id);
        // Any pending challenge TARGETING this connection: the target left, so the
        // challenger should be told it was declined and the challenge cleared.
        let aimed_here: Vec<String> = a
            .challenges
            .iter()
            .filter_map(|(cid, (tid, _))| (tid == &id).then(|| cid.clone()))
            .collect();
        let by = a.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        for cid in aimed_here {
            clear_challenge(&mut a, &cid);
            send(&a, &cid, &json!({"type":"challengeDeclined","by":by}));
        }
        // A mid-bout disconnect is NOT a forfeit anymore: the bout's tick loop sees
        // the dropped socket and enters the reconnect-grace freeze (the player may be
        // refreshing), forfeiting only if they don't reattach in time. So only fire
        // `opponentLeft` for a (non-bout) peer link; the loop owns the in-bout case.
        // We still remove this dead connection below — the loop holds its own `tx`
        // clone and detects the drop via send failure.
        let peer = a.clients.get(&id).and_then(|c| c.peer.clone());
        let in_bout = a.clients.get(&id).is_some_and(|c| c.bout.is_some());
        if let Some(p) = peer {
            if !in_bout {
                send(&a, &p, &json!({"type": "opponentLeft"}));
                if let Some(c) = a.clients.get_mut(&p) {
                    c.peer = None;
                }
            }
        }
        let was_listed = a.clients.get(&id).is_some_and(|c| c.status.is_some());
        a.clients.remove(&id);
        metrics::METRICS.ws_connections.dec();
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
    ping_task.abort();
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
    title       TEXT,
    name_a      TEXT,
    name_b      TEXT,
    lines       INTEGER
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
    // Player names per online match (for the library's "Alice vs Bob" + profile
    // links); same idempotent-ALTER migration. Older rows keep NULL names.
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN name_a TEXT", []);
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN name_b TEXT", []);
    // Total lines cleared in the recording (shown in the library). Backfilled
    // for pre-existing rows by `backfill_lines`.
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN lines INTEGER", []);
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
    backfill_lines(&conn);
    conn
}

/// Total lines cleared across all boards of a single-board (practice / vs-Computer)
/// recording — deterministically replayed to the end.
fn single_replay_lines(r: &Replay) -> i64 {
    let mut p = ReplayPlayer::new(r.clone());
    p.run_to_end();
    let mut lines = p.player().score().lines;
    if let Some(ai) = p.ai() {
        lines += ai.score().lines;
    }
    lines
}

/// Total lines cleared across both boards of an online (versus) recording.
fn versus_replay_lines(r: &VersusReplay) -> i64 {
    let mut p = VersusReplayPlayer::new(r.clone());
    p.run_to_end();
    p.game(true).score().lines + p.game(false).score().lines
}

/// One-time backfill of the `lines` column for rows recorded before it existed
/// (best-effort: malformed JSON is skipped). After the first run no rows are
/// NULL, so subsequent startups are no-ops.
fn backfill_lines(conn: &Connection) {
    let rows: Vec<(String, String)> = match conn.prepare("SELECT id, json FROM replays WHERE lines IS NULL") {
        Ok(mut stmt) => stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .and_then(|m| m.collect())
            .unwrap_or_default(),
        Err(_) => return,
    };
    for (id, json) in rows {
        // A versus recording carries seed_a/seed_b; everything else is single-board.
        let lines = if let Ok(vr) = VersusReplay::from_json(&json) {
            versus_replay_lines(&vr)
        } else if let Ok(r) = Replay::from_json(&json) {
            single_replay_lines(&r)
        } else {
            continue;
        };
        let _ = conn.execute("UPDATE replays SET lines = ?1 WHERE id = ?2", rusqlite::params![lines, id]);
    }
}

/// Insert a recording (no-op if its content id already exists). Returns rows
/// affected (1 = newly stored, 0 = already present).
fn db_insert(conn: &Connection, id: &str, r: &Replay, json: &str, created_at: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO replays
            (id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title, lines)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            single_replay_lines(r),
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
    name_a: &str,
    name_b: &str,
    lines: i64,
) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO replays
            (id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title, name_a, name_b, lines)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
            name_a,
            name_b,
            lines,
        ],
    )
}

/// Fetch a recording's JSON by id. (Kept as a focused helper; the served endpoint
/// uses [`db_get_with_names`], but tests + future callers want the plain JSON.)
#[allow(dead_code)]
fn db_get(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT json FROM replays WHERE id = ?1", [id], |row| row.get::<_, String>(0))
        .optional()
}

/// Fetch a recording's JSON plus its stored player names (an online VersusReplay row
/// has `name_a`/`name_b`; practice/vs-computer rows leave them NULL). Used to label
/// the single-replay viewer with the real names instead of "Player A/B".
fn db_get_with_names(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<Option<(String, Option<String>, Option<String>)>> {
    conn.query_row(
        "SELECT json, name_a, name_b FROM replays WHERE id = ?1",
        [id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
    )
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
        "SELECT id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, title, name_a, name_b, lines
         FROM replays ORDER BY created_at DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        let ai_level: Option<i64> = row.get(3)?;
        let title: Option<String> = row.get(8)?;
        let name_a: Option<String> = row.get(9)?;
        let name_b: Option<String> = row.get(10)?;
        let lines: Option<i64> = row.get(11)?;
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
            "name_a": name_a,
            "name_b": name_b,
            "lines": lines,
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
        match db_get_with_names(&conn, &id) {
            Ok(v) => v,
            Err(e) => { eprintln!("get_replay db error for {id}: {e}"); None }
        }
    };
    match found {
        Some((txt, name_a, name_b)) => {
            // Inject the real player names into the replay JSON so the viewer can label
            // the boards (the stored blob doesn't carry them; they live in DB columns).
            // Unknown fields are ignored by the replay deserializer, so this is safe.
            let body = match (name_a, name_b) {
                (None, None) => txt,
                (na, nb) => match serde_json::from_str::<Value>(&txt) {
                    Ok(Value::Object(mut o)) => {
                        if let Some(n) = na { o.insert("name_a".into(), json!(n)); }
                        if let Some(n) = nb { o.insert("name_b".into(), json!(n)); }
                        serde_json::to_string(&Value::Object(o)).unwrap_or(txt)
                    }
                    _ => txt,
                },
            };
            ([(header::CONTENT_TYPE, "application/json")], body).into_response()
        }
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

/// `GET /api/debug/matches` — the live-match debug list: every in-progress bout
/// (`match_id` + the two player names), so a spectator can pick one to watch
/// (`?spectate=<match_id>`). Read-only; reflects [`App::bouts`].
async fn debug_matches(State(state): State<Shared>) -> impl IntoResponse {
    let matches: Vec<Value> = {
        let app = state.lock().await;
        app.bouts
            .iter()
            .map(|(mid, h)| json!({ "match_id": mid, "name_a": h.name_a, "name_b": h.name_b }))
            .collect()
    };
    Json(json!({ "matches": matches })).into_response()
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

/// Count every served HTTP request (the "hit rate" metric).
async fn track_http(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    metrics::METRICS.http_requests.inc();
    next.run(req).await
}

#[tokio::main]
async fn main() {
    // Error tracking — inert until SENTRY_DSN is set (a fly secret). The guard is
    // held for the whole process; the `panic` integration's hook captures crashes
    // (including from spawned tasks). No-op + zero overhead when unconfigured.
    let _sentry_guard = std::env::var("SENTRY_DSN").ok().filter(|d| !d.is_empty()).map(|dsn| {
        let g = sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                send_default_pii: true,
                ..Default::default()
            },
        ));
        println!("Sentry error tracking enabled");
        g
    });

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
        .route(
            "/metrics",
            get(|| async {
                ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], metrics::render())
            }),
        )
        .route("/api/replays", post(post_replay).get(list_replays))
        .route("/api/replays/:id", get(get_replay))
        .route("/api/leaderboard", get(leaderboard))
        .route("/api/debug/matches", get(debug_matches))
        .route("/api/identity", post(post_identity))
        .route("/api/player/:name", get(player_profile))
        .route("/replay/:id", get(replay_page))
        .route("/", get(|| async { Redirect::permanent("/www/") }))
        .fallback_service(ServeDir::new(&static_dir))
        .layer(axum::middleware::from_fn(track_http))
        .with_state(state);

    // Bind the IPv6 any-address. On Linux this is dual-stack (IPV6_V6ONLY=0 by
    // default), so it serves BOTH the public site (the fly proxy reaches us over
    // IPv4) AND fly's private 6PN network, which is IPv6-ONLY — that 6PN path is
    // how the region bots reach us at ws://battletris.internal:8080. Binding only
    // 0.0.0.0 (IPv4) left the 6PN port closed, so the bots got "connection refused".
    let addr = format!("[::]:{port}");
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
            bouts: HashMap::new(),
        }
    }

    /// Add a named client with NO lobby status: it only becomes a match candidate
    /// via the explicit `waiting` queue, so the `queue`-based matchmaking tests
    /// stay isolated from the presence path. Presence tests set `status`
    /// explicitly via [`set_present`].
    fn add_client(app: &mut App, id: &str, name: &str) -> mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = mpsc::unbounded_channel();
        let state = app.rating_for(name);
        app.clients.insert(
            id.to_string(),
            Client {
                name: name.to_string(), tx, peer: None, state, bout: None,
                match_id: None, status: None, geo: None, is_bot: false,
                prev_status: None, ping_sent_at: None, ping_ms: None,
            },
        );
        rx
    }

    /// Like [`add_client`] but flags the client as a bot (for the bot-vs-bot
    /// auto-pair-exclusion test).
    fn add_bot(app: &mut App, id: &str, name: &str) -> mpsc::UnboundedReceiver<Message> {
        let rx = add_client(app, id, name);
        if let Some(c) = app.clients.get_mut(id) {
            c.is_bot = true;
            c.status = Some(Status::Available);
        }
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
        let _rx = add_client(&mut app, "1","solo");
        try_match(&mut app, "1");
        assert_eq!(app.waiting, vec!["1".to_string()], "queued while waiting for an opponent");
        assert_eq!(app.clients["1"].peer, None);
    }

    #[test]
    fn a_matched_pair_starts_a_hosted_bout() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, "1","alice");
        let mut rx_b = add_client(&mut app, "2","bob");

        assert!(try_match(&mut app, "1").is_none(), "alice queues, no opponent yet");
        let pending = try_match(&mut app, "2").expect("bob matches alice -> a hosted bout");

        assert_eq!((pending.id_a.as_str(), pending.id_b.as_str()), ("1", "2"), "first-queued is side A");
        assert!(
            app.clients["1"].bout.is_some() && app.clients["2"].bout.is_some(),
            "both clients are bound to the bout's input channel"
        );
        // Both get `matchStart` (with a seed); both go InGame.
        assert!(drained_types(&mut rx_a).contains(&"matchStart".to_string()));
        assert!(drained_types(&mut rx_b).contains(&"matchStart".to_string()));
        assert_eq!(app.clients["1"].status, Some(Status::InGame));
        assert_eq!(app.clients["2"].status, Some(Status::InGame));
    }


    #[tokio::test]
    async fn run_bout_ticks_and_broadcasts_snapshots_to_both_sides() {
        use std::sync::Arc;
        let (tx_a, mut rx_a) = mpsc::unbounded_channel::<Message>();
        let (tx_b, mut rx_b) = mpsc::unbounded_channel::<Message>();
        let (input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
        let (_control_tx, control_rx) = mpsc::channel::<BoutControl>(4);
        let app0 = test_app();
        let (state_a, state_b) = (app0.rating_for("alice"), app0.rating_for("bob"));
        let pb = PendingBout {
            match_id: "match-test".into(), id_a: "1".into(), id_b: "2".into(), seed_a: 11, seed_b: 22,
            name_a: "alice".into(), name_b: "bob".into(), state_a, state_b,
            tx_a, tx_b, input_rx, control_rx, human: [true, true],
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
        let mut rx_win = add_client(&mut app, "1","alice");
        let state_a = app.clients["1"].state;
        let state_b = app.rating_for("bob"); // bob is gone from app.clients

        settle_bout(&mut app, "m100", "1", "alice", state_a, "2", "bob", state_b, true, 30, 12);

        assert!(drained_types(&mut rx_win).contains(&"rating".to_string()), "winner got a rating");
        assert!(
            app.ratings.contains_key("alice") && app.ratings.contains_key("bob"),
            "both ratings persisted by name despite the loser being gone"
        );
        // Idempotent per pair: a duplicate settle is a no-op.
        let before = app.ratings.get("alice").copied();
        settle_bout(&mut app, "m100", "1", "alice", state_a, "2", "bob", state_b, true, 30, 12);
        assert_eq!(app.ratings.get("alice").copied(), before, "settle is idempotent");
    }

    #[test]
    fn settle_bout_rates_once_notifies_both_and_is_idempotent() {
        std::env::set_var("RATINGS_FILE", std::env::temp_dir().join("bt_glue_ratings.json"));
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, "1","alice");
        let mut rx_b = add_client(&mut app, "2","bob");
        let (sa, sb) = (app.clients["1"].state, app.clients["2"].state);

        let exp_before = app.clients["1"].state.experience;
        settle_bout(&mut app, "m100", "1", "alice", sa, "2", "bob", sb, true, 30, 12); // alice beats bob

        assert_eq!(app.clients["1"].state.experience, exp_before + 1, "rated exactly once");
        assert!(app.clients["1"].state.rating.conservative(3.0) > 0.0);
        assert!(drained_types(&mut rx_a).contains(&"rating".to_string()), "winner notified");
        assert!(drained_types(&mut rx_b).contains(&"rating".to_string()), "loser notified");

        // Settling the same match id again must be a no-op (the double-settle guard).
        let exp = app.clients["1"].state.experience;
        settle_bout(&mut app, "m100", "1", "alice", sa, "2", "bob", sb, true, 30, 12);
        assert_eq!(app.clients["1"].state.experience, exp, "second settle changes nothing");
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
        app.last_active.insert("1".to_string(), base + Duration::from_secs(99)); // 1s ago  -> active
        app.last_active.insert("2".to_string(), base + Duration::from_secs(75)); // 25s ago -> active
        app.last_active.insert("3".to_string(), base + Duration::from_secs(69)); // 31s ago -> idle
        app.last_active.insert("4".to_string(), base); //                           100s ago -> idle
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

    /// Give a client a lobby status (the presence path).
    fn set_present(app: &mut App, id: &str, status: Status) {
        app.clients.get_mut(id).unwrap().status = Some(status);
    }

    #[test]
    fn roster_lists_only_named_clients_lowercase_and_sorted() {
        let mut app = test_app();
        let _r1 = add_client(&mut app, "1","bob");
        let _r2 = add_client(&mut app, "2","alice");
        let _r3 = add_client(&mut app, "3",""); // anonymous -> never listed
        set_present(&mut app, "1",Status::Searching);
        set_present(&mut app, "2",Status::Available);

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
        let _r1 = add_client(&mut app, "1","dup");
        let _r2 = add_client(&mut app, "2","dup");
        set_present(&mut app, "1",Status::Available);
        set_present(&mut app, "2",Status::InGame);
        let r = roster(&app);
        assert_eq!(r.len(), 1, "de-duped by name");
        assert_eq!(r[0], ("dup".to_string(), Status::InGame, None, false), "the busiest status wins");
    }

    #[test]
    fn auto_match_pairs_only_two_humans_never_a_bot() {
        let mut app = test_app();
        let _b1 = add_bot(&mut app, "1","Tokyo-Ernie");
        let _b2 = add_bot(&mut app, "2","London-Ernie");
        // Bots are passive — they never auto-pair (with each other or a human).
        assert!(!is_match_candidate(&app, "1", "2"), "two bots don't auto-pair");
        // A human going Available is NOT thrown into a waiting bot.
        let _h = add_client(&mut app, "3","human");
        if let Some(c) = app.clients.get_mut("3") {
            c.status = Some(Status::Available);
        }
        assert!(!is_match_candidate(&app, "3", "1"), "a bot is NOT an auto-pair target for a human");
        assert!(try_match(&mut app, "3").is_none(), "a lone human among only bots finds no auto-pair");
        // Two open humans DO auto-pair with each other.
        let _h2 = add_client(&mut app, "4","human2");
        if let Some(c) = app.clients.get_mut("4") {
            c.status = Some(Status::Available);
        }
        assert!(is_match_candidate(&app, "3", "4"), "two humans auto-pair");
        assert!(try_match(&mut app, "3").is_some(), "two open humans auto-pair");
    }

    #[test]
    fn post_bout_status_keeps_present_players_but_drops_pure_challengers() {
        // Anyone who had lobby presence before the match returns to Available...
        assert_eq!(post_bout_status(Some(Status::Available)), Some(Status::Available));
        assert_eq!(post_bout_status(Some(Status::Searching)), Some(Status::Available));
        assert_eq!(post_bout_status(Some(Status::InGame)), Some(Status::Available));
        // ...but a one-off directed challenger (no prior presence) leaves the
        // roster instead of being force-Available + instantly auto-rematched.
        assert_eq!(post_bout_status(None), None);
    }

    #[test]
    fn start_bout_remembers_pre_match_presence_for_restore() {
        let mut app = test_app();
        // A bot (Available) challenged by a human (no presence — a pure challenger).
        let _bot = add_bot(&mut app, "1","Tokyo-Ernie");
        let _human = add_client(&mut app, "2","human"); // status stays None
        start_bout(&mut app, "2", "1", None).expect("bout starts");
        assert_eq!(app.clients["1"].prev_status, Some(Status::Available), "bot was Available");
        assert_eq!(app.clients["2"].prev_status, None, "challenger had no presence");
        // Hence at bout end the bot returns to Available, the human to nothing.
        assert_eq!(post_bout_status(app.clients["1"].prev_status), Some(Status::Available));
        assert_eq!(post_bout_status(app.clients["2"].prev_status), None);
    }

    #[test]
    fn roster_carries_the_ping_bucketed_to_ten_ms() {
        let mut app = test_app();
        let _r = add_client(&mut app, "1","Sydney-Bert");
        if let Some(c) = app.clients.get_mut("1") {
            c.status = Some(Status::Available);
            c.ping_ms = Some(43); // measured RTT
        }
        let frame = players_msg(&roster(&app));
        let players = frame["players"].as_array().unwrap();
        // The ping rides the frame, bucketed to the nearest 10ms (43 → 40).
        assert_eq!(players[0]["ping"], 40, "ping rides the players frame");
        assert_eq!(players[0]["geo"], serde_json::Value::Null, "geo no longer sent");
    }

    #[test]
    fn maybe_broadcast_players_only_pushes_on_a_real_change() {
        let mut app = test_app();
        let mut rx = add_client(&mut app, "1","alice");
        set_present(&mut app, "1",Status::Available);
        maybe_broadcast_players(&mut app);
        assert!(drained_types(&mut rx).contains(&"players".to_string()), "first roster pushed");
        // No change -> no re-broadcast.
        maybe_broadcast_players(&mut app);
        assert!(!drained_types(&mut rx).contains(&"players".to_string()), "no spurious re-push");
        // A status change -> pushed again.
        set_present(&mut app, "1",Status::Searching);
        maybe_broadcast_players(&mut app);
        assert!(drained_types(&mut rx).contains(&"players".to_string()), "change re-pushed");
    }

    // --- unified auto-pairing (Available is challengeable AND auto-pairable) ---

    #[test]
    fn two_available_clients_auto_pair_without_an_explicit_queue() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, "1","alice");
        let mut rx_b = add_client(&mut app, "2","bob");
        set_present(&mut app, "1",Status::Available);
        set_present(&mut app, "2",Status::Available);

        // alice goes through the matcher with NO one queued — bob is Available, so
        // they pair purely on presence.
        let pending = try_match(&mut app, "1").expect("two Available clients pair");
        assert_eq!((pending.id_a.as_str(), pending.id_b.as_str()), ("2", "1"), "bob (the candidate) is side A");
        assert_eq!(app.clients["1"].status, Some(Status::InGame));
        assert_eq!(app.clients["2"].status, Some(Status::InGame));
        assert!(drained_types(&mut rx_a).contains(&"matchStart".to_string()));
        assert!(drained_types(&mut rx_b).contains(&"matchStart".to_string()));
    }

    // --- directed challenge --------------------------------------------------

    #[test]
    fn challenge_accept_builds_a_directed_bout_and_sets_both_ingame() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, "1","alice");
        let mut rx_b = add_client(&mut app, "2","bob");
        set_present(&mut app, "1",Status::Available);
        set_present(&mut app, "2",Status::Available);

        // alice challenges bob (record the pending challenge as the handler would).
        let tid = find_named_available(&app, "bob").expect("bob is challengeable");
        assert_eq!(tid, "2");
        app.challenges.insert("1".to_string(), ("2".to_string(), Instant::now() + CHALLENGE_TIMEOUT));

        // bob accepts -> a directed bout for (alice=A, bob=B); challenge cleared.
        let pending = start_bout(&mut app, "1", "2", None).expect("directed bout built");
        clear_challenge(&mut app, "1");
        assert_eq!((pending.id_a.as_str(), pending.id_b.as_str()), ("1", "2"), "challenger is side A");
        assert!(app.challenges.is_empty(), "pending challenge cleared on accept");
        assert_eq!(app.clients["1"].status, Some(Status::InGame));
        assert_eq!(app.clients["2"].status, Some(Status::InGame));
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

    // --- rejoin-on-refresh + intentional leave -------------------------------

    #[test]
    fn start_bout_registers_a_rejoinable_handle_with_match_id() {
        let mut app = test_app();
        let mut rx_a = add_client(&mut app, "1","alice");
        let _rx_b = add_client(&mut app, "2","bob");
        set_present(&mut app, "1",Status::Available);
        set_present(&mut app, "2",Status::Available);

        let mut pending = start_bout(&mut app, "1", "2", None).expect("bout built");
        let mid = pending.match_id;

        // The bout is registered for rejoin, with both participants recorded, and
        // each client knows its match_id (so an intentional leave can find it).
        let handle = app.bouts.get(&mid).expect("bout registered in app.bouts");
        assert_eq!(handle.name_a, "alice");
        assert_eq!(handle.name_b, "bob");
        assert_eq!(app.clients["1"].match_id.as_deref(), Some(mid.as_str()));
        assert_eq!(app.clients["2"].match_id.as_deref(), Some(mid.as_str()));

        // matchStart carries the id the client parks in its URL for rejoin.
        let start: Value = std::iter::from_fn(|| rx_a.try_recv().ok())
            .filter_map(|m| match m {
                Message::Text(t) => serde_json::from_str::<Value>(&t).ok(),
                _ => None,
            })
            .find(|v| v["type"] == "matchStart")
            .expect("alice matchStart");
        assert_eq!(start["match_id"].as_str(), Some(mid.as_str()));

        // A reattach (from the rejoin handler) reaches the bout's control receiver —
        // the loop would consume this to swap the socket back in and resume.
        let (txn, _rxn) = mpsc::unbounded_channel::<Message>();
        app.bouts[&mid]
            .control
            .try_send(BoutControl::Reattach { side: Side::A, new_id: "7".into(), tx: txn })
            .expect("control send");
        match pending.control_rx.try_recv() {
            Ok(BoutControl::Reattach { side, new_id, .. }) => {
                assert_eq!(side, Side::A);
                assert_eq!(new_id, "7");
            }
            _ => panic!("expected a Reattach on the bout control channel"),
        }
    }

    #[test]
    fn leave_resolves_a_clients_own_bout_from_server_state() {
        // The `leaveMatch` handler reads (match_id, side) from the client's OWN
        // server-side binding — never anything client-supplied — so a client can
        // only forfeit the bout it is actually in. Mirror that resolution here.
        let mut app = test_app();
        let _a = add_client(&mut app, "1","alice");
        let _b = add_client(&mut app, "2","bob");
        set_present(&mut app, "1",Status::Available);
        set_present(&mut app, "2",Status::Available);
        let mut pending = start_bout(&mut app, "1", "2", None).expect("bout");
        let mid = pending.match_id;

        let target = app.clients.get("2").and_then(|c| match (c.match_id.clone(), c.bout.as_ref()) {
            (Some(m), Some((_, side))) => Some((m, *side)),
            _ => None,
        });
        assert_eq!(target, Some((mid.clone(), Side::B)), "bob resolves to his own bout, side B");

        app.bouts[&mid]
            .control
            .try_send(BoutControl::Forfeit { side: Side::B })
            .expect("forfeit send");
        assert!(matches!(pending.control_rx.try_recv(), Ok(BoutControl::Forfeit { side: Side::B })));

        // The bout ends and unregisters once run_bout settles (asserted via the
        // run_bout integration test); here we've covered the resolution + routing.
    }

    #[test]
    fn find_named_available_ignores_busy_or_offline_targets() {
        let mut app = test_app();
        let _r = add_client(&mut app, "1","busy");
        set_present(&mut app, "1",Status::InGame); // in a match -> not challengeable
        assert!(find_named_available(&app, "busy").is_none(), "InGame isn't available");
        assert!(find_named_available(&app, "ghost").is_none(), "offline isn't available");
        set_present(&mut app, "1",Status::Available);
        assert_eq!(find_named_available(&app, "busy"), Some("1".to_string()), "Available is challengeable");
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
