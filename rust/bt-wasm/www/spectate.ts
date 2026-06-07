// Live-match spectator (debug view). Connects to /ws, subscribes to a bout with
// {type:"spectate",match_id}, and renders the server's read-only two-board frames.
// No prediction, no input — purely a viewer (the live counterpart to replay.js).
import init, { weapon_name } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';
import { escapeHtml } from './dom-util.js';

const $ = (id: string): HTMLElement => document.getElementById(id)!;
const errBox = $('spectateError');
const matchListEl = $('matchList');
const matchListBodyEl = $('matchListBody');
const boardsEl = $('spectateBoards');
const labelAEl = $('specLabelA'), labelBEl = $('specLabelB');
const hudAEl = $('specHudA'), hudBEl = $('specHudB');
const bazaarAEl = $('specBazaarA'), bazaarBEl = $('specBazaarB');
const canvasA = $('specCanvasA') as HTMLCanvasElement, canvasB = $('specCanvasB') as HTMLCanvasElement;
const ctxA = canvasA.getContext('2d')!, ctxB = canvasB.getContext('2d')!;
const metaEl = $('spectateMeta');

// One side of a spectate frame: the read-only HUD + board state for one player.
interface SpectateSide {
    score: number;
    lines: number;
    funds: number;
    lines_til: number;
    arsenal: number[]; // flat [token, qty, …]
    effects: number[]; // flat [token, linesRemaining, …]
    board: number[];   // flat row-major cell ids
    in_bazaar: boolean;
}

// A {type:"spectate"} frame: both boards plus match metadata.
interface SpectateFrame {
    type: 'spectate';
    w: number;
    h: number;
    name_a?: string;
    name_b?: string;
    a: SpectateSide;
    b: SpectateSide;
    result: number; // 0 = ongoing, 1 = A won, 2 = B won
    tick: number;
}

// Any frame read off the socket; only `type` is guaranteed before narrowing.
type ServerMessage = SpectateFrame | { type: 'spectateFailed' } | { type: string };

// One in-progress match as returned by /api/debug/matches.
interface MatchSummary {
    match_id: string;
    name_a?: string;
    name_b?: string;
}

function showError(msg: string): void { errBox.textContent = msg; errBox.style.display = ''; }

let boardW = 0, boardH = 0;
function fitCanvases(): void {
    if (!boardW) return;
    const vh = window.innerHeight || 800;
    let scale = (vh - 320) / (boardH * CELL_SIZE);
    scale = Math.max(0.5, Math.min(1.2, scale));
    for (const c of [canvasA, canvasB]) {
        c.style.width = (boardW * CELL_SIZE * scale) + 'px';
        c.style.height = (boardH * CELL_SIZE * scale) + 'px';
        c.style.imageRendering = 'pixelated';
    }
}

// HUD: mirrors the replay viewer's renderHud, fed by a spectate frame's side obj
// ({score, lines, funds, lines_til, arsenal[], effects[]}).
function renderHud(el: HTMLElement, s: SpectateSide): void {
    const effects: string[] = [];
    for (let i = 0; i + 1 < s.effects.length; i += 2) {
        effects.push(`<div><span>${weapon_name(s.effects[i])}</span><b>${s.effects[i + 1]}</b></div>`);
    }
    const slots: string[] = [];
    for (let i = 0; i < 10 && i * 2 + 1 < s.arsenal.length; i++) {
        const tok = s.arsenal[i * 2], qty = s.arsenal[i * 2 + 1];
        const n = (i + 1) % 10;
        slots.push(tok >= 0
            ? `<div>${n}. ${weapon_name(tok)}${qty > 1 ? ' &times;' + qty : ''}</div>`
            : `<div class="rh-empty">${n}. &lt; Empty &gt;</div>`);
    }
    el.innerHTML =
        `<div class="rh-stats">` +
        `<div><span>Score</span><b>${s.score}</b></div>` +
        `<div><span>Lines</span><b>${s.lines}</b></div>` +
        `<div><span>Funds</span><b>$${s.funds}</b></div>` +
        `<div><span>'Til Bazaar</span><b>${s.lines_til}</b></div>` +
        `</div>` +
        (effects.length ? `<div class="rh-list rh-effects"><div class="rh-h">Effects</div>${effects.join('')}</div>` : '') +
        `<div class="rh-list rh-arsenal"><div class="rh-h">Arsenal</div>${slots.join('')}</div>`;
}

function renderFrame(m: SpectateFrame): void {
    if (boardW !== m.w || boardH !== m.h) {
        boardW = m.w; boardH = m.h;
        canvasA.width = boardW * CELL_SIZE; canvasA.height = boardH * CELL_SIZE;
        canvasB.width = boardW * CELL_SIZE; canvasB.height = boardH * CELL_SIZE;
        fitCanvases();
    }
    labelAEl.textContent = m.name_a || 'Player A';
    labelBEl.textContent = m.name_b || 'Player B';
    drawBoard(ctxA, m.a.board, m.w, m.h);
    drawBoard(ctxB, m.b.board, m.w, m.h);
    renderHud(hudAEl, m.a);
    renderHud(hudBEl, m.b);
    bazaarAEl.style.display = m.a.in_bazaar ? '' : 'none';
    bazaarBEl.style.display = m.b.in_bazaar ? '' : 'none';
    const res = m.result === 1 ? ' · A won' : m.result === 2 ? ' · B won' : '';
    metaEl.textContent = `🔴 LIVE · tick ${m.tick}${res}`;
}

// ── Live-matches picker ───────────────────────────────────────────────────
async function loadMatchList(): Promise<void> {
    matchListEl.style.display = '';
    boardsEl.style.display = 'none';
    let matches: MatchSummary[] = [];
    try { matches = (await (await fetch('/api/debug/matches')).json()).matches || []; } catch (_) {}
    matchListBodyEl.innerHTML = matches.length
        ? matches.map((m) =>
            `<a class="spectate-row" href="?match=${encodeURIComponent(m.match_id)}">` +
            `<span class="spectate-vs">${escapeHtml(m.name_a || '?')} <b>vs</b> ${escapeHtml(m.name_b || '?')}</span>` +
            `<span class="spectate-watch">Watch &#9654;</span></a>`).join('')
        : '<div class="spectate-empty">No matches in progress.</div>';
}

// ── Spectate a match ──────────────────────────────────────────────────────
function spectate(matchId: string): void {
    matchListEl.style.display = 'none';
    boardsEl.style.display = '';
    metaEl.textContent = 'Connecting…';
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${location.host}/ws`);
    ws.onopen = () => ws.send(JSON.stringify({ type: 'spectate', match_id: matchId }));
    ws.onmessage = (ev: MessageEvent) => {
        let m: ServerMessage; try { m = JSON.parse(ev.data); } catch (_) { return; }
        if (m.type === 'spectate') renderFrame(m as SpectateFrame);
        else if (m.type === 'spectateFailed') {
            showError('That match is no longer live.');
            metaEl.textContent = '';
            setTimeout(() => { location.href = '/www/spectate.html'; }, 1500);
        }
    };
    ws.onclose = () => { if (metaEl.textContent!.startsWith('🔴')) metaEl.textContent += ' · stream ended'; };
}

window.addEventListener('resize', fitCanvases);

(async () => {
    await init();
    const id = new URLSearchParams(location.search).get('match');
    if (id) {
        spectate(id);
    } else {
        loadMatchList();
        setInterval(loadMatchList, 4000);
    }
})();
