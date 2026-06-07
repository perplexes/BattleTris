// Verifies the ?debug=1 weapon-grant picker: it appears in vs-Computer, the
// +funds button credits funds, and clicking a weapon grants it to the arsenal.
const { test, expect } = require('@playwright/test');

test('debug weapon picker grants weapons + funds (vs-Computer)', async ({ page }) => {
  await page.goto('/www/?debug=1');
  await page.click('#playComputerBtn');

  // The picker should appear (vs-Computer has grant_weapon) and list all 34.
  await page.waitForSelector('#debugTools', { state: 'visible' });
  await page.waitForFunction(() => document.querySelectorAll('#debugTools .dt-wpn').length >= 34);

  // Funds drop.
  const funds0 = await page.evaluate(() => window.bt.game.funds());
  await page.click('#debugTools .dt-funds');
  expect(await page.evaluate(() => window.bt.game.funds())).toBe(funds0 + 99999);

  // Weapon grant: arsenal starts empty; the first button is token 0.
  expect(await page.evaluate(() => window.bt.game.arsenal_token(0))).toBe(-1);
  await page.click('#debugTools .dt-wpn >> nth=0');
  expect(await page.evaluate(() => window.bt.game.arsenal_token(0))).toBe(0);
});
