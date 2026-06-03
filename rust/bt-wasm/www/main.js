import init, { WasmGame, WasmVsComputer, fixed_dt, max_weapons, weapon_name, weapon_description, weapon_price, weapon_duration } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';
import { Sound } from './sound.js';

// Game state
let game = null;
let mode = 'practice'; // 'practice', 'vscomputer', 'vsplayer', 'online'
let lastFrameTime = 0;
// Fixed-timestep accumulator. The engine is advanced in constant FIXED_DT steps
// (read from wasm) decoupled from requestAnimationFrame, so play and every
// recording are deterministic regardless of frame-rate jitter. Set once wasm is
// initialised (see initGame).
let FIXED_DT = 16;
let tickAccumulator = 0;
const MAX_FRAME_DT = 250; // clamp huge gaps (e.g. backgrounded tab) to avoid a spiral of death
const MAX_STEPS_PER_FRAME = 8; // cap catch-up work per frame
let paused = false;
let gameEnded = false;
let broadcastChannel = null;

// Online / WebRTC state
let ws = null;       // WebSocket to signaling server
let pc = null;       // RTCPeerConnection (legacy P2P; unused in authoritative mode)
let dc = null;       // RTCDataChannel (legacy P2P)
let onlinePaused = true;  // true until the data channel opens
let onlineOpponentName = '';
let searching = false;    // background matchmaking in progress (queued, not yet matched)

// Server-authoritative online play (the client-server migration). In an
// authoritative match the server runs the real simulation; the client predicts
// locally for a 0-latency feel, sends each input to the server (tagged with a
// monotonic seq), and reconciles against the server's keyframes by restoring the
// full game state and re-applying its not-yet-acknowledged inputs.
let authoritative = false; // true during a server-authoritative online match
let inputSeq = 0;          // monotonic client input counter
let unackedInputs = [];    // [{seq, repr}] sent to the server, not yet acked
let authSelf = null;       // latest authoritative own-status {funds,in_bazaar,lines_til_bazaar}
let authOpp = null;        // latest authoritative opponent view {score,lines,game_over}
let authSpying = false;    // is a spy of ours active (server-authorized)?
let authSpyBoard = null;   // latest server-DEGRADED opponent board (from a keyframe), or null
let playerName = null;    // remembered after the first prompt

// Canvas and context
const canvas = document.getElementById('gameCanvas');
const ctx = canvas.getContext('2d');
const aiGridCanvas = document.getElementById('aiGridCanvas');
const aiCtx = aiGridCanvas.getContext('2d');

// UI elements
const scoreValue = document.getElementById('scoreValue');
const linesValue = document.getElementById('linesValue');
const fundsValue = document.getElementById('fundsValue');
const linesToBazaarValue = document.getElementById('linesToBazaarValue');
const gameOverOverlay = document.getElementById('gameOverOverlay');
const gameOverText = document.getElementById('gameOverText');
const newGameBtn = document.getElementById('newGameBtn');
const bazaarOverlay = document.getElementById('bazaarOverlay');
const bazaarFunds = document.getElementById('bazaarFunds');
const bazaarDoneBtn = document.getElementById('bazaarDoneBtn');
const bazaarAddBtn = document.getElementById('bazaarAddBtn');
const bazaarRemoveBtn = document.getElementById('bazaarRemoveBtn');
const bazaarWeaponList = document.getElementById('bazaarWeaponList');
const bazaarArsenalList = document.getElementById('bazaarArsenalList');
const bazaarInfoPrice = document.getElementById('bazaarInfoPrice');
const bazaarInfoDuration = document.getElementById('bazaarInfoDuration');
const bazaarInfoDesc = document.getElementById('bazaarInfoDesc');
const arsenalList = document.getElementById('arsenalList');
const opponentScore = document.getElementById('opponentScore');
const opponentLines = document.getElementById('opponentLines');
// Legacy element reference kept for backward compat (element hidden in HTML)
const bazaarList = document.getElementById('bazaarList');
const modePracticeBtn = document.getElementById('modePractice');
const modeVsComputerBtn = document.getElementById('modeVsComputer');
const modeVsPlayerBtn = document.getElementById('modeVsPlayer');
const modeOnlineBtn = document.getElementById('modeOnline');
const aiBoard = document.getElementById('aiBoard');
const aiLabel = document.getElementById('aiLabel');
const onlineStatus = document.getElementById('onlineStatus');
const cancelSearchBtn = document.getElementById('cancelSearch');
const playersCountEl = document.getElementById('playersCount');
const hitCounterEl = document.getElementById('hitCounter');
const ernieLevelSelect = document.getElementById('ernieLevel');
// Default Ernie difficulty: "Willing" (index 5 -> 1000ms/move). The original
// defaults to the slider minimum (Comatose); 1000ms is a fairer modern default.
const DEFAULT_ERNIE_LEVEL = 5;

// Mobile UI elements
const mobileScore = document.getElementById('mobileScore');
const mobileLines = document.getElementById('mobileLines');
const mobileFunds = document.getElementById('mobileFunds');
const mobileLinesToBazaar = document.getElementById('mobileLinesToBazaar');
const mobileOpponent = document.getElementById('mobileOpponent');
const mobileArsenalList = document.getElementById('mobileArsenalList');

const ARSENAL_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

// Keys that count as a "gameplay button" for the players-online activity ping.
const GAMEPLAY_KEYS = new Set([
    'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', ' ', 'Spacebar', 'p', 'P',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
]);

// ─── Online helpers ───────────────────────────────────────────────────────────

function setOnlineStatus(msg) {
    onlineStatus.textContent = msg;
}

// ─── Live site stats (players online + 90s hit counter) ─────────────────────────
//
// A dedicated read-only websocket, opened on page load and kept open. Sending
// "watch" counts this page as a visitor (the persistent SQLite hit counter) and
// a live player; the server then pushes {players, hits} to everyone whenever the
// numbers change. Kept separate from the matchmaking socket so it stays open the
// whole visit and never interferes with a match.
let statsWs = null;

function connectStats() {
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    statsWs = new WebSocket(`${wsProto}://${location.host}/ws`);
    statsWs.onopen = () => statsWs.send(JSON.stringify({ type: 'watch' }));
    statsWs.onmessage = (ev) => {
        let m;
        try { m = JSON.parse(ev.data); } catch (_) { return; }
        if (m.type === 'stats') updateLiveStats(m);
    };
    // Keep the counters live across a server restart / dropped connection.
    statsWs.onclose = () => { statsWs = null; setTimeout(connectStats, 3000); };
    statsWs.onerror = () => {};
}

function updateLiveStats(m) {
    if (typeof m.players === 'number' && playersCountEl) {
        playersCountEl.textContent = m.players;
    }
    if (typeof m.hits === 'number') setHitCounter(m.hits);
}

// Render the visit total as fixed-width odometer digits (classic web-counter look).
function setHitCounter(n) {
    if (!hitCounterEl) return;
    const s = String(Math.max(0, n | 0)).padStart(6, '0');
    hitCounterEl.innerHTML = Array.from(s, (d) => `<span class="odo-digit">${d}</span>`).join('');
}

// "Players online" = anyone who pressed a gameplay button in the last 30s. Tell
// the server we're active when a gameplay control fires. Throttled: one ping per
// few seconds keeps us inside the server's window without flooding the socket.
let lastActiveSent = 0;
function markActive() {
    const now = performance.now();
    if (now - lastActiveSent < 5000) return;
    lastActiveSent = now;
    if (statsWs && statsWs.readyState === WebSocket.OPEN) {
        statsWs.send(JSON.stringify({ type: 'active' }));
    }
}

function dcSend(obj) {
    if (dc && dc.readyState === 'open') {
        dc.send(JSON.stringify(obj));
    }
}

function cleanupOnline() {
    if (dc) { try { dc.close(); } catch (_) {} dc = null; }
    if (pc) { try { pc.close(); } catch (_) {} pc = null; }
    if (ws) { try { ws.close(); } catch (_) {} ws = null; }
    onlinePaused = true;
    onlineOpponentName = '';
    authoritative = false;
    unackedInputs = [];
    authSelf = null;
    authOpp = null;
    authSpying = false;
    authSpyBoard = null;
}

// ─── Server-authoritative client (prediction + reconciliation) ────────────────

