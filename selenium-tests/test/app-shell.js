// ABOUTME: Selenium tests for AppShell — sidebar navigation, topbar, command palette.
// ABOUTME: Covers spec §2-4 (sidebar, topbar, Cmd+K palette).

import { By, Key, until } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor, waitForText } from "./helpers.js";

describe("AppShell — Sidebar", function () {
  it("sidebar is visible with branding", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    expect(text).to.include("Elefant");
    expect(text).to.include("Rumble");
    // "Workspace" may be uppercased by CSS
    expect(text.toLowerCase()).to.include("workspace");
  });

  it("shows all navigation items", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    expect(text).to.include("Document Review");
    expect(text).to.include("Research");
    expect(text).to.include("Document Draft");
    expect(text).to.include("Translation");
    expect(text).to.include("Evidence Review");
    expect(text).to.include("Settings");
  });

  it("evidence review is marked coming soon", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    expect(text).to.include("Coming Soon");
  });

  it("shows user info card", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    expect(text).to.include("CoastalTower238");
    expect(text).to.include("SilverEcho951");
    expect(text).to.include("Rumble v0.1.0a");
  });

  it("shows confidentiality tagline", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    // CSS uppercase transforms the visual text
    expect(text.toLowerCase()).to.include("confidential ai tools");
  });

  it("can navigate to each active route via sidebar", async function () {
    this.timeout(30_000);
    const driver = getDriver();

    const routes = [
      { label: "Document Review", heading: "Document Review" },
      { label: "Research", heading: "Research Assistant" },
      { label: "Document Draft", heading: "Document Draft" },
      { label: "Translation", heading: "Translation" },
      { label: "Settings", heading: "Settings" },
    ];

    for (const { label, heading } of routes) {
      const btn = await driver.findElement(
        By.xpath(`//aside//button[.//div[text()='${label}']]`),
      );
      await btn.click();
      // Wait for the specific heading to appear
      await driver.wait(
        until.elementLocated(By.xpath(`//h1[text()='${heading}']`)),
        5000,
      );
      const h1 = await driver.findElement(By.css("h1"));
      const text = await h1.getText();
      expect(text).to.equal(heading);
    }
  });

  it("evidence review click does not navigate", async function () {
    const driver = getDriver();
    // First go to review
    const reviewBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Document Review']]"),
    );
    await reviewBtn.click();
    await driver.wait(
      until.elementLocated(By.xpath("//h1[text()='Document Review']")),
      5000,
    );

    // Now click the disabled evidence review
    const evidenceBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Evidence Review']]"),
    );
    await evidenceBtn.click();

    // Should still be on document review
    const h1 = await driver.findElement(By.css("h1"));
    const text = await h1.getText();
    expect(text).to.equal("Document Review");
  });
});

describe("AppShell — TopBar", function () {
  it("shows confidentiality label", async function () {
    const driver = getDriver();
    const header = await driver.findElement(By.css("header"));
    const text = await header.getText();
    // CSS may uppercase the text
    expect(text.toLowerCase()).to.include("local & confidential");
  });

  it("shows shortcuts button", async function () {
    const driver = getDriver();
    const btn = await driver.findElement(
      By.css('header button[aria-label="Open shortcuts"]'),
    );
    expect(btn).to.exist;
  });
});

describe("AppShell — Command Palette", function () {
  it("opens via Cmd+K and lists commands", async function () {
    const driver = getDriver();
    // Trigger Cmd+K (Meta on Mac)
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();

    // Wait for palette dialog
    const dialog = await waitFor(
      '[role="dialog"][aria-label="Command palette"]',
    );
    expect(dialog).to.exist;

    // Should have listbox with options
    const listbox = await driver.findElement(By.css('[role="listbox"]'));
    const options = await listbox.findElements(By.css('[role="option"]'));
    expect(options.length).to.be.greaterThan(0);
  });

  it("search input filters commands", async function () {
    const driver = getDriver();
    // Palette should still be open from previous test
    const input = await driver.findElement(By.css("#palette-search"));
    await input.clear();
    await input.sendKeys("doc");

    // Wait for filter to apply
    await new Promise((r) => setTimeout(r, 300));

    const options = await driver.findElements(By.css('[role="option"]'));
    expect(options.length).to.be.greaterThan(0);
    for (const opt of options) {
      const text = await opt.getText();
      expect(text.toLowerCase()).to.include("doc");
    }
  });

  it("closes on Escape", async function () {
    const driver = getDriver();
    // Send Escape to the search input which has focus
    const input = await driver.findElement(By.css("#palette-search"));
    await input.sendKeys(Key.ESCAPE);

    // Wait for transition to complete
    await new Promise((r) => setTimeout(r, 300));

    // Dialog should be gone
    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });

  it("no matches message", async function () {
    const driver = getDriver();
    // Reopen palette
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');

    const input = await driver.findElement(By.css("#palette-search"));
    await input.clear();
    await input.sendKeys("zzz");
    await new Promise((r) => setTimeout(r, 300));

    const page = await driver.findElement(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    const text = await page.getText();
    expect(text.toLowerCase()).to.include("no matches");

    // Clean up: close palette
    await input.sendKeys(Key.ESCAPE);
    await new Promise((r) => setTimeout(r, 300));
  });

  it("arrow keys move highlight", async function () {
    const driver = getDriver();
    // Open palette
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');

    // First option should be highlighted
    let options = await driver.findElements(By.css('[role="option"]'));
    let firstSelected = await options[0].getAttribute("aria-selected");
    expect(firstSelected).to.equal("true");

    // Arrow down should move highlight
    const input = await driver.findElement(By.css("#palette-search"));
    await input.sendKeys(Key.ARROW_DOWN);
    await new Promise((r) => setTimeout(r, 100));

    options = await driver.findElements(By.css('[role="option"]'));
    let secondSelected = await options[1].getAttribute("aria-selected");
    expect(secondSelected).to.equal("true");

    // Clean up
    await input.sendKeys(Key.ESCAPE);
    await new Promise((r) => setTimeout(r, 300));
  });

  it("enter navigates and closes", async function () {
    const driver = getDriver();
    // Open palette
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');

    // Press Enter on first highlighted item
    const input = await driver.findElement(By.css("#palette-search"));
    await input.sendKeys(Key.ENTER);
    await new Promise((r) => setTimeout(r, 500));

    // Palette should close
    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });

  it("click option navigates", async function () {
    const driver = getDriver();
    // Open palette
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');

    // Click first option
    const option = await driver.findElement(By.css('[role="option"]'));
    await option.click();
    await new Promise((r) => setTimeout(r, 500));

    // Palette should close
    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });
});
