import init, { WasmGame, WasmVsComputer, fixed_dt, max_weapons, weapon_name, weapon_description, weapon_price, weapon_duration } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';
import { Sound } from './sound.js';

// Game state
let game = null;
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
let ws = null;       // WebSocket: matchmaking handoff + the authoritative match
let onlinePaused = true;  // true until a match starts
let onlineOpponentName = '';
let searching = false;    // background matchmaking in progress (queued, not yet matched)

// Server-authoritative online play (the client-server migration). In an
// authoritative match the server runs the real simulation; the client predicts
// locally for a 0-latency feel, sends each input to the server (tagged with a
// monotonic seq), and reconciles against the server's keyframes by restoring the
// full game state and re-applying its not-yet-acknowledged inputs.
let authoritative = false; // true during a server-authoritative online match
let currentMatchId = null; // the live bout's id; parked in the URL for rejoin-on-refresh
let currentSeed = null;    // the game's RNG seed (shown in the debug overlay)
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
const watchReplayBtn = document.getElementById('watchReplayBtn');
// The just-finished online match's stored replay id (sent by the server at
// match end) — drives the game-over "Watch replay" button.
let lastMatchReplayId = null;
const newGameBtn = document.getElementById('newGameBtn');
const bazaarOverlay = document.getElementById('bazaarOverlay');
const bazaarFunds = document.getElementById('bazaarFunds');
const bazaarDoneBtn = document.getElementById('bazaarDoneBtn');
const bazaarBarrierStatus = document.getElementById('bazaarBarrierStatus');
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
const modePracticeBtn = document.getElementById('modePractice');
const playComputerBtn = document.getElementById('playComputerBtn');
const findMatchBtn = document.getElementById('findMatchBtn');
const aiBoard = document.getElementById('aiBoard');
const gameAreaEl = document.querySelector('.game-area');
const aiLabel = document.getElementById('aiLabel');
const onlineStatus = document.getElementById('onlineStatus');