// Apply a gameplay action to the LOCAL game (prediction) and, in an authoritative
// match, send it to the server tagged with a seq (and remember it for replay on
// the next keyframe). Returns the buy/sell success for the bazaar UI.
function predict(kind, arg) {
    // The bazaar freezes play: only shopping actions are valid (the server
    // rejects the rest). Gate centrally so NO call site — keys, touch, arsenal
    // clicks — can predict/send a non-shopping input while in the bazaar.
    if (inBazaar() && kind !== 'BuyWeapon' && kind !== 'SellWeapon' && kind !== 'LeaveBazaar') {
        return;
    }
    let repr = null;
    switch (kind) {
        case 'MoveLeft':  game.move_left();  repr = 'MoveLeft';  break;
        case 'MoveRight': game.move_right(); repr = 'MoveRight'; break;
        case 'Rotate':    game.rotate();     repr = 'Rotate';    break;
        case 'BeginDrop': game.begin_drop(); repr = 'BeginDrop'; break;
        case 'SoftDrop':  game.soft_drop();  repr = 'SoftDrop';  break;
        case 'LaunchWeapon': game.launch_weapon(arg); repr = { LaunchWeapon: arg >>> 0 }; break;
        case 'LeaveBazaar':
            // In an authoritative match the bazaar is a server-side barrier: send
            // "done" but DON'T leave locally (the keyframe clears in_bazaar once
            // BOTH players are done; leaving early would tick while the server is
            // frozen). Local modes leave immediately.
            if (!authoritative) game.leave_bazaar();
            repr = 'LeaveBazaar';
            break;
        case 'BuyWeapon': {
            const ok = game.buy_weapon(arg);
            if (authoritative && !gameEnded) sendInput({ BuyWeapon: arg });
            return ok;
        }
        case 'SellWeapon': {
            const ok = game.sell_weapon(arg);
            if (authoritative && !gameEnded) sendInput({ SellWeapon: arg });
            return ok;
        }
        case 'SetPaused': game.set_paused(arg); return; // local only; the server rejects pause
    }
    if (authoritative && !gameEnded && repr !== null) sendInput(repr);
}

function sendInput(repr) {
    // Only queue when we can actually send: if the socket is gone the input can
    // never be acked, so queuing it would grow unackedInputs forever (the local
    // prediction has already applied it; the match is effectively over anyway).
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    inputSeq += 1;
    unackedInputs.push({ seq: inputSeq, repr });
    ws.send(JSON.stringify({ type: 'input', seq: inputSeq, input: repr }));
}

// The bazaar barrier is server-authoritative online (authSelf.in_bazaar is prompt
// every frame); local modes use the engine flag. Used to gate input + replay so
// the client never sends/replays a non-shopping action the server would reject.
function inBazaar() {
    return (authoritative && authSelf) ? authSelf.in_bazaar : game.is_in_bazaar();
}

// Re-apply a not-yet-acked input to the (just-restored) local game, WITHOUT
// re-sending it — the reconciliation replay on top of a keyframe.
function applyReprToGame(repr) {
    // After a keyframe restore the game reflects the authoritative bazaar state.
    // While in the bazaar the server only accepts Buy/Sell, so replay only those
    // (re-applying movement/drop/launch here would drift from the server).
    const shopping = repr && (repr.BuyWeapon !== undefined || repr.SellWeapon !== undefined);
    if (game.is_in_bazaar() && !shopping) return;
    if (repr === 'MoveLeft') game.move_left();
    else if (repr === 'MoveRight') game.move_right();
    else if (repr === 'Rotate') game.rotate();
    else if (repr === 'BeginDrop') game.begin_drop();
    else if (repr === 'SoftDrop') game.soft_drop();
    else if (repr === 'LeaveBazaar') { /* server-confirmed; not predicted locally */ }
    else if (repr && repr.LaunchWeapon !== undefined) game.launch_weapon(repr.LaunchWeapon);
    else if (repr && repr.BuyWeapon !== undefined) game.buy_weapon(repr.BuyWeapon);
    else if (repr && repr.SellWeapon !== undefined) game.sell_weapon(repr.SellWeapon);
}

// An authoritative snapshot from the server: update opponent/own status, reconcile
// against the keyframe (when present), and latch the result.
function applySnapshot(msg) {
    authSelf = msg.you;
    authOpp = msg.opp;
    // Server-authorized spy: `spying` every frame; the degraded opponent board
    // rides keyframes. Keep the last board while spying; drop it when it ends.
    authSpying = !!msg.spying;
    if (msg.spy_board) authSpyBoard = Int32Array.from(msg.spy_board);
    if (!authSpying) authSpyBoard = null;
    // Discard inputs the server has now applied.
    unackedInputs = unackedInputs.filter((i) => i.seq > msg.ack);
    // On a keyframe, snap to the authoritative state then replay the unacked inputs.
    if (msg.keyframe && game.restore_keyframe) {
        game.restore_keyframe(Uint8Array.from(msg.keyframe));
        for (const i of unackedInputs) applyReprToGame(i.repr);
    }
    if (!gameEnded && msg.result === 1) {
        gameEnded = true;
        gameOverText.textContent = 'YOU WIN!';
        gameOverOverlay.style.display = 'flex';
    } else if (!gameEnded && msg.result === 2) {
        gameEnded = true;
        gameOverText.textContent = 'GAME OVER - You Lost';
        gameOverOverlay.style.display = 'flex';
    }
}

// Drop into a server-authoritative match: build the local prediction game with
// the server-assigned seed and start predicting immediately (we share the seed,
// so we're in lockstep with the server until a cross-player event arrives).
function enterAuthoritativeGame(msg) {
    searching = false;
    modeOnlineBtn.classList.remove('searching');
    cancelSearchBtn.style.display = 'none';
    mode = 'online';
    authoritative = true;
    onlineOpponentName = msg.opponent || 'Opponent';
    updateModeButtons();

    game = new WasmGame((msg.seed >>> 0));
    resetMatchState();
    inputSeq = 0;
    unackedInputs = [];
    authSelf = null;
    authOpp = null;
    authSpying = false;
    authSpyBoard = null;

    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;
    canvas.style.width = (width * CELL_SIZE * 1.6) + 'px';
    canvas.style.height = (height * CELL_SIZE * 1.6) + 'px';
    aiBoard.style.display = 'none';
    aiLabel.style.display = 'none';

    paused = false;
    gameEnded = false;
    onlinePaused = false; // server is ticking; predict immediately from the shared seed
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    onlineStatus.style.display = 'block';
    setOnlineStatus(`Matched vs ${onlineOpponentName} - fight!`);
    lastFrameTime = performance.now();
    tickAccumulator = 0;
}

function showWin() {
    gameEnded = true;
    gameOverText.textContent = 'YOU WIN!';
    gameOverOverlay.style.display = 'flex';
}

function setupDataChannel(channel) {
    channel.onopen = () => {
        startOnlineGame();
    };

    channel.onmessage = (ev) => {
        const m = JSON.parse(ev.data);
        if (handleExchange(m)) {
            // consumed (swap/susan exchange)
        } else if (m.kind === 'weapon') {
            receiveWeaponFromOpponent(m.token);
        } else if (m.kind === 'funds') {
            game.add_funds(m.amount); // Mondale/Keating credit from the victim
        } else if (m.kind === 'score') {
            game.receive_op_score(m.score, m.lines, m.funds);
            spyOnOpponentScore(); // line-based spy expiry + refresh
        } else if (m.kind === 'bazaarDone') {
            opponentBazaarDone = true;
            maybeLeaveBazaar();
        } else if (m.kind === 'dead') {
            // Opponent died - we win
            showWin();
            if (ws) {
                ws.send(JSON.stringify({
                    type: 'result',
                    won: true,
                    lines: game.lines(),
                    opLines: game.op_lines()
                }));
            }
        }
    };

    channel.onclose = () => {
        if (!gameEnded) {
            setOnlineStatus('Data channel closed.');
        }
    };

    channel.onerror = (err) => {
        console.warn('DataChannel error', err);
    };
}

function startOnlineGame() {
    onlinePaused = false;
    const oppLabel = onlineOpponentName || 'Opponent';
    setOnlineStatus(`Connected - fight!  (vs ${oppLabel})`);
}

