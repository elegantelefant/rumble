// ABOUTME: Tests for the Settings "Delete all local data" action.
// ABOUTME: The host confirms natively; only a confirmed deletion is reported as done.

import { mount, flushPromises } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import SettingsView from "../src/views/SettingsView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

const mockAddToast = vi.fn()

function deleteResponds(respond: () => Promise<unknown>) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) =>
    cmd === "delete_all_local_data" ? respond() : null,
  )
}

async function clickDelete() {
  const wrapper = mount(SettingsView, {
    global: {
      provide: { [TOAST_KEY as symbol]: { addToast: mockAddToast } },
      stubs: { "router-link": true },
    },
  })
  const storageTab = wrapper.findAll("button").find((b) => b.text().includes("Templates"))
  await storageTab!.trigger("click")
  const button = wrapper.findAll("button").find((b) => b.text() === "Delete all local data")
  await button!.trigger("click")
  await flushPromises()
  return wrapper
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("Delete all local data", () => {
  it("reports the deletion once the host confirms it happened", async () => {
    deleteResponds(async () => true)
    await clickDelete()
    expect(mockAddToast).toHaveBeenCalledWith("All local chats and jobs were deleted.", "success")
  })

  it("reports nothing when the user cancels the confirmation", async () => {
    deleteResponds(async () => false)
    await clickDelete()
    expect(mockAddToast).not.toHaveBeenCalled()
  })

  it("shows the host's error message when deletion fails", async () => {
    deleteResponds(async () => {
      throw "Rumble couldn't stop its local service. Quit and reopen Rumble, then try again."
    })
    await clickDelete()
    expect(mockAddToast).toHaveBeenCalledWith(
      "Rumble couldn't stop its local service. Quit and reopen Rumble, then try again.",
      "error",
    )
  })

  it("disables the button while the deletion runs", async () => {
    let finish!: (value: boolean) => void
    deleteResponds(() => new Promise((resolve) => (finish = resolve)))
    const wrapper = await clickDelete()
    const button = wrapper.findAll("button").find((b) => b.text() === "Delete all local data")
    expect(button!.attributes("disabled")).toBeDefined()
    finish(true)
    await flushPromises()
    expect(button!.attributes("disabled")).toBeUndefined()
  })
})
