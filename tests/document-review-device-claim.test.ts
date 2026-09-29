// ABOUTME: Tests for DocumentReviewView.vue's "Files never leave this device" copy.
// ABOUTME: The claim shows only in local (Ollama) mode, read from get_backend_mode.

import { mount, flushPromises } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"

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

const DEVICE_CLAIM = "Files never leave this device."

async function mountInMode(mode: string) {
  vi.mocked(invoke).mockResolvedValue(mode)
  const wrapper = mount(DocumentReviewView, {
    global: { provide: { [TOAST_KEY as symbol]: { addToast: vi.fn() } } },
  })
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("DocumentReviewView device claim", () => {
  it("says files never leave this device in local mode", async () => {
    const wrapper = await mountInMode("ollama")
    expect(wrapper.text()).toContain(DEVICE_CLAIM)
  })

  it("makes no device claim in BYOK mode", async () => {
    const wrapper = await mountInMode("byok")
    expect(wrapper.text()).not.toContain(DEVICE_CLAIM)
  })

  it("makes no device claim in Premium mode", async () => {
    const wrapper = await mountInMode("premium")
    expect(wrapper.text()).not.toContain(DEVICE_CLAIM)
  })

  it("makes no device claim when the mode cannot be read", async () => {
    vi.mocked(invoke).mockRejectedValue("no backend")
    const wrapper = mount(DocumentReviewView, {
      global: { provide: { [TOAST_KEY as symbol]: { addToast: vi.fn() } } },
    })
    await flushPromises()
    expect(wrapper.text()).not.toContain(DEVICE_CLAIM)
  })
})
