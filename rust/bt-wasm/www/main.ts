import init, { WasmGame, WasmVsComputer, WasmClient, fixed_dt, max_weapons, weapon_name, weapon_price, weapon_description, weapon_duration, spy_hide_pct, spy_revealed_funds } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';
import { Sound } from './sound.js';
import type { ServerMessage, ClientMessage, PlayerInfo, SideStatus, OppStatus, PlayerStats, ReplayMeta } from './protocol.js';
import { escapeHtml } from './dom-util.js';
import { nextGag, initialGagState, type GagState } from './update-gag.js';
import { showMotifDialog } from './motif-dialog.js';
import { rollSpyMask, applySpyMask } from './spy-degrade.js';

// The `game` variable holds one of three wasm classes or null.
type AnyGame = WasmGame | WasmVsComputer | WasmClient;

// Game state
let game: AnyGame | null = null;
let mode = 'practice'; // 'practice', 'vscomputer', 'online' (online = server-authoritative)
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

// Online (server-authoritative matchmaking + match) state
let ws: WebSocket | null = null;       // WebSocket: matchmaking handoff + the authoritative match
let onlinePaused = true;  // true until a match starts
let onlineOpponentName = '';
let searching = false;    // background matchmaking in progress (queued, not yet matched)

// Server-authoritative online play (the client-server migration). In an
// authoritative match the server runs the real simulation; the client predicts
// locally for a 0-latency feel, sends each input to the server, and reconciles
// against the server's keyframes. The prediction/reconciliation core — the input
// seq, the unacked-input queue, and the keyframe-restore-then-replay — lives in
// the shared `WasmClient` (bt-netcode's Predictor), the SAME code the bot runs, so
// `game` is a `WasmClient` during an online match (and a `WasmGame` /
// `WasmVsComputer` otherwise; all three expose the same read API for rendering).
let authoritative = false; // true during a server-authoritative online match
let currentMatchId: string | null = null; // the live bout's id (`match-<uuid>`); parked in the URL for rejoin-on-refresh
let currentSeed: number | null = null;    // the game's RNG seed (shown in the debug overlay)
let authSelf: SideStatus | null = null;       // latest authoritative own-status {funds,in_bazaar,lines_til_bazaar}
let authOpp: OppStatus | null = null;        // latest authoritative opponent view {score,lines,game_over,in_bazaar?}
let authSpying = false;    // is a spy of ours active (server-authorized)?
let authSpyBoard: Int32Array | null = null;   // latest opponent board (FULL, from a keyframe), or null; degraded client-side per frame
let authSpyHide = 0;       // percent of spy_board cells to hide each frame (per-spy accuracy: Ames 50, Ace 15, Condor 0)
let authSpyFunds: number | null = null;       // latest server-computed opponent funds our spy reveals, or null
// The per-frame spy static: a hide mask over the spy board, re-rolled on a timer
// so `authSpyHide`% of the opponent's cells blink out and back, reproducing the
// original's imperfect spy reveal. Reset whenever the spy board clears.
let spyFlickerMask: Uint8Array | null = null;
let spyFlickerRolledAt = 0;
const SPY_FLICKER_MS = 70; // ~14 Hz re-roll: clearly staticky without a strobe

// Degrade the FULL spy board to the spy's accuracy for display: blank `hide`% of
// the opponent's filled cells, re-rolling which cells are hidden every
// SPY_FLICKER_MS so the reveal shimmers like the original's imperfect spy feed.
// hide == 0 (Condor) returns the board untouched. The mask is held across frames and
// only re-rolled on the timer; the pure mask roll/apply live in spy-degrade.ts.
function spyFlicker(board: Int32Array, hide: number): Int32Array {
    if (hide <= 0) return board;
    const now = performance.now();
    const stale = spyFlickerMask === null
        || spyFlickerMask.length !== board.length
        || now - spyFlickerRolledAt >= SPY_FLICKER_MS;
    if (stale) {
        spyFlickerMask = rollSpyMask(board.length, hide, Math.random);
        spyFlickerRolledAt = now;
    }
    return applySpyMask(board, spyFlickerMask!);
}

// ── Mock match (test mode) ─────────────────────────────────────────────────────
// A local two-board match (you vs Ernie, via WasmVsComputer) rendered the way an
// online match is: the opponent's board is hidden except through an active spy
// (flickered to the spy's accuracy), funds are hidden except through a spy, and
// score/lines are always shown. No WebSocket, no matchmaking, and it never ends on
// you mid-test. Enabled with `?mock=1`; the spy reveal is driven entirely by
// `mockSyncSpy` below using the same `bt_core::spy` math the server uses.
let mockMatch = false;
// The spy the player currently holds in the mock, with a line budget that counts
// down as Ernie clears lines (mirrors the server's per-spy duration).
let mockSpy: { token: number; budget: number } | null = null;
let mockTick = 0;                 // a frame counter feeding the funds noise
let mockPrevOppLines = 0;         // last seen Ernie line count, for budget + tetris
let mockTetrisAtTick = -100000;   // tick of Ernie's last tetris (drives Ace's window)
const MOCK_ACE_WINDOW = 60;       // ticks Ace keeps perturbing after a tetris

// Called from `predict` when the player launches a weapon in a mock match: if the
// fired slot holds a spy, start tracking it so the reveal turns on.
function mockNoteLaunch(slot: number) {
    const g = game as WasmVsComputer;
    const token = g.arsenal_token(slot);
    if (token === 17 || token === 18 || token === 19) { // Ames / Ace / Condor
        mockSpy = { token, budget: weapon_duration(token) };
        mockPrevOppLines = g.op_lines();
        spyFlickerMask = null;
    }
}

// Each frame in a mock match: age the active spy by Ernie's line clears, note his
// tetrises, and set the authoritative spy fields (board / hide level / funds) the
// render path reads, exactly as `applySnapshot` would online.
function mockSyncSpy() {
    const g = game as WasmVsComputer;
    mockTick += 1;
    const oppLines = g.op_lines();
    if (mockSpy) {
        const delta = oppLines - mockPrevOppLines;
        if (delta >= 4) mockTetrisAtTick = mockTick; // a tetris: Ace perturbs briefly
        if (delta > 0) mockSpy.budget -= delta;
        if (mockSpy.budget <= 0) mockSpy = null;      // spy expired
    }
    mockPrevOppLines = oppLines;

    authSpying = mockSpy !== null;
    if (mockSpy) {
        authSpyBoard = Int32Array.from(g.render_ai_grid());
        authSpyHide = spy_hide_pct(mockSpy.token);
        const aceRecent = mockTick - mockTetrisAtTick < MOCK_ACE_WINDOW;
        authSpyFunds = spy_revealed_funds(g.ai_funds(), mockSpy.token, mockTick, aceRecent);
    } else {
        authSpyBoard = null;
        authSpyFunds = null;
        spyFlickerMask = null;
    }
}

let playerName: string | null = null;    // remembered after the first prompt

// Canvas and context
const canvas = document.getElementById('gameCanvas') as HTMLCanvasElement;
const ctx = canvas.getContext('2d')!;
const aiGridCanvas = document.getElementById('aiGridCanvas') as HTMLCanvasElement;
const aiCtx = aiGridCanvas.getContext('2d')!;

// UI elements
const scoreValue = document.getElementById('scoreValue') as HTMLElement;
const linesValue = document.getElementById('linesValue') as HTMLElement;
const fundsValue = document.getElementById('fundsValue') as HTMLElement;
const linesToBazaarValue = document.getElementById('linesToBazaarValue') as HTMLElement;
const gameOverOverlay = document.getElementById('gameOverOverlay') as HTMLElement;
const gameOverText = document.getElementById('gameOverText') as HTMLElement;
const watchReplayBtn = document.getElementById('watchReplayBtn') as HTMLElement | null;
// The just-finished online match's stored replay id (sent by the server at
// match end) — drives the game-over "Watch replay" button.
let lastMatchReplayId: string | null = null;
const newGameBtn = document.getElementById('newGameBtn') as HTMLElement;
const bazaarOverlay = document.getElementById('bazaarOverlay') as HTMLElement;
const bazaarFunds = document.getElementById('bazaarFunds') as HTMLElement;
const bazaarDoneBtn = document.getElementById('bazaarDoneBtn') as HTMLButtonElement;
const bazaarBarrierStatus = document.getElementById('bazaarBarrierStatus') as HTMLElement | null;
const bazaarAddBtn = document.getElementById('bazaarAddBtn') as HTMLElement;
const bazaarRemoveBtn = document.getElementById('bazaarRemoveBtn') as HTMLElement;
const bazaarWeaponList = document.getElementById('bazaarWeaponList') as HTMLElement;
const bazaarArsenalList = document.getElementById('bazaarArsenalList') as HTMLElement;
const bazaarInfoPrice = document.getElementById('bazaarInfoPrice') as HTMLElement;
const bazaarInfoDuration = document.getElementById('bazaarInfoDuration') as HTMLElement;
const bazaarInfoDesc = document.getElementById('bazaarInfoDesc') as HTMLElement;
const arsenalList = document.getElementById('arsenalList') as HTMLElement;
const opponentScore = document.getElementById('opponentScore') as HTMLElement;
const opponentLines = document.getElementById('opponentLines') as HTMLElement;
const opponentFundsRow = document.getElementById('opponentFundsRow') as HTMLElement;
const opponentFunds = document.getElementById('opponentFunds') as HTMLElement;
const modePracticeBtn = document.getElementById('modePractice') as HTMLElement;
const playComputerBtn = document.getElementById('playComputerBtn') as HTMLElement;
const findMatchBtn = document.getElementById('findMatchBtn') as HTMLElement;
const aiBoard = document.getElementById('aiBoard') as HTMLElement;
const aiBoardLabel = document.getElementById('aiBoardLabel') as HTMLElement;
const gameAreaEl = document.querySelector('.game-area') as HTMLElement | null;
const aiLabel = document.getElementById('aiLabel') as HTMLElement;
const onlineStatus = document.getElementById('onlineStatus') as HTMLElement;

