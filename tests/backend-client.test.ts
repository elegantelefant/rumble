// ABOUTME: Tests for mock backend client functions.
// ABOUTME: Verifies return shapes, delays, and content of all mock stubs.

import {
  backendRegistry,
  mockEvalsRun,
  mockTestSync,
} from "../src/modules/backend/backendClient"

beforeEach(() => {
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

describe("backendRegistry", () => {
  it("contains expected command entries", () => {
    const commands = backendRegistry.value.map((d) => d.command)
    expect(commands).toContain("documents/register_files")
    expect(commands).toContain("documents/initial_review")
    expect(commands).toContain("documents/chat")
    expect(commands).toContain("research/run_query")
    expect(commands).toContain("translation/run")
    expect(commands).toContain("settings/save")
    expect(commands).toContain("sync/test")
  })

  it("each entry has required fields", () => {
    for (const doc of backendRegistry.value) {
      expect(doc.command).toBeTruthy()
      expect(doc.description).toBeTruthy()
      expect(doc.expectedPayload).toBeDefined()
      expect(doc.notes).toBeTruthy()
    }
  })
})

describe("mockEvalsRun", () => {
  it("returns completed benchmark", async () => {
    const promise = mockEvalsRun("accuracy", ["gpt-4o"])
    await vi.advanceTimersByTimeAsync(550)
    const result = await promise
    expect(result.benchmarkId).toBeTruthy()
    expect(result.status).toBe("completed")
    expect(result.startedAt).toBeTruthy()
  })
})

describe("mockTestSync", () => {
  it("returns ok with message referencing URL", async () => {
    const promise = mockTestSync("https://sync.example.com")
    await vi.advanceTimersByTimeAsync(200)
    const result = await promise
    expect(result.ok).toBe(true)
    expect(result.message).toContain("sync.example.com")
  })
})
