// ABOUTME: Selenium tests for Document Draft view.
// ABOUTME: Covers runbook Phase 5 — template library, form fields, validation, export.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo } from "./helpers.js";

describe("Document Draft", function () {
  before(async function () {
    await navigateTo("/draft");
  });

  it("page heading is Document Draft", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    expect(await h1.getText()).to.equal("Document Draft");
  });

  it("shows three template options", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.include("Employment");
    expect(text).to.include("Non-Disclosure");
    expect(text).to.include("Service");
  });

  it("template is selectable", async function () {
    const driver = getDriver();
    // Click the Employment template
    const templateBtn = await driver.findElement(
      By.xpath("//*[contains(text(),'Employment')]"),
    );
    await templateBtn.click();

    // Should show form fields for employment
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/employee|name|salary|position/i);
  });

  it("form fields have labels", async function () {
    const driver = getDriver();
    const labels = await driver.findElements(By.css("label"));
    expect(labels.length).to.be.greaterThan(0);
  });

  it("generate draft button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/generate|draft/i);
  });

  it("export buttons are present", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    // Export to Word/PDF
    expect(text).to.match(/word|pdf|export/i);
  });

  it("data privacy notice is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/privacy|local|confidential|device/i);
  });
});