// Two-screen views (lobby <-> playfield) — like the original window swapping the
// BTChallenge and BTGame forms (BTStartup.C). Only one is visible at a time.
const lobbyScreen = document.getElementById('lobbyScreen') as HTMLElement;
const gameScreen = document.getElementById('gameScreen') as HTMLElement;
const onlineListEl = document.getElementById('onlineList') as HTMLElement | null;
const challengeBtn = document.getElementById('challengeBtn') as HTMLButtonElement | null;
const updateBtn = document.getElementById('updateBtn') as HTMLElement | null;
const availableToggle = document.getElementById('availableToggle') as HTMLInputElement | null;
const availableToggleGame = document.getElementById('availableToggleGame') as HTMLInputElement | null;
const statsPanelEl = document.getElementById('statsPanel') as HTMLElement | null;
const playingStatusEl = document.getElementById('playingStatus') as HTMLElement | null;
const ernieSlider = document.getElementById('ernieSlider') as HTMLInputElement | null;
const nameInput = document.getElementById('nameInput') as HTMLInputElement | null;
const nameHint = document.getElementById('nameHint') as HTMLElement | null;

// Ernie difficulty names (the original slider's labels), index = level 0..14.
const ERNIE_NAMES = ['Comatose', 'Somnambulant', 'Lethargic', 'Pensive', 'Able',
    'Willing', 'Focused', 'Lively', 'Energetic', 'Pepped-up', 'Caffeinated',
    'Bug-eyed', 'Supercharged', 'Hell-Bent', 'Bionic'];

// lobbyActive gates the game loop: while the lobby is showing there is no game to
// tick or render. Starts true (the app opens on the lobby, like BTStartup).
let lobbyActive = true;

function showGame() {
    lobbyActive = false;
    lobbyScreen.style.display = 'none';
    gameScreen.style.display = '';
    // The mobile touch-control bar is only useful in a game (CSS gates on this).
    document.body.classList.add('in-game');
    applyBoardScale();
}

function showLobby() {
    lobbyActive = true;
    document.body.classList.remove('in-game');
    gameScreen.style.display = 'none';
    lobbyScreen.style.display = '';
}

// ─── Rejoin-on-refresh: park the live match in the URL ───────────────────────
// While an authoritative match is live the URL carries ?match=<id>, so an
// accidental browser refresh reconnects straight back into it (the server froze
// the bout for a grace window and reattaches us). Cleared the moment the match is
// no longer live, so a later refresh lands cleanly in the lobby.
function setMatchUrl(id: string | null | undefined) {
    if (id === undefined || id === null) return;
    currentMatchId = id;
    try { history.replaceState(null, '', '?match=' + encodeURIComponent(id)); } catch (_) {}
}
function clearMatchUrl() {
    currentMatchId = null;
    try { history.replaceState(null, '', location.pathname); } catch (_) {}
}

// The reconnect overlay ("Reconnecting…" for us / "Opponent reconnecting…" for the
// other side) — shown while the server has the bout frozen during a grace window.
const reconnectOverlay = document.getElementById('reconnectOverlay') as HTMLElement | null;
const reconnectText = document.getElementById('reconnectText') as HTMLElement | null;
let reconnectTimer: number | null = null; // the live forfeit countdown (connected side)
function stopReconnectCountdown() {
    if (reconnectTimer) { clearInterval(reconnectTimer); reconnectTimer = null; }
}
function showReconnect(text: string) {
    if (reconnectText) reconnectText.textContent = text;
    if (reconnectOverlay) reconnectOverlay.style.display = 'flex';
}
function hideReconnect() {
    stopReconnectCountdown();
    if (reconnectOverlay) reconnectOverlay.style.display = 'none';
}
// Tick a visible countdown to the forfeit while the opponent reconnects. The
// server still decides the actual forfeit (it sends opponentLeft at expiry); this
// just shows how long is left so the wait doesn't feel open-ended.
function startReconnectCountdown(secs: number) {
    stopReconnectCountdown();
    let remaining = Math.max(0, secs | 0);
    const render = () => showReconnect(`Opponent reconnecting…\n${remaining}s to forfeit`);
    render();
    reconnectTimer = setInterval(() => {
        remaining = Math.max(0, remaining - 1);
        render();
        if (remaining === 0) stopReconnectCountdown(); // hold at 0 until the server ends it
    }, 1000);
}

// The Play Computer button reads "Play <Level> Ernie" off the difficulty slider
// (BTChallenge.C: "Play %s Ernie").
function updatePlayComputerLabel() {
    if (!playComputerBtn || !ernieSlider) return;
    const lvl = parseInt(ernieSlider.value, 10) || 0;
    playComputerBtn.textContent = `Play ${ERNIE_NAMES[lvl] || lvl} Ernie`;
}

// "Playing X" status line under the score box (BTGame shows the opponent name).
function setPlayingStatus() {
    if (!playingStatusEl) return;
    if (mode === 'vscomputer') {
        const lvl = ernieSlider ? (parseInt(ernieSlider.value, 10) || 0) : 5;
        playingStatusEl.textContent = `Playing Ernie (${ERNIE_NAMES[lvl] || lvl})`;
    } else if (mode === 'online' && onlineOpponentName) {
        playingStatusEl.textContent = `Playing ${onlineOpponentName}`;
    } else {
        playingStatusEl.textContent = 'Practice';
    }
}

// ─── Lobby presence + challenge (server wiring lands in Phases 3-4) ──────────
// Selected player in the online list (null = none). Drives the Challenge button
// and the stats panel.
let selectedPlayer: string | null = null;
let lobbyPlayers: PlayerInfo[] = [];

// Render the online players list. Each row selects the player (loads their stats
// + enables Challenge). Populated from the server's `players` push; empty until
// presence tracking lands.
function renderOnlineList(players: PlayerInfo[]) {
    lobbyPlayers = players || [];
    if (!onlineListEl) return;
    if (!lobbyPlayers.length) {
        onlineListEl.innerHTML = '<div class="lobby-list-empty">No one\'s around. Go "Open to matches" and wait, or Play Computer.</div>';
        return;
    }
    onlineListEl.innerHTML = '';
    for (const p of lobbyPlayers) {
        const row = document.createElement('div');
        row.className = 'lobby-list-row' + (p.name === selectedPlayer ? ' selected' : '');
        // Round-trip latency (ms) to the server, measured via ws Ping/Pong — shown so
        // a visitor can gauge how laggy a match against this player would be. Absent
        // until the first Pong returns.
        const ping = (typeof p.ping === 'number') ? `<span class="ll-ping">${p.ping} ms</span>` : '';
        row.innerHTML = `<span class="ll-name">${escapeHtml(p.name)}</span>${ping}<span class="ll-status">${escapeHtml(p.status || '')}</span>`;
        row.addEventListener('click', () => selectPlayer(p.name));
        onlineListEl.appendChild(row);
    }
}

function selectPlayer(name: string) {
    selectedPlayer = name;
    if (challengeBtn) challengeBtn.disabled = !name;
    renderOnlineList(lobbyPlayers);
    loadPlayerStats(name);
}

// The `players` roster is PUSHED live over the websocket on every change (see the
// `players` handler in onSignalMessage), so the UPDATE button has no work to do — the
// 1994 client needed it because it PULLED the roster; we don't. So instead it does a
// bit. The escalating gag, the gag pool, and the (pure) sequencer live in
// update-gag.ts (unit-tested in update-gag.test.ts); here we just hold the state,
// feed it the clock + Math.random, show the gag in a faithful Motif OK dialog
// (BTMessageDlog), and persist the achievement. The dialog body is the seam for the
// planned DDR-style UPDATE minigame (swap the string for a widget).
function updateAchUnlocked(): boolean {
    try { return localStorage.getItem('bt_ach_update') === '1'; } catch (_) { return false; }
}
let gagState: GagState = initialGagState(updateAchUnlocked());
function showUpdateGag(msg: string) { void showMotifDialog(msg, { title: 'UPDATE' }); }
function pressUpdate() {
    const r = nextGag(gagState, { now: Date.now(), rng: Math.random });
    gagState = r.state;
    if (r.unlockedAchievement) {
        try { localStorage.setItem('bt_ach_update', '1'); } catch (_) {}
    }
    showUpdateGag(r.text);
}

// Challenge the selected player (directed). Needs a signed identity first.
// Shown when you try to challenge yourself (you appear in your own roster while
// "Open to matches"). A list so it's easy to add more later; one for now.
const SELF_CHALLENGE_GAGS = [
    "There's only one of you, and that one is busy reading this.",
];

async function challengeSelected() {
    if (!selectedPlayer) return;
    if (selectedPlayer === playerName) {
        showToast(SELF_CHALLENGE_GAGS[Math.floor(Math.random() * SELF_CHALLENGE_GAGS.length)] ?? '', 4500);
        return;
    }
    if (!await ensureIdentity()) return; // need a name/token first (field is focused)
    if (ws && ws.readyState === WebSocket.OPEN) {
        sendMsg({ type: 'challenge', target: selectedPlayer,
            ...(playerName != null && { name: playerName }),
            ...(identityToken != null && { token: identityToken }),
        });
        setOnlineStatus(`Challenging ${selectedPlayer}…`);
        if (onlineStatus) onlineStatus.style.display = 'block';
    }
}

// "Open to matches" lives in two places — the lobby and the in-game top bar —
// so keep both checkboxes showing the same state.
function syncAvailableUI(v: boolean) {
    if (availableToggle) availableToggle.checked = v;
    if (availableToggleGame) availableToggleGame.checked = v;
}

// Open-to-matches: become challengeable AND eligible for auto-pairing.
async function setAvailable(v: boolean) {
    if (v && !await ensureIdentity()) {
        // Can't be challengeable without a signed identity — revert the toggle and
        // leave the name field focused so the player can fix it.
        syncAvailableUI(false);
        return;
    }
    sendMsg({ type: 'available', value: v,
        ...(playerName != null && { name: playerName }),
        ...(identityToken != null && { token: identityToken }),
    });
    syncAvailableUI(v);
}

// An incoming challenge invite (server -> us).
let pendingChallenger: string | null = null;
const challengeOverlay = document.getElementById('challengeOverlay') as HTMLElement | null;
function onChallenged(from: string) {
    pendingChallenger = from;
    const t = document.getElementById('challengeText');
    if (t) t.textContent = `${from} challenges you!`;
    if (challengeOverlay) challengeOverlay.classList.add('open');
}
function respondChallenge(accept: boolean) {
    if (challengeOverlay) challengeOverlay.classList.remove('open');
    if (!pendingChallenger) return;
    const from = pendingChallenger;
    pendingChallenger = null;
    if (accept) { sendMsg({ type: 'challengeAccept', from }); }
    else { sendMsg({ type: 'challengeDecline', from }); }
    if (accept) { setOnlineStatus(`Accepting ${from}…`); if (onlineStatus) onlineStatus.style.display = 'block'; }
}
async function loadPlayerStats(name: string) {
    if (!statsPanelEl || !name) return;
    statsPanelEl.textContent = 'Loading ' + name + '…';
    try {
        const res = await fetch('/api/player/' + encodeURIComponent(name));
        if (!res.ok) throw new Error(String(res.status));
        statsPanelEl.textContent = formatPlayerStats(await res.json());
    } catch (_) {
        // /api/player lands in Phase 4; until then show what we know.
        statsPanelEl.textContent = `      Name: ${name}\n      Rank: —\n      Wins: —\n    Losses: —`;
    }
}

