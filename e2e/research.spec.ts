// ABOUTME: E2e tests for the Research Assistant view.
// ABOUTME: Validates thread management, model selection, prompt submission, and workflow tips.
import { expect, test } from "@playwright/test";

test.describe("Research Assistant", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/research");
  });

  test("renders page heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Research Assistant" })).toBeVisible();
  });

  test("shows pre-seeded research threads", async ({ page }) => {
    await expect(page.getByText("Tax compliance for SaaS contracts")).toBeVisible();
    await expect(page.getByText("GDPR data retention checklist")).toBeVisible();
  });

  test("thread status badges display correctly", async ({ page }) => {
    await expect(page.getByText("Complete")).toBeVisible();
    await expect(page.getByText("In Progress")).toBeVisible();
  });

  test("model selector defaults to local model", async ({ page }) => {
    const select = page.locator("select").first();
    await expect(select).toHaveValue("elefant-local");
  });

  test("new thread button creates a thread", async ({ page }) => {
    await page.getByRole("button", { name: "New Thread" }).click();
    await expect(page.getByText("Untitled research thread")).toBeVisible();
  });

  test("research prompt textarea is present", async ({ page }) => {
    const textarea = page.locator("#research-question");
    await expect(textarea).toBeVisible();
  });

  test("can submit a research prompt", async ({ page }) => {
    const textarea = page.locator("#research-question");
    await textarea.fill("What are the key GDPR requirements for data retention?");
    await page.getByRole("button", { name: "Start Research" }).click();
    await expect(page.getByText("What are the key GDPR requirements for data retention?")).toBeVisible();
  });

  test("workflow notes are displayed", async ({ page }) => {
    await expect(page.getByText("Draft the request")).toBeVisible();
    await expect(page.getByText("Review generated memo")).toBeVisible();
    await expect(page.getByText("Return via history")).toBeVisible();
  });
});
