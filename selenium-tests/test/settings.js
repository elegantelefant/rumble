// ABOUTME: Selenium tests for Settings view.
// ABOUTME: Covers spec §9 — tabs, providers, storage, appearance, sync.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor, screenshot } from "./helpers.js";

describe("Settings", function () {
  before(async function () {
    await navigateTo("/settings");
    await screenshot("settings_page_loaded");
  });

  it("page heading is Settings", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    await screenshot("settings_heading");
    expect(await h1.getText()).to.equal("Settings");
  });

  describe("tab navigation", function () {
    it("shows four tab buttons", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_all_tabs");
      expect(text).to.include("Providers");
      expect(text).to.include("Templates");
      expect(text).to.match(/appearance/i);
      expect(text).to.include("Sync");
    });

    it("clicking each tab switches content", async function () {
      const driver = getDriver();

      const syncTab = await driver.findElement(
        By.xpath("//button[contains(text(),'Sync')]"),
      );
      await screenshot("settings_before_sync_tab");
      await syncTab.click();
      await new Promise((r) => setTimeout(r, 200));
      await screenshot("settings_sync_tab_active");

      let page = await driver.findElement(By.css("main"));
      let text = await page.getText();
      expect(text).to.match(/sync|server|connection/i);

      const appearanceTab = await driver.findElement(
        By.xpath("//button[contains(text(),'Appearance')]"),
      );
      await screenshot("settings_before_appearance_tab");
      await appearanceTab.click();
      await new Promise((r) => setTimeout(r, 200));
      await screenshot("settings_appearance_tab_active");

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
      await screenshot("settings_providers_tab");
    });

    it("shows pre-populated Ollama entry", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_providers_ollama");
      expect(text).to.match(/ollama/i);
    });

    it("shows provider selection options", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_providers_options");
      expect(text).to.match(/openai/i);
      expect(text).to.match(/anthropic/i);
    });

    it("shows local config fields for Ollama", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_providers_local_config");
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
      await screenshot("settings_sync_tab_loaded");
    });

    it("shows sync enable checkbox", async function () {
      const driver = getDriver();
      const checkboxes = await driver.findElements(
        By.css('input[type="checkbox"]'),
      );
      await screenshot("settings_sync_checkbox");
      expect(checkboxes.length).to.be.greaterThan(0);
    });

    it("shows team code", async function () {
      const driver = getDriver();
      const inputs = await driver.findElements(By.css("main input.input"));
      await screenshot("settings_sync_team_code");
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
      await screenshot("settings_sync_test_connection");
      expect(text).to.match(/test connection/i);
    });
  });

  describe("appearance tab", function () {
    before(async function () {
      const driver = getDriver();
      const tab = await driver.findElement(
        By.xpath("//button[contains(text(),'Appearance')]"),
      );
      await tab.click();
      await new Promise((r) => setTimeout(r, 200));
      await screenshot("settings_appearance_tab_loaded");
    });

    it("shows theme and sidebar position options", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_appearance_options");

      // Should mention theme and sidebar position
      expect(text).to.match(/theme/i);
      expect(text).to.match(/sidebar/i);

      // Should have select elements for theme and sidebar position
      const selects = await driver.findElements(By.css("main select"));
      await screenshot("settings_appearance_selects");
      expect(selects.length).to.be.greaterThanOrEqual(2);
    });
  });

  describe("save", function () {
    it("save button exists", async function () {
      const driver = getDriver();
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      await screenshot("settings_save_button");
      expect(text).to.match(/save/i);
    });
  });
});