// Render the stats panel in the original's right-aligned-label monospace style
// (BTPlayer::formatInfo).
function formatPlayerStats(p: PlayerStats) {
    const row = (label: string, val: unknown) => label.padStart(14) + ': ' + val;
    return [
        row('Name', p.name ?? '—'),
        row('Rank', p.elo ?? '—'),
        row('Wins', p.wins ?? '—'),
        row('Losses', p.losses ?? '—'),
        row('Highest score', p.high_score ?? '—'),
        row('Highest lines', p.high_lines ?? '—'),
        row('Highest funds', p.high_funds ?? '—'),
        row('Streak', (p.streak ?? '—') + (p.streak_type ? ' ' + p.streak_type : '')),
        row('Fastest kill', p.fastest_kill ?? 'None'),
        row('Quickest death', p.quickest_death ?? 'None'),
        row('Longest game', p.longest_game ?? 'None'),
        '',
        'Nickname: ' + (p.name ?? 'none'),
    ].join('\n');
}


// ─── Board display scale ──────────────────────────────────────────────────
// On the playfield the board dominates the window height — in the original the
// board drawing area fills the 670x700 window (BTGame.C). We scale the native
// 230x644 well UP to fill the available viewport height, capped so it never gets
// absurd on huge displays. Recomputed on resize. (The lobby owns the page chrome
// and the visitor counter now, so the board no longer has to share that space.)
const BOARD_MAX_SCALE = 2.2;        // generous cap for big displays
const BOARD_MIN_SCALE = 0.6;        // keep it playable on short viewports
const BOARD_MARGIN_Y = 28;          // breathing room above + below the board
const AI_SCALE_RATIO = 0.55;        // vs-Computer side board, a smaller secondary view

// Scale that makes the board fill the height that's ACTUALLY available below it —
// i.e. from the board's own top (everything above it: the game top bar, the mobile
// stats strip) down to the viewport bottom, minus a margin + the wrapper's chrome.
// Using only innerHeight (the old behaviour) ignored the top bar, so the board
// scaled too tall and its bottom rows were clipped off-screen. We measure the
// board-wrapper's live top when the game screen is visible, and fall back to an
// estimate of the top chrome when it's momentarily hidden (rect top ≈ 0).
const ESTIMATED_TOP_CHROME = 64;       // ~game top bar height, for the hidden-screen fallback
function boardDisplayScale(bufHeightPx: number) {
    const wrapChromeY = 26;            // wrapper padding (10*2) + border (3*2)
    const wrapper = canvas.parentElement;
    const top = wrapper ? wrapper.getBoundingClientRect().top : 0;
    const usableTop = top > 1 ? top : ESTIMATED_TOP_CHROME;
    const avail = window.innerHeight - usableTop - BOARD_MARGIN_Y - wrapChromeY;
    const scale = avail / bufHeightPx;
    return Math.max(BOARD_MIN_SCALE, Math.min(BOARD_MAX_SCALE, scale));
}

// Apply the fitted scale to the board (and the vs-Computer side board). Touches
// only CSS size, not the backing buffer, so it's cheap to call on every resize.
function applyBoardScale() {
    if (!game) return;
    const width = game.width();
    const height = game.height();
    const scale = boardDisplayScale(height * CELL_SIZE);
    canvas.style.width = (width * CELL_SIZE * scale) + 'px';
    canvas.style.height = (height * CELL_SIZE * scale) + 'px';
    if (aiBoard.style.display !== 'none') {
        const aiScale = scale * AI_SCALE_RATIO;
        aiGridCanvas.style.width = (width * CELL_SIZE * aiScale) + 'px';
        aiGridCanvas.style.height = (height * CELL_SIZE * aiScale) + 'px';
    }
}

window.addEventListener('resize', applyBoardScale);

const cancelSearchBtn = document.getElementById('cancelSearch') as HTMLElement;
const playersCountEl = document.getElementById('playersCount') as HTMLElement | null;
const hitCounterEl = document.getElementById('hitCounter') as HTMLElement | null;
// Default Ernie difficulty: "Willing" (index 5 -> 1000ms/move). The original
// defaults to the slider minimum (Comatose); 1000ms is a fairer modern default.
const DEFAULT_ERNIE_LEVEL = 5;

// Mobile UI elements
const mobileScore = document.getElementById('mobileScore') as HTMLElement;
const mobileLines = document.getElementById('mobileLines') as HTMLElement;
const mobileFunds = document.getElementById('mobileFunds') as HTMLElement;
const mobileLinesToBazaar = document.getElementById('mobileLinesToBazaar') as HTMLElement;
const mobileOpponent = document.getElementById('mobileOpponent') as HTMLElement;
const mobileArsenalList = document.getElementById('mobileArsenalList') as HTMLElement;

const ARSENAL_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'];

// Keys that count as a "gameplay button" for the players-online activity ping.
const GAMEPLAY_KEYS = new Set([
    'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', ' ', 'Spacebar', 'p', 'P',
    'w', 'a', 's', 'd', 'W', 'A', 'S', 'D',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
]);

// ─── Online helpers ───────────────────────────────────────────────────────────

function setOnlineStatus(msg: string) {
    onlineStatus.textContent = msg;
}

// Type-safe WebSocket send: serialises a ClientMessage to JSON and sends it.
// Only call when the socket is open; callers are responsible for the readyState
// guard (so the send sites can choose how to handle a closed socket).
function sendMsg(msg: ClientMessage): void {
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(msg));
}

// ─── Live site stats (players online + 90s hit counter) ─────────────────────────
//
// A dedicated read-only websocket, opened on page load and kept open. Sending
// "watch" counts this page as a visitor (the persistent SQLite hit counter) and
// a live player; the server then pushes {players, hits} to everyone whenever the
// numbers change. Kept separate from the matchmaking socket so it stays open the
// whole visit and never interferes with a match.
// A single persistent websocket carries everything: the lobby (watch/stats/
// players/presence/challenge) AND the authoritative match (matchStart/snapshot/
// input). One connection per client = the server's model; it also means a
// challenge accepted in the lobby flows straight into a match on the same socket.
let lobbyReconnectTimer: number | null = null;
let pendingQueue = false;   // queue for a match as soon as the socket (re)opens
let pendingRejoin: string | null = null;   // match_id to reattach to as soon as the socket (re)opens

function connectLobby() {
    // Never open a second socket on top of a live one.
    if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) return;
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    const sock = new WebSocket(`${wsProto}://${location.host}/ws`);
    ws = sock;
    sock.onopen = () => {
        if (ws !== sock) return; // superseded before it opened; ws !== sock so sendMsg is a no-op anyway
        sendMsg({ type: 'watch' });
        // Rejoin takes priority: reattach to a live bout (after a refresh or a brief
        // socket drop) before doing any lobby presence. The server reattaches us and
        // replays matchStart + a keyframe; on failure it sends rejoinFailed.
        if (pendingRejoin !== null) {
            const mid = pendingRejoin; // a tagged-UUID string (match-<uuid>) from the URL
            pendingRejoin = null;
            sendMsg({ type: 'rejoin', match_id: mid, token: identityToken ?? '', name: playerName });
            return;
        }
        // Re-assert "Open to matches" across a reconnect (else the server forgets).
        if (availableToggle && availableToggle.checked) {
            sendMsg({ type: 'available', value: true,
                ...(playerName != null && { name: playerName }),
                ...(identityToken != null && { token: identityToken }),
            });
        }
        // A Find Match requested while the socket was down (e.g. just after a
        // forfeit-leave): send the queue now that we're connected.
        if (pendingQueue) {
            pendingQueue = false;
            sendMsg({ type: 'queue', name: playerName ?? '', token: identityToken ?? '', authoritative: true });
        }
    };
    sock.onmessage = onSignalMessage;
    sock.onclose = () => {
        if (ws !== sock) return; // a newer socket already replaced this one
        // A socket drop DURING a live match is no longer an instant loss: the server
        // freezes the bout for a grace window. Queue a rejoin and show the overlay;
        // the reconnect timer below fires it. (A finished/forfeited match has
        // gameEnded set, so it falls through to the normal lobby reconnect.)
        if (authoritative && !gameEnded && currentMatchId !== null) {
            pendingRejoin = currentMatchId;
            onlinePaused = true; // stop predicting while we're detached
            stopReconnectCountdown(); // it's now US reconnecting, not the opponent
            showReconnect('Reconnecting…');
        }
        if (searching) {
            searching = false;
            findMatchBtn.classList.remove('searching');
            cancelSearchBtn.style.display = 'none';
        }
        ws = null;
        renderOnlineList([]); // clear the roster while disconnected
        clearTimeout(lobbyReconnectTimer!);
        // Reconnect fast when a rejoin is pending (get back into the frozen match
        // before the grace window expires); the usual cadence otherwise.
        lobbyReconnectTimer = setTimeout(connectLobby, pendingRejoin !== null ? 600 : 3000);
    };
    sock.onerror = () => {};
}

// ─── Identity (a signed name token, set via the always-visible name field) ───
// Your name is set through the lobby's name field (never a blocking prompt that
// could be dismissed/blocked, leaving you stuck — the bug this replaces). On a
// name set/change we mint a signed JWT from the server (cached in localStorage) so
// the name is tamper-evident while held. Online actions require a name + token.
let identityToken: string | null = null;

function loadIdentity() {
    // Persist only the NAME across sessions — NEVER the signed token. A cached bearer
    // token can go stale (e.g. it was minted under an earlier server secret); the
    // server then ignores it, and because a bare (untokened) `name` can't claim an
    // already-rated name, the player silently collapses to "anon"/unlisted. So we
    // always mint a FRESH token from /api/identity each session (it signs under the
    // current secret, so it verifies). Drop any token a prior build left behind.
    try {
        localStorage.removeItem('bt_token');
        playerName = localStorage.getItem('bt_player_name') || playerName;
    } catch (_) {}
    // Never leave the player nameless: default to a random handle they can edit.
    if (!playerName) {
        playerName = 'player' + Math.floor(Math.random() * 900 + 100);
        try { localStorage.setItem('bt_player_name', playerName); } catch (_) {}
    }
    if (nameInput) nameInput.value = playerName;
}

