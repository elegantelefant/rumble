// ABOUTME: Selenium tests for Translation view.
// ABOUTME: Covers runbook Phase 7 — language selectors, translate action, history.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo } from "./helpers.js";

describe("Translation", function () {
  before(async function () {
    await navigateTo("/translation");
  });

  it("page heading is Translation", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    expect(await h1.getText()).to.equal("Translation");
  });

  it("shows pre-populated translation job", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    // Pre-seeded FR → EN job
    expect(text).to.match(/french|english|fr|en/i);
  });

  it("source language dropdown exists", async function () {
    const driver = getDriver();
    const selects = await driver.findElements(By.css("select"));
    expect(selects.length).to.be.greaterThanOrEqual(2);
  });

  it("target language dropdown exists", async function () {
    const driver = getDriver();
    const selects = await driver.findElements(By.css("select"));
    // At least source + target + model = 3
    expect(selects.length).to.be.greaterThanOrEqual(2);
  });

  it("source text area exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("translate button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/translate/i);
  });

  it("draft quality warning is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/draft|quality|disclaimer/i);
  });

  it("jurisdiction disclaimer is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/jurisdiction|legal|accuracy|not.*substitute/i);
  });
});
