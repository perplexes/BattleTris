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
//! and easy-going **Ernie** (faithful placement, slower, no weapons). A third,
//! **The Count** (`BT_BOT_PERSONA=count`), roams the lobby issuing directed
//! challenges — preferring humans, exponential-backing-off anyone who declines, and
//! dueling the regional bots when it gets bored — and dials its skill to each
//! opponent's Elo (carried on `matchStart`) to aim for an even match.
//!
//! The bot keeps a LOCAL `bt-core::Game` seeded from `matchStart.seed` (the same
//! deterministic piece stream the server runs for this side) and reconciles it
//! to the authoritative `keyframe` bytes whenever one arrives — exactly the
//! prediction/reconciliation model the browser client uses. Hard-drop column
//! placement is robust to latency: the column is decided by the move/rotate
//! inputs we send (applied in order on both sides), not by where gravity has the
//! piece when the drop lands.

use std::time::Duration;

mod sync;
use sync::{decide, BotAction, SyncState};

use bt_ai::weapons::{buy_plan, launch_choice};
use bt_ai::{best_placement, best_placement_skill, best_placement_strong, Placement};
use bt_core::weapons::WeaponToken;
use bt_core::Game;
// The SAME prediction/reconciliation core the browser runs (bt-wasm's WasmClient):
// the local predicted sim, the unacked-input queue, per-bout seq, and keyframe replay
// all live in `Predictor`, so the bot and the browser can't drift apart.
use bt_netcode::{input_frame, Predictor};
use bt_replay::Input;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;
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
/// If a match goes this many ticks (~14s) with no snapshot, treat it as over. This
/// MUST exceed the server's `REJOIN_GRACE` (12s): when a human opponent drops, the
/// server FREEZES the bout for that long waiting for them to reconnect (an accidental
/// refresh drops straight back into the same game), and we get no snapshots while it's
/// frozen. A shorter timeout would abandon the match mid-grace — and a snapshot gap
/// under heavy RTT could trip it too — so we wait out the full grace (plus a margin for
/// the resume frame + latency). A natural top-out still sends a final `snapshot` with a
/// `result`, which ends the match cleanly; a dropped opponent ends the bout server-side
/// without that final snapshot. The server's end-of-bout frames for that case
/// (`opponentLeft`, `rating`) are all non-`snapshot`, which the bot ignores — so this
/// timeout is how we eventually notice and return to the lobby.
const STALE_TICKS: u32 = 875;
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

// ─── The Count: roaming-challenger tuning ───────────────────────────────────
/// Issue a challenge attempt at most this often while idle (eager but polite).
const ROAM_CADENCE: Duration = Duration::from_secs(10);
/// First backoff after a decline; doubles each further decline, capped.
const ROAM_BASE_BACKOFF: Duration = Duration::from_secs(60);
const ROAM_MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);
/// Don't re-pick a target while a challenge to them is outstanding (the server's
/// own answer/timeout window is ~30s).
const ROAM_PENDING_WAIT: Duration = Duration::from_secs(35);
/// Cooldown on someone we just played, so we don't immediately re-challenge them.
const ROAM_POST_MATCH: Duration = Duration::from_secs(90);
/// Roam difficulty: placement pace at skill 0 (slow) vs skill 1 (brisk).
const ROAM_PLACE_SLOW: i32 = 50; // ~800ms
const ROAM_PLACE_FAST: i32 = 22; // ~350ms
/// Skill at/above which the roamer bothers with weapons.
const ROAM_WEAPON_SKILL: f64 = 0.4;

/// Map an opponent's Elo (server `elo_styled`: ~700 weak, 1000 new, 1700+ strong)
/// to a skill in `[0,1]` for [`best_placement_skill`] + the pace/weapon dials.
fn elo_to_skill(elo: i64) -> f64 {
    ((elo as f64 - 700.0) / 1000.0).clamp(0.0, 1.0)
}
fn roam_place_ticks(skill: f64) -> i32 {
    (ROAM_PLACE_SLOW as f64 - skill * (ROAM_PLACE_SLOW - ROAM_PLACE_FAST) as f64).round() as i32
}

