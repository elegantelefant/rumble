// ABOUTME: Tests for MattersView.vue: the list of matters, starting one and joining with an invite link.
// ABOUTME: invoke is the boundary; a dismissed folder picker is not a success.

import { mount, flushPromises } from "@vue/test-utils"
import { invoke } from "@tauri-apps/api/core"
import MattersView from "../src/views/MattersView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }))

const addToast = vi.fn()
const push = vi.fn()
vi.mock("vue-router", () => ({ useRouter: () => ({ push }) }))

const acme = { id: "aa", name: "Acme NDA", folder: "/x/Acme NDA", standing: "admin", waitingFor: null, unavailable: null }
const beta = { id: "bb", name: "Beta lease", folder: "/x/Beta lease", standing: "waiting", waitingFor: "Ana Lim", unavailable: null }
const gone = { id: "cc", name: "Gone", folder: "/x/Gone", standing: "unavailable", waitingFor: null, unavailable: "folder missing" }

function serve(replies: Record<string, unknown>) {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => replies[cmd])
}

async function mountMatters() {
  const wrapper = mount(MattersView, { global: { provide: { [TOAST_KEY as symbol]: { addToast } } } })
  await flushPromises()
  return wrapper
}

function button(wrapper: Awaited<ReturnType<typeof mountMatters>>, text: string) {
  const found = wrapper.findAll("button").find((b) => b.text() === text)
  if (!found) throw new Error(`no button "${text}"`)
  return found
}

beforeEach(() => vi.clearAllMocks())

describe("MattersView", () => {
  it("lists each matter by name", async () => {
    serve({ sync_matters: [acme, beta] })
    const wrapper = await mountMatters()
    expect(wrapper.text()).toContain("Acme NDA")
  })

  it("marks a matter you are waiting to join", async () => {
    serve({ sync_matters: [beta] })
    const wrapper = await mountMatters()
    expect(wrapper.text()).toContain("Waiting for Ana Lim")
  })

  it("keeps a matter that can't be read in the list, saying why", async () => {
    serve({ sync_matters: [gone] })
    const wrapper = await mountMatters()
    expect(wrapper.text()).toContain("Can't be read right now: folder missing")
  })

  it("says what a matter is when there are none", async () => {
    serve({ sync_matters: [] })
    const wrapper = await mountMatters()
    expect(wrapper.text()).toContain("No matters yet.")
  })

  it("opens a new matter once its folder is picked", async () => {
    serve({ sync_matters: [], sync_create_matter: acme })
    const wrapper = await mountMatters()
    await wrapper.find("input[name=your-name]").setValue("Ana Lim")
    await button(wrapper, "New matter").trigger("click")
    await flushPromises()
    expect(push).toHaveBeenCalledWith("/matters/aa")
  })

  it("stays put when the folder picker is dismissed", async () => {
    serve({ sync_matters: [], sync_create_matter: null })
    const wrapper = await mountMatters()
    await wrapper.find("input[name=your-name]").setValue("Ana Lim")
    await button(wrapper, "New matter").trigger("click")
    await flushPromises()
    expect(push).not.toHaveBeenCalled()
  })

  it("asks for your name before starting a matter", async () => {
    serve({ sync_matters: [] })
    const wrapper = await mountMatters()
    expect(button(wrapper, "New matter").attributes("disabled")).toBeDefined()
  })

  it("joins with a pasted invite link", async () => {
    serve({ sync_matters: [], sync_join_matter: beta })
    const wrapper = await mountMatters()
    await wrapper.find("input[name=your-name]").setValue("Ben Tan")
    await wrapper.find("textarea[name=join-link]").setValue("c0ffee")
    await button(wrapper, "Join").trigger("click")
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith("sync_join_matter", { link: "c0ffee", yourName: "Ben Tan" })
  })
})
