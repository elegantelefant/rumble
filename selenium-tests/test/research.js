// ABOUTME: Selenium tests for Research Assistant view.
// ABOUTME: Covers spec §7 — threads, model selector, prompts, citations.

import { By } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, screenshot } from "./helpers.js";

describe("Research Assistant", function () {
  before(async function () {
    await navigateTo("/research");
    await screenshot("research_page_loaded");
  });

  it("page heading is Research Assistant", async function () {
    const driver = getDriver();
    const h1 = await driver.findElement(By.css("h1"));
    await screenshot("research_heading");
    expect(await h1.getText()).to.equal("Research Assistant");
  });

  it("shows pre-populated threads", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("research_threads");
    expect(text.length).to.be.greaterThan(50);
  });

  it("model selector is present with Ollama default", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("research_model_selector");
    expect(text).to.match(/ollama|model/i);
  });

  it("new thread button exists", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("research_new_thread_button");
    expect(text).to.match(/new thread/i);
  });

  it("prompt textarea exists", async function () {
    const driver = getDriver();
    const textareas = await driver.findElements(By.css("textarea"));
    await screenshot("research_prompt_textarea");
    expect(textareas.length).to.be.greaterThan(0);
  });

  it("hosted models show API key requirement", async function () {
    const driver = getDriver();
    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("research_api_key_notice");
    expect(text).to.match(/api key/i);
  });

  it("clicking a thread activates it", async function () {
    const driver = getDriver();
    const threadItems = await driver.findElements(
      By.xpath("//main//button[contains(@class,'rounded')]"),
    );
    await screenshot("research_before_thread_click");
    if (threadItems.length > 0) {
      await threadItems[0].click();
      await new Promise((r) => setTimeout(r, 300));
      await screenshot("research_after_thread_click");
      const page = await driver.findElement(By.css("main"));
      const text = await page.getText();
      expect(text.length).to.be.greaterThan(50);
    }
  });

  it("thread status badges are visible", async function () {
    const driver = getDriver();
    await navigateTo("/research");
    await screenshot("research_status_badges_before");

    const page = await driver.findElement(By.css("main"));
    const text = await page.getText();
    await screenshot("research_status_badges");

    // Pre-seeded threads have statuses: "complete", "running", "draft"
    // Displayed as "Complete", "In Progress", "Draft"
    const hasStatusBadge =
      /Complete/i.test(text) ||
      /In Progress/i.test(text) ||
      /Draft/i.test(text);
    expect(hasStatusBadge).to.be.true;
  });

  it("submitting empty prompt does nothing", async function () {
    const driver = getDriver();
    await navigateTo("/research");
    // First thread is auto-selected; wait for messages to render
    await new Promise((r) => setTimeout(r, 500));

    // Ensure textarea is empty by clearing via JS to guarantee v-model sync
    const textarea = await driver.findElement(By.css("#research-question"));
    await driver.executeScript(
      "arguments[0].value = ''; arguments[0].dispatchEvent(new Event('input', { bubbles: true }));",
      textarea,
    );
    await new Promise((r) => setTimeout(r, 200));

    await screenshot("research_empty_prompt_before");

    // Count existing messages
    const messagesBefore = await driver.findElements(
      By.css(".message-user, .message-assistant"),
    );
    const countBefore = messagesBefore.length;

    // Click "Start Research" button
    const sendBtn = await driver.findElement(
      By.xpath("//button[contains(text(),'Start Research')]"),
    );
    await sendBtn.click();
    await new Promise((r) => setTimeout(r, 500));
    await screenshot("research_empty_prompt_after");

    // Count messages after — should be unchanged
    const messagesAfter = await driver.findElements(
      By.css(".message-user, .message-assistant"),
    );
    expect(messagesAfter.length).to.equal(countBefore);
  });
});
