// ABOUTME: Tests for DocumentReviewView.vue initial-review triggering and retry.
// ABOUTME: Covers single-fire on upload and retry after a failed review.

import { mount, flushPromises } from "@vue/test-utils"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import {
  createChat,
  createReviewJob,
  extractDocument,
  sendMessage,
  waitForJob,
} from "../src/api/sidecar"

// The view reads the backend mode on mount; the host answers local.
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
})

describe("DocumentReviewView initial review", () => {
  it("triggers exactly one review job on upload", async () => {
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { summary: "Summary.", issues: [] },
    } as never)
    vi.mocked(createChat).mockResolvedValue({ id: "chat-1" } as never)
    vi.mocked(sendMessage).mockResolvedValue(undefined as never)

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(createReviewJob).toHaveBeenCalledTimes(1)
  })

  it("retries the review when a failed session is reopened", async () => {
    vi.mocked(createReviewJob).mockRejectedValueOnce(new Error("backend down"))

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(createReviewJob).toHaveBeenCalledTimes(1)
    expect(mockAddToast).toHaveBeenCalledWith(
      "Failed to start initial review. Please try again.",
      "error",
    )

    // Second attempt succeeds
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-2" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { summary: "Recovered summary.", issues: [] },
    } as never)
    vi.mocked(createChat).mockResolvedValue({ id: "chat-2" } as never)
    vi.mocked(sendMessage).mockResolvedValue(undefined as never)

    // Reopen the session from the sessions list
    const sessionBtn = wrapper.findAll("button").find((b) => b.text().includes("contract.txt"))
    expect(sessionBtn).toBeDefined()
    await sessionBtn!.trigger("click")
    await flushPromises()

    expect(createReviewJob).toHaveBeenCalledTimes(2)
    expect(wrapper.text()).toContain("Recovered summary.")
  })

  it("keeps prior messages after a failed review", async () => {
    vi.mocked(createReviewJob).mockRejectedValue(new Error("backend down"))

    const wrapper = mountReview()
    await uploadFile(wrapper)

    expect(wrapper.text()).toContain("Initial review failed")
  })
})