function getPlayerName() {
    if (playerName) return playerName;
    try { playerName = localStorage.getItem('bt_player_name') || null; } catch (_) {}
    if (!playerName) {
        // Asked once, then remembered (it's your leaderboard identity). Clear
        // localStorage 'bt_player_name' to be asked again.
        const dflt = 'player' + Math.floor(Math.random() * 900 + 100);
        playerName = prompt('Choose a player name (shown on the leaderboard):', dflt) || dflt;
        try { localStorage.setItem('bt_player_name', playerName); } catch (_) {}
    }
    return playerName;
}

// Background matchmaking: open the signaling socket and queue WITHOUT
// interrupting your current local game. You keep playing practice / vs Computer
// while the search runs; `enterOnlineGame` swaps you in when a match is found.
// Switching between local modes leaves the search running (see startGame); only
// an explicit Cancel or tearing down an established online game stops it.
function findMatch() {
    if (searching || mode === 'online') return; // already queued, or already playing online
    const name = getPlayerName();

    cleanupOnline(); // drop any stale socket/peer from a previous session
    searching = true;
    modeOnlineBtn.classList.add('searching');
    onlineStatus.style.display = 'block';
    cancelSearchBtn.style.display = 'inline-block';
    setOnlineStatus('Searching for an opponent... keep playing - you\'ll drop in when matched.');

    // Same-origin WebSocket: the bt-server serves both the page and /ws on one
    // port (ws on localhost/LAN, wss behind TLS, e.g. on fly.io).
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    const wsUrl = `${wsProto}://${location.host}/ws`;
    ws = new WebSocket(wsUrl);

    // Announce the server-authoritative protocol (the client-server migration).
    ws.onopen = () => ws.send(JSON.stringify({ type: 'queue', name, authoritative: true }));

    ws.onerror = (err) => {
        console.warn('WebSocket error', err);
        setOnlineStatus(`Connection error - is the server reachable at ${wsUrl}?`);
    };

    ws.onclose = () => {
        if (!gameEnded) setOnlineStatus('Disconnected from server.');
        // A dropped socket during an authoritative match ends it (the server is
        // the only source of truth) — don't keep predicting a zombie game.
        if (authoritative && !gameEnded) {
            gameEnded = true;
            gameOverText.textContent = 'Disconnected from server.';
            gameOverOverlay.style.display = 'flex';
        }
        unackedInputs = [];
        if (searching) {
            searching = false;
            modeOnlineBtn.classList.remove('searching');
            cancelSearchBtn.style.display = 'none';
        }
    };

    ws.onmessage = onSignalMessage;
}

// Stop a background search and hide its UI. Leaves your local game untouched.
function cancelSearch() {
    searching = false;
    modeOnlineBtn.classList.remove('searching');
    cancelSearchBtn.style.display = 'none';
    onlineStatus.style.display = 'none';
    cleanupOnline();
}

// Drop into a fresh online match. Online boards are independent (each player has
// their own seed and exchanges weapons + scores over the data channel), so this
// starts a clean board - the practice / vs-Computer game you played while waiting
// is discarded here.
function enterOnlineGame() {
    searching = false;
    modeOnlineBtn.classList.remove('searching');
    cancelSearchBtn.style.display = 'none';
    mode = 'online';
    updateModeButtons();

    const seed = (performance.now() | 0) ^ (Math.floor(Math.random() * 1e9));
    game = new WasmGame(seed);
    onlinePaused = true;

    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;
    canvas.style.width = (width * CELL_SIZE * 1.6) + 'px';
    canvas.style.height = (height * CELL_SIZE * 1.6) + 'px';
    aiBoard.style.display = 'none';
    aiLabel.style.display = 'none';
    resetMatchState(); // drop any spy/bazaar/swap state from a previous match

    paused = false;
    gameEnded = false;
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    onlineStatus.style.display = 'block';
    lastFrameTime = performance.now();
    tickAccumulator = 0;
}

// Signaling-socket message handler: matchmaking result + WebRTC relay. Shared by
// the background search and the live online game.
async function onSignalMessage(ev) {
    const msg = JSON.parse(ev.data);

    // Server-authoritative match handoff + per-frame authoritative state.
    if (msg.type === 'matchStart') {
        enterAuthoritativeGame(msg);
        return;
    }
    if (msg.type === 'snapshot') {
        if (authoritative && game) applySnapshot(msg);
        return;
    }

    if (msg.type === 'matched') {
        enterOnlineGame();
        onlineOpponentName = msg.opponent || 'Opponent';
        const conservative = (msg.oppMu - 3 * msg.oppSigma).toFixed(1);
        const quality = msg.quality != null ? Math.round(msg.quality * 100) : '?';
        setOnlineStatus(
            `Matched vs ${onlineOpponentName} (rating μ-3σ ~ ${conservative}` +
            `, quality ${quality}%) - connecting...`
        );

        // Create peer connection
        pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] });

        pc.onicecandidate = (e) => {
            if (e.candidate) {
                ws.send(JSON.stringify({ type: 'signal', data: { candidate: e.candidate } }));
            }
        };

        pc.onconnectionstatechange = () => {
            console.log('PC connection state:', pc.connectionState);
        };

        if (msg.role === 'offer') {
            // We are the offerer: create data channel then offer
            dc = pc.createDataChannel('game');
            setupDataChannel(dc);

            const offer = await pc.createOffer();
            await pc.setLocalDescription(offer);
            ws.send(JSON.stringify({ type: 'signal', data: { sdp: pc.localDescription } }));

        } else {
            // We are the answerer: wait for data channel from peer
            pc.ondatachannel = (e) => {
                dc = e.channel;
                setupDataChannel(dc);
            };
            // The answer SDP is sent when we receive the offer signal (below)
        }

    } else if (msg.type === 'signal') {
        const d = msg.data;
        if (d.sdp) {
            await pc.setRemoteDescription(d.sdp);
            if (d.sdp.type === 'offer') {
                const ans = await pc.createAnswer();
                await pc.setLocalDescription(ans);
                ws.send(JSON.stringify({ type: 'signal', data: { sdp: pc.localDescription } }));
            }
        } else if (d.candidate) {
            try {
                await pc.addIceCandidate(d.candidate);
            } catch (e) {
                console.warn('addIceCandidate error', e);
            }
        }

    } else if (msg.type === 'rating') {
        const conservative = (msg.mu - 3 * msg.sigma).toFixed(1);
        const result = msg.won ? 'WIN' : 'LOSS';
        setOnlineStatus(
            `${result} - New rating: μ=${msg.mu.toFixed(2)}, σ=${msg.sigma.toFixed(2)},` +
            ` μ-3σ ~ ${conservative}`
        );

    } else if (msg.type === 'opponentLeft') {
        setOnlineStatus('Opponent left.');
        if (!gameEnded) {
            gameEnded = true;
            gameOverText.textContent = 'Opponent left.';
            gameOverOverlay.style.display = 'flex';
        }
    }
}

// ─── Game initialization ──────────────────────────────────────────────────────

async function initGame() {
    await init();
    FIXED_DT = fixed_dt(); // canonical timestep from the engine
    startGame('practice');
}

function startGame(newMode) {
    // Online matchmaking is a background action, not a local mode - route there.
    if (newMode === 'online') {
        findMatch();
        return;
    }

    // Tearing down an established online game closes its connection. A background
    // search (mode is still a local one) is intentionally left running, so you
    // can switch between practice and vs Computer while you wait to be matched.
    if (mode === 'online') {
        cleanupOnline();
        searching = false;
        modeOnlineBtn.classList.remove('searching');
        cancelSearchBtn.style.display = 'none';
        onlineStatus.style.display = 'none';
    }

    mode = newMode;

    const seed = (performance.now() | 0) ^ (Math.floor(Math.random() * 1e9));

    // Create game instance based on mode
    if (mode === 'vscomputer') {
        const level = ernieLevelSelect ? (parseInt(ernieLevelSelect.value, 10) || 0) : DEFAULT_ERNIE_LEVEL;
        game = new WasmVsComputer(seed, level);
    } else {
        game = new WasmGame(seed);
    }

    // Set canvas size based on game dimensions
    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;

    // Set CSS for scaling (1.6x)
    canvas.style.width = (width * CELL_SIZE * 1.6) + 'px';
    canvas.style.height = (height * CELL_SIZE * 1.6) + 'px';

    // Set up AI canvas in vscomputer mode
    if (mode === 'vscomputer') {
        aiGridCanvas.width = width * CELL_SIZE;
        aiGridCanvas.height = height * CELL_SIZE;
        // Scale AI canvas smaller (1.0x)
        aiGridCanvas.style.width = (width * CELL_SIZE * 1.0) + 'px';
        aiGridCanvas.style.height = (height * CELL_SIZE * 1.0) + 'px';
        aiBoard.style.display = 'block';
        aiLabel.style.display = 'block';
    } else {
        aiBoard.style.display = 'none';
        aiLabel.style.display = 'none';
    }

    // Update UI
    updateModeButtons();
    paused = false;
    gameEnded = false;
    authoritative = false; // local modes are not server-authoritative
    resetMatchState();
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    lastFrameTime = performance.now();
    tickAccumulator = 0;
}

