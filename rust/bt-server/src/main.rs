//! BattleTris server: one binary that serves the web client (static files) and
//! the online backend on the same port.
//!    * `GET /ws`        WebSocket: matchmaking (paired by TrueSkill match
//!      quality) and the server-authoritative match itself. The server runs the
//!      deterministic engine (a [`bout::Bout`]); clients predict locally and
//!      reconcile against snapshots and forwarded events; results are
//!      persisted as rating updates.
//!    * everything else  static files from `STATIC_DIR` (default `bt-wasm`,
//!      which holds `www/` and `pkg/`); `/` redirects to `/www/`.
//!
//! Serving both on one port means the browser uses a same-origin
//! `ws(s)://<host>/ws`, which works locally and behind fly.io's TLS.
//!
//! Env: `PORT` (default 8080), `STATIC_DIR` (default `bt-wasm`),
//! `RATINGS_FILE` (default `ratings.json`).
//!
//! Protocol (JSON text frames):
//!
//! ```text
//!   client -> server:
//!     {"type":"queue","token":"<jwt>"}      (Find Match; or {"name":"alice"})
//!     {"type":"available","value":true,"token":"<jwt>"}   (open to matches)
//!     {"type":"challenge","target":"bob"}   (directed challenge)
//!     {"type":"input","seq":N,"input":<bt_replay::Input>}   (a gameplay action)
//!     {"type":"rejoin","match_id":"<match-id>","token":"<jwt>"}   (reattach after a refresh)
//!     {"type":"leaveMatch"}                 (intentional leave -> immediate forfeit)
//!     {"type":"resync"}                     (local sim diverged, request a keyframe)
//!   server -> client:
//!     {"type":"matchStart","side":"A|B","seed":N,"opponent":"bob","match_id":"<match-id>",...}
//!     {"type":"snapshot","tick":N,"ack":N,"result":...,"you":...,"opp":...,"lock_seq":N,"lock_hash":N,"keyframe"?:[..]}
//!     {"type":"rating","mu":...,"sigma":...,"conservative":...,"won":true}
//!     {"type":"players","players":[{"name":..,"status":"available|searching|ingame"}]}
//!     {"type":"opponentLeft"}
//!     {"type":"opponentReconnecting"}       (opponent dropped; bout frozen, grace)
//!     {"type":"opponentResumed"}            (opponent reattached; play resumes)
//!     {"type":"rejoinFailed"}               (no such live bout for this identity)
//! ```
//!
//! In a bout, clients send legal inputs; the server is the only side that ticks
//! the sim, so a client cannot inject board state or cross-player effects. Each
//! client's own board runs on local prediction, kept in step by the
//! server-forwarded `event` frames for cross-player effects, corrected by a
//! full keyframe only on a trigger, with the per-lock hash on each snapshot
//! driving a `resync` request when the two sims disagree.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{FromRef, Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use bt_core::versus::Side;
use bt_core::weapons::WeaponToken;
use bt_replay::{Input, Replay, ReplayPlayer, VersusReplay, VersusReplayPlayer};
use bt_trueskill::ts2::{rate_match, MatchOutcome, PlayerState, Ts2Params, Winner};
use bt_trueskill::{quality_1v1, Rating};
use futures_util::{SinkExt, StreamExt};
// Replay blob storage: `ObjectStore` is the trait `ReplayStore::Bucket` stores a
// trait object of; `ObjectStoreExt` supplies the ergonomic `put`/`get` methods
// used against it. Aliased to `ObjectPath` because axum's extractor is also
// named `Path` (imported above).
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

/// Server-authoritative online match engine (the client-server migration).
mod bout;
mod metrics;
use bout::Bout;

/// Player identity (HS256 JWT). Factored into a shared crate so the bots can
/// mint the same tokens. Aliased to `identity` so existing `identity::...` call
/// sites stand.
use bt_identity as identity;

/// One queued input headed for an authoritative [`Bout`]: (side, action, seq).
type BoutInput = (Side, Input, u64);

/// The engine build online replays are stamped with (the `git` short SHA passed
/// at compile time via `BT_GIT_SHA`, or "dev" locally).
const ENGINE_SHA: &str = match option_env!("BT_GIT_SHA") {
    Some(s) => s,
    None => "dev",
};

/// Matchmaking/relay state (tokio mutex, held across `.await`).
type Shared = Arc<Mutex<App>>;
/// The replay index/store. `rusqlite::Connection` is `Send` but not `Sync`, so a
/// std mutex guards it; replay queries are sub-millisecond and never `.await`
/// while the guard is held, so a blocking lock on the async runtime is fine here.
type Db = Arc<std::sync::Mutex<Connection>>;

/// Where a replay's JSON blob lives. Two explicit modes, not an `Option`
/// fallback: an unconfigured bucket means every replay lives in SQLite, exactly
/// as before blob storage existed; a HALF-configured bucket is refused at boot
/// (see [`replay_store_config`]) rather than silently landing in one of the two
/// real modes. `Bucket` holds a trait object (not a concrete `AmazonS3`) so
/// tests can swap in [`object_store::memory::InMemory`] with no network.
#[derive(Clone)]
enum ReplayStore {
    /// The `replays.json` column holds the full replay text, same as before
    /// blob storage existed.
    Sqlite,
    /// The `json` column is written NULL; the blob lives at `replays/<id>.json`
    /// in the wrapped store (see [`replay_blob_path`]).
    Bucket(Arc<dyn ObjectStore>),
}

/// The bucket key one replay's blob lives at: one JSON object per content-hash
/// id, mirroring the DB's own primary key so the two stay trivially correlated.
fn replay_blob_path(id: &str) -> ObjectPath {
    ObjectPath::from(format!("replays/{id}.json"))
}

/// Decide the replay blob storage mode from env, via an injected lookup so
/// tests exercise every branch without mutating process env (production passes
/// `|k| std::env::var(k).ok()`; tests pass a `HashMap` lookup). `BUCKET_NAME`
/// unset means Sqlite, today's behavior. `BUCKET_NAME` set means Bucket, built
/// from the Tigris-injected `AWS_*` vars; a companion var missing while
/// `BUCKET_NAME` is set is a misconfiguration, not a reason to fall back to
/// Sqlite, so it panics loudly naming the missing var rather than degrading
/// silently. `AWS_REGION` is the one exception: Tigris doesn't need a real
/// region, so a missing one just defaults to `"auto"`.
fn replay_store_config(get: impl Fn(&str) -> Option<String>) -> ReplayStore {
    let Some(bucket) = get("BUCKET_NAME") else {
        println!("replay blobs: sqlite (no BUCKET_NAME configured)");
        return ReplayStore::Sqlite;
    };
    let require = |name: &str| {
        get(name).unwrap_or_else(|| {
            panic!("BUCKET_NAME is set but {name} is missing (Tigris sets both together via fly secrets)")
        })
    };
    let endpoint = require("AWS_ENDPOINT_URL_S3");
    let access_key_id = require("AWS_ACCESS_KEY_ID");
    let secret_access_key = require("AWS_SECRET_ACCESS_KEY");
    let region = get("AWS_REGION").unwrap_or_else(|| "auto".to_string());
    let s3 = object_store::aws::AmazonS3Builder::new()
        .with_bucket_name(&bucket)
        .with_endpoint(&endpoint)
        .with_region(&region)
        .with_access_key_id(&access_key_id)
        .with_secret_access_key(&secret_access_key)
        .build()
        .unwrap_or_else(|e| panic!("build S3 client for bucket {bucket} at {endpoint}: {e}"));
    println!("replay blobs: bucket {bucket} via {endpoint}");
    ReplayStore::Bucket(Arc::new(s3))
}

/// Combined router state. `FromRef` lets each handler extract just the piece it
/// needs: `State<Shared>` for the websocket, `State<Db>` for replay endpoints,
/// `State<ReplayStore>` for the ones that read/write the replay blob.
#[derive(Clone)]
struct AppState {
    app: Shared,
    db: Db,
    store: ReplayStore,
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

impl FromRef<AppState> for ReplayStore {
    fn from_ref(s: &AppState) -> ReplayStore {
        s.store.clone()
    }
}

/// Lobby presence for a named client. An UN-named connection has no status and
/// never appears in the `players` roster (the implicit 4th "anonymous" state).
///
/// A client is *Available* iff it's both challengeable (a directed challenge can
/// reach it) and eligible for auto-pairing: "open to matches" is one switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    /// Open to matches: challengeable and auto-pairable.
    Available,
    /// Pressed "Find Match", actively looking (still pairable and challengeable).
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
    /// The player name this connection claims (empty until it establishes one).
    /// Keys ratings, the roster, and bout participation. An already-rated name can
    /// only be claimed with a valid token, not a bare `name` (see [`resolve_name`]).
    name: String,
    /// Writer side of this connection's outbound channel. Every send to the client
    /// (direct or broadcast) enqueues here; the per-socket writer task drains it.
    tx: mpsc::UnboundedSender<Message>,
    /// The opponent connection while in a match, used to send `opponentLeft` if
    /// this client disconnects mid-bout. Set in [`start_bout`], cleared at end.
    peer: Option<String>,
    /// This connection's current rating, loaded for `name` at queue/available time.
    /// Refreshed in place when a settled bout returns a new rating. The `challenge`
    /// path updates `name` without reloading `state`; `state` is therefore only
    /// guaranteed current after `queue` or `available`.
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
    /// auto-pair with each other; if they did they'd drain the lobby playing
    /// themselves and leave nobody for a visitor to challenge.
    is_bot: bool,
    /// Lobby presence captured at the start of the current bout, restored when it
    /// ends (see [`post_bout_status`]). `None` outside a bout.
    prev_status: Option<Status>,
    /// When the last ws Ping was sent to this client (to measure round-trip time on
    /// the matching Pong). `None` between a Pong and the next Ping.
    ping_sent_at: Option<Instant>,
    /// Last measured round-trip time in ms, shown next to the name in the lobby
    /// roster. `None` until the first Pong comes back.
    ping_ms: Option<u32>,
}

/// Presence to restore when a bout ends. A player who had ANY lobby presence
/// before the match (Available/Searching) returns to Available; a one-off
/// directed CHALLENGER (no prior presence) leaves the roster instead of being
/// forced back to Available; if they were, the moment the match ended they'd be
/// auto-paired straight back into another one (challenge a bot, top out, get
/// re-matched with a bot forever), which reads as the board "resetting"
/// mid-session.
fn post_bout_status(prev: Option<Status>) -> Option<Status> {
    prev.map(|_| Status::Available)
}

/// Shared server state. One instance lives behind the [`Shared`] tokio mutex; a
/// connection's read loop, the per-bout tick loops, and the timed broadcasts all
/// reach in through it.
struct App {
    /// Every live connection, keyed by its tagged-UUID `client-…` id. A single
    /// player may hold several (multiple tabs); the roster de-dupes by name.
    clients: HashMap<String, Client>,
    /// Connections waiting for an auto-paired opponent. Fed by both Find Match
    /// (`queue`) and the Available presence path (`available` calls `try_match` and
    /// may enqueue here when no match is found immediately). Scanned alongside
    /// lobby-Available clients.
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
    /// Pairings already rated (keyed by `match_id`), so a bout settles a rating
    /// change exactly once even if the settle path is reached more than once.
    settled: HashSet<String>,
    /// TrueSkill tuning shared by matchmaking quality, rating updates, and the
    /// new-player base rating.
    params: Ts2Params,
    /// Replay/counters DB (shared with the HTTP handlers). Backs the hit counter
    /// and the per-player stats (`players`) table.
    db: Db,
    /// Where a finished bout's replay blob is written (shared with the HTTP
    /// handlers, which read it back). Cloned once per bout at the top of
    /// [`run_bout`] rather than read through `App` at settle time, so the
    /// Bucket-mode upload's `.await` never happens while this mutex is held.
    store: ReplayStore,
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
    /// Quiesce-in-place drain: once set (via `POST /admin/drain` at the start of a
    /// deploy) this machine accepts no new matches while its in-flight bouts finish.
    /// Lobby clients stay connected and see "matches paused". The deploy script waits
    /// for the active-bout count to reach zero, then replaces the machine in place so
    /// no live game is killed; the fresh boot clears this flag. See [`admin_drain`].
    draining: bool,
}

impl App {
    /// Boot the shared state over a replay/counters DB and a replay blob store
    /// (see [`ReplayStore`]). `ratings.json` is the working source-of-truth for
    /// matchmaking ratings; the `players` table is seeded from it once
    /// (idempotently) so the per-player profile endpoint can answer from one
    /// indexed lookup rather than re-reading the JSON.
    fn new(db: Db, store: ReplayStore) -> App {
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
            store,
            challenges: HashMap::new(),
            last_players: Vec::new(),
            bouts: HashMap::new(),
            draining: false,
        }
    }

    /// The rating state to play `name` at: their persisted `(mu, sigma,
    /// experience)` if known, otherwise the configured new-player rating. An
    /// empty name (an un-named connection) also falls through to a new rating.
    fn rating_for(&self, name: &str) -> PlayerState {
        match self.ratings.get(name) {
            Some(&(mu, sigma, experience)) => PlayerState { rating: Rating::new(mu, sigma), experience },
            None => PlayerState::new(self.params.base.new_rating()),
        }
    }

    /// Record `name`'s post-match rating in the in-memory map (the caller
    /// persists the whole map to `ratings.json` after settling).
    fn store_rating(&mut self, name: &str, s: PlayerState) {
        self.ratings
            .insert(name.to_string(), (s.rating.mu, s.rating.sigma, s.experience));
    }
}

/// Path to the persisted ratings JSON (`RATINGS_FILE`, default `ratings.json`;
/// prod points it at the fly volume alongside the replay DB).
fn ratings_file() -> String {
    std::env::var("RATINGS_FILE").unwrap_or_else(|_| "ratings.json".to_string())
}

/// Send one message to a single client by id (no-op if it has disconnected).
/// Enqueues on the client's writer channel rather than touching the socket, so
/// the caller never blocks on a slow peer.
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

/// The current persistent visit total (the "you are visitor #N" counter) read
/// from the DB, the figure that rides every `stats` broadcast.
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
/// `ping` (round-trip latency in ms, shown next to the name) and, for bots, a
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

