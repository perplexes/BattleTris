// Custom Motif XmScrolledList scrollbar.
//
// Native browser scrollbars can't reproduce the authentic OSF/Motif scrolled-list:
// a content box and a scrollbar that are TWO separate sunken boxes with a few-px
// gutter between them (XmNspacing), the bar being one continuous sunken trough with
// bare embossed triangles top/bottom and a raised thumb padded off the trough walls.
// So we render the bar in real DOM and drive the content's native scroll.
//
// Wrapping keeps the content element's identity, so the app's `innerHTML = …`
// re-renders are fine — a MutationObserver re-measures the thumb.

/** A content element tagged once-initialized via the `__msl` marker property. */
type MslElement = HTMLElement & { __msl?: boolean };

function motifScroll(content: MslElement): void {
  if (!content || content.__msl) return;
  content.__msl = true;

  const wrap = document.createElement('div');
  wrap.className = 'msl-wrap';
  content.parentNode!.insertBefore(wrap, content);
  wrap.appendChild(content);
  content.classList.add('msl-content');

  const bar = document.createElement('div');
  bar.className = 'msl-bar';
  const up = document.createElement('div');
  up.className = 'msl-arrow up';
  const track = document.createElement('div');
  track.className = 'msl-track';
  const thumb = document.createElement('div');
  thumb.className = 'msl-thumb';
  const down = document.createElement('div');
  down.className = 'msl-arrow down';
  track.appendChild(thumb);
  bar.appendChild(up);
  bar.appendChild(track);
  bar.appendChild(down);
  wrap.appendChild(bar);

  let thumbTop = 0;
  function refresh(): void {
    const sh = content.scrollHeight;
    const ch = content.clientHeight;
    if (sh <= ch + 1) {
      bar.style.display = 'none';        // collapse the bar (and its gutter) when nothing scrolls
      return;
    }
    bar.style.display = 'flex';
    const trackH = track.clientHeight;
    const th = Math.min(trackH, Math.max(20, Math.round((trackH * ch) / sh)));
    const maxTop = Math.max(0, trackH - th);
    thumbTop = maxTop * (content.scrollTop / (sh - ch));
    thumb.style.height = th + 'px';
    thumb.style.transform = 'translateY(' + thumbTop + 'px)';
  }

  content.addEventListener('scroll', refresh, { passive: true });
  if (window.ResizeObserver) {
    const ro = new ResizeObserver(refresh);
    ro.observe(content);
    ro.observe(track);
  }
  const mo = new MutationObserver(refresh);
  mo.observe(content, { childList: true, subtree: true });

  // Arrow buttons: scroll by a line, repeating while held.
  const STEP = 34;
  function holdScroll(delta: number): void {
    content.scrollTop += delta;
    let t = setTimeout(function rep() {
      content.scrollTop += delta;
      t = setTimeout(rep, 55);
    }, 280);
    const stop = () => {
      clearTimeout(t);
      window.removeEventListener('mouseup', stop);
    };
    window.addEventListener('mouseup', stop);
  }
  up.addEventListener('mousedown', (e: MouseEvent) => { e.preventDefault(); holdScroll(-STEP); });
  down.addEventListener('mousedown', (e: MouseEvent) => { e.preventDefault(); holdScroll(STEP); });

  // Click the trough above/below the thumb → page.
  track.addEventListener('mousedown', (e: MouseEvent) => {
    if (e.target !== track) return;
    const r = track.getBoundingClientRect();
    const dir = e.clientY - r.top < thumbTop ? -1 : 1;
    content.scrollTop += dir * content.clientHeight * 0.9;
  });

  // Drag the thumb.
  thumb.addEventListener('mousedown', (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const startY = e.clientY;
    const startScroll = content.scrollTop;
    const range = content.scrollHeight - content.clientHeight;
    const maxTop = Math.max(1, track.clientHeight - thumb.offsetHeight);
    thumb.classList.add('dragging');
    function move(ev: MouseEvent): void {
      const dy = ev.clientY - startY;
      content.scrollTop = startScroll + dy * (range / maxTop);
    }
    function end(): void {
      thumb.classList.remove('dragging');
      document.removeEventListener('mousemove', move);
      document.removeEventListener('mouseup', end);
    }
    document.addEventListener('mousemove', move);
    document.addEventListener('mouseup', end);
  });

  refresh();
}

// Lists to convert (selectors are queried per-page; absent ones simply match
// nothing). Covers the in-app lists plus the library/leaderboard pages.
const MSL_SELECTORS = [
  '#bazaarWeaponList',
  '#bazaarArsenalList',
  '#onlineList',
  '#arsenalList',
  '.library-list',
  '.leaderboard-list',
];

function initMotifScroll(): void {
  MSL_SELECTORS.forEach((sel) => {
    document.querySelectorAll<HTMLElement>(sel).forEach(motifScroll);
  });
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', initMotifScroll);
} else {
  initMotifScroll();
}

export { motifScroll };
