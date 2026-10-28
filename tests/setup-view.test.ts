// ABOUTME: Tests for SetupView.vue's readiness messaging.
// ABOUTME: Covers showing the sidecar's /ready reason instead of a generic "not responding", and the no-model path.

import { flushPromises, mount } from "@vue/test-utils"
import { vi } from "vitest"
import { defineComponent } from "vue"
import { createMemoryHistory, createRouter, routerKey } from "vue-router"
import { createReviewJob, extractDocument, health, listModels, ready, waitForJob } from "../src/api/sidecar"
import SetupView from "../src/views/SetupView.vue"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import sampleNda from "../src/assets/sample-nda.txt?raw"
import { setupVerified } from "../src/router"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => "ollama"),
}))

vi.mock("../src/router", () => ({
  setupVerified: { value: false },
}))

vi.mock("../src/api/sidecar", () => ({
  health: vi.fn(),
  ready: vi.fn(),
  listModels: vi.fn(),
  createChat: vi.fn(),
  createReviewJob: vi.fn(),
  extractDocument: vi.fn(),
  sendMessage: vi.fn(),
  streamMessage: vi.fn(),
  waitForJob: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(health).mockResolvedValue({ status: "ok", mode: "ollama" })
})

function mountSetup() {
  return mount(SetupView, {
    global: {
      provide: {
        [routerKey as symbol]: createRouter({ history: createMemoryHistory(), routes: [] }),
      },
    },
  })
}

describe("SetupView", () => {
  it("queues the bundled sample NDA review when the ready-state button is clicked", async () => {
    setupVerified.value = false
    vi.mocked(ready).mockResolvedValue({ status: "ready", mode: "ollama" })
    vi.mocked(listModels).mockResolvedValue({ models: [{ id: "llama3.2", name: "llama3.2" }] })
    vi.mocked(extractDocument).mockImplementation(async (file) => file.text())
    vi.mocked(createReviewJob).mockResolvedValue({ job_id: "sample-job" } as never)
    // Keep the real initial-review path pending after its job has been queued.
    vi.mocked(waitForJob).mockReturnValue(new Promise(() => {}))
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/setup", component: SetupView },
        { path: "/review", component: DocumentReviewView },
      ],
    })
    await router.push("/setup")
    const wrapper = mount(defineComponent({ template: "<router-view />" }), {
      global: {
        plugins: [router],
        provide: { [TOAST_KEY as symbol]: { addToast: vi.fn() } },
      },
    })
    try {
      await flushPromises()
      expect(createReviewJob).not.toHaveBeenCalled()
      const button = wrapper.findAll("button").find((button) => button.text() === "Review a sample NDA")
      expect(button).toBeDefined()
      await button!.trigger("click")
      await flushPromises()

      expect(setupVerified.value).toBe(true)
      expect(router.currentRoute.value.path).toBe("/review")
      expect(router.currentRoute.value.query.sample).toBeUndefined()
      expect(createReviewJob).toHaveBeenCalledExactlyOnceWith({ text: sampleNda, instructions: undefined })
      expect(extractDocument).toHaveBeenCalledTimes(1)
      expect(sampleNda).toContain("MUTUAL NON-DISCLOSURE AGREEMENT")
      expect(wrapper.text()).toContain("Sample document — synthetic NDA with fictional parties.")
      expect(wrapper.text()).toContain("Review in progress")
    } finally {
      wrapper.unmount()
    }
  })

  it("shows the sidecar's refusal reason when not ready", async () => {
    vi.mocked(ready).mockResolvedValue({
      status: "not_ready",
      mode: "ollama",
      error: "OLLAMA_BASE_URL must be loopback, got 'http://evil.example.com:11434'",
    })

    const wrapper = mountSetup()
    await flushPromises()

    expect(wrapper.text()).toContain("OLLAMA_BASE_URL must be loopback")
  })

  it("asks for a model pull when Ollama has no usable local model", async () => {
    vi.mocked(ready).mockResolvedValue({
      status: "not_ready",
      mode: "ollama",
      error: "no local Ollama model is pulled; cloud models are not used in local mode",
      checks: { models: "none" },
    })

    const wrapper = mountSetup()
    await flushPromises()

    expect([wrapper.text().includes("Pull a model"), wrapper.text().includes("cloud models are not used")]).toEqual([
      true,
      true,
    ])
  })

  it("falls back to 'not responding' when not ready without a reason", async () => {
    vi.mocked(ready).mockResolvedValue({ status: "not_ready", mode: "ollama" })

    const wrapper = mountSetup()
    await flushPromises()

    expect(wrapper.text()).toContain("Ollama is not responding")
  })
})
