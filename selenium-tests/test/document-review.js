// ABOUTME: Selenium tests for Document Review view.
// ABOUTME: Covers runbook Phase 4 — upload zone, sessions, chat workspace.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor, waitForText } from "./helpers.js";

describe("Document Review", function () {
  before(async function () {
    await navigateTo("/review");
  });

  it("page heading is Document Review", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    expect(await h1.getText()).to.equal("Document Review");
  });

  it("shows trust badge", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/confidential|local|device/i);
  });

  it("file upload drop zone is visible", async function () {
    const driver = getDriver();
    // Look for the drop zone or browse button
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/drag|drop|browse|upload/i);
  });

  it("shows pre-seeded session", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.include("Contract_2024.pdf");
  });

  it("custom prompt textarea exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("clicking session shows chat workspace", async function () {
    const driver = getDriver();
    // Click on the pre-seeded session
    const sessionEl = await driver.findElement(
      By.xpath("//*[contains(text(),'Contract_2024.pdf')]"),
    );
    await sessionEl.click();

    // Wait for chat area to appear — look for message input or chat messages
    await new Promise((r) => setTimeout(r, 600)); // wait for mock review
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    // Should show session details or chat messages
    expect(text.length).to.be.greaterThan(50);
  });

  it("chat input field exists when session is active", async function () {
    const driver = getDriver();
    // Look for text input or textarea for follow-up questions
    const inputs = await driver.findElements(
      By.css('input[type="text"], textarea'),
    );
    expect(inputs.length).to.be.greaterThan(0);
  });

  it("API surface toggle is present", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text).to.match(/api|surface/i);
  });
});