/// A potential challenge target, tracked across roster updates.
struct Target {
    is_bot: bool,
    available: bool,
    /// Earliest time we may (re)challenge — pushed out by exponential backoff.
    next_eligible: Instant,
    declines: u32,
}

/// The Count's roaming state: who's around, who's in backoff, and the outstanding
/// challenge. Prefers humans; when none are eligible it gets "bored" and duels the
/// regional bots.
struct Roam {
    me: String,
    targets: HashMap<String, Target>,
    /// (target, sent-at) of the challenge we're awaiting an answer to.
    pending: Option<(String, Instant)>,
    next_attempt: Instant,
}

impl Roam {
    fn new(me: String) -> Roam {
        Roam { me, targets: HashMap::new(), pending: None, next_attempt: Instant::now() }
    }

    /// Refresh availability + bot flags from a `players` roster frame.
    fn update_roster(&mut self, players: &[Value]) {
        for t in self.targets.values_mut() {
            t.available = false;
        }
        let now = Instant::now();
        for p in players {
            let name = match p.get("name").and_then(|n| n.as_str()) {
                Some(n) if n != self.me => n.to_string(),
                _ => continue,
            };
            let available = p.get("status").and_then(|s| s.as_str()) == Some("available");
            let is_bot = p.get("bot").and_then(|b| b.as_bool()).unwrap_or(false);
            let t = self.targets.entry(name).or_insert(Target {
                is_bot,
                available: false,
                next_eligible: now,
                declines: 0,
            });
            t.is_bot = is_bot;
            t.available = available;
        }
    }

    /// A challenge was declined (or timed out) by `who`: exponential backoff.
    fn on_declined(&mut self, who: &str) {
        self.pending = None;
        let now = Instant::now();
        if let Some(t) = self.targets.get_mut(who) {
            t.declines += 1;
            let shift = (t.declines - 1).min(20);
            let backoff = ROAM_BASE_BACKOFF.saturating_mul(1u32 << shift).min(ROAM_MAX_BACKOFF);
            t.next_eligible = now + backoff;
        }
        self.next_attempt = now + Duration::from_secs(2);
    }

    /// We matched `opp` (they accepted, or challenged us): clear pending + a polite
    /// post-match cooldown on them, and reset their decline streak.
    fn on_matched(&mut self, opp: &str) {
        self.pending = None;
        if let Some(t) = self.targets.get_mut(opp) {
            t.declines = 0;
            t.next_eligible = Instant::now() + ROAM_POST_MATCH;
        }
    }

    fn on_match_end(&mut self) {
        self.next_attempt = Instant::now() + Duration::from_secs(3);
    }

    /// Pick someone to challenge: an eligible human first, else (bored) a bot.
    fn pick(&self, now: Instant) -> Option<String> {
        let mut bot: Option<&String> = None;
        let mut human: Option<&String> = None;
        for (name, t) in &self.targets {
            if !t.available || t.next_eligible > now {
                continue;
            }
            if t.is_bot {
                bot = bot.or(Some(name));
            } else {
                human = human.or(Some(name));
            }
        }
        human.or(bot).cloned()
    }

    /// Step while idle; returns Some(target) to send a `challenge` to.
    fn step(&mut self) -> Option<String> {
        let now = Instant::now();
        if let Some((_, sent)) = &self.pending {
            if now.duration_since(*sent) > ROAM_PENDING_WAIT {
                self.pending = None; // stale; the server already declined on timeout
            } else {
                return None;
            }
        }
        if now < self.next_attempt {
            return None;
        }
        self.next_attempt = now + ROAM_CADENCE;
        let target = self.pick(now)?;
        if let Some(t) = self.targets.get_mut(&target) {
            t.next_eligible = now + ROAM_PENDING_WAIT; // hold while outstanding
        }
        self.pending = Some((target.clone(), now));
        Some(target)
    }
}

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
    /// The Count: a roaming challenger that matches the opponent's rating per match
    /// (placement/pace/weapons dialed by `skill`, set from `matchStart.opp_elo`).
    roam: bool,
}

