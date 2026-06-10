// Pure helpers for the spy board reveal's per-frame degradation (the spy static).
//
// The server sends the FULL opponent board while a spy is active; the client hides
// `hidePct`% of the cells each frame, re-rolling which ones, so an inaccurate spy
// reads as flickering static (the original re-rolls shown cells every render). These
// two functions are the whole of that degradation, kept dependency-free so they
// unit-test with `node --test` (see spy-degrade.test.ts). main.ts drives them with
// the real clock, `Math.random`, and a persistent mask it re-rolls on a timer.

/**
 * Build a hide mask over `len` cells: `mask[i] = 1` (hide this cell) with
 * probability `hidePct`%, else 0. `rng` returns a value in [0, 1) and is injectable
 * so a test can pin which cells are hidden. `hidePct <= 0` yields an all-zero mask
 * (Condor: reveal everything); `hidePct >= 100` an all-one mask (blackout).
 */
export function rollSpyMask(len: number, hidePct: number, rng: () => number): Uint8Array {
    const mask = new Uint8Array(len);
    for (let i = 0; i < len; i++) {
        mask[i] = rng() * 100 < hidePct ? 1 : 0;
    }
    return mask;
}

/**
 * Apply a hide mask to a render-id board (empty = -2): a FILLED cell (id !== -2)
 * whose mask bit is set is blanked to -2; empty cells and unmasked cells pass
 * through. Returns a new array; the input board is never mutated. Hiding only ever
 * removes filled cells, so a spy can never invent a cell the opponent does not have.
 */
export function applySpyMask(board: Int32Array | readonly number[], mask: Uint8Array): Int32Array {
    const out = new Int32Array(board.length);
    for (let i = 0; i < board.length; i++) {
        const v = board[i]!;
        out[i] = v !== -2 && mask[i] ? -2 : v;
    }
    return out;
}
