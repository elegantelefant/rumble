// ABOUTME: Tests for the shared backend-mode composable's ordering and knock-on effects.
// ABOUTME: Covers stale reads racing a switch, and the model list being dropped after a switch.

import { invoke } from "@tauri-apps/api/core"
import { listModels } from "../src/api/sidecar"
import { backendMode, loadBackendMode, setBackendMode } from "../src/composables/backendMode"
import { useModels } from "../src/composables/models"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

vi.mock("../src/api/sidecar", () => ({
  listModels: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  backendMode.value = null
})

describe("backend mode composable", () => {
  it("does not let a read that started before a switch overwrite it", async () => {
    let answerRead!: (mode: string) => void
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_backend_mode") return new Promise((resolve) => (answerRead = resolve))
      return undefined
    })
    const staleRead = loadBackendMode()
    await setBackendMode("byok")
    answerRead("ollama")
    await staleRead
    expect(backendMode.value).toBe("byok")
  })

  it("drops the model list on a switch, so the next load asks the new sidecar", async () => {
    vi.mocked(listModels).mockResolvedValue({
      models: [{ id: "llama3.2:latest", provider: "ollama", name: "llama3.2:latest", default: true }],
    })
    const { models, loadModels } = useModels()
    await loadModels()
    vi.mocked(invoke).mockResolvedValue(undefined)

    await setBackendMode("byok")
    expect(models.value).toEqual([])

    vi.mocked(listModels).mockResolvedValue({
      models: [{ id: "gpt-4o-mini", provider: "openai", name: "GPT-4o Mini", default: true }],
    })
    await loadModels()
    expect(models.value.map((m) => m.id)).toEqual(["gpt-4o-mini"])
  })
})
