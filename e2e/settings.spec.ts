// ABOUTME: E2e tests for the Settings view.
// ABOUTME: Validates tab navigation, provider management, appearance, and sync controls.
import { expect, test } from "@playwright/test";

test.describe("Settings", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/settings");
  });

  test("renders page heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByText("API keys are stored in your system keychain.")).toBeVisible();
  });

  test("shows all setting tabs", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Providers & API keys" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Templates & workspace storage" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Appearance" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Sync" })).toBeVisible();
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

  test("switching to sync tab shows sync config", async ({ page }) => {
    await page.getByRole("button", { name: "Sync" }).click();
    await expect(page.getByText("Workspace sync")).toBeVisible();
    await expect(page.getByText("Enable sync")).toBeVisible();
    await expect(page.getByRole("button", { name: "Test connection" })).toBeVisible();
  });

  test("save settings button is present", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Save settings" })).toBeVisible();
  });
});