function updateModeButtons() {
    modePracticeBtn.classList.remove('active');
    modeVsComputerBtn.classList.remove('active');
    modeVsPlayerBtn.classList.remove('active');
    modeOnlineBtn.classList.remove('active');

    if (mode === 'practice') {
        modePracticeBtn.classList.add('active');
    } else if (mode === 'vscomputer') {
        modeVsComputerBtn.classList.add('active');
    } else if (mode === 'vsplayer') {
        modeVsPlayerBtn.classList.add('active');
    } else if (mode === 'online') {
        modeOnlineBtn.classList.add('active');
    }
}

function newGame() {
    if (mode === 'online') {
        // A P2P match can't be unilaterally restarted - drop back to a local
        // board and queue for a fresh opponent. (startGame sees mode==='online'
        // here, so it tears the connection down first.)
        startGame('practice');
        findMatch();
        return;
    }
    startGame(mode);
}

function render() {
    // Draw player board
    const grid = game.render_grid();
    const width = game.width();
    const height = game.height();
    drawBoard(ctx, grid, width, height);

    // Draw AI board in vscomputer mode
    if (mode === 'vscomputer') {
        const aiGrid = game.render_ai_grid();
        drawBoard(aiCtx, aiGrid, width, height);
    } else if (authoritative) {
        // Server-authorized spy: show the opponent's board (already degraded to
        // the spy's accuracy server-side) only while a spy of ours is active.
        if (authSpying && authSpyBoard) {
            aiBoard.style.display = 'block';
            drawBoard(aiCtx, authSpyBoard, width, height);
        } else {
            aiBoard.style.display = 'none';
        }
    } else if (spyType >= 0) {
        // A spy is up (online/2-tab): show the opponent's board on the same
        // panel. It expires on the opponent's line-clears (spyOnOpponentScore);
        // here we just keep the view live with a ~1s re-snapshot fallback.
        const now = performance.now();
        if (now - spyLastReq > 1000) requestSpy();
        aiBoard.style.display = spyHidden ? 'none' : 'block';
        if (spyGrid && !spyHidden) drawBoard(aiCtx, spyGrid, width, height);
    }
}

function updateStats() {
    const score = game.score();
    const lines = game.lines();
    // In an authoritative match, funds (changed by opponent taxes) and the bazaar
    // countdown (depends on combined lines) are authoritative per-frame; score and
    // lines come from local prediction.
    const funds = (authoritative && authSelf) ? authSelf.funds : game.funds();
    const tilBazaar = (authoritative && authSelf) ? authSelf.lines_til_bazaar : game.lines_til_bazaar();

    scoreValue.textContent = score;
    linesValue.textContent = lines;
    fundsValue.textContent = funds;
    linesToBazaarValue.textContent = tilBazaar;

    // Mirror to mobile stats bar
    mobileScore.textContent = score;
    mobileLines.textContent = lines;
    mobileFunds.textContent = funds;
    mobileLinesToBazaar.textContent = tilBazaar;
}

function updateOpponentPanel() {
    // The opponent's score/lines are authoritative per-frame in an online match.
    const opScore = (authoritative && authOpp) ? authOpp.score : game.op_score();
    const opLines = (authoritative && authOpp) ? authOpp.lines : game.op_lines();
    opponentScore.textContent = opScore >= 0 ? opScore : '-';
    opponentLines.textContent = opLines >= 0 ? opLines : '-';

    // Derive opponent name for mobile stats bar
    let oppName = 'Opponent';
    if (mode === 'vscomputer') {
        oppName = 'Ernie (computer)';
    } else if (mode === 'online' && onlineOpponentName) {
        oppName = onlineOpponentName;
    }

    // Mirror to mobile stats bar
    if (opScore >= 0) {
        mobileOpponent.textContent = `${oppName}: ${opScore}pts ${opLines}ln`;
    } else {
        mobileOpponent.textContent = '';
    }
}

function updateArsenalPanel() {
    arsenalList.innerHTML = '';
    mobileArsenalList.innerHTML = '';

    for (let i = 0; i < 10; i++) {
        const token = game.arsenal_token(i);
        const key = ARSENAL_KEYS[i];
        const slot = i; // capture for closure

        // ── Desktop arsenal item ──────────────────────────────────────────
        const div = document.createElement('div');
        div.className = 'arsenal-item';

        if (token >= 0) {
            const name = weapon_name(token);
            const qty = game.arsenal_quantity(i);
            div.textContent = `${key}. ${name} (x${qty})`;
            div.classList.add('occupied');
            div.addEventListener('click', () => {
                if (!game) return;
                predict('LaunchWeapon', slot);
            });
        } else {
            div.textContent = `${key}. < Empty >`;
        }
        arsenalList.appendChild(div);

        // ── Mobile arsenal slot ───────────────────────────────────────────
        const mslot = document.createElement('div');
        mslot.className = 'mobile-arsenal-slot';

        if (token >= 0) {
            const name = weapon_name(token);
            const qty = game.arsenal_quantity(i);
            mslot.textContent = `${key}\n${name}\nx${qty}`;
            mslot.style.whiteSpace = 'pre';
            mslot.classList.add('occupied');
            mslot.addEventListener('click', () => {
                if (!game) return;
                predict('LaunchWeapon', slot);
            });
        } else {
            mslot.textContent = `${key}\n-`;
            mslot.style.whiteSpace = 'pre';
        }
        mobileArsenalList.appendChild(mslot);
    }
}

// Currently selected token in bazaar (-1 means nothing selected)
let bazaarSelectedToken = -1;

function refreshBazaarArsenal() {
    bazaarArsenalList.innerHTML = '';
    for (let i = 0; i < 10; i++) {
        const token = game.arsenal_token(i);
        const key = ARSENAL_KEYS[i];
        const slot = document.createElement('div');
        slot.className = 'bazaar-arsenal-slot';
        if (token >= 0) {
            const qty = game.arsenal_quantity(i);
            const nm = weapon_name(token);
            slot.textContent = `${key}. ${nm} x${qty}`;
            slot.classList.add('occupied');
        } else {
            slot.textContent = `${key}. < Empty >`;
        }
        bazaarArsenalList.appendChild(slot);
    }
}

function selectBazaarToken(token) {
    bazaarSelectedToken = token;

    // Highlight the selected row
    const rows = bazaarWeaponList.querySelectorAll('.bazaar-weapon-row');
    rows.forEach((r) => {
        if (parseInt(r.dataset.token, 10) === token) {
            r.classList.add('selected');
        } else {
            r.classList.remove('selected');
        }
    });

    // Show weapon info
    if (token >= 0) {
        const price = game.bazaar_price(token);
        const duration = weapon_duration(token);
        const desc = weapon_description(token);
        bazaarInfoPrice.textContent = `Price: $${price}`;
        bazaarInfoDuration.textContent = `Duration: ${duration} lines`;
        bazaarInfoDesc.textContent = desc;
    } else {
        bazaarInfoPrice.textContent = '';
        bazaarInfoDuration.textContent = '';
        bazaarInfoDesc.textContent = '';
    }
}

function populateBazaar() {
    bazaarWeaponList.innerHTML = '';
    bazaarSelectedToken = -1;
    bazaarInfoPrice.textContent = '';
    bazaarInfoDuration.textContent = '';
    bazaarInfoDesc.textContent = '';

    const maxWeapons = max_weapons();
    for (let t = 0; t < maxWeapons; t++) {
        const name = weapon_name(t);
        const row = document.createElement('div');
        row.className = 'bazaar-weapon-row';
        row.dataset.token = t;
        row.textContent = name;
        row.addEventListener('click', () => {
            selectBazaarToken(t);
        });
        bazaarWeaponList.appendChild(row);
    }

    bazaarFunds.textContent = game.funds();
    refreshBazaarArsenal();
}

