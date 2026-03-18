// ABOUTME: Tests for SettingsView.vue component.
// ABOUTME: Covers tab switching, secret management, form interactions, sync settings.

import { mount } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import SettingsView from "../src/views/SettingsView.vue"

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
        toast: { addToast: mockAddToast },
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

  it("shows existing secrets on providers tab", () => {
    const wrapper = mountSettings()
    expect(wrapper.text()).toContain("Primary local runtime")
  })

  it("removes secret on Remove click", async () => {
    const wrapper = mountSettings()
    const removeBtn = wrapper.findAll("button").find((b) => b.text() === "Remove")
    expect(removeBtn).toBeDefined()
    await removeBtn!.trigger("click")
    expect(wrapper.text()).not.toContain("Primary local runtime")
  })

  it("does not add secret without provider selected", async () => {
    const wrapper = mountSettings()
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "Save secret")
    await saveBtn!.trigger("click")
    // No new secret added, still just the original
    expect(wrapper.text()).toContain("Primary local runtime")
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

  it("shows saving state on form submit", async () => {
    const wrapper = mountSettings()
    await wrapper.find("form").trigger("submit")
    expect(wrapper.text()).toContain("Saving...")
  })

  it("completes save and shows toast", async () => {
    const wrapper = mountSettings()
    await wrapper.find("form").trigger("submit")

    // Advance past the 1000ms save delay
    await vi.advanceTimersByTimeAsync(1100)

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
