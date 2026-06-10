// Unit tests for the spy board reveal's per-frame degradation (the spy static).
//
// Run with:  npm run test:unit
//   -> node --test --experimental-strip-types www/spy-degrade.test.ts
// No build step and no dependency: Node strips the TS types and spy-degrade.ts
// imports nothing external. The `.ts` extension is what type-stripping resolves.
//
// What broke without coverage: the spy reveal was added straight in the render
// loop, so a canvas-sizing bug shipped (the opponent board rendered black online).
// The degradation logic itself is pure and is what these tests pin: a spy hides
// some of the opponent's cells, never invents cells, and never blanks an empty.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { rollSpyMask, applySpyMask } from './spy-degrade.ts';

test('applySpyMask blanks only masked filled cells, leaving empties and unmasked cells', () => {
    const board = [5, -2, 7, 9, -2];
    const mask = Uint8Array.from([1, 1, 0, 1, 0]);
    const out = applySpyMask(board, mask);
    // cell 0: filled + masked -> hidden; cell 1: empty stays empty; cell 2: filled
    // unmasked -> kept; cell 3: filled + masked -> hidden; cell 4: empty stays empty.
    assert.deepEqual(Array.from(out), [-2, -2, 7, -2, -2]);
});

test('applySpyMask never mutates the input board', () => {
    const board = [5, -2, 7, 9];
    applySpyMask(board, Uint8Array.from([1, 1, 1, 1]));
    assert.deepEqual(board, [5, -2, 7, 9], 'the source board is untouched');
});

test('applySpyMask with an all-zero mask reveals the whole board (Condor)', () => {
    const board = [5, -2, 7, 9, 3];
    const out = applySpyMask(board, new Uint8Array(board.length));
    assert.deepEqual(Array.from(out), board);
});

test('applySpyMask can never invent a cell the opponent does not have', () => {
    // An all-ones mask is the worst case: every output cell is either the original
    // (empty) or -2 (hidden). No empty (-2) cell can become filled.
    const board = [-2, 4, -2, 8, -2];
    const out = applySpyMask(board, Uint8Array.from([1, 1, 1, 1, 1]));
    for (let i = 0; i < board.length; i++) {
        if (board[i] === -2) assert.equal(out[i], -2, `empty cell ${i} must stay empty`);
        else assert.equal(out[i], -2, `filled cell ${i} is hidden under a full mask`);
    }
});

test('rollSpyMask hides everything at 100% and nothing at 0%', () => {
    const r = () => 0.5; // 0.5 * 100 = 50
    assert.deepEqual(Array.from(rollSpyMask(4, 100, r)), [1, 1, 1, 1], '50 < 100 -> all hidden');
    assert.deepEqual(Array.from(rollSpyMask(4, 0, r)), [0, 0, 0, 0], '50 < 0 is false -> none hidden');
});

test('rollSpyMask hides a cell iff rng()*100 < hidePct', () => {
    // rng cycles 0.10, 0.60 -> *100 = 10, 60. At hidePct=50: 10<50 hide, 60<50 show.
    let i = 0;
    const r = () => [0.10, 0.60][i++ % 2]!;
    assert.deepEqual(Array.from(rollSpyMask(2, 50, r)), [1, 0]);
});

test('rollSpyMask hides about hidePct of the cells over a large board', () => {
    // A small deterministic LCG for a uniform-ish [0,1); no real randomness needed.
    let s = 12345;
    const r = () => {
        s = (s * 1103515245 + 12345) & 0x7fffffff;
        return (s % 1000) / 1000;
    };
    const mask = rollSpyMask(5000, 50, r);
    const hidden = mask.reduce((a, b) => a + b, 0);
    const pct = (hidden / mask.length) * 100;
    assert.ok(pct > 40 && pct < 60, `Ames (~50%) should hide ~half; got ${pct.toFixed(1)}%`);
});
