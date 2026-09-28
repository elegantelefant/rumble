// ABOUTME: Tests for extractDocument in src/api/sidecar.ts.
// ABOUTME: Verifies the file's bytes and filename header reach invoke("extract_document").

import { invoke } from "@tauri-apps/api/core"
import { extractDocument } from "../src/api/sidecar"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

beforeEach(() => {
  vi.clearAllMocks()
})

describe("extractDocument", () => {
  it("sends the file's bytes with the filename header", async () => {
    vi.mocked(invoke).mockResolvedValue("Clause one.")
    const file = new File(["Clause one."], "contract.pdf", { type: "application/pdf" })

    const result = await extractDocument(file)

    expect(result).toBe("Clause one.")
    expect(invoke).toHaveBeenCalledTimes(1)
    const [cmd, bytes, options] = vi.mocked(invoke).mock.calls[0]
    expect(cmd).toBe("extract_document")
    expect(bytes).toBeInstanceOf(Uint8Array)
    expect(new TextDecoder().decode(bytes as Uint8Array)).toBe("Clause one.")
    expect(options).toEqual({ headers: { "X-Rumble-Filename": "contract.pdf" } })
  })

  it("percent-encodes a non-ASCII filename before it reaches invoke", async () => {
    vi.mocked(invoke).mockResolvedValue("Clause one.")
    const file = new File(["Clause one."], "合同.pdf", { type: "application/pdf" })

    await extractDocument(file)

    const [, , options] = vi.mocked(invoke).mock.calls[0]
    expect(options).toEqual({
      headers: { "X-Rumble-Filename": "%E5%90%88%E5%90%8C.pdf" },
    })
  })

  it("propagates the sidecar's rejection message", async () => {
    vi.mocked(invoke).mockRejectedValue("Unsupported file type: contract.doc")
    const file = new File(["anything"], "contract.doc")

    await expect(extractDocument(file)).rejects.toBe("Unsupported file type: contract.doc")
  })
})
