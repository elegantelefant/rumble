// ABOUTME: Full interactive UX walkthrough — acts as a new user.
// ABOUTME: Screenshots every page and exercises every clickable/typable interaction.
import { expect, test } from "@playwright/test";

const S = (name: string) => `screenshots/${name}.png`;

// ═══════════════════════════════════════
// 1. FIRST LAUNCH — Login & Entry
// ═══════════════════════════════════════

test.describe("1. First Launch", () => {
  test("1a. login page (auth stubbed — redirects to review)", async ({ page }) => {
    await page.goto("/login");
    await page.screenshot({ path: S("01a-login-redirect"), fullPage: true });
    // Auth is stubbed true, so /login redirects to /review
    await expect(page).toHaveURL(/\/(review|login)/);
  });

  test("1b. root URL redirects to review", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveURL(/\/review/);
    await page.screenshot({ path: S("01b-root-redirect"), fullPage: true });
  });

  test("1c. unknown URL redirects gracefully", async ({ page }) => {
    await page.goto("/nonexistent-page-xyz");
    await expect(page).toHaveURL(/\/review/);
  });
});

// ═══════════════════════════════════════
// 2. SIDEBAR NAVIGATION
// ═══════════════════════════════════════

test.describe("2. Sidebar Navigation", () => {
  test("2a. sidebar visible on desktop with all tools", async ({ page }) => {
    await page.goto("/review");
    const sidebar = page.locator("aside.sidebar");
    await expect(sidebar).toBeVisible();
    await page.screenshot({ path: S("02a-sidebar-desktop"), fullPage: true });

    // All nav items present
    await expect(sidebar.getByText("Document Review")).toBeVisible();
    await expect(sidebar.getByText("Research")).toBeVisible();
    await expect(sidebar.getByText("Document Draft")).toBeVisible();
    await expect(sidebar.getByText("Evidence Review")).toBeVisible();
    await expect(sidebar.getByText("Translation")).toBeVisible();
    await expect(sidebar.getByText("Settings")).toBeVisible();
  });

  test("2b. sidebar shows user info", async ({ page }) => {
    await page.goto("/review");
    const sidebar = page.locator("aside.sidebar");
    await expect(sidebar.getByText("Local User")).toBeVisible();
    await expect(sidebar.getByText("Rumble", { exact: true })).toBeVisible();
  });

  test("2c. sidebar Evidence Review is disabled", async ({ page }) => {
    await page.goto("/review");
    await expect(page.getByText("Coming Soon")).toBeVisible();
  });

  test("2d. sidebar navigate to each page", async ({ page }) => {
    const routes = [
      { click: "Research", heading: "Research Assistant", path: "/research" },
      { click: "Document Draft", heading: "Document Draft", path: "/draft" },
      { click: "Translation", heading: "Translation", path: "/translation" },
      { click: "Settings", heading: "Settings", path: "/settings" },
      { click: "Document Review", heading: "Document Review", path: "/review" },
    ];
    await page.goto("/review");
    for (const r of routes) {
      await page.locator("aside.sidebar").getByText(r.click, { exact: true }).first().click();
      await expect(page).toHaveURL(new RegExp(r.path));
      await expect(page.getByRole("heading", { name: r.heading }).first()).toBeVisible();
      await page.screenshot({ path: S(`02d-nav-${r.path.slice(1)}`), fullPage: true });
    }
  });

  test("2e. sidebar reorder arrows appear on hover", async ({ page }) => {
    await page.goto("/review");
    const firstNavItem = page.locator("aside.sidebar nav > div").first();
    await firstNavItem.hover();
    await page.screenshot({ path: S("02e-sidebar-hover-arrows"), fullPage: true });
  });

  test("2f. mobile sidebar hidden by default", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/review");
    await page.screenshot({ path: S("02f-mobile-sidebar-hidden"), fullPage: true });
    const sidebar = page.locator("aside.sidebar");
    await expect(sidebar).not.toBeInViewport();
  });

  test("2g. mobile hamburger opens sidebar", async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 812 });
    await page.goto("/review");
    const hamburger = page.getByRole("button", { name: "Toggle navigation" });
    await hamburger.click();
    await page.screenshot({ path: S("02g-mobile-sidebar-open"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 3. TOP BAR
// ═══════════════════════════════════════

test.describe("3. Top Bar", () => {
  test("3a. top bar shows confidentiality badge and user info", async ({ page }) => {
    await page.goto("/review");
    // Scoped by title (unique to the TopBar pill): Document Review's own
    // trust badge can show the same label text with no title attribute.
    await expect(page.getByTitle("Could not determine where requests are sent.")).toBeVisible();
    await expect(page.getByLabel("Current user").getByText("Local User")).toBeVisible();
    await page.screenshot({ path: S("03a-topbar"), fullPage: true });
  });

  test("3b. command palette opens via button", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    await page.screenshot({ path: S("03b-command-palette-open"), fullPage: true });
    await expect(page.getByPlaceholder(/Search shortcuts/)).toBeVisible();
    await expect(page.getByText("Go to Document Review")).toBeVisible();
    await expect(page.getByText("Upload documents")).toBeVisible();
    await expect(page.getByText("Start a new draft")).toBeVisible();
  });

  test("3c. command palette filters on typing", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    await page.getByPlaceholder(/Search shortcuts/).fill("draft");
    await page.screenshot({ path: S("03c-palette-filtered"), fullPage: true });
    await expect(page.getByText("Start a new draft")).toBeVisible();
  });

  test("3d. command palette navigate via click", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    await page.getByText("Start a new draft").click();
    await expect(page).toHaveURL(/\/draft/);
    await page.screenshot({ path: S("03d-palette-navigated"), fullPage: true });
  });

  test("3e. command palette close on escape", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    await expect(page.getByPlaceholder(/Search shortcuts/)).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByPlaceholder(/Search shortcuts/)).not.toBeVisible();
  });

  test("3f. command palette no matches state", async ({ page }) => {
    await page.goto("/review");
    await page.getByRole("button", { name: "Open shortcuts" }).click();
    await page.getByPlaceholder(/Search shortcuts/).fill("xyznonexistent");
    await page.screenshot({ path: S("03f-palette-no-matches"), fullPage: true });
    await expect(page.getByText("No matches")).toBeVisible();
  });
});

