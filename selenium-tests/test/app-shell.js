// ABOUTME: Selenium tests for AppShell — sidebar navigation, topbar, command palette.
// ABOUTME: Covers spec §2-4 (sidebar, topbar, Cmd+K palette).

import { By, Key, until } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor, waitForText, screenshot } from "./helpers.js";

describe("AppShell — Sidebar", function () {
  it("sidebar is visible with branding", async function () {
    const driver = getDriver();
    await screenshot("sidebar_initial");
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    expect(text).to.include("Elefant");
    expect(text).to.include("Rumble");
    expect(text.toLowerCase()).to.include("workspace");
    await screenshot("sidebar_branding_verified");
  });

  it("shows all navigation items", async function () {
    const driver = getDriver();
    const sidebar = await driver.findElement(By.css("aside"));
    const text = await sidebar.getText();
    await screenshot("sidebar_nav_items");
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
    await screenshot("sidebar_coming_soon");
    expect(text).to.include("Coming Soon");
  });

  it("shows user info card", async function () {
    const driver = getDriver();
    await screenshot("sidebar_user_card");
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
    await screenshot("sidebar_confidentiality_tagline");
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
      await screenshot(`sidebar_before_click_${label.replace(/\s/g, "_")}`);
      const btn = await driver.findElement(
        By.xpath(`//aside//button[.//div[text()='${label}']]`),
      );
      await btn.click();
      await driver.wait(
        until.elementLocated(By.xpath(`//h1[text()='${heading}']`)),
        5000,
      );
      await screenshot(`sidebar_after_click_${label.replace(/\s/g, "_")}`);
      const h1 = await driver.findElement(By.css("h1"));
      const text = await h1.getText();
      expect(text).to.equal(heading);
    }
  });

  it("sidebar reorder — move first item down, boundary arrows disabled", async function () {
    this.timeout(15_000);
    const driver = getDriver();
    await navigateTo("/review");
    await screenshot("sidebar_reorder_initial");

    async function getToolLabels() {
      const nav = await driver.findElement(
        By.css('nav[aria-label="Primary tools"]'),
      );
      const toolDivs = await nav.findElements(By.css(":scope > div"));
      const labels = [];
      for (const div of toolDivs) {
        const labelEl = await div.findElement(By.css(".text-sm.font-semibold"));
        labels.push(await labelEl.getText());
      }
      return labels;
    }

    const initialOrder = await getToolLabels();
    expect(initialOrder.length).to.be.greaterThan(1);
    const firstLabel = initialOrder[0];
    const secondLabel = initialOrder[1];
    const lastLabel = initialOrder[initialOrder.length - 1];

    const upBtn = await driver.findElement(
      By.css(`button[aria-label="Move ${firstLabel} up"]`),
    );
    expect(await upBtn.getAttribute("disabled")).to.not.be.null;
    await screenshot("sidebar_reorder_first_up_disabled");

    const downBtnLast = await driver.findElement(
      By.css(`button[aria-label="Move ${lastLabel} down"]`),
    );
    expect(await downBtnLast.getAttribute("disabled")).to.not.be.null;
    await screenshot("sidebar_reorder_last_down_disabled");

    const downBtnFirst = await driver.findElement(
      By.css(`button[aria-label="Move ${firstLabel} down"]`),
    );
    await driver.executeScript("arguments[0].click()", downBtnFirst);
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("sidebar_reorder_after_move");

    const newOrder = await getToolLabels();
    expect(newOrder[0]).to.equal(secondLabel);
    expect(newOrder[1]).to.equal(firstLabel);
  });

  it("evidence review click does not navigate", async function () {
    const driver = getDriver();
    const reviewBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Document Review']]"),
    );
    await reviewBtn.click();
    await driver.wait(
      until.elementLocated(By.xpath("//h1[text()='Document Review']")),
      5000,
    );
    await screenshot("sidebar_before_evidence_click");

    const evidenceBtn = await driver.findElement(
      By.xpath("//aside//button[.//div[text()='Evidence Review']]"),
    );
    await evidenceBtn.click();
    await screenshot("sidebar_after_evidence_click");

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
    await screenshot("topbar_confidentiality");
    expect(text.toLowerCase()).to.include("local & confidential");
  });

  it("shows shortcuts button", async function () {
    const driver = getDriver();
    const btn = await driver.findElement(
      By.css('header button[aria-label="Open shortcuts"]'),
    );
    await screenshot("topbar_shortcuts_button");
    expect(btn).to.exist;
  });

  it("user avatar chip shows initials", async function () {
    const driver = getDriver();
    await screenshot("topbar_avatar_before");
    const chip = await driver.findElement(
      By.css('header [aria-label="Current user"]'),
    );
    const avatar = await chip.findElement(By.css(".rounded-full.h-8.w-8"));
    const initials = await avatar.getText();
    await screenshot("topbar_avatar_initials");
    expect(initials).to.match(/^[A-Z]{1,3}$/);
    expect(initials).to.equal("CT");
  });
});

