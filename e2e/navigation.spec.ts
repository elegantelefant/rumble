// ABOUTME: E2e tests for app navigation, sidebar, and route accessibility.
// ABOUTME: Verifies every active route renders its heading and the sidebar links work.
import { expect, test } from "@playwright/test";

test.describe("App Shell & Navigation", () => {
  test("root redirects to /review", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveURL(/\/review/);
  });

  test("sidebar shows all navigation tools", async ({ page }) => {
    await page.goto("/review");
    const sidebar = page.locator("aside.sidebar");
    await expect(sidebar).toBeVisible();
    await expect(sidebar.getByText("Document Review")).toBeVisible();
    await expect(sidebar.getByText("Research")).toBeVisible();
    await expect(sidebar.getByText("Document Draft")).toBeVisible();
    await expect(sidebar.getByText("Translation")).toBeVisible();
    await expect(sidebar.getByText("Settings")).toBeVisible();
  });

  test("sidebar shows user info and version", async ({ page }) => {
    await page.goto("/review");
    const sidebar = page.locator("aside.sidebar");
    await expect(sidebar.getByText("Local User")).toBeVisible();
    await expect(sidebar.getByText("Rumble", { exact: true })).toBeVisible();
  });

  test("Evidence Review shows Coming Soon badge", async ({ page }) => {
    await page.goto("/review");
    await expect(page.getByText("Coming Soon")).toBeVisible();
  });

  test("unknown routes redirect to /review", async ({ page }) => {
    await page.goto("/nonexistent-page");
    await expect(page).toHaveURL(/\/review/);
    await expect(page.getByRole("heading", { name: "Document Review" })).toBeVisible();
  });
});

test.describe("Route Navigation via Sidebar", () => {
  const routes = [
    { link: "Document Review", heading: "Document Review", path: "/review" },
    { link: "Research", heading: "Research Assistant", path: "/research" },
    { link: "Document Draft", heading: "Document Draft", path: "/draft" },
    { link: "Translation", heading: "Translation", path: "/translation" },
    { link: "Settings", heading: "Settings", path: "/settings" },
  ];

  for (const route of routes) {
    test(`navigates to ${route.path} via sidebar`, async ({ page }) => {
      await page.goto("/review");
      const sidebar = page.locator("aside.sidebar");
      await sidebar.getByText(route.link, { exact: true }).first().click();
      await expect(page).toHaveURL(new RegExp(route.path));
      await expect(page.getByRole("heading", { name: route.heading }).first()).toBeVisible();
    });
  }
});
