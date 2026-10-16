// ABOUTME: Deep UX audit — tests interactions that were missed in prior walkthrough.
// ABOUTME: Covers file upload, toasts, focus management, keyboard nav, and edge cases.
import { expect, test } from "@playwright/test";
import { gotoRoute } from "./helpers";
import * as path from "path";
import { fileURLToPath } from "url";
import { mockTauri } from "./mockTauri";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const S = (name: string) => `screenshots/deep-${name}.png`;

const SAMPLE_DOCUMENT = path.join(__dirname, "fixtures", "sample-document.txt");

async function uploadSampleDocument(page: import("@playwright/test").Page) {
  await page.locator('input[type="file"]').setInputFiles(SAMPLE_DOCUMENT);
}

// ═══════════════════════════════════════
// 1. LOGIN PAGE — Actually see it
// ═══════════════════════════════════════

test.describe("1. Login Page (bypass auth stub)", () => {
  test("1a. see the actual login page UI", async ({ page }) => {
    // The router guard always returns true, so /login redirects.
    // But we can still verify the redirect behavior is correct.
    await gotoRoute(page, "/login");
    await page.screenshot({ path: S("01a-login-or-redirect"), fullPage: true });
    // Since isAuthenticated=true, we should be at /review
    await expect(page).toHaveURL(/\/review/);
  });
});

// ═══════════════════════════════════════
// 2. FILE UPLOAD — Actual file selection
// ═══════════════════════════════════════