// Two-screen views (lobby <-> playfield) — like the original window swapping the
// BTChallenge and BTGame forms (BTStartup.C). Only one is visible at a time.
const lobbyScreen = document.getElementById('lobbyScreen');
const gameScreen = document.getElementById('gameScreen');
const onlineListEl = document.getElementById('onlineList');
const challengeBtn = document.getElementById('challengeBtn');
const updateBtn = document.getElementById('updateBtn');
const availableToggle = document.getElementById('availableToggle');
const availableToggleGame = document.getElementById('availableToggleGame');
const statsPanelEl = document.getElementById('statsPanel');
const playingStatusEl = document.getElementById('playingStatus');
const ernieSlider = document.getElementById('ernieSlider');
const nameInput = document.getElementById('nameInput');
const nameHint = document.getElementById('nameHint');

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
function setMatchUrl(id) {
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
const reconnectOverlay = document.getElementById('reconnectOverlay');
const reconnectText = document.getElementById('reconnectText');
let reconnectTimer = null; // the live forfeit countdown (connected side)
function stopReconnectCountdown() {
    if (reconnectTimer) { clearInterval(reconnectTimer); reconnectTimer = null; }
}
function showReconnect(text) {
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
function startReconnectCountdown(secs) {
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
let selectedPlayer = null;
let lobbyPlayers = [];

// Render the online players list. Each row selects the player (loads their stats
// + enables Challenge). Populated from the server's `players` push; empty until
// presence tracking lands.
function renderOnlineList(players) {
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

function selectPlayer(name) {
    selectedPlayer = name;
    if (challengeBtn) challengeBtn.disabled = !name;
    renderOnlineList(lobbyPlayers);
    loadPlayerStats(name);
}

// The `players` roster is PUSHED live over the websocket on every change (see the
// `players` handler in onSignalMessage), so the UPDATE button has no work to do — the
// 1994 client needed it because it PULLED the roster; we don't. So instead it does a
// bit. The original escalating gag is a one-shot SEQUENCE that leads (you can't drop
// into its middle); after it, a random draw from the pool. Sequences (the intro, the
// infomercial) play whole, in order, and only ONCE. A few entries are dynamic — the
// live press count, a running "speedrun" timer, a self-referential index. Past one
// threshold it gets concerned about you; past a higher one it grants a saved achievement.
const UPDATE_CONCERNED_AFTER = 25; // the "we are concerned" line only after this many
const UPDATE_ACHIEVEMENT_AT = 50;  // press count that unlocks the achievement (once, persisted)
function gagOrdinal(n) { const s = ['th', 'st', 'nd', 'rd'], v = n % 100; return n + (s[(v - 20) % 10] || s[v] || s[0]); }
function gagElapsed(ms) { const t = Math.max(0, Math.floor(ms / 1000)); return Math.floor(t / 60) + ':' + String(t % 60).padStart(2, '0'); }

// The opening sequence (the original escalating bit). Plays once, in order, the moment
// you first press; it's NOT in the random pool, so it can never surface mid-way.
const UPDATE_INTRO = [
    'You press UPDATE. Nothing happens.',
    'You press UPDATE again. Still nothing happens.',
    'You press UPDATE yet again and meditate on the nature of nothing.',
    'You press UPDATE and sneeze. Bless you.',
    'UPDATE presses YOU.',
    'If only there was a technology that sent updates to the client automatically.',
    'Some sort of socket.',
    'Some sort of socket for the web.',
    'XMLHttpRequest rolls off the tongue.',
];

// The pool drawn from after the intro. Standalone entries repeat; a `{seq}` is a
// one-shot sequence (plays whole, in order, then never again — and since only the
// unit sits in the pool, you can never land on a sequence's middle).
const UPDATE_GAGS = [
    'You press UPDATE harder. The nothing intensifies.',
    'Nothing happened. But you grew, a little, as a person.',
    'You press UPDATE. There is no undo without a do.',
    'You press UPDATE and achieve a brief, perfect emptiness.',
    'The button thanks you for the attention.',
    'You press UPDATE. The absence of news continues to develop.',
    'You poll. The server already pushed. You poll anyway.',
    'The data arrived 30 times a second. You pressed once. Bold.',
    '> EXAMINE UPDATE. A button, convinced it has a job.',
    '> PICK UP UPDATE. Taken. Inventory: one (1) button that does nothing.',
    '> TALK TO UPDATE. "I remember when people needed me," it says.',
    'Please insert Disk 2.',
    'A grizzled NPC blocks the path: "Ye cannot refresh what is already fresh."',
    '> OPEN UPDATE. It is already Open To Matches. So, it seems, are you.',
    'Thank you for your update. It has been carefully ignored.',
    'The button would like you to know it is trying its best.',
    'We kept this button for emotional reasons.',
    'Please rate your nothing: ☆☆☆☆☆',
    (c) => `The Count counts your presses: ${c.n}. The Count is delighted.`,
    "The button is empty, and so are you, and that's alright.",
    'To press is human; to do nothing, divine.',
    'There is no refresh. There is only the eternal now of the open socket.',
    { after: UPDATE_CONCERNED_AFTER, fn: (c) => `You press UPDATE for the ${gagOrdinal(c.n)} time. We are a little concerned.` },
    'You press UPDATE. It has started a journal. You are in it.',
    'You press UPDATE. Okay. We get it. You like the button.',
    (c) => `UPDATE any% WR: ${gagElapsed(c.elapsed)}`,
    'We value your press. Please continue to hold for nothing.',
    'In a rare display, the user presses again. The button does not flee.',
    'Mercury is in retrograde. The websocket is fine.',
    () => 'Best viewed in ' + (Math.random() < 0.5 ? 'Netscape Navigator' : 'NCSA Mosaic') + '.',
    'This button is under construction.',
    'The button no longer dreams of working. The button is free.',
    'You could be playing a game right now. Just saying.',
    'Anyway.',
    "Cool. Cool cool cool. Nothing's happening, but cool.",
    // A sequence: once entered, all of it plays in order (the bit only lands as a set
    // — "a SECOND nothing" needs a first). See `seq` handling in pressUpdate.
    { seq: [
        "But WAIT — there's still nothing! Press again for even less!",
        'For three easy payments of nothing, this button is yours.',
        'Order now and receive a SECOND nothing, free.',
    ] },
    "You can't refresh what's already whole.",
    "Today's affirmation: I am already up to date.",
    "Chef's note: this button is purely garnish.",
    'Somewhere, a different button does something. Not this one. Be at peace.',
    "Entropy increased very slightly. You're welcome, universe.",
    'You contain multitudes. The button contains a single event listener.',
    'The heat death of the universe is now marginally closer. Worth it?',
    'Someone wrote this message instead of removing the button.',
    (c) => `This is the ${gagOrdinal(c.idx + 1)} thing the button can say and zero things it can do.`,
    'A developer is watching you press this. They are not okay.',
    "That's a lot of presses.",
    'We admire the commitment. We worry about the commitment.',
];

let updatePresses = 0, updateFirstMs = 0;
let updateSeq = null;                // { list, pos } while a sequence is playing
let updateIntroDone = false;         // the opening sequence is one-shot
const updatePlayedSeq = new Set();   // pool indices of one-shot sequences already run
function updateAchUnlocked() { try { return localStorage.getItem('bt_ach_update') === '1'; } catch (_) { return false; } }
function resolveGag(e, ctx) { return typeof e === 'function' ? e(ctx) : (e && e.fn ? e.fn(ctx) : e); }
function pressUpdate() {
    updatePresses++;
    if (!updateFirstMs) updateFirstMs = Date.now();
    const ctx = { n: updatePresses, elapsed: Date.now() - updateFirstMs, idx: 0 };

    // A sequence in progress runs to its end — never interrupted, never re-entered.
    if (updateSeq) {
        const e = updateSeq.list[updateSeq.pos++];
        if (updateSeq.pos >= updateSeq.list.length) updateSeq = null;
        showToast(resolveGag(e, ctx), 4500);
        return;
    }

    // The intro is a one-shot sequence that leads.
    if (!updateIntroDone) {
        updateIntroDone = true;
        if (UPDATE_INTRO.length > 1) updateSeq = { list: UPDATE_INTRO, pos: 1 };
        showToast(resolveGag(UPDATE_INTRO[0], ctx), 4500);
        return;
    }

    // A real, saved achievement the first time you cross the threshold (>= so a
    // sequence that straddled the mark can't make it miss).
    if (updatePresses >= UPDATE_ACHIEVEMENT_AT && !updateAchUnlocked()) {
        try { localStorage.setItem('bt_ach_update', '1'); } catch (_) {}
        showToast('🏆 Achievement Unlocked — "Pressed UPDATE more than anyone reasonably should"', 6000);
        return;
    }

    // Random draw from the pool, skipping threshold-gated entries and any one-shot
    // sequence that has already run.
    let i = -1;
    for (let tries = 0; tries < 30; tries++) {
        const j = Math.floor(Math.random() * UPDATE_GAGS.length);
        const e = UPDATE_GAGS[j];
        if (e && e.after && updatePresses < e.after) continue;
        if (e && e.seq && updatePlayedSeq.has(j)) continue;
        i = j; break;
    }
    if (i < 0) { // everything eligible was gated/spent — fall back to a plain entry
        i = UPDATE_GAGS.findIndex((e) => !(e && (e.seq || e.after)));
        if (i < 0) i = 0;
    }
    ctx.idx = i;
    const e = UPDATE_GAGS[i];
    // Start a one-shot sequence: play its first message now, the rest on later presses.
    if (e && e.seq) {
        updatePlayedSeq.add(i);
        updateSeq = { list: e.seq, pos: 1 };
        showToast(resolveGag(e.seq[0], ctx), 4500);
        return;
    }
    showToast(resolveGag(e, ctx), 4500);
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
        showToast(SELF_CHALLENGE_GAGS[Math.floor(Math.random() * SELF_CHALLENGE_GAGS.length)], 4500);
        return;
    }
    if (!await ensureIdentity()) return; // need a name/token first (field is focused)
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'challenge', target: selectedPlayer, name: playerName, token: identityToken }));
        setOnlineStatus(`Challenging ${selectedPlayer}…`);
        if (onlineStatus) onlineStatus.style.display = 'block';
    }
}

