// ABOUTME: Tests for SetupView.vue's readiness messaging.
// ABOUTME: Covers showing the sidecar's /ready reason instead of a generic "not responding".

import { flushPromises, mount } from "@vue/test-utils"
import { vi } from "vitest"
import { health, ready } from "../src/api/sidecar"
import SetupView from "../src/views/SetupView.vue"

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

vi.mock("../src/router", () => ({
  setupVerified: { value: false },
}))

vi.mock("../src/api/sidecar", () => ({
  health: vi.fn(),
  ready: vi.fn(),
  listModels: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(health).mockResolvedValue({ status: "ok", mode: "ollama" })
})

describe("SetupView", () => {
  it("shows the sidecar's refusal reason when not ready", async () => {
    vi.mocked(ready).mockResolvedValue({
      status: "not_ready",
      mode: "ollama",
      error: "OLLAMA_BASE_URL must be loopback, got 'http://evil.example.com:11434'",
    })

    const wrapper = mount(SetupView)
    await flushPromises()

    expect(wrapper.text()).toContain("OLLAMA_BASE_URL must be loopback")
  })

  it("falls back to 'not responding' when not ready without a reason", async () => {
    vi.mocked(ready).mockResolvedValue({ status: "not_ready", mode: "ollama" })

    const wrapper = mount(SetupView)
    await flushPromises()

    expect(wrapper.text()).toContain("Ollama is not responding")
  })
})
