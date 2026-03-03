// ABOUTME: Tests for mock backend client functions.
// ABOUTME: Verifies return shapes, delays, and content of all mock stubs.

import {
  backendRegistry,
  mockRegisterReview,
  mockInitialReview,
  mockDocumentChat,
  mockResearchRun,
  mockTranslationRun,
  mockEvalsRun,
  mockSaveSettings,
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

describe("mockRegisterReview", () => {
  it("returns one ReviewSummary per file", async () => {
    const promise = mockRegisterReview([
      { id: "f1", name: "Contract.pdf", sizeBytes: 1000 },
      { id: "f2", name: "NDA.pdf", sizeBytes: 2000 },
    ])
    await vi.advanceTimersByTimeAsync(300)
    const result = await promise
    expect(result).toHaveLength(2)
    expect(result[0].sessionId).toBe("f1")
    expect(result[1].sessionId).toBe("f2")
  })

  it("includes summary referencing file name", async () => {
    const promise = mockRegisterReview([
      { id: "f1", name: "Contract.pdf", sizeBytes: 1000 },
    ])
    await vi.advanceTimersByTimeAsync(300)
    const result = await promise
    expect(result[0].summary).toContain("Contract.pdf")
  })

  it("includes generatedAt timestamp", async () => {
    const promise = mockRegisterReview([
      { id: "f1", name: "test.pdf", sizeBytes: 100 },
    ])
    await vi.advanceTimersByTimeAsync(300)
    const result = await promise
    expect(result[0].generatedAt).toBeTruthy()
  })
})

describe("mockInitialReview", () => {
  it("returns session response with messages", async () => {
    const promise = mockInitialReview("session-1")
    await vi.advanceTimersByTimeAsync(350)
    const result = await promise
    expect(result.sessionId).toBe("session-1")
    expect(result.summary).toBeTruthy()
    expect(result.messages).toHaveLength(1)
    expect(result.messages[0].role).toBe("assistant")
  })
})

describe("mockDocumentChat", () => {
  it("returns assistant message referencing question", async () => {
    const promise = mockDocumentChat("s-1", "What is the term?")
    await vi.advanceTimersByTimeAsync(250)
    const result = await promise
    expect(result.role).toBe("assistant")
    expect(result.content).toContain("What is the term?")
    expect(result.id).toBeTruthy()
    expect(result.timestamp).toBeTruthy()
  })
})

describe("mockResearchRun", () => {
  it("returns answer with citations", async () => {
    const promise = mockResearchRun("t-1", "negligence elements")
    await vi.advanceTimersByTimeAsync(450)
    const result = await promise
    expect(result.threadId).toBe("t-1")
    expect(result.answer).toContain("negligence elements")
    expect(result.citations).toHaveLength(2)
  })
})

describe("mockTranslationRun", () => {
  it("returns translated text with language markers", async () => {
    const promise = mockTranslationRun({
      sourceLanguage: "fr",
      targetLanguage: "en",
      text: "Bonjour",
      model: "test",
    })
    await vi.advanceTimersByTimeAsync(250)
    const result = await promise
    expect(result.jobId).toBeTruthy()
    expect(result.translatedText).toContain("fr→en")
    expect(result.translatedText).toContain("Bonjour")
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

describe("mockSaveSettings", () => {
  it("resolves without error", async () => {
    const promise = mockSaveSettings()
    await vi.advanceTimersByTimeAsync(200)
    await expect(promise).resolves.toBeUndefined()
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