// Track whether bazaar was open last frame to avoid re-populating every tick
let bazaarWasOpen = false;

// Synchronized bazaar barrier for 2-player / online: the original freezes BOTH
// boards until BOTH players leave the shop (BTServer BT_START_BAZ/BT_END_BAZ),
// so one player can't finish early and play unopposed. We hold the local game
// frozen (it stays is_in_bazaar) until each side has hit Done.
let localBazaarDone = false;
let opponentBazaarDone = false;

// Reset all per-match relay state when a new game starts, so a previous match's
// half-finished bazaar handshake, spy view, or pending swap can't leak in.
function resetMatchState() {
    bazaarWasOpen = false;
    localBazaarDone = false;
    opponentBazaarDone = false;
    swapPending = false;
    susanPending = false;
    spyType = -1;
    spyGrid = null;
    spyRemaining = 0;
}

function maybeLeaveBazaar() {
    if (localBazaarDone && opponentBazaarDone) {
        game.leave_bazaar();
        bazaarOverlay.style.display = 'none';
        localBazaarDone = false;
        opponentBazaarDone = false;
    }
}

function updateBazaarOverlay() {
    // The bazaar is a server-authoritative barrier online: open/close on the
    // authoritative in_bazaar (prompt every frame), not the slightly-lagged local
    // prediction. Local modes use the engine's own flag.
    const inBaz = (authoritative && authSelf) ? authSelf.in_bazaar : game.is_in_bazaar();
    if (inBaz) {
        bazaarOverlay.style.display = 'flex';
        if (!bazaarWasOpen) {
            // Only fully repopulate when bazaar first opens
            populateBazaar();
            bazaarWasOpen = true;
            // New round: reset OUR done-state (keep the opponent's, which may
            // have arrived before our slightly-lagged score relay opened ours).
            localBazaarDone = false;
            if (bazaarDoneBtn) {
                bazaarDoneBtn.disabled = false;
                bazaarDoneBtn.textContent = 'DONE';
            }
        } else {
            // Keep funds and arsenal display fresh while open
            bazaarFunds.textContent = (authoritative && authSelf) ? authSelf.funds : game.funds();
            refreshBazaarArsenal();
        }
    } else {
        if (bazaarWasOpen) {
            bazaarOverlay.style.display = 'none';
            bazaarWasOpen = false;
        }
    }
}

// Weapon token indices (from WeaponToken). Mirror Mirror is OFFENSIVE
// (BTWeaponManager.C:204-219): launching it curses your OPPONENT. While a
// player is mirror-cursed, every weapon THEY launch is caught by the curse -
// these nine simply fizzle (the original's nullify list), everything else
// backfires onto the cursed launcher. The curse is resolved on the LAUNCH side,
// so an incoming weapon is just applied (the sender already handled their curse).
const SWAP_TOKEN = 5;
const UPBYSIDE_TOKEN = 3;
const BOTTLE_TOKEN = 24;
const SUSAN_TOKEN = 26;
const MIRROR_TOKEN = 28;
const MIRROR_NULLIFY = new Set([5, 13, 14, 17, 18, 19, 20, 26, 28]);
// Swap, Mondale, Keating, Ames, Ace, Condor, NiceDay, Susan, Mirror.

// Initiator guards for the two-way exchanges (the original's `swapper_`): while
// one is outstanding, a simultaneous swap/susan from the opponent is a collision
// - decline it so neither side double-imports and corrupts its board (D3).
let swapPending = false;
let susanPending = false;

function sendToOpponent(msg) {
    if (gameEnded) return;
    if (mode === 'vsplayer' && broadcastChannel) broadcastChannel.postMessage(msg);
    else if (mode === 'online') dcSend(msg);
}

function sendWeaponToOpponent(token) {
    sendToOpponent({ kind: 'weapon', token });
}

// Swap and Lazy Susan are two-way exchanges over the channel: we send ours, the
// opponent applies it and sends theirs back, and we apply that. Both clear
// Bottle/Upbyside on a board swap. A Mirror-shielded opponent nullifies either.
function initiateSwap() {
    // Clear Bottle/Upbyside on OUR side before exporting, so the board we hand
    // over is already cleaned (the original drops both on a swap, both sides).
    clearBottleUpbyside();
    swapPending = true;
    sendToOpponent({ kind: 'swap', board: Array.from(game.export_board()) });
}
function initiateSusan() {
    susanPending = true;
    sendToOpponent({ kind: 'susan', arsenal: Array.from(game.export_arsenal()) });
}
function clearBottleUpbyside() {
    game.force_weapon_off(BOTTLE_TOKEN);
    game.force_weapon_off(UPBYSIDE_TOKEN);
}

// Spies (Ames/Ace/Condor): launching one requests the opponent's board for
// display on the aiBoard panel. The cheaper spies are unreliable (cells drop
// out); the Condor is perfect. Token indices + obscure-fractions.
const AMES_TOKEN = 17, ACE_TOKEN = 18, CONDOR_TOKEN = 19;
const SPY_TOKENS = new Set([AMES_TOKEN, ACE_TOKEN, CONDOR_TOKEN]);
// Fraction of the opponent's cells that DROP OUT of the spy view (1 - report_prob
// from BTRecon.C:58-62): Ames shows 50%, Ace 85%, the Condor satellite is perfect.
const SPY_DROP = { 17: 0.50, 18: 0.15, 19: 0.0 };
const EMPTY_ID = -2; // matches WasmGame render-grid EMPTY
const aiBoardLabel = document.getElementById('aiBoardLabel');
let spyType = -1;     // active spy token, or -1
let spyRemaining = 0; // lines of opponent clears left before the spy expires (BTRecon spy_on_)
let spyOpLines = 0;   // opponent line count at the last decrement (to measure the delta)
let spyGrid = null;   // latest (degraded) opponent grid
let spyLastReq = 0;   // last spyRequest time
let spyHidden = false; // Condor 'c' toggle

function initiateSpy(token) {
    // Duration is measured in OPPONENT line-clears (BTRecon.C:201-209), not
    // seconds: the spy expires after the opponent clears `duration` lines.
    // Relaunching ACCUMULATES the budget and switches the accuracy to the newest
    // spy (BTRecon.C:171-179: `spy_on_ += duration; spy_token_ = token`).
    if (spyType < 0) spyOpLines = game.op_lines(); // fresh spy: start counting now
    spyRemaining += weapon_duration(token) || 20;
    spyType = token;
    spyHidden = false;
    if (aiBoardLabel) aiBoardLabel.textContent = 'Opponent (spy)';
    requestSpy();
}

// Decrement the spy's line budget by the opponent's new clears, and refresh the
// view (the original re-snapshots on every BT_OP_SCORE). Called when an opponent
// score update arrives.
function spyOnOpponentScore() {
    if (spyType < 0) return;
    const opLines = game.op_lines();
    const delta = opLines - spyOpLines;
    if (delta > 0) {
        spyRemaining -= delta;
        spyOpLines = opLines;
    }
    if (spyRemaining <= 0) clearSpy();
    else requestSpy(); // re-snapshot the opponent board
}
function requestSpy() {
    spyLastReq = performance.now();
    sendToOpponent({ kind: 'spyRequest' });
}
function degradeGrid(grid, token) {
    const drop = SPY_DROP[token] || 0;
    if (drop <= 0) return grid;
    return grid.map((id) => (id !== EMPTY_ID && Math.random() < drop ? EMPTY_ID : id));
}
function clearSpy() {
    spyType = -1;
    spyGrid = null;
    if (mode !== 'vscomputer') aiBoard.style.display = 'none';
}

