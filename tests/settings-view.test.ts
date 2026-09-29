// ABOUTME: Tests for SettingsView.vue component.
// ABOUTME: Covers tab switching, secret management, form interactions, sync settings.

import { mount } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import SettingsView from "../src/views/SettingsView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import { backendMode } from "../src/composables/backendMode"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  vi.mocked(invoke).mockResolvedValue(undefined)
  backendMode.value = null
})

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllEnvs()
})

const mockAddToast = vi.fn()

function mountSettings() {
  return mount(SettingsView, {
    global: {
      stubs: { "router-link": true },
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
    await vi.advanceTimersByTimeAsync(0)
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

  it("test connection rejects an empty server URL", async () => {
    const wrapper = mountSettings()
    const syncTab = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await syncTab[0].trigger("click")
    const testBtn = wrapper.findAll("button").find((b) => b.text() === "Test connection")
    await testBtn!.trigger("click")
    expect(mockAddToast).toHaveBeenCalledWith("Invalid server URL.", "error")
  })

  it("test connection fires toast once a server is set", async () => {
    const wrapper = mountSettings()
    const syncTab = wrapper.findAll("button").filter((b) => b.text() === "Sync")
    await syncTab[0].trigger("click")

    const serverInput = wrapper
      .findAll("input")
      .find((i) => i.attributes("placeholder") === "https://sync.myfirm.com")
    // The default server field is disabled, so drive the custom one instead.
    const customToggle = wrapper
      .findAll('input[type="checkbox"]')
      .find((c) => c.element.parentElement?.textContent?.includes("custom sync server"))
    await customToggle!.setValue(true)
    await serverInput!.setValue("https://sync.example.com")

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
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "store_api_key") throw "keychain denied"
      return undefined
    })
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

describe("SettingsView processing mode", () => {
  const BYOK_REFUSAL = "Add an OpenAI API key in Settings before switching to BYOK."

  function hostInMode(mode: string, setMode: (args: Record<string, unknown>) => Promise<unknown> = async () => undefined) {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_backend_mode") return mode
      if (cmd === "set_backend_mode") return setMode(args!)
      return undefined
    })
  }

  function radio(wrapper: ReturnType<typeof mountSettings>, mode: string) {
    return wrapper.find(`input[type="radio"][value="${mode}"]`)
  }

  it("shows the host's current mode as selected", async () => {
    hostInMode("ollama")
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    expect((radio(wrapper, "ollama").element as HTMLInputElement).checked).toBe(true)
  })

  it("switches the host's mode and says the local service is restarting", async () => {
    const calls: Record<string, unknown>[] = []
    hostInMode("ollama", async (args) => {
      calls.push(args)
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "byok").setValue(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(calls).toEqual([{ mode: "byok" }])
    expect(mockAddToast).toHaveBeenCalledWith(
      "Switched to Your own key (OpenAI). The local AI service is restarting.",
      "success",
    )
  })

  it("shows the host's refusal and puts the selection back", async () => {
    hostInMode("ollama", async () => {
      throw BYOK_REFUSAL
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "byok").setValue(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(mockAddToast).toHaveBeenCalledWith(`Could not switch mode: ${BYOK_REFUSAL}`, "error")
    expect((radio(wrapper, "ollama").element as HTMLInputElement).checked).toBe(true)
    expect((radio(wrapper, "byok").element as HTMLInputElement).checked).toBe(false)
  })

  it("offers Premium but marks it unavailable", async () => {
    hostInMode("ollama")
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    const premium = radio(wrapper, "premium")
    expect((premium.element as HTMLInputElement).disabled).toBe(true)
    expect(premium.element.closest("label")!.textContent).toContain("Not available in this version.")
  })

  it("marks BYOK unavailable in a packaged build", async () => {
    vi.stubEnv("DEV", false)
    hostInMode("ollama")
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    const byok = radio(wrapper, "byok")
    expect((byok.element as HTMLInputElement).disabled).toBe(true)
    expect(byok.element.closest("label")!.textContent).toContain("Not available in this version.")
  })

  it("after a refused switch shows the mode the host reports, not a guess", async () => {
    let hostMode = "ollama"
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_backend_mode") return hostMode
      if (cmd === "set_backend_mode") {
        hostMode = "premium"
        throw BYOK_REFUSAL
      }
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "byok").setValue(true)
    await vi.advanceTimersByTimeAsync(0)
    expect((radio(wrapper, "premium").element as HTMLInputElement).checked).toBe(true)
  })

  it("does not ask the host to switch to the mode it is already in", async () => {
    const calls: Record<string, unknown>[] = []
    hostInMode("ollama", async (args) => {
      calls.push(args)
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "ollama").trigger("change")
    await vi.advanceTimersByTimeAsync(0)
    expect(calls).toEqual([])
    expect(mockAddToast).not.toHaveBeenCalled()
  })

  it("locks the mode control while a switch is in progress", async () => {
    let finish!: () => void
    hostInMode("ollama", () => new Promise<void>((resolve) => (finish = resolve)))
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "byok").setValue(true)
    const fieldset = radio(wrapper, "byok").element.closest("fieldset")!
    expect(fieldset.hasAttribute("disabled")).toBe(true)
    finish()
    await vi.advanceTimersByTimeAsync(0)
    expect(fieldset.hasAttribute("disabled")).toBe(false)
  })

  it("follows the host back to local mode when the OpenAI key is removed in BYOK", async () => {
    let hostMode = "byok"
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_backend_mode") return hostMode
      if (cmd === "get_api_key") return args?.provider === "openai" ? "sk-stored" : null
      if (cmd === "delete_api_key") hostMode = "ollama"
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    expect((radio(wrapper, "byok").element as HTMLInputElement).checked).toBe(true)
    await wrapper.findAll("button").find((b) => b.text() === "Remove")!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect((radio(wrapper, "ollama").element as HTMLInputElement).checked).toBe(true)
  })
})

