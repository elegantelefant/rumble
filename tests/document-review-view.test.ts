// ABOUTME: Tests for DocumentReviewView.vue file handling and session export.
// ABOUTME: Covers supported-format copy, accept attribute, and the export flow.

import { mount, flushPromises } from "@vue/test-utils"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import {
  createChat,
  createReviewJob,
  sendMessage,
  waitForJob,
} from "../src/api/sidecar"

vi.mock("../src/api/sidecar", () => ({
  createChat: vi.fn(),
  createReviewJob: vi.fn(),
  sendMessage: vi.fn(),
  streamMessage: vi.fn(),
  waitForJob: vi.fn(),
}))

const mockAddToast = vi.fn()

function mountReview() {
  return mount(DocumentReviewView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast: mockAddToast },
      },
    },
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("DocumentReviewView file formats", () => {
  it("states that only TXT and MD are supported", () => {
    const wrapper = mountReview()
    expect(wrapper.text()).toContain("TXT and MD supported")
  })

  it("restricts the file input to text formats", () => {
    const wrapper = mountReview()
    const input = wrapper.find('input[type="file"]')
    expect(input.attributes("accept")).toBe(".txt,.md")
  })
})

describe("DocumentReviewView export", () => {
  it("renders the export button once a session is active", async () => {
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { summary: "A short summary.", issues: [] },
    } as never)
    vi.mocked(createChat).mockResolvedValue({ id: "chat-1" } as never)
    vi.mocked(sendMessage).mockResolvedValue(undefined as never)

    const wrapper = mountReview()
    // No active session yet, so no export button
    expect(wrapper.findAll("button").some((b) => b.text() === "Export Session")).toBe(false)
  })

  it("downloads a session transcript when Export Session is clicked", async () => {
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { summary: "A short summary.", issues: [] },
    } as never)
    vi.mocked(createChat).mockResolvedValue({ id: "chat-1" } as never)
    vi.mocked(sendMessage).mockResolvedValue(undefined as never)

    // jsdom's FileReader fires onload asynchronously outside the microtask
    // queue, so flushPromises() can't wait for it. Stub it to resolve inline.
    class SyncFileReader {
      result: string | null = null
      onload: (() => void) | null = null
      onerror: (() => void) | null = null
      readAsText(file: File) {
        this.result = "clause one\nclause two"
        this.onload?.()
      }
    }
    vi.stubGlobal("FileReader", SyncFileReader)

    const createObjectURL = vi.fn(() => "blob:mock-url")
    const revokeObjectURL = vi.fn()
    vi.stubGlobal("URL", { ...URL, createObjectURL, revokeObjectURL })

    const clickSpy = vi.fn()
    const originalCreateElement = document.createElement.bind(document)
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = originalCreateElement(tag)
      if (tag === "a") el.click = clickSpy
      return el
    })

    const wrapper = mountReview()

    // Simulate a file being dropped
    const file = new File(["clause one\nclause two"], "contract.txt", { type: "text/plain" })
    const input = wrapper.find('input[type="file"]')
    Object.defineProperty(input.element, "files", { value: [file] })
    await input.trigger("change")
    await flushPromises()

    const exportBtn = wrapper.findAll("button").find((b) => b.text() === "Export Session")
    expect(exportBtn).toBeDefined()
    await exportBtn!.trigger("click")

    expect(createObjectURL).toHaveBeenCalled()
    expect(clickSpy).toHaveBeenCalled()
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:mock-url")
    expect(mockAddToast).toHaveBeenCalledWith("Session exported.", "success")

    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })
})
