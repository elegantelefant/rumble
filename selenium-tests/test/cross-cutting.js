// ABOUTME: Selenium tests for cross-cutting concerns: toast system and KeepAlive state preservation.
// ABOUTME: Covers spec §10 (toasts) and §11 (navigation state preservation).

import { By, until } from "selenium-webdriver";
import { expect } from "chai";
import { getDriver, navigateTo, waitFor } from "./helpers.js";

// Toast elements have pointer-events-auto + cursor-pointer + rounded-lg classes.
// This combo is unique to toast items inside the ToastProvider container.
const TOAST_ITEM_CSS = "div.pointer-events-auto.cursor-pointer.rounded-lg";

/**
 * Find all visible toast elements on the page.
 */
async function findToasts(driver) {
  return driver.findElements(By.css(TOAST_ITEM_CSS));
}

/**
 * Wait until all toasts have dismissed.
 */
async function waitForNoToasts(driver, timeoutMs = 8000) {
  await driver.wait(async () => {
    const toasts = await findToasts(driver);
    return toasts.length === 0;
  }, timeoutMs, "Toasts did not all auto-dismiss");
}

/**
 * Click the "Save settings" submit button using JavaScript to bypass overlays.
 */
async function clickSaveSettings(driver) {
  const btn = await driver.findElement(
    By.xpath("//button[@type='submit' and contains(., 'Save settings')]"),
  );
  await driver.executeScript("arguments[0].click()", btn);
}

describe("Toast System", function () {
  before(async function () {
    await navigateTo("/settings");
    // Small settle time for Vue to mount
    await new Promise((r) => setTimeout(r, 300));
  });

  it("saving settings shows a success toast that auto-dismisses", async function () {
    const driver = getDriver();

    // Trigger save
    await clickSaveSettings(driver);

    // Wait for toast to appear (mock delays 150ms + render)
    await driver.wait(async () => {
      const toasts = await findToasts(driver);
      return toasts.length > 0;
    }, 5000, "Toast did not appear after saving settings");

    // Verify toast text and success class
    const toasts = await findToasts(driver);
    expect(toasts.length).to.be.greaterThan(0);
    const text = await toasts[0].getText();
    expect(text).to.include("Settings stored securely");
    const classes = await toasts[0].getAttribute("class");
    expect(classes).to.include("bg-[var(--success)]");

    // Wait for auto-dismiss (~3s duration + buffer)
    await waitForNoToasts(driver);
  });

  it("multiple toasts stack independently", async function () {
    const driver = getDriver();

    // Ensure no leftover toasts
    await waitForNoToasts(driver);

    // Trigger two saves in quick succession via JS click (avoids overlay issues)
    await clickSaveSettings(driver);
    await new Promise((r) => setTimeout(r, 200));
    await clickSaveSettings(driver);

    // Wait for at least 2 toasts
    await driver.wait(async () => {
      const toasts = await findToasts(driver);
      return toasts.length >= 2;
    }, 5000, "Expected at least 2 toasts from rapid saves");

    const toasts = await findToasts(driver);
    expect(toasts.length).to.be.at.least(2);

    // Wait for all to dismiss
    await waitForNoToasts(driver);
  });
});

describe("KeepAlive State Preservation", function () {
  it("Document Review preserves chat input across navigation", async function () {
    const driver = getDriver();

    // Navigate to Document Review
    await navigateTo("/review");
    await new Promise((r) => setTimeout(r, 300));

    // Click the pre-seeded session to activate chat
    const sessionEl = await driver.findElement(
      By.xpath("//*[contains(text(),'Contract_2024.pdf')]"),
    );
    await sessionEl.click();
    await new Promise((r) => setTimeout(r, 600));

    // Find a text input/textarea for chat and type into it
    const inputs = await driver.findElements(
      By.css("main input[type='text'], main textarea"),
    );
    expect(inputs.length, "Expected at least one chat input").to.be.greaterThan(0);

    const chatInput = inputs[inputs.length - 1]; // Usually the last input is the chat input
    await chatInput.clear();
    await chatInput.sendKeys("test keepalive text");

    // Navigate away to settings
    await navigateTo("/settings");
    await new Promise((r) => setTimeout(r, 300));

    // Navigate back to Document Review
    await navigateTo("/review");
    await new Promise((r) => setTimeout(r, 600));

    // Check the input value is preserved
    const inputsAfter = await driver.findElements(
      By.css("main input[type='text'], main textarea"),
    );
    let preserved = false;
    for (const input of inputsAfter) {
      const val = await input.getAttribute("value");
      if (val && val.includes("test keepalive text")) {
        preserved = true;
        break;
      }
    }
    expect(preserved, "Chat input text should be preserved after navigation").to.be.true;
  });

  it("Research preserves prompt textarea across navigation", async function () {
    const driver = getDriver();

    // Navigate to Research
    await navigateTo("/research");
    await new Promise((r) => setTimeout(r, 300));

    // Find prompt textarea
    const textareas = await driver.findElements(By.css("main textarea"));
    expect(textareas.length, "Expected at least one textarea").to.be.greaterThan(0);

    const promptArea = textareas[textareas.length - 1];
    await promptArea.clear();
    await promptArea.sendKeys("research keepalive test");

    // Navigate away
    await navigateTo("/settings");
    await new Promise((r) => setTimeout(r, 300));

    // Navigate back
    await navigateTo("/research");
    await new Promise((r) => setTimeout(r, 300));

    // Check preservation
    const textareasAfter = await driver.findElements(By.css("main textarea"));
    let preserved = false;
    for (const ta of textareasAfter) {
      const val = await ta.getAttribute("value");
      if (val && val.includes("research keepalive test")) {
        preserved = true;
        break;
      }
    }
    expect(preserved, "Research prompt text should be preserved after navigation").to.be.true;
  });

  it("Translation preserves source text across navigation", async function () {
    const driver = getDriver();

    // Navigate to Translation
    await navigateTo("/translation");
    await new Promise((r) => setTimeout(r, 300));

    // Find source textarea
    const textareas = await driver.findElements(By.css("main textarea"));
    expect(textareas.length, "Expected at least one textarea").to.be.greaterThan(0);

    const sourceArea = textareas[0]; // First textarea is typically the source text
    await sourceArea.clear();
    await sourceArea.sendKeys("translation keepalive test");

    // Navigate away
    await navigateTo("/settings");
    await new Promise((r) => setTimeout(r, 300));

    // Navigate back
    await navigateTo("/translation");
    await new Promise((r) => setTimeout(r, 300));

    // Check preservation
    const textareasAfter = await driver.findElements(By.css("main textarea"));
    let preserved = false;
    for (const ta of textareasAfter) {
      const val = await ta.getAttribute("value");
      if (val && val.includes("translation keepalive test")) {
        preserved = true;
        break;
      }
    }
    expect(preserved, "Translation source text should be preserved after navigation").to.be.true;
  });
});
