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
const bazaarList = document.getElementById('bazaarList');
const bazaarFunds = document.getElementById('bazaarFunds');
const bazaarDoneBtn = document.getElementById('bazaarDoneBtn');
const arsenalList = document.getElementById('arsenalList');
const opponentScore = document.getElementById('opponentScore');
const opponentLines = document.getElementById('opponentLines');
const modePracticeBtn = document.getElementById('modePractice');
const modeVsComputerBtn = document.getElementById('modeVsComputer');
const modeVsPlayerBtn = document.getElementById('modeVsPlayer');
const modeOnlineBtn = document.getElementById('modeOnline');
const aiBoard = document.getElementById('aiBoard');
const aiLabel = document.getElementById('aiLabel');
const onlineStatus = document.getElementById('onlineStatus');

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
    // Update the opponent panel heading
    const opponentLabel = document.querySelector('.opponent-panel h3');
    if (opponentLabel) opponentLabel.textContent = oppLabel;
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
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    const wsHost = location.hostname || '127.0.0.1';
    const wsUrl = `${wsProto}://${wsHost}:9000`;
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

    // GIMP (23)
    if (cellId === 23) {
        context.fillStyle = '#800080';
        context.fillRect(px, py, CELL_SIZE, CELL_SIZE);
        context.fillStyle = '#ff00ff';
        context.fillRect(px, py, CELL_SIZE - BEVEL_BORDER, CELL_SIZE - BEVEL_BORDER);
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
    scoreValue.textContent = game.score();
    linesValue.textContent = game.lines();
    fundsValue.textContent = game.funds();
    linesToBazaarValue.textContent = game.lines_til_bazaar();
}

function updateOpponentPanel() {
    const opScore = game.op_score();
    const opLines = game.op_lines();
    opponentScore.textContent = opScore >= 0 ? opScore : '—';
    opponentLines.textContent = opLines >= 0 ? opLines : '—';

    // Update opponent label based on mode
    const opponentLabel = document.querySelector('.opponent-panel h3');
    if (mode === 'vscomputer') {
        opponentLabel.textContent = 'Ernie (computer)';
    } else if (mode === 'online' && onlineOpponentName) {
        opponentLabel.textContent = onlineOpponentName;
    } else if (mode !== 'online') {
        opponentLabel.textContent = 'Opponent';
    }
    // In online mode before match: keep whatever text is already there
}

function updateArsenalPanel() {
    arsenalList.innerHTML = '';
    for (let i = 0; i < 10; i++) {
        const token = game.arsenal_token(i);
        const key = ARSENAL_KEYS[i];
        const div = document.createElement('div');
        div.className = 'arsenal-item';

        if (token >= 0) {
            const name = weapon_name(token);
            const qty = game.arsenal_quantity(i);
            div.textContent = `${key}. ${name} (x${qty})`;
        } else {
            div.textContent = `${key}. < Empty >`;
        }
        arsenalList.appendChild(div);
    }
}

function populateBazaar() {
    bazaarList.innerHTML = '';
    const maxWeapons = max_weapons();

    for (let t = 0; t < maxWeapons; t++) {
        const name = weapon_name(t);
        const desc = weapon_description(t);
        // Effective price reflects Carter Years doubling (matches the charge).
        const price = game.bazaar_price(t);
        const duration = weapon_duration(t);

        const row = document.createElement('div');
        row.className = 'bazaar-item';
        row.title = desc;

        const html = `
            <div class="bazaar-item-name">${name}</div>
            <div>$${price} <span class="bazaar-item-duration">(${duration} lines)</span></div>
        `;
        row.innerHTML = html;

        row.addEventListener('click', () => {
            if (game.buy_weapon(t)) {
                updateStats();
                updateArsenalPanel();
                bazaarFunds.textContent = game.funds();
            }
        });

        bazaarList.appendChild(row);
    }

    bazaarFunds.textContent = game.funds();
}

function updateBazaarOverlay() {
    if (game.is_in_bazaar()) {
        bazaarOverlay.style.display = 'flex';
        populateBazaar();
    } else {
        bazaarOverlay.style.display = 'none';
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

    requestAnimationFrame(gameLoop);
})();
