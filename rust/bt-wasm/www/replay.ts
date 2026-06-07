// Replay playback page. Loads a recording by id, reconstructs the game with
// WasmReplayPlayer (deterministic - same seed + inputs => bit-identical), and
// plays it back at the original fixed timestep with play/pause/seek/speed.
import init, { WasmReplayPlayer, WasmVersusReplayPlayer, fixed_dt, weapon_name } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';
import { escapeHtml } from './dom-util.js';
import type { ReplayMeta } from './protocol.js';

// Per-side HUD object the page renders. Returned by `hud(sideA)`; null for the
// AI side of a single-board replay (no funds/arsenal recorded for Ernie).
interface Hud {
    score: number;
    lines: number;
    funds: number;
    linesTil: number;
    inBazaar: boolean;
    arsenal: number[];   // flat [token0, qty0, token1, qty1, …]
    effects: number[];   // flat [tokenIndex, linesRemaining, …]
}

// The common interface BOTH adapters below expose — the single shape the page
// drives, regardless of whether the source is a one-board or two-board replay.
interface PlayerAdapter {
    render_grid(): Int32Array;
    render_ai_grid(): Int32Array;
    has_ai(): boolean;
    width(): number;
    height(): number;
    tick_index(): number;
    tick_count(): number;
    step(): boolean;
    seek(t: number): void;
    result(): number;
    mode(): string;
    seed(): string;
    engine_sha(): string;
    labelA(): string;
    labelB(): string;
    recentLaunches(): number[];
    hasEvents(): boolean;
    inputsAtTick(t: number): string[];
    hud(a: boolean): Hud | null;
}

// Real player names threaded into the Versus replay JSON by the server. The names
// can be absent (older rows), so `| undefined` is explicit — the object is built
// straight from `parsed.name_a`/`name_b` (both `string | undefined`), which
// exactOptionalPropertyTypes requires the type to accept.
interface VersusNames {
    a?: string | undefined;
    b?: string | undefined;
}

// The page reads the small subset of the fetched replay JSON it needs to pick an
// adapter (`seed_a` to detect a two-board recording, `name_a`/`name_b` for the
// labels) from the shared `ReplayMeta` wire shape (protocol.ts).

// Both adapters below expose the SAME interface the page drives:
//   render_grid/render_ai_grid, has_ai, width/height, tick_*, step/seek/result,
//   mode/seed/engine_sha, labelA/labelB, and hud(sideA) -> per-side HUD object
//   { score, lines, funds, linesTil, inBazaar, arsenal[], effects[] } | null.

// Adapt a two-board online (Versus) replay: side A on the main canvas, side B on
// the second canvas. Each side gets a full HUD (both players are real games).
function versusAdapter(vp: WasmVersusReplayPlayer, names: VersusNames): PlayerAdapter {
    const hud = (a: boolean): Hud => ({
        score: a ? vp.score_a() : vp.score_b(),
        lines: a ? vp.lines_a() : vp.lines_b(),
        funds: a ? vp.funds_a() : vp.funds_b(),
        linesTil: a ? vp.lines_til_bazaar_a() : vp.lines_til_bazaar_b(),
        inBazaar: a ? vp.in_bazaar_a() : vp.in_bazaar_b(),
        arsenal: Array.from(a ? vp.arsenal_a() : vp.arsenal_b()),
        effects: Array.from(a ? vp.effects_a() : vp.effects_b()),
    });
    return {
        render_grid: () => vp.render_a(),
        render_ai_grid: () => vp.render_b(),
        has_ai: () => true,
        width: () => vp.width(),
        height: () => vp.height(),
        tick_index: () => vp.tick_index(),
        tick_count: () => vp.tick_count(),
        step: () => vp.step(),
        seek: (t) => vp.seek(t),
        result: () => vp.result(),
        mode: () => 'Online',
        seed: () => 'online',
        engine_sha: () => vp.engine_sha(),
        // Real player names from the DB (threaded into the replay JSON by the
        // server); fall back to the generic labels for older rows with no names.
        labelA: () => (names && names.a) || 'Player A',
        labelB: () => (names && names.b) || 'Player B',
        // Weapon launches from the most recent step(), flat [side, token, …].
        recentLaunches: () => Array.from(vp.recent_launches()),
        hasEvents: () => true,
        // Raw input frames at a tick ("A: MoveLeft", …) for the debug panel.
        inputsAtTick: (t) => Array.from(vp.inputs_at_tick(t)),
        hud,
    };
}

