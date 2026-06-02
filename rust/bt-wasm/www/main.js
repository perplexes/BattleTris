import init, { WasmGame, WasmVsComputer, max_weapons, weapon_name, weapon_description, weapon_price, weapon_duration } from '../pkg/bt_wasm.js';

// Game state
let game = null;
let mode = 'practice'; // 'practice', 'vscomputer', 'vsplayer', 'online'
let lastFrameTime = 0;
let paused = false;
let gameEnded = false;
let broadcastChannel = null;

// Online / WebRTC state
let ws = null;       // WebSocket to signaling server
let pc = null;       // RTCPeerConnection
let dc = null;       // RTCDataChannel
let onlinePaused = true;  // true until the data channel opens
let onlineOpponentName = '';

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

// Mobile UI elements
const mobileScore = document.getElementById('mobileScore');
const mobileLines = document.getElementById('mobileLines');
const mobileFunds = document.getElementById('mobileFunds');
const mobileLinesToBazaar = document.getElementById('mobileLinesToBazaar');
const mobileOpponent = document.getElementById('mobileOpponent');
const mobileArsenalList = document.getElementById('mobileArsenalList');

// Preload gimp image for cell id 23
const gimpImg = new Image();
gimpImg.src = 'assets/btgimp.png';

// Palette: cell id -> { bright, dark }. Exact RGB from the original X11
// resource defaults (BattleTris.C): bright = base color, dark = its
// dark/shadow variant used for the bevel border.
const PALETTE = {
    1: { bright: '#eeeee0', dark: '#a8a8a8' }, // IVORY  / GRAY
    2: { bright: '#eeee00', dark: '#daa520' }, // YELLOW / dark (goldenrod)
    3: { bright: '#ee0000', dark: '#8b0000' }, // RED    / dark red
    4: { bright: '#0000cd', dark: '#00008b' }, // BLUE   / dark blue
    5: { bright: '#ee9a00', dark: '#da7600' }, // ORANGE / dark orange
    6: { bright: '#32cd32', dark: '#228b22' }, // GREEN  / forest green
    7: { bright: '#009acd', dark: '#436eee' }, // CYAN   (a deep blue!) / variant
    8: { bright: '#a020f0', dark: '#68228b' }, // PURPLE / dark purple
    9: { bright: '#bfbfbf', dark: '#bfbfbf' }, // NEUTRAL
    20: { bright: '#bfbfbf', dark: '#bfbfbf' }, // BOTTLE-NECK STRUCT
    23: { bright: '#ff00ff', dark: '#800080' }, // GIMP (placeholder; orig is an image)
};

const CELL_SIZE = 23;
const BEVEL_BORDER = 3;
const ARSENAL_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

// ─── Online helpers ───────────────────────────────────────────────────────────

function setOnlineStatus(msg) {
    onlineStatus.textContent = msg;
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
            // Opponent died — we win
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
    setOnlineStatus(`Connected — fight!  (vs ${oppLabel})`);
}

