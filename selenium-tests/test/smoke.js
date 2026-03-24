// ABOUTME: Selenium smoke tests for Rumble via tauri-driver.
// ABOUTME: Verifies the full app launches and core UI elements render (Linux/Windows CI only).

import { Builder, By, until } from "selenium-webdriver";
import { expect } from "chai";
import { spawn, spawnSync } from "child_process";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { homedir, platform } from "os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "../..");

let tauriDriver;
let driver;
let exit = false;

// Debug builds use the crate name on all platforms
const BINARY_NAME =
  platform() === "win32" ? "elefant_rumble.exe" : "elefant_rumble";
const BINARY_PATH = resolve(
  projectRoot,
  "src-tauri",
  "target",
  "debug",
  BINARY_NAME,
);

describe("Rumble Smoke Tests", function () {
  before(async function () {
    this.timeout(300_000);

    // Build the debug app (no bundle — just the binary)
    spawnSync("pnpm", ["tauri", "build", "--debug", "--no-bundle"], {
      cwd: projectRoot,
      stdio: "inherit",
      shell: true,
    });

    // Start tauri-driver
    const driverPath = resolve(homedir(), ".cargo/bin/tauri-driver");
    tauriDriver = spawn(driverPath, [], {
      stdio: [null, process.stdout, process.stderr],
    });

    tauriDriver.on("error", (err) => {
      console.error("tauri-driver error:", err);
      process.exit(1);
    });

    tauriDriver.on("exit", (code) => {
      if (!exit) {
        console.error("tauri-driver exited with code:", code);
        process.exit(1);
      }
    });

    // Wait for tauri-driver to be ready
    await new Promise((r) => setTimeout(r, 2000));

    // Connect Selenium to tauri-driver
    driver = await new Builder()
      .usingServer("http://127.0.0.1:4444/")
      .withCapabilities({
        "tauri:options": { application: BINARY_PATH },
        browserName: "wry",
      })
      .build();
  });

  after(async function () {
    if (driver) await driver.quit();
    exit = true;
    tauriDriver?.kill();
  });

  it("app window renders content", async function () {
    // Wait for the app to load — look for any heading
    const heading = await driver.wait(
      until.elementLocated(By.css("h1")),
      10_000,
    );
    const text = await heading.getText();
    expect(text).to.be.a("string").with.length.greaterThan(0);
  });

  it("sidebar shows navigation items", async function () {
    const sidebar = await driver.findElement(By.css("aside"));
    const sidebarText = await sidebar.getText();
    expect(sidebarText).to.include("Document Review");
    expect(sidebarText).to.include("Research");
    expect(sidebarText).to.include("Document Draft");
    expect(sidebarText).to.include("Translation");
    expect(sidebarText).to.include("Settings");
  });

  it("sidebar shows branding", async function () {
    const sidebar = await driver.findElement(By.css("aside"));
    const sidebarText = await sidebar.getText();
    expect(sidebarText).to.include("Elefant");
    expect(sidebarText).to.include("Rumble");
  });

  it("can navigate to settings", async function () {
    // Click Settings in the sidebar
    const settingsBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Settings']]"),
    );
    await settingsBtn.click();

    // Wait for settings heading
    const heading = await driver.wait(
      until.elementLocated(By.xpath("//h1[text()='Settings']")),
      5000,
    );
    const text = await heading.getText();
    expect(text).to.equal("Settings");
  });

  it("can navigate back to document review", async function () {
    const reviewBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Document Review']]"),
    );
    await reviewBtn.click();

    const heading = await driver.wait(
      until.elementLocated(By.xpath("//h1[text()='Document Review']")),
      5000,
    );
    expect(await heading.getText()).to.equal("Document Review");
  });
});
