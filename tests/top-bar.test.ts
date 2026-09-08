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

describe("TopBar confidentiality indicator", () => {
  it("shows Local & Confidential for ollama", async () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("shows Direct to Provider for byok, not Local & Confidential", async () => {
    vi.mocked(invoke).mockResolvedValue("byok")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Direct to Provider")
    expect(wrapper.text()).not.toContain("Local & Confidential")
  })

  it("shows the hybrid state for premium", async () => {
    vi.mocked(invoke).mockResolvedValue("premium")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).toContain("Hybrid")
    expect(wrapper.text()).not.toContain("Local & Confidential")
  })

  it("does not claim confidentiality when the mode cannot be read", async () => {
    vi.mocked(invoke).mockRejectedValue("IPC error")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).not.toContain("Local & Confidential")
    expect(wrapper.text()).toContain("Mode unavailable")
  })

  it("does not claim confidentiality for an unrecognised mode", async () => {
    vi.mocked(invoke).mockResolvedValue("something-new")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).not.toContain("Local & Confidential")
  })

  it("exposes the full message as a tooltip", async () => {
    vi.mocked(invoke).mockResolvedValue("byok")
    const wrapper = mountTopBar()
    await flushPromises()
    const pill = wrapper.find("span[title]")
    expect(pill.attributes("title")).toContain("your API key")
  })

  it("calls get_backend_mode on mount", () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    mountTopBar()
    expect(invoke).toHaveBeenCalledWith("get_backend_mode")
  })
})