async function beginOnlineMode() {
    // Ask for player name
    const defaultName = 'player' + Math.floor(Math.random() * 900 + 100);
    const playerName = prompt('Enter your player name:', defaultName) || defaultName;

    // Clean up any previous online session
    cleanupOnline();

    // Start a paused WasmGame (seed from time + random)
    const seed = (performance.now() | 0) ^ (Math.floor(Math.random() * 1e9));
    game = new WasmGame(seed);
    onlinePaused = true;

    // Resize canvases
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
    setOnlineStatus('Matchmaking…');

    // Open WebSocket to the signaling server on the same host that served this
    // page (so it works over localhost, LAN, or Tailscale). Use wss when the
    // page itself is served over https.
    // Same-origin WebSocket: the bt-server serves both the page and /ws on one
    // port (and wss behind TLS, e.g. on fly.io).
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    const wsUrl = `${wsProto}://${location.host}/ws`;
    ws = new WebSocket(wsUrl);

    ws.onopen = () => {
        ws.send(JSON.stringify({ type: 'queue', name: playerName }));
    };

    ws.onerror = (err) => {
        console.warn('WebSocket error', err);
        setOnlineStatus(`Connection error — is bt-server running on ${wsUrl}?`);
    };

    ws.onclose = () => {
        if (!gameEnded && mode === 'online') {
            setOnlineStatus('Disconnected from server.');
        }
    };

    ws.onmessage = async (ev) => {
        const msg = JSON.parse(ev.data);

        if (msg.type === 'matched') {
            onlineOpponentName = msg.opponent || 'Opponent';
            const conservative = (msg.oppMu - 3 * msg.oppSigma).toFixed(1);
            const quality = msg.quality != null ? Math.round(msg.quality * 100) : '?';
            setOnlineStatus(
                `vs ${onlineOpponentName} (rating μ−3σ ≈ ${conservative})` +
                ` — match quality ${quality}% — Waiting for opponent…`
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
                // The answer SDP is sent when we receive the offer signal (see 'signal' handler below)
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
                `${result} — New rating: μ=${msg.mu.toFixed(2)}, σ=${msg.sigma.toFixed(2)},` +
                ` μ−3σ ≈ ${conservative}`
            );

        } else if (msg.type === 'opponentLeft') {
            setOnlineStatus('Opponent left.');
            if (!gameEnded) {
                gameEnded = true;
                gameOverText.textContent = 'Opponent left.';
                gameOverOverlay.style.display = 'flex';
            }
        }
    };
}

// ─── Game initialization ──────────────────────────────────────────────────────

async function initGame() {
    await init();
    startGame('practice');
}

function startGame(newMode) {
    // Clean up online resources when leaving Online mode
    if (mode === 'online' || newMode !== 'online') {
        cleanupOnline();
        onlineStatus.style.display = 'none';
    }

    mode = newMode;

    if (mode === 'online') {
        // Online mode bootstraps separately via beginOnlineMode()
        updateModeButtons();
        beginOnlineMode();
        return;
    }

    const seed = (performance.now() | 0) ^ (Math.floor(Math.random() * 1e9));

    // Create game instance based on mode
    if (mode === 'vscomputer') {
        game = new WasmVsComputer(seed);
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
    startGame(mode);
}

function drawBoard(context, grid, width, height) {
    // Clear canvas with black background
    context.fillStyle = '#000000';
    context.fillRect(0, 0, width * CELL_SIZE, height * CELL_SIZE);

    // Draw each cell
    for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
            const cellId = grid[y * width + x];
            drawCellOnContext(context, x, y, cellId);
        }
    }
}