function setNameHint(msg: string, isError?: boolean) {
    if (!nameHint) return;
    nameHint.textContent = msg || '';
    nameHint.classList.toggle('name-hint-error', !!isError);
}

// Mint (or reuse) a signed token for the current name. Returns the token, or null
// if there's no name yet or the server call failed — callers must NOT proceed
// online on null (we surface why instead of silently dropping the player).
async function ensureIdentity() {
    if (!playerName) {
        if (nameInput) nameInput.focus();
        setNameHint('Enter a name above to play online.', true);
        return null;
    }
    if (identityToken) return identityToken;
    try {
        const res = await fetch('/api/identity', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name: playerName }),
        });
        if (res.ok) {
            // Kept in-memory only for this session; deliberately NOT persisted (see
            // loadIdentity) so a stale token can never shadow the live identity.
            identityToken = (await res.json()).token;
            setNameHint('');
        } else {
            setNameHint('Could not register that name. Try another.', true);
            return null;
        }
    } catch (_) {
        setNameHint('Network error setting your name. Try again.', true);
        return null;
    }
    return identityToken;
}

// The name field changed: adopt the new name, drop the old token (it signed the old
// name) and re-mint, then re-assert presence if we were open to matches.
async function commitNameFromField() {
    if (!nameInput) return;
    const raw = (nameInput.value || '').trim().slice(0, 20);
    if (!raw || raw === playerName) { if (raw) setNameHint(''); return; }
    playerName = raw;
    nameInput.value = raw;
    try { localStorage.setItem('bt_player_name', playerName); } catch (_) {}
    identityToken = null; // drop the old-name token; ensureIdentity re-mints for the new name
    const tok = await ensureIdentity();
    if (tok && availableToggle && availableToggle.checked) setAvailable(true);
}

function updateLiveStats(m: { players?: number; hits?: number }) {
    if (typeof m.players === 'number' && playersCountEl) {
        playersCountEl.textContent = m.players as unknown as string;
    }
    if (typeof m.hits === 'number') setHitCounter(m.hits);
}

// Render the visit total as fixed-width odometer digits (classic web-counter look).
function setHitCounter(n: number) {
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
    sendMsg({ type: 'active' });
}

// Reset match/search state. Does NOT close the socket — it's the persistent
// lobby connection. (Leaving a match resets here; the server ends the bout when
// the player tops out, the opponent wins, or the socket actually drops.)
function cleanupOnline() {
    onlinePaused = true;
    onlineOpponentName = '';
    authoritative = false;
    authSelf = null;
    authOpp = null;
    authSpying = false;
    authSpyBoard = null;
    authSpyFunds = null;
    spyFlickerMask = null;
}

// ─── Server-authoritative client (prediction + reconciliation) ────────────────

// Apply a gameplay action. In an authoritative (online) match this routes through
// the shared `WasmClient` predictor (predict locally + hand back the wire frame to
// send); the server-side seq/unacked/replay bookkeeping all lives in there. In local
// modes (practice / vs-computer) it just mutates the local engine — nothing is sent.
// Returns the buy/sell success for the bazaar UI.
// The action names accepted by predict() — exactly the cases handled in both
// switches below, so tsc checks the switches stay exhaustive and rejects a typo'd
// action string at the call sites.
type PredictKind =
    | 'MoveLeft' | 'MoveRight' | 'Rotate' | 'BeginDrop' | 'SoftDrop'
    | 'LaunchWeapon' | 'LeaveBazaar' | 'BuyWeapon' | 'SellWeapon' | 'SetPaused';

function predict(kind: PredictKind, arg?: any): boolean | void {
    if (authoritative) {
        if (gameEnded) return;
        // The Predictor gates internally (a non-shopping input under a bazaar barrier,
        // or an unaffordable buy/sell, returns "") — so there's no JS-side gate here.
        let frame = '';
        switch (kind) {
            case 'MoveLeft':     frame = (game as WasmClient).predict_move_left();  break;
            case 'MoveRight':    frame = (game as WasmClient).predict_move_right(); break;
            case 'Rotate':       frame = (game as WasmClient).predict_rotate();     break;
            case 'BeginDrop':    frame = (game as WasmClient).predict_begin_drop(); break;
            case 'SoftDrop':     frame = (game as WasmClient).predict_soft_drop();  break;
            case 'LaunchWeapon': frame = (game as WasmClient).predict_launch(arg >>> 0); break;
            case 'LeaveBazaar':  frame = (game as WasmClient).predict_leave_bazaar(); break;
            case 'BuyWeapon':    frame = (game as WasmClient).predict_buy(arg);  break;
            case 'SellWeapon':   frame = (game as WasmClient).predict_sell(arg); break;
            case 'SetPaused':    return; // local only; the server rejects pause
        }
        if (frame && ws && ws.readyState === WebSocket.OPEN) ws.send(frame);
        // For buy/sell the bazaar UI wants the accept bool; a non-empty frame ⇒ the
        // engine accepted it (and we sent it).
        if (kind === 'BuyWeapon' || kind === 'SellWeapon') return !!frame;
        return;
    }
    // ── Local modes: apply directly to the engine. The bazaar freezes play, so
    // gate out non-shopping inputs while the local bazaar is open. ──
    if ((game as WasmGame).is_in_bazaar() && kind !== 'BuyWeapon' && kind !== 'SellWeapon' && kind !== 'LeaveBazaar') {
        return;
    }
    switch (kind) {
        case 'MoveLeft':     (game as WasmGame).move_left();  break;
        case 'MoveRight':    (game as WasmGame).move_right(); break;
        case 'Rotate':       (game as WasmGame).rotate();     break;
        case 'BeginDrop':    (game as WasmGame).begin_drop(); break;
        case 'SoftDrop':     (game as WasmGame).soft_drop();  break;
        case 'LaunchWeapon':
            if (mockMatch) mockNoteLaunch(arg); // capture a spy before the slot fires
            (game as WasmGame).launch_weapon(arg);
            break;
        case 'LeaveBazaar':  (game as WasmGame).leave_bazaar(); break;
        case 'BuyWeapon':    return (game as WasmGame).buy_weapon(arg);
        case 'SellWeapon':   return (game as WasmGame).sell_weapon(arg);
        case 'SetPaused':    (game as WasmGame).set_paused(arg); break;
    }
}

// The bazaar barrier is server-authoritative online (authSelf.in_bazaar is prompt
// every frame); local modes use the engine flag. Used by the touch/key handlers to
// block gameplay gestures while the screen is frozen. (Online input gating proper
// now lives inside the Predictor; this is the UI-side read.)
function inBazaar() {
    if (authoritative && authSelf) {
        // Barrier active while EITHER side is still shopping — keep gameplay
        // inputs blocked (and the overlay up) until BOTH players have hit Done.
        return authSelf.in_bazaar || !!(authOpp && authOpp.in_bazaar);
    }
    return game!.is_in_bazaar();
}

// An authoritative snapshot from the server: update opponent/own status, reconcile
// the local prediction against it (in the shared Predictor), and latch the result.
function applySnapshot(msg: Extract<ServerMessage, { type: 'snapshot' }>) {
    authSelf = msg.you;
    authOpp = msg.opp;
    // Server-authorized spy: `spying` every frame; the FULL opponent board and the
    // hide level ride keyframes. Keep the last board while spying; drop it when it
    // ends. The board is flickered to `authSpyHide`% accuracy at render time.
    authSpying = !!msg.spying;
    if (msg.spy_board) authSpyBoard = Int32Array.from(msg.spy_board);
    if (msg.spy_hide !== undefined) authSpyHide = msg.spy_hide;
    if (msg.spy_funds !== undefined) authSpyFunds = msg.spy_funds;
    if (!authSpying) { authSpyBoard = null; authSpyFunds = null; spyFlickerMask = null; }
    // Reconcile: prune acked inputs and, on a keyframe, restore the authoritative
    // state then replay the still-unacked tail — all inside WasmClient.on_snapshot
    // (the shared, proptested core). An empty keyframe array ⇒ no keyframe this frame.
    const youBaz = !!(msg.you && msg.you.in_bazaar);
    const oppBaz = !!(msg.opp && msg.opp.in_bazaar);
    const keyframe = msg.keyframe ? Uint8Array.from(msg.keyframe) : new Uint8Array(0);
    (game as WasmClient).on_snapshot(msg.ack >>> 0, youBaz, oppBaz, keyframe);
    if (!gameEnded && (msg.result === 1 || msg.result === 2)) {
        gameEnded = true;
        gameOverText.textContent = msg.result === 1 ? 'YOU WIN!' : 'GAME OVER - You Lost';
        gameOverOverlay.style.display = 'flex';
        clearMatchUrl(); // match decided — a refresh now lands in the lobby
    }
}

// Drop into a server-authoritative match: build the local prediction game with
// the server-assigned seed and start predicting immediately (we share the seed,
// so we're in lockstep with the server until a cross-player event arrives).
function enterAuthoritativeGame(msg: Extract<ServerMessage, { type: 'matchStart' }>) {
    searching = false;
    findMatchBtn.classList.remove('searching');
    cancelSearchBtn.style.display = 'none';
    mode = 'online';
    authoritative = true;
    onlineOpponentName = msg.opponent || 'Opponent';
    // Park the match in the URL so an accidental refresh can rejoin it. This also
    // fires on a rejoin's matchStart, so the URL stays correct after reconnecting.
    setMatchUrl(msg.match_id);
    hideReconnect(); // a rejoin handoff clears the "Reconnecting…" overlay
    updateModeButtons();
    showGame();

    currentSeed = msg.seed >>> 0;
    // Online uses the shared prediction/reconciliation client (a fresh one starts with
    // input_seq 0 and an empty unacked queue — per-bout, matching the server's ack).
    game = new WasmClient(currentSeed);
    resetMatchState();
    authSelf = null;
    authOpp = null;
    authSpying = false;
    authSpyBoard = null;
    authSpyFunds = null;
    spyFlickerMask = null;
    // Fresh match: no stored replay yet (the server sends matchReplay at the end).
    lastMatchReplayId = null;
    if (watchReplayBtn) watchReplayBtn.style.display = 'none';

    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;
    aiBoard.style.display = 'none';
    aiLabel.style.display = 'none';
    applyBoardScale();

    paused = false;
    gameEnded = false;
    onlinePaused = false; // server is ticking; predict immediately from the shared seed
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    setPlayingStatus();
    lastFrameTime = performance.now();
    tickAccumulator = 0;
}

