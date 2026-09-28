// ABOUTME: Tests for DocumentReviewView.vue's file intake via extractDocument.
// ABOUTME: Covers routing every upload through extraction and surfacing its error message.

import { mount, flushPromises } from "@vue/test-utils"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import { createReviewJob, extractDocument, waitForJob } from "../src/api/sidecar"

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

async function uploadFile(wrapper: ReturnType<typeof mountReview>, file: File) {
  const input = wrapper.find('input[type="file"]')
  Object.defineProperty(input.element, "files", { value: [file], configurable: true })
  await input.trigger("change")
  await flushPromises()
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("DocumentReviewView file intake", () => {
  it("sends the uploaded file through extractDocument", async () => {
    vi.mocked(extractDocument).mockResolvedValue("Clause one.")
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
    vi.mocked(waitForJob).mockResolvedValue({
      status: "completed",
      result: { summary: "Summary.", issues: [] },
    } as never)

    const wrapper = mountReview()
    const file = new File(["Clause one."], "contract.pdf", { type: "application/pdf" })
    await uploadFile(wrapper, file)

    expect(extractDocument).toHaveBeenCalledWith(file)
  })

  it("shows the extraction error's message in a toast", async () => {
    vi.mocked(extractDocument).mockRejectedValue(
      "No text found in this PDF. It may be a scan or image-only document.",
    )

    const wrapper = mountReview()
    const file = new File(["scanned bytes"], "scan.pdf", { type: "application/pdf" })
    await uploadFile(wrapper, file)

    expect(mockAddToast).toHaveBeenCalledWith(
      "No text found in this PDF. It may be a scan or image-only document.",
      "error",
    )
    expect(createReviewJob).not.toHaveBeenCalled()
  })
})
