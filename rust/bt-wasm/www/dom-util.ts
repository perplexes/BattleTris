// Shared DOM helpers for the browser client.

// Escape a string for safe interpolation into innerHTML. Escapes the FULL set
// `& < > " '` so the result is safe in BOTH text and attribute contexts. In a
// text context the extra `"`/`'` escapes render identically to the raw
// characters, so this is behavior-preserving for display.
//
// Single shared copy: the page scripts (main / leaderboard / library / replay /
// spectate) all import this instead of carrying their own near-duplicate.
const HTML_ESCAPES: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;',
};

export function escapeHtml(s: string): string {
    return String(s).replace(/[&<>"']/g, (c) => HTML_ESCAPES[c]!);
}
