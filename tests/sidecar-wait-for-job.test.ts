// ABOUTME: Tests for waitForJob's timeout behaviour -- the real implementation,
// ABOUTME: not mocked, so the message and poll duration are actually exercised.

import { invoke } from "@tauri-apps/api/core"
import { waitForJob } from "../src/api/sidecar"

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
