// ABOUTME: Tests for the useModels composable's sidecar loading and fallback behaviour.
// ABOUTME: Covers successful load, mapping, empty response, error fallback, and caching.

import { listModels } from "../src/api/sidecar"

vi.mock("../src/api/sidecar", () => ({
  listModels: vi.fn(),
}))

// The composable holds module-level state, so each test needs a fresh module.
async function freshUseModels() {
  vi.resetModules()
  const mod = await import("../src/composables/models")
  return mod.useModels()
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("useModels", () => {
  it("maps models from the sidecar response", async () => {
    vi.mocked(listModels).mockResolvedValue({
      models: [
        { id: "llama3.2:latest", provider: "ollama", name: "llama3.2:latest", default: true },
        { id: "gpt-4o", provider: "openai", name: "GPT-4o", default: false },
      ],
    })

    const { models, loadModels } = await freshUseModels()
    await loadModels()

    expect(models.value).toHaveLength(2)
    expect(models.value[0]).toEqual({
      id: "llama3.2:latest",
      label: "llama3.2:latest",
      provider: "ollama",
      available: true,
    })
  })

  it("returns an empty list when the sidecar reports no models", async () => {
    vi.mocked(listModels).mockResolvedValue({ models: [] })

    const { models, loadModels } = await freshUseModels()
    await loadModels()

    expect(models.value).toEqual([])
  })

  it("falls back to the local model when the sidecar is unreachable", async () => {
    vi.mocked(listModels).mockRejectedValue(new Error("sidecar down"))

    const { models, loadModels } = await freshUseModels()
    await loadModels()

    expect(models.value).toHaveLength(1)
    expect(models.value[0].id).toBe("elefant-local")
  })

  it("does not re-fetch once loaded", async () => {
    vi.mocked(listModels).mockResolvedValue({
      models: [{ id: "llama3.2", provider: "ollama", name: "llama3.2", default: true }],
    })

    const { loadModels } = await freshUseModels()
    await loadModels()
    await loadModels()

    expect(listModels).toHaveBeenCalledTimes(1)
  })

  it("re-fetches when forced", async () => {
    vi.mocked(listModels).mockResolvedValue({
      models: [{ id: "llama3.2", provider: "ollama", name: "llama3.2", default: true }],
    })

    const { loadModels } = await freshUseModels()
    await loadModels()
    await loadModels(true)

    expect(listModels).toHaveBeenCalledTimes(2)
  })

  it("availableModels filters to available entries", async () => {
    vi.mocked(listModels).mockResolvedValue({
      models: [{ id: "llama3.2", provider: "ollama", name: "llama3.2", default: true }],
    })

    const { availableModels, loadModels } = await freshUseModels()
    await loadModels()

    expect(availableModels.value).toHaveLength(1)
  })
})
