// ABOUTME: Tests for DocumentReviewView.vue's handling of the /review result payload.
// ABOUTME: Covers ReviewIssue field rendering against the generated contract (#49).

import { mount, flushPromises } from "@vue/test-utils"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import { createChat, createReviewJob, extractDocument, sendMessage, waitForJob } from "../src/api/sidecar"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => "ollama"),
}))

vi.mock("../src/api/sidecar", () => ({
  createChat: vi.fn(),
  createReviewJob: vi.fn(),
  extractDocument: vi.fn(),
  sendMessage: vi.fn(),
  streamMessage: vi.fn(),
  waitForJob: vi.fn(),
}))

const mockAddToast = vi.fn()

function mountReview() {
  return mount(DocumentReviewView, {
    global: {
      provide: { [TOAST_KEY as symbol]: { addToast: mockAddToast } },
    },
  })
}

async function uploadFile(wrapper: ReturnType<typeof mountReview>) {
  const file = new File(["clause one"], "contract.txt", { type: "text/plain" })
  const input = wrapper.find('input[type="file"]')
  Object.defineProperty(input.element, "files", { value: [file], configurable: true })
  await input.trigger("change")
  await flushPromises()
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(extractDocument).mockResolvedValue("clause one\nclause two")
  vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
  vi.mocked(createChat).mockResolvedValue({ id: "chat-1" } as never)
  vi.mocked(sendMessage).mockResolvedValue(undefined as never)
})

describe("DocumentReviewView issue rendering (#49)", () => {
  it("renders kind as a readable label, message as the body, and location/suggestion on their own lines", async () => {
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: {
        summary: "One risk found.",
        issues: [
          {
            kind: "risk",
            message: "Liability is uncapped.",
            location: "Section 5",
            suggestion: "Cap liability at a fixed amount.",
          },
        ],
      },
    } as never)

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(wrapper.text()).toContain("Risk")
    expect(wrapper.text()).toContain("Liability is uncapped.")
    expect(wrapper.text()).toContain("Location: Section 5")
    expect(wrapper.text()).toContain("Suggestion: Cap liability at a fixed amount.")
    expect(wrapper.text()).not.toContain("Info")
  })

  it("renders the issues message with no literal markdown asterisks and its line breaks preserved in the DOM", async () => {
    // Checks the rendered element, not just the string formatReviewIssue
    // builds: this view has no markdown renderer, so a literal "**" would
    // show up as two asterisks rather than bold, and a plain <p> collapses
    // "\n" to a single space unless something tells the browser not to.
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: {
        summary: "One risk found.",
        issues: [
          {
            kind: "risk",
            message: "Liability is uncapped.",
            location: "Section 5",
            suggestion: "Cap liability at a fixed amount.",
          },
        ],
      },
    } as never)

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(wrapper.html()).not.toContain("**")

    const issuesParagraph = wrapper
      .findAll("p")
      .find((p) => p.text().includes("Liability is uncapped."))
    expect(issuesParagraph).toBeDefined()
    expect(issuesParagraph!.classes()).toContain("whitespace-pre-line")
  })

  it("omits the location and suggestion lines when the issue doesn't have them", async () => {
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: {
        summary: "One style note.",
        issues: [{ kind: "style", message: "Use defined terms consistently." }],
      },
    } as never)

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(wrapper.text()).toContain("Style")
    expect(wrapper.text()).toContain("Use defined terms consistently.")
    expect(wrapper.text()).not.toContain("Location:")
    expect(wrapper.text()).not.toContain("Suggestion:")
  })

  it("labels the contract's \"other\" kind, and any kind it doesn't recognise, as Note", async () => {
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: {
        summary: "Two notes.",
        issues: [
          { kind: "other", message: "A general observation." },
          { kind: "risk|ambiguity", message: "A kind value outside the contract's five." },
        ],
      },
    } as never)

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(wrapper.text().match(/Note/g)).toHaveLength(2)
  })
})
