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
        statusEl.textContent = 'No replays yet — play a game and hit 🔗 Share.';
        return;
    }
    statusEl.textContent = `${replays.length} replay${replays.length === 1 ? '' : 's'}`;

    for (const r of replays) {
        const a = document.createElement('a');
        a.className = 'library-item';
        a.href = '/replay/' + r.id;
        const lvl = (r.ai_level !== null && r.ai_level !== undefined) ? ` (Ernie ${r.ai_level})` : '';
        a.innerHTML =
            `<span class="library-mode">${r.mode}${lvl}</span>` +
            `<span class="library-stat">${r.tick_count} ticks</span>` +
            `<span class="library-stat">${r.inputs} inputs</span>` +
            `<span class="library-stat">seed ${r.seed}</span>` +
            `<span class="library-stat library-sha">${r.engine_sha}</span>` +
            `<span class="library-age">${fmtAge(r.mtime)}</span>` +
            `<span class="library-watch">Watch &#9654;</span>`;
        listEl.appendChild(a);
    }
})();
