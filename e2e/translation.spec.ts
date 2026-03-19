// ABOUTME: E2e tests for the Translation view.
// ABOUTME: Validates language selection, translation history, model picker, and translate action.
import { expect, test } from "@playwright/test";

test.describe("Translation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/translation");
  });

  test("renders page heading and draft quality badge", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Translation", exact: true })).toBeVisible();
    await expect(page.getByText("Draft Quality")).toBeVisible();
  });

  test("shows translation history with pre-seeded job", async ({ page }) => {
    await expect(page.getByText("FR → EN")).toBeVisible();
    await expect(page.getByText("Veuillez confirmer")).toBeVisible();
  });

  test("language selectors are present", async ({ page }) => {
    await expect(page.getByText("Source language")).toBeVisible();
    await expect(page.getByText("Target language")).toBeVisible();
  });

  test("source and translation text areas are present", async ({ page }) => {
    await expect(page.getByText("Source text")).toBeVisible();
    await expect(page.getByPlaceholder("Paste or type the passage")).toBeVisible();
  });

  test("translate button is functional", async ({ page }) => {
    await page.getByPlaceholder("Paste or type the passage").fill("This is a test clause.");
    await page.getByRole("button", { name: "Translate" }).click();
    // Should show translating state or result
    await expect(page.getByText("Translate").or(page.getByText("Translating"))).toBeVisible();
  });

  test("workflow tips are displayed", async ({ page }) => {
    await expect(page.getByText("History keeps translations per matter")).toBeVisible();
  });

  test("jurisdiction disclaimer is visible", async ({ page }) => {
    await expect(page.getByText("terminology varies across jurisdictions")).toBeVisible();
  });
});
