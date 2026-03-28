// ABOUTME: Selenium tests for Document Draft view.
// ABOUTME: Covers spec §6 — template library, form fields, validation, export.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, screenshot } from "./helpers.js";

describe("Document Draft", function () {
  before(async function () {
    await navigateTo("/draft");
    await screenshot("draft_page_loaded");
  });

  it("page heading is Document Draft", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    await screenshot("draft_heading");
    expect(await h1.getText()).to.equal("Document Draft");
  });

  it("shows three template options", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("draft_template_options");
    expect(text).to.include("Employment");
    expect(text).to.include("Non-Disclosure");
    expect(text).to.include("Service");
  });

  it("template is selectable", async function () {
    const driver = getDriver();
    const templateBtn = await driver.findElement(
      By.xpath("//*[contains(text(),'Employment')]"),
    );
    await screenshot("draft_before_template_click");
    await templateBtn.click();
    await screenshot("draft_after_template_click");

    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/employee|name|salary|position/i);
  });

  it("form fields have labels", async function () {
    const driver = getDriver();
    const labels = await driver.findElements(By.css("label"));
    await screenshot("draft_form_labels");
    expect(labels.length).to.be.greaterThan(0);
  });

  it("generate draft button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("draft_generate_button");
    expect(text).to.match(/generate|draft/i);
  });

  it("export buttons are present", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("draft_export_buttons");
    expect(text).to.match(/word|pdf|export/i);
  });

  it("data privacy notice is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("draft_privacy_notice");
    expect(text).to.match(/privacy|local|confidential|device/i);
  });

  it("clicking NDA template switches context", async function () {
    const driver = getDriver();
    await screenshot("draft_before_nda_click");

    // Click the Non-Disclosure Agreement template
    const ndaBtn = await driver.findElement(
      By.xpath("//*[contains(text(),'Non-Disclosure')]"),
    );
    await ndaBtn.click();
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("draft_after_nda_click");

    // Assert template heading changed
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.include("Non-Disclosure");

    // Assert NDA-specific form fields are shown
    expect(text).to.match(/disclosing|receiving|duration/i);
  });

  it("generate draft with empty fields shows validation errors", async function () {
    const driver = getDriver();

    // Fresh navigation ensures Employment template is selected with empty fields
    await navigateTo("/draft");
    await screenshot("draft_validation_start");

    // Click Generate Draft without filling any fields
    // Button text is inside a <span> child, so use "." (descendant text) not "text()"
    const genBtn = await driver.findElement(
      By.xpath("//button[contains(.,'Generate Draft')]"),
    );
    await screenshot("draft_before_generate_click");
    await genBtn.click();
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("draft_after_generate_click");

    // Assert validation error messages appear
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/required/i);
  });
});