// Auto-queue for a match over the persistent lobby socket. Requires a signed
// identity first (so the server knows who you are). `enterAuthoritativeGame`
// drops you into the playfield when matched.
async function findMatch() {
    if (searching || mode === 'online') return; // already queued, or already playing online
    if (!await ensureIdentity()) return;         // no name/token yet (field is focused)
    searching = true;
    findMatchBtn.classList.add('searching');
    onlineStatus.style.display = 'block';
    cancelSearchBtn.style.display = 'inline-block';
    setOnlineStatus('Searching for an opponent…');
    if (ws && ws.readyState === WebSocket.OPEN) {
        sendMsg({ type: 'queue', name: playerName ?? '', token: identityToken ?? '', authoritative: true });
    } else {
        // Socket down (e.g. just after a forfeit-leave) — reconnect and queue on open.
        pendingQueue = true;
        connectLobby();
    }
}

// Stop a background search and hide its UI (dequeues server-side via available:false).
function cancelSearch() {
    searching = false;
    pendingQueue = false;
    findMatchBtn.classList.remove('searching');
    cancelSearchBtn.style.display = 'none';
    onlineStatus.style.display = 'none';
    sendMsg({ type: 'available', value: false });
}

// Drop into a fresh online match. Online boards are independent (each player has
// their own seed and exchanges weapons + scores over the data channel), so this
// starts a clean board - the practice / vs-Computer game you played while waiting
// is discarded here.

// Matchmaking-socket message handler: the match handoff (matchStart) + the
// per-frame authoritative state (snapshot) + rating / opponentLeft. Shared by
// the background search and the live authoritative match.
async function onSignalMessage(ev: MessageEvent) {
    const msg = JSON.parse(ev.data) as ServerMessage;

    // ── Liveness probe ──────────────────────────────────────────────────────
    // The server emits {"type":"heartbeat"} ~2 Hz to detect a dropped socket while a
    // bout is frozen; the client has nothing to do with it. Handled EXPLICITLY (not a
    // silent fall-through) so it's part of the modelled contract.
    if (msg.type === 'heartbeat') { return; }

    // ── Lobby channel (live on the same socket) ──────────────────────────────
    if (msg.type === 'stats') { updateLiveStats(msg); return; }
    if (msg.type === 'players') { renderOnlineList(msg.players); return; }
    if (msg.type === 'challenged') { onChallenged(msg.from); return; }
    if (msg.type === 'challengeDeclined') {
        searching = false;
        findMatchBtn.classList.remove('searching');
        cancelSearchBtn.style.display = 'none';
        setOnlineStatus(`${msg.by} declined.`);
        if (onlineStatus) onlineStatus.style.display = 'block';
        return;
    }
    // ── Deploy quiesce: the server paused new matches for an in-place update. Any
    // match already in progress keeps running and finishes; only NEW matchmaking is
    // held. Stop the search spinner and tell the player; `resumed` clears it. ───────
    if (msg.type === 'draining') {
        searching = false;
        findMatchBtn.classList.remove('searching');
        cancelSearchBtn.style.display = 'none';
        setOnlineStatus('Server updating — new matches paused, back in a moment…');
        if (onlineStatus) onlineStatus.style.display = 'block';
        return;
    }
    if (msg.type === 'resumed') {
        setOnlineStatus('Server ready — find a match!');
        if (onlineStatus) onlineStatus.style.display = 'block';
        return;
    }
    // ── Reconnect (rejoin-on-refresh) ────────────────────────────────────────
    if (msg.type === 'opponentReconnecting') {
        // The server froze the bout while our opponent reconnects. Stop predicting
        // (no snapshots arrive while frozen) and show a countdown to the forfeit.
        onlinePaused = true;
        startReconnectCountdown(typeof msg.grace_secs === 'number' ? msg.grace_secs : 12);
        return;
    }
    if (msg.type === 'opponentResumed') {
        // Both sides back; the server resumes ticking. Un-pause local prediction and
        // drop the overlay (a keyframe rides right behind this to resync the board).
        onlinePaused = false;
        hideReconnect();
        return;
    }
    if (msg.type === 'rejoinFailed') {
        // No live bout for us (grace expired / match already ended / stale link).
        // Fail loudly: clear the URL and return to the lobby with a note.
        clearMatchUrl();
        authoritative = false;
        gameEnded = true;
        hideReconnect();
        showLobby();
        setOnlineStatus('That match has ended.');
        if (onlineStatus) onlineStatus.style.display = 'block';
        return;
    }

    // ── Match channel ────────────────────────────────────────────────────────
    // Server-authoritative match handoff + per-frame authoritative state.
    if (msg.type === 'matchStart') {
        enterAuthoritativeGame(msg);
        return;
    }
    if (msg.type === 'snapshot') {
        if (authoritative && game) applySnapshot(msg);
        return;
    }

    if (msg.type === 'rating') {
        const conservative = (msg.mu - 3 * msg.sigma).toFixed(1);
        const result = msg.won ? 'WIN' : 'LOSS';
        setOnlineStatus(
            `${result} - New rating: μ=${msg.mu.toFixed(2)}, σ=${msg.sigma.toFixed(2)},` +
            ` μ-3σ ~ ${conservative}`
        );

    } else if (msg.type === 'matchReplay') {
        // The server stored this match — reveal the "Watch replay" button on the
        // (already-showing or upcoming) game-over screen.
        lastMatchReplayId = msg.id;
        showWatchReplayBtn();
    } else if (msg.type === 'opponentLeft') {
        setOnlineStatus('Opponent left.');
        hideReconnect();
        clearMatchUrl(); // match over — a refresh now lands in the lobby
        if (!gameEnded) {
            gameEnded = true;
            gameOverText.textContent = 'Opponent left.';
            gameOverOverlay.style.display = 'flex';
        }
    }
}

// Show the game-over "Watch replay" button iff we have a stored replay id.
function showWatchReplayBtn() {
    if (watchReplayBtn) {
        watchReplayBtn.style.display = lastMatchReplayId ? '' : 'none';
    }
}

// ─── Game initialization ──────────────────────────────────────────────────────

async function initGame() {
    await init();
    FIXED_DT = fixed_dt(); // canonical timestep from the engine
    // Open on the lobby (like BTStartup), not in a game. A game starts when the
    // player picks Practice / Play Computer / Find Match / accepts a challenge.
    updatePlayComputerLabel();
    showLobby();
    applyScreenFromUrl();
}

// Dev: jump straight to a screen via the URL (for testing / styling without the
// normal flow). Examples:
//   ?screen=lobby
//   ?screen=practice            (or screen=game)
//   ?screen=vscomputer
//   ?screen=online              (kicks off matchmaking)
//   ?screen=bazaar              (force the bazaar overlay open, both shopping)
//   ?screen=bazaar&baz=waiting  (preview the "Waiting for opponent..." prompt)
//   ?screen=bazaar&baz=oppready (preview the "Your opponent is waiting..." prompt)
function applyScreenFromUrl() {
    const p = new URLSearchParams(location.search);
    // ?match=<id> means we were dropped from a live match (an accidental refresh) —
    // reconnect straight back into it before anything else.
    const match = p.get('match');
    if (match) { rejoinMatch(match); return; }
    // ?player=<name> opens a player's profile (stats panel) in the lobby — the
    // target of the replay-library name links.
    const player = p.get('player');
    if (player) selectPlayer(player);
    const screen = p.get('screen');
    if (!screen) return;
    switch (screen) {
        case 'lobby': showLobby(); break;
        case 'practice':
        case 'game': startGame('practice'); break;
        case 'vscomputer': startGame('vscomputer'); break;
        case 'online': startGame('online'); break;
        case 'bazaar':
            startGame('practice');
            // Force the overlay open for preview; `baz` picks the barrier prompt.
            debugBazaar = p.get('baz') || 'both';
            break;
        default: break;
    }
}

// Reattach to a live match after an accidental refresh (?match=<id>). Show the
// game screen with a "Reconnecting…" overlay, then connect + send `rejoin` on open
// (connectLobby.onopen consumes pendingRejoin). The server answers with either a
// matchStart handoff + keyframe (we're back in) or rejoinFailed (→ lobby).
function rejoinMatch(id: string) {
    currentMatchId = id;
    mode = 'online';
    authoritative = true;
    gameEnded = false;
    onlinePaused = true;
    pendingRejoin = id;
    showGame();
    showReconnect('Reconnecting…');
    connectLobby();
}

function startGame(newMode: string) {
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
        findMatchBtn.classList.remove('searching');
        cancelSearchBtn.style.display = 'none';
        onlineStatus.style.display = 'none';
    }

    mode = newMode;

    const seed = (performance.now() | 0) ^ (Math.floor(Math.random() * 1e9));
    currentSeed = seed >>> 0;

    // Create game instance based on mode
    if (mode === 'vscomputer') {
        const level = ernieSlider ? (parseInt(ernieSlider.value, 10) || 0) : DEFAULT_ERNIE_LEVEL;
        game = new WasmVsComputer(seed, level);
        // `?mock=1`: run this local vs-Ernie match as a mock ONLINE match (opponent
        // shown only through spies, funds spy-gated). Reset per-match spy tracking.
        mockMatch = new URLSearchParams(location.search).get('mock') === '1';
        mockSpy = null;
        mockTick = 0;
        mockPrevOppLines = 0;
    } else {
        game = new WasmGame(seed);
        mockMatch = false;
    }

    // Set canvas size based on game dimensions
    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;

    // Set up the side board canvas in vscomputer mode (Ernie's board). In an
    // online match the same canvas shows the spy-revealed opponent board, but it is
    // sized and labelled lazily when a spy first reveals it (see updateBoard), since
    // the panel is hidden until then.
    if (mode === 'vscomputer') {
        aiGridCanvas.width = width * CELL_SIZE;
        aiGridCanvas.height = height * CELL_SIZE;
        aiBoardLabel.textContent = 'Ernie (computer)';
        aiBoard.style.display = 'block';
        aiLabel.style.display = 'block';
    } else {
        aiBoard.style.display = 'none';
        aiLabel.style.display = 'none';
    }

    // Fit the board (and side board) to the viewport so the hit counter below
    // stays above the fold. Must run after aiBoard's display is set.
    applyBoardScale();

    // Update UI
    updateModeButtons();
    paused = false;
    gameEnded = false;
    authoritative = false; // local modes are not server-authoritative
    resetMatchState();
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    setPlayingStatus();
    showGame();
    lastFrameTime = performance.now();
    tickAccumulator = 0;
}

