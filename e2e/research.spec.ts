// ABOUTME: E2e tests for the Research Assistant view.
// ABOUTME: Validates thread management, model selection, prompt submission, and workflow tips.
import { expect, test } from "@playwright/test";
import { fakeHostCalls, installResearchHost, LOCAL_MODEL, RESEARCH_ANSWER } from "./fake-host";
import { gotoRoute } from "./helpers";

test.describe("Research Assistant", () => {
  test.beforeEach(async ({ page }) => {
    await gotoRoute(page, "/research");
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

  test("model selector defaults to the local model the sidecar lists", async ({ page }) => {
    await installResearchHost(page);
    await page.reload();
    await expect(page.locator("select").first()).toHaveValue(LOCAL_MODEL.id);
  });

  test("new thread button creates a thread", async ({ page }) => {
    await page.getByRole("button", { name: "New Thread" }).click();
    await expect(page.getByText("Untitled research thread")).toBeVisible();
  });

  test("research prompt textarea is present", async ({ page }) => {
    const textarea = page.locator("#research-question");
    await expect(textarea).toBeVisible();
  });

  test("can submit a research prompt and see the answer", async ({ page }) => {
    await installResearchHost(page);
    await page.reload();
    await expect(page.locator("select").first()).toHaveValue(LOCAL_MODEL.id);
    const textarea = page.locator("#research-question");
    await textarea.fill("What are the key GDPR requirements for data retention?");
    await page.getByRole("button", { name: "Start Research" }).click();
    await expect(page.getByText("What are the key GDPR requirements for data retention?")).toBeVisible();
    // Once as the thread's summary, once as the assistant's message.
    await expect(page.getByText(RESEARCH_ANSWER)).toHaveCount(2);
    const research = (await fakeHostCalls(page)).find((call) => call.path === "/research");
    expect(research?.body).toMatchObject({
      question: "What are the key GDPR requirements for data retention?",
      model: LOCAL_MODEL.id,
    });
  });

  test("workflow notes are displayed", async ({ page }) => {
    await expect(page.getByText("Draft the request")).toBeVisible();
    await expect(page.getByText("Review generated memo")).toBeVisible();
    await expect(page.getByText("Return via history")).toBeVisible();
  });
});
