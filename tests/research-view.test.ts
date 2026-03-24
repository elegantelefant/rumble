// ABOUTME: Tests for ResearchView.vue component.
// ABOUTME: Covers thread management, prompt submission, status changes, model selection.

import { mount } from "@vue/test-utils"
import ResearchView from "../src/views/ResearchView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("../src/modules/backend/backendClient", () => ({
  backendRegistry: { value: [] },
  mockResearchRun: vi.fn().mockResolvedValue({
    threadId: "t-1",
    answer: "Research findings about negligence elements.",
    citations: ["Case v. Case (2021)"],
  }),
}))

const mockAddToast = vi.fn()

function mountResearch() {
  return mount(ResearchView, {
    global: {
      provide: {
        [TOAST_KEY as symbol]: { addToast: mockAddToast },
      },
    },
  })
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe("ResearchView", () => {
  it("renders page title", () => {
    const wrapper = mountResearch()
    expect(wrapper.find("h1").text()).toBe("Research Assistant")
  })

  it("renders initial threads in sidebar", () => {
    const wrapper = mountResearch()
    expect(wrapper.text()).toContain("Tax compliance for SaaS contracts")
    expect(wrapper.text()).toContain("GDPR data retention checklist")
  })

  it("renders workflow notes", () => {
    const wrapper = mountResearch()
    expect(wrapper.text()).toContain("Draft the request")
    expect(wrapper.text()).toContain("Review generated memo")
    expect(wrapper.text()).toContain("Return via history")
  })

  it("creates new thread on button click", async () => {
    const wrapper = mountResearch()
    const newThreadBtn = wrapper.findAll("button").find(
      (b) => b.text() === "New Thread",
    )
    await newThreadBtn!.trigger("click")
    expect(wrapper.text()).toContain("Untitled research thread")
  })

  it("switches active thread on click", async () => {
    const wrapper = mountResearch()
    const threadButtons = wrapper.findAll("aside button")
    // Click the second thread
    await threadButtons[1].trigger("click")
    // Verify active styling — the second thread should have the active border
    expect(threadButtons[1].classes()).toContain("border-[var(--accent-500)]")
  })

  it("renders model selector with available models", () => {
    const wrapper = mountResearch()
    const select = wrapper.find("select")
    expect(select.exists()).toBe(true)
    expect(wrapper.text()).toContain("Elefant Legal Blend")
  })

  it("does not submit empty prompt", async () => {
    const wrapper = mountResearch()
    const submitBtn = wrapper.findAll("button").find(
      (b) => b.text() === "Start Research",
    )
    await submitBtn!.trigger("click")
    // No toast or message change expected
    expect(mockAddToast).not.toHaveBeenCalled()
  })

  it("shows thread status badges", () => {
    const wrapper = mountResearch()
    expect(wrapper.text()).toContain("Complete")
    expect(wrapper.text()).toContain("In Progress")
  })

  it("renders API surface toggle button", () => {
    const wrapper = mountResearch()
    expect(wrapper.text()).toContain("API surface")
  })

  it("renders research question textarea", () => {
    const wrapper = mountResearch()
    const textarea = wrapper.find("textarea")
    expect(textarea.exists()).toBe(true)
  })
})
