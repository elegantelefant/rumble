// ABOUTME: Deep UX audit — tests interactions that were missed in prior walkthrough.
// ABOUTME: Covers file upload, toasts, focus management, keyboard nav, and edge cases.
import { expect, test } from "@playwright/test";
import * as path from "path";
import * as fs from "fs";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const S = (name: string) => `screenshots/deep-${name}.png`;

// Create a temp test file for upload testing
const TEST_FILE_PATH = path.join(__dirname, "test-upload.txt");
test.beforeAll(() => {
  fs.writeFileSync(TEST_FILE_PATH, "This is a test legal document for review.\nSection 1: Obligations\nSection 2: Termination");
});
test.afterAll(() => {
  fs.unlinkSync(TEST_FILE_PATH);
});

// ═══════════════════════════════════════
// 1. LOGIN PAGE — Actually see it
// ═══════════════════════════════════════

test.describe("1. Login Page (bypass auth stub)", () => {
  test("1a. see the actual login page UI", async ({ page }) => {
    // The router guard always returns true, so /login redirects.
    // But we can still verify the redirect behavior is correct.
    await page.goto("/login");
    await page.screenshot({ path: S("01a-login-or-redirect"), fullPage: true });
    // Since isAuthenticated=true, we should be at /review
    await expect(page).toHaveURL(/\/review/);
  });
});

// ═══════════════════════════════════════
// 2. FILE UPLOAD — Actual file selection
// ═══════════════════════════════════════

