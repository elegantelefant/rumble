// ABOUTME: Tests for MatterView.vue: versions with authors, Open in Word, invites and the safety number.
// ABOUTME: Covers both sides of joining, the second-admin prompt and the fork banner; invoke is the boundary.

import { mount, flushPromises } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import MatterView from "../src/views/MatterView.vue"
import { TOAST_KEY } from "../src/composables/toast"
import type { MatterDetail, FileView } from "../src/api/sync"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const addToast = vi.fn()
const ID = "ab".repeat(32)

function version(label: string, author: string, head: boolean, mine = false) {
  return { id: label.padEnd(64, "0"), label, author, mine, savedAtMs: 0, head }
}

function file(overrides: Partial<FileView> = {}): FileView {
  return {
    id: "f1",
    name: "nda.docx",
    openable: true,
    versions: [version("2", "Ben Tan", true), version("1", "Ana Lim", false, true)],
    forkedFrom: null,
    ...overrides,
  }
}

function detail(overrides: Partial<MatterDetail> = {}): MatterDetail {
  return {
    matter: { id: ID, name: "Acme NDA", folder: "/x/Acme NDA", standing: "admin", waitingFor: null, unavailable: null },
    files: [file()],
    members: [{ device: "d1", name: "Ana Lim", admin: true, mine: true }],
    requests: [],
    safetyNumber: null,
    confirmed: true,
    needsSecondAdmin: false,
    contested: [],
    notRecorded: [],
    ...overrides,
  }
}

function serve(d: MatterDetail, replies: Record<string, unknown> = {}) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => (cmd === "sync_matter" ? d : replies[cmd]))
}

async function mountMatter() {
  const wrapper = mount(MatterView, {
    props: { id: ID },
    global: { provide: { [TOAST_KEY as symbol]: { addToast } }, stubs: { RouterLink: { template: "<a><slot /></a>" } } },
  })
  await flushPromises()
  return wrapper
}

function button(wrapper: Awaited<ReturnType<typeof mountMatter>>, text: string) {
  const found = wrapper.findAll("button").find((b) => b.text() === text)
  if (!found) throw new Error(`no button "${text}"`)
  return found
}

beforeEach(() => vi.clearAllMocks())