function drawCellOnContext(context, x, y, cellId) {
    const px = x * CELL_SIZE;
    const py = y * CELL_SIZE;

    // Empty or hidden cells: draw nothing (black background)
    if (cellId <= 0) {
        return;
    }

    // Beveled colored boxes (1-8)
    if (cellId >= 1 && cellId <= 8) {
        const colors = PALETTE[cellId];
        // Dark shadow on bottom-right
        context.fillStyle = colors.dark;
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        // Bright inset on top-left
        context.fillStyle = colors.bright;
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        return;
    }

    // NEUTRAL / BOTTLE-NECK (9 or 20)
    if (cellId === 9 || cellId === 20) {
        context.fillStyle = '#bebebe';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        return;
    }

    // GIMP (23): draw image if loaded, else magenta bevel placeholder
    if (cellId === 23) {
        if (gimpImg.complete && gimpImg.naturalWidth > 0) {
            context.drawImage(gimpImg, px, py, CELL_SIZE, CELL_SIZE);
        } else {
            context.fillStyle = '#800080';
            context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
            context.fillStyle = '#ff00ff';
            context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
        }
        return;
    }

    // HAPPY (21) and UNHAPPY (22)
    if (cellId === 21 || cellId === 22) {
        // Beveled yellow box (goldenrod shadow, yellow face) — as BTBox.C.
        context.fillStyle = '#daa520';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        context.fillStyle = '#eeee00';
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw face
        context.fillStyle = '#000000';

        // Eyes: two ellipses
        const eyeWidth = 4;
        const eyeHeight = 7;
        const eyeY = py + 5;

        // Left eye
        context.beginPath();
        context.ellipse(px + 4, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        context.fill();

        // Right eye
        context.beginPath();
        context.ellipse(px + 13, eyeY, eyeWidth / 2, eyeHeight / 2, 0, 0, Math.PI * 2);
        context.fill();

        // Mouth
        if (cellId === 21) {
            // Happy: smile (lower half of arc)
            context.beginPath();
            context.arc(px + 11.5, py + 12, 5, 0, Math.PI);
            context.stroke();
        } else {
            // Unhappy: frown (upper half of arc)
            context.beginPath();
            context.arc(px + 11.5, py + 12, 5, Math.PI, 0);
            context.stroke();

            // Tear: small blue dot below right eye
            context.fillStyle = '#3050ff';
            context.beginPath();
            context.arc(px + 13, py + 8, 2, 0, Math.PI * 2);
            context.fill();
        }
        return;
    }

    // DICE (24-29)
    if (cellId >= 24 && cellId <= 29) {
        // Beveled ivory box (gray shadow, ivory face) — as BTBox.C die boxes.
        context.fillStyle = '#a8a8a8';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        context.fillStyle = '#eeeee0';
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);

        // Draw pips: a 5x5 gray square with a 3x3 black inset (BTBox.C).
        const value = cellId - 23;
        const X = [1, 7, 13];
        const Y = [1, 7, 13];
        const pipSize = 5;

        const drawPip = (offsetX, offsetY) => {
            context.fillStyle = '#a8a8a8';
            context.fillRect(px + offsetX, py + offsetY, pipSize, pipSize);
            context.fillStyle = '#000000';
            context.fillRect(px + offsetX + 1, py + offsetY + 1, pipSize - 2, pipSize - 2);
        };

        // Pip placement rules
        if (value > 1) {
            drawPip(X[0], Y[0]); // TL
            drawPip(X[2], Y[2]); // BR
        }
        if (value > 3) {
            drawPip(X[2], Y[0]); // TR
            drawPip(X[0], Y[2]); // BL
        }
        if (value % 2 === 1) {
            drawPip(X[1], Y[1]); // Center
        }
        if (value === 6) {
            drawPip(X[0], Y[1]); // ML
            drawPip(X[2], Y[1]); // MR
        }
        return;
    }
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
    opponentScore.textContent = opScore >= 0 ? opScore : '—';
    opponentLines.textContent = opLines >= 0 ? opLines : '—';

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
            mslot.textContent = `${key}\n—`;
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
            gameOverText.textContent = 'GAME OVER — You Lost';
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

    const dt = Math.min(now - lastFrameTime, 100);
    lastFrameTime = now;

    // In online mode, don't tick until the data channel is open
    const shouldTick = !paused && !gameEnded && !game.is_game_over() &&
        !(mode === 'online' && onlinePaused);

    // Advance game if not paused and not in bazaar
    if (shouldTick) {
        game.tick(dt);
    }

    // Process events and relay to opponent (vsplayer or online)
    if (!onlinePaused || mode !== 'online') {
        processEvents();
    }

    // Check for win/loss in vscomputer mode
    if (mode === 'vscomputer' && !gameEnded && game.is_game_over() === false) {
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

    if (!game || gameEnded || game.is_game_over()) return;
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

    // Tap → rotate (small movement, short time, no drop)
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
        if (!game || gameEnded || game.is_game_over()) return;
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

// Initialize and start game loop
(async () => {
    await initGame();

    // Set up broadcast channel for two-player communication
    broadcastChannel = new BroadcastChannel('battletris');
    broadcastChannel.onmessage = handleBroadcastMessage;

    // Wire up on-screen touch control buttons
    setupTouchControls();

    requestAnimationFrame(gameLoop);
})();