// ═══════════════════════════════════════
// 4. DOCUMENT REVIEW
// ═══════════════════════════════════════

test.describe("4. Document Review", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/review");
  });

  test("4a. page heading and trust badge", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Document Review" })).toBeVisible();
    // Text varies with the backend mode (honest per #56); the badge itself is what's asserted.
    await expect(page.locator(".badge-trust")).toBeVisible();
    await page.screenshot({ path: S("04a-review-heading"), fullPage: true });
  });

  test("4b. file upload area", async ({ page }) => {
    await expect(page.getByText("Drop files here or browse")).toBeVisible();
    await expect(page.getByRole("button", { name: "Browse Files" })).toBeVisible();
    await expect(page.getByText("PDF, DOCX, TXT supported")).toBeVisible();
    await page.screenshot({ path: S("04b-review-upload-area"), fullPage: true });
  });

  test("4c. custom prompt textarea", async ({ page }) => {
    const textarea = page.getByPlaceholder(/Focus on indemnity/);
    await expect(textarea).toBeVisible();
    await textarea.fill("Focus on termination and renewal clauses");
    await page.screenshot({ path: S("04c-review-custom-prompt"), fullPage: true });
  });

  test("4d. pre-seeded session visible in sidebar", async ({ page }) => {
    await expect(page.getByText("Contract_2024.pdf").first()).toBeVisible();
    await expect(page.getByText("Summary ready")).toBeVisible();
    await page.screenshot({ path: S("04d-review-session-list"), fullPage: true });
  });

  test("4e. active session shows chat with citations", async ({ page }) => {
    await expect(page.getByText("Elefant Assistant").first()).toBeVisible();
    await expect(page.getByText("Highlight the key renewal obligations")).toBeVisible();
    await expect(page.getByText("Renewal clauses flagged")).toBeVisible();
    await expect(page.getByText("Contract_2024.pdf · Section 4")).toBeVisible();
    await page.screenshot({ path: S("04e-review-chat-citations"), fullPage: true });
  });

  test("4f. type and send a follow-up question", async ({ page }) => {
    const input = page.locator("#document-question");
    await input.fill("What are the termination clauses?");
    await page.screenshot({ path: S("04f-review-typing-question"), fullPage: true });
    await page.getByRole("button", { name: "Send" }).click();
    await expect(page.getByText("What are the termination clauses?")).toBeVisible();
    await page.screenshot({ path: S("04g-review-question-sent"), fullPage: true });
  });

  test("4h. workflow steps visible", async ({ page }) => {
    await expect(page.getByText("Drop or browse for documents")).toBeVisible();
    await expect(page.getByText("Initial review auto-starts")).toBeVisible();
    await expect(page.getByText("Chat in the workspace")).toBeVisible();
    await expect(page.getByText("Return via dashboard")).toBeVisible();
    await page.screenshot({ path: S("04h-review-workflow"), fullPage: true });
  });

  test("4i. API surface toggle", async ({ page }) => {
    await page.getByRole("button", { name: "API surface" }).click();
    await expect(page.getByText("Backend expectations")).toBeVisible();
    await page.screenshot({ path: S("04i-review-api-surface"), fullPage: true });
    // Close it
    await page.getByRole("button", { name: "Hide" }).click();
    await expect(page.getByText("Backend expectations")).not.toBeVisible();
  });

  test("4j. export session button present", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Export Session" })).toBeVisible();
  });
});

