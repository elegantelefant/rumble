// ABOUTME: Tests for ToastProvider.vue component.
// ABOUTME: Covers toast addition, auto-dismiss via fake timers, and toast rendering.

import { enableAutoUnmount, mount, flushPromises } from "@vue/test-utils"
import { defineComponent } from "vue"
import ToastProvider from "../src/components/ToastProvider.vue"
import { useToast } from "../src/composables/toast"
import { navigationError } from "../src/router"

// Each test's wrapper watches the shared navigationError ref, so a wrapper left
// mounted from a prior test would race the current test's own reset of it to null.
enableAutoUnmount(afterEach)

beforeAll(() => {
  vi.useFakeTimers()
})

afterAll(() => {
  vi.useRealTimers()
})

beforeEach(() => {
  vi.clearAllMocks()
  navigationError.value = null
})

// Child component that exposes the injected toast API for testing
const ToastConsumer = defineComponent({
  setup() {
    const toast = useToast()
    return { toast }
  },
  template: `<button data-testid="add" @click="toast.addToast('Hello', 'info')">Add</button>`,
})

function mountProvider() {
  return mount(ToastProvider, {
    slots: {
      default: ToastConsumer,
    },
  })
}

describe("useToast", () => {
  it("throws when called outside provider", () => {
    const Orphan = defineComponent({
      setup() { useToast(); return {}; },
      template: "<div />",
    });
    expect(() => mount(Orphan)).toThrow("useToast() called outside ToastProvider");
  });
});

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
        const toast = useToast()
        return { toast }
      },
      template: `<button data-testid="add" @click="toast.addToast('Saved', 'success')">Add</button>`,
    })

    const wrapper = mount(ToastProvider, {
      slots: { default: SuccessConsumer },
    })
    await wrapper.find("[data-testid='add']").trigger("click")

    const toastEl = wrapper.find(".pointer-events-auto")
    expect(toastEl.classes()).toContain("bg-[var(--success)]")
  })

  it("has an aria-live region for screen reader announcements", () => {
    const wrapper = mountProvider()
    const region = wrapper.find('[role="status"]')
    expect(region.exists()).toBe(true)
    expect(region.attributes("aria-live")).toBe("polite")
  })

  it("surfaces navigationError as an error toast and resets it to null", async () => {
    const wrapper = mountProvider()
    navigationError.value = "Something went wrong reaching the local backend."
    await flushPromises()

    expect(wrapper.text()).toContain("Something went wrong reaching the local backend.")
    const toastEl = wrapper.find(".pointer-events-auto")
    expect(toastEl.classes()).toContain("bg-[var(--error)]")
    expect(navigationError.value).toBeNull()
  })

  it("applies correct CSS class for error type", async () => {
    const ErrorConsumer = defineComponent({
      setup() {
        const toast = useToast()
        return { toast }
      },
      template: `<button data-testid="add" @click="toast.addToast('Failed', 'error')">Add</button>`,
    })

    const wrapper = mount(ToastProvider, {
      slots: { default: ErrorConsumer },
    })
    await wrapper.find("[data-testid='add']").trigger("click")

    const toastEl = wrapper.find(".pointer-events-auto")
    expect(toastEl.classes()).toContain("bg-[var(--error)]")
  })
})
