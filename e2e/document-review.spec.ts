// ABOUTME: E2e tests for the Document Review view.
// ABOUTME: Validates file upload UI, session list, chat interface, and workflow display.
import { expect, test } from "@playwright/test";

test.describe("Document Review", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/review");
  });

  test("renders page heading and description", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Document Review" })).toBeVisible();
    await expect(page.getByText("Review and chat with your documents")).toBeVisible();
  });

  test("shows local-only trust badge", async ({ page }) => {
    await expect(page.getByText("Local-only")).toBeVisible();
  });

  test("displays file upload area with browse button", async ({ page }) => {
    await expect(page.getByText("Drop files here or browse")).toBeVisible();
    await expect(page.getByRole("button", { name: "Browse Files" })).toBeVisible();
    await expect(page.getByText("PDF, DOCX, TXT supported")).toBeVisible();
  });

  test("shows pre-seeded document session", async ({ page }) => {
    await expect(page.getByText("Contract_2024.pdf").first()).toBeVisible();
    await expect(page.getByText("Summary ready")).toBeVisible();
  });

  test("active session displays chat messages", async ({ page }) => {
    await expect(page.getByText("Elefant Assistant").first()).toBeVisible();
    await expect(page.getByText("Highlight the key renewal obligations")).toBeVisible();
    await expect(page.getByText("Renewal clauses flagged")).toBeVisible();
  });

  test("shows citation chips in assistant messages", async ({ page }) => {
    await expect(page.getByText("Contract_2024.pdf · Section 4")).toBeVisible();
  });

  test("chat input and send button are present", async ({ page }) => {
    const input = page.locator("#document-question");
    await expect(input).toBeVisible();
    await expect(page.getByRole("button", { name: "Send" })).toBeVisible();
  });

  test("can type and submit a follow-up question", async ({ page }) => {
    const input = page.locator("#document-question");
    await input.fill("What are the termination clauses?");
    await page.getByRole("button", { name: "Send" }).click();
    // User message appears in chat
    await expect(page.getByText("What are the termination clauses?")).toBeVisible();
  });

  test("workflow steps are displayed", async ({ page }) => {
    await expect(page.getByText("Drop or browse for documents")).toBeVisible();
    await expect(page.getByText("Initial review auto-starts")).toBeVisible();
    await expect(page.getByText("Chat in the workspace")).toBeVisible();
    await expect(page.getByText("Return via dashboard")).toBeVisible();
  });

  test("API surface toggle works", async ({ page }) => {
    const toggleBtn = page.getByRole("button", { name: "API surface" });
    await toggleBtn.click();
    await expect(page.getByText("Backend expectations")).toBeVisible();
  });
});