describe("MatterView", () => {
  it("shows each version with its number and author", async () => {
    serve(detail())
    const wrapper = await mountMatter()
    expect(wrapper.text()).toContain("v2 · Ben Tan")
  })

  it("names your own versions as yours", async () => {
    serve(detail())
    const wrapper = await mountMatter()
    expect(wrapper.text()).toContain("v1 · You")
  })

  it("reminds you to save in Word after opening a file", async () => {
    serve(detail(), { sync_open_file: true })
    const wrapper = await mountMatter()
    await button(wrapper, "Open in Word").trigger("click")
    await flushPromises()
    expect(addToast).toHaveBeenCalledWith("Press Save in Word to share your changes.", "info")
  })

  it("offers only Show in folder for a type Rumble doesn't open", async () => {
    serve(detail({ files: [file({ name: "run.exe", openable: false })] }))
    const wrapper = await mountMatter()
    expect(wrapper.findAll("button").map((b) => b.text())).toContain("Show in folder")
  })

  it("shows a host refusal as an error toast", async () => {
    serve(detail())
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "sync_matter") return detail()
      throw "Only an admin of this matter can do that."
    })
    const wrapper = await mountMatter()
    await button(wrapper, "Open in Word").trigger("click")
    await flushPromises()
    expect(addToast).toHaveBeenCalledWith("Only an admin of this matter can do that.", "error")
  })

  it("shows the invite link once created", async () => {
    serve(detail(), { sync_invite: "c0ffee" })
    const wrapper = await mountMatter()
    await wrapper.find("input[name=invitee]").setValue("Ben Tan")
    await button(wrapper, "Create invite link").trigger("click")
    await flushPromises()
    expect((wrapper.find("textarea[name=invite-link]").element as HTMLTextAreaElement).value).toBe("c0ffee")
  })

  it("names the person the invite link was created for", async () => {
    serve(detail(), { sync_invite: "c0ffee" })
    const wrapper = await mountMatter()
    await wrapper.find("input[name=invitee]").setValue("Ben Tan")
    await button(wrapper, "Create invite link").trigger("click")
    await flushPromises()
    await wrapper.find("input[name=invitee]").setValue("Someone else")
    expect(wrapper.text()).toContain("Send this to Ben Tan.")
  })

  it("says when a save couldn't be added as a version", async () => {
    serve(detail({ notRecorded: ["nda.docx"] }))
    const wrapper = await mountMatter()
    expect(wrapper.text()).toContain("Your saved changes to nda.docx couldn't be added as a new version yet.")
  })

  it("asks for a second admin before a third person is invited", async () => {
    serve(detail({ needsSecondAdmin: true, members: [
      { device: "d1", name: "Ana Lim", admin: true, mine: true },
      { device: "d2", name: "Ben Tan", admin: false, mine: false },
    ] }))
    const wrapper = await mountMatter()
    expect(wrapper.text()).toContain("If this laptop is lost, nobody can add or remove people in this matter.")
  })

  describe("joining", () => {
    const waiting = () =>
      detail({
        matter: { id: ID, name: "Acme NDA", folder: "/x", standing: "waiting", waitingFor: "Ana Lim", unavailable: null },
        files: [],
        safetyNumber: "48213 90557",
        confirmed: false,
      })

    it("tells the joiner who they are waiting for", async () => {
      serve(waiting())
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("Waiting for Ana Lim to let you in.")
    })

    it("shows the joiner the number to read out", async () => {
      serve(waiting())
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("48213 90557")
    })

    it("confirms the joiner's side when they tap Numbers match", async () => {
      serve(waiting(), { sync_confirm: null })
      const wrapper = await mountMatter()
      await button(wrapper, "Numbers match").trigger("click")
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith("sync_confirm", { id: ID, number: "48213 90557" })
    })

    const request = () => detail({ requests: [{ device: "d2", name: "Ben Tan", safetyNumber: "48213 90557" }] })

    it("tells the admin who is waiting to join", async () => {
      serve(request())
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("Ben Tan is waiting to join Acme NDA.")
    })

    it("lets them in on the admin's Numbers match", async () => {
      serve(request(), { sync_approve: null })
      const wrapper = await mountMatter()
      await button(wrapper, "Numbers match: let Ben Tan in").trigger("click")
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith("sync_approve", { id: ID, device: "d2", number: "48213 90557" })
    })

    it("tells the admin when two people answered one invite", async () => {
      serve(detail({ contested: ["inv-1"] }))
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("Two people answered this invite.")
    })

    it("withdraws a contested invite", async () => {
      serve(detail({ contested: ["inv-1"] }), { sync_withdraw: null })
      const wrapper = await mountMatter()
      await button(wrapper, "Withdraw this invite").trigger("click")
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith("sync_withdraw", { id: ID, invite: "inv-1" })
    })
  })

  describe("fork banner", () => {
    const forked = () =>
      detail({
        files: [file({
          forkedFrom: "1",
          versions: [version("2-AL", "Ana Lim", true, true), version("2-BT", "Ben Tan", true), version("1", "Ana Lim", false, true)],
        })],
      })

    it("says who edited which version", async () => {
      serve(forked())
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("You and Ben Tan each edited v1.")
    })

    it("says plainly that combining in Word is done by hand", async () => {
      serve(forked())
      const wrapper = await mountMatter()
      expect(wrapper.text()).toContain("Bring the changes across yourself")
    })

    it("makes the next version from the colleague's side on Use theirs", async () => {
      serve(forked(), { sync_resolve: null })
      const wrapper = await mountMatter()
      await button(wrapper, "Use Ben Tan's").trigger("click")
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith("sync_resolve", { id: ID, fileId: "f1", version: "2-BT".padEnd(64, "0") })
    })

    it("says the versions were only shown in their folder when they couldn't be opened", async () => {
      serve(forked(), { sync_combine: false })
      const wrapper = await mountMatter()
      await button(wrapper, "Combine in Word").trigger("click")
      await flushPromises()
      expect(addToast).toHaveBeenCalledWith(
        "Both versions are shown in their folder. Bring the changes across, then save yours.",
        "info",
      )
    })

    it("replaces Open in Word with the banner's choices", async () => {
      serve(forked())
      const wrapper = await mountMatter()
      expect(wrapper.findAll("button").map((b) => b.text())).not.toContain("Open in Word")
    })
  })
})
