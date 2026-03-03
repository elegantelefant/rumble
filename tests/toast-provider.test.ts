// ABOUTME: Tests for ToastProvider.vue component.
// ABOUTME: Covers toast addition, auto-dismiss via fake timers, and toast rendering.

import { mount } from "@vue/test-utils"
import { defineComponent, inject } from "vue"
import ToastProvider from "../src/components/ToastProvider.vue"

beforeAll(() => {
  vi.useFakeTimers()
})

afterAll(() => {
  vi.useRealTimers()
})

beforeEach(() => {
  vi.clearAllMocks()
})

// Child component that exposes the injected toast API for testing
const ToastConsumer = defineComponent({
  setup() {
    const toast = inject<{ addToast: (msg: string, type?: string, duration?: number) => void }>("toast")
    return { toast }
  },
  template: `<button data-testid="add" @click="toast?.addToast('Hello', 'info')">Add</button>`,
})

function mountProvider() {
  return mount(ToastProvider, {
    slots: {
      default: ToastConsumer,
    },
  })
}

describe("ToastProvider", () => {
  it("renders slot content", () => {
    const wrapper = mountProvider()
    expect(wrapper.find("[data-testid='add']").exists()).toBe(true)
  })

  it("shows no toasts initially", () => {
    const wrapper = mountProvider()
    // Only the slot button and the toast container exist
    expect(wrapper.text()).not.toContain("Hello")
  })

  it("adds a toast when addToast is called", async () => {
    const wrapper = mountProvider()
    await wrapper.find("[data-testid='add']").trigger("click")
    expect(wrapper.text()).toContain("Hello")
  })

  it("adds multiple toasts", async () => {
    const wrapper = mountProvider()
    const button = wrapper.find("[data-testid='add']")
    await button.trigger("click")
    await button.trigger("click")
    await button.trigger("click")
    // Each click adds a toast with text "Hello"
    const toastElements = wrapper.findAll(".pointer-events-auto")
    expect(toastElements).toHaveLength(3)
  })

  it("auto-removes toast after duration", async () => {
    const wrapper = mountProvider()
    await wrapper.find("[data-testid='add']").trigger("click")
    expect(wrapper.text()).toContain("Hello")

    // Default duration is 3000ms
    vi.advanceTimersByTime(3000)
    await wrapper.vm.$nextTick()

    expect(wrapper.text()).not.toContain("Hello")
  })

  it("applies correct CSS class for success type", async () => {
    // Mount a custom consumer that adds a success toast
    const SuccessConsumer = defineComponent({
      setup() {
        const toast = inject<{ addToast: (msg: string, type?: string) => void }>("toast")
        return { toast }
      },
      template: `<button data-testid="add" @click="toast?.addToast('Saved', 'success')">Add</button>`,
    })

    const wrapper = mount(ToastProvider, {
      slots: { default: SuccessConsumer },
    })
    await wrapper.find("[data-testid='add']").trigger("click")

    const toastEl = wrapper.find(".pointer-events-auto")
    expect(toastEl.classes()).toContain("bg-[var(--success)]")
  })

  it("applies correct CSS class for error type", async () => {
    const ErrorConsumer = defineComponent({
      setup() {
        const toast = inject<{ addToast: (msg: string, type?: string) => void }>("toast")
        return { toast }
      },
      template: `<button data-testid="add" @click="toast?.addToast('Failed', 'error')">Add</button>`,
    })

    const wrapper = mount(ToastProvider, {
      slots: { default: ErrorConsumer },
    })
    await wrapper.find("[data-testid='add']").trigger("click")

    const toastEl = wrapper.find(".pointer-events-auto")
    expect(toastEl.classes()).toContain("bg-[var(--error)]")
  })
})
