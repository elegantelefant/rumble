// ABOUTME: Tests for SettingsView.vue component.
// ABOUTME: Covers tab switching, secret management, settings persistence, form interactions, sync settings.

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

// router-link is stubbed: no router is installed, and resolving it warns on every render.
function mountSettings() {
  return mount(SettingsView, {
    global: {
      stubs: { "router-link": true },
      provide: {
        [TOAST_KEY as symbol]: { addToast: mockAddToast },
      },
      stubs: { RouterLink: true, "router-link": true },
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

  describe("persistence", () => {
    const saved = {
      appearance: { theme: "dark", navigationSidebar: "right" },
      workspace: { templatesPath: "/firm/templates", briefcases: ["Matter A"] },
      sync: { teamCode: "TEAM-42", useCustom: true },
      localConfig: { model: "llama3.2" },
    }

    function mockCommands(handlers: Record<string, (args?: any) => unknown>) {
      vi.mocked(invoke).mockImplementation(async (cmd: string, args?: any) => handlers[cmd]?.(args))
    }

    async function mountAndLoad() {
      const wrapper = mountSettings()
      await vi.advanceTimersByTimeAsync(0)
      return wrapper
    }

    function savedPayload() {
      const call = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "save_settings")
      return (call![1] as any).settings
    }

    async function openTab(wrapper: ReturnType<typeof mountSettings>, label: string) {
      await wrapper.findAll("button").find((b) => b.text().includes(label))!.trigger("click")
    }

    function inputByValue(wrapper: ReturnType<typeof mountSettings>, value: string) {
      return wrapper.findAll("input").find((i) => (i.element as HTMLInputElement).value === value)
    }

    async function submit(wrapper: ReturnType<typeof mountSettings>) {
      await wrapper.find("form").trigger("submit")
      await vi.advanceTimersByTimeAsync(0)
    }

    it("fills the form from saved settings on mount", async () => {
      mockCommands({ load_settings: () => saved })
      const wrapper = await mountAndLoad()
      await openTab(wrapper, "Appearance")
      const theme = wrapper.findAll("select").find((s) => s.text().includes("Match system"))!
      expect((theme.element as HTMLSelectElement).value).toBe("dark")
    })

    it("shows saved briefcases", async () => {
      mockCommands({ load_settings: () => saved })
      const wrapper = await mountAndLoad()
      await openTab(wrapper, "Templates")
      expect(wrapper.text()).toContain("Matter A")
    })

    it("fills sync settings from the saved file", async () => {
      mockCommands({ load_settings: () => saved })
      const wrapper = await mountAndLoad()
      await openTab(wrapper, "Sync")
      expect(inputByValue(wrapper, "TEAM-42")).toBeDefined()
    })

    it("fills the local runtime config from the saved file", async () => {
      mockCommands({ load_settings: () => saved })
      const wrapper = await mountAndLoad()
      await wrapper.find("select").setValue("elefant-local")
      expect(inputByValue(wrapper, "llama3.2")).toBeDefined()
    })

    it("keeps defaults for fields a partial file doesn't mention", async () => {
      mockCommands({ load_settings: () => saved, save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().workspace.templatesPath).toBe("/firm/templates")
      expect(savedPayload().workspace.workspacePath).toBe("~/Library/Application Support/Elefant/Rumble")
    })

    it("ignores saved values whose shape doesn't match the field", async () => {
      mockCommands({
        load_settings: () => ({
          workspace: { briefcases: "Matter A" },
          sync: "xy",
          appearance: { theme: 3 },
        }),
        save_settings: () => null,
      })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().workspace.briefcases).toEqual([])
      expect(savedPayload().sync.teamCode).toBe("")
      expect(savedPayload().appearance.theme).toBe("system")
    })

    it("does not carry unknown saved keys into the form", async () => {
      mockCommands({
        load_settings: () => ({ appearance: { theme: "dark", legacyColour: "teal" } }),
        save_settings: () => null,
      })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().appearance).not.toHaveProperty("legacyColour")
    })

    it("starts from blank lists and no error on first run", async () => {
      mockCommands({ load_settings: () => null, save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(mockAddToast).not.toHaveBeenCalledWith(expect.anything(), "error")
      expect(savedPayload().workspace.briefcases).toEqual([])
      expect(savedPayload().workspace.attachableResources).toEqual([])
    })

    it("keeps defaults and reports the error when saved settings can't be loaded", async () => {
      mockCommands({ load_settings: () => Promise.reject("failed to parse settings.json") })
      const wrapper = await mountAndLoad()
      expect(mockAddToast).toHaveBeenCalledWith(
        "Couldn't load saved settings: failed to parse settings.json",
        "error",
      )
      await openTab(wrapper, "Appearance")
      const theme = wrapper.findAll("select").find((s) => s.text().includes("Match system"))!
      expect((theme.element as HTMLSelectElement).value).toBe("system")
    })

    it("unlocks the form after a failed load", async () => {
      mockCommands({ load_settings: () => Promise.reject("failed to parse settings.json") })
      const wrapper = await mountAndLoad()
      expect(wrapper.find("fieldset").attributes("disabled")).toBeUndefined()
      expect(wrapper.find('button[type="submit"]').attributes("disabled")).toBeUndefined()
    })

    it("merges the groups a file has when it lacks others", async () => {
      mockCommands({ load_settings: () => ({ sync: { teamCode: "TEAM-42" } }), save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(mockAddToast).not.toHaveBeenCalledWith(expect.anything(), "error")
      expect(savedPayload().sync.teamCode).toBe("TEAM-42")
    })

    it("ignores a saved list that holds anything but strings", async () => {
      mockCommands({ load_settings: () => ({ workspace: { briefcases: [1, { name: "x" }] } }), save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().workspace.briefcases).toEqual([])
    })

    it("disables saving until saved settings have loaded", async () => {
      let finishLoad!: (value: unknown) => void
      mockCommands({ load_settings: () => new Promise((resolve) => (finishLoad = resolve)) })
      const wrapper = mountSettings()
      await vi.advanceTimersByTimeAsync(0)
      const saveButton = () => wrapper.find('button[type="submit"]')
      expect(saveButton().attributes("disabled")).toBeDefined()
      finishLoad(saved)
      await vi.advanceTimersByTimeAsync(0)
      expect(saveButton().attributes("disabled")).toBeUndefined()
    })

    it("locks the form against edits until saved settings have loaded", async () => {
      let finishLoad!: (value: unknown) => void
      mockCommands({ load_settings: () => new Promise((resolve) => (finishLoad = resolve)) })
      const wrapper = mountSettings()
      await vi.advanceTimersByTimeAsync(0)
      expect(wrapper.find("fieldset").attributes("disabled")).toBeDefined()
      finishLoad(saved)
      await vi.advanceTimersByTimeAsync(0)
      expect(wrapper.find("fieldset").attributes("disabled")).toBeUndefined()
    })

    it("does not save while the load is still pending", async () => {
      mockCommands({ load_settings: () => new Promise(() => {}), save_settings: () => null })
      const wrapper = mountSettings()
      await submit(wrapper)
      expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "save_settings")).toBe(false)
    })

    it("saves all four groups, including unsaved edits", async () => {
      mockCommands({ load_settings: () => saved, save_settings: () => null })
      const wrapper = await mountAndLoad()
      await openTab(wrapper, "Templates")
      await wrapper.find('input[placeholder="New briefcase name"]').setValue("Matter B")
      await wrapper.findAll("button").find((b) => b.text() === "Add briefcase")!.trigger("click")
      await submit(wrapper)
      const settings = savedPayload()
      expect(Object.keys(settings).sort()).toEqual(["appearance", "localConfig", "sync", "workspace"])
      expect(settings.workspace.briefcases).toEqual(["Matter A", "Matter B"])
      expect(settings.appearance.theme).toBe("dark")
    })

    it("saves the loaded sync settings", async () => {
      mockCommands({ load_settings: () => saved, save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().sync).toMatchObject({ teamCode: "TEAM-42", useCustom: true })
    })

    it("saves the loaded local runtime config", async () => {
      mockCommands({ load_settings: () => saved, save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(savedPayload().localConfig).toEqual({ host: "http://127.0.0.1", port: "11434", model: "llama3.2" })
    })

    it("confirms the save only once the host has stored it", async () => {
      mockCommands({ save_settings: () => null })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(mockAddToast).toHaveBeenCalledWith("Settings saved on this device.", "success")
    })

    it("says where an unreadable settings file was kept when the host moved it aside", async () => {
      mockCommands({ save_settings: () => "/data/settings.json.corrupt-20260929T120000Z" })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(mockAddToast).toHaveBeenCalledWith(
        "Settings saved on this device. The unreadable settings file was kept as /data/settings.json.corrupt-20260929T120000Z.",
        "success",
      )
    })

    it("reports a failed save and never claims success", async () => {
      mockCommands({ save_settings: () => Promise.reject("couldn't save settings to /data/settings.json: denied") })
      const wrapper = await mountAndLoad()
      await submit(wrapper)
      expect(mockAddToast).toHaveBeenCalledWith(
        "Couldn't save settings: couldn't save settings to /data/settings.json: denied",
        "error",
      )
      expect(mockAddToast).not.toHaveBeenCalledWith(expect.anything(), "success")
    })
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

  function hostInMode(mode: string, setMode: (args: Record<string, unknown>) => Promise<unknown> = async () => true) {
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
      return true
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

  it("with the mode unknown, asks the host first and does nothing when it is already in that mode", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {})
    let reads = 0
    const switches: Record<string, unknown>[] = []
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "get_backend_mode") {
        reads++
        if (reads === 1) throw "IPC hiccup"
        return "ollama"
      }
      if (cmd === "set_backend_mode") switches.push(args!)
      return true
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "ollama").setValue(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(switches).toEqual([])
    expect(mockAddToast).not.toHaveBeenCalled()
    expect(consoleError).toHaveBeenCalledWith("Failed to read backend mode:", "IPC hiccup")
    consoleError.mockRestore()
  })

  it("does not say the service is restarting when the host did not change mode", async () => {
    hostInMode("ollama", async () => false)
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await radio(wrapper, "byok").setValue(true)
    await vi.advanceTimersByTimeAsync(0)
    expect(mockAddToast).not.toHaveBeenCalled()
  })
})

describe("SettingsView key changes that the local service fails to follow", () => {
  const SAVED_BUT = "Key saved, but the local AI service didn't restart: spawn failed"
  const REMOVED_BUT = "Key removed, but the local AI service didn't restart: spawn failed"

  async function addOpenAiKey(wrapper: ReturnType<typeof mountSettings>, key: string) {
    await wrapper.find("select").setValue("openai")
    await wrapper.find('input[type="password"]').setValue(key)
    await wrapper.findAll("button").find((b) => b.text() === "Save secret")!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
  }

  it("lists a key the host saved before failing to restart, and says so", async () => {
    let stored: string | null = null
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "store_api_key") {
        stored = args!.key as string
        throw SAVED_BUT
      }
      if (cmd === "get_api_key") return args?.provider === "openai" ? stored : null
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await addOpenAiKey(wrapper, "sk-new")
    expect(wrapper.findAll("button").some((b) => b.text() === "Remove")).toBe(true)
    expect(mockAddToast).toHaveBeenCalledWith(SAVED_BUT, "error")
  })

  it("does not list a key the keychain refused", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "store_api_key") throw "keychain locked"
      if (cmd === "get_api_key") return null
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await addOpenAiKey(wrapper, "sk-new")
    expect(wrapper.findAll("button").some((b) => b.text() === "Remove")).toBe(false)
    expect(mockAddToast).toHaveBeenCalledWith("Failed to store API key: keychain locked", "error")
  })

  it("drops a key the host removed before failing to restart, and says so", async () => {
    let stored: string | null = "sk-old"
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "delete_api_key") {
        stored = null
        throw REMOVED_BUT
      }
      if (cmd === "get_api_key") return args?.provider === "openai" ? stored : null
      if (cmd === "get_backend_mode") return "ollama"
      return undefined
    })
    const wrapper = mountSettings()
    await vi.advanceTimersByTimeAsync(0)
    await wrapper.findAll("button").find((b) => b.text() === "Remove")!.trigger("click")
    await vi.advanceTimersByTimeAsync(0)
    expect(wrapper.findAll("button").some((b) => b.text() === "Remove")).toBe(false)
    expect(mockAddToast).toHaveBeenCalledWith(REMOVED_BUT, "error")
  })
})