function updateModeButtons() {
    modePracticeBtn.classList.remove('active');
    playComputerBtn.classList.remove('active');
    findMatchBtn.classList.remove('active');

    if (mode === 'practice') {
        modePracticeBtn.classList.add('active');
    } else if (mode === 'vscomputer') {
        playComputerBtn.classList.add('active');
    } else if (mode === 'online') {
        findMatchBtn.classList.add('active');
    }
}

function newGame() {
    if (mode === 'online') {
        // An online match can't be unilaterally restarted — return to the lobby,
        // where the player can Find Match or challenge someone again.
        leaveToLobby();
        return;
    }
    startGame(mode);
}

function render() {
    // Draw player board
    const grid = game!.render_grid();
    const width = game!.width();
    const height = game!.height();
    drawBoard(ctx, grid, width, height);

    // Draw AI board in vscomputer mode (but NOT in a mock match, where the opponent
    // is shown through the spy path below, exactly like an online match).
    if (mode === 'vscomputer' && !mockMatch) {
        const aiGrid = (game as WasmVsComputer).render_ai_grid();
        drawBoard(aiCtx, aiGrid, width, height);
    } else if (authoritative || mockMatch) {
        // Server-authorized spy (or the mock's local equivalent): show the opponent's
        // board only while a spy of ours is active, flickered to the spy's accuracy.
        if (authSpying && authSpyBoard) {
            // Size the side canvas to the board the first time a spy reveals it
            // (initGame only sizes it for vs-Computer), then fit it to the viewport.
            // The resize is guarded so it runs once, not every frame.
            if (aiGridCanvas.width !== width * CELL_SIZE || aiGridCanvas.height !== height * CELL_SIZE) {
                aiGridCanvas.width = width * CELL_SIZE;
                aiGridCanvas.height = height * CELL_SIZE;
                aiBoard.style.display = 'block';
                applyBoardScale();
            }
            // Label it with the real opponent (not "Ernie"); value-guarded so it
            // tracks a new opponent across matches without a per-frame DOM write.
            const oppLabel = mockMatch ? 'Ernie' : (onlineOpponentName || 'Opponent');
            if (aiBoardLabel.textContent !== oppLabel) aiBoardLabel.textContent = oppLabel;
            aiBoard.style.display = 'block';
            drawBoard(aiCtx, spyFlicker(authSpyBoard, authSpyHide), width, height);
        } else {
            aiBoard.style.display = 'none';
        }
    }
}

function updateStats() {
    const score = game!.score();
    const lines = game!.lines();
    // In an authoritative match, funds (changed by opponent taxes) and the bazaar
    // countdown (depends on combined lines) are authoritative per-frame; score and
    // lines come from local prediction.
    const funds = (authoritative && authSelf) ? authSelf.funds : game!.funds();
    const tilBazaar = (authoritative && authSelf) ? authSelf.lines_til_bazaar : game!.lines_til_bazaar();

    scoreValue.textContent = score as unknown as string;
    linesValue.textContent = lines as unknown as string;
    fundsValue.textContent = funds as unknown as string;
    linesToBazaarValue.textContent = tilBazaar as unknown as string;

    // Mirror to mobile stats bar
    mobileScore.textContent = score as unknown as string;
    mobileLines.textContent = lines as unknown as string;
    mobileFunds.textContent = funds as unknown as string;
    mobileLinesToBazaar.textContent = tilBazaar as unknown as string;
}