// Adapt a single-board (practice / vs-computer) replay. Side A is the player and
// gets the full HUD; the AI side (Ernie) shows its board only (the recording
// doesn't carry Ernie's funds/arsenal), so hud(false) is null.
function singleAdapter(sp: WasmReplayPlayer): PlayerAdapter {
    const arsenal = (): number[] => {
        const a: number[] = [];
        for (let i = 0; i < 10; i++) { a.push(sp.arsenal_token(i), sp.arsenal_quantity(i)); }
        return a;
    };
    return {
        render_grid: () => sp.render_grid(),
        render_ai_grid: () => sp.render_ai_grid(),
        has_ai: () => sp.has_ai(),
        width: () => sp.width(),
        height: () => sp.height(),
        tick_index: () => sp.tick_index(),
        tick_count: () => sp.tick_count(),
        step: () => sp.step(),
        seek: (t) => sp.seek(t),
        result: () => sp.result(),
        mode: () => sp.mode(),
        seed: () => String(sp.seed()),
        engine_sha: () => sp.engine_sha(),
        labelA: () => 'Player',
        labelB: () => 'Ernie',
        recentLaunches: () => [],
        hasEvents: () => false,
        inputsAtTick: () => [],
        hud: (a) => a ? {
            score: sp.score(), lines: sp.lines(), funds: sp.funds(),
            linesTil: sp.lines_til_bazaar(), inBazaar: sp.is_in_bazaar(),
            arsenal: arsenal(), effects: [],
        } : null,
    };
}

const boards = document.getElementById('replayBoards')!;
const canvas = document.getElementById('replayCanvas') as HTMLCanvasElement;
const ctx = canvas.getContext('2d')!;
const aiBoard = document.getElementById('replayAiBoard')!;
const aiCanvas = document.getElementById('replayAiCanvas') as HTMLCanvasElement;
const aiCtx = aiCanvas.getContext('2d')!;
const labelAEl = document.getElementById('replayLabelA')!;
const labelBEl = document.getElementById('replayLabelB')!;
const hudAEl = document.getElementById('replayHudA')!;
const hudBEl = document.getElementById('replayHudB')!;
const bazaarAEl = document.getElementById('replayBazaarA')!;
const bazaarBEl = document.getElementById('replayBazaarB')!;
const playBtn = document.getElementById('replayPlay')!;
const restartBtn = document.getElementById('replayRestart')!;
const seek = document.getElementById('replaySeek') as HTMLInputElement;
const tickLabel = document.getElementById('replayTick')!;
const speedSel = document.getElementById('replaySpeed') as HTMLSelectElement;
const metaEl = document.getElementById('replayMeta')!;
const errBox = document.getElementById('replayError')!;
const eventLogEl = document.getElementById('replayEventLog');
const eventLogBodyEl = document.getElementById('replayEventLogBody')!;
const debugEl = document.getElementById('replayDebug');
const debugInputsEl = document.getElementById('replayDebugInputs');
const debugTickEl = document.getElementById('replayDebugTick');
const stepBtn = document.getElementById('replayStep');
const jumpInput = document.getElementById('replayJump') as HTMLInputElement | null;
const goBtn = document.getElementById('replayGo');
const copyBtn = document.getElementById('replayCopy');
const debugMode = new URLSearchParams(location.search).get('debug') === '1';

let player: PlayerAdapter | null = null;
let playing = false;
let FIXED_DT = 16;
let accum = 0;
let lastT = 0;

