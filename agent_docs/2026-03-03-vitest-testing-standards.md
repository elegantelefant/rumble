# Vitest Testing Standards — Ivory Frontend

Rules for writing tests in this codebase. Not guidelines — requirements.

Sources: [Vue.js official testing guide](https://vuejs.org/guide/scaling-up/testing), [LogRocket advanced Vitest guide](https://blog.logrocket.com/advanced-guide-vitest-testing-mocking/).

---

## 0. Testing Strategy

Three tiers. Each has a job; none replaces the others.

| Tier | Scope | Speed | What it catches |
|------|-------|-------|-----------------|
| **Unit** | Functions, composables, utilities | Fast | Logical correctness |
| **Component** | Mounting, rendering, user interaction | Medium | Visual + behavioural regressions |
| **E2E** | Multi-page flows against real backend | Slow | Integration failures across layers |

**Stack:** Vitest + happy-dom (unit/component), @vue/test-utils (mounting), Playwright (E2E when needed).

Start testing early. The longer you wait, the more dependencies accumulate, the harder it gets.

---

## 1. Test What Matters, Mock What Doesn't

**Mock external dependencies, never the test candidate.**

```ts
// WRONG — mocking the function you're testing proves nothing
vi.mock("./quote.service")
vi.mocked(fetchQuote).mockResolvedValue(dummyQuote)
expect(await fetchQuote()).toEqual(dummyQuote) // tautology

// RIGHT — mock the dependency, test the function
vi.spyOn(globalThis, "fetch").mockResolvedValue(mockResponse)
expect(await fetchQuote()).toEqual(dummyQuote) // actually tests fetchQuote
```

Boundaries to mock:
- Network calls (`fetch`, Tauri `invoke()`)
- Timers (`setTimeout`, `setInterval`)
- Browser/OS APIs (`URL.createObjectURL`, clipboard, keychain)
- Pinia stores when testing components that consume them

Never mock: pure functions, your own utilities, the code under test.

---

## 2. AAA Structure — No Exceptions

Every test follows Arrange → Act → Assert. No interleaving.

```ts
it("formats bytes to human-readable size", () => {
  // arrange
  const bytes = 1536

  // act
  const result = formatSize(bytes)

  // assert
  expect(result).toBe("1.50 KB")
})
```

For trivial tests the sections can be single lines, but the order is sacred.

---

## 3. Tests Must Be Deterministic

No test may depend on real network, real time, or real randomness. Flaky = broken.

```ts
// WRONG — different result every run
const response = await fetchQuote()
expect(response).toMatchSnapshot()

// RIGHT — controlled input, predictable output
vi.spyOn(globalThis, "fetch").mockResolvedValue({
  ok: true,
  json: async () => ({ quote: "Hello, World!" }),
} as Response)
const response = await fetchQuote()
expect(response.quote).toBe("Hello, World!")
```

Use `vi.useFakeTimers()` for anything time-dependent. Use `vi.advanceTimersByTimeAsync(ms)` to control progression.

---

## 4. Choose the Right Test Double

| Double | Tool | When |
|--------|------|------|
| **Spy** | `vi.spyOn()` | Verify a call happened + args, keep real implementation |
| **Spy + stub** | `vi.spyOn().mockResolvedValue()` | Verify call AND control return value |
| **Mock** | `vi.fn()` | Replace a function entirely (e.g., callback props) |
| **Module mock** | `vi.mock()` | Replace an entire module's exports (hoisted) |
| **Scoped mock** | `vi.doMock()` | Non-hoisted mock that can access outer scope vars |
| **Global stub** | `vi.stubGlobal()` | Replace browser globals (`URL`, `navigator`) |

**Default to spies.** Only escalate to full mocks when you need to control return values from code you don't own.

---

## 5. Mock Cleanup Is Mandatory

```ts
beforeEach(() => {
  vi.clearAllMocks()
})
```

Every test file. No exceptions. Without this, mock call counts leak between tests and produce false passes/failures.

For fake timers:

```ts
beforeAll(() => { vi.useFakeTimers() })
afterAll(() => { vi.useRealTimers() })
```

---

## 6. Module Mocking Patterns

### Named export
```ts
vi.mock("./myModule")
vi.mocked(myFunction).mockReturnValue("controlled")
```

### Default export
```ts
vi.mock("./myModule", () => ({
  default: vi.fn(() => "controlled"),
}))
```

### Partial mock (keep some originals)
```ts
vi.mock("./myModule", async (importOriginal) => {
  const original = await importOriginal() as typeof import("./myModule")
  return {
    ...original,
    expensiveFunction: vi.fn().mockReturnValue("cheap"),
  }
})
```

### Property/getter spy
```ts
import * as exports from "./myModule"
vi.spyOn(exports, "magicNumber", "get").mockReturnValue(42)
```

---

## 7. Async Testing Patterns

### Resolved promises
```ts
vi.mocked(fetchData).mockResolvedValue({ id: 1, name: "test" })
```

### Rejected promises
```ts
vi.mocked(fetchData).mockRejectedValue(new Error("Network error"))
const result = await useFetch("/api/data")
expect(result.hasError).toBe(true)
expect(result.error!.message).toBe("Network error")
```

### Sequential resolutions (call 1 returns X, call 2 returns Y)
```ts
vi.mocked(fetchData)
  .mockResolvedValueOnce(firstResponse)
  .mockResolvedValueOnce(secondResponse)
```

### Vue reactivity + async
```ts
import { flushPromises } from "@vue/test-utils"
import { nextTick } from "vue"

// flushPromises — wait for ALL pending promises (onMounted, API calls)
await flushPromises()

// nextTick — wait for Vue's DOM update after reactive state change
await nextTick()
```

---

## 8. Component Testing Rules

### Blackbox by default

Test what a component **does**, not how it does it. This is the Vue team's official recommendation.

**DO test:**
- Rendered output based on props and slots (visual logic)
- Rendered updates and emitted events in response to user input (behavioural logic)
- Element presence via `data-testid` attributes or roles

**DO NOT test:**
- Private component state (`wrapper.vm.someRef`)
- Private methods
- Implementation details — these break on refactoring and prove nothing about correctness

```ts
// WRONG — testing implementation details
expect(wrapper.vm.isOpen).toBe(true)
expect(wrapper.vm.handleClick).toHaveBeenCalled()

// RIGHT — testing what the user sees and does
await wrapper.find('[data-testid="toggle"]').trigger("click")
expect(wrapper.find('[data-testid="panel"]').isVisible()).toBe(true)
```

### Use `shallowMount` for isolation, `mount` for integration

```ts
import { shallowMount } from "@vue/test-utils"

// Isolation — child components render as stubs
const wrapper = shallowMount(MyComponent, {
  props: { name: "test" },
})
```

Use `mount` (full render) only when you specifically need to test that parent + child work together. Most component tests should use `shallowMount`.

### Mock Pinia stores for component tests

```ts
import { createPinia, defineStore } from "pinia"

const useMockStore = defineStore("myStore", () => ({
  items: ref([]),
  fetchItems: vi.fn(),
}))

const pinia = createPinia()
useMockStore(pinia)
const wrapper = shallowMount(MyComponent, { global: { plugins: [pinia] } })
```

### Test lifecycle hooks with flushPromises

```ts
it("loads data on mount", async () => {
  const wrapper = shallowMount(MyComponent)
  await flushPromises() // wait for onMounted async work
  expect(wrapper.find(".data").text()).toContain("loaded")
})
```

---

## 9. Composable Testing

Two categories with different approaches.

### A. Pure composables (no lifecycle hooks, no provide/inject)

Test directly — they're just functions returning reactive state.

```ts
import { useCounter } from "./useCounter"

it("increments counter", () => {
  const { count, increment } = useCounter()
  expect(count.value).toBe(0)

  increment()
  expect(count.value).toBe(1)
})
```

### B. Composables with lifecycle hooks or provide/inject

Wrap in a host component. Use this helper:

```ts
// test-utils.ts
import { createApp } from "vue"

export function withSetup<T>(composable: () => T): [T, ReturnType<typeof createApp>] {
  let result!: T
  const app = createApp({
    setup() {
      result = composable()
      return () => {}
    },
  })
  app.mount(document.createElement("div"))
  return [result, app]
}
```

Usage:

```ts
import { withSetup } from "./test-utils"
import { useFoo } from "./useFoo"

it("initialises on mount", () => {
  const [result, app] = withSetup(() => useFoo(123))
  expect(result.foo.value).toBe(1)
  app.unmount() // triggers onUnmounted if needed
})
```

For complex composables that need provide/inject, pass it via `app.provide(...)` before asserting.

---

## 10. Timer Testing

For polling, debounce, setTimeout — never use real time.

```ts
beforeAll(() => { vi.useFakeTimers() })
afterAll(() => { vi.useRealTimers() })

it("polls every 5 seconds", async () => {
  vi.mocked(fetchData).mockResolvedValue(firstResult)
  const { data } = usePolling(5000)

  await flushPromises()
  expect(data.value).toEqual(firstResult)

  vi.mocked(fetchData).mockResolvedValue(secondResult)
  await vi.advanceTimersByTimeAsync(5000)
  expect(data.value).toEqual(secondResult)
})
```

---

## 11. Verification Methods

### Behaviour verification (did it call the right thing?)
```ts
expect(spy).toHaveBeenCalledTimes(1)
expect(spy).toHaveBeenCalledWith("/api/users", { method: "POST" })
```

### Argument inspection for sequential calls
```ts
expect(spy.mock.calls[0][0]).toBe("/api/step1")
expect(spy.mock.calls[1][0]).toBe("/api/step2")
```

### Check if something is mocked
```ts
expect(vi.isMockFunction(myFn)).toBe(true)
```

---

## 12. Snapshot Testing — Use Sparingly

Snapshots catch unintended changes. They do not describe correctness. Never use them as your only assertion.

```ts
// OK — supplementary check for HTML regressions
expect(wrapper.html()).toMatchSnapshot()

// NOT OK — snapshot as the only test (lazy, hides intent)
expect(response).toMatchSnapshot()
```

Update snapshots with `U` key in watch mode. Review every diff before accepting.

---

## 13. Tauri-Specific: Mocking `invoke()`

Every API call in this codebase flows through Tauri's `invoke()`. Mock it at the boundary:

```ts
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}))

import { invoke } from "@tauri-apps/api/core"

vi.mocked(invoke).mockResolvedValue({ status: "ok" })
```

This is the single most important mock in the Ivory test suite. Define it once in a shared test utility, reuse everywhere.

---

## Banned Practices

| Don't | Why |
|-------|-----|
| Mock the function you're testing | Tautological — proves nothing |
| Test private component state (`wrapper.vm.*`) | Implementation detail — breaks on refactoring |
| Test private methods | Same — test the public interface |
| Use real network calls in tests | Non-deterministic, slow, fragile |
| Skip `clearAllMocks` in `beforeEach` | Leaks state between tests |
| Use `mount` when `shallowMount` suffices | Tests child internals you don't own |
| Sleep instead of `advanceTimersByTimeAsync` | Slow, flaky, non-deterministic |
| Rely on snapshots alone | Doesn't describe correctness, breaks on any change |
| Use `--no-verify` to skip failing tests | Fix the test |
| Write tests that test mocked behaviour | Tests must exercise real code paths |

---

## API Quick Reference

| Function | Purpose |
|----------|---------|
| `vi.fn()` | Create mock function |
| `vi.mock(path)` | Hoisted module mock |
| `vi.doMock(path)` | Non-hoisted module mock (can access outer scope) |
| `vi.spyOn(obj, method)` | Spy on method, keep original |
| `vi.mocked(fn)` | Type-safe access to mock |
| `vi.stubGlobal(name, impl)` | Replace global (`URL`, `navigator`) |
| `vi.useFakeTimers()` | Activate fake timers |
| `vi.useRealTimers()` | Restore real timers |
| `vi.advanceTimersByTimeAsync(ms)` | Advance fake time |
| `vi.clearAllMocks()` | Reset all mock call counts and state |
| `vi.isMockFunction(fn)` | Check if fn is mocked |
| `.mockResolvedValue(val)` | Mock async return |
| `.mockResolvedValueOnce(val)` | Mock single async return |
| `.mockRejectedValue(err)` | Mock async throw |
| `.mockReturnValue(val)` | Mock sync return |
| `.mockReturnValueOnce(val)` | Mock single sync return |
| `flushPromises()` | Await all pending promises (Vue Test Utils) |
| `nextTick()` | Await Vue DOM update cycle |
