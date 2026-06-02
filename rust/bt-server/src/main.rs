//! BattleTris server: one binary that serves the web client (static files) AND
//! the online backend on the SAME port —
//!   * `GET /ws`        WebSocket: matchmaking (paired by TrueSkill match
//!                      quality), WebRTC signaling relay (gameplay stays P2P),
//!                      and result -> rating updates persisted to a JSON file.
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
//!     {"type":"queue","name":"alice"}
//!     {"type":"signal","data":<any>}                 (relayed to your peer)
//!     {"type":"result","won":true,"lines":30,"opLines":18}
//!   server -> client:
//!     {"type":"matched","role":"offer|answer","opponent":"bob",...}
//!     {"type":"signal","data":<any>}
//!     {"type":"rating","mu":...,"sigma":...,"conservative":...,"won":true}
//!     {"type":"opponentLeft"}

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;
use bt_trueskill::ts2::{rate_match, MatchOutcome, PlayerState, Ts2Params, Winner};
use bt_trueskill::{quality_1v1, Rating};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;

type Shared = Arc<Mutex<App>>;

/// Per-connected-client state.
struct Client {
    name: String,
    tx: mpsc::UnboundedSender<Message>,
    peer: Option<u64>,
    state: PlayerState,
}

/// Shared server state.
struct App {
    clients: HashMap<u64, Client>,
    waiting: Vec<u64>,
    /// Persisted ratings by player name: (mu, sigma, experience).
    ratings: HashMap<String, (f64, f64, u32)>,
    /// Pairings already rated (keyed by min(id_a, id_b)).
    settled: HashSet<u64>,
    params: Ts2Params,
}

impl App {
    fn new() -> App {
        App {
            clients: HashMap::new(),
            waiting: Vec::new(),
            ratings: load_ratings(),
            settled: HashSet::new(),
            params: Ts2Params::default(),
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

/// Match `id` against the best-quality waiting opponent; otherwise queue.
fn try_match(app: &mut App, id: u64) {
    let my_rating = match app.clients.get(&id).map(|c| c.state.rating) {
        Some(r) => r,
        None => return,
    };

    let mut best: Option<(u64, f64)> = None;
    for &wid in &app.waiting {
        if wid == id {
            continue;
        }
        if let Some(other) = app.clients.get(&wid) {
            let q = quality_1v1(my_rating, other.state.rating, &app.params.base);
            if best.map(|(_, bq)| q > bq).unwrap_or(true) {
                best = Some((wid, q));
            }
        }
    }

    match best {
        Some((opp, quality)) => {
            app.waiting.retain(|&w| w != opp && w != id);
            if let (Some(a), Some(b)) = (app.clients.get(&opp), app.clients.get(&id)) {
                let (a_name, a_state) = (a.name.clone(), a.state);
                let (b_name, b_state) = (b.name.clone(), b.state);
                let matched = |role: &str, you: &PlayerState, opp_name: &str, opp_s: &PlayerState| {
                    json!({
                        "type": "matched", "role": role, "opponent": opp_name,
                        "yourMu": you.rating.mu, "yourSigma": you.rating.sigma,
                        "oppMu": opp_s.rating.mu, "oppSigma": opp_s.rating.sigma,
                        "quality": quality,
                    })
                };
                send(app, opp, &matched("offer", &a_state, &b_name, &b_state));
                send(app, id, &matched("answer", &b_state, &a_name, &a_state));
            }
            if let Some(c) = app.clients.get_mut(&opp) {
                c.peer = Some(id);
            }
            if let Some(c) = app.clients.get_mut(&id) {
                c.peer = Some(opp);
            }
            println!("matched {opp} <-> {id} (quality {quality:.3})");
        }
        None => {
            if !app.waiting.contains(&id) {
                app.waiting.push(id);
            }
        }
    }
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

async fn handle_message(state: &Shared, id: u64, text: &str) {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("queue") => {
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("anon").to_string();
            let mut app = state.lock().await;
            let st = app.rating_for(&name);
            if let Some(c) = app.clients.get_mut(&id) {
                c.name = name;
                c.state = st;
            }
            try_match(&mut app, id);
        }
        Some("signal") => {
            let app = state.lock().await;
            if let Some(peer) = app.clients.get(&id).and_then(|c| c.peer) {
                let data = v.get("data").cloned().unwrap_or(Value::Null);
                send(&app, peer, &json!({"type": "signal", "data": data}));
            }
        }
        Some("result") => {
            let won = v.get("won").and_then(|b| b.as_bool()).unwrap_or(false);
            let lines = v.get("lines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let op_lines = v.get("opLines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let mut app = state.lock().await;
            settle_result(&mut app, id, won, lines, op_lines);
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
        a.clients.insert(id, Client { name: String::new(), tx, peer: None, state: st });
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
        let peer = a.clients.get(&id).and_then(|c| c.peer);
        if let Some(p) = peer {
            send(&a, p, &json!({"type": "opponentLeft"}));
            if let Some(c) = a.clients.get_mut(&p) {
                c.peer = None;
            }
        }
        a.clients.remove(&id);
    }
    writer.abort();
    println!("client {id} disconnected");
}

#[tokio::main]
async fn main() {
    let state: Shared = Arc::new(Mutex::new(App::new()));

    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "bt-wasm".to_string());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);

    let app = Router::new()
        .route("/ws", get(ws_handler))
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
}