// "Open to matches" lives in two places — the lobby and the in-game top bar —
// so keep both checkboxes showing the same state.
function syncAvailableUI(v) {
    if (availableToggle) availableToggle.checked = v;
    if (availableToggleGame) availableToggleGame.checked = v;
}

// Open-to-matches: become challengeable AND eligible for auto-pairing.
async function setAvailable(v) {
    if (v && !await ensureIdentity()) {
        // Can't be challengeable without a signed identity — revert the toggle and
        // leave the name field focused so the player can fix it.
        syncAvailableUI(false);
        return;
    }
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'available', value: v, name: playerName, token: identityToken }));
    }
    syncAvailableUI(v);
}

// An incoming challenge invite (server -> us).
let pendingChallenger = null;
const challengeOverlay = document.getElementById('challengeOverlay');
function onChallenged(from) {
    pendingChallenger = from;
    const t = document.getElementById('challengeText');
    if (t) t.textContent = `${from} challenges you!`;
    if (challengeOverlay) challengeOverlay.classList.add('open');
}
function respondChallenge(accept) {
    if (challengeOverlay) challengeOverlay.classList.remove('open');
    if (!pendingChallenger) return;
    const from = pendingChallenger;
    pendingChallenger = null;
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: accept ? 'challengeAccept' : 'challengeDecline', from }));
    }
    if (accept) { setOnlineStatus(`Accepting ${from}…`); if (onlineStatus) onlineStatus.style.display = 'block'; }
}
async function loadPlayerStats(name) {
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
function formatPlayerStats(p) {
    const row = (label, val) => label.padStart(14) + ': ' + val;
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

function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
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
function boardDisplayScale(bufHeightPx) {
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

const cancelSearchBtn = document.getElementById('cancelSearch');
const playersCountEl = document.getElementById('playersCount');
const hitCounterEl = document.getElementById('hitCounter');
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
    'w', 'a', 's', 'd', 'W', 'A', 'S', 'D',
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
// A single persistent websocket carries everything: the lobby (watch/stats/
// players/presence/challenge) AND the authoritative match (matchStart/snapshot/
// input). One connection per client = the server's model; it also means a
// challenge accepted in the lobby flows straight into a match on the same socket.
let lobbyReconnectTimer = null;
let pendingQueue = false;   // queue for a match as soon as the socket (re)opens
let pendingRejoin = null;   // match_id to reattach to as soon as the socket (re)opens

function connectLobby() {
    // Never open a second socket on top of a live one.
    if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) return;
    const wsProto = location.protocol === 'https:' ? 'wss' : 'ws';
    const sock = new WebSocket(`${wsProto}://${location.host}/ws`);
    ws = sock;
    sock.onopen = () => {
        if (ws !== sock) return; // superseded before it opened
        sock.send(JSON.stringify({ type: 'watch' }));
        // Rejoin takes priority: reattach to a live bout (after a refresh or a brief
        // socket drop) before doing any lobby presence. The server reattaches us and
        // replays matchStart + a keyframe; on failure it sends rejoinFailed.
        if (pendingRejoin !== null) {
            const mid = pendingRejoin; // a tagged-UUID string (match-<uuid>) from the URL
            pendingRejoin = null;
            sock.send(JSON.stringify({ type: 'rejoin', match_id: mid, token: identityToken, name: playerName }));
            return;
        }
        // Re-assert "Open to matches" across a reconnect (else the server forgets).
        if (availableToggle && availableToggle.checked) {
            sock.send(JSON.stringify({ type: 'available', value: true, name: playerName, token: identityToken }));
        }
        // A Find Match requested while the socket was down (e.g. just after a
        // forfeit-leave): send the queue now that we're connected.
        if (pendingQueue) {
            pendingQueue = false;
            sock.send(JSON.stringify({ type: 'queue', name: playerName, token: identityToken, authoritative: true }));
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
        unackedInputs = [];
        if (searching) {
            searching = false;
            findMatchBtn.classList.remove('searching');
            cancelSearchBtn.style.display = 'none';
        }
        ws = null;
        renderOnlineList([]); // clear the roster while disconnected
        clearTimeout(lobbyReconnectTimer);
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
let identityToken = null;

function loadIdentity() {
    try {
        identityToken = localStorage.getItem('bt_token');
        playerName = localStorage.getItem('bt_player_name') || playerName;
    } catch (_) {}
    // Never leave the player nameless: default to a random handle they can edit.
    if (!playerName) {
        playerName = 'player' + Math.floor(Math.random() * 900 + 100);
        try { localStorage.setItem('bt_player_name', playerName); } catch (_) {}
    }
    if (nameInput) nameInput.value = playerName;
}

function setNameHint(msg, isError) {
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
            identityToken = (await res.json()).token;
            try { localStorage.setItem('bt_token', identityToken); } catch (_) {}
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
    identityToken = null;
    try { localStorage.removeItem('bt_token'); } catch (_) {}
    const tok = await ensureIdentity();
    if (tok && availableToggle && availableToggle.checked) setAvailable(true);
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
    if (ws && ws.readyState === WebSocket.OPEN) {
        ws.send(JSON.stringify({ type: 'active' }));
    }
}

// Reset match/search state. Does NOT close the socket — it's the persistent
// lobby connection. (Leaving a match resets here; the server ends the bout when
// the player tops out, the opponent wins, or the socket actually drops.)
function cleanupOnline() {
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
    if (authoritative && authSelf) {
        // Barrier active while EITHER side is still shopping — keep gameplay
        // inputs blocked (and the overlay up) until BOTH players have hit Done.
        return authSelf.in_bazaar || !!(authOpp && authOpp.in_bazaar);
    }
    return game.is_in_bazaar();
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
function enterAuthoritativeGame(msg) {
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
    game = new WasmGame(currentSeed);
    resetMatchState();
    inputSeq = 0;
    unackedInputs = [];
    authSelf = null;
    authOpp = null;
    authSpying = false;
    authSpyBoard = null;
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

function showWin() {
    gameEnded = true;
    gameOverText.textContent = 'YOU WIN!';
    gameOverOverlay.style.display = 'flex';
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
        ws.send(JSON.stringify({ type: 'queue', name: playerName, token: identityToken, authoritative: true }));
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
    if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: 'available', value: false }));
}

// Drop into a fresh online match. Online boards are independent (each player has
// their own seed and exchanges weapons + scores over the data channel), so this
// starts a clean board - the practice / vs-Computer game you played while waiting
// is discarded here.

// Matchmaking-socket message handler: the match handoff (matchStart) + the
// per-frame authoritative state (snapshot) + rating / opponentLeft. Shared by
// the background search and the live authoritative match.
async function onSignalMessage(ev) {
    const msg = JSON.parse(ev.data);

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
function rejoinMatch(id) {
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
    } else {
        game = new WasmGame(seed);
    }

    // Set canvas size based on game dimensions
    const width = game.width();
    const height = game.height();
    canvas.width = width * CELL_SIZE;
    canvas.height = height * CELL_SIZE;

    // Set up AI canvas in vscomputer mode
    if (mode === 'vscomputer') {
        aiGridCanvas.width = width * CELL_SIZE;
        aiGridCanvas.height = height * CELL_SIZE;
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

let lastArsenalSig = null;
function updateArsenalPanel() {
    // Only rebuild the DOM when the arsenal actually changes. Rebuilding it every
    // frame (this runs from the game loop) was destroying the item a user is
    // mid-click on, so the click — which deploys the weapon — never landed.
    let sig = '';
    for (let i = 0; i < 10; i++) sig += game.arsenal_token(i) + ':' + game.arsenal_quantity(i) + ',';
    if (sig === lastArsenalSig) return;
    lastArsenalSig = sig;

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

// Track whether bazaar was open last frame to avoid re-populating every tick.
// The synchronized bazaar BARRIER is server-authoritative online (the bout freezes
// both boards until both players hit Done — the client reads authSelf.in_bazaar);
// in local modes the engine's own flag drives it.
let bazaarWasOpen = false;
// Dev preview override set by ?screen=bazaar&baz=… : 'both' | 'waiting' | 'oppready'.
let debugBazaar = null;

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
    let youShopping, oppShopping;
    if (debugBazaar) {
        // Dev preview: 'waiting' = you've hit Done; 'oppready' = opponent has.
        youShopping = debugBazaar !== 'waiting';
        oppShopping = debugBazaar !== 'oppready';
    } else {
        youShopping = online ? authSelf.in_bazaar : game.is_in_bazaar();
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
            bazaarFunds.textContent = online ? authSelf.funds : game.funds();
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
function updateBazaarBarrierStatus(youShopping, oppShopping) {
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
const debugOverlayEl = document.getElementById('debugOverlay');
let debugOn = new URLSearchParams(location.search).get('debug') === '1';
window.addEventListener('keydown', (e) => {
    if (e.key === '`') {
        debugOn = !debugOn;
        if (!debugOn && debugOverlayEl) debugOverlayEl.style.display = 'none';
    }
});
function updateDebugOverlay() {
    if (!debugOverlayEl) return;
    if (!debugOn || lobbyActive || !game) { debugOverlayEl.style.display = 'none'; return; }
    debugOverlayEl.style.display = '';
    const L = [];
    L.push('▟ DEBUG  (` toggles)');
    L.push(`mode=${mode} auth=${authoritative} ended=${gameEnded}`);
    if (authoritative) L.push(`onlinePaused=${onlinePaused}`);
    if (currentMatchId) L.push(`match=${currentMatchId}`);
    if (currentSeed != null) L.push(`seed=${currentSeed}`);
    L.push(`inputSeq=${inputSeq} unacked=${unackedInputs.length}`);
    try {
        L.push(`local you: score=${game.score()} lines=${game.lines()} funds=${game.funds()} tilBaz=${game.lines_til_bazaar()} baz=${game.is_in_bazaar()} result=${game.result()}`);
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
        const active = [];
        const max = (typeof max_weapons === 'function') ? max_weapons() : 34;
        for (let t = 0; t < max; t++) {
            if (game.weapon_active(t)) active.push(`${weapon_name(t)}(${game.weapon_remaining(t)})`);
        }
        L.push('weapons: ' + (active.length ? active.join(', ') : '—'));
    } catch (_) {}
    debugOverlayEl.textContent = L.join('\n');
}

function gameLoop(now) {
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
    updateDebugOverlay();

    // When an opponent board is visible (vs-Computer Ernie, or an online spy),
    // mark the game-area so the mobile layout puts the two boards side by side
    // (instead of stacked, which buried the weapon buttons).
    if (gameAreaEl) gameAreaEl.classList.toggle('two-boards', aiBoard.style.display === 'block');

    requestAnimationFrame(gameLoop);
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
    // Hard drop: slam to the bottom (the touch equivalent of Space). One-shot.
    setupTouchButton('touchHardDrop', () => predict('BeginDrop'), null);
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
    if (forfeiting && ws && ws.readyState === WebSocket.OPEN) {
        try { ws.send(JSON.stringify({ type: 'leaveMatch' })); } catch (_) {}
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

// Copy a /replay/<id> link to the clipboard (with a visible fallback).
async function shareReplayLink(id) {
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
    if (mode === 'online' || !game || typeof game.export_replay !== 'function') {
        if (lastMatchReplayId) {
            await shareReplayLink(lastMatchReplayId);
        } else {
            showToast('No finished match to share yet.');
        }
        return;
    }
    showToast('Saving replay...', 10000);
    try {
        await shareReplayLink(await uploadReplay(game.export_replay()));
    } catch (e) {
        showToast('Share failed: ' + e.message);
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
window.bt = { get game() { return game; }, get mode() { return mode; } };

// Initialize and start game loop
(async () => {
    // Load identity FIRST: initGame's applyScreenFromUrl may rejoin a match from
    // ?match=<id>, which needs our name + token ready (and populates the name field).
    loadIdentity();

    await initGame();

    // Wire up on-screen touch control buttons
    setupTouchControls();

    // One persistent socket for the lobby (presence/stats/challenge) + matches.
    // (A rejoin from ?match=<id> already opened it; this is a no-op then.)
    connectLobby();

    requestAnimationFrame(gameLoop);
})();
