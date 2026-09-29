// ABOUTME: Tests for TopBar.vue confidentiality indicator and backend mode fetching.
// ABOUTME: Covers local/hybrid state mapping and graceful handling of invoke failures.

import { mount, flushPromises } from "@vue/test-utils"
import TopBar from "../src/modules/navigation/TopBar.vue"
import { invoke } from "@tauri-apps/api/core"
import { backendMode, loadBackendMode, setBackendMode } from "../src/composables/backendMode"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  backendMode.value = null
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
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {})
    vi.mocked(invoke).mockRejectedValue("IPC error")
    const wrapper = mountTopBar()
    await flushPromises()
    expect(wrapper.text()).not.toContain("Local & Confidential")
    expect(wrapper.text()).toContain("Mode unavailable")
    expect(consoleError).toHaveBeenCalledWith("Failed to read backend mode:", "IPC error")
    consoleError.mockRestore()
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

  it("refreshes the pill when the mode is switched, without remounting", async () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    const wrapper = mountTopBar()
    await flushPromises()
    vi.mocked(invoke).mockResolvedValue(undefined)
    await setBackendMode("byok")
    await flushPromises()
    expect(wrapper.text()).toContain("Direct to Provider")
  })

  it("keeps the pill and surfaces the host's refusal as a string when a switch fails", async () => {
    vi.mocked(invoke).mockResolvedValue("ollama")
    const wrapper = mountTopBar()
    await flushPromises()
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "set_backend_mode") throw "Add an OpenAI API key in Settings before switching to BYOK."
      return "ollama"
    })
    await expect(setBackendMode("byok")).rejects.toBe(
      "Add an OpenAI API key in Settings before switching to BYOK.",
    )
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("keeps the current pill while a switch is pending, changing only once the host confirms", async () => {
    vi.mocked(invoke).mockResolvedValue("byok")
    const wrapper = mountTopBar()
    await flushPromises()
    let confirm!: () => void
    vi.mocked(invoke).mockImplementation(() => new Promise((resolve) => (confirm = () => resolve(undefined))))
    const switching = setBackendMode("ollama")
    await flushPromises()
    expect(wrapper.text()).toContain("Direct to Provider")
    expect(wrapper.text()).not.toContain("Local & Confidential")
    confirm()
    await switching
    await flushPromises()
    expect(wrapper.text()).toContain("Local & Confidential")
  })

  it("stops claiming a mode when a later read fails", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {})
    vi.mocked(invoke).mockResolvedValue("ollama")
    const wrapper = mountTopBar()
    await flushPromises()
    vi.mocked(invoke).mockRejectedValue("IPC error")
    await loadBackendMode()
    await flushPromises()
    expect(wrapper.text()).toContain("Mode unavailable")
    consoleError.mockRestore()
  })
})

