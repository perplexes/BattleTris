//! BattleTris online backend: WebSocket **matchmaking** (paired by TrueSkill
//! match quality) + **WebRTC signaling relay** (so gameplay flows peer-to-peer
//! over a data channel, not through the server) + **rating** updates on match
//! results, persisted to a JSON file.
//!
//! Protocol (JSON text frames):
//!   client → server:
//!     {"type":"queue","name":"alice"}        join the matchmaking queue
//!     {"type":"signal","data":<any>}         relay a WebRTC offer/answer/ICE to the peer
//!     {"type":"result","won":true,"lines":30,"opLines":18}   report the match result
//!   server → client:
//!     {"type":"matched","role":"offer|answer","opponent":"bob",
//!      "yourMu":...,"yourSigma":...,"oppMu":...,"oppSigma":...,"quality":0.42}
//!     {"type":"signal","data":<any>}
//!     {"type":"rating","mu":...,"sigma":...,"conservative":...,"won":true}
//!     {"type":"opponentLeft"}

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bt_trueskill::ts2::{rate_match, MatchOutcome, PlayerState, Ts2Params, Winner};
use bt_trueskill::{quality_1v1, Rating};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;

const LISTEN_ADDR: &str = "127.0.0.1:9000";
const RATINGS_FILE: &str = "ratings.json";

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
    /// Ids waiting for a match.
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

fn send(app: &App, id: u64, msg: &Value) {
    if let Some(c) = app.clients.get(&id) {
        let _ = c.tx.send(Message::Text(msg.to_string()));
    }
}

fn load_ratings() -> HashMap<String, (f64, f64, u32)> {
    let mut out = HashMap::new();
    if let Ok(txt) = std::fs::read_to_string(RATINGS_FILE) {
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
        let _ = std::fs::write(RATINGS_FILE, txt);
    }
}

/// Try to match `id` against the best-quality waiting opponent; otherwise queue.
fn try_match(app: &mut App, id: u64) {
    let my_rating = app.clients.get(&id).map(|c| c.state.rating);
    let my_rating = match my_rating {
        Some(r) => r,
        None => return,
    };

    // Pick the waiting client with the highest TrueSkill match quality.
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
            // The waiting player offers; the new arrival answers.
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

async fn handle_message(app: &Arc<Mutex<App>>, id: u64, text: &str) {
    let v: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("queue") => {
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("anon").to_string();
            let mut app = app.lock().await;
            let state = app.rating_for(&name);
            if let Some(c) = app.clients.get_mut(&id) {
                c.name = name;
                c.state = state;
            }
            try_match(&mut app, id);
        }
        Some("signal") => {
            let app = app.lock().await;
            if let Some(peer) = app.clients.get(&id).and_then(|c| c.peer) {
                let data = v.get("data").cloned().unwrap_or(Value::Null);
                send(&app, peer, &json!({"type": "signal", "data": data}));
            }
        }
        Some("result") => {
            let won = v.get("won").and_then(|b| b.as_bool()).unwrap_or(false);
            let lines = v.get("lines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let op_lines = v.get("opLines").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let mut app = app.lock().await;
            settle_result(&mut app, id, won, lines, op_lines);
        }
        _ => {}
    }
}

async fn handle_conn(app: Arc<Mutex<App>>, stream: tokio::net::TcpStream, id: u64) {
    let ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(_) => return,
    };
    let (mut write, mut read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    {
        let mut a = app.lock().await;
        let state = a.rating_for("");
        a.clients.insert(id, Client { name: String::new(), tx, peer: None, state });
    }

    // Writer task: drain the per-client channel to the socket.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if write.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = read.next().await {
        match msg {
            Message::Text(t) => handle_message(&app, id, &t).await,
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup on disconnect: notify the peer, drop from queue/clients.
    {
        let mut a = app.lock().await;
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
    let app = Arc::new(Mutex::new(App::new()));
    let listener = TcpListener::bind(LISTEN_ADDR)
        .await
        .unwrap_or_else(|e| panic!("bind {LISTEN_ADDR}: {e}"));
    println!("BattleTris server listening on ws://{LISTEN_ADDR}");
    let ids = AtomicU64::new(1);

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let id = ids.fetch_add(1, Ordering::Relaxed);
                let app = app.clone();
                tokio::spawn(handle_conn(app, stream, id));
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
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
