// Unit tests for the resync request gate (the pure `shouldSendResync`).
//
// Run with:  npm run test:unit
//   → node --test --experimental-strip-types www/resync.test.ts
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { shouldSendResync, RESYNC_MIN_INTERVAL_MS } from './resync.ts';

test('the first request is always allowed (no prior send)', () => {
    assert.equal(shouldSendResync(0, null), true);
    assert.equal(shouldSendResync(1_000_000, null), true);
});

test('a request inside the window is denied', () => {
    assert.equal(shouldSendResync(1000, 0), false);
    assert.equal(shouldSendResync(RESYNC_MIN_INTERVAL_MS - 1, 0), false);
});

test('a request exactly at the window boundary is allowed', () => {
    assert.equal(shouldSendResync(RESYNC_MIN_INTERVAL_MS, 0), true);
});

test('a request past the window is allowed', () => {
    assert.equal(shouldSendResync(RESYNC_MIN_INTERVAL_MS + 500, 0), true);
});