// Handle the cross-player exchange messages. Returns true if it consumed `m`.
function handleExchange(m) {
    switch (m.kind) {
        case 'spyRequest':
            // The opponent is spying us; send our current board.
            sendToOpponent({ kind: 'spyBoard', grid: Array.from(game.render_grid()) });
            return true;
        case 'spyBoard':
            if (spyType >= 0) spyGrid = degradeGrid(m.grid, spyType);
            return true;
        case 'swap': {
            // Collision guard (D3): if we also have a swap outstanding, both
            // sides launched at once - decline so neither double-imports.
            if (swapPending) {
                sendToOpponent({ kind: 'swapAck', nullified: true });
            } else {
                const mine = Array.from(game.export_board());
                game.import_board(Int32Array.from(m.board));
                clearBottleUpbyside();
                sendToOpponent({ kind: 'swapAck', board: mine });
            }
            return true;
        }
        case 'swapAck': {
            swapPending = false;
            if (!m.nullified) {
                game.import_board(Int32Array.from(m.board));
                clearBottleUpbyside();
            }
            return true;
        }
        case 'susan': {
            if (susanPending) {
                sendToOpponent({ kind: 'susanAck', nullified: true });
            } else {
                const mine = Array.from(game.export_arsenal());
                game.import_arsenal(Int32Array.from(m.arsenal));
                sendToOpponent({ kind: 'susanAck', arsenal: mine });
            }
            return true;
        }
        case 'susanAck': {
            susanPending = false;
            if (!m.nullified) game.import_arsenal(Int32Array.from(m.arsenal));
            return true;
        }
    }
    return false;
}

// An incoming weapon from the opponent (online/2-tab; in vs-Computer the engine
// relay does this instead). With the offensive Mirror, the sender already
// resolved their own curse before launching, so we just apply what arrives -
// including a Mirror, which curses us (game.receive_weapon sets BTActive[MIRROR]).
function receiveWeaponFromOpponent(token) {
    game.receive_weapon(token);
}

function processEvents() {
    const events = game.drain_events();

    for (let i = 0; i < events.length; i += 4) {
        const tag = events[i];
        const a = events[i + 1];
        const b = events[i + 2];
        const c = events[i + 3];

        // Audio for the local player's events (synthesized in sound.js).
        if (tag === 0) {
            // Locked: a = lines cleared (0 = just a lock).
            if (a > 0) Sound.clear(a); else Sound.lock();
        } else if (tag === 1) {
            Sound.weapon();
        } else if (tag === 3) {
            Sound.bazaar();
        } else if (tag === 4) {
            // Airslide: the piece tucked under a ledge.
            Sound.airslide();
        } else if (tag === 5) {
            Sound.gameOver();
        } else if (tag === 6) {
            // Idiot: a = reason (0 = bad move, 1 = near death, 2 = missed smiley).
            if (a === 0) Sound.badMove();
            else if (a === 1) Sound.nearDeath();
            else if (a === 2) Sound.missedSmiley();
        }

        // In an authoritative match the SERVER resolves every cross-player effect
        // and game-over: the client sends its actions via predict() and takes all
        // state from snapshots, so here it only plays audio for its own events.
        if (authoritative) continue;

        if (tag === 1) {
            // WeaponLaunched. (vs-Computer routes weapons through the engine
            // relay, so this block only fires for vsplayer/online.)
            if ((mode === 'vsplayer' || mode === 'online') && !gameEnded) {
                if (game.weapon_active && game.weapon_active(MIRROR_TOKEN)) {
                    // We are mirror-cursed: our own launch is caught by the curse.
                    // The nullify-9 (incl. Mirror itself and the spies - D6) fizzle;
                    // everything else backfires onto us.
                    if (!MIRROR_NULLIFY.has(a)) game.receive_weapon(a);
                } else if (a === SWAP_TOKEN) initiateSwap();
                else if (a === SUSAN_TOKEN) initiateSusan();
                else if (SPY_TOKENS.has(a)) initiateSpy(a); // request opponent board
                else sendWeaponToOpponent(a); // Mirror included: curses the opponent
            }
        } else if (tag === 7) {
            // FundsStolen: we (the victim) were taxed/robbed - credit the
            // attacker (the opponent banks it via game.add_funds).
            if ((mode === 'vsplayer' || mode === 'online') && !gameEnded && a !== 0) {
                sendToOpponent({ kind: 'funds', amount: a });
            }
        } else if (tag === 2) {
            // Scored: relay score update
            if (mode === 'vsplayer' && broadcastChannel && !gameEnded) {
                broadcastChannel.postMessage({ kind: 'score', score: a, lines: b, funds: c });
            } else if (mode === 'online' && !gameEnded) {
                dcSend({ kind: 'score', score: a, lines: b, funds: c });
            }
        } else if (tag === 5) {
            // GameOver: tell opponent we died
            gameEnded = true;
            gameOverText.textContent = 'GAME OVER - You Lost';
            gameOverOverlay.style.display = 'flex';
            if (mode === 'vsplayer' && broadcastChannel) {
                broadcastChannel.postMessage({ kind: 'dead' });
            } else if (mode === 'online') {
                dcSend({ kind: 'dead' });
                if (ws) {
                    ws.send(JSON.stringify({
                        type: 'result',
                        won: false,
                        lines: game.lines(),
                        opLines: game.op_lines()
                    }));
                }
            }
        }
    }
}

function gameLoop(now) {
    if (!game) {
        requestAnimationFrame(gameLoop);
        return;
    }

    if (lastFrameTime === 0) {
        lastFrameTime = now;
    }

    let frameDt = now - lastFrameTime;
    lastFrameTime = now;
    if (frameDt > MAX_FRAME_DT) frameDt = MAX_FRAME_DT;

    // In online mode, don't tick until the data channel is open
    const shouldTick = !paused && !gameEnded && !game.is_game_over() &&
        !(mode === 'online' && onlinePaused);

    // Advance the engine in fixed FIXED_DT steps (accumulator). This keeps the
    // simulation - and therefore every recording - deterministic and decoupled
    // from frame-rate jitter.
    if (shouldTick) {
        tickAccumulator += frameDt;
        let steps = 0;
        while (tickAccumulator >= FIXED_DT && steps < MAX_STEPS_PER_FRAME) {
            game.tick(FIXED_DT);
            tickAccumulator -= FIXED_DT;
            steps++;
        }
        // If we hit the catch-up cap, drop the backlog rather than banking time.
        if (steps >= MAX_STEPS_PER_FRAME) tickAccumulator = 0;
    } else {
        // Don't accumulate wall-clock time while paused / over / not yet live.
        tickAccumulator = 0;
    }

    // Process events and relay to opponent (vsplayer or online)
    if (!onlinePaused || mode !== 'online') {
        processEvents();
    }

    // Check for win/loss in vscomputer mode. NOTE: don't gate on
    // game.is_game_over() - it returns true as soon as `result` is set (it ORs
    // in result != 0), so gating on it would suppress the win banner when Ernie
    // tops out (the player is still alive). Read `result` directly.
    if (mode === 'vscomputer' && !gameEnded) {
        const result = game.result();
        if (result === 1) {
            gameEnded = true;
            gameOverText.textContent = 'YOU WIN!';
            gameOverOverlay.style.display = 'flex';
        } else if (result === 2) {
            gameEnded = true;
            gameOverText.textContent = 'GAME OVER';
            gameOverOverlay.style.display = 'flex';
        }
    }

    // Render
    render();
    updateStats();
    updateOpponentPanel();
    updateArsenalPanel();
    updateBazaarOverlay();

    requestAnimationFrame(gameLoop);
}

function handleBroadcastMessage(ev) {
    if (mode !== 'vsplayer') return;

    const m = ev.data;

    if (handleExchange(m)) {
        // consumed (swap/susan exchange)
    } else if (m.kind === 'weapon') {
        // Opponent launched a weapon at us (the sender resolved their own curse).
        receiveWeaponFromOpponent(m.token);
    } else if (m.kind === 'funds') {
        game.add_funds(m.amount); // Mondale/Keating credit from the victim
    } else if (m.kind === 'bazaarDone') {
        opponentBazaarDone = true;
        maybeLeaveBazaar();
    } else if (m.kind === 'score') {
        // Opponent score update
        game.receive_op_score(m.score, m.lines, m.funds);
        spyOnOpponentScore(); // line-based spy expiry + refresh
    } else if (m.kind === 'dead') {
        // Opponent died; we win
        gameEnded = true;
        gameOverText.textContent = 'YOU WIN!\n(opponent died)';
        gameOverOverlay.style.display = 'flex';
    }
}

// ─── Touch gesture handling on game canvas ────────────────────────────────────

