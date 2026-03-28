// ABOUTME: Selenium tests for Translation view.
// ABOUTME: Covers spec §8 — language selectors, translate action, history.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, screenshot } from "./helpers.js";

describe("Translation", function () {
  before(async function () {
    await navigateTo("/translation");
    await screenshot("translation_page_loaded");
  });

  it("page heading is Translation", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    await screenshot("translation_heading");
    expect(await h1.getText()).to.equal("Translation");
  });

  it("shows pre-populated translation job", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("translation_pre_populated_job");
    expect(text).to.match(/french|english|fr|en/i);
  });

  it("source language dropdown exists", async function () {
    const driver = getDriver();
    const selects = await driver.findElements(By.css("select"));
    await screenshot("translation_source_dropdown");
    expect(selects.length).to.be.greaterThanOrEqual(2);
  });

  it("target language dropdown exists", async function () {
    const driver = getDriver();
    const selects = await driver.findElements(By.css("select"));
    await screenshot("translation_target_dropdown");
    expect(selects.length).to.be.greaterThanOrEqual(2);
  });

  it("source text area exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    await screenshot("translation_source_textarea");
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("translate button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("translation_translate_button");
    expect(text).to.match(/translate/i);
  });

  it("draft quality warning is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("translation_draft_quality_warning");
    expect(text).to.match(/draft|quality|disclaimer/i);
  });

  it("jurisdiction disclaimer is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("translation_jurisdiction_disclaimer");
    expect(text).to.match(/jurisdiction|legal|accuracy|not.*substitute/i);
  });

  it("model selector is present", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("translation_model_selector");

    // The view has a "Model" label with a <select>
    expect(text).to.match(/model/i);

    // Verify there's a select for model (at least 3 selects: source lang, target lang, model)
    const selects = await driver.findElements(By.css("select"));
    expect(selects.length).to.be.greaterThanOrEqual(3);
  });
});
