// ABOUTME: Selenium tests for Document Review view.
// ABOUTME: Covers spec §5 — upload zone, sessions, chat workspace.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor, waitForText, screenshot } from "./helpers.js";

describe("Document Review", function () {
  before(async function () {
    await navigateTo("/review");
    await screenshot("review_page_loaded");
  });

  it("page heading is Document Review", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    await screenshot("review_heading");
    expect(await h1.getText()).to.equal("Document Review");
  });

  it("shows trust badge", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("review_trust_badge");
    expect(text).to.match(/confidential|local|device/i);
  });

  it("file upload drop zone is visible", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("review_drop_zone");
    expect(text).to.match(/drag|drop|browse|upload/i);
  });

  it("shows pre-seeded session", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("review_pre_seeded_session");
    expect(text).to.include("Contract_2024.pdf");
  });

  it("custom prompt textarea exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    await screenshot("review_custom_prompt_textarea");
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("clicking session shows chat workspace", async function () {
    const driver = getDriver();
    const sessionEl = await driver.findElement(
      By.xpath("//*[contains(text(),'Contract_2024.pdf')]"),
    );
    await screenshot("review_before_session_click");
    await sessionEl.click();
    await new Promise((r) => setTimeout(r, 600));
    await screenshot("review_after_session_click");
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    expect(text.length).to.be.greaterThan(50);
  });

  it("chat input field exists when session is active", async function () {
    const driver = getDriver();
    const inputs = await driver.findElements(
      By.css('input[type="text"], textarea'),
    );
    await screenshot("review_chat_input");
    expect(inputs.length).to.be.greaterThan(0);
  });

  it("API surface toggle is present", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("review_api_surface_toggle");
    expect(text).to.match(/api|surface/i);
  });

  it("clicking toggle expands/collapses API surface panel", async function () {
    const driver = getDriver();
    // Find and click the "API surface" button to expand
    const toggleBtn = await driver.findElement(
      By.xpath("//button[contains(text(),'API surface')]"),
    );
    await screenshot("review_api_panel_before_expand");
    await toggleBtn.click();
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("review_api_panel_expanded");

    // Assert the panel is now visible with "Backend expectations" heading
    const page = await driver.findElement(By.css("main"));
    const expandedText = await page.getText();
    expect(expandedText).to.include("Backend expectations");

    // Click "Hide" button to collapse
    const hideBtn = await driver.findElement(
      By.xpath("//button[contains(text(),'Hide')]"),
    );
    await screenshot("review_api_panel_before_collapse");
    await hideBtn.click();
    await new Promise((r) => setTimeout(r, 300));
    await screenshot("review_api_panel_collapsed");

    // Assert the panel is gone
    const collapsedText = await (
      await driver.findElement(By.css("main"))
    ).getText();
    expect(collapsedText).to.not.include("Backend expectations");
  });

  it("sending empty question does nothing", async function () {
    const driver = getDriver();

    // Ensure session is active — click the pre-seeded session if chat input isn't visible
    const inputs = await driver.findElements(By.css("#document-question"));
    if (inputs.length === 0) {
      const sessionEl = await driver.findElement(
        By.xpath("//*[contains(text(),'Contract_2024.pdf')]"),
      );
      await sessionEl.click();
      await new Promise((r) => setTimeout(r, 600));
    }

    await screenshot("review_empty_send_before");

    // Count existing messages
    const messagesBefore = await driver.findElements(
      By.css(".message-user, .message-assistant"),
    );
    const countBefore = messagesBefore.length;

    // Ensure the input is empty
    const input = await driver.findElement(By.css("#document-question"));
    await input.clear();
    await screenshot("review_empty_send_input_cleared");

    // Submit the empty form
    const sendBtn = await driver.findElement(
      By.css('button[type="submit"].btn-primary'),
    );
    await sendBtn.click();
    await new Promise((r) => setTimeout(r, 500));
    await screenshot("review_empty_send_after");

    // Count messages after — should be unchanged
    const messagesAfter = await driver.findElements(
      By.css(".message-user, .message-assistant"),
    );
    expect(messagesAfter.length).to.equal(countBefore);
  });
});