let touchState = null; // Tracks the active game touch gesture

canvas.addEventListener('touchstart', (e) => {
    // Only track the first touch
    if (e.changedTouches.length === 0) return;
    e.preventDefault();

    if (!game || gameEnded || game.is_game_over() || inBazaar()) return;
    if (mode === 'online' && onlinePaused) return;

    const touch = e.changedTouches[0];
    const cell = canvas.clientWidth / game.width();

    touchState = {
        id: touch.identifier,
        startX: touch.clientX,
        startY: touch.clientY,
        lastX: touch.clientX,
        startTime: performance.now(),
        accDx: 0,   // accumulated horizontal delta (in pixels, reset per-cell)
        totalDx: 0, // total horizontal travel
        totalDy: 0, // total vertical travel
        cell: cell,
        dropped: false,
    };
}, { passive: false });

canvas.addEventListener('touchmove', (e) => {
    e.preventDefault();
    if (!touchState || !game) return;

    // Find our tracked touch
    let touch = null;
    for (let i = 0; i < e.changedTouches.length; i++) {
        if (e.changedTouches[i].identifier === touchState.id) {
            touch = e.changedTouches[i];
            break;
        }
    }
    if (!touch) return;

    const dx = touch.clientX - touchState.lastX;
    const dy = touch.clientY - touchState.startY;

    touchState.totalDx = touch.clientX - touchState.startX;
    touchState.totalDy = dy;
    touchState.accDx += dx;
    touchState.lastX = touch.clientX;

    // Horizontal drag: move piece one cell at a time
    const cell = touchState.cell;
    while (touchState.accDx >= cell) {
        predict('MoveRight');
        touchState.accDx -= cell;
    }
    while (touchState.accDx <= -cell) {
        predict('MoveLeft');
        touchState.accDx += cell;
    }

    // Downward flick detection during move (early trigger)
    if (!touchState.dropped &&
        touchState.totalDy > 45 &&
        Math.abs(touchState.totalDy) > Math.abs(touchState.totalDx)) {
        touchState.dropped = true;
        predict('BeginDrop');
    }
}, { passive: false });

canvas.addEventListener('touchend', (e) => {
    e.preventDefault();
    if (!touchState || !game) return;

    // Find our tracked touch
    let touch = null;
    for (let i = 0; i < e.changedTouches.length; i++) {
        if (e.changedTouches[i].identifier === touchState.id) {
            touch = e.changedTouches[i];
            break;
        }
    }
    if (!touch) {
        // Touch ended but not tracked; clear state
        touchState = null;
        return;
    }

    const duration = performance.now() - touchState.startTime;
    const absDx = Math.abs(touchState.totalDx);
    const absDy = Math.abs(touchState.totalDy);

    // Downward flick (if not already triggered during move)
    if (!touchState.dropped &&
        touchState.totalDy > 45 &&
        absDy > absDx) {
        predict('BeginDrop');
        touchState = null;
        return;
    }

    // Tap -> rotate (small movement, short time, no drop)
    if (!touchState.dropped && absDx < 12 && absDy < 12 && duration < 250) {
        predict('Rotate');
    }

    touchState = null;
}, { passive: false });

canvas.addEventListener('touchcancel', (e) => {
    touchState = null;
}, { passive: false });

// ─── On-screen touch control bar ─────────────────────────────────────────────

function setupTouchButton(btnId, action, repeatInterval) {
    const btn = document.getElementById(btnId);
    if (!btn) return;

    // Initial delay before a held button starts auto-repeating (key-repeat
    // style). A quick tap fires exactly once; only holding past this repeats.
    const REPEAT_DELAY = 250;

    let repeatTimer = null;
    let delayTimer = null;

    function fireAction() {
        if (!game || gameEnded || game.is_game_over() || inBazaar()) return;
        if (mode === 'online' && onlinePaused) return;
        Sound.resume(); // touch is the gesture that unlocks Web Audio
        markActive();
        action();
    }

    function startRepeat() {
        fireAction(); // exactly one action on press
        if (repeatInterval != null) {
            // Don't repeat until the button has been held for REPEAT_DELAY,
            // so a single tap is never double-counted.
            delayTimer = setTimeout(() => {
                repeatTimer = setInterval(fireAction, repeatInterval);
            }, REPEAT_DELAY);
        }
    }

    function stopRepeat() {
        if (delayTimer !== null) {
            clearTimeout(delayTimer);
            delayTimer = null;
        }
        if (repeatTimer !== null) {
            clearInterval(repeatTimer);
            repeatTimer = null;
        }
    }

    btn.addEventListener('pointerdown', (e) => {
        e.preventDefault();
        try { btn.setPointerCapture(e.pointerId); } catch (_) {}
        startRepeat();
    });

    btn.addEventListener('pointerup', (e) => {
        e.preventDefault();
        stopRepeat();
    });

    btn.addEventListener('pointercancel', (e) => {
        stopRepeat();
    });

    btn.addEventListener('pointerleave', (e) => {
        stopRepeat();
    });
}

// Set up buttons after DOM is ready (called after initGame)
function setupTouchControls() {
    setupTouchButton('touchLeft',   () => predict('MoveLeft'),  90);
    setupTouchButton('touchRight',  () => predict('MoveRight'), 90);
    setupTouchButton('touchRotate', () => predict('Rotate'),    null);
    // Soft drop: tap = one cell; hold = fast controlled descent (35ms/cell).
    setupTouchButton('touchDrop',   () => predict('SoftDrop'),  35);
}

// Input handling
function handleKeyDown(e) {
    if (!game) return;

    // First keypress is the user gesture that unlocks Web Audio.
    Sound.resume();

    // In online mode, don't accept input until connected
    if (mode === 'online' && onlinePaused) return;

    // The bazaar freezes the match (its own Add/Remove/Done buttons drive it);
    // ignore gameplay keys so a held Space can't slam pieces or inflate score.
    if (inBazaar()) return;

    const key = e.key;

    // Count this as activity only for keys that actually drive the game (so a
    // stray Tab/Shift doesn't mark you "online").
    if (GAMEPLAY_KEYS.has(key)) markActive();

    // Condor is a hold-'c' toggle: flip the spy view on/off (BTGame::condor).
    if ((key === 'c' || key === 'C') && spyType === CONDOR_TOKEN) {
        spyHidden = !spyHidden;
        return;
    }

    // Arrow keys and pause
    switch (key) {
        case 'ArrowLeft':
            e.preventDefault();
            predict('MoveLeft');
            return;
        case 'ArrowRight':
            e.preventDefault();
            predict('MoveRight');
            return;
        case 'ArrowUp':
            e.preventDefault();
            predict('Rotate');
            return;
        case 'ArrowDown':
            // Soft drop one cell; holding repeats via the OS key-repeat.
            e.preventDefault();
            predict('SoftDrop');
            return;
        case ' ':
        case 'Spacebar':
            // Hard drop (slam to the bottom).
            e.preventDefault();
            predict('BeginDrop');
            return;
        case 'p':
        case 'P':
            paused = !paused;
            predict('SetPaused', paused);
            return;
    }

    // Weapon launch: digits 1-0 map to arsenal slots 0-9
    if (key >= '1' && key <= '9') {
        const slot = parseInt(key) - 1;
        predict('LaunchWeapon', slot);
    } else if (key === '0') {
        predict('LaunchWeapon', 9);
    }
}

// Event listeners
document.addEventListener('keydown', handleKeyDown);
newGameBtn.addEventListener('click', newGame);

bazaarDoneBtn.addEventListener('click', () => {
    if (authoritative) {
        // Server-authoritative barrier: tell the server we're done. It clears our
        // in_bazaar (via the next keyframe / you-status) once BOTH players are done.
        predict('LeaveBazaar');
        bazaarDoneBtn.disabled = true;
        bazaarDoneBtn.textContent = 'WAITING FOR OPPONENT...';
    } else if (mode === 'vsplayer' || mode === 'online') {
        // Don't resume until the opponent is also done (synchronized barrier).
        localBazaarDone = true;
        bazaarDoneBtn.disabled = true;
        bazaarDoneBtn.textContent = 'WAITING FOR OPPONENT...';
        if (mode === 'vsplayer' && broadcastChannel) broadcastChannel.postMessage({ kind: 'bazaarDone' });
        else if (mode === 'online') dcSend({ kind: 'bazaarDone' });
        maybeLeaveBazaar();
    } else {
        game.leave_bazaar();
        bazaarOverlay.style.display = 'none';
    }
});