describe("AppShell — Command Palette", function () {
  it("opens via Cmd+K and lists commands", async function () {
    const driver = getDriver();
    await screenshot("palette_before_open");
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();

    const dialog = await waitFor(
      '[role="dialog"][aria-label="Command palette"]',
    );
    expect(dialog).to.exist;
    await screenshot("palette_opened");

    const listbox = await driver.findElement(By.css('[role="listbox"]'));
    const options = await listbox.findElements(By.css('[role="option"]'));
    expect(options.length).to.be.greaterThan(0);
    await screenshot("palette_commands_listed");
  });

  it("search input filters commands", async function () {
    const driver = getDriver();
    const input = await driver.findElement(By.css("#palette-search"));
    await input.clear();
    await screenshot("palette_filter_before_type");
    await input.sendKeys("doc");
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("palette_filter_after_type_doc");

    const options = await driver.findElements(By.css('[role="option"]'));
    expect(options.length).to.be.greaterThan(0);
    for (const opt of options) {
      const text = await opt.getText();
      expect(text.toLowerCase()).to.include("doc");
    }
  });

  it("closes on Escape", async function () {
    const driver = getDriver();
    const input = await driver.findElement(By.css("#palette-search"));
    await screenshot("palette_before_escape");
    await input.sendKeys(Key.ESCAPE);
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("palette_after_escape");

    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });

  it("no matches message", async function () {
    const driver = getDriver();
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
    await screenshot("palette_no_matches");

    const page = await driver.findElement(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    const text = await page.getText();
    expect(text.toLowerCase()).to.include("no matches");

    await input.sendKeys(Key.ESCAPE);
    await new Promise((r) => setTimeout(r, 300));
  });

  it("arrow keys move highlight", async function () {
    const driver = getDriver();
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');
    await screenshot("palette_arrow_initial");

    let options = await driver.findElements(By.css('[role="option"]'));
    let firstSelected = await options[0].getAttribute("aria-selected");
    expect(firstSelected).to.equal("true");

    const input = await driver.findElement(By.css("#palette-search"));
    await input.sendKeys(Key.ARROW_DOWN);
    await new Promise((r) => setTimeout(r, 100));
    await screenshot("palette_arrow_down");

    options = await driver.findElements(By.css('[role="option"]'));
    let secondSelected = await options[1].getAttribute("aria-selected");
    expect(secondSelected).to.equal("true");

    await input.sendKeys(Key.ESCAPE);
    await new Promise((r) => setTimeout(r, 300));
  });

  it("enter navigates and closes", async function () {
    const driver = getDriver();
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');
    await screenshot("palette_before_enter");

    const input = await driver.findElement(By.css("#palette-search"));
    await input.sendKeys(Key.ENTER);
    await new Promise((r) => setTimeout(r, 500));
    await screenshot("palette_after_enter");

    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });

  it("click option navigates", async function () {
    const driver = getDriver();
    await driver
      .actions()
      .keyDown(Key.META)
      .sendKeys("k")
      .keyUp(Key.META)
      .perform();
    await waitFor('[role="dialog"][aria-label="Command palette"]');
    await screenshot("palette_before_click_option");

    const option = await driver.findElement(By.css('[role="option"]'));
    await option.click();
    await new Promise((r) => setTimeout(r, 500));
    await screenshot("palette_after_click_option");

    const dialogs = await driver.findElements(
      By.css('[role="dialog"][aria-label="Command palette"]'),
    );
    expect(dialogs.length).to.equal(0);
  });
});
