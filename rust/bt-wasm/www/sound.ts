// Synthesized sound effects - the port's answer to the original's
// BTSoundManager (usr/src/game/BTSoundManager.{H,C}), which played sampled
// audio. We have no original assets, so these are Web Audio square/triangle
// blips in the spirit of the era's chiptune hardware. No files, no network.
//
// AudioContext can't start until a user gesture, so call Sound.resume() from
// the first keydown / touch / click.

let ctx: AudioContext | null = null;
let muted = false;

/** Safari historically exposed AudioContext only as `webkitAudioContext`. */
type WebkitWindow = typeof window & { webkitAudioContext?: typeof AudioContext };

function ensureCtx(): AudioContext | null {
    if (!ctx) {
        const AC = window.AudioContext || (window as WebkitWindow).webkitAudioContext;
        if (AC) ctx = new AC();
    }
    if (ctx && ctx.state === 'suspended') ctx.resume();
    return ctx;
}

interface NoteOpts {
    type?: OscillatorType;
    vol?: number;
    slideTo?: number | null;
    delay?: number;
}

// One enveloped oscillator note. `slideTo` bends the pitch over the note;
// `delay` schedules it ahead of now (for arpeggios).
function note(freq: number, dur: number, opts: NoteOpts = {}): void {
    const { type = 'square', vol = 0.14, slideTo = null, delay = 0 } = opts;
    const c = ensureCtx();
    if (!c || muted) return;
    const t0 = c.currentTime + delay;
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(freq, t0);
    if (slideTo) osc.frequency.exponentialRampToValueAtTime(Math.max(1, slideTo), t0 + dur);
    // Quick attack, exponential decay - a clean retro blip.
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(vol, t0 + 0.005);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
    osc.connect(gain).connect(c.destination);
    osc.start(t0);
    osc.stop(t0 + dur + 0.02);
}

export const Sound = {
    setMuted(m: boolean) { muted = !!m; },
    isMuted(): boolean { return muted; },
    resume() { ensureCtx(); },

    // A piece locks into place - a short low click.
    lock() { note(150, 0.05, { type: 'square', vol: 0.10 }); },

    // Lines cleared - an ascending arpeggio; bigger clears climb higher, and a
    // tetris (4) gets a bright capstone.
    clear(lines: number) {
        const n = Math.max(1, Math.min(4, lines | 0));
        const base = 392; // G4
        for (let i = 0; i < n; i++) {
            note(base * Math.pow(2, i / 4), 0.10, { type: 'square', vol: 0.16, delay: i * 0.05 });
        }
        if (n >= 4) note(base * 2, 0.22, { type: 'triangle', vol: 0.18, delay: n * 0.05 });
    },

    // A weapon goes out - a downward laser zap.
    weapon() { note(880, 0.16, { type: 'sawtooth', vol: 0.12, slideTo: 180 }); },

    // The weapons bazaar opens - a two-note chime.
    bazaar() {
        note(659, 0.14, { type: 'triangle', vol: 0.16 });           // E5
        note(988, 0.20, { type: 'triangle', vol: 0.16, delay: 0.12 }); // B5
    },

    // Game over - a slow descending slide.
    gameOver() { note(440, 0.6, { type: 'square', vol: 0.16, slideTo: 110 }); },

    // Near death - an urgent high pulse.
    nearDeath() { note(1320, 0.08, { type: 'square', vol: 0.10 }); },

    // A smiley face was buried instead of cleared - a glum wah-wah.
    missedSmiley() { note(330, 0.28, { type: 'triangle', vol: 0.14, slideTo: 196 }); },

    // A bad move (BT_BAD_MOVE / idiot reason 0) - a short low dissonant buzz.
    badMove() { note(98, 0.12, { type: 'sawtooth', vol: 0.10, slideTo: 73 }); },

    // An airslide (BT_AIRSLIDE) - a quick rising whoosh as the piece tucks under.
    airslide() { note(520, 0.07, { type: 'triangle', vol: 0.09, slideTo: 880 }); },
};
