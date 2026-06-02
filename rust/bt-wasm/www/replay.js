// Replay playback page. Loads a recording by id, reconstructs the game with
// WasmReplayPlayer (deterministic — same seed + inputs => bit-identical), and
// plays it back at the original fixed timestep with play/pause/seek/speed.
import init, { WasmReplayPlayer, fixed_dt } from '../pkg/bt_wasm.js';
import { CELL_SIZE, drawBoard } from './render.js';

const canvas = document.getElementById('replayCanvas');
const ctx = canvas.getContext('2d');
const aiBoard = document.getElementById('replayAiBoard');
const aiCanvas = document.getElementById('replayAiCanvas');
const aiCtx = aiCanvas.getContext('2d');
const playBtn = document.getElementById('replayPlay');
const restartBtn = document.getElementById('replayRestart');
const seek = document.getElementById('replaySeek');
const tickLabel = document.getElementById('replayTick');
const speedSel = document.getElementById('replaySpeed');
const metaEl = document.getElementById('replayMeta');
const errBox = document.getElementById('replayError');

let player = null;
let playing = false;
let FIXED_DT = 16;
let accum = 0;
let lastT = 0;

function showError(msg) {
    errBox.style.display = '';
    errBox.textContent = msg;
}

function idFromUrl() {
    const q = new URLSearchParams(location.search).get('id');
    if (q) return q;
    const m = location.pathname.match(/\/replay\/([0-9a-fA-F]+)/);
    return m ? m[1] : null;
}

function sizeCanvas(c, w, h, scale) {
    c.width = w * CELL_SIZE;
    c.height = h * CELL_SIZE;
    c.style.width = (w * CELL_SIZE * scale) + 'px';
    c.style.height = (h * CELL_SIZE * scale) + 'px';
}

function renderFrame() {
    drawBoard(ctx, player.render_grid(), player.width(), player.height());
    if (player.has_ai()) {
        drawBoard(aiCtx, player.render_ai_grid(), player.width(), player.height());
    }
    seek.value = player.tick_index();
    tickLabel.textContent = `${player.tick_index()} / ${player.tick_count()}`;
}

function setPlaying(p) {
    playing = p;
    playBtn.innerHTML = p ? '&#9208; Pause' : '&#9654; Play';
    if (p) { lastT = 0; accum = 0; }
}

function loop(now) {
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
        }
        if (advanced) renderFrame();
    }
    requestAnimationFrame(loop);
}

function resultText(r, mode) {
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

    let text;
    try {
        const res = await fetch(`/api/replays/${id}`);
        if (!res.ok) throw new Error(`server returned ${res.status}`);
        text = await res.text();
    } catch (e) {
        showError('Could not load replay: ' + e.message);
        return;
    }

    try {
        player = WasmReplayPlayer.from_json(text);
    } catch (e) {
        showError('This replay is invalid or from an incompatible engine build.');
        return;
    }

    const w = player.width();
    const h = player.height();
    sizeCanvas(canvas, w, h, 1.4);
    if (player.has_ai()) {
        aiBoard.style.display = '';
        sizeCanvas(aiCanvas, w, h, 1.0);
    }
    seek.max = player.tick_count();

    // Determine the final outcome once (it's only known at the end), then rewind.
    player.seek(player.tick_count());
    const finalResult = player.result();
    player.seek(0);

    metaEl.textContent =
        `mode: ${player.mode()} · seed: ${player.seed()} · ticks: ${player.tick_count()} · engine: ${player.engine_sha()}${resultText(finalResult, player.mode())}`;
    renderFrame();

    playBtn.addEventListener('click', () => {
        if (player.tick_index() >= player.tick_count()) player.seek(0);
        setPlaying(!playing);
    });
    restartBtn.addEventListener('click', () => {
        player.seek(0);
        setPlaying(false);
        renderFrame();
    });
    seek.addEventListener('input', () => {
        player.seek(parseInt(seek.value, 10) || 0);
        setPlaying(false);
        renderFrame();
    });

    requestAnimationFrame(loop);
})();
