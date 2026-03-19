// ABOUTME: Playwright config for e2e testing the Vue frontend via Vite dev server.
// ABOUTME: Runs against localhost:1420 with chromium only.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  retries: 0,
  use: {
    baseURL: "http://localhost:1420",
    headless: true,
    screenshot: "only-on-failure",
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
  webServer: {
    command: "pnpm dev",
    port: 1420,
    reuseExistingServer: true,
    timeout: 15_000,
  },
});