function updateOpponentPanel() {
    // The opponent's score/lines are authoritative per-frame in an online match.
    const opScore = (authoritative && authOpp) ? authOpp.score : game!.op_score();
    const opLines = (authoritative && authOpp) ? authOpp.lines : game!.op_lines();
    opponentScore.textContent = opScore >= 0 ? opScore as unknown as string : '-';
    opponentLines.textContent = opLines >= 0 ? opLines as unknown as string : '-';

    // Opponent funds are revealed only while a spy of ours is active (online: the
    // server sends the per-spy adjusted value; mock: computed locally the same way);
    // hide the row otherwise.
    if ((authoritative || mockMatch) && authSpying && authSpyFunds !== null) {
        opponentFunds.textContent = authSpyFunds as unknown as string;
        opponentFundsRow.style.display = '';
    } else {
        opponentFundsRow.style.display = 'none';
    }

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

let lastArsenalSig: string | null = null;
function updateArsenalPanel() {
    // Only rebuild the DOM when the arsenal actually changes. Rebuilding it every
    // frame (this runs from the game loop) was destroying the item a user is
    // mid-click on, so the click — which deploys the weapon — never landed.
    let sig = '';
    for (let i = 0; i < 10; i++) sig += game!.arsenal_token(i) + ':' + game!.arsenal_quantity(i) + ',';
    if (sig === lastArsenalSig) return;
    lastArsenalSig = sig;

    arsenalList.innerHTML = '';
    mobileArsenalList.innerHTML = '';

    for (let i = 0; i < 10; i++) {
        const token = game!.arsenal_token(i);
        const key = ARSENAL_KEYS[i];
        const slot = i; // capture for closure

        // ── Desktop arsenal item ──────────────────────────────────────────
        const div = document.createElement('div');
        div.className = 'arsenal-item';

        if (token >= 0) {
            const name = weapon_name(token);
            const qty = game!.arsenal_quantity(i);
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
            const qty = game!.arsenal_quantity(i);
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
        const token = game!.arsenal_token(i);
        const key = ARSENAL_KEYS[i];
        const slot = document.createElement('div');
        slot.className = 'bazaar-arsenal-slot';
        if (token >= 0) {
            const qty = game!.arsenal_quantity(i);
            const nm = weapon_name(token);
            slot.textContent = `${key}. ${nm} x${qty}`;
            slot.classList.add('occupied');
        } else {
            slot.textContent = `${key}. < Empty >`;
        }
        bazaarArsenalList.appendChild(slot);
    }
}

function selectBazaarToken(token: number) {
    bazaarSelectedToken = token;

    // Highlight the selected row
    const rows = bazaarWeaponList.querySelectorAll('.bazaar-weapon-row');
    rows.forEach((r) => {
        if (parseInt((r as HTMLElement).dataset['token']!, 10) === token) {
            r.classList.add('selected');
        } else {
            r.classList.remove('selected');
        }
    });

    // Show weapon info
    if (token >= 0) {
        const price = game!.bazaar_price(token);
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
        row.dataset['token'] = t as unknown as string;
        row.textContent = name;
        row.addEventListener('click', () => {
            selectBazaarToken(t);
        });
        bazaarWeaponList.appendChild(row);
    }

    bazaarFunds.textContent = game!.funds() as unknown as string;
    refreshBazaarArsenal();
}

// Track whether bazaar was open last frame to avoid re-populating every tick.
// The synchronized bazaar BARRIER is server-authoritative online (the bout freezes
// both boards until both players hit Done — the client reads authSelf.in_bazaar);
// in local modes the engine's own flag drives it.
let bazaarWasOpen = false;
// Dev preview override set by ?screen=bazaar&baz=… : 'both' | 'waiting' | 'oppready'.
let debugBazaar: string | null = null;

// Reset per-match overlay state when a new game starts.
function resetMatchState() {
    bazaarWasOpen = false;
    lastArsenalSig = null; // force the arsenal to re-render for the new game
}

function updateBazaarOverlay() {
    // The bazaar is a server-authoritative BARRIER online: it stays open until
    // BOTH players hit Done, so it tracks `you.in_bazaar || opp.in_bazaar` (each
    // prompt every frame) — not just our own flag, which clears the instant WE
    // hit Done. Local modes use the engine's own flag.
    const online = authoritative && authSelf;
    let youShopping: boolean, oppShopping: boolean;
    if (debugBazaar) {
        // Dev preview: 'waiting' = you've hit Done; 'oppready' = opponent has.
        youShopping = debugBazaar !== 'waiting';
        oppShopping = debugBazaar !== 'oppready';
    } else {
        youShopping = online ? authSelf!.in_bazaar : game!.is_in_bazaar();
        oppShopping = online ? !!(authOpp && authOpp.in_bazaar) : false;
    }
    const inBaz = youShopping || oppShopping;
    // The bazaar is a full-screen page: while it's open the gameplay touch
    // controls are useless and would overlap it, so hide them (CSS keys off this).
    document.body.classList.toggle('bazaar-open', inBaz);
    if (inBaz) {
        bazaarOverlay.style.display = 'flex';
        if (!bazaarWasOpen) {
            // Only fully repopulate when bazaar first opens; re-enable Done.
            populateBazaar();
            bazaarWasOpen = true;
            if (bazaarDoneBtn) {
                bazaarDoneBtn.disabled = false;
                bazaarDoneBtn.textContent = 'DONE';
            }
        } else {
            // Keep funds and arsenal display fresh while open
            bazaarFunds.textContent = online ? authSelf!.funds as unknown as string : game!.funds() as unknown as string;
            refreshBazaarArsenal();
        }
        if (online || debugBazaar) updateBazaarBarrierStatus(youShopping, oppShopping);
    } else {
        if (bazaarWasOpen) {
            bazaarOverlay.style.display = 'none';
            bazaarWasOpen = false;
        }
    }
}

// Surface both sides' readiness during the online bazaar barrier:
//   • I hit Done, opponent still shopping  → "Waiting for opponent…" (Done locked)
//   • Opponent hit Done, I'm still shopping → "Opponent is ready" (I can keep buying)
//   • Both still shopping                   → no prompt
function updateBazaarBarrierStatus(youShopping: boolean, oppShopping: boolean) {
    if (!bazaarBarrierStatus) return;
    if (!youShopping && oppShopping) {
        bazaarBarrierStatus.textContent = 'Waiting for opponent...';
        bazaarBarrierStatus.className = 'bazaar-barrier-status waiting';
        if (bazaarDoneBtn) bazaarDoneBtn.disabled = true;
    } else if (youShopping && !oppShopping) {
        bazaarBarrierStatus.textContent = 'Your opponent is waiting...';
        bazaarBarrierStatus.className = 'bazaar-barrier-status opp-ready';
    } else {
        bazaarBarrierStatus.textContent = '';
        bazaarBarrierStatus.className = 'bazaar-barrier-status';
    }
}

function processEvents() {
    const events = game!.drain_events();

    for (let i = 0; i < events.length; i += 4) {
        // Events are packed [tag, a, b, c]; only tag + a are consumed here.
        // i < events.length guarantees both slots exist; ?? 0 satisfies the checker.
        const tag = events[i] ?? 0;
        const a = events[i + 1] ?? 0;

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

        // Local-mode game over (practice / vs-computer top-out). In an
        // authoritative online match the server's result drives game over (via
        // the snapshot), so the client doesn't latch it from its own prediction.
        if (!authoritative && tag === 5) {
            gameEnded = true;
            gameOverText.textContent = 'GAME OVER';
            gameOverOverlay.style.display = 'flex';
        }
    }
}

// ─── Debug overlay (internal state on the playfield) ─────────────────────────
// Off by default; ?debug=1 or the backtick key toggles it. A diagnostic surface
// for netcode/desync work: tick/seed/match, prediction-vs-server drift, unacked
// inputs, and active weapons + remaining lines.
const debugOverlayEl = document.getElementById('debugOverlay') as HTMLElement | null;
const debugToolsEl = document.getElementById('debugTools') as HTMLElement | null;
// `?debug=1` (or the backtick key) shows the debug overlay + weapon-grant picker.
// `?mock=1` implies it, so a mock match has the grant picker for arming spies/weapons.
let debugOn = (() => { const q = new URLSearchParams(location.search); return q.get('debug') === '1' || q.get('mock') === '1'; })();
window.addEventListener('keydown', (e) => {
    if (e.key === '`') {
        debugOn = !debugOn;
        if (!debugOn && debugOverlayEl) debugOverlayEl.style.display = 'none';
        if (!debugOn && debugToolsEl) debugToolsEl.style.display = 'none';
    }
});
function updateDebugOverlay() {
    if (!debugOverlayEl) return;
    if (!debugOn || lobbyActive || !game) { debugOverlayEl.style.display = 'none'; return; }
    debugOverlayEl.style.display = '';
    const L: string[] = [];
    L.push('▟ DEBUG  (` toggles)');
    L.push(`mode=${mode} auth=${authoritative} ended=${gameEnded}`);
    if (authoritative) L.push(`onlinePaused=${onlinePaused}`);
    if (currentMatchId) L.push(`match=${currentMatchId}`);
    if (currentSeed != null) L.push(`seed=${currentSeed}`);
    // Prediction queue stats come from the shared client (online only).
    if (authoritative && typeof (game as WasmClient).input_seq === 'function') {
        L.push(`inputSeq=${(game as WasmClient).input_seq()} unacked=${(game as WasmClient).unacked_len()}`);
    }
    try {
        // `result()` exists on the vs-computer engine, not the online WasmClient.
        const localResult = (typeof (game as WasmVsComputer).result === 'function') ? (game as WasmVsComputer).result() : '—';
        L.push(`local you: score=${game.score()} lines=${game.lines()} funds=${game.funds()} tilBaz=${game.lines_til_bazaar()} baz=${game.is_in_bazaar()} result=${localResult}`);
        L.push(`local opp: score=${game.op_score()} lines=${game.op_lines()}`);
    } catch (_) {}
    if (authoritative && authSelf) {
        L.push(`srv you: funds=${authSelf.funds} baz=${authSelf.in_bazaar} tilBaz=${authSelf.lines_til_bazaar}`);
    }
    if (authoritative && authOpp) {
        L.push(`srv opp: score=${authOpp.score} lines=${authOpp.lines} over=${authOpp.game_over}`);
        try {
            const drift = game.op_score() - authOpp.score;
            if (drift !== 0) L.push(`  ⚠ opp-score drift=${drift}`);
        } catch (_) {}
    }
    if (authoritative) L.push(`spying=${authSpying}`);
    try {
        const active: string[] = [];
        const max = (typeof max_weapons === 'function') ? max_weapons() : 34;
        for (let t = 0; t < max; t++) {
            if ((game as WasmGame).weapon_active(t)) active.push(`${weapon_name(t)}(${(game as WasmGame).weapon_remaining(t)})`);
        }
        L.push('weapons: ' + (active.length ? active.join(', ') : '—'));
    } catch (_) {}
    debugOverlayEl.textContent = L.join('\n');
}

// Debug weapon-grant picker (vs-Computer only): a funds drop + one-click grant of
// any of the 34 weapons straight into the arsenal. Behind the same ?debug=1 / `
// gate as the overlay. Built once, lazily. Not recorded into the replay (a debug
// mutation), so granted weapons won't reproduce on replay — fine for live testing.
let debugToolsBuilt = false;
function buildDebugTools() {
    if (!debugToolsEl || debugToolsBuilt) return;
    debugToolsBuilt = true;
    const head = document.createElement('div');
    head.className = 'dt-head';
    head.textContent = '▟ WEAPON GRANT';
    debugToolsEl.appendChild(head);

    const fundsBtn = document.createElement('button');
    fundsBtn.className = 'dt-funds';
    fundsBtn.textContent = '+99,999 ¢';
    fundsBtn.addEventListener('click', () => {
        const g = game as WasmVsComputer | null;
        if (g && typeof g.add_funds === 'function') { g.add_funds(99999); updateArsenalPanel(); }
    });
    debugToolsEl.appendChild(fundsBtn);

    const grid = document.createElement('div');
    grid.className = 'dt-grid';
    const max = (typeof max_weapons === 'function') ? max_weapons() : 34;
    for (let t = 0; t < max; t++) {
        const btn = document.createElement('button');
        btn.className = 'dt-wpn';
        btn.textContent = weapon_name(t);
        btn.title = `${weapon_name(t)} — ${weapon_price(t)}¢`;
        btn.addEventListener('click', () => {
            const g = game as WasmVsComputer | null;
            if (!g || typeof g.grant_weapon !== 'function') return;
            if (g.grant_weapon(t)) { updateArsenalPanel(); showToast(`granted ${weapon_name(t)}`, 1200); }
            else { showToast('arsenal full (10 slots) — sell one first', 1600); }
        });
        grid.appendChild(btn);
    }
    debugToolsEl.appendChild(grid);
}
function updateDebugTools() {
    if (!debugToolsEl) return;
    const show = debugOn && !lobbyActive && !!game && typeof (game as WasmVsComputer).grant_weapon === 'function';
    if (!show) { debugToolsEl.style.display = 'none'; return; }
    buildDebugTools();
    debugToolsEl.style.display = '';
}

function gameLoop(now: number) {
    // While the lobby is showing there is nothing to tick or render.
    if (lobbyActive || !game) {
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

    // Process events (audio; local-mode game-over). In an authoritative match the
    // server resolves cross-player effects; inputs are sent via predict().
    if (!onlinePaused || mode !== 'online') {
        processEvents();
    }

    // Check for win/loss in vscomputer mode. NOTE: don't gate on
    // game.is_game_over() - it returns true as soon as `result` is set (it ORs
    // in result != 0), so gating on it would suppress the win banner when Ernie
    // tops out (the player is still alive). Read `result` directly.
    if (mode === 'vscomputer' && !gameEnded) {
        const result = (game as WasmVsComputer).result();
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

    // In a mock match, refresh the spy reveal from the local opponent before drawing
    // (online this arrives in a snapshot; here it is synthesized each frame).
    if (mockMatch) mockSyncSpy();

    // Render
    render();
    updateStats();
    updateOpponentPanel();
    updateArsenalPanel();
    updateBazaarOverlay();
    updateDebugOverlay();
    updateDebugTools();

    // When an opponent board is visible (vs-Computer Ernie, or an online spy),
    // mark the game-area so the mobile layout puts the two boards side by side
    // (instead of stacked, which buried the weapon buttons).
    if (gameAreaEl) gameAreaEl.classList.toggle('two-boards', aiBoard.style.display === 'block');

    requestAnimationFrame(gameLoop);
}


interface TouchGesture {
    id: number;
    startX: number;
    startY: number;
    lastX: number;
    startTime: number;
    accDx: number;
    totalDx: number;
    totalDy: number;
    cell: number;
    dropped: boolean;
}

// ─── Touch gesture handling on game canvas ────────────────────────────────────

let touchState: TouchGesture | null = null; // Tracks the active game touch gesture

canvas.addEventListener('touchstart', (e) => {
    // Only track the first touch
    if (e.changedTouches.length === 0) return;
    e.preventDefault();

    if (!game || gameEnded || game.is_game_over() || inBazaar()) return;
    if (mode === 'online' && onlinePaused) return;

    const touch = e.changedTouches[0];
    if (!touch) return; // length check above guarantees this, but satisfies the checker
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
    let touch: Touch | null = null;
    for (let i = 0; i < e.changedTouches.length; i++) {
        const t = e.changedTouches[i];
        if (t && t.identifier === touchState.id) { touch = t; break; }
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
    let touch: Touch | null = null;
    for (let i = 0; i < e.changedTouches.length; i++) {
        const t = e.changedTouches[i];
        if (t && t.identifier === touchState.id) { touch = t; break; }
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

canvas.addEventListener('touchcancel', (_e) => {
    touchState = null;
}, { passive: false });

// ─── On-screen touch control bar ─────────────────────────────────────────────

function setupTouchButton(btnId: string, action: () => void, repeatInterval: number | null) {
    const btn = document.getElementById(btnId);
    if (!btn) return;

    // Initial delay before a held button starts auto-repeating (key-repeat
    // style). A quick tap fires exactly once; only holding past this repeats.
    const REPEAT_DELAY = 250;

    let repeatTimer: number | null = null;
    let delayTimer: number | null = null;

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

    btn.addEventListener('pointercancel', (_e) => {
        stopRepeat();
    });

    btn.addEventListener('pointerleave', (_e) => {
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
    // Hard drop: slam to the bottom (the touch equivalent of Space). One-shot.
    setupTouchButton('touchHardDrop', () => predict('BeginDrop'), null);
}

// Input handling
function handleKeyDown(e: KeyboardEvent) {
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

    // Arrow keys / WASD and pause.
    switch (key) {
        case 'ArrowLeft':
        case 'a':
        case 'A':
            e.preventDefault();
            predict('MoveLeft');
            return;
        case 'ArrowRight':
        case 'd':
        case 'D':
            e.preventDefault();
            predict('MoveRight');
            return;
        case 'ArrowUp':
        case 'w':
        case 'W':
            e.preventDefault();
            predict('Rotate');
            return;
        case 'ArrowDown':
        case 's':
        case 'S':
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
        // Optimistic prompt; the authoritative status (updateBazaarBarrierStatus)
        // takes over next frame and flips to "Opponent is ready" if they finish.
        if (bazaarBarrierStatus) {
            bazaarBarrierStatus.textContent = 'Waiting for opponent...';
            bazaarBarrierStatus.className = 'bazaar-barrier-status waiting';
        }
    } else {
        (game as WasmGame | WasmVsComputer).leave_bazaar();
        bazaarOverlay.style.display = 'none';
    }
});

bazaarAddBtn.addEventListener('click', () => {
    if (bazaarSelectedToken < 0 || !game) return;
    if (predict('BuyWeapon', bazaarSelectedToken)) {
        bazaarFunds.textContent = game.funds() as unknown as string;
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
        bazaarFunds.textContent = game.funds() as unknown as string;
        updateStats();
        updateArsenalPanel();
        refreshBazaarArsenal();
        selectBazaarToken(bazaarSelectedToken);
    }
});

// ─── Lobby actions ──────────────────────────────────────────────────────────
modePracticeBtn.addEventListener('click', () => startGame('practice'));
playComputerBtn.addEventListener('click', () => startGame('vscomputer'));
findMatchBtn.addEventListener('click', () => startGame('online'));
if (cancelSearchBtn) cancelSearchBtn.addEventListener('click', cancelSearch);

// Ernie difficulty slider: update the button label live, and (if a vs-Computer
// game is in progress) restart it so the new level takes effect immediately.
if (ernieSlider) {
    ernieSlider.addEventListener('input', () => {
        updatePlayComputerLabel();
        if (mode === 'vscomputer' && !lobbyActive) startGame('vscomputer');
    });
}

// Lobby presence/challenge controls. The list, challenge and stats are populated
// by the presence/identity server work (Phases 3-4); wired here so the buttons
// exist and degrade gracefully until then.
// Name field: commit on blur or Enter (Enter also blurs so it's a single commit).
if (nameInput) {
    nameInput.addEventListener('change', () => commitNameFromField());
    nameInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') nameInput.blur(); });
}
if (challengeBtn) challengeBtn.addEventListener('click', () => challengeSelected());
if (updateBtn) updateBtn.addEventListener('click', pressUpdate);
if (availableToggle) availableToggle.addEventListener('change', () => setAvailable(availableToggle.checked));
if (availableToggleGame) availableToggleGame.addEventListener('change', () => setAvailable(availableToggleGame.checked));
const challengeAcceptBtn = document.getElementById('challengeAccept');
const challengeDeclineBtn = document.getElementById('challengeDecline');
if (challengeAcceptBtn) challengeAcceptBtn.addEventListener('click', () => respondChallenge(true));
if (challengeDeclineBtn) challengeDeclineBtn.addEventListener('click', () => respondChallenge(false));

// Leaving a game returns to the lobby (BTGame -> BTChallenge). Back-to-lobby on
// the game-over overlay, and an explicit Leave button.
const backToLobbyBtn = document.getElementById('backToLobbyBtn');
const leaveGameBtn = document.getElementById('leaveGameBtn');
function leaveToLobby() {
    // Forfeit only if leaving a still-live online match (a finished match has
    // already settled server-side). An INTENTIONAL leave forfeits at once via
    // `leaveMatch` — NOT by dropping the socket, which now triggers the reconnect
    // grace (that's only for an accidental refresh). We keep the persistent lobby
    // socket; the server resets us to lobby presence when the bout ends.
    const forfeiting = (mode === 'online' && !gameEnded);
    if (forfeiting) {
        try { sendMsg({ type: 'leaveMatch' }); } catch (_) {}
    }
    pendingRejoin = null;        // cancel any queued reconnect — we're leaving on purpose
    clearMatchUrl();             // a refresh now lands in the lobby, not back in the match
    hideReconnect();
    cleanupOnline();
    mode = 'practice';           // back to a local/lobby mode so Find Match works again
    gameEnded = true;
    game = null;
    gameOverOverlay.style.display = 'none';
    bazaarOverlay.style.display = 'none';
    // Keep lastMatchReplayId so the lobby Share button can still share the match you
    // just finished (it's reset when the next match starts / a new matchReplay lands).
    if (watchReplayBtn) watchReplayBtn.style.display = 'none';
    showLobby();
}
const leaveGameTop = document.getElementById('leaveGameTop');
if (backToLobbyBtn) backToLobbyBtn.addEventListener('click', leaveToLobby);
if (watchReplayBtn) watchReplayBtn.addEventListener('click', () => {
    if (lastMatchReplayId) location.href = '/replay/' + lastMatchReplayId;
});
if (leaveGameBtn) leaveGameBtn.addEventListener('click', leaveToLobby);
if (leaveGameTop) leaveGameTop.addEventListener('click', leaveToLobby);

// ─── Bug report ─────────────────────────────────────────────────────────────
// Capture a deterministic replay of the current game, upload it for a shareable
// link, and open a prefilled GitHub issue. No server-side secret: the user
// reviews and posts the issue themselves.
const BUG_REPO = 'perplexes/BattleTris';
const bugOverlay = document.getElementById('bugOverlay') as HTMLElement;
const bugTitleInput = document.getElementById('bugTitle') as HTMLInputElement;
const bugExpected = document.getElementById('bugExpected') as HTMLTextAreaElement;
const bugActual = document.getElementById('bugActual') as HTMLTextAreaElement;
const bugStatus = document.getElementById('bugStatus') as HTMLElement;
const bugSubmit = document.getElementById('bugSubmit') as HTMLButtonElement;
const bugCancel = document.getElementById('bugCancel') as HTMLElement | null;
const reportBugBtn = document.getElementById('reportBug') as HTMLElement | null;

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
let bugReplayJson: string | null = null;

function openBug() {
    bugReplayJson = (game && typeof (game as WasmGame).export_replay === 'function') ? (game as WasmGame).export_replay() : null;
    bugStatus.textContent = bugReplayJson ? '' : 'No active game - the report will have no replay attached.';
    bugSubmit.disabled = false;
    bugOverlay.classList.add('open');
    bugTitleInput.focus();
}

function closeBug() {
    bugOverlay.classList.remove('open');
}

async function uploadReplay(json: string) {
    const res = await fetch('/api/replays', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: json,
    });
    if (!res.ok) throw new Error('upload failed (' + res.status + ')');
    return (await res.json()).id;
}

function downloadReplay(json: string) {
    const blob = new Blob([json], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'battletris-replay.json';
    a.click();
    URL.revokeObjectURL(a.href);
}

// The replay metadata rendered into a bug report — derived from the recording's
// ReplayMeta (frames collapsed to a count).
interface BugMeta {
    // `| undefined` (not just `?`) because the object is built by spreading optional
    // ReplayMeta fields straight through — under exactOptionalPropertyTypes an
    // explicit `undefined` value must be allowed by the type.
    mode?: string | undefined;
    ai_level?: number | null | undefined;
    engine_sha?: string | undefined;
    seed?: number | undefined;
    tick_count?: number | undefined;
    inputs: number;
}

function buildIssueBody(expected: string, actual: string, replayUrl: string | null, meta: BugMeta | null) {
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
    let replayUrl: string | null = null;
    let meta: BugMeta | null = null;
    if (bugReplayJson) {
        try {
            const r = JSON.parse(bugReplayJson) as ReplayMeta;
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
const toast = document.getElementById('toast') as HTMLElement;
let toastTimer: number | null = null;

function showToast(msg: string, ms = 4500) {
    toast.textContent = msg;
    toast.classList.add('show');
    clearTimeout(toastTimer!);
    toastTimer = setTimeout(() => toast.classList.remove('show'), ms);
}

// Copy a /replay/<id> link to the clipboard (with a visible fallback).
async function shareReplayLink(id: string) {
    const url = `${location.origin}/replay/${id}`;
    try { await navigator.clipboard.writeText(url); showToast('Replay link copied: ' + url); }
    catch (e) { showToast('Replay link: ' + url); }
}

async function shareReplay() {
    // An ONLINE match's real recording is the server-stored VersusReplay
    // (`lastMatchReplayId`, set at match end) — the local `game` is only this
    // client's prediction. So for online (or when there's no local game, e.g. in
    // the lobby after a match) share the stored id; only a local practice /
    // vs-computer game uploads its own recording.
    if (mode === 'online' || !game || typeof (game as WasmGame).export_replay !== 'function') {
        if (lastMatchReplayId) {
            await shareReplayLink(lastMatchReplayId);
        } else {
            showToast('No finished match to share yet.');
        }
        return;
    }
    showToast('Saving replay...', 10000);
    try {
        await shareReplayLink(await uploadReplay((game as WasmGame).export_replay()));
    } catch (e) {
        showToast('Share failed: ' + (e as Error).message);
    }
}

if (shareReplayBtn) shareReplayBtn.addEventListener('click', shareReplay);

const openLibraryBtn = document.getElementById('openLibrary');
if (openLibraryBtn) openLibraryBtn.addEventListener('click', () => { location.href = '/www/library.html'; });

const openLeaderboardBtn = document.getElementById('openLeaderboard');
if (openLeaderboardBtn) openLeaderboardBtn.addEventListener('click', () => { location.href = '/www/leaderboard.html'; });

// Debug / e2e hook: live access to the current game instance + mode (the getter
// closes over the module's `game`, so it always returns the active one). Used by
// the Playwright weapon-deploy test to pre-stock weapons and read Ernie's board.
(window as any).bt = { get game() { return game; }, get mode() { return mode; } };

// Initialize and start game loop
(async () => {
    // Load identity FIRST: initGame's applyScreenFromUrl may rejoin a match from
    // ?match=<id>, which needs our name + a VALID token ready (and populates the name
    // field). We no longer persist the token across sessions (it can go stale), so
    // mint a fresh one now — before initGame can fire that rejoin — so the rejoin and
    // any lobby presence carry a token the server will actually verify.
    loadIdentity();
    await ensureIdentity();

    await initGame();

    // Wire up on-screen touch control buttons
    setupTouchControls();

    // One persistent socket for the lobby (presence/stats/challenge) + matches.
    // (A rejoin from ?match=<id> already opened it; this is a no-op then.)
    connectLobby();

    requestAnimationFrame(gameLoop);
})();