bazaarAddBtn.addEventListener('click', () => {
    if (bazaarSelectedToken < 0 || !game) return;
    if (predict('BuyWeapon', bazaarSelectedToken)) {
        bazaarFunds.textContent = game.funds();
        updateStats();
        updateArsenalPanel();
        refreshBazaarArsenal();
        // Re-select to refresh price (may have changed due to Carter doubling)
        selectBazaarToken(bazaarSelectedToken);
    }
});

bazaarRemoveBtn.addEventListener('click', () => {
    if (bazaarSelectedToken < 0 || !game) return;
    if (predict('SellWeapon', bazaarSelectedToken)) {
        bazaarFunds.textContent = game.funds();
        updateStats();
        updateArsenalPanel();
        refreshBazaarArsenal();
        selectBazaarToken(bazaarSelectedToken);
    }
});

// Mode selector buttons
modePracticeBtn.addEventListener('click', () => startGame('practice'));
modeVsComputerBtn.addEventListener('click', () => startGame('vscomputer'));
modeVsPlayerBtn.addEventListener('click', () => startGame('vsplayer'));
modeOnlineBtn.addEventListener('click', () => startGame('online'));
if (cancelSearchBtn) cancelSearchBtn.addEventListener('click', cancelSearch);

// Changing Ernie's difficulty restarts the current vs-computer game so it
// takes effect immediately (it's read at WasmVsComputer construction).
if (ernieLevelSelect) {
    ernieLevelSelect.addEventListener('change', () => {
        if (mode === 'vscomputer') startGame('vscomputer');
    });
}

// ─── Bug report ─────────────────────────────────────────────────────────────
// Capture a deterministic replay of the current game, upload it for a shareable
// link, and open a prefilled GitHub issue. No server-side secret: the user
// reviews and posts the issue themselves.
const BUG_REPO = 'perplexes/BattleTris';
const bugOverlay = document.getElementById('bugOverlay');
const bugTitleInput = document.getElementById('bugTitle');
const bugExpected = document.getElementById('bugExpected');
const bugActual = document.getElementById('bugActual');
const bugStatus = document.getElementById('bugStatus');
const bugSubmit = document.getElementById('bugSubmit');
const bugCancel = document.getElementById('bugCancel');
const reportBugBtn = document.getElementById('reportBug');

// Sound on/off toggle (persisted). Default on.
const soundToggleBtn = document.getElementById('soundToggle');
if (soundToggleBtn) {
    const savedMute = localStorage.getItem('bt_muted') === '1';
    Sound.setMuted(savedMute);
    soundToggleBtn.textContent = savedMute ? 'Sound: Off' : 'Sound: On';
    soundToggleBtn.addEventListener('click', () => {
        const muted = !Sound.isMuted();
        Sound.setMuted(muted);
        Sound.resume();
        localStorage.setItem('bt_muted', muted ? '1' : '0');
        soundToggleBtn.textContent = muted ? 'Sound: Off' : 'Sound: On';
    });
}

// Replay snapshot taken when the modal opens - the "bug moment" - so it isn't
// affected by play continuing while the user types.
let bugReplayJson = null;

function openBug() {
    bugReplayJson = (game && typeof game.export_replay === 'function') ? game.export_replay() : null;
    bugStatus.textContent = bugReplayJson ? '' : 'No active game - the report will have no replay attached.';
    bugSubmit.disabled = false;
    bugOverlay.classList.add('open');
    bugTitleInput.focus();
}

function closeBug() {
    bugOverlay.classList.remove('open');
}

async function uploadReplay(json) {
    const res = await fetch('/api/replays', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: json,
    });
    if (!res.ok) throw new Error('upload failed (' + res.status + ')');
    return (await res.json()).id;
}

function downloadReplay(json) {
    const blob = new Blob([json], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'battletris-replay.json';
    a.click();
    URL.revokeObjectURL(a.href);
}

function buildIssueBody(expected, actual, replayUrl, meta) {
    let b = '';
    b += '**Expected**\n' + (expected.trim() || '_(not provided)_') + '\n\n';
    b += '**Actual**\n' + (actual.trim() || '_(not provided)_') + '\n\n';
    b += '---\n';
    if (replayUrl) b += '- Replay: ' + replayUrl + '\n';
    if (meta) {
        const lvl = (meta.ai_level !== null && meta.ai_level !== undefined) ? ` (Ernie ${meta.ai_level})` : '';
        b += `- Mode: ${meta.mode}${lvl}\n`;
        b += `- Engine: \`${meta.engine_sha}\` · seed: ${meta.seed} · ticks: ${meta.tick_count} · inputs: ${meta.inputs}\n`;
    }
    b += `- Page: ${location.href}\n`;
    b += `- Browser: ${navigator.userAgent}\n`;
    return b;
}

async function submitBug() {
    bugSubmit.disabled = true;
    let replayUrl = null;
    let meta = null;
    if (bugReplayJson) {
        try {
            const r = JSON.parse(bugReplayJson);
            meta = { mode: r.mode, ai_level: r.ai_level, engine_sha: r.engine_sha, seed: r.seed, tick_count: r.tick_count, inputs: (r.frames || []).length };
        } catch (e) { /* metadata is best-effort */ }
        try {
            bugStatus.textContent = 'Uploading replay...';
            const id = await uploadReplay(bugReplayJson);
            replayUrl = `${location.origin}/replay/${id}`;
        } catch (e) {
            bugStatus.textContent = 'Replay upload failed - downloading it instead; attach it to the issue manually.';
            downloadReplay(bugReplayJson);
        }
    }
    const title = bugTitleInput.value.trim() || 'Bug report';
    const body = buildIssueBody(bugExpected.value, bugActual.value, replayUrl, meta);
    const url = `https://github.com/${BUG_REPO}/issues/new?title=${encodeURIComponent(title)}&body=${encodeURIComponent(body)}&labels=bug`;
    window.open(url, '_blank', 'noopener');
    bugSubmit.disabled = false;
    closeBug();
}

if (reportBugBtn) reportBugBtn.addEventListener('click', openBug);
if (bugCancel) bugCancel.addEventListener('click', closeBug);
if (bugSubmit) bugSubmit.addEventListener('click', submitBug);
if (bugOverlay) bugOverlay.addEventListener('click', (e) => { if (e.target === bugOverlay) closeBug(); });

// ─── Share replay ─────────────────────────────────────────────────────────
// Save the current game to the replay library and copy a shareable link.
const shareReplayBtn = document.getElementById('shareReplay');
const toast = document.getElementById('toast');
let toastTimer = null;

function showToast(msg, ms = 4500) {
    toast.textContent = msg;
    toast.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => toast.classList.remove('show'), ms);
}

async function shareReplay() {
    if (!game || typeof game.export_replay !== 'function') {
        showToast('No active game to share.');
        return;
    }
    showToast('Saving replay...', 10000);
    try {
        const id = await uploadReplay(game.export_replay());
        const url = `${location.origin}/replay/${id}`;
        try {
            await navigator.clipboard.writeText(url);
            showToast('Replay link copied: ' + url);
        } catch (e) {
            showToast('Replay link: ' + url);
        }
    } catch (e) {
        showToast('Share failed: ' + e.message);
    }
}

if (shareReplayBtn) shareReplayBtn.addEventListener('click', shareReplay);

const openLibraryBtn = document.getElementById('openLibrary');
if (openLibraryBtn) openLibraryBtn.addEventListener('click', () => { location.href = '/www/library.html'; });

const openLeaderboardBtn = document.getElementById('openLeaderboard');
if (openLeaderboardBtn) openLeaderboardBtn.addEventListener('click', () => { location.href = '/www/leaderboard.html'; });

// Initialize and start game loop
(async () => {
    await initGame();

    // Set up broadcast channel for two-player communication
    broadcastChannel = new BroadcastChannel('battletris');
    broadcastChannel.onmessage = handleBroadcastMessage;

    // Wire up on-screen touch control buttons
    setupTouchControls();

    // Open the live-stats channel (players online + visitor hit counter).
    connectStats();

    requestAnimationFrame(gameLoop);
})();
