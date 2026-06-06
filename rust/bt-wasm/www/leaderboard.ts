// Leaderboard page: ranks online players by Elo-styled TrueSkill (served by
// GET /api/leaderboard). TrueSkill is the engine; the Elo figure is cosmetic.
const listEl = document.getElementById('leaderboardList')!;
const statusEl = document.getElementById('leaderboardStatus')!;

/** One ranked player as returned by GET /api/leaderboard. */
interface LeaderboardPlayer {
    name: string;
    elo: number;
    games: number;
    mu: number;
    sigma: number;
}

/** Shape of the GET /api/leaderboard JSON response. */
interface LeaderboardResponse {
    players?: LeaderboardPlayer[];
}

(async () => {
    let data: LeaderboardResponse | undefined;
    try {
        const res = await fetch('/api/leaderboard');
        if (!res.ok) throw new Error('server ' + res.status);
        data = await res.json();
    } catch (e) {
        statusEl.textContent = 'Could not load leaderboard: ' + (e as Error).message;
        return;
    }

    const players = (data && data.players) || [];
    if (!players.length) {
        statusEl.textContent = 'No ranked players yet - play an online match (Find Match) to get on the board.';
        return;
    }
    statusEl.textContent = `${players.length} ranked player${players.length === 1 ? '' : 's'}`;

    players.forEach((p, i) => {
        const row = document.createElement('div');
        row.className = 'leaderboard-item';
        const games = `${p.games} game${p.games === 1 ? '' : 's'}`;
        const detail = `μ ${p.mu.toFixed(1)} · σ ${p.sigma.toFixed(1)}`;
        row.innerHTML =
            `<span class="leaderboard-rank">#${i + 1}</span>` +
            `<span class="leaderboard-name">${escapeHtml(p.name)}</span>` +
            `<span class="leaderboard-elo">${p.elo}</span>` +
            `<span class="leaderboard-stat">${games}</span>` +
            `<span class="leaderboard-stat leaderboard-detail">${detail}</span>`;
        listEl.appendChild(row);
    });
})();

// Names are user-supplied - escape before injecting as HTML.
function escapeHtml(s: string): string {
    return String(s).replace(/[&<>"']/g, (c) => ({
        '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]!));
}

// Loaded as <script type="module"> (leaderboard.html), so mark this a module —
// isolates its top-level scope from the other page scripts under tsc's single-
// program compile (else `listEl`/`statusEl` collide with library.ts).
export {};