fn persona() -> Persona {
    let which = std::env::var("BT_BOT_PERSONA")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("FLY_PROCESS_GROUP").ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match which.as_str() {
        "count" | "roam" => Persona {
            suffix: "The Count", // identity() uses this whole string, un-prefixed
            strong: true,
            place_ticks: PLACE_INTERVAL_TICKS, // overridden per match by skill
            weapons: true,
            roam: true,
        },
        "bert" | "strong" | "hard" => Persona {
            suffix: "Bert",
            strong: true,
            place_ticks: PLACE_INTERVAL_TICKS,
            weapons: true,
            roam: false,
        },
        _ => Persona {
            suffix: "Ernie",
            strong: false,
            place_ticks: 44, // ~700ms — slower, more beatable
            weapons: false,
            roam: false,
        },
    }
}

/// Per-match local state: our predicted client plus the placement pacer and the
/// bazaar gate.
struct MatchState {
    /// Our local prediction + reconciliation against the server's keyframes — the
    /// shared [`bt_netcode::Predictor`] (owns the local sim, the unacked-input queue,
    /// and the per-bout input seq). `sync::decide` reads `predictor.input_seq()`
    /// (= "last sent") and `predictor.game().is_in_bazaar()` to gate when we act.
    predictor: Predictor,
    /// This bot's difficulty/name persona.
    persona: Persona,
    /// Skill for this match in `[0,1]` — 1.0 (Bert), 0.0 (Ernie), or rating-matched
    /// from the opponent's Elo (The Count). Drives placement noise / pace / weapons.
    skill: f64,
    /// Ticks between placements this match (from `skill` for roam, else the persona).
    place_interval: i32,
    /// Whether weapons are used this match (skill-gated for roam, else the persona).
    weapons_on: bool,
    /// xorshift seed for skill-noise (kept off the engine RNG — see best_placement_skill).
    rng: u64,
    /// Weapons launched this match (for the end-of-match summary log).
    launched: u32,
    /// Ticks lived this match, for a periodic progress log.
    live_ticks: u32,
    /// Ticks until the next placement (paces the bot to a human-ish speed).
    cooldown: i32,
    /// Authoritative "you are in the bazaar" from the latest snapshot — while
    /// set, play is frozen server-side, so we just leave the bazaar and wait.
    in_bazaar: bool,
    /// "Your opponent is in the bazaar" from the latest snapshot. The bazaar is a
    /// BARRIER: while either side shops the match is frozen, so we wait this out too
    /// (don't keep placing — the server rejects it anyway, see Bout::apply_input).
    opp_in_bazaar: bool,
    /// Did we already buy our loadout for this bazaar visit? (We keep re-sending
    /// LeaveBazaar each tick until the server confirms, but only buy once.)
    bazaar_bought: bool,
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
    /// The last input seq the SERVER has acknowledged processing for us — the snapshot
    /// `ack`. (It means "I've seen your inputs through seq N", not necessarily "applied":
    /// the server acks a fresh legal input even when a bazaar barrier then drops it.)
    /// While this trails `predictor.input_seq()` (the seq of our most recently sent input
    /// this bout) we have inputs still in flight, and the bot waits — so its local
    /// prediction can't run ahead of the authoritative sim. This is the general "never
    /// act before the server has caught up" gate; the bazaar's predicted-leave freeze has
    /// its own dedicated guard on top (`sync::decide`'s `auth_baz && local_baz`). The
    /// "last sent" half of this gate is the [`Predictor`]'s per-bout `input_seq` (reset
    /// to 0 with each new bout).
    acked: u64,
}

