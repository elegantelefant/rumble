// ABOUTME: Tests for TranslationView.vue component.
// ABOUTME: Covers language selection, translation execution, job history, loading states.

import { mount } from "@vue/test-utils"
import TranslationView from "../src/views/TranslationView.vue"
import { TOAST_KEY } from "../src/composables/toast"

vi.mock("../src/modules/backend/backendClient", () => ({
  backendRegistry: { value: [] },
  mockTranslationRun: vi.fn().mockResolvedValue({
    jobId: "job-123",
    translatedText: "[Mock translation fr→en] Test text",
  }),
}))

const mockAddToast = vi.fn()

function mountTranslation() {
  return mount(TranslationView, {
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

describe("TranslationView", () => {
  it("renders page title", () => {
    const wrapper = mountTranslation()
    expect(wrapper.find("h1").text()).toBe("Translation")
  })

  it("renders source and target language selectors", () => {
    const wrapper = mountTranslation()
    const selects = wrapper.findAll("select")
    // source, target, model = 3 selects
    expect(selects.length).toBeGreaterThanOrEqual(2)
  })

  it("renders model selector", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("Elefant Legal Blend")
  })

  it("renders translate button", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("Translate")
  })

  it("renders job history in sidebar", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("FR → EN")
  })

  it("populates editor from initial job", () => {
    const wrapper = mountTranslation()
    // The initial job's input text should appear in the source textarea
    const textareas = wrapper.findAll("textarea")
    const sourceTextarea = textareas[0]
    expect(sourceTextarea.element.value).toContain("Veuillez confirmer")
  })

  it("does not translate with empty source text", async () => {
    const wrapper = mountTranslation()
    // Clear the source text
    const textareas = wrapper.findAll("textarea")
    await textareas[0].setValue("")

    const translateBtn = wrapper.findAll("button").find(
      (b) => b.text() === "Translate",
    )
    await translateBtn!.trigger("click")
    expect(mockAddToast).not.toHaveBeenCalled()
  })

  it("renders draft quality badge", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("Draft Quality")
  })

  it("renders workflow tips", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("Translation history")
  })

  it("renders API surface toggle", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("API surface")
  })

  it("renders translation notes", () => {
    const wrapper = mountTranslation()
    expect(wrapper.text()).toContain("terminology varies across jurisdictions")
  })
})