function showError(msg: string): void {
    errBox.style.display = '';
    errBox.textContent = msg;
}

function idFromUrl(): string | null {
    const q = new URLSearchParams(location.search).get('id');
    if (q) return q;
    // Replay ids are a 16-char hex content hash (bt-server `replay_id` =
    // `format!("{:016x}", …)`, enforced hex-only by `valid_replay_id`), so the
    // hex-only capture is exact — no hyphens/UUIDs to truncate. Confirmed against
    // bt-server/src/main.rs.
    const m = location.pathname.match(/\/replay\/([0-9a-fA-F]+)/);
    return m ? m[1] : null;
}

function sizeCanvas(c: HTMLCanvasElement, w: number, h: number, scale: number): void {
    c.width = w * CELL_SIZE;
    c.height = h * CELL_SIZE;
    c.style.width = (w * CELL_SIZE * scale) + 'px';
    c.style.height = (h * CELL_SIZE * scale) + 'px';
}

// Both boards always render at the SAME scale, each with its HUD panel beside it
// (board left, score/arsenal right — like the original playfield). Two
// board+HUD columns sit side-by-side on a wide viewport, stacked on a narrow one.
const HUD_W = 168;        // px reserved for the HUD panel beside a board
const COL_GAP = 18;       // gap between the two columns

function layoutBoards(): void {
    if (!player) return;
    const w = player.width(), h = player.height();
    const boardW = w * CELL_SIZE, boardH = h * CELL_SIZE;   // native px
    const twoUp = player.has_ai();
    const vw = Math.min(window.innerWidth, document.documentElement.clientWidth || window.innerWidth);
    const vh = window.innerHeight || 800;
    const stacked = twoUp && vw < 760;
    boards.classList.toggle('stacked', stacked);

    // Width budget: how many board+HUD columns sit across the viewport.
    const cols = (twoUp && !stacked) ? 2 : 1;
    const widthForBoards = (vw - 32 - COL_GAP * (cols - 1)) / cols - HUD_W - 10;
    const widthScale = widthForBoards / boardW;

    // Height budget (side-by-side only): keep one board + the title/controls/meta
    // in view. Stacked (mobile) is expected to scroll, so width drives the scale.
    const heightScale = stacked ? Infinity : (vh - 300) / boardH;

    let scale = Math.min(widthScale, heightScale);
    scale = Math.max(0.5, Math.min(1.4, scale));
    sizeCanvas(canvas, w, h, scale);
    if (twoUp) sizeCanvas(aiCanvas, w, h, scale);   // EQUAL scale — same as side A
}

function renderHud(el: HTMLElement, h: Hud | null): void {
    if (!h) { el.style.display = 'none'; el.innerHTML = ''; return; }
    el.style.display = '';

    const effects: string[] = [];
    for (let i = 0; i + 1 < h.effects.length; i += 2) {
        effects.push(`<div><span>${weapon_name(h.effects[i])}</span><b>${h.effects[i + 1]}</b></div>`);
    }
    const slots: string[] = [];
    for (let i = 0; i < 10 && i * 2 + 1 < h.arsenal.length; i++) {
        const tok = h.arsenal[i * 2], qty = h.arsenal[i * 2 + 1];
        const n = (i + 1) % 10;
        slots.push(tok >= 0
            ? `<div>${n}. ${weapon_name(tok)}${qty > 1 ? ' &times;' + qty : ''}</div>`
            : `<div class="rh-empty">${n}. &lt; Empty &gt;</div>`);
    }

    el.innerHTML =
        `<div class="rh-stats">` +
        `<div><span>Score</span><b>${h.score}</b></div>` +
        `<div><span>Lines</span><b>${h.lines}</b></div>` +
        `<div><span>Funds</span><b>$${h.funds}</b></div>` +
        `<div><span>'Til Bazaar</span><b>${h.linesTil}</b></div>` +
        `</div>` +
        (effects.length ? `<div class="rh-list rh-effects"><div class="rh-h">Effects</div>${effects.join('')}</div>` : '') +
        `<div class="rh-list rh-arsenal"><div class="rh-h">Arsenal</div>${slots.join('')}</div>`;
}

