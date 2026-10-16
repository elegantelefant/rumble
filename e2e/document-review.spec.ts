// ABOUTME: E2e tests for the Document Review view.
// ABOUTME: Validates file upload UI, session list, chat interface, and workflow display.
import { expect, test } from "@playwright/test";
import * as path from "path";
import { fileURLToPath } from "url";
import { gotoRoute } from "./helpers";
import { mockTauri } from "./mockTauri";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SAMPLE_DOCUMENT = path.join(__dirname, "fixtures", "sample-document.txt");

async function uploadSampleDocument(page: import("@playwright/test").Page) {
  await page.locator('input[type="file"]').setInputFiles(SAMPLE_DOCUMENT);
}

test.describe("Document Review", () => {
  test.beforeEach(async ({ page }) => {
    await gotoRoute(page, "/review");
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

  // The app no longer ships a seeded session (#62) — these upload through a mocked IPC bridge
  // instead of expecting demo data, and assert only the uploaded filename, the summary text, and
  // the chat flow, not issue rendering (this branch predates #91's issue-rendering rework).
  test("uploading a document creates a session with a completed summary", async ({ page }) => {
    await mockTauri(page, { reviewSummary: "No material risks found in the uploaded document." });
    // beforeEach already navigated to /review; reload (not gotoRoute to the same URL again) so
    // the init script registered above actually takes effect on a fresh document.
    await page.reload();
    await uploadSampleDocument(page);

    await expect(page.getByText("sample-document.txt").first()).toBeVisible();
    await expect(page.getByText("Summary ready")).toBeVisible();
    // Appears both in the summary panel and as the final chat message — assert it's shown at all.
    await expect(page.getByText("No material risks found in the uploaded document.").first()).toBeVisible();
  });

  test("active session displays chat messages", async ({ page }) => {
    await mockTauri(page, { reviewSummary: "Summary: two obligations flagged for review." });
    await page.reload();
    await uploadSampleDocument(page);

    await expect(page.getByText("Elefant Assistant").first()).toBeVisible();
    await expect(page.getByText("Starting initial review for sample-document.txt.")).toBeVisible();
    // Appears both in the summary panel and as the final chat message — assert it's shown at all.
    await expect(page.getByText("Summary: two obligations flagged for review.").first()).toBeVisible();
  });

  // The citation-chip UI is dead code on this branch — nothing ever sets message.citations for a
  // review message (confirmed while building this mock; filed as #96, asking whether to remove
  // the chip UI or wire issue locations into it). Un-skip once #96 is resolved either way.
  test.fixme("shows citation chips in assistant messages — dead code, see #96", async ({ page }) => {
    await mockTauri(page);
    await page.reload();
    await uploadSampleDocument(page);
    await expect(page.locator(".chip").first()).toBeVisible();
  });

  test("chat input and send button are present", async ({ page }) => {
    await mockTauri(page);
    await page.reload();
    await uploadSampleDocument(page);

    const input = page.locator("#document-question");
    await expect(input).toBeVisible();
    await expect(page.getByRole("button", { name: "Send" })).toBeVisible();
  });

  test("can type and submit a follow-up question", async ({ page }) => {
    await mockTauri(page);
    await page.reload();
    await uploadSampleDocument(page);

    const input = page.locator("#document-question");
    await expect(input).toBeVisible();
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
