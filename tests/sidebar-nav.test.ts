// ABOUTME: Tests for SidebarNav.vue component.
// ABOUTME: Covers tool reordering, boundary checks, active highlighting, disabled clicks.

import { mount } from "@vue/test-utils"
import SidebarNav from "../src/modules/navigation/SidebarNav.vue"

beforeEach(() => {
  vi.clearAllMocks()
})

function mountSidebar(activePath = "/review") {
  return mount(SidebarNav, {
    props: { open: true, activePath },
    global: {
      stubs: {
        // Stub lucide icons to avoid import resolution issues
        FileText: { template: "<span />" },
        Edit3: { template: "<span />" },
        Microscope: { template: "<span />" },
        BookOpen: { template: "<span />" },
        Globe: { template: "<span />" },
        Settings: { template: "<span />" },
        ArrowUp: { template: "<span />" },
        ArrowDown: { template: "<span />" },
      },
    },
  })
}

describe("SidebarNav", () => {
  it("renders all reorderable tools", () => {
    const wrapper = mountSidebar()
    expect(wrapper.text()).toContain("Document Review")
    expect(wrapper.text()).toContain("Research")
    expect(wrapper.text()).toContain("Document Draft")
    expect(wrapper.text()).toContain("Evidence Review")
    expect(wrapper.text()).toContain("Translation")
  })

  it("renders fixed tools (Settings)", () => {
    const wrapper = mountSidebar()
    expect(wrapper.text()).toContain("Settings")
  })

  it("emits navigate on tool click", async () => {
    const wrapper = mountSidebar()
    // Click the first non-disabled tool button
    const toolButtons = wrapper.findAll("nav button")
    await toolButtons[0].trigger("click")
    expect(wrapper.emitted("navigate")).toBeTruthy()
    expect(wrapper.emitted("navigate")![0]).toEqual(["/review"])
  })

  it("does not emit navigate for disabled tools", async () => {
    const wrapper = mountSidebar()
    // Evidence Review is disabled (index 3)
    const toolButtons = wrapper.findAll("nav button")
    // Find the Evidence Review button by checking aria-disabled
    const disabledButton = toolButtons.find(
      (btn) => btn.attributes("aria-disabled") === "true",
    )
    expect(disabledButton).toBeDefined()
    await disabledButton!.trigger("click")
    expect(wrapper.emitted("navigate")).toBeUndefined()
  })

  it("moves tool down when down arrow is clicked", async () => {
    const wrapper = mountSidebar()
    // Find move-down buttons (ArrowDown stubs inside nav)
    const moveDownButtons = wrapper.findAll('[aria-label*="down"]')
    // Click "Move Document Review down"
    await moveDownButtons[0].trigger("click")

    // After moving, Research should be first, Document Review second
    const navText = wrapper.find("nav").text()
    const researchIdx = navText.indexOf("Research")
    const reviewIdx = navText.indexOf("Document Review")
    expect(researchIdx).toBeLessThan(reviewIdx)
  })

  it("moves tool up when up arrow is clicked", async () => {
    const wrapper = mountSidebar()
    // Move "Research" (index 1) up — it should become first
    const moveUpButtons = wrapper.findAll('[aria-label*="up"]')
    // moveUpButtons[1] is the "Move Research up" button (index 0 is disabled for first item)
    await moveUpButtons[1].trigger("click")

    const navText = wrapper.find("nav").text()
    const researchIdx = navText.indexOf("Research")
    const reviewIdx = navText.indexOf("Document Review")
    expect(researchIdx).toBeLessThan(reviewIdx)
  })

  it("disables move-up for first tool", () => {
    const wrapper = mountSidebar()
    const moveUpButtons = wrapper.findAll('[aria-label*="up"]')
    expect(moveUpButtons[0].attributes("disabled")).toBeDefined()
  })

  it("disables move-down for last tool", () => {
    const wrapper = mountSidebar()
    const moveDownButtons = wrapper.findAll('[aria-label*="down"]')
    const lastDown = moveDownButtons[moveDownButtons.length - 1]
    expect(lastDown.attributes("disabled")).toBeDefined()
  })

  it("shows Coming Soon sublabel for disabled tools", () => {
    const wrapper = mountSidebar()
    expect(wrapper.text()).toContain("Coming Soon")
  })

  it("displays version info", () => {
    const wrapper = mountSidebar()
    expect(wrapper.text()).toContain("Rumble v0.1.0a")
  })
})