// Append the weapon launches from the latest step() to the scrolling log. Called
// once per simulated step (not per rendered frame) so nothing is missed when
// playback advances several ticks between frames at high speed.
function captureLaunches(): void {
    if (!player || !player.hasEvents || !player.hasEvents()) return;
    const l = player.recentLaunches();
    if (!l || l.length === 0) return;
    const tick = player.tick_index();
    for (let i = 0; i + 1 < l.length; i += 2) {
        const side = l[i], token = l[i + 1];
        const who = side === 0 ? player.labelA() : player.labelB();
        const row = document.createElement('div');
        row.className = 'rel-row rel-side-' + side;
        row.innerHTML = `<span class="rel-t">${tick}</span> <span class="rel-who">${escapeHtml(who)}</span> launched <b>${escapeHtml(weapon_name(token))}</b>`;
        eventLogBodyEl.appendChild(row);
    }
    while (eventLogBodyEl.childElementCount > 200) eventLogBodyEl.removeChild(eventLogBodyEl.firstChild!);
    eventLogBodyEl.scrollTop = eventLogBodyEl.scrollHeight;
}

// The log builds during real-time forward play; a seek/restart jumps the sim
// (no per-step launches captured), so reset it to avoid a misleading partial log.
function clearEventLog(): void {
    if (eventLogBodyEl) eventLogBodyEl.innerHTML = '';
}

// ── Replay debug controls (?debug=1) ─────────────────────────────────────────
function updateInputStream(): void {
    if (!debugMode || !debugInputsEl || !player) return;
    const tick = player.tick_index();
    if (debugTickEl) debugTickEl.textContent = String(tick);
    const inputs = player.inputsAtTick ? player.inputsAtTick(tick) : [];
    debugInputsEl.textContent = inputs.length ? inputs.join('    ') : '—';
}
function stepOne(): void {
    if (!player) return;
    setPlaying(false);
    if (player.step()) captureLaunches();
    renderFrame();
}
function jumpTo(t: number): void {
    if (!player) return;
    setPlaying(false);
    player.seek(Math.max(0, Math.min(player.tick_count(), t | 0)));
    clearEventLog();
    renderFrame();
}
async function copyState(): Promise<void> {
    if (!player) return;
    const state = {
        tick: player.tick_index(), tick_count: player.tick_count(), result: player.result(),
        a: player.hud(true), b: player.hud(false),
        inputs: player.inputsAtTick ? player.inputsAtTick(player.tick_index()) : [],
    };
    try { await navigator.clipboard.writeText(JSON.stringify(state, null, 2)); } catch (_) {}
    if (copyBtn) { copyBtn.textContent = 'Copied!'; setTimeout(() => { copyBtn.textContent = 'Copy state'; }, 1200); }
}

function renderFrame(): void {
    if (!player) return;
    drawBoard(ctx, player.render_grid(), player.width(), player.height());
    const ha = player.hud(true);
    renderHud(hudAEl, ha);
    bazaarAEl.style.display = (ha && ha.inBazaar) ? '' : 'none';

    if (player.has_ai()) {
        drawBoard(aiCtx, player.render_ai_grid(), player.width(), player.height());
        const hb = player.hud(false);
        renderHud(hudBEl, hb);
        bazaarBEl.style.display = (hb && hb.inBazaar) ? '' : 'none';
    }
    seek.value = String(player.tick_index());
    tickLabel.textContent = `${player.tick_index()} / ${player.tick_count()}`;
    if (debugMode) updateInputStream();
}

function setPlaying(p: boolean): void {
    playing = p;
    playBtn.innerHTML = p ? '&#9208; Pause' : '&#9654; Play';
    if (p) { lastT = 0; accum = 0; }
}

