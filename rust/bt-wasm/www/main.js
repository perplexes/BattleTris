import init, { WasmGame, WasmVsComputer, fixed_dt, max_weapons, weapon_name, weapon_description, weapon_price, weapon_duration } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';

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
let pc = null;       // RTCPeerConnection
let dc = null;       // RTCDataChannel
let onlinePaused = true;  // true until the data channel opens
let onlineOpponentName = '';
let searching = false;    // background matchmaking in progress (queued, not yet matched)
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
        if (m.kind === 'weapon') {
            game.receive_weapon(m.token);
        } else if (m.kind === 'score') {
            game.receive_op_score(m.score, m.lines, m.funds);
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

    ws.onopen = () => ws.send(JSON.stringify({ type: 'queue', name }));

    ws.onerror = (err) => {
        console.warn('WebSocket error', err);
        setOnlineStatus(`Connection error - is the server reachable at ${wsUrl}?`);
    };

    ws.onclose = () => {
        if (!gameEnded) setOnlineStatus('Disconnected from server.');
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
    }
}

function updateStats() {
    const score = game.score();
    const lines = game.lines();
    const funds = game.funds();
    const tilBazaar = game.lines_til_bazaar();

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
    const opScore = game.op_score();
    const opLines = game.op_lines();
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
                game.launch_weapon(slot);
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
                game.launch_weapon(slot);
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

function updateBazaarOverlay() {
    if (game.is_in_bazaar()) {
        bazaarOverlay.style.display = 'flex';
        if (!bazaarWasOpen) {
            // Only fully repopulate when bazaar first opens
            populateBazaar();
            bazaarWasOpen = true;
        } else {
            // Keep funds and arsenal display fresh while open
            bazaarFunds.textContent = game.funds();
            refreshBazaarArsenal();
        }
    } else {
        if (bazaarWasOpen) {
            bazaarOverlay.style.display = 'none';
            bazaarWasOpen = false;
        }
    }
}

function processEvents() {
    const events = game.drain_events();

    for (let i = 0; i < events.length; i += 4) {
        const tag = events[i];
        const a = events[i + 1];
        const b = events[i + 2];
        const c = events[i + 3];

        if (tag === 1) {
            // WeaponLaunched: relay to opponent
            if (mode === 'vsplayer' && broadcastChannel && !gameEnded) {
                broadcastChannel.postMessage({ kind: 'weapon', token: a });
            } else if (mode === 'online' && !gameEnded) {
                dcSend({ kind: 'weapon', token: a });
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

    if (m.kind === 'weapon') {
        // Opponent launched a weapon at us
        game.receive_weapon(m.token);
    } else if (m.kind === 'score') {
        // Opponent score update
        game.receive_op_score(m.score, m.lines, m.funds);
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

    if (!game || gameEnded || game.is_game_over() || game.is_in_bazaar()) return;
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
        game.move_right();
        touchState.accDx -= cell;
    }
    while (touchState.accDx <= -cell) {
        game.move_left();
        touchState.accDx += cell;
    }

    // Downward flick detection during move (early trigger)
    if (!touchState.dropped &&
        touchState.totalDy > 45 &&
        Math.abs(touchState.totalDy) > Math.abs(touchState.totalDx)) {
        touchState.dropped = true;
        game.begin_drop();
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
        game.begin_drop();
        touchState = null;
        return;
    }

    // Tap -> rotate (small movement, short time, no drop)
    if (!touchState.dropped && absDx < 12 && absDy < 12 && duration < 250) {
        game.rotate();
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
        if (!game || gameEnded || game.is_game_over() || game.is_in_bazaar()) return;
        if (mode === 'online' && onlinePaused) return;
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
    setupTouchButton('touchLeft',   () => game.move_left(),   90);
    setupTouchButton('touchRight',  () => game.move_right(),  90);
    setupTouchButton('touchRotate', () => game.rotate(),      null);
    // Soft drop: tap = one cell; hold = fast controlled descent (35ms/cell).
    setupTouchButton('touchDrop',   () => game.soft_drop(),   35);
}

// Input handling
function handleKeyDown(e) {
    if (!game) return;

    // In online mode, don't accept input until connected
    if (mode === 'online' && onlinePaused) return;

    // The bazaar freezes the match (its own Add/Remove/Done buttons drive it);
    // ignore gameplay keys so a held Space can't slam pieces or inflate score.
    if (game.is_in_bazaar()) return;

    const key = e.key;

    // Arrow keys and pause
    switch (key) {
        case 'ArrowLeft':
            e.preventDefault();
            game.move_left();
            return;
        case 'ArrowRight':
            e.preventDefault();
            game.move_right();
            return;
        case 'ArrowUp':
            e.preventDefault();
            game.rotate();
            return;
        case 'ArrowDown':
            // Soft drop one cell; holding repeats via the OS key-repeat.
            e.preventDefault();
            game.soft_drop();
            return;
        case ' ':
        case 'Spacebar':
            // Hard drop (slam to the bottom).
            e.preventDefault();
            game.begin_drop();
            return;
        case 'p':
        case 'P':
            paused = !paused;
            game.set_paused(paused);
            return;
    }

    // Weapon launch: digits 1-0 map to arsenal slots 0-9
    if (key >= '1' && key <= '9') {
        const slot = parseInt(key) - 1;
        game.launch_weapon(slot);
    } else if (key === '0') {
        game.launch_weapon(9);
    }
}

// Event listeners
document.addEventListener('keydown', handleKeyDown);
newGameBtn.addEventListener('click', newGame);

bazaarDoneBtn.addEventListener('click', () => {
    game.leave_bazaar();
    bazaarOverlay.style.display = 'none';
});

bazaarAddBtn.addEventListener('click', () => {
    if (bazaarSelectedToken < 0 || !game) return;
    if (game.buy_weapon(bazaarSelectedToken)) {
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
    if (game.sell_weapon(bazaarSelectedToken)) {
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
