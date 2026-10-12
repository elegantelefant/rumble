// ABOUTME: Screenshot capture test for visual UI/UX review.
// ABOUTME: Takes full-page screenshots of every active route for manual inspection.
import { expect, test } from "@playwright/test";
import { gotoRoute } from "./helpers";

const routes = [
  { path: "/login", name: "login" },
  { path: "/review", name: "review" },
  { path: "/draft", name: "draft" },
  { path: "/research", name: "research" },
  { path: "/translation", name: "translation" },
  { path: "/settings", name: "settings" },
];

for (const route of routes) {
  test(`screenshot ${route.name}`, async ({ page }) => {
    await gotoRoute(page, route.path);
    await page.waitForLoadState("networkidle");
    await page.screenshot({ path: `screenshots/${route.name}.png`, fullPage: true });
  });
}
