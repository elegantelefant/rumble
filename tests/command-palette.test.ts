// ABOUTME: Tests for CommandPalette.vue component.
// ABOUTME: Covers filtering, keyboard navigation, command execution, and close behaviour.

import { mount } from "@vue/test-utils"
import CommandPalette from "../src/components/CommandPalette.vue"

const makeCommands = () => [
  { id: "review", label: "Document Review", action: vi.fn() },
  { id: "research", label: "Research", action: vi.fn() },
  { id: "draft", label: "Document Draft", action: vi.fn() },
  { id: "translate", label: "Translation", action: vi.fn() },
]

function mountPalette(open = true, commands = makeCommands()) {
  return mount(CommandPalette, {
    props: { open, commands },
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("CommandPalette", () => {
  it("renders all commands when open with no query", () => {
    const wrapper = mountPalette()
    const items = wrapper.findAll("li")
    // 4 commands, no "no matches" message
    expect(items).toHaveLength(4)
  })

  it("does not render when closed", () => {
    const wrapper = mountPalette(false)
    expect(wrapper.find("input").exists()).toBe(false)
  })

  it("filters commands by query (case-insensitive)", async () => {
    const wrapper = mountPalette()
    await wrapper.find("input").setValue("doc")
    const items = wrapper.findAll("li")
    expect(items).toHaveLength(2) // "Document Review" and "Document Draft"
  })

  it("shows no-matches message when filter yields nothing", async () => {
    const wrapper = mountPalette()
    await wrapper.find("input").setValue("zzzzzzz")
    expect(wrapper.text()).toContain("No matches")
  })

  it("highlights first item by default", () => {
    const wrapper = mountPalette()
    const items = wrapper.findAll("li")
    expect(items[0].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("moves highlight down on ArrowDown", async () => {
    const wrapper = mountPalette()
    await wrapper.find("input").trigger("keydown", { key: "ArrowDown" })
    // Can't easily trigger global keydown on the window from vue test utils,
    // so we test via the component's internal handler by dispatching on window
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }))
    await wrapper.vm.$nextTick()
    const items = wrapper.findAll("li")
    expect(items[1].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("wraps highlight from last to first on ArrowDown", async () => {
    const commands = makeCommands()
    const wrapper = mountPalette(true, commands)
    // Press ArrowDown 4 times to wrap around (4 items)
    for (let i = 0; i < 4; i++) {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }))
    }
    await wrapper.vm.$nextTick()
    const items = wrapper.findAll("li")
    expect(items[0].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("moves highlight up on ArrowUp (wrapping to last)", async () => {
    const wrapper = mountPalette()
    // First item is highlighted; ArrowUp should wrap to last
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }))
    await wrapper.vm.$nextTick()
    const items = wrapper.findAll("li")
    expect(items[3].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("executes highlighted command and emits close on Enter", async () => {
    const commands = makeCommands()
    const wrapper = mountPalette(true, commands)
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }))
    await wrapper.vm.$nextTick()
    expect(commands[0].action).toHaveBeenCalledOnce()
    expect(wrapper.emitted("close")).toBeTruthy()
  })

  it("emits close on Escape", async () => {
    const wrapper = mountPalette()
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }))
    await wrapper.vm.$nextTick()
    expect(wrapper.emitted("close")).toBeTruthy()
  })

  it("executes command and closes on click", async () => {
    const commands = makeCommands()
    const wrapper = mountPalette(true, commands)
    await wrapper.findAll("li")[2].trigger("click")
    expect(commands[2].action).toHaveBeenCalledOnce()
    expect(wrapper.emitted("close")).toBeTruthy()
  })

  it("highlights item on mouse enter", async () => {
    const wrapper = mountPalette()
    await wrapper.findAll("li")[2].trigger("mouseenter")
    const items = wrapper.findAll("li")
    expect(items[2].classes()).toContain("bg-[var(--primary-200)]")
  })

  it("arrow keys do not corrupt index when no matches", async () => {
    const wrapper = mountPalette()
    await wrapper.find("input").setValue("zzzzzzz")
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }))
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp" }))
    await wrapper.vm.$nextTick()
    expect(wrapper.text()).toContain("No matches")
  })

  it("keeps focus on the input when Tab is pressed", async () => {
    const wrapper = mount(CommandPalette, {
      props: { open: true, commands: makeCommands() },
      attachTo: document.body,
    })
    const input = wrapper.find("input").element as HTMLInputElement
    input.focus()
    expect(document.activeElement).toBe(input)

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab" }))
    await wrapper.vm.$nextTick()

    expect(document.activeElement).toBe(input)
    wrapper.unmount()
  })

  it("displays shortcut text when provided", () => {
    const commands = [
      { id: "test", label: "Test", shortcut: "Ctrl+T", action: vi.fn() },
    ]
    const wrapper = mountPalette(true, commands)
    expect(wrapper.text()).toContain("Ctrl+T")
  })
})