test.describe("2. File Upload", () => {
  test("2a. upload a file via file input", async ({ page }) => {
    await page.goto("/review");

    // Get the hidden file input and set files on it
    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles(TEST_FILE_PATH);

    // Should see the uploaded file in the session list
    await page.waitForTimeout(500);
    await page.screenshot({ path: S("02a-file-uploaded"), fullPage: true });
    // File should appear as "test-upload.txt" in the session heading
    await expect(page.getByRole("heading", { name: "test-upload.txt" })).toBeVisible();
  });

  test("2b. uploaded file shows review status", async ({ page }) => {
    await page.goto("/review");
    const fileInput = page.locator('input[type="file"]');
    await fileInput.setInputFiles(TEST_FILE_PATH);
    await page.waitForTimeout(500);

    // Click on the uploaded file session in the sidebar list
    await page.getByRole("button", { name: /test-upload\.txt/ }).first().click();
    await page.waitForTimeout(1000);
    await page.screenshot({ path: S("02b-uploaded-session-active"), fullPage: true });
  });

  test("2c. browse button triggers file picker", async ({ page }) => {
    await page.goto("/review");
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
    await page.goto("/settings");
    await page.getByRole("button", { name: "Save settings" }).click();
    // Wait for toast to appear
    await page.waitForTimeout(500);
    await page.screenshot({ path: S("03a-toast-settings-saved"), fullPage: true });
    await expect(page.getByText("Couldn't save settings")).toBeVisible();
  });

  test("3b. draft export shows info toast", async ({ page }) => {
    await page.goto("/draft");
    await page.getByRole("button", { name: "Export to Word" }).click();
    await page.waitForTimeout(300);
    await page.screenshot({ path: S("03b-toast-export-word"), fullPage: true });
    await expect(page.getByText("Exported draft as WORD")).toBeVisible();
  });

  test("3c. draft export PDF shows info toast", async ({ page }) => {
    await page.goto("/draft");
    await page.getByRole("button", { name: "Export to PDF" }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Exported draft as PDF")).toBeVisible();
    await page.screenshot({ path: S("03c-toast-export-pdf"), fullPage: true });
  });

  test("3d. manage templates shows coming soon toast", async ({ page }) => {
    await page.goto("/draft");
    await page.getByRole("button", { name: "Manage Templates" }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Template management coming soon")).toBeVisible();
    await page.screenshot({ path: S("03d-toast-manage-templates"), fullPage: true });
  });

  test("3e. browse local templates shows coming soon toast", async ({ page }) => {
    await page.goto("/draft");
    await page.getByRole("button", { name: "Browse Local Templates..." }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Template browsing coming soon")).toBeVisible();
    await page.screenshot({ path: S("03e-toast-browse-templates"), fullPage: true });
  });

  test("3f. save provider secret without key shows error toast", async ({ page }) => {
    await page.goto("/settings");
    await page.getByLabel("Provider").selectOption("openai");
    // Don't fill the key — just click save
    await page.getByRole("button", { name: "Save secret" }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Enter the provider key")).toBeVisible();
    await page.screenshot({ path: S("03f-toast-secret-error"), fullPage: true });
  });

  test("3g. research prompt shows info toast", async ({ page }) => {
    await page.goto("/research");
    const textarea = page.locator("#research-question");
    await textarea.fill("What are the elements of negligence?");
    await page.getByRole("button", { name: "Start Research" }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Research request sent")).toBeVisible();
    await page.screenshot({ path: S("03g-toast-research-sent"), fullPage: true });
  });

  test("3h. translation generates success toast", async ({ page }) => {
    await page.goto("/translation");
    await page.getByPlaceholder("Paste or type the passage").fill("This is a test");
    await page.getByRole("button", { name: "Translate" }).click();
    await page.waitForTimeout(500);
    await expect(page.getByText("Translation generated")).toBeVisible();
    await page.screenshot({ path: S("03h-toast-translation-success"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 4. KEYBOARD NAVIGATION
// ═══════════════════════════════════════

test.describe("4. Keyboard Navigation", () => {
  test("4a. tab through review page form elements", async ({ page }) => {
    await page.goto("/review");
    // Tab to Browse Files button
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.keyboard.press("Tab");
    await page.screenshot({ path: S("04a-tab-focus-review"), fullPage: true });
  });

  test("4b. command palette keyboard navigation", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    // Arrow down to highlight second item
    await page.keyboard.press("ArrowDown");
    await page.screenshot({ path: S("04b-palette-arrow-down"), fullPage: true });
    // Arrow down again
    await page.keyboard.press("ArrowDown");
    await page.screenshot({ path: S("04c-palette-arrow-down-2"), fullPage: true });
    // Press Enter to navigate
    await page.keyboard.press("Enter");
    await page.screenshot({ path: S("04d-palette-enter-navigate"), fullPage: true });
  });

  test("4e. Enter submits review question", async ({ page }) => {
    await page.goto("/review");
    const input = page.locator("#document-question");
    await input.fill("Is there a force majeure clause?");
    await input.press("Enter");
    // Form should submit — question appears in chat
    await page.waitForTimeout(300);
    await expect(page.getByText("Is there a force majeure clause?", { exact: true })).toBeVisible();
    await page.screenshot({ path: S("04e-enter-submit-question"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 5. EDGE CASES & EMPTY STATES
// ═══════════════════════════════════════

test.describe("5. Edge Cases", () => {
  test("5a. translation same language error", async ({ page }) => {
    await page.goto("/translation");
    // Set both to English
    await page.getByLabel("Source language").selectOption("en");
    await page.getByLabel("Target language").selectOption("en");
    await page.getByPlaceholder("Paste or type the passage").fill("Test text");
    await page.getByRole("button", { name: "Translate" }).click();
    await page.waitForTimeout(300);
    await expect(page.getByText("Source and target languages are the same")).toBeVisible();
    await page.screenshot({ path: S("05a-same-language-error"), fullPage: true });
  });

  test("5b. translation empty source text does nothing", async ({ page }) => {
    await page.goto("/translation");
    // Don't fill any text, just click translate
    await page.getByRole("button", { name: "Translate" }).click();
    // Nothing should happen — no toast, no error, button still enabled
    await page.screenshot({ path: S("05b-translate-empty-noop"), fullPage: true });
  });

  test("5c. research empty prompt does nothing", async ({ page }) => {
    await page.goto("/research");
    await page.getByRole("button", { name: "Start Research" }).click();
    // Nothing should happen — prompt is empty
    await page.screenshot({ path: S("05c-research-empty-noop"), fullPage: true });
  });

  test("5d. review send empty question does nothing", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Send" }).click();
    // No error, no action — empty question guard
    await page.screenshot({ path: S("05d-review-empty-send"), fullPage: true });
  });

  test("5e. settings remove all secrets", async ({ page }) => {
    await page.goto("/settings");
    // Remove the pre-seeded secret
    await page.getByText("Remove").first().click();
    await page.screenshot({ path: S("05e-settings-no-secrets"), fullPage: true });
  });

  test("5f. draft validation clears when switching templates", async ({ page }) => {
    await page.goto("/draft");
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
    { path: "/review", name: "review" },
    { path: "/draft", name: "draft" },
    { path: "/research", name: "research" },
    { path: "/translation", name: "translation" },
    { path: "/settings", name: "settings" },
  ];

  for (const p of mobilePages) {
    test(`6-mobile-${p.name}`, async ({ page }) => {
      await page.setViewportSize({ width: 375, height: 812 });
      await page.goto(p.path);
      await page.screenshot({ path: S(`06-mobile-${p.name}`), fullPage: true });
    });
  }
});

// ═══════════════════════════════════════
// 7. FOCUS VISIBLE STATES
// ═══════════════════════════════════════

test.describe("7. Focus States", () => {
  test("7a. input focus ring visible", async ({ page }) => {
    await page.goto("/draft");
    await page.getByPlaceholder("Full name").focus();
    await page.screenshot({ path: S("07a-input-focus-ring"), fullPage: true });
  });

  test("7b. button focus ring visible", async ({ page }) => {
    await page.goto("/draft");
    await page.getByRole("button", { name: "Generate Draft" }).focus();
    await page.screenshot({ path: S("07b-button-focus-ring"), fullPage: true });
  });

  test("7c. select focus ring visible", async ({ page }) => {
    await page.goto("/settings");
    await page.getByRole("button", { name: "Appearance" }).click();
    await page.getByLabel("Theme").focus();
    await page.screenshot({ path: S("07c-select-focus-ring"), fullPage: true });
  });
});
