// ABOUTME: Tests for TopBar.vue confidentiality indicator and backend mode fetching.
// ABOUTME: Covers local/hybrid state mapping and graceful handling of invoke failures.

import { mount, flushPromises } from "@vue/test-utils"
import TopBar from "../src/modules/navigation/TopBar.vue"
import { invoke } from "@tauri-apps/api/core"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
})

function mountTopBar() {
  return mount(TopBar)
}

describe("TopBar", () => {
  it("shows 'Local & Confidential' when backend mode is ollama", async () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("shows 'Local & Confidential' when backend mode is byok", async () => {
    vi.mocked(invoke).mockResolvedValue("byok")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("shows the hybrid message when backend mode is premium", async () => {
    vi.mocked(invoke).mockResolvedValue("premium")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("remote agents may assist on request")
  })

  it("defaults to 'Local & Confidential' if invoke fails", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("IPC error"))
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("calls get_backend_mode on mount", () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    mountTopBar()
    expect(invoke).toHaveBeenCalledWith("get_backend_mode")
  })
})
