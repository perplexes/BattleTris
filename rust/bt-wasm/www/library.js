// Replay library browse page: lists stored games (newest first) with a link to
// watch each via /replay/:id.
const listEl = document.getElementById('libraryList');
const statusEl = document.getElementById('libraryStatus');

function fmtAge(mtime) {
    if (!mtime) return '';
    const secs = Math.max(0, Math.floor(Date.now() / 1000) - mtime);
    if (secs < 60) return secs + 's ago';
    if (secs < 3600) return Math.floor(secs / 60) + 'm ago';
    if (secs < 86400) return Math.floor(secs / 3600) + 'h ago';
    return Math.floor(secs / 86400) + 'd ago';
}

(async () => {
    let data;
    try {
        const res = await fetch('/api/replays');
        if (!res.ok) throw new Error('server ' + res.status);
        data = await res.json();
    } catch (e) {
        statusEl.textContent = 'Could not load library: ' + e.message;
        return;
    }

    const replays = (data && data.replays) || [];
    if (!replays.length) {
        statusEl.textContent = 'No replays yet - play a game and hit Share.';
        return;
    }
    statusEl.textContent = `${replays.length} replay${replays.length === 1 ? '' : 's'}`;

    const esc = (s) => String(s).replace(/[&<>"]/g, (c) => (
        { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]
    ));

    // A player name as a link to their profile (the lobby stats panel).
    const playerLink = (n) => n
        ? `<a class="library-player" href="/www/?player=${encodeURIComponent(n)}">${esc(n)}</a>`
        : '';

    for (const r of replays) {
        const item = document.createElement('div');
        item.className = 'library-item';
        item.style.cursor = 'pointer';
        const lvl = (r.ai_level !== null && r.ai_level !== undefined) ? ` (Ernie ${r.ai_level})` : '';
        const title = r.title ? `<span class="library-title">${esc(r.title)}</span>` : '';
        // Online matches that recorded both names get a "Alice vs Bob" matchup of
        // profile links; everything else gets a plain-English match descriptor.
        const hasNames = !!(r.name_a || r.name_b);
        const matchup = hasNames
            ? `<span class="library-matchup">${playerLink(r.name_a)} <span class="library-vs">vs</span> ${playerLink(r.name_b)}</span>`
            : '';
        let modeLabel = '';
        if (!hasNames) {
            if (r.mode === 'Practice') modeLabel = 'User practice';
            else if (r.mode === 'VsComputer') modeLabel = 'User vs Computer' + lvl;
            else if (r.mode === 'Online') modeLabel = 'User vs User';
            else modeLabel = r.mode;
        }
        const modeSpan = modeLabel ? `<span class="library-mode">${modeLabel}</span>` : '';
        const lines = (r.lines !== null && r.lines !== undefined)
            ? `${r.lines} line${r.lines === 1 ? '' : 's'} cleared` : '';
        item.innerHTML =
            matchup +
            modeSpan +
            title +
            `<span class="library-stat">${lines}</span>` +
            `<span class="library-age">${fmtAge(r.mtime)}</span>` +
            `<a class="library-watch" href="/replay/${r.id}">Watch &#9654;</a>`;
        // Click anywhere on the row watches the replay — except on a real link
        // (a player profile or the Watch link), which navigates on its own.
        item.addEventListener('click', (e) => {
            if (e.target.closest('a')) return;
            location.href = '/replay/' + r.id;
        });
        listEl.appendChild(item);
    }
})();
