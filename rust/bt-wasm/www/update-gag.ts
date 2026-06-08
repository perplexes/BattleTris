// The UPDATE-button gag sequencer, extracted from main.ts so it can be unit-tested
// in isolation. The 1994 client's UPDATE button PULLED the lobby roster; ours is
// PUSHED live over the websocket, so the button has no work to do — instead it
// tells a one-shot escalating joke, then draws from a pool of one-liners.
//
// `nextGag` is PURE: it takes the current state + a context and returns the toast
// to show and the NEW state. All I/O (the actual showToast call, localStorage
// persistence of the achievement, the clock, and Math.random) stays in main.ts and
// is injected via the context, so the same call is deterministic in a test.

// Context passed to a dynamic gag (the values it can interpolate).
export interface GagCtx { n: number; elapsed: number; idx: number; }

// A gag entry: a plain string, a dynamic function, a threshold-gated function, or
// a one-shot sequence. The union is discriminated structurally (see `resolveGag`).
export type GagEntry =
    | string
    | ((c: GagCtx) => string)
    | { after: number; fn: (c: GagCtx) => string }
    | { seq: string[] };

const UPDATE_CONCERNED_AFTER = 25; // the "we are concerned" line only after this many
export const UPDATE_ACHIEVEMENT_AT = 50;  // press count that unlocks the achievement (once, persisted)

export function gagOrdinal(n: number): string {
    const s: [string, string, string, string] = ['th', 'st', 'nd', 'rd'], v = n % 100;
    return n + (s[(v - 20) % 10] ?? s[v] ?? s[0]);
}
export function gagElapsed(ms: number): string {
    const t = Math.max(0, Math.floor(ms / 1000));
    return Math.floor(t / 60) + ':' + String(t % 60).padStart(2, '0');
}

// The opening sequence (the original escalating bit). Plays once, in order, the
// moment you first press; it's NOT in the random pool, so it can never surface
// mid-way.
export const UPDATE_INTRO: GagEntry[] = [
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
export const UPDATE_GAGS: GagEntry[] = [
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
    // A sequence: once entered, all of it plays in order (the bit only lands as a
    // set — "a SECOND nothing" needs a first).
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

// Resolve a gag entry to its string. Proper narrowing of the GagEntry union
// (`typeof === 'function'` / `'after' in e` / `'seq' in e`) — no `any` casts, so
// the union actually does its job.
export function resolveGag(e: GagEntry, ctx: GagCtx): string {
    if (typeof e === 'string') return e;
    if (typeof e === 'function') return e(ctx);
    if ('fn' in e) return e.fn(ctx);
    return e.seq[0]!;  // a bare sequence resolves to its opener
}

// The full mutable state of the sequencer. Held by main.ts and threaded through
// each `nextGag` call, so the pure function never reads/writes module globals.
export interface GagState {
    presses: number;
    firstMs: number;
    /** The in-progress sequence (intro or a pool `{seq}`), or null. */
    seq: { list: GagEntry[]; pos: number } | null;
    /** The opening sequence is one-shot. */
    introDone: boolean;
    /** Pool indices of one-shot sequences already run. */
    playedSeq: Set<number>;
    /** Whether the persisted achievement has been unlocked (mirrors localStorage). */
    achUnlocked: boolean;
}

export function initialGagState(achUnlocked: boolean): GagState {
    return { presses: 0, firstMs: 0, seq: null, introDone: false, playedSeq: new Set<number>(), achUnlocked };
}

// What one press produces: the toast text + duration, the updated state, and
// (once) a flag telling main.ts to persist the achievement.
export interface GagResult {
    text: string;
    ms: number;
    state: GagState;
    /** True exactly once, the press that crosses the achievement threshold. */
    unlockedAchievement: boolean;
}

// Advance the sequencer one press. PURE: same (state, ctx) ⇒ same result. `now`
// is the wall clock (Date.now) and `rng` is the random source (Math.random),
// injected so a test can pin them.
export function nextGag(state: GagState, env: { now: number; rng: () => number }): GagResult {
    const presses = state.presses + 1;
    const firstMs = state.firstMs || env.now;
    const ctx: GagCtx = { n: presses, elapsed: env.now - firstMs, idx: 0 };
    const base: GagState = { ...state, presses, firstMs };

    // A sequence in progress runs to its end — never interrupted, never re-entered.
    if (base.seq) {
        const e = base.seq.list[base.seq.pos]!;
        const nextPos = base.seq.pos + 1;
        const seq = nextPos >= base.seq.list.length ? null : { list: base.seq.list, pos: nextPos };
        return { text: resolveGag(e, ctx), ms: 4500, state: { ...base, seq }, unlockedAchievement: false };
    }

    // The intro is a one-shot sequence that leads.
    if (!base.introDone) {
        const seq = UPDATE_INTRO.length > 1 ? { list: UPDATE_INTRO, pos: 1 } : null;
        return {
            text: resolveGag(UPDATE_INTRO[0]!, ctx), ms: 4500,
            state: { ...base, introDone: true, seq }, unlockedAchievement: false,
        };
    }

    // A real, saved achievement the first time you cross the threshold (>= so a
    // sequence that straddled the mark can't make it miss).
    if (presses >= UPDATE_ACHIEVEMENT_AT && !base.achUnlocked) {
        return {
            text: '🏆 Achievement Unlocked — "Pressed UPDATE more than anyone reasonably should"', ms: 6000,
            state: { ...base, achUnlocked: true }, unlockedAchievement: true,
        };
    }

    // Random draw from the pool, skipping threshold-gated entries and any one-shot
    // sequence that has already run.
    let i = -1;
    for (let tries = 0; tries < 30; tries++) {
        const j = Math.floor(env.rng() * UPDATE_GAGS.length);
        const e = UPDATE_GAGS[j]!;
        if (typeof e === 'object' && 'after' in e && presses < e.after) continue;
        if (typeof e === 'object' && 'seq' in e && base.playedSeq.has(j)) continue;
        i = j; break;
    }
    if (i < 0) { // everything eligible was gated/spent — fall back to a plain entry
        i = UPDATE_GAGS.findIndex((e) => !(typeof e === 'object' && ('seq' in e || 'after' in e)));
        if (i < 0) i = 0;
    }
    ctx.idx = i;
    const e = UPDATE_GAGS[i]!;
    // Start a one-shot sequence: play its first message now, the rest on later presses.
    if (typeof e === 'object' && 'seq' in e) {
        const playedSeq = new Set(base.playedSeq);
        playedSeq.add(i);
        return {
            text: resolveGag(e.seq[0]!, ctx), ms: 4500,
            state: { ...base, playedSeq, seq: { list: e.seq, pos: 1 } }, unlockedAchievement: false,
        };
    }
    return { text: resolveGag(e, ctx), ms: 4500, state: base, unlockedAchievement: false };
}
