// ABOUTME: E2e tests for the Document Draft view.
// ABOUTME: Validates template selection, form fields, validation, and draft generation.
import { expect, test } from "@playwright/test";

test.describe("Document Draft", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/draft");
  });

  test("renders page heading", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Document Draft" })).toBeVisible();
    await expect(page.getByText("Generate drafts from your local templates")).toBeVisible();
  });

  test("shows template library with three templates", async ({ page }) => {
    const sidebar = page.locator("aside.card");
    await expect(sidebar.getByText("Employment Agreement")).toBeVisible();
    await expect(sidebar.getByText("Non-Disclosure Agreement")).toBeVisible();
    await expect(sidebar.getByText("Service Contract")).toBeVisible();
  });

  test("employment template shows correct fields", async ({ page }) => {
    await expect(page.getByText("Employee Name")).toBeVisible();
    await expect(page.getByText("Start Date")).toBeVisible();
    await expect(page.getByText("Salary")).toBeVisible();
    await expect(page.getByText("Position")).toBeVisible();
  });

  test("switching template updates fields", async ({ page }) => {
    await page.getByText("Non-Disclosure Agreement").click();
    await expect(page.getByText("Disclosing Party")).toBeVisible();
    await expect(page.getByText("Receiving Party")).toBeVisible();
    await expect(page.getByText("Duration")).toBeVisible();
  });

  test("shows validation errors for empty required fields", async ({ page }) => {
    await page.getByRole("button", { name: "Generate Draft" }).click();
    await expect(page.getByText("Employee name is required")).toBeVisible();
  });

  test("export buttons are present", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Export to Word" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Export to PDF" })).toBeVisible();
  });

  test("data privacy notice is visible", async ({ page }) => {
    await expect(page.getByText("Sensitive data stays local")).toBeVisible();
  });
});
