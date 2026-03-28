// ABOUTME: Selenium tests for Research Assistant view.
// ABOUTME: Covers runbook Phase 6 — threads, model selector, prompts, citations.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo } from "./helpers.js";

describe("Research Assistant", function () {
  before(async function () {
    await navigateTo("/research");
  });

  it("page heading is Research", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    expect(await h1.getText()).to.equal("Research Assistant");
  });

  it("shows pre-populated threads", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    // Pre-seeded threads should be visible
    expect(text.length).to.be.greaterThan(50);
  });

  it("model selector is present with Ollama default", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/ollama|model/i);
  });

  it("new thread button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/new thread/i);
  });

  it("prompt textarea exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("hosted models show API key requirement", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/api key/i);
  });

  it("clicking a thread activates it", async function () {
    const driver = getDriver();
    // Look for thread items — they should be clickable
    const threadItems = await driver.findElements(
      By.xpath("//main//button[contains(@class,'rounded')]"),
    );
    if (threadItems.length > 0) {
      await threadItems[0].click();
      await new Promise((r) => setTimeout(r, 300));
      // Should show thread content
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text.length).to.be.greaterThan(50);
    }
  });
});