// ═══════════════════════════════════════
// 5. DOCUMENT DRAFT
// ═══════════════════════════════════════

test.describe("5. Document Draft", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/draft");
  });

  test("5a. page heading and manage templates button", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Document Draft" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Manage Templates" })).toBeVisible();
    await page.screenshot({ path: S("05a-draft-heading"), fullPage: true });
  });

  test("5b. template library with three templates", async ({ page }) => {
    const sidebar = page.locator("aside.card");
    await expect(sidebar.getByText("Employment Agreement")).toBeVisible();
    await expect(sidebar.getByText("Non-Disclosure Agreement")).toBeVisible();
    await expect(sidebar.getByText("Service Contract")).toBeVisible();
    await page.screenshot({ path: S("05b-draft-templates"), fullPage: true });
  });

  test("5c. employment template fields", async ({ page }) => {
    await expect(page.getByText("Employee Name")).toBeVisible();
    await expect(page.getByText("Start Date")).toBeVisible();
    await expect(page.getByText("Salary")).toBeVisible();
    await expect(page.getByText("Position")).toBeVisible();
    await page.screenshot({ path: S("05c-draft-employment-fields"), fullPage: true });
  });

  test("5d. fill out employment form", async ({ page }) => {
    await page.getByPlaceholder("Full name").fill("Jane Smith");
    await page.getByPlaceholder("$100,000").fill("$150,000");
    await page.getByPlaceholder("Role").fill("Senior Associate");
    await page.getByPlaceholder("Enter additional clauses").fill("90-day probation period with quarterly reviews.");
    await page.screenshot({ path: S("05d-draft-form-filled"), fullPage: true });
  });

  test("5e. validation errors on empty submit", async ({ page }) => {
    await page.getByRole("button", { name: "Generate Draft" }).click();
    await expect(page.getByText("Employee name is required")).toBeVisible();
    await page.screenshot({ path: S("05e-draft-validation-errors"), fullPage: true });
  });

  test("5f. switch to NDA template", async ({ page }) => {
    await page.getByText("Non-Disclosure Agreement").click();
    await expect(page.getByText("Disclosing Party")).toBeVisible();
    await expect(page.getByText("Receiving Party")).toBeVisible();
    await expect(page.getByText("Duration")).toBeVisible();
    await page.screenshot({ path: S("05f-draft-nda-template"), fullPage: true });
  });

  test("5g. switch to Service Contract template", async ({ page }) => {
    await page.getByText("Service Contract").click();
    await expect(page.getByText("Service Provider")).toBeVisible();
    await expect(page.getByText("Client Name")).toBeVisible();
    await expect(page.getByText("Scope of Work")).toBeVisible();
    await page.screenshot({ path: S("05g-draft-service-template"), fullPage: true });
  });

  test("5h. fill NDA and generate draft", async ({ page }) => {
    await page.getByText("Non-Disclosure Agreement").click();
    await page.getByPlaceholder("Company or person").first().fill("Acme Corp");
    await page.getByPlaceholder("Company or person").last().fill("Beta Inc");
    await page.getByPlaceholder("e.g. 2 years").fill("3 years");
    await page.getByRole("button", { name: "Generate Draft" }).click();
    await page.screenshot({ path: S("05h-draft-generating"), fullPage: true });
  });

  test("5i. export buttons", async ({ page }) => {
    await expect(page.getByRole("button", { name: "Export to Word" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Export to PDF" })).toBeVisible();
    await page.getByRole("button", { name: "Export to Word" }).click();
    await page.screenshot({ path: S("05i-draft-export-word"), fullPage: true });
  });

  test("5j. privacy notice stays hidden until the mode is known", async ({ page }) => {
    // Only renders once the backend mode is known (honest per #56). e2e has
    // no real Tauri host, so the read rejects — loadBackendMode's catch resets
    // the mode to null (same as before any read), so the notice stays hidden
    // rather than showing unknown-state text. vitest's document-draft-view.test.ts
    // covers the actual per-mode text once a read succeeds.
    await expect(page.getByText("Your data stays on this device.")).not.toBeVisible();
  });

  test("5k. browse templates button shows toast", async ({ page }) => {
    await page.getByRole("button", { name: "Browse Local Templates..." }).click();
    await page.screenshot({ path: S("05k-draft-browse-toast"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 6. RESEARCH ASSISTANT
// ═══════════════════════════════════════

test.describe("6. Research Assistant", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/research");
  });

  test("6a. page heading and pre-seeded threads", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Research Assistant" })).toBeVisible();
    await expect(page.getByText("Tax compliance for SaaS contracts")).toBeVisible();
    await expect(page.getByText("GDPR data retention checklist")).toBeVisible();
    await page.screenshot({ path: S("06a-research-threads"), fullPage: true });
  });

  test("6b. thread status badges", async ({ page }) => {
    await expect(page.getByText("Complete")).toBeVisible();
    await expect(page.getByText("In Progress")).toBeVisible();
    await page.screenshot({ path: S("06b-research-badges"), fullPage: true });
  });

  test("6c. model selector defaults to local", async ({ page }) => {
    const select = page.locator("select").first();
    await expect(select).toHaveValue("elefant-local");
    await page.screenshot({ path: S("06c-research-model-select"), fullPage: true });
  });

  test("6d. create new thread", async ({ page }) => {
    await page.getByRole("button", { name: "New Thread" }).click();
    await expect(page.getByText("Untitled research thread")).toBeVisible();
    await page.screenshot({ path: S("06d-research-new-thread"), fullPage: true });
  });

  test("6e. type and submit research prompt", async ({ page }) => {
    const textarea = page.locator("#research-question");
    await textarea.fill("What are the key requirements for PDPA compliance in Singapore for SaaS companies?");
    await page.screenshot({ path: S("06e-research-typing-prompt"), fullPage: true });
    await page.getByRole("button", { name: "Start Research" }).click();
    await expect(page.getByText("PDPA compliance")).toBeVisible();
    await page.screenshot({ path: S("06f-research-prompt-submitted"), fullPage: true });
  });

  test("6g. switch between threads", async ({ page }) => {
    await page.getByText("GDPR data retention checklist").click();
    await expect(page.getByText("supervisory decisions")).toBeVisible();
    await page.screenshot({ path: S("06g-research-switch-thread"), fullPage: true });
  });

  test("6h. API surface toggle", async ({ page }) => {
    await page.getByRole("button", { name: /API surface/ }).click();
    await page.screenshot({ path: S("06h-research-api-surface"), fullPage: true });
  });

  test("6i. workflow notes visible", async ({ page }) => {
    await expect(page.getByText("Draft the request")).toBeVisible();
    await expect(page.getByText("Review generated memo")).toBeVisible();
    await expect(page.getByText("Return via history")).toBeVisible();
  });
});

// ═══════════════════════════════════════
// 7. TRANSLATION
// ═══════════════════════════════════════

test.describe("7. Translation", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/translation");
  });

  test("7a. page heading and draft quality badge", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Translation", exact: true })).toBeVisible();
    await expect(page.getByText("Draft Quality")).toBeVisible();
    await page.screenshot({ path: S("07a-translation-heading"), fullPage: true });
  });

  test("7b. pre-seeded translation history", async ({ page }) => {
    await expect(page.getByText("FR → EN")).toBeVisible();
    await expect(page.getByText("Veuillez confirmer")).toBeVisible();
    await page.screenshot({ path: S("07b-translation-history"), fullPage: true });
  });

  test("7c. language selectors and model", async ({ page }) => {
    await expect(page.getByText("Source language")).toBeVisible();
    await expect(page.getByText("Target language")).toBeVisible();
    await expect(page.getByLabel("Model")).toBeVisible();
    await page.screenshot({ path: S("07c-translation-selectors"), fullPage: true });
  });

  test("7d. change source language to German", async ({ page }) => {
    await page.getByLabel("Source language").selectOption("de");
    await page.screenshot({ path: S("07d-translation-german-src"), fullPage: true });
  });

  test("7e. type source text and translate", async ({ page }) => {
    await page.getByPlaceholder("Paste or type the passage").fill("Der Vertrag tritt am 1. Januar in Kraft.");
    await page.screenshot({ path: S("07e-translation-typing"), fullPage: true });
    await page.getByRole("button", { name: "Translate" }).click();
    await page.screenshot({ path: S("07f-translation-submitted"), fullPage: true });
  });

  test("7g. workflow tips visible", async ({ page }) => {
    await expect(page.getByText("History keeps translations per matter")).toBeVisible();
    await page.screenshot({ path: S("07g-translation-tips"), fullPage: true });
  });

  test("7h. jurisdiction disclaimer", async ({ page }) => {
    await expect(page.getByText("terminology varies across jurisdictions")).toBeVisible();
  });

  test("7i. API surface toggle", async ({ page }) => {
    await page.getByRole("button", { name: /API surface/ }).click();
    await page.screenshot({ path: S("07i-translation-api-surface"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 8. SETTINGS — All 4 Tabs
// ═══════════════════════════════════════

test.describe("8. Settings", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/settings");
  });

  test("8a. page heading and tabs", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "Settings" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Providers & API keys" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Templates & workspace storage" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Appearance" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Sync" })).toBeVisible();
    await page.screenshot({ path: S("08a-settings-tabs"), fullPage: true });
  });

  test("8b. providers tab — configured secrets", async ({ page }) => {
    await expect(page.getByText("Configured secrets")).toBeVisible();
    await expect(page.getByText("Primary local runtime")).toBeVisible();
    await page.screenshot({ path: S("08b-settings-providers"), fullPage: true });
  });

  test("8c. providers tab — add secret form", async ({ page }) => {
    await page.getByLabel("Provider").selectOption("openai");
    await page.getByPlaceholder("e.g. Drafting primary key").fill("Research key");
    await page.screenshot({ path: S("08c-settings-add-openai"), fullPage: true });
  });

  test("8d. providers tab — API key field appears for hosted provider", async ({ page }) => {
    await page.getByLabel("Provider").selectOption("openai");
    await expect(page.getByPlaceholder("sk-...")).toBeVisible();
    await page.getByPlaceholder("sk-...").fill("sk-test-key-12345");
    await page.screenshot({ path: S("08d-settings-api-key-field"), fullPage: true });
  });

  test("8e. providers tab — local config fields for Ollama", async ({ page }) => {
    await page.getByLabel("Provider").selectOption("elefant-local");
    await expect(page.getByPlaceholder("http://127.0.0.1")).toBeVisible();
    await expect(page.getByPlaceholder("11434")).toBeVisible();
    await expect(page.getByPlaceholder("elefant-legal-blend")).toBeVisible();
    await page.screenshot({ path: S("08e-settings-ollama-config"), fullPage: true });
  });

  test("8f. providers tab — save secret button", async ({ page }) => {
    await page.getByLabel("Provider").selectOption("anthropic");
    await page.getByPlaceholder("e.g. Drafting primary key").fill("Claude key");
    await page.getByPlaceholder("sk-...").fill("sk-ant-test");
    await page.getByRole("button", { name: "Save secret" }).click();
    await page.screenshot({ path: S("08f-settings-secret-saved"), fullPage: true });
  });

  test("8g. providers tab — remove secret", async ({ page }) => {
    await page.getByText("Remove").first().click();
    await page.screenshot({ path: S("08g-settings-secret-removed"), fullPage: true });
  });

  test("8h. storage tab", async ({ page }) => {
    await page.getByRole("button", { name: "Templates & workspace storage" }).click();
    await expect(page.getByText("Template library")).toBeVisible();
    await expect(page.getByText("Templates folder")).toBeVisible();
    await expect(page.getByText("Workspace data")).toBeVisible();
    await page.screenshot({ path: S("08h-settings-storage"), fullPage: true });
  });

  test("8i. storage tab — briefcases and resources", async ({ page }) => {
    await page.getByRole("button", { name: "Templates & workspace storage" }).click();
    await expect(page.getByRole("button", { name: "Add briefcase" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Attach resource" })).toBeVisible();
    await page.screenshot({ path: S("08i-settings-briefcases"), fullPage: true });
  });

  test("8j. appearance tab", async ({ page }) => {
    await page.getByRole("button", { name: "Appearance" }).click();
    await expect(page.getByText("Layout preferences")).toBeVisible();
    await page.screenshot({ path: S("08j-settings-appearance"), fullPage: true });
  });

  test("8k. appearance tab — change theme", async ({ page }) => {
    await page.getByRole("button", { name: "Appearance" }).click();
    await page.getByLabel("Theme").selectOption("dark");
    await page.screenshot({ path: S("08k-settings-theme-dark"), fullPage: true });
  });

  test("8l. sync tab", async ({ page }) => {
    await page.getByRole("button", { name: "Sync" }).click();
    await expect(page.getByText("Workspace sync")).toBeVisible();
    await expect(page.getByText("Enable sync")).toBeVisible();
    await page.screenshot({ path: S("08l-settings-sync"), fullPage: true });
  });

  test("8m. sync tab — custom server toggle", async ({ page }) => {
    await page.getByRole("button", { name: "Sync" }).click();
    await page.getByText("Use custom sync server").click();
    await page.getByPlaceholder("https://sync.myfirm.com").fill("https://sync.mylaw.com");
    await page.screenshot({ path: S("08m-settings-custom-sync"), fullPage: true });
  });

  test("8n. sync tab — test connection button", async ({ page }) => {
    await page.getByRole("button", { name: "Sync" }).click();
    await page.getByRole("button", { name: "Test connection" }).click();
    await page.screenshot({ path: S("08n-settings-test-connection"), fullPage: true });
  });

  test("8o. save settings button", async ({ page }) => {
    await page.getByRole("button", { name: "Save settings" }).click();
    await page.screenshot({ path: S("08o-settings-saved"), fullPage: true });
  });
});

// ═══════════════════════════════════════
// 9. RUNTIME HEALTH — No errors
// ═══════════════════════════════════════

test.describe("9. Runtime Health", () => {
  const routes = ["/review", "/draft", "/research", "/translation", "/settings"];

  for (const path of routes) {
    test(`no console errors on ${path}`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (err) => errors.push(err.message));
      page.on("console", (msg) => {
        if (msg.type() === "error") errors.push(msg.text());
      });
      await page.goto(path);
      await page.waitForLoadState("networkidle");
      const real = errors.filter(
        (e) => !e.includes("__TAURI__") && !e.includes("tauri") && !e.includes("invoke"),
      );
      expect(real).toEqual([]);
    });
  }
});
