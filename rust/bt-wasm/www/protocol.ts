// The websocket wire protocol between the browser and bt-server, as TypeScript
// types. Server frames are a discriminated union on `type`, so the message handler
// (onSignalMessage in main.ts) is exhaustively checked — a new server message, or a
// renamed field, is a compile error rather than a silent `undefined` at runtime.
//
// These mirror the JSON bt-server emits/accepts (see bt-server/src/main.rs and the
// Snapshot struct in bt-server/src/bout.rs). Input frames are built in Rust now
// (bt-netcode `input_frame`, surfaced via WasmClient.predict_*), so they're not
// modelled here — the client only constructs the control frames in ClientMessage.

// ─── Shared sub-shapes ───────────────────────────────────────────────────────

/** A player as listed in the lobby roster (`players` frame). */
export interface PlayerInfo {
    name: string;
    status: string;          // e.g. 'available' | 'searching' | 'in_game'
    ping?: number;
    bot?: boolean;
    geo?: string;
    elo?: number;
}

/** Authoritative own-side status carried every snapshot (`msg.you`). */
export interface SideStatus {
    funds: number;
    in_bazaar: boolean;
    lines_til_bazaar: number;
}

/** Authoritative opponent view carried every snapshot (`msg.opp`). */
export interface OppStatus {
    score: number;
    lines: number;
    game_over: boolean;
    in_bazaar?: boolean;
}

// ─── REST shapes (not websocket frames, but shared wire types) ───────────────

/** The `GET /api/player/:name` JSON (bt-server `player_record_json`). All the
 *  best/streak figures are nullable — a never-recorded stat comes back as null. */
export interface PlayerStats {
    name: string;
    elo: number;
    mu: number;
    sigma: number;
    games: number;
    wins: number;
    losses: number;
    streak: number;
    streak_type: string | null;
    high_score: number | null;
    high_lines: number | null;
    high_funds: number | null;
    fastest_kill: number | null;
    quickest_death: number | null;
    longest_game: number | null;
}

/** The subset of a stored replay's JSON the client reads (the recording struct in
 *  bt-replay, plus `name_a`/`name_b` the server threads in, plus `seed_a` which
 *  marks a two-board Versus recording). Every field is optional because the same
 *  shape covers single-board (`seed`/`mode`/`ai_level`/…) and Versus (`seed_a`/…)
 *  recordings, and older rows omit `name_a`/`name_b`/`title`. */
export interface ReplayMeta {
    mode?: string;
    ai_level?: number | null;
    engine_sha?: string;
    seed?: number;
    tick_count?: number;
    frames?: unknown[];
    title?: string | null;
    /** Present only on a two-board online (Versus) recording. */
    seed_a?: unknown;
    name_a?: string;
    name_b?: string;
}

// ─── Server → client frames ──────────────────────────────────────────────────

export interface StatsMsg { type: 'stats'; players?: number; hits?: number; }
export interface PlayersMsg { type: 'players'; players: PlayerInfo[]; }
export interface ChallengedMsg { type: 'challenged'; from: string; }
export interface ChallengeDeclinedMsg { type: 'challengeDeclined'; by: string; }
export interface DrainingMsg { type: 'draining'; }
export interface ResumedMsg { type: 'resumed'; }
export interface OpponentReconnectingMsg { type: 'opponentReconnecting'; grace_secs?: number; }
export interface OpponentResumedMsg { type: 'opponentResumed'; }
export interface RejoinFailedMsg { type: 'rejoinFailed'; }

export interface MatchStartMsg {
    type: 'matchStart';
    seed: number;
    /** Tagged-UUID match id (`match-<uuid>`), parked in the URL for rejoin-on-refresh. */
    match_id: string;
    opponent?: string;
    opp_elo?: number;
    /** Which side this client plays; the client doesn't branch on it, modelled for completeness. */
    side?: 'A' | 'B';
    /** Matchmaking quality figure (auto-pairs only; absent for a directed challenge). */
    quality?: number;
}

export interface SnapshotMsg {
    type: 'snapshot';
    ack: number;
    you: SideStatus;
    opp: OppStatus;
    /** 0 = ongoing, 1 = you won, 2 = you lost. Always present (bout.rs `result: i32`). */
    result: number;
    /** Full authoritative state (Game::snapshot_bytes); present only on keyframes. */
    keyframe?: number[];
    /** A spy of ours is active this frame. */
    spying?: boolean;
    /** The opponent's FULL board (render ids, empty = -2), rides keyframes while
     *  spying. The client flickers `spy_hide`% of the cells each frame to render
     *  the spy's accuracy. */
    spy_board?: number[];
    /** Percent of `spy_board`'s cells to hide each frame (Ames 50, Ace 15, Condor
     *  0); the per-frame re-roll of the hidden set is the spy static. */
    spy_hide?: number;
    /** Server-computed opponent funds revealed by our spy (Ames perturbed, Ace
     *  mostly exact, Condor exact); rides keyframes while spying. */
    spy_funds?: number;
}

export interface RatingMsg { type: 'rating'; mu: number; sigma: number; won: boolean; }
export interface MatchReplayMsg { type: 'matchReplay'; id: string; }
export interface OpponentLeftMsg { type: 'opponentLeft'; }
/** A liveness probe the server emits ~2 Hz (bout.rs); the client treats it as a no-op. */
export interface HeartbeatMsg { type: 'heartbeat'; }

/** Every frame the server can send on the lobby/match socket. */
/** A cross-player effect the server applied to its copy of our board (an opponent
 *  weapon arriving, the opponent's score mirror, a funds credit), forwarded for the
 *  client to apply to its own local sim. `input` is the serde form of a bt_replay
 *  Input; the client hands it to `WasmClient.apply_event` verbatim. */
export interface EventMsg { type: 'event'; input: unknown; }

export type ServerMessage =
    | StatsMsg
    | EventMsg
    | PlayersMsg
    | ChallengedMsg
    | ChallengeDeclinedMsg
    | DrainingMsg
    | ResumedMsg
    | OpponentReconnectingMsg
    | OpponentResumedMsg
    | RejoinFailedMsg
    | MatchStartMsg
    | SnapshotMsg
    | RatingMsg
    | MatchReplayMsg
    | OpponentLeftMsg
    | HeartbeatMsg;

/** Discriminant strings, for narrowing helpers/tests. */
export type ServerMessageType = ServerMessage['type'];

// ─── Client → server frames (control only; input frames come from WasmClient) ──

export type ClientMessage =
    | { type: 'available'; value: boolean; name?: string; token?: string; geo?: string; bot?: boolean }
    | { type: 'queue'; name: string; token: string; authoritative: true }
    /** Directed challenge to another player. `name`/`token` are forwarded server-side
     *  for identity verification when a challenge is sent while already open-to-matches. */
    | { type: 'challenge'; target: string; name?: string; token?: string }
    | { type: 'challengeAccept'; from: string }
    | { type: 'challengeDecline'; from: string }
    | { type: 'rejoin'; match_id: string; token: string; name: string | null }
    | { type: 'leaveMatch' }
    /** Subscribe to live-site stats (lobby watch) — no target field for the stats variant. */
    | { type: 'watch' }
    | { type: 'active' };