impl MatchState {
    fn new(seed: u64, persona: Persona, opp_elo: i64) -> MatchState {
        let skill = if persona.roam {
            elo_to_skill(opp_elo)
        } else if persona.strong {
            1.0
        } else {
            0.0
        };
        let place_interval = if persona.roam { roam_place_ticks(skill) } else { persona.place_ticks };
        let weapons_on = if persona.roam { skill >= ROAM_WEAPON_SKILL } else { persona.weapons };
        MatchState {
            predictor: Predictor::new(seed),
            persona,
            skill,
            place_interval,
            weapons_on,
            rng: (seed ^ 0x9E37_79B9_7F4A_7C15) | 1, // non-zero xorshift seed
            launched: 0,
            live_ticks: 0,
            cooldown: place_interval,
            in_bazaar: false,
            opp_in_bazaar: false,
            bazaar_bought: false,
            launch_cooldown: LAUNCH_INTERVAL_TICKS,
            spy_fresh: 0,
            opp_high: false,
            done: false,
            idle_ticks: 0,
            acked: 0,
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
    let name = std::env::var("BT_BOT_NAME").unwrap_or_else(|_| {
        if p.roam {
            p.suffix.to_string() // "The Count" — one roamer, not region-tagged
        } else {
            format!("{city}-{}", p.suffix)
        }
    });
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
    // The Count's roaming state (None for the fixed personas).
    let mut roam: Option<Roam> = if persona.roam { Some(Roam::new(name.to_string())) } else { None };

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
                    Message::Text(t) => handle_text(&t, &out, &mut ms, persona, &mut roam),
                    Message::Ping(p) => { let _ = out.send(Message::Pong(p)); }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            _ = ticker.tick() => {
                if let Some(state) = ms.as_mut() {
                    drive_tick(state, &out);
                    if state.done {
                        println!(
                            "match over — {name}: {} lines cleared, {} weapons launched",
                            state.predictor.game().score().lines, state.launched
                        );
                        ms = None;
                        if let Some(r) = roam.as_mut() {
                            r.on_match_end();
                        }
                        // Back to Available for the next challenger. (The server
                        // already resets us to Available at bout end; this also
                        // refreshes the bot tag.)
                        let _ = out.send(available_msg(name, geo, &token));
                    }
                } else if let Some(r) = roam.as_mut() {
                    // Idle + roaming: maybe fire off a directed challenge.
                    if let Some(target) = r.step() {
                        println!("[The Count] challenging {target}");
                        let _ = out.send(Message::Text(
                            json!({ "type": "challenge", "target": target }).to_string(),
                        ));
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
fn handle_text(
    text: &str,
    out: &Out,
    ms: &mut Option<MatchState>,
    persona: Persona,
    roam: &mut Option<Roam>,
) {
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
        // The match is starting: seed a fresh local sim for our side. The Count dials
        // its difficulty to the opponent's Elo (carried on matchStart).
        Some("matchStart") => {
            let seed = v.get("seed").and_then(|s| s.as_u64()).unwrap_or(0);
            let opp = v.get("opponent").and_then(|o| o.as_str()).unwrap_or("?").to_string();
            let opp_elo = v.get("opp_elo").and_then(|e| e.as_i64()).unwrap_or(1000);
            let st = MatchState::new(seed, persona, opp_elo);
            if persona.roam {
                println!(
                    "matchStart vs {opp} (elo {opp_elo} -> skill {:.2}, seed {seed})",
                    st.skill
                );
            } else {
                println!("matchStart vs {opp} (seed {seed})");
            }
            if let Some(r) = roam.as_mut() {
                r.on_matched(&opp);
            }
            *ms = Some(st);
        }
        // Lobby roster: The Count tracks who's around (+ bot flags) to challenge.
        Some("players") => {
            if let (Some(r), Some(arr)) = (roam.as_mut(), v.get("players").and_then(|p| p.as_array())) {
                r.update_roster(arr);
            }
        }
        // Our challenge was declined (or timed out) — exponential backoff on them.
        Some("challengeDeclined") => {
            if let Some(r) = roam.as_mut() {
                let by = v.get("by").and_then(|b| b.as_str()).unwrap_or("");
                r.on_declined(by);
            }
        }
        // Authoritative frame: track result + bazaar, and reconcile on keyframes.
        Some("snapshot") => {
            if let Some(state) = ms.as_mut() {
                state.idle_ticks = 0; // the bout is alive
                // The server's confirmation of our inputs: `ack` is the last seq it
                // applied for us. `sync::decide` gates every action on this catching up
                // to `predictor.input_seq()`, so we never run ahead of the server.
                let ack = v.get("ack").and_then(|a| a.as_u64()).unwrap_or(state.acked);
                state.acked = ack;
                if v.get("result").and_then(|r| r.as_i64()).unwrap_or(0) != 0 {
                    state.done = true;
                }
                // Authoritative bazaar flags. The bazaar is a BARRIER: while either side
                // shops the match is frozen, so we wait it out (decide → WaitBazaar)
                // rather than placing (the server rejects placements anyway).
                let you_bazaar = v
                    .get("you")
                    .and_then(|y| y.get("in_bazaar"))
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                state.in_bazaar = you_bazaar;
                if !you_bazaar {
                    state.bazaar_bought = false; // armed again for the next visit
                }
                let opp_bazaar = v
                    .get("opp")
                    .and_then(|o| o.get("in_bazaar"))
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false);
                state.opp_in_bazaar = opp_bazaar;
                // The full authoritative board rides the throttled keyframe (a JSON
                // array of bytes). Hand the whole frame to the predictor: it prunes
                // acked inputs, restores our local sim to the keyframe, and replays the
                // still-unacked tail on top. Routing reconciliation through this single
                // path (shared with the browser) — rather than a direct game-state
                // restore here — is what keeps the bot and browser sims from drifting.
                let keyframe: Option<Vec<u8>> = v
                    .get("keyframe")
                    .and_then(|k| k.as_array())
                    .map(|arr| arr.iter().filter_map(|n| n.as_u64().map(|x| x as u8)).collect());
                state.predictor.on_snapshot(ack, you_bazaar, opp_bazaar, keyframe.as_deref());
                // Spy intel: a `spy_board` (the opponent's board, empty cells = -2)
                // rides keyframes only while one of our spies is active. Refresh the
                // "is the opponent stacked high?" read used to time board-raisers.
                if let Some(arr) = v.get("spy_board").and_then(|s| s.as_array()) {
                    let grid: Vec<i32> =
                        arr.iter().filter_map(|n| n.as_i64().map(|x| x as i32)).collect();
                    let w = state.predictor.game().board().width;
                    let h = state.predictor.game().board().height;
                    state.opp_high = opponent_is_high(&grid, w, h);
                    state.spy_fresh = SPY_FRESH_TICKS;
                }
            }
        }
        _ => {}
    }
}

/// One 16ms step of an in-progress match. Per-tick bookkeeping (idle/progress/spy
/// aging) runs first; then the sync state machine ([`sync::decide`]) picks exactly
/// one thing to do, which we interpret here. Keeping the policy in `decide` (pure +
/// proptested) and only the side effects here is what keeps the netcode invariants
/// — never act ahead of the server, never leave a bazaar we only predicted entering
/// — from drifting back into a tangle of ad-hoc booleans.
fn drive_tick(state: &mut MatchState, out: &Out) {
    if state.done {
        return;
    }
    // ── Per-tick bookkeeping, independent of phase ──
    // No authoritative frame for a while ⇒ the opponent vanished.
    state.idle_ticks += 1;
    let idle_timed_out = state.idle_ticks > STALE_TICKS;
    // Periodic progress log (~every 12s) so a long match still shows lines cleared
    // and weapons fired without waiting for a top-out.
    state.live_ticks += 1;
    if state.live_ticks.is_multiple_of(750) {
        println!(
            "[{}] progress: {} lines, {} weapons launched",
            state.persona.suffix,
            state.predictor.game().score().lines,
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

    // ── Decide, then interpret. `decide` owns the "never run ahead of the server"
    // policy (see its invariants); the side effects live here. ──
    let action = decide(&SyncState {
        done: state.done,
        idle_timed_out,
        acked: state.acked,
        last_sent: state.predictor.input_seq(),
        auth_baz: state.in_bazaar,
        opp_baz: state.opp_in_bazaar,
        local_baz: state.predictor.game().is_in_bazaar(),
        bought: state.bazaar_bought,
    });
    match action {
        BotAction::End => {
            if idle_timed_out {
                println!("match ended (opponent went silent)");
                state.done = true; // the driver loop ends the match next tick
            }
        }
        // Inputs in flight — hold until the server catches up (we never run ahead).
        BotAction::WaitAck => {}
        // A bazaar barrier that isn't ours to initiate-clear — hold. BUT if the server
        // authoritatively still has US in the bazaar and we've already shopped, keep
        // (idempotently) re-sending LeaveBazaar until it takes. This makes escaping a
        // bazaar we're in independent of the `bazaar_bought` re-arm ever observing an
        // out-of-bazaar snapshot (which a too-fast re-entry could otherwise skip).
        // Gated on `in_bazaar` (the AUTHORITATIVE flag), never on a mere local
        // prediction, so we can't send a leave the server would no-op away before we're
        // really in the bazaar (the predicted-leave freeze). We're here only when not
        // WaitAck, so the re-leave can't race an unacked input.
        BotAction::WaitBazaar => {
            if state.in_bazaar && state.bazaar_bought {
                send_input(&mut state.predictor, out, Input::LeaveBazaar);
            }
        }
        // Authoritatively + locally in our bazaar: buy a loadout and leave, once. The
        // server stays in the bazaar until our LeaveBazaar, and we only got here after
        // it acked our prior inputs, so the leave can't race the entry.
        BotAction::Shop => {
            shop_bazaar(state, out);
            state.bazaar_bought = true;
            // The buys + leave bumped `predictor.input_seq()` past `acked`, so the next
            // tick's decide → WaitAck holds us until the server confirms them.
        }
        BotAction::Play => play(state, out),
    }
}

/// Advance the local sim one tick and, subject to the launch/place cooldowns, fire a
/// weapon and/or place a piece. Reached only when [`sync::decide`] returns `Play`, so
/// there are no inputs in flight and no bazaar barrier is up.
fn play(state: &mut MatchState, out: &Out) {
    state.predictor.tick(TICK_MS);
    if state.predictor.game().is_game_over() {
        return; // wait for the authoritative result to end the match
    }

    // Launch a weapon on a cadence (when this match uses weapons) — a spy first (for
    // intel), board-raisers when the opponent is high, harassment/fund-drains else.
    if state.weapons_on {
        state.launch_cooldown -= 1;
        if state.launch_cooldown <= 0 {
            let arsenal = arsenal_of(state.predictor.game());
            let spy_active = state.spy_fresh > 0;
            if let Some(slot) = launch_choice(&arsenal, spy_active, state.opp_high) {
                send_input(&mut state.predictor, out, Input::LaunchWeapon(slot as u32));
                state.launched += 1;
                state.launch_cooldown = LAUNCH_INTERVAL_TICKS;
            } else {
                state.launch_cooldown = LAUNCH_RETRY_TICKS;
            }
        }
    }

    if state.cooldown > 0 {
        state.cooldown -= 1;
        // A weapon may have launched above; the predictor already bumped its seq, so
        // decide → WaitAck holds the next action until the server catches up.
        return;
    }
    // Pick the placement per this match's difficulty: roam uses the skill-noised eval,
    // Bert the strong eval, Ernie the faithful one.
    if let Some(p) = state.predictor.game().current_piece().cloned() {
        let pl = if state.persona.roam {
            best_placement_skill(state.predictor.game().board(), &p, state.skill, &mut state.rng)
        } else if state.persona.strong {
            best_placement_strong(state.predictor.game().board(), &p)
        } else {
            best_placement(state.predictor.game().board(), &p)
        };
        play_piece(&mut state.predictor, out, pl);
    }
    state.cooldown = state.place_interval;
}

/// The local arsenal as the ten protocol token indices (-1 = empty slot).
fn arsenal_of(game: &Game) -> [i32; 10] {
    let mut a = [-1i32; 10];
    for (i, slot) in a.iter_mut().enumerate() {
        *slot = game.arsenal_token(i);
    }
    a
}

/// In the bazaar, buy a smart loadout ([`buy_plan`]) within our funds, then leave.
/// Buys are predicted locally (applied to the sim) AND forwarded — only the ones the
/// engine accepts, keeping the sim in sync. The LeaveBazaar is forwarded but NOT
/// applied locally (the local sim stays in the bazaar until a keyframe clears the
/// barrier). The caller guarantees we're authoritatively AND locally in the bazaar
/// before calling, so the buys + the leave all land while the server is genuinely
/// shopping (no entry race). This shop batch runs once per visit (`bazaar_bought`
/// gates re-entry); a still-frozen LeaveBazaar may then be re-sent from `WaitBazaar`.
fn shop_bazaar(state: &mut MatchState, out: &Out) {
    if state.predictor.game().is_in_bazaar() && state.weapons_on {
        let funds = state.predictor.game().score().funds;
        let arsenal = arsenal_of(state.predictor.game());
        let carter = state.predictor.game().weapon_active(WeaponToken::Carter);
        for tok in buy_plan(funds, &arsenal, carter) {
            // `predict` applies the buy locally and returns a frame to send ONLY if the
            // engine accepted it, so we forward exactly the buys that landed — no stream
            // of rejected/no-op buys clogging the wire and the ack stream.
            send_input(&mut state.predictor, out, Input::BuyWeapon(tok.index() as i32));
        }
    }
    // Tell the SERVER we're done — this is what un-freezes the match. The Predictor
    // sends LeaveBazaar but does NOT leave the local sim: the bazaar is a barrier that
    // clears (via the next keyframe) only once BOTH sides are done, so our board
    // resumes in sync rather than ticking ahead of a still-frozen server.
    send_input(&mut state.predictor, out, Input::LeaveBazaar);
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
fn play_piece(predictor: &mut Predictor, out: &Out, pl: Placement) {
    let rot_cap = match predictor.game().current_piece() {
        Some(p) => p.orientations.max(1),
        None => return,
    };
    let (target_x, target_or) = (pl.x, pl.orientation);

    // Rotate to the target orientation (first, like take_turn). Read the orientation
    // as a Copy value so the immutable borrow ends before the mutable `send_input`.
    for _ in 0..rot_cap {
        match predictor.game().current_piece().map(|p| p.orientation) {
            Some(or) if or != target_or => send_input(predictor, out, Input::Rotate),
            _ => break,
        }
    }
    // Slide to the target column.
    let move_cap = predictor.game().board().width * 2;
    for _ in 0..move_cap {
        match predictor.game().current_piece().map(|p| p.x) {
            Some(px) if px < target_x => send_input(predictor, out, Input::MoveRight),
            Some(px) if px > target_x => send_input(predictor, out, Input::MoveLeft),
            _ => break,
        }
    }
    // Slam it home.
    send_input(predictor, out, Input::BeginDrop);
}

/// Predict an input through the shared [`Predictor`] (applies it to the local sim and
/// queues it for reconciliation) and forward the wire frame it hands back to the
/// server. A gated/rejected input (e.g. a non-shopping action under a bazaar barrier,
/// or an unaffordable buy) returns `None` and sends nothing.
fn send_input(predictor: &mut Predictor, out: &Out, input: Input) {
    if let Some((seq, inp)) = predictor.predict(input) {
        let _ = out.send(Message::Text(input_frame(seq, &inp)));
    }
}
