// ABOUTME: Tests for waitForJob's timeout behaviour -- the real implementation,
// ABOUTME: not mocked, so the message and poll duration are actually exercised.

import { invoke } from "@tauri-apps/api/core"
import { flushPromises, mount } from "@vue/test-utils"
import { waitForJob } from "../src/api/sidecar"
import DocumentDraftView from "../src/views/DocumentDraftView.vue"
import DocumentReviewView from "../src/views/DocumentReviewView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  vi.useFakeTimers()
  // Every poll comes back "running" -- the job never finishes, so
  // waitForJob is forced all the way to its timeout branch.
  vi.mocked(invoke).mockResolvedValue({ id: "job-1", status: "running", result: null })
})

afterEach(() => {
  vi.useRealTimers()
})

describe("waitForJob timeout", () => {
  it("reports actual elapsed milliseconds on every poll including the terminal poll", async () => {
    const onElapsed = vi.fn()
    vi.mocked(invoke)
      .mockResolvedValueOnce({ status: "running" })
      .mockImplementationOnce(() => new Promise((resolve) => {
        setTimeout(() => resolve({ status: "running" }), 500)
      }))
      .mockResolvedValueOnce({ status: "completed", result: {} })

    const promise = waitForJob("draft", "job-1", onElapsed)
    await vi.runAllTimersAsync()
    await expect(promise).resolves.toMatchObject({ status: "completed" })
    expect(onElapsed.mock.calls).toEqual([[0], [1500], [3500]])
    expect(invoke).toHaveBeenCalledTimes(3)
    expect(vi.getTimerCount()).toBe(0)
  })

  it("throws a plain draft message with no job ID in it", async () => {
    const promise = waitForJob("draft", "abc-123-def-456")
    // .rejects attaches a handler synchronously, before the timers that
    // actually trigger the rejection are advanced -- otherwise the promise
    // rejects during runAllTimersAsync with nothing listening yet.
    const assertion = expect(promise).rejects.toThrow("Drafting took too long. Please try again.")
    await vi.runAllTimersAsync()
    await assertion
  })

  it("the draft timeout message contains no identifier at all", async () => {
    const promise = waitForJob("draft", "abc-123-def-456")
    const assertion = promise.catch((error: Error) => error)
    await vi.runAllTimersAsync()
    const error = await assertion
    expect(error.message).not.toContain("abc-123-def-456")
    expect(error.message).not.toMatch(/[0-9a-f]{8}-[0-9a-f]{4}/i) // no UUID-shaped fragment
  })

  it("the review timeout message also contains no identifier", async () => {
    const promise = waitForJob("review", "review-job-789")
    const assertion = promise.catch((error: Error) => error)
    await vi.runAllTimersAsync()
    const error = await assertion
    expect(error.message).not.toContain("review-job-789")
    expect(error.message).not.toMatch(/[0-9a-f]{8}-[0-9a-f]{4}/i)
  })

  it("waits at least as long as the server's 300s job timeout before giving up", async () => {
    const promise = waitForJob("draft", "job-1")
    const assertion = expect(promise).rejects.toThrow()
    let settled = false
    promise.then(
      () => (settled = true),
      () => (settled = true),
    )
    await vi.advanceTimersByTimeAsync(300_000)
    // Still polling at the server's own timeout mark -- not yet rejected.
    expect(settled).toBe(false)
    await vi.runAllTimersAsync()
    await assertion
  })

  it("review gets its own plain message, matching draft's shape", async () => {
    const promise = waitForJob("review", "review-job-1")
    const assertion = expect(promise).rejects.toThrow("Review took too long. Please try again.")
    await vi.runAllTimersAsync()
    await assertion
  })
})

describe("job progress in views", () => {
  for (const kind of ["draft", "review"] as const) {
    for (const outcome of ["completed", "failed", "rejected", "timeout"] as const) {
      it(`${kind} shows progress from 60 s and clears it on ${outcome}`, async () => {
        let status = "running"
        const addToast = vi.fn()
        const consoleError = vi.spyOn(console, "error").mockImplementation(() => {})
        vi.mocked(invoke).mockImplementation(async (command) => {
          if (command === "get_backend_mode") return "ollama"
          if (command === "extract_document") return "Contract text."
          if (status === "rejected") throw new Error("Poll unavailable")
          return {
            id: "job-1", job_id: "job-1", status,
            result: { draft: "Finished draft.", summary: "Finished review.", issues: [] },
          }
        })
        const wrapper = mount(kind === "draft" ? DocumentDraftView : DocumentReviewView, {
          global: { provide: { [TOAST_KEY as symbol]: { addToast } } },
        })
        try {
          if (kind === "draft") {
            await wrapper.find('input[placeholder="Full name"]').setValue("Jane Doe")
            await wrapper.find('input[type="date"]').setValue("2026-01-01")
            await wrapper.find('input[placeholder="$100,000"]').setValue("120000")
            await wrapper.find('input[placeholder="Role"]').setValue("Engineer")
            await wrapper.find("button.btn-primary").trigger("click")
          } else {
            const input = wrapper.find('input[type="file"]')
            Object.defineProperty(input.element, "files", {
              value: [new File(["Contract text."], "contract.txt", { type: "text/plain" })],
            })
            await input.trigger("change")
          }
          await flushPromises()
          expect(wrapper.find('[role="status"]').exists()).toBe(false)
          await vi.advanceTimersByTimeAsync(59_999)
          expect(wrapper.find('[role="status"]').exists()).toBe(false)
          await vi.advanceTimersByTimeAsync(1)
          expect(wrapper.find('[role="status"]').text()).toBe("Still working, about 1 min so far.")
          await vi.advanceTimersByTimeAsync(60_000)
          expect(wrapper.findAll('[role="status"]')).toHaveLength(1)
          expect(wrapper.find('[role="status"]').text()).toBe("Still working, about 2 min so far.")

          if (outcome === "timeout") {
            await vi.runAllTimersAsync()
          } else {
            status = outcome
            await vi.advanceTimersByTimeAsync(1500)
          }
          await flushPromises()
          expect(wrapper.find('[role="status"]').exists()).toBe(false)
          expect(vi.getTimerCount()).toBe(0)
          if (outcome === "completed") {
            expect(wrapper.text()).toContain(kind === "draft" ? "Finished draft." : "Finished review.")
          } else {
            expect(addToast).toHaveBeenCalledWith(expect.any(String), "error")
          }
        } finally {
          wrapper.unmount()
          consoleError.mockRestore()
        }
      })
    }
  }
})
