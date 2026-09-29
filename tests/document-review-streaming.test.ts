// ABOUTME: Tests for DocumentReviewView.vue's streamed follow-up answers.
// ABOUTME: Partial text must render while the stream is still open, not only once it finishes.

import { mount, flushPromises } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import {
  createChat,
  createReviewJob,
  extractDocument,
  sendMessage,
  streamMessage,
  waitForJob,
} from "../src/api/sidecar"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

vi.mock("../src/api/sidecar", () => ({
  createChat: vi.fn(),
  createReviewJob: vi.fn(),
  extractDocument: vi.fn(),
  sendMessage: vi.fn(),
  streamMessage: vi.fn(),
  waitForJob: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(invoke).mockResolvedValue("ollama")
  vi.mocked(extractDocument).mockResolvedValue("clause one")
  vi.mocked(createReviewJob).mockResolvedValue({ job_id: "job-1" } as never)
  vi.mocked(waitForJob).mockResolvedValue({
    status: "completed",
    result: { summary: "Summary.", issues: [] },
  } as never)
  vi.mocked(createChat).mockResolvedValue({ id: "chat-1" } as never)
  vi.mocked(sendMessage).mockResolvedValue(undefined as never)
})

async function mountWithReadySession() {
  const wrapper = mount(DocumentReviewView, {
    global: { provide: { [TOAST_KEY as symbol]: { addToast: vi.fn() } } },
  })
  const file = new File(["clause one"], "contract.txt", { type: "text/plain" })
  const input = wrapper.find('input[type="file"]')
  Object.defineProperty(input.element, "files", { value: [file], configurable: true })
  await input.trigger("change")
  await flushPromises()
  return wrapper
}

describe("DocumentReviewView streamed answer", () => {
  it("renders partial text before the stream finishes", async () => {
    let sendDelta!: (chunk: string) => void
    let finish!: (text: string) => void
    vi.mocked(streamMessage).mockImplementation((_chatId, _text, onDelta) => {
      sendDelta = onDelta
      return new Promise<string>((resolve) => (finish = resolve))
    })
    const wrapper = await mountWithReadySession()
    await wrapper.find("#document-question").setValue("What is the notice period?")
    await wrapper.find("form").trigger("submit")
    await flushPromises()

    // The placeholder has rendered empty; a delta arriving now must re-render it.
    sendDelta("The notice period ")
    await flushPromises()

    expect(wrapper.text()).toContain("The notice period")
    finish("The notice period is 30 days.")
    await flushPromises()
    expect(wrapper.text()).toContain("The notice period is 30 days.")
  })
})
