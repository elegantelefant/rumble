// ABOUTME: Shared Selenium setup/teardown for Rumble E2E tests.
// ABOUTME: Builds the app, starts preview server, connects Chrome WebDriver, screenshots every step.

import { Builder, until } from "selenium-webdriver";
import chrome from "selenium-webdriver/chrome.js";
import { spawn, spawnSync } from "child_process";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { writeFileSync, mkdirSync, existsSync } from "fs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "../..");
const screenshotDir = resolve(projectRoot, "e2e-screenshots");

let screenshotCounter = 0;

const PREVIEW_PORT = 4173;
const PREVIEW_URL = `http://localhost:${PREVIEW_PORT}`;

let previewServer;
let driver;

/**
 * Build the frontend for preview serving.
 * Call once in the root suite's before() hook.
 */
export function buildApp() {
  const result = spawnSync("pnpm", ["build"], {
    cwd: projectRoot,
    stdio: "inherit",
    shell: true,
  });
  if (result.status !== 0) {
    throw new Error(`Frontend build failed with exit code ${result.status}`);
  }
}

/**
 * Start Vite preview server and connect Selenium Chrome driver.
 * Returns the WebDriver instance.
 */
export async function startDriver() {
  // Start vite preview server
  previewServer = spawn("pnpm", ["preview", "--port", String(PREVIEW_PORT)], {
    cwd: projectRoot,
    stdio: ["ignore", "pipe", "pipe"],
    shell: true,
  });

  // Wait for server to be ready
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => {
      resolve(); // proceed anyway after 5s
    }, 5000);

    previewServer.stdout.on("data", (data) => {
      const output = data.toString();
      if (output.includes("Local") || output.includes(String(PREVIEW_PORT))) {
        clearTimeout(timeout);
        resolve();
      }
    });

    previewServer.on("error", (err) => {
      clearTimeout(timeout);
      reject(err);
    });
  });

  // Extra settle time for server
  await new Promise((r) => setTimeout(r, 1000));

  // Connect Chrome via Selenium Manager (auto-downloads chromedriver)
  const options = new chrome.Options();
  options.addArguments("--headless=new");
  options.addArguments("--no-sandbox");
  options.addArguments("--disable-dev-shm-usage");
  options.addArguments("--window-size=1280,800");

  driver = await new Builder()
    .forBrowser("chrome")
    .setChromeOptions(options)
    .build();

  // Navigate to the app
  await driver.get(PREVIEW_URL);
  // Wait for Vue to mount
  await driver.wait(until.elementLocated({ css: "h1" }), 15_000);

  return driver;
}

/**
 * Shut down WebDriver session and preview server.
 */
export async function stopDriver() {
  if (driver) {
    try {
      await driver.quit();
    } catch {
      // driver may already be closed
    }
  }
  if (previewServer) {
    previewServer.kill("SIGTERM");
    // Also kill the child process tree (pnpm spawns node)
    try {
      process.kill(-previewServer.pid, "SIGTERM");
    } catch {
      // process group may not exist
    }
  }
}

/**
 * Get the current WebDriver instance.
 */
export function getDriver() {
  return driver;
}

/**
 * Navigate to a hash route and wait for a heading to appear.
 * Hash routing means the URL is e.g. http://localhost:4173/#/review
 */
export async function navigateTo(path) {
  await driver.get(`${PREVIEW_URL}/#${path}`);
  // Wait for Vue to render — look for any h1
  await driver.wait(until.elementLocated({ css: "h1" }), 10_000);
}

/**
 * Wait for an element matching a CSS selector to be present in the DOM.
 */
export async function waitFor(css, timeoutMs = 5000) {
  return driver.wait(until.elementLocated({ css }), timeoutMs);
}

/**
 * Wait for text to appear anywhere on the page.
 */
export async function waitForText(text, timeoutMs = 5000) {
  return driver.wait(
    until.elementLocated({ xpath: `//*[contains(text(),'${text}')]` }),
    timeoutMs,
  );
}

/**
 * Take a screenshot and save to e2e-screenshots/.
 * Name format: NNN_<label>.png (auto-incrementing counter for ordering).
 * Call this before/after every interaction and assertion.
 */
export async function screenshot(label) {
  if (!driver) return;
  if (!existsSync(screenshotDir)) {
    mkdirSync(screenshotDir, { recursive: true });
  }
  screenshotCounter++;
  const padded = String(screenshotCounter).padStart(3, "0");
  const safeName = label.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, 80);
  const filePath = resolve(screenshotDir, `${padded}_${safeName}.png`);
  const base64 = await driver.takeScreenshot();
  writeFileSync(filePath, Buffer.from(base64, "base64"));
  return filePath;
}
