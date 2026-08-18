// ABOUTME: Tests for SettingsView.vue component.
// ABOUTME: Covers tab switching, secret management, form interactions, sync settings.

import { mount } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import SettingsView from "../src/views/SettingsView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.mocked(invoke).mockResolvedValue(undefined)
})

afterEach(() => {
  vi.useRealTimers()
})

const mockAddToast = vi.fn()

function mountSettings() {
  return mount(SettingsView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast: mockAddToast },
      },
    },
  })
}

describe("SettingsView", () => {
  it("renders page title", () => {
    const wrapper = mountSettings()
    expect(wrapper.find("h1").text()).toBe("Settings")
  })

  it("renders all four tabs", () => {
    const wrapper = mountSettings()
    expect(wrapper.text()).toContain("Providers & API keys")
    expect(wrapper.text()).toContain("Templates & workspace storage")
    expect(wrapper.text()).toContain("Appearance")
    expect(wrapper.text()).toContain("Sync")
  })

  it("defaults to providers tab", () => {
    const wrapper = mountSettings()
    expect(wrapper.text()).toContain("Configured secrets")
  })

  it("switches to appearance tab on click", async () => {
    const wrapper = mountSettings()
    const tabs = wrapper.findAll("button").filter((b) => b.text() === "Appearance")
    await tabs[0].trigger("click")
    expect(wrapper.text()).toContain("Layout preferences")
  })

  it("switches to storage tab on click", async () => {
    const wrapper = mountSettings()
    const tabs = wrapper.findAll("button").filter((b) => b.text().includes("Templates"))
    await tabs[0].trigger("click")
    expect(wrapper.text()).toContain("Template library")
  })

  it("switches to sync tab on click", async () => {
    const wrapper = mountSettings()
    const tabs = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await tabs[0].trigger("click")
    expect(wrapper.text()).toContain("Workspace sync")
  })

  // Adds a local-provider secret through the real UI flow, so removal tests
  // have something to act on now that the list starts empty.
  async function addLocalSecret(wrapper: ReturnType<typeof mountSettings>, label = "Test runtime") {
    const select = wrapper.find("select")
    await select.setValue("elefant-local")
    const labelInput = wrapper.find('input[placeholder="e.g. Drafting primary key"]')
    await labelInput.setValue(label)
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
  }

  it("starts with no configured secrets", () => {
    const wrapper = mountSettings()
    expect(wrapper.findAll("button").some((b) => b.text() === "Remove")).toBe(false)
  })

  it("shows a secret after it is added", async () => {
    const wrapper = mountSettings()
    await addLocalSecret(wrapper)
    expect(wrapper.text()).toContain("Test runtime")
  })

  it("removes secret on Remove click", async () => {
    const wrapper = mountSettings()
    await addLocalSecret(wrapper)
    const removeBtn = wrapper.findAll("button").find((b) => b.text() === "Remove")
    expect(removeBtn).toBeDefined()
    await removeBtn!.trigger("click")
    expect(wrapper.text()).not.toContain("Test runtime")
  })

  it("does not remove secret from UI when keychain delete fails", async () => {
    const wrapper = mountSettings()
    await addLocalSecret(wrapper)
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "delete_api_key") throw new Error("keychain locked")
      return undefined
    })
    const removeBtn = wrapper.findAll("button").find((b) => b.text() === "Remove")
    await removeBtn!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect(wrapper.text()).toContain("Test runtime")
    expect(mockAddToast).toHaveBeenCalledWith(
      "Could not remove credential from system keychain.",
      "error",
    )
  })

  it("does not add secret without provider selected", async () => {
    const wrapper = mountSettings()
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    expect(wrapper.findAll("button").some((b) => b.text() === "Remove")).toBe(false)
  })

  it("shows error toast when adding hosted provider without key", async () => {
    const wrapper = mountSettings()
    // Select OpenAI (requires key)
    const providerSelect = wrapper.find('select')
    await providerSelect.setValue("openai")
    // Click save without entering key
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    expect(mockAddToast).toHaveBeenCalledWith(
      "Enter the provider key before saving.",
      "error",
    )
  })

  it("adds local provider secret successfully", async () => {
    const wrapper = mountSettings()
    const providerSelect = wrapper.find('select')
    await providerSelect.setValue("elefant-local")
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    expect(mockAddToast).toHaveBeenCalledWith(
      expect.stringContaining("Secret saved"),
      "success",
    )
  })

  it("shows save settings button", () => {
    const wrapper = mountSettings()
    expect(wrapper.text()).toContain("Save settings")
  })

  it("shows toast on form submit", async () => {
    const wrapper = mountSettings()
    await wrapper.find("form").trigger("submit")
    await vi.advanceTimersByTimeAsync(0)
    expect(mockAddToast).toHaveBeenCalledWith(
      "Settings stored securely on this device.",
      "success",
    )
  })

  it("sync tab shows test connection button", async () => {
    const wrapper = mountSettings()
    const syncTab = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await syncTab[0].trigger("click")
    expect(wrapper.text()).toContain("Test connection")
  })

  it("test connection fires toast", async () => {
    const wrapper = mountSettings()
    const syncTab = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await syncTab[0].trigger("click")
    const testBtn = wrapper.findAll("button").find((b) => b.text() === "Test connection")
    await testBtn!.trigger("click")
    expect(mockAddToast).toHaveBeenCalledWith(
      expect.stringContaining("Pinging"),
      "info",
    )
  })

  it("sync tab custom server toggle enables input", async () => {
    const wrapper = mountSettings()
    const syncTab = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await syncTab[0].trigger("click")
    // Find the custom server checkbox
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    const customCheckbox = checkboxes.find(
      (cb) => cb.element.closest("label")?.textContent?.includes("custom sync"),
    )
    expect(customCheckbox).toBeDefined()
  })

  it("addSecret calls invoke store_api_key for hosted provider", async () => {
    const wrapper = mountSettings()
    const providerSelect = wrapper.find("select")
    await providerSelect.setValue("openai")
    const keyInput = wrapper.find('input[type="password"]')
    await keyInput.setValue("sk-test-key-123")
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect(invoke).toHaveBeenCalledWith("store_api_key", {
      provider: "openai",
      key: "sk-test-key-123",
    })
    expect(mockAddToast).toHaveBeenCalledWith(
      expect.stringContaining("Secret saved"),
      "success",
    )
  })

  it("loads stored keys from keychain on mount", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_api_key" && args?.provider === "openai") return "sk-stored"
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    expect(invoke).toHaveBeenCalledWith("get_api_key", { provider: "openai" })
    expect(invoke).toHaveBeenCalledWith("get_api_key", { provider: "anthropic" })
    expect(wrapper.text()).toContain("OpenAI")
  })

  it("shows error toast when keychain invoke fails", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("keychain denied"))
    const wrapper = mountSettings()
    const providerSelect = wrapper.find("select")
    await providerSelect.setValue("openai")
    const keyInput = wrapper.find('input[type="password"]')
    await keyInput.setValue("sk-test-key-456")
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect(mockAddToast).toHaveBeenCalledWith(
      expect.stringContaining("Failed to store API key"),
      "error",
    )
  })

  it("clears form after successful secret save", async () => {
    const wrapper = mountSettings()
    const providerSelect = wrapper.find("select")
    await providerSelect.setValue("openai")
    const keyInput = wrapper.find('input[type="password"]')
    await keyInput.setValue("sk-test-key-789")
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect((providerSelect.element as HTMLSelectElement).value).toBe("")
  })
})
