// ABOUTME: Tests for the router's readiness gate and its Tauri-IPC catch narrowing.
// ABOUTME: Mocks ready() at the boundary to verify the guard's string-vs-structural-error split.

import { vi } from "vitest"
import router, { setupVerified } from "../src/router"
import { ready } from "../src/api/sidecar"

vi.mock("../src/api/sidecar", () => ({
  ready: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
  setupVerified.value = false
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
})
