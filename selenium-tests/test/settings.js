// ABOUTME: Selenium tests for Settings view.
// ABOUTME: Covers runbook Phase 8 — tabs, providers, storage, appearance, sync.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor } from "./helpers.js";

describe("Settings", function () {
  before(async function () {
    await navigateTo("/settings");
  });

  it("page heading is Settings", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    expect(await h1.getText()).to.equal("Settings");
  });

  describe("tab navigation", function () {
    it("shows four tab buttons", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.include("Providers");
      expect(text).to.include("Templates");
      // "Appearance" or "Appearance" tab
      expect(text).to.match(/appearance/i);
      expect(text).to.include("Sync");
    });

    it("clicking each tab switches content", async function () {
      const driver = getDriver();

      // Click Sync tab
      const syncTab = await driver.findElement(
        By.xpath("//button[contains(text(),'Sync')]"),
      );
      await syncTab.click();
      await new Promise((r) => setTimeout(r, 200));

      let page = await driver.findElement(By.css("main"));
      let text = await page.getText();
      expect(text).to.match(/sync|server|connection/i);

      // Click Appearance tab
      const appearanceTab = await driver.findElement(
        By.xpath("//button[contains(text(),'Appearance')]"),
      );
      await appearanceTab.click();
      await new Promise((r) => setTimeout(r, 200));

      page = await driver.findElement(By.css("main"));
      text = await page.getText();
      expect(text).to.match(/theme|sidebar|position/i);
    });
  });

  describe("providers tab", function () {
    before(async function () {
      const driver = getDriver();
      const tab = await driver.findElement(
        By.xpath("//button[contains(text(),'Providers')]"),
      );
      await tab.click();
      await new Promise((r) => setTimeout(r, 200));
    });

    it("shows pre-populated Ollama entry", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.match(/ollama/i);
    });

    it("shows provider selection options", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.match(/openai/i);
      expect(text).to.match(/anthropic/i);
    });

    it("shows local config fields for Ollama", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.match(/host|port|model/i);
    });
  });

  describe("sync tab", function () {
    before(async function () {
      const driver = getDriver();
      const tab = await driver.findElement(
        By.xpath("//button[contains(text(),'Sync')]"),
      );
      await tab.click();
      await new Promise((r) => setTimeout(r, 200));
    });

    it("shows sync enable checkbox", async function () {
      const driver = getDriver();
      const checkboxes = await driver.findElements(
        By.css('input[type="checkbox"]'),
      );
      expect(checkboxes.length).to.be.greaterThan(0);
    });

    it("shows team code", async function () {
      const driver = getDriver();
      // Team code is in an input field, check value attribute
      const inputs = await driver.findElements(By.css('main input.input'));
      let found = false;
      for (const input of inputs) {
        const val = await input.getAttribute("value");
        if (val && val.includes("SilverEcho951")) {
          found = true;
          break;
        }
      }
      expect(found, "SilverEcho951 should appear in an input value").to.be.true;
    });

    it("test connection button exists", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.match(/test connection/i);
    });
  });

  describe("save", function () {
    it("save button exists", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text).to.match(/save/i);
    });
  });
});
