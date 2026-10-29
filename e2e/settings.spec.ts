// ABOUTME: E2e tests for the Settings view.
// ABOUTME: Validates tab navigation, provider management, and appearance controls.
import { expect, test } from "@playwright/test";
import { gotoRoute } from "./helpers";

test.describe("Settings", () => {
  test.beforeEach(async ({ page }) => {
    await gotoRoute(page, "/settings");
  });

  test("renders page heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByText("API keys are stored in your system keychain.")).toBeVisible();
  });

  test("shows all setting tabs", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Providers & API keys" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Templates & workspace storage" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Appearance" })).toBeVisible();
    // Sync has no tab at all now (#87 review, Ask 3) -- hidden, not disabled.
    await expect(page.getByRole("button", { name: "Sync" })).not.toBeVisible();
  });

  test("providers tab shows configured secrets", async ({ page }) => {
    await expect(page.getByText("Configured secrets")).toBeVisible();
    await expect(page.getByText("Primary local runtime")).toBeVisible();
  });

  test("providers tab has add provider form", async ({ page }) => {
    await expect(page.getByText("Add provider secret")).toBeVisible();
    await expect(page.getByRole("button", { name: "Save secret" })).toBeVisible();
  });

  test("switching to storage tab shows template config", async ({ page }) => {
    await page.getByRole("button", { name: "Templates & workspace storage" }).click();
    await expect(page.getByText("Template library")).toBeVisible();
    await expect(page.getByText("Templates folder")).toBeVisible();
  });

  test("switching to appearance tab shows layout options", async ({ page }) => {
    await page.getByRole("button", { name: "Appearance" }).click();
    await expect(page.getByText("Layout preferences")).toBeVisible();
    await expect(page.getByLabel("Navigation sidebar")).toBeVisible();
    await expect(page.getByLabel("Theme")).toBeVisible();
  });

  test("save settings button is present", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Save settings" })).toBeVisible();
  });
});