test.describe("2. File Upload", () => {
  // extract_document goes through Tauri invoke, which has no bridge in plain Chromium — needs
  // the mocked IPC from #62, same as the rest of Document Review's "no seeded session" tests.
  test("2a. upload a file via file input", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/review");
    await uploadSampleDocument(page);

    await page.screenshot({ path: S("02a-file-uploaded"), fullPage: true });
    // File should appear as "sample-document.txt" in the session heading
    await expect(page.getByRole("heading", { name: "sample-document.txt" })).toBeVisible();
  });

  test("2b. uploaded file shows review status", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/review");
    await uploadSampleDocument(page);

    // Click on the uploaded file session in the sidebar list
    await page.getByRole("button", { name: /sample-document\.txt/ }).first().click();
    await expect(page.getByText("Summary ready")).toBeVisible();
    await page.screenshot({ path: S("02b-uploaded-session-active"), fullPage: true });
  });

  test("2c. browse button triggers file picker", async ({ page }) => {
    await gotoRoute(page, "/review");
    // The Browse button should trigger the hidden input
    const fileChooserPromise = page.waitForEvent("filechooser");
    await page.getByRole("button", { name: "Browse Files" }).click();
    const fileChooser = await fileChooserPromise;
    expect(fileChooser).toBeTruthy();
    await page.screenshot({ path: S("02c-file-chooser-triggered"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 3. TOAST NOTIFICATIONS
// ═══════════════════════════════════════

test.describe("3. Toast Notifications", () => {
  // Playwright runs against Vite with no Tauri host, so nothing can be stored:
  // the honest outcome is the failure toast, never a success claim.
  test("3a. settings save without the host reports the failure", async ({ page }) => {
    await gotoRoute(page, "/settings");
    await page.getByRole("button", { name: "Save settings" }).click();
    // Wait for toast to appear
    await page.waitForTimeout(500);
    await page.screenshot({ path: S("03a-toast-settings-saved"), fullPage: true });
    await expect(page.getByText("Couldn't save settings")).toBeVisible();
  });

  test("3b. draft export shows info toast", async ({ page }) => {
    await gotoRoute(page, "/draft");
    await page.getByRole("button", { name: "Export to Word" }).click();
    await expect(page.getByText("Generate a draft before exporting.")).toBeVisible();
    await page.screenshot({ path: S("03b-toast-export-word"), fullPage: true });
  });

  // Needs createDraftJob/waitForJob to resolve a non-empty draft before the PDF-specific branch
  // past the no-draft guard is reachable — the mocked IPC bridge's /draft handler (#97) does that.
  test("3c. draft export PDF shows info toast", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/draft");
    await page.getByLabel("Employee Name").fill("Jane Doe");
    await page.getByLabel("Start Date").fill("2026-01-01");
    await page.getByLabel("Salary").fill("$100,000");
    await page.getByLabel("Position").fill("Associate");
    await page.getByRole("button", { name: "Generate Draft" }).click();
    await expect(page.getByText("Draft prepared. Review before sharing with clients.")).toBeVisible();
    await page.getByRole("button", { name: "Export to PDF" }).click();
    await expect(page.getByText("PDF export is coming soon.")).toBeVisible();
    await page.screenshot({ path: S("03c-toast-export-pdf"), fullPage: true });
  });

  test("3d. manage templates shows coming soon toast", async ({ page }) => {
    await gotoRoute(page, "/draft");
    await page.getByRole("button", { name: "Manage Templates" }).click();
    await expect(page.getByText("Template management coming soon")).toBeVisible();
    await page.screenshot({ path: S("03d-toast-manage-templates"), fullPage: true });
  });

  test("3e. browse local templates shows coming soon toast", async ({ page }) => {
    await gotoRoute(page, "/draft");
    await page.getByRole("button", { name: "Browse Local Templates..." }).click();
    await expect(page.getByText("Template browsing coming soon")).toBeVisible();
    await page.screenshot({ path: S("03e-toast-browse-templates"), fullPage: true });
  });

  test("3f. save provider secret without key shows error toast", async ({ page }) => {
    await gotoRoute(page, "/settings");
    await page.getByRole("combobox", { name: "Provider" }).selectOption("openai");
    // Don't fill the key — just click save
    await page.getByRole("button", { name: "Save secret" }).click();
    await expect(page.getByText("Enter the provider key")).toBeVisible();
    await page.screenshot({ path: S("03f-toast-secret-error"), fullPage: true });
  });

  // submitPrompt() bails at its "no models available" guard before this toast fires, unless
  // /models resolves to something usable — there's no sidecar host in plain e2e otherwise (#62).
  test("3g. research prompt shows info toast", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/research");
    const textarea = page.locator("#research-question");
    await textarea.fill("What are the elements of negligence?");
    await page.getByRole("button", { name: "Start Research" }).click();
    await expect(page.getByText("Research request sent")).toBeVisible();
    await page.screenshot({ path: S("03g-toast-research-sent"), fullPage: true });
  });

  // The real success copy is "Translation complete." (TranslationView.vue) — "Translation
  // generated" never existed. translate() goes through Tauri invoke, so it needs the mocked
  // IPC bridge from #62 to resolve at all; before that, this failed on both the wrong text and
  // the missing host.
  test("3h. translation generates success toast", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/translation");
    await page.getByPlaceholder("Paste or type the passage").fill("This is a test");
    await page.getByRole("button", { name: "Translate" }).click();
    await expect(page.getByText("Translation complete.")).toBeVisible();
    await page.screenshot({ path: S("03h-toast-translation-success"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 4. KEYBOARD NAVIGATION
// ═══════════════════════════════════════

test.describe("4. Keyboard Navigation", () => {
  test("4a. tab order reaches the sidebar's Research item, not a review form field", async ({ page }) => {
    await gotoRoute(page, "/review");
    // Click a non-interactive element first to set the sequential focus
    // navigation start point — without it, Tab never leaves <body> in
    // headless Chromium. Click the sidebar itself (not main content) so
    // the start point precedes the sidebar's own nav buttons.
    await page.locator("aside.sidebar").click({ position: { x: 5, y: 5 } });
    // Three tabs land on the sidebar's "Research" item, not "Browse Files":
    // each reorderable nav item contributes its own up/down arrow stop
    // before the next item's button.
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await expect(page.getByRole("button", { name: "Research", exact: true })).toBeFocused();
    await page.screenshot({ path: S("04a-tab-focus-review"), fullPage: true });
  });

  test("4b. command palette keyboard navigation", async ({ page }) => {
    await gotoRoute(page, "/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    // Arrow down to highlight second item
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Upload documents" })).toHaveAttribute("aria-selected", "true");
    await page.screenshot({ path: S("04b-palette-arrow-down"), fullPage: true });
    // Arrow down again
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Start a new draft" })).toHaveAttribute("aria-selected", "true");
    await page.screenshot({ path: S("04c-palette-arrow-down-2"), fullPage: true });
    // Press Enter to navigate
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(/\/draft/);
    await expect(page.getByRole("dialog", { name: "Command palette" })).not.toBeVisible();
    await page.screenshot({ path: S("04d-palette-enter-navigate"), fullPage: true });
  });

  // #document-question only renders once a session is active — there's no seeded session (#62),
  // so this uploads one through the mocked IPC bridge first.
  test("4e. Enter submits review question", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/review");
    await uploadSampleDocument(page);

    const input = page.locator("#document-question");
    await expect(input).toBeVisible();
    await input.fill("Is there a force majeure clause?");
    await input.press("Enter");
    // Form should submit — question appears in chat
    await expect(page.getByText("Is there a force majeure clause?", { exact: true })).toBeVisible();
    await page.screenshot({ path: S("04e-enter-submit-question"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 5. EDGE CASES & EMPTY STATES
// ═══════════════════════════════════════

test.describe("5. Edge Cases", () => {
  test("5a. translation same language error", async ({ page }) => {
    await gotoRoute(page, "/translation");
    // Set both to English
    await page.getByLabel("Source language").selectOption("en");
    await page.getByLabel("Target language").selectOption("en");
    await page.getByPlaceholder("Paste or type the passage").fill("Test text");
    await page.getByRole("button", { name: "Translate" }).click();
    await expect(page.getByText("Source and target languages are the same")).toBeVisible();
    await page.screenshot({ path: S("05a-same-language-error"), fullPage: true });
  });

  test("5b. translation empty source text does nothing", async ({ page }) => {
    await gotoRoute(page, "/translation");
    // Don't fill any text, just click translate
    await page.getByRole("button", { name: "Translate" }).click();
    // runTranslation()'s empty-input guard returns before setting
    // isTranslating — if it fired, the button stays enabled synchronously.
    await expect(page.getByRole("button", { name: "Translate" })).toBeEnabled();
    await page.screenshot({ path: S("05b-translate-empty-noop"), fullPage: true });
  });

  test("5c. research empty prompt does nothing", async ({ page }) => {
    await gotoRoute(page, "/research");
    await page.getByRole("button", { name: "Start Research" }).click();
    // submitPrompt()'s empty-prompt guard returns before setting
    // isResearching — if it fired, the button stays enabled synchronously.
    await expect(page.getByRole("button", { name: "Start Research" })).toBeEnabled();
    await page.screenshot({ path: S("05c-research-empty-noop"), fullPage: true });
  });

  // "Send" only renders once a session is active — there's no seeded session (#62), so this
  // uploads one through the mocked IPC bridge first, then checks the empty-question guard.
  test("5d. review send empty question does nothing", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/review");
    await uploadSampleDocument(page);

    await expect(page.getByRole("button", { name: "Send" })).toBeVisible();
    await page.getByRole("button", { name: "Send" }).click();
    // No error, no action — empty question guard
    await page.screenshot({ path: S("05d-review-empty-send"), fullPage: true });
  });

  test("5e. settings add ollama secret shows it with a remove button", async ({ page }) => {
    await gotoRoute(page, "/settings");
    // Ollama is the one provider with requiresKey: false, so adding it never
    // calls Tauri invoke and succeeds with no host (unlike openai/anthropic).
    await page.getByRole("combobox", { name: "Provider" }).selectOption("elefant-local");
    await page.getByRole("button", { name: "Save secret" }).click();
    const secretRow = page.getByRole("listitem").filter({ hasText: "Ollama · Elefant Legal Blend" });
    await expect(secretRow).toBeVisible();
    await expect(secretRow.getByRole("button", { name: "Remove" })).toBeVisible();
    await page.screenshot({ path: S("05e-settings-secret-added"), fullPage: true });
  });

  // Removal always calls invoke("delete_api_key"), which rejects with no Tauri host — mocked
  // here (#62) so the keychain delete actually succeeds and the row comes off the list.
  test("5e-removal. settings remove secret", async ({ page }) => {
    await mockTauri(page);
    await gotoRoute(page, "/settings");
    await page.getByRole("combobox", { name: "Provider" }).selectOption("elefant-local");
    await page.getByRole("button", { name: "Save secret" }).click();
    const secretRow = page.getByRole("listitem").filter({ hasText: "Ollama · Elefant Legal Blend" });
    await secretRow.getByRole("button", { name: "Remove" }).click();
    await expect(secretRow).not.toBeVisible();
    await page.screenshot({ path: S("05e-settings-no-secrets"), fullPage: true });
  });

  test("5f. draft validation clears when switching templates", async ({ page }) => {
    await gotoRoute(page, "/draft");
    // Trigger validation on employment
    await page.getByRole("button", { name: "Generate Draft" }).click();
    await expect(page.getByText("Employee name is required")).toBeVisible();
    // Switch to NDA — errors should clear
    await page.getByText("Non-Disclosure Agreement").click();
    await expect(page.getByText("Employee name is required")).not.toBeVisible();
    await page.screenshot({ path: S("05f-validation-clears-on-switch"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 6. RESPONSIVE — Mobile views of each page
// ═══════════════════════════════════════

test.describe("6. Mobile Views", () => {
  const mobilePages = [
    { path: "/review", name: "review", heading: "Document Review" },
    { path: "/draft", name: "draft", heading: "Document Draft" },
    { path: "/research", name: "research", heading: "Research Assistant" },
    { path: "/translation", name: "translation", heading: "Translation" },
    { path: "/settings", name: "settings", heading: "Settings" },
  ];

  for (const p of mobilePages) {
    test(`6-mobile-${p.name}`, async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 812 });
      await gotoRoute(page, p.path);
      await expect(page.getByRole("heading", { name: p.heading, exact: true }).first()).toBeVisible();
      await page.screenshot({ path: S(`06-mobile-${p.name}`), fullPage: true });
    });
  }
});

// ═══════════════════════════════════════
// 7. FOCUS VISIBLE STATES
// ═══════════════════════════════════════

test.describe("7. Focus States", () => {
  test("7a. input focus ring visible", async ({ page }) => {
    await gotoRoute(page, "/draft");
    const input = page.getByPlaceholder("Full name");
    await input.focus();
    await expect(input).toBeFocused();
    await page.screenshot({ path: S("07a-input-focus-ring"), fullPage: true });
  });

  test("7b. button focus ring visible", async ({ page }) => {
    await gotoRoute(page, "/draft");
    const button = page.getByRole("button", { name: "Generate Draft" });
    await button.focus();
    await expect(button).toBeFocused();
    await page.screenshot({ path: S("07b-button-focus-ring"), fullPage: true });
  });

  test("7c. select focus ring visible", async ({ page }) => {
    await gotoRoute(page, "/settings");
    await page.getByRole("button", { name: "Appearance" }).click();
    const select = page.getByLabel("Theme");
    await select.focus();
    await expect(select).toBeFocused();
    await page.screenshot({ path: S("07c-select-focus-ring"), fullPage: true });
  });
});
