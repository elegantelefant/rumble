// ABOUTME: A fake host that reviews any uploaded document, and the upload that triggers it,
// ABOUTME: shared by the e2e specs that need a reviewed document to chat with or export.
import { expect, type Page } from "@playwright/test";
import { installFakeHost } from "./fake-host";

export const REVIEWED_FILE = "Contract_2024.txt";
export const REVIEW_SUMMARY = "Renewal clauses flagged: the term renews automatically each year.";
export const FOLLOW_UP_REPLY = "Either party may terminate on 90 days' notice.";

export function installReviewHost(page: Page) {
  return installFakeHost(page, {
    extractedText: "MASTER SERVICES AGREEMENT. 1. Term. This agreement renews automatically each year.",
    routes: {
      "POST /review": { job_id: "review-1", poll_url: "/review/review-1/result" },
      "GET /review/review-1/result": { id: "review-1", status: "completed", result: { summary: REVIEW_SUMMARY, issues: [] } },
      "POST /chats": { id: "chat-1" },
      "POST /chats/chat-1/message": { id: "m-1", chat_id: "chat-1", role: "assistant", content: "Noted.", created_at: "now" },
    },
    streamedReply: ["Either party may terminate ", "on 90 days' notice."],
  });
}

/** Uploads a document on /review and waits for the fake host's review to land. */
export async function uploadReviewedDocument(page: Page) {
  // The input exists before the view's change handler is wired up; wait for the mounted view.
  await expect(page.getByRole("heading", { name: "Start a new review" })).toBeVisible();
  await page.locator('input[type="file"]').setInputFiles({
    name: REVIEWED_FILE,
    mimeType: "text/plain",
    buffer: Buffer.from("MASTER SERVICES AGREEMENT"),
  });
  await expect(page.getByText(REVIEW_SUMMARY).first()).toBeVisible();
}
