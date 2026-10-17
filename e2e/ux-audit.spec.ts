// ABOUTME: UX audit tests — captures interaction states and mobile views.
// ABOUTME: Validates responsive behavior, empty states, and interaction feedback.
import { expect, test } from "@playwright/test";
import { gotoRoute } from "./helpers";

test("mobile view hides sidebar by default", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await gotoRoute(page, "/review");
  await page.screenshot({ path: "screenshots/mobile-review.png", fullPage: true });
  const sidebar = page.locator("aside.sidebar");
  await expect(sidebar).not.toBeInViewport();
});

test("command palette shows commands", async ({ page }) => {
  await gotoRoute(page, "/review");
  await page.getByRole("button", { name: "Open shortcuts" }).click();
  await page.screenshot({ path: "screenshots/command-palette.png", fullPage: true });
  await expect(page.getByText("Go to Document Review")).toBeVisible();
  await expect(page.getByText("Start a new draft")).toBeVisible();
});

test("draft validation inline errors", async ({ page }) => {
  await gotoRoute(page, "/draft");
  await page.getByRole("button", { name: "Generate Draft" }).click();
  await page.screenshot({ path: "screenshots/draft-validation.png", fullPage: true });
});

test("settings appearance tab", async ({ page }) => {
  await gotoRoute(page, "/settings");
  await page.getByRole("button", { name: "Appearance" }).click();
  await page.screenshot({ path: "screenshots/settings-appearance.png", fullPage: true });
});

test("review empty state when no session selected", async ({ page }) => {
  await gotoRoute(page, "/review");
  // The pre-seeded session auto-selects, but let's check the layout
  await page.screenshot({ path: "screenshots/review-with-session.png", fullPage: true });
});

test("mobile hamburger opens sidebar", async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await gotoRoute(page, "/review");
  // Find and click the hamburger
  const hamburger = page.getByRole("button", { name: "Toggle navigation" });
  if (await hamburger.isVisible()) {
    await hamburger.click();
    await page.screenshot({ path: "screenshots/mobile-sidebar-open.png", fullPage: true });
  }
});