/// Load persisted ratings from `ratings.json` (name -> `(mu, sigma,
/// experience)`). A missing or malformed file yields an empty map; a fresh
/// server simply starts everyone at the new-player rating. A field absent from a
/// row falls back to the new-player default rather than dropping the player.
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

/// Persist the whole ratings map back to `ratings.json` (pretty-printed).
/// Best-effort: a write error is swallowed rather than failing a settled match;
/// the in-memory map stays authoritative until the next successful write.
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
/// enough not to over-stall a genuine quit. A bot drop never waits; it forfeits
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
    /// `side` intentionally left the match (the in-app "Leave game" button). End
    /// the bout immediately, with no reconnect grace, and the other side winning by forfeit.
    Forfeit { side: Side },
    /// A read-only spectator wants the live two-board stream (the debug live-match
    /// view). The loop adds `tx` to its spectator list; the spectator sends no
    /// inputs, so there's no anti-cheat surface.
    AddSpectator { tx: mpsc::UnboundedSender<Message> },
    /// An out-of-band ADMIN grant (the gated `POST /admin/grant` dev tool): add one
    /// weapon and/or some funds to `side`'s authoritative game, applied INSIDE the
    /// bout task so it never races the tick loop. It is not a recorded `Input`; the
    /// loop mutates the game directly and never appends to the replay frame stream,
    /// so it cannot perturb input ordering or determinism (see `Bout::debug_grant`).
    DebugGrant { side: Side, weapon: Option<WeaponToken>, funds: Option<i64> },
    /// A client detected that its local sim has diverged from the server (its own
    /// lock hash disagrees with the one riding `side`'s snapshots) and is asking for
    /// a fresh keyframe to resync. Sent by the `resync` frame handler; the loop
    /// rate-limits how often it actually grants one (see `resync_grant_allowed`).
    Resync { side: Side },
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
    /// The two participants' names, so a `rejoin` can confirm the reconnecting
    /// identity actually belongs to this match (and pick its side) without
    /// touching the bout task.
    name_a: String,
    name_b: String,
}

