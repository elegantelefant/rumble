// ABOUTME: Tests for the router's readiness gate and its Tauri-IPC catch narrowing.
// ABOUTME: Mocks ready() at the boundary to verify the guard's string-vs-structural-error split.

import { vi } from "vitest"
import router, { navigationError, setupVerified } from "../src/router"
import { ready } from "../src/api/sidecar"

vi.mock("../src/api/sidecar", () => ({
  ready: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  setupVerified.value = false
  navigationError.value = null
  ;(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {}
})

afterEach(() => {
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
})

describe("router readiness guard", () => {
  it("redirects to /setup when ready() rejects with a string (real backend error)", async () => {
    vi.mocked(ready).mockRejectedValue("sidecar unreachable")

    await router.push("/review")

    expect(router.currentRoute.value.path).toBe("/setup")
  })

  it("rethrows when ready() rejects with a non-string (structural IPC failure)", async () => {
    vi.mocked(ready).mockRejectedValue(
      new TypeError("Cannot read properties of undefined (reading 'invoke')"),
    )

    await expect(router.push("/review")).rejects.toThrow(TypeError)
  })

  it("sets navigationError via router.onError when a rethrown navigation is rejected", async () => {
    vi.mocked(ready).mockRejectedValue(new TypeError("boom"))

    expect(navigationError.value).toBeNull()
    await router.push("/review").catch(() => {})

    expect(navigationError.value).toBe(
      "Something went wrong reaching the local backend. Try again, or restart Rumble if it keeps happening.",
    )
  })

  it("does not set navigationError when ready() rejects with a string (handled, not rethrown)", async () => {
    vi.mocked(ready).mockRejectedValue("sidecar unreachable")

    await router.push("/review")

    expect(navigationError.value).toBeNull()
  })
})
