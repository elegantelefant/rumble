// ABOUTME: Tests for src/api/client.ts Tauri IPC bridge.
// ABOUTME: Verifies URL parsing, query param extraction, body handling, method forwarding.

import { invoke } from "@tauri-apps/api/core"
import { apiClient } from "../src/api/client"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
})

describe("apiClient", () => {
  it("forwards GET request with path only", async () => {
    vi.mocked(invoke).mockResolvedValue({ status: "ok" })

    await apiClient("/health", { method: "GET" })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "GET",
      path: "/health",
      body: null,
      params: null,
    })
  })

  it("defaults to GET when method is omitted", async () => {
    vi.mocked(invoke).mockResolvedValue({})

    await apiClient("/chats", {})

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "GET",
      path: "/chats",
      body: null,
      params: null,
    })
  })

  it("extracts query params from URL", async () => {
    vi.mocked(invoke).mockResolvedValue({ results: [] })

    await apiClient("/search?q=contract&page=2", { method: "GET" })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "GET",
      path: "/search",
      body: null,
      params: { q: "contract", page: "2" },
    })
  })

  it("parses JSON string body", async () => {
    vi.mocked(invoke).mockResolvedValue({ id: "abc" })

    const body = JSON.stringify({ title: "Test chat" })
    await apiClient("/chats", { method: "POST", body })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "POST",
      path: "/chats",
      body: { title: "Test chat" },
      params: null,
    })
  })

  it("forwards POST method", async () => {
    vi.mocked(invoke).mockResolvedValue({})

    await apiClient("/translate", {
      method: "POST",
      body: JSON.stringify({ text: "hello", target_lang: "es" }),
    })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "POST",
      path: "/translate",
      body: { text: "hello", target_lang: "es" },
      params: null,
    })
  })

  it("forwards DELETE method", async () => {
    vi.mocked(invoke).mockResolvedValue({ status: "ok" })

    await apiClient("/chats/abc-123", { method: "DELETE" })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "DELETE",
      path: "/chats/abc-123",
      body: null,
      params: null,
    })
  })

  it("returns the invoke result", async () => {
    const expected = { id: "chat-1", title: "My Chat" }
    vi.mocked(invoke).mockResolvedValue(expected)

    const result = await apiClient("/chats/chat-1", { method: "GET" })

    expect(result).toEqual(expected)
  })

  it("propagates invoke errors", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("sidecar not running"))

    await expect(apiClient("/health", { method: "GET" })).rejects.toThrow(
      "sidecar not running",
    )
  })

  it("handles URL with no query params", async () => {
    vi.mocked(invoke).mockResolvedValue({})

    await apiClient("/models", { method: "GET" })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "GET",
      path: "/models",
      body: null,
      params: null,
    })
  })

  it("handles body that is null", async () => {
    vi.mocked(invoke).mockResolvedValue({})

    await apiClient("/chats", { method: "POST" })

    expect(invoke).toHaveBeenCalledWith("api_call", {
      method: "POST",
      path: "/chats",
      body: null,
      params: null,
    })
  })
})
