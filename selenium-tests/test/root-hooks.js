// ABOUTME: Mocha root hooks — build app and manage tauri-driver lifecycle.
// ABOUTME: Runs once before/after all test files, not per-file.

import { buildApp, startDriver, stopDriver } from "./helpers.js";

export const mochaHooks = {
  async beforeAll() {
    this.timeout(300_000);
    buildApp();
    await startDriver();
  },

  async afterAll() {
    await stopDriver();
  },
};