function loop(now: number): void {
    if (player && playing) {
        if (lastT === 0) lastT = now;
        let dt = now - lastT;
        lastT = now;
        if (dt > 250) dt = 250;
        const speed = parseFloat(speedSel.value) || 1;
        accum += dt * speed;
        let advanced = false;
        let steps = 0;
        while (accum >= FIXED_DT && steps < 4000) {
            accum -= FIXED_DT;
            steps++;
            advanced = true;
            if (!player.step()) { setPlaying(false); accum = 0; break; }
            captureLaunches();
        }
        if (advanced) renderFrame();
    }
    requestAnimationFrame(loop);
}

function resultText(r: number, mode: string): string {
    if (mode === 'Online') {
        if (r === 1) return ' · side A won';
        if (r === 2) return ' · side B won';
        return '';
    }
    if (mode !== 'VsComputer') return '';
    if (r === 1) return ' · player won';
    if (r === 2) return ' · player lost';
    return '';
}

(async () => {
    const id = idFromUrl();
    await init();
    FIXED_DT = fixed_dt();

    if (!id) { showError('No replay id in the URL.'); return; }

    let text: string;
    try {
        const res = await fetch(`/api/replays/${id}`);
        if (!res.ok) throw new Error(`server returned ${res.status}`);
        text = await res.text();
    } catch (e) {
        showError('Could not load replay: ' + (e as Error).message);
        return;
    }

    try {
        // An online match recording carries two seeds (seed_a/seed_b); play it
        // with the two-board Versus player. Everything else is a single-board game.
        // The server threads the real player names (name_a/name_b) into the JSON.
        let parsed: ReplayMeta = {};
        try { parsed = JSON.parse(text) || {}; } catch (_) {}
        const isVersus = parsed.seed_a !== undefined;
        player = isVersus
            ? versusAdapter(WasmVersusReplayPlayer.from_json(text), { a: parsed.name_a, b: parsed.name_b })
            : singleAdapter(WasmReplayPlayer.from_json(text));
    } catch (e) {
        showError('This replay is invalid or from an incompatible engine build.');
        return;
    }

    labelAEl.textContent = player.labelA();
    labelBEl.textContent = player.labelB();
    if (player.has_ai()) aiBoard.style.display = '';
    if (player.hasEvents && player.hasEvents() && eventLogEl) eventLogEl.style.display = '';
    layoutBoards();
    window.addEventListener('resize', layoutBoards);
    seek.max = String(player.tick_count());

    // Determine the final outcome once (it's only known at the end), then rewind.
    player.seek(player.tick_count());
    const finalResult = player.result();
    player.seek(0);

    metaEl.textContent =
        `mode: ${player.mode()} · seed: ${player.seed()} · ticks: ${player.tick_count()} · engine: ${player.engine_sha()}${resultText(finalResult, player.mode())}`;
    renderFrame();

    playBtn.addEventListener('click', () => {
        if (!player) return;
        if (player.tick_index() >= player.tick_count()) player.seek(0);
        setPlaying(!playing);
    });
    restartBtn.addEventListener('click', () => {
        if (!player) return;
        player.seek(0);
        setPlaying(false);
        clearEventLog();
        renderFrame();
    });
    seek.addEventListener('input', () => {
        if (!player) return;
        player.seek(parseInt(seek.value, 10) || 0);
        setPlaying(false);
        clearEventLog();
        renderFrame();
    });

    // Debug controls (?debug=1): single-step, jump-to-tick, copy-state.
    if (debugMode && debugEl) {
        debugEl.style.display = '';
        if (jumpInput) jumpInput.max = String(player.tick_count());
        if (stepBtn) stepBtn.addEventListener('click', stepOne);
        if (goBtn) goBtn.addEventListener('click', () => jumpTo(parseInt(jumpInput!.value, 10) || 0));
        if (jumpInput) jumpInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') jumpTo(parseInt(jumpInput.value, 10) || 0); });
        if (copyBtn) copyBtn.addEventListener('click', copyState);
        updateInputStream();
    }

    requestAnimationFrame(loop);
})();
