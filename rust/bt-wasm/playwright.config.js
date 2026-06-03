// Playwright config for the wasm client e2e tests.
//
// vs-Computer is fully client-side (WasmVsComputer — no bt-server needed), so a
// plain static file server over the bt-wasm dir is enough: it serves both
// /www/ (the page) and /pkg/ (the wasm the page imports via ../pkg).
//
// Build the wasm first (or use `npm test`, which does it):
//   wasm-pack build . --target web --out-dir pkg --dev
const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: './tests',
  timeout: 45000,
  expect: { timeout: 20000 },
  use: { baseURL: 'http://localhost:4173' },
  webServer: {
    command: 'python3 -m http.server 4173',
    cwd: '.',                       // serves bt-wasm/ → /www/ + /pkg/
    url: 'http://localhost:4173/www/',
    reuseExistingServer: true,
    timeout: 30000,
  },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
