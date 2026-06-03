const { test, expect } = require('@playwright/test');

// Regression test for the weapon-deploy bug: the in-game arsenal panel used to
// rebuild its DOM every frame, so a click on a weapon button landed on an element
// that had already been destroyed/recreated — the click never fired, the weapon
// never launched, and nothing reached Ernie. (Keyboard 1–0 still worked, which is
// why it was easy to miss.)
//
// A pure Rust engine test can't catch this: the engine's deliver_weapon logic was
// always fine; the break was in the UI→engine path. So this drives the real DOM —
// it CLICKS the weapon button — and asserts the weapon deployed AND affected Ernie.
//
// We use Swap (token 5): deploying it exchanges the two boards, so Ernie's board
// becomes the player's old board — a clean, deterministic "his screen is affected"
// signal that doesn't depend on Ernie's own play.
const cellCount = `(a) => a.filter(v => v > 0).length`;

test('clicking an arsenal weapon button deploys it and affects Ernie', async ({ page }) => {
  await page.goto('/www/');
  await page.waitForFunction(() => !!window.bt, null, { timeout: 20000 });

  // Make Ernie fast so he stacks a board clearly distinct from the player's.
  await page.evaluate(() => { document.getElementById('ernieSlider').value = '13'; });
  await page.click('#playComputerBtn');
  await page.waitForFunction(() => window.bt.mode === 'vscomputer' && !!window.bt.game);

  // Pre-stock one Swap (token 5) in slot 0 — no need to play to the bazaar.
  await page.evaluate(() => {
    const a = [5, 1];
    for (let i = 1; i < 10; i++) a.push(-1, 0); // 10 slots × (token, qty)
    window.bt.game.import_arsenal(a);
  });

  // Let Ernie build a board that differs from the player's (so a swap is visible).
  await page.waitForFunction((c) => {
    const g = window.bt.game, cnt = eval(c);
    return cnt(g.render_ai_grid()) >= 4 && cnt(g.render_ai_grid()) !== cnt(g.render_grid());
  }, cellCount, { timeout: 25000 });

  const before = await page.evaluate((c) => {
    const g = window.bt.game, cnt = eval(c);
    return { player: cnt(g.render_grid()), ernie: cnt(g.render_ai_grid()), slot0: g.arsenal_token(0) };
  }, cellCount);
  expect(before.slot0).toBe(5);                  // Swap is pre-stocked in slot 0
  expect(before.ernie).not.toBe(before.player);  // boards differ → the swap is observable

  // The actual bug surface: CLICK the weapon button (not the keyboard).
  await page.click('#arsenalList .arsenal-item.occupied');

  // Swap exchanges boards → Ernie's board becomes the player's old board, and the
  // weapon is consumed. (Times out → fails if the click didn't deploy — the bug.)
  await page.waitForFunction(([c, p]) => {
    const g = window.bt.game, cnt = eval(c);
    return g.arsenal_token(0) === -1 && Math.abs(cnt(g.render_ai_grid()) - p) <= 4;
  }, [cellCount, before.player], { timeout: 6000 });

  const after = await page.evaluate((c) => {
    const g = window.bt.game, cnt = eval(c);
    return { ernie: cnt(g.render_ai_grid()), slot0: g.arsenal_token(0) };
  }, cellCount);

  expect(after.slot0).toBe(-1);                                          // consumed ⇒ the click deployed it
  expect(Math.abs(after.ernie - before.player)).toBeLessThanOrEqual(4);  // Ernie got the player's board (Swap)
});
