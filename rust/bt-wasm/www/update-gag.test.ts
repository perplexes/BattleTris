// Unit tests for the UPDATE-button gag sequencer (the pure `nextGag`).
//
// Run with:  npm run test:unit
//   → node --test --experimental-strip-types www/update-gag.test.ts
// No build step and no new dependency: Node strips the TS types in-process and the
// module under test imports nothing external. The import below uses the explicit
// `.ts` extension because that's what type-stripping resolves (a `.js` specifier
// would point at a file that doesn't exist when running straight from source).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
    nextGag, initialGagState, UPDATE_INTRO, UPDATE_GAGS, UPDATE_ACHIEVEMENT_AT,
    type GagState, type GagEntry,
} from './update-gag.ts';

// A deterministic env: fixed clock, and an rng we can point at a chosen pool index.
function envAt(index: number) {
    // nextGag draws `Math.floor(rng() * UPDATE_GAGS.length)`, so to land on `index`
    // return a value that floors to it.
    return { now: 0, rng: () => (index + 0.5) / UPDATE_GAGS.length };
}

// Drive past the one-shot intro so the pool is in play, returning the live state.
function skipIntro(s: GagState): GagState {
    let state = s;
    for (let i = 0; i < UPDATE_INTRO.length; i++) {
        state = nextGag(state, { now: 0, rng: () => 0 }).state;
    }
    return state;
}

// Find a pool index whose entry is a one-shot `{seq}`.
function seqIndex(): number {
    return UPDATE_GAGS.findIndex((e) => typeof e === 'object' && 'seq' in e);
}
// Find a pool index whose entry is threshold-gated (`{after, fn}`).
function gatedIndex(): { idx: number; after: number } {
    const idx = UPDATE_GAGS.findIndex((e) => typeof e === 'object' && 'after' in e);
    const e = UPDATE_GAGS[idx] as Extract<GagEntry, { after: number }>;
    return { idx, after: e.after };
}

test('intro plays once, in order, then never recurs', () => {
    let state = initialGagState(false);
    // Every line of the intro, in order.
    for (let i = 0; i < UPDATE_INTRO.length; i++) {
        const r = nextGag(state, { now: 0, rng: () => 0 });
        assert.equal(r.text, UPDATE_INTRO[i], `intro line ${i}`);
        state = r.state;
    }
    // The intro is exhausted: the next press draws from the pool, not the intro.
    // rng→0 lands on pool index 0, which is a plain string (the first pool entry).
    const after = nextGag(state, { now: 0, rng: () => 0 });
    assert.equal(after.text, UPDATE_GAGS[0]);
    assert.ok(!(UPDATE_INTRO as string[]).includes(after.text), 'never re-enters the intro');
});

test('a one-shot {seq} plays whole, in order, and never re-enters', () => {
    const si = seqIndex();
    assert.ok(si >= 0, 'the pool has a one-shot sequence');
    const seqList = (UPDATE_GAGS[si] as { seq: string[] }).seq;

    let state = skipIntro(initialGagState(false));

    // Land on the sequence: its FIRST line shows now, and it's marked as played.
    const first = nextGag(state, envAt(si));
    assert.equal(first.text, seqList[0], 'sequence opens with its first line');
    assert.ok(first.state.playedSeq.has(si), 'sequence index recorded as played');
    state = first.state;

    // The rest of the sequence plays in order REGARDLESS of the rng (an in-progress
    // sequence ignores the pool draw entirely).
    for (let i = 1; i < seqList.length; i++) {
        const r = nextGag(state, envAt(si)); // rng still points at the seq entry
        assert.equal(r.text, seqList[i], `sequence line ${i} in order`);
        state = r.state;
    }
    assert.equal(state.seq, null, 'sequence finished');

    // Re-landing on the same pool index must NOT replay it: the draw loop skips a
    // played sequence and (with rng pinned) falls back to a plain entry.
    const replay = nextGag(state, envAt(si));
    assert.notEqual(replay.text, seqList[0], 'a spent one-shot sequence never re-enters');
});

test('the achievement fires exactly once at the threshold', () => {
    // Start already at the threshold-minus-one so the next press crosses it. The
    // intro must be done and we must not be mid-sequence; build that state directly.
    let state: GagState = {
        presses: UPDATE_ACHIEVEMENT_AT - 1,
        firstMs: 1,
        seq: null,
        introDone: true,
        playedSeq: new Set<number>(),
        achUnlocked: false,
    };

    const cross = nextGag(state, envAt(0));
    assert.ok(cross.unlockedAchievement, 'crossing the threshold unlocks the achievement');
    assert.match(cross.text, /Achievement Unlocked/, 'shows the trophy toast');
    assert.equal(cross.state.achUnlocked, true, 'state records the unlock');
    state = cross.state;

    // The very next press does NOT re-fire it (it draws a normal gag instead).
    const again = nextGag(state, envAt(0));
    assert.equal(again.unlockedAchievement, false, 'achievement never fires twice');
    assert.doesNotMatch(again.text, /Achievement Unlocked/);

    // And a fresh state that's already unlocked never fires it even at the threshold.
    const preUnlocked: GagState = { ...state, presses: UPDATE_ACHIEVEMENT_AT - 1, achUnlocked: true };
    const r = nextGag(preUnlocked, envAt(0));
    assert.equal(r.unlockedAchievement, false, 'an already-unlocked achievement stays silent');
});

test('a threshold-gated line is skipped below its threshold and shown past it', () => {
    const { idx, after } = gatedIndex();
    const gated = UPDATE_GAGS[idx] as Extract<GagEntry, { after: number }>;

    // BELOW the threshold: landing the rng on the gated index must NOT produce its
    // text — the draw loop skips it and falls back to a plain entry.
    const below: GagState = {
        presses: after - 5, firstMs: 1, seq: null, introDone: true,
        playedSeq: new Set<number>(), achUnlocked: true, // unlocked so the achievement can't interfere
    };
    const belowText = gated.fn({ n: after - 5 + 1, elapsed: 0, idx });
    const rBelow = nextGag(below, envAt(idx));
    assert.notEqual(rBelow.text, belowText, 'gated line not shown below its threshold');

    // PAST the threshold: the same draw now yields the gated line.
    const past: GagState = { ...below, presses: after + 5 };
    const pastN = after + 5 + 1;
    const pastText = gated.fn({ n: pastN, elapsed: 0, idx });
    const rPast = nextGag(past, envAt(idx));
    assert.equal(rPast.text, pastText, 'gated line appears once past its threshold');
});