/// Everything the async caller needs to spawn an authoritative match's tick
/// loop, handed back by [`try_match`] when it pairs two authoritative clients.
/// Player names + rating states are captured here so the bout can settle even
/// if a client has disconnected (and been removed from `app.clients`) by the
/// time the match ends.
struct PendingBout {
    /// Tagged-UUID match id (`match-<uuid>`), unguessable and unique across restarts.
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

/// A per-match seed from a connection id. Distinct per connection (ids are random
/// uuids) without an rng dependency, and masked to 32 bits so it round-trips through
/// the JS client's `WasmGame::new(seed: u32)` exactly; the same RNG stream on both
/// sides keeps client prediction consistent with the authoritative sim.
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
/// itself, not already in a bout, and "open to matches": either explicitly
/// queued (in `waiting`, the Find-Match path) or lobby-Available/Searching
/// (the unified presence switch; Available means challengeable and auto-pairable).
fn is_match_candidate(app: &App, id: &str, cid: &str) -> bool {
    if cid == id {
        return false;
    }
    // The auto-matcher pairs only two humans who are both open to play. Bots are
    // passive: a human reaches a regional bot by challenging it, and The Count
    // reaches humans via its own roaming challenges. Neither side auto-pairs into a
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
/// The candidate pool is everyone "open to matches": explicitly queued
/// (`waiting`) and lobby-Available clients. Going Available auto-pairs a client
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
        .keys().filter(|&cid| is_match_candidate(app, id, cid)).cloned()
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
    // Blue/green drain: this machine starts no new bouts once draining, only
    // finishing the ones already in flight. This is the single chokepoint for both
    // the auto-matcher and the challenge-accept path, so nothing slips through.
    if app.draining {
        return None;
    }
    app.waiting.retain(|w| w != a && w != b);
    // Drop any pending challenges involving either player; they're now in a
    // match, so a stale accept must not later kick off a second, unwanted bout.
    app.challenges
        .retain(|cid, (tid, _)| cid != a && cid != b && tid != a && tid != b);
    if let Some(c) = app.clients.get_mut(a) {
        c.peer = Some(b.to_string());
    }
    if let Some(c) = app.clients.get_mut(b) {
        c.peer = Some(a.to_string());
    }

    let a_name = app.clients.get(a)?.name.clone();
    let b_name = app.clients.get(b)?.name.clone();
    // Settle against each name's CURRENT stored rating, looked up here rather than
    // read from the connection's cached `c.state`. The queue and available handlers
    // refresh `c.state`, but a directed challenge sets only `c.name`, so reading
    // `c.state` would settle a challenge initiator against the default rating and
    // overwrite their real one. `rating_for` is the single source of truth and is a
    // no-op for the queue/available paths, whose `c.state` already equals it.
    let a_state = app.rating_for(&a_name);
    let b_state = app.rating_for(&b_name);

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
    // Roster changed (both went InGame); let the lobby reflect it.
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

/// Whether a resync request for one side may be granted right now, throttling a
/// client that re-sends `resync` on every frame it still sees a divergence (or a
/// buggy one that resyncs constantly) to at most one granted keyframe per second.
/// Records `now` into `last_grant` when granting, so the next call measures from
/// the grant, not from whenever the caller happens to check again. A pure function
/// of its arguments (no wall-clock read inside), so the rate limit is unit-testable
/// without the async tick loop.
fn resync_grant_allowed(last_grant: &mut Option<Instant>, now: Instant) -> bool {
    let allowed = match *last_grant {
        None => true,
        Some(last) => now.duration_since(last) >= Duration::from_secs(1),
    };
    if allowed {
        *last_grant = Some(now);
    }
    allowed
}

/// The per-match authoritative tick loop. Advances the deterministic engine on
/// the server's clock, broadcasts a snapshot to each client (~30Hz), and settles
/// on the natural end.
///
/// Reconnect grace (pause-both): when a human side's socket drops, the whole
/// bout freezes (sim and inputs halt) for up to [`REJOIN_GRACE`] while waiting
/// for that player to reconnect and reattach via [`BoutControl::Reattach`] (the
/// `rejoin` handler). The still-connected side is told `opponentReconnecting`; on
/// reattach both get a keyframe and `opponentResumed`, and play resumes from the
/// exact frozen state. If the grace expires the absent side forfeits. A bot side
/// dropping forfeits at once (bots don't refresh).
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
    let mut want_keyframe = [true, true]; // send one on the very first frame, both sides
    let mut connected = [true, true]; // is each side's socket currently attached?
    let mut spectators: Vec<mpsc::UnboundedSender<Message>> = Vec::new(); // live-view watchers
    let mut grace_until: Option<Instant> = None; // freeze deadline while a human reconnects
    // Per side: when this side's last resync-triggered keyframe was granted, so
    // `resync_grant_allowed` can throttle a client that asks repeatedly.
    let mut resync_last_grant: [Option<Instant>; 2] = [None, None];
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
                    // The reconnected client restarts its input `seq` at 0 (the resent
                    // matchStart below rebuilds its local game), so drop our ack baseline
                    // for this side to match; otherwise every fresh input would satisfy
                    // `seq <= ack`, get rejected, and the player's piece would snap back
                    // all match. Done before the snapshot so the client sees `ack:0`.
                    bout.reset_ack(side);
                    // The fresh client needs the match handoff, THEN a keyframe to resync.
                    let (opp, seed, side_str) = match side {
                        Side::A => (&name_b, seed_a, "A"),
                        Side::B => (&name_a, seed_b, "B"),
                    };
                    let _ = tx.send(Message::Text(
                        json!({"type":"matchStart","side":side_str,"seed":seed,"opponent":opp,"match_id":match_id}).to_string(),
                    ));
                    let _ = tx.send(Message::Text(bout.snapshot_message(side, true, true)));
                    // Both back? Lift the freeze, resync both boards, tell both to resume.
                    if connected[0] && connected[1] {
                        grace_until = None;
                        let resumed = json!({"type":"opponentResumed"}).to_string();
                        let _ = tx_a.send(Message::Text(bout.snapshot_message(Side::A, true, true)));
                        let _ = tx_a.send(Message::Text(resumed.clone()));
                        let _ = tx_b.send(Message::Text(bout.snapshot_message(Side::B, true, true)));
                        let _ = tx_b.send(Message::Text(resumed));
                        want_keyframe = [true, true];
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
                BoutControl::DebugGrant { side, weapon, funds } => {
                    // Out-of-band admin grant: mutate the authoritative game for `side`
                    // right here in the bout task (so it never races the tick loop). It
                    // is not a recorded input, so the replay/input stream is untouched;
                    // determinism for normal gameplay is preserved. Force a keyframe so
                    // the next snapshot resyncs the client with the granted arsenal/funds.
                    let (w, f) = bout.debug_grant(side, weapon, funds);
                    println!("bout {match_id}: admin grant side {side:?} weapon={weapon:?} funds={funds:?} (weapon_granted={w}, funds_applied={f})");
                    want_keyframe = [true, true];
                }
                BoutControl::Resync { side } => {
                    // Rate-limited: a client re-sends `resync` on every frame it still
                    // sees a mismatch, so only actually grant one keyframe per side per
                    // second (`resync_grant_allowed`); an over-eager repeat is dropped
                    // silently, and the client will simply ask again.
                    if resync_grant_allowed(&mut resync_last_grant[sidx(side)], Instant::now()) {
                        want_keyframe[sidx(side)] = true;
                        println!("bout {match_id}: resync grant side {side:?}");
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
            if frame.is_multiple_of(32) {
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
        // clients couldn't predict; push a prompt keyframe on the next send.
        if bout.take_dirty() {
            want_keyframe = [true, true];
        }
        // Forward the relay's cross-player events to each side's client, in order, so
        // its local sim applies the same effect this tick (the model-B event channel).
        // This is now the PRIMARY path: with the periodic keyframe retired above, an
        // ordinary weapon / funds tax / op-score reaches the client only as an event.
        // A keyframe still corrects the rare unpredictable cases (bazaar, Swap/Susan,
        // rejoin), but it no longer rides every snapshot to paper over a missed event.
        if connected[0] {
            for ev in bout.take_events_for(Side::A) {
                let _ = tx_a.send(Message::Text(json!({"type":"event","input": ev}).to_string()));
            }
        }
        if connected[1] {
            for ev in bout.take_events_for(Side::B) {
                let _ = tx_b.send(Message::Text(json!({"type":"event","input": ev}).to_string()));
            }
        }

        if bout.is_over() {
            // Final frame carries a keyframe so both clients settle on the end state.
            let _ = tx_a.send(Message::Text(bout.snapshot_message(Side::A, true, true)));
            let _ = tx_b.send(Message::Text(bout.snapshot_message(Side::B, true, true)));
            break Some(bout.result() == 1); // 1 = A won
        }
        // Snapshots go out at ~30Hz (every other 16ms tick); this is also where a
        // client disconnect is detected (the send fails). Under model B a keyframe is
        // sent only on a trigger (first frame, bazaar entry, Swap/Susan, rejoin, debug
        // grant, final), NOT periodically: the client tracks the server through the
        // event channel between keyframes. The spy reveal rides its own ~7.5Hz cadence.
        if frame.is_multiple_of(2) {
            // Each side's keyframe flag is cleared independently: a resync grant for
            // one side (part of divergence recovery) must not force an unwanted
            // keyframe on the other.
            let kf = [want_keyframe[0], want_keyframe[1]];
            want_keyframe = [false, false];
            let send_spy = frame.is_multiple_of(8);
            let a_ok = tx_a.send(Message::Text(bout.snapshot_message(Side::A, kf[0], send_spy))).is_ok();
            let b_ok = tx_b.send(Message::Text(bout.snapshot_message(Side::B, kf[1], send_spy))).is_ok();
            connected = [a_ok, b_ok];
            if !a_ok || !b_ok {
                // A *bot* dropping (no human is down) forfeits immediately. A human
                // dropping starts the freeze and waits for a reconnect.
                let human_down = (!a_ok && human[0]) || (!b_ok && human[1]);
                if !human_down {
                    // No human is down; only a bot dropped (incl. a bot-vs-bot match,
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
        // means the watcher closed the tab; drop them. No effect on the players.
        if frame.is_multiple_of(4) && !spectators.is_empty() {
            let msg = bout.spectator_message(&name_a, &name_b);
            spectators.retain(|tx| tx.send(Message::Text(msg.clone())).is_ok());
        }
        frame += 1;
    };

    // Snapshot the replay blob store once, up front (a cheap `Arc` clone), and
    // resolve the finished match's replay write BEFORE taking the app lock
    // below: in Bucket mode that's an async `put`, and `Shared`'s doc comment
    // requires the app mutex never be held across an `.await`.
    let store = state.lock().await.store.clone();
    let replay_write: Option<ReplayWrite> = match outcome {
        Some(_) if bout.is_over() => Some(prepare_replay_write(&store, &bout, &match_id, &name_a, &name_b).await),
        _ => None,
    };

    let mut app = state.lock().await;
    // The bout is over; drop its reattach registry entry so a late `rejoin` for
    // this match cleanly fails (`rejoinFailed`) instead of finding a dead loop.
    app.bouts.remove(&match_id);
    // Match over: both sides leave the bout and return to their pre-match
    // presence (see [`post_bout_status`]). A player who was "open to matches"
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
        // A forfeit (an intentional leave or a grace-window expiry, anything that
        // is not a natural top-out) doesn't latch a game-over on the winner's client
        // by itself, and the winner may be sitting on the "opponent reconnecting"
        // freeze. Tell whoever's still attached the opponent left. (The loser's tx
        // is dead on a disconnect, a harmless no-op; on an intentional leave the
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
        // Real per-player stats (the `players` table): record/streak/bests/time.
        // settle_bout has updated app.ratings; read each side's post-match rating.
        record_bout_player_stats(
            &app, &name_a, &name_b, a_won,
            (bout.score(Side::A), bout.lines(Side::A) as i64, bout.funds(Side::A)),
            (bout.score(Side::B), bout.lines(Side::B) as i64, bout.funds(Side::B)),
            bout.tick_count(),
            bout.is_over(), // natural finish only; a forfeit isn't a real top-out
        );
        // Persist the match as a VersusReplay for natural finishes only (a real
        // top-out the board reaches). A forfeit (a client disconnected) is not in
        // the seed+input stream, so its playback would never latch a winner; we
        // don't store those. `replay_write` was already resolved (and, in Bucket
        // mode, uploaded) above, before the app lock was taken.
        if let Some(write) = replay_write {
            match write {
                // Sampled out by the bot-vs-bot filler policy, or (Bucket mode) the
                // blob `put` failed: either way no row exists, so there's nothing to
                // insert and no `matchReplay` id to announce (one would point the
                // game-over screen's "Watch replay" button at nothing).
                ReplayWrite::Skip => {}
                ReplayWrite::Row { id, replay, json_col, lines } => {
                    let stored = app
                        .db
                        .lock()
                        .ok()
                        .and_then(|conn| {
                            db_insert_versus(&conn, &id, &replay, json_col.as_deref(), now_secs(), &name_a, &name_b, lines).ok()
                        })
                        .is_some();
                    if stored {
                        println!("stored online replay {id} ({} ticks)", replay.tick_count);
                    }
                    // Tell both clients the replay id so the game-over screen can offer
                    // a "Watch replay" button (best-effort; a disconnected client
                    // ignores it).
                    let msg = json!({ "type": "matchReplay", "id": id }).to_string();
                    let _ = tx_a.send(Message::Text(msg.clone()));
                    let _ = tx_b.send(Message::Text(msg));
                }
            }
        }
    }
    // Both players went back to Available (or left); refresh the lobby roster.
    maybe_broadcast_players(&mut app);

    // Quiesce-in-place: this bout is fully settled and removed from the registry. We
    // do not exit here; the deploy script polls the active-bout count and replaces
    // the machine once it reaches zero. (Self-exiting would just bounce us back up on
    // the OLD image via fly's restart policy, racing that poll.)
    if app.draining {
        println!("drain: bout ({match_id}) settled - {} bout(s) remaining", app.bouts.len());
    }
}

/// The `rating` frame a player receives when a bout settles: their new mu/sigma,
/// the conservative (μ−3σ) figure the ladder ranks on, and whether they won.
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
            // can still mint a token for any name via /api/identity; identity is
            // deliberately lightweight rather than account-grade, but that's a step
            // up from trivially spoofing a bare name.)
            if !app.ratings.contains_key(&clean) || clean == prior {
                return clean;
            }
        }
    }
    prior.to_string()
}

/// While this machine is draining for an in-place deploy, it accepts no NEW matches.
/// Tell the requester "matches paused" (the client shows a notice) but KEEP the
/// socket; there's no second machine to move to, and the in-place restart will
/// reconnect them onto the new version shortly. Returns true if the request was
/// refused (the caller should stop). Never touches `rejoin` (a mid-bout player
/// must still reach their bout here) or clients already in a bout.
async fn reject_if_draining(state: &Shared, id: &str) -> bool {
    let app = state.lock().await;
    if !app.draining {
        return false;
    }
    if let Some(c) = app.clients.get(id) {
        let _ = c.tx.send(Message::Text(json!({"type":"draining"}).to_string()));
    }
    true
}

/// Dispatch one client text frame by its `type`, the lobby/matchmaking protocol's
/// whole inbound surface. A malformed frame or an unknown `type` is ignored
/// (the wire is untrusted; nothing crashes a connection).
///
/// The arms split into presence/identity (`watch`/`active`/`available`), pairing
/// (`queue`/`challenge`/`challengeAccept`/`challengeDecline`), and in-bout control
/// (`input`/`resync`/`rejoin`/`leaveMatch`/`spectate`). A matchmaking arm that pairs two
/// clients returns a [`PendingBout`] from inside the lock and spawns [`run_bout`]
/// for it outside the lock (the tick loop must not hold the `App` mutex).
async fn handle_message(state: &Shared, id: &str, text: &str) {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        // A page opened: count it as a visitor (persistent hit counter), then
        // push the current numbers to everyone so the new page is populated.
        // (It isn't an active player yet; that needs a gameplay button.)
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
            // Send the current lobby roster to this just-connected client; the
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
            if reject_if_draining(state, id).await {
                return;
            }
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
        // `token`/`name`) marks this client "open to matches": both challengeable
        // and auto-pairable. `{"value":false}` leaves the roster. Going Available
        // also attempts an immediate auto-pair (a directed challenge isn't required
        // to get into a game).
        Some("available") => {
            if reject_if_draining(state, id).await {
                return;
            }
            let value = v.get("value").and_then(|b| b.as_bool()).unwrap_or(true);
            let pending = {
                let mut app = state.lock().await;
                if app.clients.get(id).is_some_and(|c| c.bout.is_some()) {
                    None // already in a match; ignore
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
            if reject_if_draining(state, id).await {
                return;
            }
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
                // Target offline / busy / self; decline immediately.
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
        // A gameplay action in a server-authoritative match; forward it to that
        // match's tick loop, which validates and applies it. {seq, input}.
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
        // The client detected a lock-hash mismatch against its own local sim (see
        // `Snapshot::lock_hash`) and is asking for a fresh keyframe to resync. Routed
        // to the client's own bout via its server-side binding, exactly like
        // `leaveMatch` below (never a client-supplied match/side). Dropped silently
        // if the client isn't in a bout, the match is unknown, or the bout's control
        // channel is full: a lost resync request only delays the repair until the
        // client notices the mismatch again and re-asks, so there's nothing to fail
        // loudly about here.
        Some("resync") => {
            let app = state.lock().await;
            let target = app.clients.get(id).and_then(|c| match (c.match_id.as_ref(), c.bout.as_ref()) {
                (Some(mid), Some((_, side))) => Some((mid.clone(), *side)),
                _ => None,
            });
            if let Some((mid, side)) = target {
                if let Some(h) = app.bouts.get(&mid) {
                    let _ = h.control.try_send(BoutControl::Resync { side });
                }
            }
        }
        // Reconnect after an accidental refresh: the client parked the match in its
        // URL (`?match=<id>`) and now reattaches to the still-running, frozen bout.
        // Requires a signed token whose name is one of the two participants, so a
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
                // No such live bout / not a participant / the loop just ended; fail
                // loudly so the client clears the URL and returns to the lobby.
                send(&app, id, &json!({"type":"rejoinFailed"}));
            }
        }
        // Intentional in-app "Leave game": forfeit this client's own bout right away
        // (no reconnect grace; that's only for an accidental socket drop). We use
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
        // bout's spectator stream. No identity/token needed; spectators send no
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

/// `GET /ws`: complete the WebSocket upgrade and hand the socket to
/// [`handle_socket`] for the lifetime of the connection.
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Shared>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Own one client connection end to end: register a fresh [`Client`], spawn the
/// writer and keepalive-ping tasks, pump inbound frames through [`handle_message`],
/// and clean up on disconnect.
///
/// The socket is split into a reader (this task's loop) and a writer task fed by
/// an unbounded channel, so every other part of the server sends to this client by
/// enqueuing a [`Message`]; it never awaits the socket directly and a slow client
/// cannot stall the matchmaking lock. The ws Ping/Pong loop doubles as the lobby's
/// per-player latency probe. On disconnect we notify a non-bout peer, retract any
/// pending challenges, and drop the connection; an in-bout drop is left to the
/// bout's tick loop (which holds its own writer clone) so the reconnect grace can
/// apply.
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
    // the bot auto-Pong, and the read loop turns the Pong into a round-trip time
    // for the lobby's per-player latency display. Stops when the client is gone.
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
            .filter(|&(_cid, (tid, _))| tid == &id).map(|(cid, (_tid, _))| cid.clone())
            .collect();
        let by = a.clients.get(&id).map(|c| c.name.clone()).unwrap_or_default();
        for cid in aimed_here {
            clear_challenge(&mut a, &cid);
            send(&a, &cid, &json!({"type":"challengeDeclined","by":by}));
        }
        // A mid-bout disconnect is NOT a forfeit: the bout's tick loop sees the
        // dropped socket and enters the reconnect-grace freeze (the player may be
        // refreshing), forfeiting only if they don't reattach in time. So only fire
        // `opponentLeft` for a (non-bout) peer link; the loop owns the in-bout case.
        // We still remove this dead connection below; the loop holds its own `tx`
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
        // If a named/listed client left, the lobby roster changed; push it.
        if was_listed {
            maybe_broadcast_players(&mut a);
        }
        // If an active player's page closed, the live count may drop; recompute
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
// browsing is one indexed `SELECT ... ORDER BY created_at` rather than a scan
// that opens and parses every file. A recording uploaded by the "report a bug" button or
// the 🔗 Share button lands here keyed by a content hash (dedup), and is fetched
// back by id. A plain on-disk JSON directory, if present, is imported into the DB
// once at startup (see `import_dir`).

/// Path to the SQLite database (`REPLAY_DB`, default `replays.db`; prod sets
/// `/data/replays.db` on the fly volume).
fn db_path() -> String {
    std::env::var("REPLAY_DB").unwrap_or_else(|_| "replays.db".to_string())
}

/// On-disk JSON replay directory, imported into the DB once at startup if present
/// (`REPLAYS_DIR`, default `replays`; prod points it at `/data/replays`).
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

/// Ids are our own hex hashes; reject anything else as a defensive check (the DB
/// binds ids as parameters, so this is belt-and-suspenders against malformed input).
fn valid_replay_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Wall-clock seconds since the Unix epoch, used as the `created_at` stamp for
/// stored recordings (0 if the clock is somehow before the epoch).
fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// The DB schema, applied idempotently at every boot (`CREATE TABLE IF NOT
/// EXISTS`). Three tables: `replays` (the recording store + browse metadata),
/// `counters` (the persistent visit counter), and `players` (per-player stats).
/// Columns present in `SCHEMA` but missing from an older existing table are
/// reconciled by [`init_schema`] using idempotent `ALTER TABLE ADD COLUMN`.
/// `json` is nullable (not the original constraint; see
/// [`relax_replays_json_not_null`]) so a Bucket-mode or migrated row can leave
/// it NULL once its blob lives in the store instead.
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
    json        TEXT,
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

/// Create the schema and reconcile the `replays` columns that `CREATE TABLE IF NOT
/// EXISTS` cannot add to an already-existing table. Each `ADD COLUMN` is
/// idempotent: the duplicate-column error is ignored, and existing rows read
/// newly added columns as `NULL` until written. Runs on every boot.
fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    // Optional replay title.
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN title TEXT", []);
    // Per-online-match player names (the library's "Alice vs Bob" + profile links).
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN name_a TEXT", []);
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN name_b TEXT", []);
    // Total lines cleared in the recording (shown in the library); a NULL is
    // filled in by `backfill_lines`.
    let _ = conn.execute("ALTER TABLE replays ADD COLUMN lines INTEGER", []);
    // Must run after the ADD COLUMNs above: it rebuilds the table from whatever
    // columns already exist, so an old db missing e.g. `lines` needs that column
    // added FIRST or the rebuild would drop it.
    relax_replays_json_not_null(conn)?;
    Ok(())
}

/// Blob storage moves the replay JSON out of the row and into a bucket object,
/// leaving `json` NULL for a migrated or Bucket-mode row (see
/// [`ReplayStore::Bucket`] and [`migrate_replay_blobs`]). The original schema
/// declared `json TEXT NOT NULL`; SQLite has no `ALTER TABLE` form that drops a
/// column constraint, so relaxing it means rebuilding the table once, the same
/// one-time-then-forever-idempotent shape as the `ALTER TABLE ADD COLUMN`
/// reconciliations above, just done as a full rebuild instead of an add. A
/// table that's never had the constraint (a brand-new db, or one already
/// rebuilt by a prior boot) is left untouched: this is a no-op after the first
/// boot that runs it.
fn relax_replays_json_not_null(conn: &Connection) -> rusqlite::Result<()> {
    // `notnull` is quoted: unquoted, SQLite parses it as the `NOTNULL` postfix
    // operator (a spelling of `IS NOT NULL`) rather than the pragma's column name.
    let not_null: i64 = conn.query_row(
        "SELECT \"notnull\" FROM pragma_table_info('replays') WHERE name = 'json'",
        [],
        |r| r.get(0),
    )?;
    if not_null == 0 {
        return Ok(());
    }
    let rebuild = conn.execute_batch(
        "BEGIN;
         ALTER TABLE replays RENAME TO replays_pre_blob_storage;
         CREATE TABLE replays (
             id          TEXT PRIMARY KEY,
             mode        TEXT NOT NULL,
             seed        INTEGER NOT NULL,
             ai_level    INTEGER,
             tick_count  INTEGER NOT NULL,
             inputs      INTEGER NOT NULL,
             engine_sha  TEXT NOT NULL,
             created_at  INTEGER NOT NULL,
             json        TEXT,
             title       TEXT,
             name_a      TEXT,
             name_b      TEXT,
             lines       INTEGER
         );
         INSERT INTO replays
             (id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title, name_a, name_b, lines)
         SELECT id, mode, seed, ai_level, tick_count, inputs, engine_sha, created_at, json, title, name_a, name_b, lines
             FROM replays_pre_blob_storage;
         DROP TABLE replays_pre_blob_storage;
         CREATE INDEX IF NOT EXISTS idx_replays_created ON replays(created_at);
         COMMIT;",
    );
    if let Err(e) = rebuild {
        // The rebuild copies the whole table, so a full volume fails it. Roll back
        // the partial transaction (the RENAME may already have landed) so `replays`
        // is left intact under its original name and a later call can retry.
        let _ = conn.execute_batch("ROLLBACK;");
        // A full disk here is not fatal: this relaxation only enables Bucket-mode
        // and migrated (json NULL) writes, and the boot prune runs right after this
        // to free space, after which `main` retries this rebuild with room. Reads
        // and legacy inline writes work regardless, so a still-full disk keeps the
        // server serving rather than crash-looping the boot before it can self-heal.
        // Any other error is a real schema fault and propagates.
        if is_disk_full(&e) {
            eprintln!("relax replays.json NOT NULL deferred (disk full; retried after the boot prune): {e}");
            return Ok(());
        }
        return Err(e);
    }
    Ok(())
}

/// Whether a rusqlite error is the "database or disk is full" failure (SQLITE_FULL).
/// Boot steps that only need to succeed once the volume has room (the json NOT NULL
/// relaxation, the blob migration) treat this as retryable-later rather than fatal,
/// so a full disk degrades to serving instead of crash-looping the boot.
fn is_disk_full(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::DiskFull)
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

/// One-time (per row, then forever a no-op) boot migration: move every
/// existing row's JSON out of SQLite and into the bucket. Runs before the
/// server starts listening, right after the bot-replay prune, and only in
/// Bucket mode; there's nowhere to move a blob to in Sqlite mode, so that mode
/// skips this entirely.
///
/// Per row, the blob is `put` BEFORE the `json` column is NULLed, so a crash
/// mid-migration only ever leaves a re-uploadable row (json still inline)
/// behind, never a NULLed row with no blob: the metadata-row-pointing-at-a-
/// missing-blob failure mode is impossible by the same construction as the
/// settle path's write (see [`prepare_replay_write`]). The predicate (`json IS
/// NOT NULL`) is exactly what makes a re-run resumable: a crash-interrupted or
/// already-finished pass both leave nothing further for the query to find.
///
/// Rows are pulled in bounded batches (the NULLing makes `LIMIT` re-queries
/// walk forward on their own) so peak memory stays a few MB no matter how many
/// rows are pending. The machine has 256MB; loading the whole pre-prune corpus
/// (~1GB of JSON) in one Vec would OOM the server into a boot loop precisely
/// when the prune had failed and the migration mattered most.
async fn migrate_replay_blobs(conn: &Connection, store: &Arc<dyn ObjectStore>) -> Result<usize, Box<dyn std::error::Error>> {
    const BATCH: usize = 500;
    let started = Instant::now();
    let mut migrated = 0usize;
    loop {
        let batch: Vec<(String, String)> = {
            let mut stmt =
                conn.prepare("SELECT id, json FROM replays WHERE json IS NOT NULL LIMIT ?1")?;
            let rows = stmt
                .query_map([BATCH as i64], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };
        if batch.is_empty() {
            break;
        }
        for (id, json) in batch {
            let path = replay_blob_path(&id);
            store.put(&path, json.into()).await?;
            conn.execute("UPDATE replays SET json = NULL WHERE id = ?1", [&id])?;
            migrated += 1;
            if migrated.is_multiple_of(500) {
                println!("replay blob migration: {migrated} row(s) so far");
            }
        }
    }
    println!("replay blob migration: moved {migrated} row(s) in {:.1}s", started.elapsed().as_secs_f64());
    Ok(migrated)
}

/// Total lines cleared across all boards of a single-board (practice / vs-Computer)
/// recording, counted by deterministically replaying to the end.
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

/// Fill the `lines` column for any row that still has it NULL, by deterministically
/// replaying the recording to count cleared lines (best-effort: malformed JSON is
/// skipped). Once every row has a value the query matches nothing, so later boots
/// are no-ops. `json IS NOT NULL` is belt-and-suspenders: every write path fills
/// `lines` at insert time, so a Bucket-mode row (json NULL) should never reach
/// here with `lines` still NULL, but this runs before the blob migration ever
/// gets a chance to NULL a row's json, so nothing here needs to reach the bucket.
fn backfill_lines(conn: &Connection) {
    let rows: Vec<(String, String)> =
        match conn.prepare("SELECT id, json FROM replays WHERE lines IS NULL AND json IS NOT NULL") {
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

// --- bot-vs-bot replay retention policy --------------------------------
//
// The always-on house bot ("The Count") duels the regional bots around the
// clock; every natural finish used to persist a replay, and 87k of those
// filled the volume with games nobody will ever watch. Human games are rare
// and precious and are always kept; bot-vs-bot filler is sampled down to 5%,
// both for new matches as they settle and, once, for the existing backlog.

/// True for a server-controlled bot identity: the roaming "The Count", or a
/// regional persona named "<Region>-Bert" / "<Region>-Ernie" (see bt-bot's
/// `Persona`). A replay row only carries `name_a`/`name_b`, not a bot flag,
/// so the retention policy below has to classify by name alone. A human who
/// registers a name ending in "-Bert" or "-Ernie" would have their games
/// against an actual bot swept into the same-name bucket and sampled at 5%;
/// that's an acceptable misclassification for untitled throwaway games, and
/// a real human opponent (see `bot_filler_replay`) or a titled showcase
/// still keeps the replay regardless.
fn is_bot_name(name: &str) -> bool {
    name == "The Count" || name.ends_with("-Bert") || name.ends_with("-Ernie")
}

/// Deterministic 5% sampler over a stable string key. Hashes with FNV-1a
/// (hand-rolled, like `bt_core`'s lock hash) because the sampler's answer must
/// be stable across BINARY versions, and std's `DefaultHasher` documents its
/// algorithm as unspecified between Rust releases: a toolchain bump changing
/// the hash would re-roll every already-decided row at the next boot prune and
/// delete formerly-kept survivors. With a fixed hash, the persist-time skip
/// decision (keyed on the match id) and the startup prune (keyed on the stored
/// replay id) enforce exactly the same predicate, just over two different keys
/// (see `db_prune_bot_replays` for why the prune can't reuse the match id),
/// and a key that says "keep" keeps saying "keep" forever.
fn replay_retained(key: &str) -> bool {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h.is_multiple_of(20)
}

/// True when a replay is disposable bot-vs-bot filler: both names classify
/// as bot names ([`is_bot_name`]) and the replay carries no title. A title
/// marks a curated showcase, kept regardless of who played it; a replay with
/// a human on either side is precious regardless of title. Only the
/// intersection, an untitled bot-vs-bot grind, is filler.
fn bot_filler_replay(name_a: &str, name_b: &str, title: Option<&str>) -> bool {
    title.is_none() && is_bot_name(name_a) && is_bot_name(name_b)
}

/// Delete bot-vs-bot filler rows down to the [`replay_retained`] 5% policy.
/// Called once at boot, right after `db_init`, to prune the existing backlog
/// under the same rule new matches are sampled with at persist time (see
/// [`prepare_replay_write`]'s sampling gate). Does NOT `VACUUM`; the boot
/// sequence runs [`db_vacuum_if_worthwhile`] once at the very end, after this
/// AND the blob migration have each freed their own space, rather than paying
/// the rewrite cost twice.
///
/// The stored `id` column is the replay's content hash, not the match id the
/// persist-time decision hashed; a match id is never written anywhere a
/// prune could read it back from, so this samples by content-hash id instead.
/// That's a different key from the persist-time decision, but the same
/// policy and the same hash function: two keys, one policy, and each row's
/// fate is fixed forever by whichever key governs it. That's what keeps this
/// idempotent across boots. Re-running it after a prior prune finds every
/// surviving row still hashing "keep" and deletes nothing.
///
/// The name predicate mirrors [`is_bot_name`] directly in SQL (there are only
/// two suffixes and one fixed name, so keeping them in sync by hand is
/// cheap) so SQLite can select candidate rows itself; the 5% hash selection
/// stays in Rust, in the one place [`replay_retained`] is defined, instead of
/// reimplementing `DefaultHasher` as a SQL expression.
fn db_prune_bot_replays(conn: &Connection) -> rusqlite::Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT id FROM replays WHERE title IS NULL \
         AND (name_a = 'The Count' OR name_a LIKE '%-Bert' OR name_a LIKE '%-Ernie') \
         AND (name_b = 'The Count' OR name_b LIKE '%-Bert' OR name_b LIKE '%-Ernie')",
    )?;
    let candidates: Vec<String> =
        stmt.query_map([], |row| row.get::<_, String>(0))?.collect::<rusqlite::Result<_>>()?;
    let to_delete: Vec<&str> = candidates.iter().map(String::as_str).filter(|id| !replay_retained(id)).collect();
    if to_delete.is_empty() {
        return Ok(0);
    }

    conn.execute_batch("BEGIN;")?;
    for id in &to_delete {
        if let Err(e) = conn.execute("DELETE FROM replays WHERE id = ?1", [*id]) {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(e);
        }
    }
    conn.execute_batch("COMMIT;")?;
    Ok(to_delete.len())
}

/// `VACUUM` the replay DB if its freelist has grown past a worthwhile amount.
/// Split out of `db_prune_bot_replays` so the boot sequence can run this ONCE,
/// after both the bot-replay prune and (in Bucket mode) the blob migration have
/// each freed their own space, instead of vacuuming twice.
///
/// `VACUUM` rewrites the whole file and needs scratch space roughly the size of
/// the DB, so it only runs once whatever freed space is worth the rewrite; the
/// fly volume must already have that headroom before the first such boot, since
/// that boot is the one walking the largest backlog.
fn db_vacuum_if_worthwhile(conn: &Connection) -> rusqlite::Result<bool> {
    let freelist_pages: i64 = conn.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
    let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let vacuumed = if freelist_pages * page_size > 100 * 1024 * 1024 {
        conn.execute_batch("VACUUM;")?;
        true
    } else {
        false
    };
    Ok(vacuumed)
}

/// What a finished bout's replay resolves to, decided (and in Bucket mode,
/// already uploaded) by [`prepare_replay_write`] before the settle path takes
/// the app lock.
enum ReplayWrite {
    /// Sampled out by the bot-vs-bot filler policy, or (Bucket mode) the blob
    /// `put` failed: either way no row is written. A metadata row that points at
    /// a missing blob is the failure mode blob storage is designed to make
    /// impossible; an orphan blob left behind by a skipped row is harmless.
    Skip,
    /// A row to insert. `json_col` is the full replay text in Sqlite mode, or
    /// `None` in Bucket mode once the blob is already durably stored at
    /// `replays/<id>.json`.
    Row { id: String, replay: VersusReplay, json_col: Option<String>, lines: i64 },
}

/// Resolve a finished bout's replay write. The bot-vs-bot 5% sampling gate
/// (see [`bot_filler_replay`]/[`replay_retained`]) runs first and is untouched
/// by blob storage; only what happens to a replay that survives it depends on
/// `store`. In Bucket mode the JSON is uploaded to `replays/<id>.json` here,
/// before the caller ever touches SQLite, so a metadata row is only ever
/// written once its blob is confirmed durable. A failed upload is logged loudly
/// with the match id and the whole row is skipped rather than written pointing
/// at nothing.
///
/// Async only for the Bucket-mode upload; called before the settle path's app
/// lock is taken (see the `.await` discipline noted on [`Shared`]).
async fn prepare_replay_write(
    store: &ReplayStore,
    bout: &Bout,
    match_id: &str,
    name_a: &str,
    name_b: &str,
) -> ReplayWrite {
    let replay = bout.to_replay(bout::TICK_MS, ENGINE_SHA);
    let json = replay.to_json();
    let id = replay_id(&json);
    let lines = bout.lines(Side::A) as i64 + bout.lines(Side::B) as i64;
    // The Count's regional bot-vs-bot filler is sampled to 5% here, before
    // anything is written or uploaded (see `bot_filler_replay`/`replay_retained`).
    // The sampling key is `match_id`, the bout's wire id, not the content hash
    // `id` above: a sampled-out replay never gets a row, so there's nothing to
    // hash by content, and `match_id` is what's already in scope at settle time.
    // (The startup prune in `db_prune_bot_replays` samples EXISTING rows
    // instead, keyed on the stored content-hash `id`, since a match id is never
    // written anywhere the prune could read it back from.)
    if bot_filler_replay(name_a, name_b, None) && !replay_retained(match_id) {
        println!("bout {match_id}: bot-vs-bot replay sampled out (5% retention)");
        return ReplayWrite::Skip;
    }
    match store {
        ReplayStore::Sqlite => ReplayWrite::Row { id, replay, json_col: Some(json), lines },
        ReplayStore::Bucket(os) => {
            let path = replay_blob_path(&id);
            match os.put(&path, json.into()).await {
                Ok(_) => ReplayWrite::Row { id, replay, json_col: None, lines },
                Err(e) => {
                    eprintln!("bout {match_id}: replay blob put failed for {id}: {e}");
                    ReplayWrite::Skip
                }
            }
        }
    }
}

/// Store a server-recorded online (`Versus`) match. Same `replays` table, mode
/// `"Online"`, `seed` = side A's seed; `json` is the full [`VersusReplay`] text
/// (two seeds and the ordered input stream) in Sqlite mode, or `None` in Bucket
/// mode once [`prepare_replay_write`] has already uploaded it to
/// `replays/<id>.json`. Content id and row are always inserted together (the
/// caller decides `json_col`); this fn never touches the blob store itself.
// 8 columns to persist; bundling them into a struct wouldn't earn its keep.
#[allow(clippy::too_many_arguments)]
fn db_insert_versus(
    conn: &Connection,
    id: &str,
    r: &VersusReplay,
    json: Option<&str>,
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

/// Fetch a recording's raw JSON by id, with no name injection and no blob-store
/// fallback. The served endpoint uses [`db_get_with_names`] plus
/// [`resolve_replay_json`]; this focused helper is the plain-inline-JSON form
/// tests read against (only ever pointed at Sqlite-mode rows, where `json` is
/// always present).
#[allow(dead_code)]
fn db_get(conn: &Connection, id: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT json FROM replays WHERE id = ?1", [id], |row| row.get::<_, String>(0))
        .optional()
}

/// A replay row's JSON column (`None` for a Bucket-mode row whose blob lives in
/// the store instead) plus its optional stored player names (`name_a`/`name_b`).
type ReplayWithNames = (Option<String>, Option<String>, Option<String>);

/// Fetch a recording's JSON column plus its stored player names (an online
/// VersusReplay row has `name_a`/`name_b`; practice/vs-computer rows leave them
/// NULL). A `None` json is a migrated or Bucket-mode row; the caller resolves
/// the actual body with [`resolve_replay_json`]. Used to label the
/// single-replay viewer with the real names instead of "Player A/B".
fn db_get_with_names(
    conn: &Connection,
    id: &str,
) -> rusqlite::Result<Option<ReplayWithNames>> {
    conn.query_row(
        "SELECT json, name_a, name_b FROM replays WHERE id = ?1",
        [id],
        |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
    )
    .optional()
}

/// Resolve a replay row's JSON body: the row's `json` column if non-NULL (a
/// legacy row, or any row in Sqlite mode), otherwise fetched from the bucket (a
/// migrated or Bucket-mode row). Never touches the store for a non-NULL column,
/// so a legacy row serves with no blob-store dependency at all. A NULL column
/// with no bucket configured, or a NULL column whose blob is missing, is a
/// server-side error, never an empty replay: either means metadata exists with
/// nowhere to find its content, which the write path ([`prepare_replay_write`])
/// is designed to make impossible, so seeing it here means something upstream
/// is broken and callers must surface that loudly rather than paper over it.
async fn resolve_replay_json(store: &ReplayStore, id: &str, json_col: Option<String>) -> Result<String, String> {
    if let Some(json) = json_col {
        return Ok(json);
    }
    match store {
        ReplayStore::Bucket(os) => {
            let path = replay_blob_path(id);
            let bytes = os
                .get(&path)
                .await
                .map_err(|e| format!("bucket get replays/{id}.json: {e}"))?
                .bytes()
                .await
                .map_err(|e| format!("bucket read replays/{id}.json: {e}"))?;
            String::from_utf8(bytes.to_vec()).map_err(|e| format!("bucket blob replays/{id}.json not utf8: {e}"))
        }
        ReplayStore::Sqlite => Err(format!("replay {id} has a NULL json column and no bucket is configured")),
    }
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

/// A player's stored stats, one row in the `players` table. `None` figures are
/// "never recorded yet". `mu`/`sigma` mirror `ratings.json` so a profile lookup
/// is one indexed read; `ratings.json` stays the matchmaking source-of-truth.
#[derive(Debug, Clone, PartialEq)]
struct PlayerStats {
    name: String,
    mu: f64,
    sigma: f64,
    games: i64,
    wins: i64,
    losses: i64,
    /// Length of the current unbroken run, of the kind named by `streak_type`.
    streak: i64,
    /// Which result the current `streak` counts: `"wins"` or `"losses"`. `None`
    /// only before the player's first recorded match.
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
    // Kill/death timing records only count real top-outs; a forfeit's length is
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

/// One-time seed of the `players` table from `ratings.json`, run only when the
/// table is empty so it never clobbers accumulated stats. mu/sigma/experience map
/// to mu/sigma/games (the rating's experience is its games-played count).
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

/// Import legacy single-game `Replay` JSON files from `dir` into the DB at
/// startup, so a plain JSON directory (e.g. a fly volume) folds into the indexed
/// store. Only files that parse as [`Replay`] are imported; `VersusReplay` JSON
/// is not handled here. Idempotent (`INSERT OR IGNORE` by content id),
/// best-effort: malformed files are skipped, and each file's mtime is preserved
/// as `created_at` so ordering by recency survives the import. Returns the number
/// newly inserted.
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

/// Upper bound on a stored recording's `tick_count`. Storing a replay re-plays it
/// to the end (`run_to_end` ticks the engine once per tick) to compute its line
/// total, and `POST /api/replays` is unauthenticated with an attacker-controlled
/// `tick_count`, so the replay work must be bounded. This ceiling is multiple hours
/// of play at the engine's tick rate, far above any genuine recording, while keeping
/// the per-upload work finite.
const MAX_REPLAY_TICKS: u32 = 1_000_000;

/// `POST /api/replays`: store a recording, return `{"id": "..."}`. Validates
/// it parses as a [`Replay`] first so we never persist junk.
async fn post_replay(State(db): State<Db>, body: String) -> impl IntoResponse {
    let replay = match Replay::from_json(&body) {
        Ok(r) => r,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid replay").into_response(),
    };
    if replay.tick_count > MAX_REPLAY_TICKS {
        return (StatusCode::BAD_REQUEST, "replay too long").into_response();
    }
    // Persisting computes the line total by deterministically replaying to the end;
    // CPU work is proportional to `tick_count`. Run it on the blocking pool so a burst
    // of uploads can't starve the async runtime (matchmaking, WS heartbeats).
    let stored = tokio::task::spawn_blocking(move || {
        let id = replay_id(&body);
        let conn = db.lock().unwrap();
        db_insert(&conn, &id, &replay, &body, now_secs()).map(|_| id)
    })
    .await;
    match stored {
        Ok(Ok(id)) => (StatusCode::OK, Json(json!({ "id": id }))).into_response(),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "store unavailable").into_response(),
    }
}

/// `GET /api/replays/:id`: fetch a stored recording as JSON. A legacy or
/// Sqlite-mode row serves straight from the DB; a migrated or Bucket-mode row
/// (json NULL) is fetched from the bucket (see [`resolve_replay_json`]).
async fn get_replay(
    State(db): State<Db>,
    State(store): State<ReplayStore>,
    Path(id): Path<String>,
) -> impl IntoResponse {
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
    let (json_col, name_a, name_b) = match found {
        Some(row) => row,
        None => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    let txt = match resolve_replay_json(&store, &id, json_col).await {
        Ok(txt) => txt,
        Err(e) => {
            eprintln!("get_replay blob error for {id}: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "replay unavailable").into_response();
        }
    };
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

/// `GET /replay/:id`: the shareable playback link. Redirects to the static
/// player page, which fetches the recording from `/api/replays/:id`.
async fn replay_page(Path(id): Path<String>) -> impl IntoResponse {
    if !valid_replay_id(&id) {
        return (StatusCode::BAD_REQUEST, "bad id").into_response();
    }
    Redirect::temporary(&format!("/www/replay.html?id={id}")).into_response()
}

/// `GET /api/replays`: list stored recordings (newest first, capped) with just
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
// as an Elo-styled figure, a cosmetic linear transform so the board reads like
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

/// `GET /api/leaderboard`: players ranked by conservative TrueSkill, Elo-styled.
/// Backs `leaderboard.html`.
async fn leaderboard(State(state): State<Shared>) -> impl IntoResponse {
    let players = {
        let app = state.lock().await;
        rank_players(&app.ratings)
    };
    Json(json!({ "players": players })).into_response()
}

/// `GET /api/debug/matches`: the live-match debug list; every in-progress bout
/// (`match_id` and the two player names), so a spectator can pick one to watch
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

/// Constant-time byte-slice equality: no early-out on the first differing byte, so
/// comparing a presented secret against the configured one yields no timing oracle
/// for the secret's bytes. (A length mismatch is allowed to short-circuit; only the
/// equal-length content comparison must be masked.) Mirrors the constant-time MAC
/// check the identity layer relies on.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify the `x-admin-token` header against `BT_ADMIN_TOKEN`. With that env unset the
/// admin endpoints are closed (fail-closed; a missing token never silently opens them).
fn admin_authed(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let configured = std::env::var("BT_ADMIN_TOKEN").ok().filter(|t| !t.is_empty());
    let presented = headers.get("x-admin-token").and_then(|h| h.to_str().ok());
    match (configured.as_deref(), presented) {
        (Some(want), Some(got)) if ct_eq(want.as_bytes(), got.as_bytes()) => Ok(()),
        (None, _) => Err((StatusCode::FORBIDDEN, "admin disabled: BT_ADMIN_TOKEN not set\n".into())),
        _ => Err((StatusCode::FORBIDDEN, "forbidden\n".into())),
    }
}

/// `POST /admin/drain`: begin a quiesce-in-place drain (called at the start of a
/// deploy). Flips [`App::draining`] so no new matches start, and notifies every
/// lobby (non-bout) client that matches are paused (they stay connected; the
/// in-place restart reconnects them onto the new version). Returns the count of
/// bouts still in flight; the deploy script polls `/api/debug/matches` until that
/// hits zero, then replaces the machine, so no live game is killed. Idempotent.
async fn admin_drain(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = admin_authed(&headers) {
        return resp;
    }
    let mut app = state.lock().await;
    app.draining = true;
    let notice = json!({"type":"draining"}).to_string();
    for c in app.clients.values() {
        if c.bout.is_none() {
            let _ = c.tx.send(Message::Text(notice.clone()));
        }
    }
    let bouts = app.bouts.len();
    println!("drain: started; new matches paused, {bouts} bout(s) to finish");
    (StatusCode::OK, format!("draining; {bouts} bout(s) in flight\n"))
}

/// `POST /admin/resume`: clear the drain flag (an `undrain`), used by the deploy
/// script to roll back if the deploy aborts after draining, so the lobby isn't
/// left paused. Tells lobby clients matches are open again. Idempotent.
async fn admin_resume(State(state): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    if let Err(resp) = admin_authed(&headers) {
        return resp;
    }
    let mut app = state.lock().await;
    app.draining = false;
    let notice = json!({"type":"resumed"}).to_string();
    for c in app.clients.values() {
        if c.bout.is_none() {
            let _ = c.tx.send(Message::Text(notice.clone()));
        }
    }
    println!("drain: resumed; matchmaking re-enabled");
    (StatusCode::OK, "resumed\n".to_string())
}

/// `POST /admin/grant`: a gated dev tool to inject a weapon and/or funds into a
/// live online bout, so a developer can exercise the cross-player weapon relay with
/// a single `curl` instead of two coordinated browser tabs. Body JSON:
///   `{ "match_id": "<string>", "side": "A"|"B", "weapon": <0..=33 optional>, "funds": <i64 optional> }`
/// At least one of `weapon`/`funds` must be present.
///
/// Uses the same fail-closed gate as the other `/admin/*` endpoints (`admin_authed`
/// first, 403 if the `x-admin-token` header is missing/wrong, or `BT_ADMIN_TOKEN`
/// is unset). It injects only a weapon/funds grant (never board state or an
/// arbitrary input), routed through the bout's own task, so there is no determinism
/// surface: the grant is not recorded into the replay's input stream (see
/// [`BoutControl::DebugGrant`] and [`Bout::debug_grant`]).
async fn admin_grant(State(state): State<Shared>, headers: HeaderMap, body: String) -> impl IntoResponse {
    if let Err((code, msg)) = admin_authed(&headers) {
        return (code, msg).into_response();
    }
    let bad = |m: &str| (StatusCode::BAD_REQUEST, format!("{m}\n")).into_response();

    let v: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return bad("malformed JSON body"),
    };
    let match_id = match v.get("match_id").and_then(|m| m.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return bad("match_id (non-empty string) required"),
    };
    let side = match v.get("side").and_then(|s| s.as_str()) {
        Some("A") => Side::A,
        Some("B") => Side::B,
        _ => return bad("side must be \"A\" or \"B\""),
    };
    // `weapon` is the wire index 0..=33 (WeaponToken::from_index validates the range).
    let weapon = match v.get("weapon") {
        None | Some(Value::Null) => None,
        Some(w) => match w.as_i64().and_then(|i| i32::try_from(i).ok()).and_then(WeaponToken::from_index) {
            Some(tok) => Some(tok),
            None => return bad("weapon must be an integer in 0..=33"),
        },
    };
    let funds = match v.get("funds") {
        None | Some(Value::Null) => None,
        Some(f) => match f.as_i64() {
            Some(n) => Some(n),
            None => return bad("funds must be an integer (i64)"),
        },
    };
    if weapon.is_none() && funds.is_none() {
        return bad("at least one of weapon/funds must be present");
    }

    // Look up the LIVE bout and hand the grant to its task (out-of-band control
    // channel), so the mutation happens inside the bout's own loop and never races
    // the tick. try_send (not await): we never hold the App lock across a send, and a
    // full control channel means the bout is wedged; surface that rather than block.
    let control = {
        let app = state.lock().await;
        app.bouts.get(&match_id).map(|h| h.control.clone())
    };
    let control = match control {
        Some(c) => c,
        None => return (StatusCode::NOT_FOUND, "no such live match\n".to_string()).into_response(),
    };
    if control
        .try_send(BoutControl::DebugGrant { side, weapon, funds })
        .is_err()
    {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "bout busy (control channel full); try again\n".to_string(),
        )
            .into_response();
    }
    let side_str = match side {
        Side::A => "A",
        Side::B => "B",
    };
    println!("admin grant queued: match {match_id} side {side_str} weapon={weapon:?} funds={funds:?}");
    Json(json!({
        "granted": true,
        "match_id": match_id,
        "side": side_str,
        "weapon": weapon.map(|t| t.index() as i32),
        "funds": funds,
    }))
    .into_response()
}

/// `POST /api/identity` with `{"name":"<str>"}`: mints an HS256 identity token
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

/// `GET /api/player/:name`: a player's stats. An unknown player returns a fresh,
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

/// Paths whose responses carry `Cache-Control: no-cache`: the page, the compiled
/// browser bundle, the wasm, and assets. API/WS/admin paths are left untouched.
fn is_static_path(path: &str) -> bool {
    path == "/"
        || path.starts_with("/www")
        || path.starts_with("/pkg")
        || path.starts_with("/assets")
}

/// Make browsers REVALIDATE static assets on every load instead of serving a
/// heuristically-cached copy, so a deploy actually reaches clients rather than a
/// stale `main.js` lingering (notably on mobile Safari, which caches aggressively).
/// `no-cache` means "you may store it, but revalidate before using": ServeDir sends
/// `Last-Modified`, so the conditional request gets a cheap 304 when nothing changed
/// and a fresh 200 when it did. This gives correctness without re-downloading the
/// wasm every time. Scoped to [`is_static_path`]; dynamic API/WS responses are
/// unaffected.
async fn static_no_cache(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let is_static = is_static_path(req.uri().path());
    let mut res = next.run(req).await;
    if is_static {
        res.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    res
}

/// Boot the server: open the replay DB (importing legacy `Replay` JSON from
/// `REPLAYS_DIR` if present), build the shared state, wire the axum router (the
/// `/ws` matchmaking socket, the replay / leaderboard / player / identity /
/// admin / metrics endpoints, then static files as a fallback), and serve
/// forever. The two middleware layers add a revalidate-on-load cache policy to
/// static assets and a request counter; the idle-decay task keeps the live
/// "players online" count honest as clients go quiet. Binds the IPv6 any-address
/// so the same listener serves the public site, fly's IPv4 proxy, and the
/// IPv6-only 6PN network the region bots reach us on.
#[tokio::main]
async fn main() {
    // Error tracking, inert until SENTRY_DSN is set (a fly secret). The guard is
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
    // Where replay blobs live for this boot (see `ReplayStore`); logs its own
    // one-line mode announcement. A half-configured bucket panics here, before
    // anything else touches the DB, rather than degrading silently.
    let store = replay_store_config(|k| std::env::var(k).ok());
    // Prune the existing bot-vs-bot filler backlog down to the same 5% policy
    // new matches are sampled with (see `db_prune_bot_replays`). A failure here
    // is not fatal to boot (the volume filling up again slowly is recoverable;
    // refusing to serve is not), but it must never be swallowed silently.
    match db_prune_bot_replays(&conn) {
        Ok(deleted) => println!("replay prune: dropped {deleted} bot-vs-bot filler replay(s)"),
        Err(e) => eprintln!("replay prune failed: {e}"),
    }
    // Retry the json NOT NULL relaxation (see `relax_replays_json_not_null`). On a
    // full-disk boot the copy inside `open_db`'s `init_schema` deferred instead of
    // crashing; the prune just above frees the bot backlog, so it can now succeed.
    // A no-op when `open_db` already relaxed it (the healthy-disk path). Still
    // non-fatal: the migration below and Bucket-mode writes need the relaxed
    // column, but a persistently full disk keeps serving reads and inline writes.
    match relax_replays_json_not_null(&conn) {
        Ok(()) => {}
        Err(e) => eprintln!("relax replays.json NOT NULL failed: {e}"),
    }
    // Bucket mode only: move every not-yet-migrated row's JSON into the bucket.
    // A failure here is logged loudly but never fatal to boot: legacy rows keep
    // serving straight from SQLite either way, and the `json IS NOT NULL`
    // predicate means the next boot resumes exactly where this one left off.
    if let ReplayStore::Bucket(ref os) = store {
        match migrate_replay_blobs(&conn, os).await {
            Ok(n) => println!("replay blob migration: {n} row(s) moved to the bucket"),
            Err(e) => eprintln!("replay blob migration failed (legacy rows still serve from sqlite; next boot resumes): {e}"),
        }
    }
    // One VACUUM at the end, after the prune above and (in Bucket mode) the
    // migration above have both freed whatever space they're going to free,
    // instead of paying the rewrite cost once per deletion source.
    match db_vacuum_if_worthwhile(&conn) {
        Ok(vacuumed) => println!("replay vacuum: ran={vacuumed}"),
        Err(e) => eprintln!("replay vacuum failed: {e}"),
    }
    let imported = import_dir(&conn, &replays_dir());
    if imported > 0 {
        println!("imported {imported} replay(s) from {} into {}", replays_dir(), db_path());
    }
    // One DB handle, shared by the matchmaking/stats websocket (App) and the HTTP
    // replay/leaderboard handlers (AppState.db).
    let db: Db = Arc::new(std::sync::Mutex::new(conn));
    let state = AppState {
        app: Arc::new(Mutex::new(App::new(db.clone(), store.clone()))),
        db,
        store,
    };

    // Materialize the identity-token secret once now (so the per-process-random
    // fallback is fixed for the run, and a missing BT_JWT_SECRET is logged).
    let _ = identity::secret();
    if std::env::var("BT_JWT_SECRET").is_err() {
        println!("BT_JWT_SECRET unset; using a per-process-random token secret");
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
        .route("/admin/drain", post(admin_drain))
        .route("/admin/resume", post(admin_resume))
        .route("/admin/grant", post(admin_grant))
        .route("/api/identity", post(post_identity))
        .route("/api/player/:name", get(player_profile))
        .route("/replay/:id", get(replay_page))
        .route("/", get(|| async { Redirect::permanent("/www/") }))
        .fallback_service(ServeDir::new(&static_dir))
        .layer(axum::middleware::from_fn(static_no_cache))
        .layer(axum::middleware::from_fn(track_http))
        .with_state(state);

    // Bind the IPv6 any-address. On Linux this is dual-stack (IPV6_V6ONLY=0 by
    // default), so it serves both the public site (the fly proxy reaches us over
    // IPv4) and fly's private 6PN network (IPv6-only). That 6PN path is how the
    // region bots reach us at ws://battletris.internal:8080. A 0.0.0.0
    // (IPv4-only) bind would leave the 6PN port closed and the bots unable to connect.
    let addr = format!("[::]:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    println!("BattleTris server on http://{addr}  (static: {static_dir}, ws: /ws)");
    axum::serve(listener, app).await.unwrap();
}

/// Tests for the matchmaking/settlement "between-the-game" layer: rating updates,
/// pairing eligibility, the drain gate, presence restoration, challenge routing,
/// and the replay/counters DB helpers, exercised against an in-memory SQLite and
/// hand-built [`App`] state, with no live sockets.
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
            store: ReplayStore::Sqlite,
            challenges: HashMap::new(),
            last_players: Vec::new(),
            bouts: HashMap::new(),
            draining: false,
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

    // Blue/green drain: a draining machine starts NO new bouts (the single chokepoint
    // is `start_bout`, so both the auto-matcher and a challenge-accept are refused).
    // Without this, a deploy cutover would still spin up matches on the machine that's
    // about to be reaped.
    #[test]
    fn a_draining_machine_starts_no_new_bouts() {
        let mut app = test_app();
        let _rx_a = add_client(&mut app, "1", "alice");
        let _rx_b = add_client(&mut app, "2", "bob");
        app.draining = true;

        assert!(try_match(&mut app, "1").is_none(), "draining: alice can't even queue a match");
        assert!(try_match(&mut app, "2").is_none(), "draining: bob must NOT auto-pair into a bout");
        assert!(start_bout(&mut app, "1", "2", None).is_none(), "draining: a direct start_bout is refused");
        assert!(
            app.clients["1"].bout.is_none() && app.clients["2"].bout.is_none(),
            "draining: no client gets bound to a bout"
        );
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

    // --- resync rate limiter (a pure function, no tick loop needed) ----------

    #[test]
    fn resync_grant_allowed_grants_the_first_call() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(resync_grant_allowed(&mut last, t0), "a side that never got a grant is allowed one");
        assert_eq!(last, Some(t0), "the grant time is recorded");
    }

    #[test]
    fn resync_grant_allowed_denies_a_second_call_within_a_second_and_keeps_the_window() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(resync_grant_allowed(&mut last, t0), "first call granted");

        let t1 = t0 + Duration::from_millis(500);
        assert!(!resync_grant_allowed(&mut last, t1), "a second call inside the 1s window is denied");
        // Denial must NOT reset the window to t1: a further call still inside 1s of
        // the ORIGINAL grant (t0) must also be denied, not measured from t1.
        let t2 = t0 + Duration::from_millis(900);
        assert!(!resync_grant_allowed(&mut last, t2), "denial must not have reset the window");
        assert_eq!(last, Some(t0), "last_grant is untouched by a denied call");
    }

    #[test]
    fn resync_grant_allowed_grants_again_a_second_later() {
        let mut last = None;
        let t0 = Instant::now();
        assert!(resync_grant_allowed(&mut last, t0), "first call granted");

        let t1 = t0 + Duration::from_secs(1);
        assert!(resync_grant_allowed(&mut last, t1), "exactly 1s later is allowed again");
        assert_eq!(last, Some(t1), "the new grant time replaces the old one");
    }

    // --- admin grant (the gated live-bout weapon/funds injection dev tool) ----

    use axum::body::to_bytes;
    use axum::response::Response;

    /// `BT_ADMIN_TOKEN` is PROCESS-GLOBAL, so the tests that set/unset it must not run
    /// concurrently (one's `remove_var` would race another's read). This serializes
    /// them: each such test holds this lock for its whole body. A poisoned lock (an
    /// earlier panic) is fine to reuse, since we only need mutual exclusion, not the data.
    fn admin_env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    /// Build a `HeaderMap` carrying (or omitting) an `x-admin-token`.
    fn admin_headers(token: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = token {
            h.insert("x-admin-token", t.parse().unwrap());
        }
        h
    }

    /// Extract (status, parsed-JSON-or-Null) from an axum response.
    async fn response_json(resp: Response) -> (StatusCode, Value) {
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
        (status, v)
    }

    /// Register a fake live bout in `app.bouts` and hand back its control RECEIVER
    /// (so a test can observe the `DebugGrant` the handler routes to it) plus the
    /// input receiver (kept alive so the input channel isn't closed). No tick loop:
    /// these tests assert the HTTP handler's auth/validation/routing, not application.
    fn register_fake_bout(app: &mut App, match_id: &str) -> (mpsc::Receiver<BoutControl>, mpsc::Receiver<BoutInput>) {
        let (control_tx, control_rx) = mpsc::channel::<BoutControl>(4);
        let (input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
        app.bouts.insert(
            match_id.to_string(),
            BoutHandle { control: control_tx, input_tx, name_a: "alice".into(), name_b: "bob".into() },
        );
        (control_rx, input_rx)
    }

    // (1) An UNAUTHED request (no x-admin-token while BT_ADMIN_TOKEN is set) is 403,
    // and (2) with BT_ADMIN_TOKEN UNSET admin is fail-closed: even a token-bearing
    // request is 403. Both share one test because BT_ADMIN_TOKEN is process-global;
    // running them as separate parallel tests would race the env var.
    //
    // These three tests mutate the process-global `BT_ADMIN_TOKEN`, so they (a) hold
    // `admin_env_lock()` for mutual exclusion and (b) are SYNCHRONOUS `#[test]`s that
    // drive the async handler via `block_on`, never holding the std guard across an
    // `.await` in async code (which clippy's `await_holding_lock` rightly forbids).
    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn admin_grant_is_fail_closed_and_requires_the_token() {
        let _env = admin_env_lock();
        let rt = rt();
        let shared: Shared = Arc::new(Mutex::new(test_app()));
        let body = json!({"match_id":"match-x","side":"A","weapon":7}).to_string();
        let status = |hdr: HeaderMap, body: String| {
            rt.block_on(admin_grant(State(shared.clone()), hdr, body)).into_response().status()
        };

        // BT_ADMIN_TOKEN set, but the request presents NO token -> 403.
        std::env::set_var("BT_ADMIN_TOKEN", "s3cret");
        assert_eq!(status(admin_headers(None), body.clone()), StatusCode::FORBIDDEN,
            "missing x-admin-token must be forbidden");
        // A WRONG token is also 403.
        assert_eq!(status(admin_headers(Some("nope")), body.clone()), StatusCode::FORBIDDEN,
            "wrong x-admin-token must be forbidden");

        // BT_ADMIN_TOKEN UNSET -> fail-closed: even a token-bearing request is refused.
        std::env::remove_var("BT_ADMIN_TOKEN");
        assert_eq!(status(admin_headers(Some("anything")), body), StatusCode::FORBIDDEN,
            "admin disabled (BT_ADMIN_TOKEN unset) must fail closed");
    }

    // (4) Authed-but-malformed requests are 4xx: bad side, out-of-range weapon, neither
    // field present, and an unknown match_id (404). Auth must NOT mask these, as they're
    // only reachable once admin_authed passes.
    #[test]
    fn admin_grant_validates_body_and_match() {
        let _env = admin_env_lock();
        let rt = rt();
        std::env::set_var("BT_ADMIN_TOKEN", "s3cret");
        let mut app = test_app();
        let (_ctrl_rx, _in_rx) = register_fake_bout(&mut app, "match-live");
        let shared: Shared = Arc::new(Mutex::new(app));
        let call = |b: Value| {
            rt.block_on(admin_grant(State(shared.clone()), admin_headers(Some("s3cret")), b.to_string()))
                .into_response()
                .status()
        };

        assert_eq!(call(json!({"match_id":"match-live","side":"C","weapon":7})),
            StatusCode::BAD_REQUEST, "a bad side is rejected");
        assert_eq!(call(json!({"match_id":"match-live","side":"A","weapon":34})),
            StatusCode::BAD_REQUEST, "weapon 34 (out of 0..=33) is rejected");
        assert_eq!(call(json!({"match_id":"match-live","side":"A","weapon":-1})),
            StatusCode::BAD_REQUEST, "a negative weapon index is rejected");
        assert_eq!(call(json!({"match_id":"match-live","side":"A"})),
            StatusCode::BAD_REQUEST, "neither weapon nor funds present is rejected");
        assert_eq!(call(json!({"side":"A","weapon":7})),
            StatusCode::BAD_REQUEST, "a missing match_id is rejected");
        assert_eq!(call(json!({"match_id":"","side":"A","weapon":7})),
            StatusCode::BAD_REQUEST, "an empty match_id is rejected");
        // Authed + well-formed, but no such live bout -> 404 (not 400/403).
        assert_eq!(call(json!({"match_id":"match-ghost","side":"A","weapon":7})),
            StatusCode::NOT_FOUND, "an unknown match_id is 404");

        std::env::remove_var("BT_ADMIN_TOKEN");
    }

    // (3a) A valid authed grant ROUTES the right DebugGrant to the bout's task: the
    // handler resolves the live bout by match_id and sends a control message carrying
    // the parsed side/weapon/funds. (3b below proves the bout task then applies it.)
    #[test]
    fn admin_grant_routes_a_debug_grant_to_the_bout() {
        let _env = admin_env_lock();
        let rt = rt();
        std::env::set_var("BT_ADMIN_TOKEN", "s3cret");
        let mut app = test_app();
        let (mut ctrl_rx, _in_rx) = register_fake_bout(&mut app, "match-live");
        let shared: Shared = Arc::new(Mutex::new(app));

        let body = json!({"match_id":"match-live","side":"B","weapon":7,"funds":500}).to_string();
        let resp = rt.block_on(admin_grant(State(shared.clone()), admin_headers(Some("s3cret")), body)).into_response();
        let (status, j) = rt.block_on(response_json(resp));
        assert_eq!(status, StatusCode::OK, "a valid authed grant is 200");
        assert_eq!(j["granted"], json!(true));
        assert_eq!(j["side"], json!("B"));
        assert_eq!(j["weapon"], json!(7));
        assert_eq!(j["funds"], json!(500));

        // The bout task would receive exactly this control message and apply it.
        match ctrl_rx.try_recv() {
            Ok(BoutControl::DebugGrant { side, weapon, funds }) => {
                assert_eq!(side, Side::B);
                assert_eq!(weapon, Some(WeaponToken::from_index(7).unwrap()));
                assert_eq!(funds, Some(500));
            }
            _ => panic!("expected a DebugGrant on the bout control channel"),
        }
        std::env::remove_var("BT_ADMIN_TOKEN");
    }

    // Bout::debug_grant applies a weapon and/or funds to the named side's
    // authoritative game AND records each as a replay frame, so a debug-granted bout
    // replays faithfully (the grant lands in the arsenal at the same tick, ahead of
    // any later LaunchWeapon that spends it). This is the unit-level teeth for the
    // "grants are captured" claim.
    #[test]
    fn debug_grant_mutates_the_named_side_and_records_a_frame() {
        let mut bout = bout::Bout::new(1, 2);
        let tok = WeaponToken::from_index(7).unwrap(); // Rise Up
        assert_eq!(bout.arsenal_count(Side::A, tok), 0, "arsenal starts empty for this weapon");
        let funds_before = bout.funds(Side::A);

        // Grant mid-bout (after one tick), as a live admin grant would land, so the
        // recording has ticks around the grant frame for playback to apply it within.
        bout.tick(bout::TICK_MS);
        let (w, f) = bout.debug_grant(Side::A, Some(tok), Some(250));
        bout.tick(bout::TICK_MS);
        assert!(w && f, "both the weapon and the funds were applied");
        assert_eq!(bout.arsenal_count(Side::A, tok), 1, "side A's arsenal gained the weapon");
        assert_eq!(bout.funds(Side::A), funds_before + 250, "side A's funds increased");
        // The OTHER side is untouched.
        assert_eq!(bout.arsenal_count(Side::B, tok), 0, "side B's arsenal is unchanged");

        // The grant IS recorded: one GrantWeapon frame then one AddFunds frame, both
        // stamped to side A, so replaying reproduces the grant rather than firing an
        // empty slot later.
        let replay = bout.to_replay(bout::TICK_MS, "test");
        let a_frames: Vec<&bt_replay::Input> =
            replay.frames.iter().filter(|fr| fr.side == 0).map(|fr| &fr.input).collect();
        assert_eq!(
            a_frames,
            vec![&bt_replay::Input::GrantWeapon(7), &bt_replay::Input::AddFunds(250)],
            "the admin grant must be recorded as a GrantWeapon then an AddFunds frame for side A"
        );

        // End to end: re-simulate the recording and confirm side A's arsenal holds the
        // granted weapon, proving the recorded frame actually reproduces the grant.
        let mut pl = bt_replay::VersusReplayPlayer::new(replay);
        pl.run_to_end();
        assert_eq!(
            pl.game(true).arsenal_token(0),
            7,
            "replaying the recorded grant arms side A with Rise Up in slot 0"
        );
    }

    // (3b) END-TO-END through the real bout task: register a live bout, spawn run_bout,
    // route a grant via the SAME control channel the HTTP handler uses, and confirm the
    // change surfaces in the AUTHORITATIVE game (read back through the spectator stream,
    // which reports each side's arsenal + funds). This exercises channel + task + apply.
    #[tokio::test]
    async fn admin_grant_changes_the_live_authoritative_bout() {
        let (tx_a, _rx_a) = mpsc::unbounded_channel::<Message>();
        let (tx_b, _rx_b) = mpsc::unbounded_channel::<Message>();
        let (_input_tx, input_rx) = mpsc::channel::<BoutInput>(BOUT_INPUT_CAP);
        let (control_tx, control_rx) = mpsc::channel::<BoutControl>(4);
        let app0 = test_app();
        let (sa, sb) = (app0.rating_for("alice"), app0.rating_for("bob"));
        let pb = PendingBout {
            match_id: "match-live".into(), id_a: "1".into(), id_b: "2".into(), seed_a: 11, seed_b: 22,
            name_a: "alice".into(), name_b: "bob".into(), state_a: sa, state_b: sb,
            tx_a, tx_b, input_rx, control_rx, human: [true, true],
        };
        let shared: Shared = Arc::new(Mutex::new(test_app()));
        let handle = tokio::spawn(run_bout(shared, pb));

        // A read-only spectator so we can read the authoritative arsenal/funds back.
        let (spec_tx, mut spec_rx) = mpsc::unbounded_channel::<Message>();
        control_tx.send(BoutControl::AddSpectator { tx: spec_tx }).await.unwrap();

        // Route the grant exactly as admin_grant does: Rise Up (token 7) + 500 funds to A.
        let tok = WeaponToken::from_index(7).unwrap();
        control_tx.send(BoutControl::DebugGrant { side: Side::A, weapon: Some(tok), funds: Some(500) }).await.unwrap();

        // Let the bout task drain the control channel and emit a fresh spectator frame.
        tokio::time::sleep(Duration::from_millis(120)).await;

        // Read the LATEST spectator frame and confirm A's authoritative game changed.
        let mut last_a: Option<Value> = None;
        while let Ok(Message::Text(t)) = spec_rx.try_recv() {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                if v["type"] == "spectate" {
                    last_a = Some(v["a"].clone());
                }
            }
        }
        let a = last_a.expect("got at least one spectator frame for side A");
        assert!(a["funds"].as_i64().unwrap() >= 500, "side A's authoritative funds reflect the grant");
        // The arsenal is a flat [token, qty, token, qty, ...]; token 7 must appear with qty >= 1.
        let arsenal = a["arsenal"].as_array().expect("arsenal array");
        let has_weapon_7 = arsenal.chunks(2).any(|c| c[0].as_i64() == Some(7) && c[1].as_i64().unwrap_or(0) >= 1);
        assert!(has_weapon_7, "side A's authoritative arsenal gained weapon 7 (Rise Up): {arsenal:?}");

        drop(control_tx);
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

    // --- bot-vs-bot replay retention policy ---------------------------------

    #[test]
    fn is_bot_name_matches_the_house_bot_and_regional_personas() {
        assert!(is_bot_name("The Count"));
        assert!(is_bot_name("Tokyo-Bert"));
        assert!(is_bot_name("SaoPaulo-Ernie"));
        assert!(!is_bot_name("colin"));
        assert!(!is_bot_name("Bert"), "no dash-suffix match without the dash");
        assert!(!is_bot_name("The Countess"));
    }

    #[test]
    fn replay_retained_is_deterministic_and_lands_near_5_percent() {
        assert_eq!(replay_retained("m42"), replay_retained("m42"), "same key -> same answer, called twice");

        let ids: Vec<String> = (0..200).map(|n| format!("m{n}")).collect();
        let kept = ids.iter().filter(|id| replay_retained(id.as_str())).count();
        assert!((2..=25).contains(&kept), "expected roughly 5% of 200 retained, got {kept}");
        assert!(kept > 0, "at least one retained id in the fixture range");
        assert!(kept < ids.len(), "at least one dropped id in the fixture range");
    }

    #[test]
    fn bot_filler_replay_requires_both_bot_names_and_no_title() {
        assert!(bot_filler_replay("The Count", "Tokyo-Bert", None), "bot vs bot, untitled -> filler");
        assert!(!bot_filler_replay("The Count", "Tokyo-Bert", Some("Showcase")), "titled -> never filler");
        assert!(!bot_filler_replay("colin", "Tokyo-Bert", None), "human vs bot -> never filler");
        assert!(!bot_filler_replay("Tokyo-Bert", "colin", None), "bot vs human (order swapped) -> never filler");
    }

    /// A distinct-content [`VersusReplay`] from a bout seeded with `seed_a`/`seed_b`,
    /// so each call produces a different content-hash id, mirroring the real settle
    /// path's `bout.to_replay(...)`.
    fn sample_versus_replay(seed_a: u64, seed_b: u64) -> VersusReplay {
        let mut b = bout::Bout::new(seed_a, seed_b);
        b.tick(bout::TICK_MS);
        b.to_replay(bout::TICK_MS, "test")
    }

    #[test]
    fn db_prune_bot_replays_keeps_titled_and_human_rows_and_samples_the_rest() {
        let conn = mem_db();

        // A spread of untitled bot-vs-bot rows with distinct content (and so distinct
        // ids), enough that both a retained and a dropped id are all but certain.
        let mut bot_ids = Vec::new();
        for seed in 0..40u64 {
            let r = sample_versus_replay(seed, seed + 1000);
            let json = r.to_json();
            let id = replay_id(&json);
            db_insert_versus(&conn, &id, &r, Some(&json), 1000, "The Count", "Tokyo-Bert", 0).unwrap();
            bot_ids.push(id);
        }

        // One titled bot-vs-bot row: a curated showcase, never filler.
        let mut titled = sample_versus_replay(9001, 9002);
        titled.title = Some("Showcase".to_string());
        let titled_json = titled.to_json();
        let titled_id = replay_id(&titled_json);
        db_insert_versus(&conn, &titled_id, &titled, Some(&titled_json), 1000, "The Count", "Tokyo-Bert", 0).unwrap();

        // One human-vs-bot row: precious regardless of title.
        let human = sample_versus_replay(9003, 9004);
        let human_json = human.to_json();
        let human_id = replay_id(&human_json);
        db_insert_versus(&conn, &human_id, &human, Some(&human_json), 1000, "colin", "Tokyo-Bert", 0).unwrap();

        let deleted = db_prune_bot_replays(&conn).unwrap();

        let expect_kept: HashSet<&String> = bot_ids.iter().filter(|id| replay_retained(id.as_str())).collect();
        assert_eq!(deleted, bot_ids.len() - expect_kept.len(), "deleted count matches the sampled-out bot rows");
        for id in &bot_ids {
            let present = db_get(&conn, id).unwrap().is_some();
            assert_eq!(present, expect_kept.contains(id), "row {id} survival matches replay_retained(id)");
        }
        assert!(db_get(&conn, &titled_id).unwrap().is_some(), "titled bot-vs-bot row always survives");
        assert!(db_get(&conn, &human_id).unwrap().is_some(), "human-vs-bot row always survives");

        // Idempotence is the property that matters most: pruning an already-pruned
        // DB deletes nothing, because every surviving row's id still hashes "keep".
        let deleted_again = db_prune_bot_replays(&conn).unwrap();
        assert_eq!(deleted_again, 0, "a second prune is a no-op");
    }

    #[test]
    fn db_vacuum_if_worthwhile_is_a_noop_below_the_freelist_threshold() {
        let conn = mem_db();
        assert!(!db_vacuum_if_worthwhile(&conn).unwrap(), "a fresh tiny db has nothing worth vacuuming");
    }

    #[test]
    fn persist_time_sampling_matches_replay_retained_by_match_id() {
        // Scan the fixture range for a Count-vs-Bert id known dropped and one known
        // retained, then confirm the composed persist-time predicate agrees.
        let ids: Vec<String> = (0..200).map(|n| format!("m{n}")).collect();
        let retained_id = ids.iter().find(|id| replay_retained(id.as_str())).expect("at least one retained id");
        let dropped_id = ids.iter().find(|id| !replay_retained(id.as_str())).expect("at least one dropped id");

        let sampled_out = |id: &str| bot_filler_replay("The Count", "Tokyo-Bert", None) && !replay_retained(id);
        assert!(!sampled_out(retained_id), "a retained id is not sampled out");
        assert!(sampled_out(dropped_id), "a dropped id is sampled out");
    }

    // --- replay blob storage (Bucket mode) ------------------------------------

    use futures_util::stream::BoxStream;
    use object_store::memory::InMemory;
    use object_store::{
        CopyOptions, GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, PutMultipartOptions, PutOptions,
        PutPayload, PutResult,
    };

    /// Wraps a real store but always fails `put`, standing in for a bucket
    /// outage (credentials revoked, network partition) so the "a failed upload
    /// skips the row" behavior is tested without a real network dependency.
    /// Every other method delegates to the wrapped store, so anything that DOES
    /// reach one still behaves like a normal store rather than panicking.
    #[derive(Debug)]
    struct PutAlwaysFails(InMemory);

    impl std::fmt::Display for PutAlwaysFails {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "PutAlwaysFails({})", self.0)
        }
    }

    #[async_trait::async_trait]
    impl ObjectStore for PutAlwaysFails {
        async fn put_opts(
            &self,
            location: &ObjectPath,
            _payload: PutPayload,
            _opts: PutOptions,
        ) -> object_store::Result<PutResult> {
            Err(object_store::Error::Generic {
                store: "PutAlwaysFails",
                source: format!("put refused for {location} (simulated bucket outage)").into(),
            })
        }
        async fn put_multipart_opts(
            &self,
            location: &ObjectPath,
            opts: PutMultipartOptions,
        ) -> object_store::Result<Box<dyn MultipartUpload>> {
            self.0.put_multipart_opts(location, opts).await
        }
        async fn get_opts(&self, location: &ObjectPath, options: GetOptions) -> object_store::Result<GetResult> {
            self.0.get_opts(location, options).await
        }
        fn delete_stream(
            &self,
            locations: BoxStream<'static, object_store::Result<ObjectPath>>,
        ) -> BoxStream<'static, object_store::Result<ObjectPath>> {
            self.0.delete_stream(locations)
        }
        fn list(&self, prefix: Option<&ObjectPath>) -> BoxStream<'static, object_store::Result<ObjectMeta>> {
            self.0.list(prefix)
        }
        async fn list_with_delimiter(&self, prefix: Option<&ObjectPath>) -> object_store::Result<ListResult> {
            self.0.list_with_delimiter(prefix).await
        }
        async fn copy_opts(&self, from: &ObjectPath, to: &ObjectPath, options: CopyOptions) -> object_store::Result<()> {
            self.0.copy_opts(from, to, options).await
        }
    }

    #[test]
    fn replay_store_config_defaults_to_sqlite_when_bucket_name_unset() {
        assert!(matches!(replay_store_config(|_| None), ReplayStore::Sqlite));
    }

    /// A closure-backed lookup over a fixed map, standing in for
    /// `|k| std::env::var(k).ok()` without ever touching process env.
    fn env_lookup<'a>(vars: &'a HashMap<&str, &str>) -> impl Fn(&str) -> Option<String> + 'a {
        |k: &str| vars.get(k).map(|v| v.to_string())
    }

    #[test]
    fn replay_store_config_builds_bucket_mode_when_fully_configured() {
        let vars = HashMap::from([
            ("BUCKET_NAME", "replays-bucket"),
            ("AWS_ENDPOINT_URL_S3", "https://fly.storage.tigris.dev"),
            ("AWS_ACCESS_KEY_ID", "key"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
            ("AWS_REGION", "auto"),
        ]);
        assert!(matches!(replay_store_config(env_lookup(&vars)), ReplayStore::Bucket(_)));
    }

    #[test]
    fn replay_store_config_defaults_region_to_auto_when_unset() {
        let vars = HashMap::from([
            ("BUCKET_NAME", "replays-bucket"),
            ("AWS_ENDPOINT_URL_S3", "https://fly.storage.tigris.dev"),
            ("AWS_ACCESS_KEY_ID", "key"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
        ]);
        assert!(
            matches!(replay_store_config(env_lookup(&vars)), ReplayStore::Bucket(_)),
            "a missing AWS_REGION defaults to \"auto\" rather than failing boot"
        );
    }

    #[test]
    #[should_panic(expected = "AWS_ACCESS_KEY_ID")]
    fn replay_store_config_panics_loudly_on_a_missing_companion_var() {
        let vars = HashMap::from([
            ("BUCKET_NAME", "replays-bucket"),
            ("AWS_ENDPOINT_URL_S3", "https://fly.storage.tigris.dev"),
            ("AWS_SECRET_ACCESS_KEY", "secret"),
            // AWS_ACCESS_KEY_ID deliberately missing: a half-configured bucket
            // must fail boot loudly, naming the missing var, not fall back to Sqlite.
        ]);
        replay_store_config(env_lookup(&vars));
    }

    /// A one-tick [`bout::Bout`] with a settled outcome to feed
    /// `prepare_replay_write`, mirroring what `run_bout`'s settle path has in
    /// scope once the tick loop ends. Human names (never bot names), so the 5%
    /// filler-sampling gate never sampled it out regardless of `match_id`.
    fn sample_bout(seed_a: u64, seed_b: u64) -> Bout {
        let mut b = Bout::new(seed_a, seed_b);
        b.tick(bout::TICK_MS);
        b
    }

    #[tokio::test]
    async fn bucket_mode_write_puts_the_blob_then_inserts_a_null_json_row() {
        let conn = mem_db();
        let os: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let store = ReplayStore::Bucket(os.clone());
        let bout = sample_bout(1, 2);

        let write = prepare_replay_write(&store, &bout, "match-bucket-ok", "colin", "ada").await;
        let ReplayWrite::Row { id, replay, json_col, lines } = write else { panic!("expected a row, not Skip") };
        assert!(json_col.is_none(), "bucket mode leaves json_col None once the blob is uploaded");

        db_insert_versus(&conn, &id, &replay, json_col.as_deref(), 1000, "colin", "ada", lines).unwrap();
        let stored_json: Option<String> =
            conn.query_row("SELECT json FROM replays WHERE id = ?1", [&id], |r| r.get(0)).unwrap();
        assert!(stored_json.is_none(), "the json column is NULL once the blob lives in the bucket");

        let bytes = os.get(&replay_blob_path(&id)).await.unwrap().bytes().await.unwrap();
        assert_eq!(bytes.as_ref(), replay.to_json().as_bytes(), "the bucket object holds exactly the replay JSON bytes");
    }

    #[tokio::test]
    async fn bucket_mode_write_skips_the_row_when_the_put_fails() {
        let conn = mem_db();
        let store = ReplayStore::Bucket(Arc::new(PutAlwaysFails(InMemory::new())));
        let bout = sample_bout(3, 4);

        let write = prepare_replay_write(&store, &bout, "match-bucket-fail", "colin", "ada").await;
        assert!(matches!(write, ReplayWrite::Skip), "a failed put must never leave a row behind");
        assert_eq!(db_list(&conn, 10).unwrap().len(), 0, "no row was ever inserted");
    }

    #[tokio::test]
    async fn resolve_replay_json_serves_legacy_inline_rows_without_touching_the_store() {
        // PutAlwaysFails only fails `put`; a `get` still works (delegates to the
        // wrapped InMemory). Reaching for the store at all on a non-NULL column
        // would therefore still pass this assertion by accident, so the real
        // proof is `store` never being asked: nothing here ever populates the
        // wrapped InMemory, so an accidental `get` would 404 instead.
        let store = ReplayStore::Bucket(Arc::new(PutAlwaysFails(InMemory::new())));
        let got = resolve_replay_json(&store, "legacy-id", Some(r#"{"seed":1}"#.to_string())).await;
        assert_eq!(got.unwrap(), r#"{"seed":1}"#);
    }

    #[tokio::test]
    async fn resolve_replay_json_fetches_a_nulled_row_from_the_bucket() {
        let mem = InMemory::new();
        let id = "migrated-id";
        mem.put(&replay_blob_path(id), r#"{"seed":2}"#.to_string().into()).await.unwrap();
        let store = ReplayStore::Bucket(Arc::new(mem));
        let got = resolve_replay_json(&store, id, None).await;
        assert_eq!(got.unwrap(), r#"{"seed":2}"#);
    }

    #[tokio::test]
    async fn resolve_replay_json_errors_when_the_bucket_object_is_missing() {
        let store = ReplayStore::Bucket(Arc::new(InMemory::new()));
        let got = resolve_replay_json(&store, "ghost-id", None).await;
        assert!(got.is_err(), "a NULLed row with no object behind it is an error, never an empty replay");
    }

    #[tokio::test]
    async fn resolve_replay_json_errors_on_a_nulled_row_with_no_bucket_configured() {
        let got = resolve_replay_json(&ReplayStore::Sqlite, "orphan-id", None).await;
        assert!(got.is_err(), "sqlite mode with a NULL json column is a hard error, not an empty replay");
    }

    #[tokio::test]
    async fn migrate_replay_blobs_moves_existing_rows_and_is_idempotent() {
        let conn = mem_db();
        let os: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let mut ids = Vec::new();
        for seed in 0..3u64 {
            let r = sample_versus_replay(seed, seed + 100);
            let json = r.to_json();
            let id = replay_id(&json);
            db_insert_versus(&conn, &id, &r, Some(&json), 1000 + seed as i64, "colin", "ada", 5).unwrap();
            ids.push(id);
        }

        let migrated = migrate_replay_blobs(&conn, &os).await.unwrap();
        assert_eq!(migrated, 3);

        // A row's json column, title, names, and lines, as `Option`s (mirroring the
        // schema's nullability) so the assertions below can check each independently.
        type RowMeta = (Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>);
        for id in &ids {
            let (json, title, name_a, name_b, lines): RowMeta = conn
                .query_row(
                    "SELECT json, title, name_a, name_b, lines FROM replays WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap();
            assert!(json.is_none(), "row {id}'s json column is NULLed after migration");
            assert!(title.is_none(), "migration doesn't touch unrelated metadata");
            assert_eq!(name_a.as_deref(), Some("colin"), "migration doesn't touch name_a");
            assert_eq!(name_b.as_deref(), Some("ada"), "migration doesn't touch name_b");
            assert_eq!(lines, Some(5), "migration doesn't touch lines");
            let bytes = os.get(&replay_blob_path(id)).await.unwrap().bytes().await.unwrap();
            assert!(!bytes.is_empty(), "the blob is present in the bucket");
        }

        let migrated_again = migrate_replay_blobs(&conn, &os).await.unwrap();
        assert_eq!(migrated_again, 0, "a second pass finds nothing left with json still set (resumable/idempotent)");
    }

    #[test]
    fn active_count_respects_30s_window() {
        let db: Db = Arc::new(std::sync::Mutex::new(mem_db()));
        let mut app = App::new(db, ReplayStore::Sqlite);
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
        // Bots are passive: they never auto-pair (with each other or a human).
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

    #[tokio::test]
    async fn post_replay_rejects_an_absurd_tick_count() {
        // An unauthenticated upload claiming u32::MAX ticks would otherwise spin
        // run_to_end ~4.3 billion times on a runtime worker. It must be rejected.
        let db: Db = Arc::new(std::sync::Mutex::new(mem_db()));
        let mut r = sample_replay(1, bt_replay::Mode::Practice, None);
        r.tick_count = u32::MAX;
        let resp = post_replay(State(db), r.to_json()).await.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn post_replay_accepts_a_normal_recording() {
        let db: Db = Arc::new(std::sync::Mutex::new(mem_db()));
        let r = sample_replay(1, bt_replay::Mode::Practice, None); // tick_count 100
        let resp = post_replay(State(db), r.to_json()).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn ct_eq_is_true_only_for_identical_slices() {
        assert!(ct_eq(b"s3cret-admin-token", b"s3cret-admin-token"));
        assert!(ct_eq(b"", b""));
        assert!(!ct_eq(b"s3cret-admin-token", b"s3cret-admin-tokeX"));
        assert!(!ct_eq(b"short", b"short-but-longer"));
        assert!(!ct_eq(b"longer-than-other", b"short"));
    }

    #[test]
    fn no_cache_scoped_to_static_assets_only() {
        // The page, bundle, wasm, and assets revalidate; dynamic endpoints don't.
        assert!(is_static_path("/"));
        assert!(is_static_path("/www/"));
        assert!(is_static_path("/www/main.js"));
        assert!(is_static_path("/pkg/bt_wasm_bg.wasm"));
        assert!(is_static_path("/assets/btbiff1.png"));
        assert!(!is_static_path("/ws"));
        assert!(!is_static_path("/api/identity"));
        assert!(!is_static_path("/api/leaderboard"));
        assert!(!is_static_path("/admin/grant"));
    }

    #[test]
    fn stale_token_plus_rated_name_collapses_to_empty() {
        // The mobile-Safari "anon" trap. A returning, already-rated player whose
        // cached identity token no longer verifies (e.g. it was minted under an
        // earlier server secret) falls back to the bare `name`. resolve_name refuses
        // a bare, already-rated name (the anti-hijack guard) unless it equals the
        // connection's `prior` name (empty on a fresh socket), so the player
        // collapses to "". `queue` then lists them as "anon"; `available` drops them
        // from the roster entirely. The fix is client-side: always mint a FRESH token
        // (it verifies under the current secret), so the token path resolves the name
        // and this bare-name guard is never reached.
        let mut app = test_app();
        app.ratings.insert("player972".to_string(), (25.0, 8.0, 10)); // already rated

        // A token signed under a DIFFERENT secret won't verify -> ignored -> falls to
        // the bare name, which is rejected as an already-rated name -> "".
        let stale = identity::issue_token_with(b"an-older-server-secret", "player972", 1_700_000_000);
        assert_eq!(
            resolve_name(&app, &json!({ "name": "player972", "token": stale }), ""),
            "",
            "stale token + rated bare name -> empty (the bug: shows as anon / unlisted)"
        );

        // A FRESH token (the current process secret) verifies, so the name resolves
        // directly and the bare-name guard is bypassed. This is what the client now
        // always sends: one fresh mint per session instead of a cached, stale token.
        let fresh = identity::issue_token("player972");
        assert_eq!(
            resolve_name(&app, &json!({ "name": "player972", "token": fresh }), ""),
            "player972",
            "a fresh, verifiable token resolves the rated name"
        );
    }

    #[test]
    fn challenge_initiator_settles_against_stored_rating_not_stale_default() {
        // A directed challenge sets c.name but not c.state (the queue/available
        // paths refresh c.state; the challenge path does not). If start_bout read
        // the stale c.state, a rated player who only ever challenges would settle
        // against the default rating and overwrite their real one. start_bout must
        // look the rating up fresh by name.
        let mut app = test_app();
        app.ratings.insert("alice".to_string(), (33.0, 2.0, 40)); // alice is strongly rated
        let _rb = add_client(&mut app, "B", "bob");
        // alice's connection: name established via challenge (set directly), state
        // left at the connection default (the bug's precondition).
        let _ra = add_client(&mut app, "A", "");
        if let Some(c) = app.clients.get_mut("A") {
            c.name = "alice".to_string();
        }
        assert!(
            (app.clients["A"].state.rating.mu - 33.0).abs() > 1.0,
            "precondition: alice's cached c.state is the stale default, not her real rating"
        );

        let pb = start_bout(&mut app, "A", "B", None).expect("bout starts");
        assert_eq!(pb.name_a, "alice");
        assert_eq!(pb.state_a.rating.mu, 33.0, "settles against alice's stored rating, not the default");
        assert_eq!(pb.state_a.experience, 40);
    }

    #[test]
    fn start_bout_remembers_pre_match_presence_for_restore() {
        let mut app = test_app();
        // A bot (Available) challenged by a human (no presence; a pure challenger).
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

        // alice goes through the matcher with NO one queued; bob is Available, so
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

        // A reattach (from the rejoin handler) reaches the bout's control receiver;
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
        // server-side binding (never anything client-supplied), so a client can
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

    // The `resync` handler, like `leaveMatch`, must resolve (match_id, side) from the
    // client's OWN server-side binding and route a `BoutControl::Resync` to that
    // bout's control channel. Drive it through the real dispatcher (`handle_message`)
    // rather than mirroring the resolution, since the point is to pin the actual wire
    // frame shape (`{"type":"resync"}`) end to end.
    #[tokio::test]
    async fn resync_frame_routes_to_the_clients_own_bout() {
        let mut app = test_app();
        let _rx_a = add_client(&mut app, "1", "alice");
        let _rx_b = add_client(&mut app, "2", "bob");
        set_present(&mut app, "1", Status::Available);
        set_present(&mut app, "2", Status::Available);
        let pending = start_bout(&mut app, "1", "2", None).expect("bout built");
        let mid = pending.match_id.clone();
        let mut ctrl_rx = pending.control_rx;

        let shared: Shared = Arc::new(Mutex::new(app));
        handle_message(&shared, "1", r#"{"type":"resync"}"#).await;

        match ctrl_rx.try_recv() {
            Ok(BoutControl::Resync { side }) => assert_eq!(side, Side::A, "alice (side A) sent it"),
            _ => panic!("expected a Resync control message on alice's bout"),
        }

        let app = shared.lock().await;
        assert!(app.bouts.contains_key(&mid), "the bout is still registered");
    }

    // A client not currently in a bout (no `match_id`/`bout` binding) sends `resync`:
    // per the documented drop semantics, this must be silently ignored, not panic.
    #[tokio::test]
    async fn resync_frame_from_a_client_not_in_a_bout_is_silently_dropped() {
        let mut app = test_app();
        let _rx = add_client(&mut app, "1", "solo");
        let shared: Shared = Arc::new(Mutex::new(app));

        handle_message(&shared, "1", r#"{"type":"resync"}"#).await; // must not panic

        let app = shared.lock().await;
        assert!(app.bouts.is_empty(), "no bout existed to route anything into");
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

        // A second migration is a no-op (table no longer empty) and must not
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
