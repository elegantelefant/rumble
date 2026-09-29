// ABOUTME: Tests for streamMessage in src/api/sidecar.ts, which consumes the host's stream_message IPC channel.
// ABOUTME: Covers delta order, settling on the terminal event, error propagation, and that no secret is requested.

import { invoke } from "@tauri-apps/api/core"
import { streamMessage } from "../src/api/sidecar"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: class {
    onmessage: (message: unknown) => void = () => {}
  },
}))

type Event = { type: "delta" | "done" | "error"; value: string }
type ChannelLike = { onmessage: (message: Event) => void }

/** Makes invoke deliver `events` over the channel it is handed, then resolve. */
function hostSends(events: Event[]) {
  vi.mocked(invoke).mockImplementation(async (_cmd, args) => {
    const channel = (args as { onEvent: ChannelLike }).onEvent
    for (const event of events) channel.onmessage(event)
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("streamMessage", () => {
  it("passes each delta to onDelta in the order the host sent them", async () => {
    hostSends([
      { type: "delta", value: "Hel" },
      { type: "delta", value: "lo" },
      { type: "done", value: "Hello" },
    ])
    const deltas: string[] = []
    await streamMessage("chat-1", "hi", (d) => deltas.push(d))
    expect(deltas).toEqual(["Hel", "lo"])
  })

  it("resolves with the complete text from the done event", async () => {
    hostSends([
      { type: "delta", value: "Hel" },
      { type: "done", value: "Hello" },
    ])
    await expect(streamMessage("chat-1", "hi", () => {})).resolves.toBe("Hello")
  })

  it("rejects with the host's error message as a string", async () => {
    hostSends([
      { type: "delta", value: "par" },
      { type: "error", value: "ollama unreachable" },
    ])
    await expect(streamMessage("chat-1", "hi", () => {})).rejects.toBe("ollama unreachable")
  })

  it("rejects when the stream_message command itself fails", async () => {
    vi.mocked(invoke).mockRejectedValue("command stream_message not found")
    await expect(streamMessage("chat-1", "hi", () => {})).rejects.toBe(
      "command stream_message not found",
    )
  })

  it("waits for the terminal event even when the command returns first", async () => {
    let channel!: ChannelLike
    vi.mocked(invoke).mockImplementation(async (_cmd, args) => {
      channel = (args as { onEvent: ChannelLike }).onEvent
    })
    let settled = false
    const result = streamMessage("chat-1", "hi", () => {}).then((text) => {
      settled = true
      return text
    })
    await vi.waitFor(() => expect(channel).toBeDefined())
    await Promise.resolve()
    expect(settled).toBe(false)
    channel.onmessage({ type: "done", value: "late" })
    await expect(result).resolves.toBe("late")
  })

  it("sends chat id, text and model to stream_message and invokes nothing else", async () => {
    hostSends([{ type: "done", value: "" }])
    await streamMessage("chat-1", "hi", () => {}, "llama3.2")
    expect(vi.mocked(invoke).mock.calls.map(([cmd]) => cmd)).toEqual(["stream_message"])
    expect(vi.mocked(invoke).mock.calls[0][1]).toMatchObject({
      chatId: "chat-1",
      text: "hi",
      model: "llama3.2",
    })
  })
})
