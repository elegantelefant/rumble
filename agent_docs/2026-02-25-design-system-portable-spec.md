# Elefant Design System — Portable Specification

> For engineers building the **Tauri desktop app** and **Microsoft Word Add-in**.
> Generated 2026-02-25 from the Nuxt 3 frontend codebase.

---

## 1. Design Philosophy

Elefant is a legal research platform. The aesthetic is **professional, clean, and information-dense** — not playful. Think Bloomberg Terminal meets modern SaaS. Key principles:

- **Information hierarchy** over decoration
- **High contrast** text for extended reading
- **Subtle elevation** (shadows, not borders) to separate layers
- **Teal/cyan accent palette** — distinctive but not loud
- **Dark mode** as first-class citizen

---

## 2. Color System

All colors are defined as CSS custom properties. Use these exact values.

### 2.1 Light Mode

| Token | Hex | Usage |
|-------|-----|-------|
| `--background` | `#F7FBFC` | Page background |
| `--foreground` | `#132C35` | Primary text |
| `--card` | `#FFFFFF` | Card/surface background |
| `--card-foreground` | `#132C35` | Text on cards |
| `--primary` | `#030213` | Primary buttons, focus rings |
| `--primary-foreground` | `#F7FBFC` | Text on primary |
| `--secondary` | `#2E7288` | Secondary buttons, links |
| `--secondary-foreground` | `#F7FBFC` | Text on secondary |
| `--muted` | `#D5E6EB` | Disabled backgrounds, subtle fills |
| `--muted-foreground` | `#1F3C46` | Secondary text |
| `--accent` | `#166D7E` | Highlights, active states |
| `--accent-foreground` | `#E9FBFF` | Text on accent |
| `--destructive` | `#984637` | Errors, delete actions |
| `--destructive-foreground` | `#FDF8F8` | Text on destructive |
| `--border` | `#B7D1D9` | Default borders |
| `--input` | `#C6DDE4` | Input borders |
| `--ring` | `#030213` | Focus ring color |

### 2.2 Dark Mode

| Token | Hex | Usage |
|-------|-----|-------|
| `--background` | `#08141A` | Page background |
| `--foreground` | `#E3F3F8` | Primary text |
| `--card` | `#0F1F27` | Card/surface background |
| `--primary` | `#030213` | Primary buttons |
| `--secondary` | `#5FBBD0` | Secondary accent |
| `--accent` | `#49C2E2` | Bright highlights |
| `--destructive` | `#F06A5C` | Errors |
| `--border` | `#123545` | Borders |
| `--input` | `#1B4151` | Input borders |
| `--muted` | `#132630` | Muted backgrounds |
| `--muted-foreground` | `#BBD8E2` | Secondary text |

### 2.3 Semantic Colors

| Purpose | Light | Dark |
|---------|-------|------|
| Success | `#16a34a` (green-600) | `#22c55e` (green-500) |
| Warning | `#F0995A` | `#F0995A` |
| Info | `#44a4fc` | `#44a4fc` |
| Chart 1 | `#030213` | `#030213` |
| Chart 2 | `#166D7E` | `#49C2E2` |
| Chart 3 | `#2E7288` | `#5FBBD0` |
| Chart 4 | `#5FBBD0` | `#8FD6E8` |
| Chart 5 | `#F0995A` | `#F06A5C` |

### 2.4 Domain-Specific Badge Colors

| Role | Border | Background | Text |
|------|--------|------------|------|
| Layperson | `blue-200` | `blue-100` | `blue-500` |
| Legal practitioner | `green-200` | `green-100` | `green-600` |
| Judge | `gray-200` | `gray-100` | `gray-700` |

---

## 3. Typography

### 3.1 Font Families

| Role | Family | Fallback Stack |
|------|--------|----------------|
| Body / UI | **Inter** | `Inter var`, `system-ui`, `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `sans-serif` |
| Display / Headings | **Outfit** | `sans-serif` |

### 3.2 Scale

| Name | Size | Usage |
|------|------|-------|
| `text-tiny` | 10px (0.625rem) | Labels, badges |
| `text-xs` | 12px (0.75rem) | Validation messages, captions |
| `text-sm` | 14px (0.875rem) | Secondary text, inputs (desktop) |
| `text-base` | 16px (1rem) | Body text, inputs (mobile) |
| `text-lg` | 18px (1.125rem) | Subheadings |
| `text-xl` | 20px (1.25rem) | Section titles |
| `text-2xl` | 24px (1.5rem) | Page headings |
| `text-3xl` | 30px (1.875rem) | Hero headings |

### 3.3 Weights

- **400** (Regular) — body text
- **500** (Medium) — labels, active nav items
- **600** (Semibold) — subheadings, buttons
- **700** (Bold) — headings, emphasis

---

## 4. Spacing & Layout

### 4.1 Border Radius

| Token | Value | Usage |
|-------|-------|-------|
| `--radius` (base) | **12px** | Default |
| `--radius-sm` | 8px | Small inputs, tags |
| `--radius-md` | 14px | Dialogs, popovers |
| `--radius-lg` | 18px | Cards |
| `--radius-xl` | 26px | Large cards |
| `--radius-2xl` | 34px | Hero elements |
| `--radius-3xl` | 50px | Pills |
| `--radius-full` | 9999px | Badges, avatars |

### 4.2 Elevation (Box Shadows)

| Level | Shadow | Usage |
|-------|--------|-------|
| 1 | `0 2px 4px rgba(0,0,0,0.08), 0 1px 2px -1px rgba(0,0,0,0.08)` | Cards at rest |
| 2 | `0 4px 12px rgba(0,0,0,0.12), 0 2px 8px -2px rgba(0,0,0,0.10)` | Cards on hover, dialogs |
| 3 | `0 12px 24px rgba(0,0,0,0.16), 0 8px 24px -8px rgba(0,0,0,0.12)` | Modals |
| 4 | `0 24px 48px rgba(0,0,0,0.20), 0 16px 40px -12px rgba(0,0,0,0.14)` | Prominent overlays |
| 5 | `0 24px 60px -16px rgba(0,0,0,0.18), 0 18px 40px -12px rgba(0,0,0,0.12)` | Hero cards |
| Dropdown | `0 24px 60px -25px rgba(12,11,23,0.45)` | Dropdown menus |

### 4.3 Container Widths

| Name | Width | Usage |
|------|-------|-------|
| Standard | `min(100% - 2.5rem, 72rem)` = max 1152px | Content pages |
| Wide | `min(100% - 2.5rem, 80rem)` = max 1280px | Dashboard |
| Sidebar | ~250px (collapsible to 64px) | Dashboard sidebar |
| Header | 64px height (48px collapsed) | Top bar |

### 4.4 Breakpoints

| Name | Width | Behavior |
|------|-------|----------|
| xs | 480px | Custom mobile |
| sm | 640px | Small mobile |
| md | 768px | Tablet |
| lg | 1024px | Desktop / sidebar visible |
| xl | 1280px | Wide desktop |
| 2xl | 1536px | Ultra-wide |

---

## 5. Component Specifications

### 5.1 Button

**Variant system** (CVA — class-variance-authority):

| Intent | Appearance |
|--------|------------|
| `default` | Dark bg (`--primary`), light text, subtle shadow |
| `secondary` | Teal bg (`--secondary`), light text |
| `destructive` | Red bg (`--destructive`), white text |
| `outline` | Transparent bg, border, hover fills accent |
| `ghost` | No bg/border, hover fills accent |
| `link` | Text-only, underline on hover |
| `success` | Green-600 bg, white text |

| Size | Height | Padding |
|------|--------|---------|
| `default` | 36px (h-9) | `px-4 py-2` |
| `sm` | 32px (h-8) | `px-3` |
| `lg` | 40px (h-10) | `px-6` |
| `icon` | 36x36px | centered |
| `icon-sm` | 28x28px | centered |

**States:**
- Hover: 90% opacity of bg color
- Active/pressed: `scale(0.98)` + inset shadow, 150ms
- Disabled: 50% opacity, `pointer-events: none`
- Focus: 3px ring at 50% opacity of `--ring`
- Loading: spinner icon replaces content, same dimensions

### 5.2 Input

| Property | Value |
|----------|-------|
| Height | 36px (h-9) |
| Border | 1px solid `--input` |
| Border radius | `--radius-sm` (8px) |
| Padding | `px-3 py-1` |
| Font size | 16px mobile, 14px desktop |
| Background | transparent (light), `--input` at 30% (dark) |
| Shadow | `shadow-xs` (very subtle) |
| Focus | 3px ring `--ring` at 50%, border → `--ring` |
| Invalid | border → `--destructive`, ring → `--destructive` at 20% |
| Disabled | 50% opacity, `not-allowed` cursor |
| Placeholder | `--muted-foreground` |
| Selection | `--primary` bg, `--primary-foreground` text |

### 5.3 Card

| Property | Value |
|----------|-------|
| Background | `--card` |
| Border | 1px solid `--border` |
| Border radius | 18px (`--radius-lg`) |
| Padding | 24px (1.5rem) |
| Shadow | elevation-1 at rest |
| Gap (between sections) | 24px |
| Hover (interactive) | `scale(1.02) translateY(-4px)`, elevation-2 |
| Active (interactive) | `scale(0.98)`, elevation-1 |
| Transition | 200ms `ease-smooth` |

### 5.4 Badge

- Shape: `rounded-full` (pill)
- Padding: `px-2 py-0.5`
- Font: `text-xs` (12px)
- Icon size: 12px (size-3)
- Border: 1px, transparent for filled intents
- Intents: default, secondary, destructive, outline + domain-specific (layperson, legal_practitioner, judge)

### 5.5 Dialog / Modal

| Property | Value |
|----------|-------|
| Overlay | `rgba(0,0,0,0.4)` light / `rgba(0,0,0,0.6)` dark |
| Content bg | `--card` |
| Content border | 1px solid `--border` |
| Content radius | 14px (`--radius-md`) |
| Content padding | 24px |
| Content shadow | elevation-2 |
| Entry animation | fade + scale from 0.95, 200ms |
| Exit animation | fade out, 200ms |
| Close button | top-right, icon-only (X) |

### 5.6 Form Pattern

```
┌─────────────────────────────┐
│ Label (14px, medium weight) │
│ ┌─────────────────────────┐ │
│ │ Input (36px tall)       │ │
│ └─────────────────────────┘ │
│ Validation message (12px,   │
│ destructive color, slides   │
│ in from above 250ms)        │
└─────────────────────────────┘
```

### 5.7 Sidebar (Dashboard)

- Width: 250px default, 64px collapsed (icon-only)
- Background: `--sidebar` (slightly tinted)
- Menu items: 32px height, `text-sm`, `rounded-md`
- Active item: `--sidebar-accent` bg, `font-medium`
- Hover: `--sidebar-accent` bg
- Mobile: slides in from left with overlay backdrop
- Trigger: hamburger icon at < 1024px

### 5.8 Toast / Notification

| Type | Background | Border |
|------|------------|--------|
| Success | `#b6f9ca` | `#42a85f` |
| Error | `#f6d9d7` | `#b82e24` |
| Warning | `#ffb648` | `#f48a06` |
| Info | `#44a4fc` | `#187fe7` |

- Border radius: 5px
- Padding: `20px 15px`
- Entry: slide in from right + scale
- Exit: slide out to right + scale down

### 5.9 Skeleton / Loading States

**Pulse:** opacity 1 → 0.5 → 1, 1.5s ease-in-out infinite, bg `--primary` at 10%
**Wave:** linear gradient sweep left-to-right, 1.5s infinite
**Spinner:** SVG circle, `rotate 360°` 1s linear infinite, `currentColor`

---

## 6. Animation & Motion

### 6.1 Timing Tokens

| Token | Duration | Usage |
|-------|----------|-------|
| `instant` | 100ms | Hover color changes |
| `quick` | 200ms | Page transitions, tooltips |
| `base` | 300ms | Most interactions |
| `slow` | 400ms | Complex transitions |
| `slower` | 600ms | Page-level animations |

### 6.2 Easing Curves

| Name | Curve | Usage |
|------|-------|-------|
| `smooth` | `cubic-bezier(0.25, 0.46, 0.45, 0.94)` | Default for most animations |
| `bounce` | `cubic-bezier(0.34, 1.56, 0.64, 1)` | Checkbox, form messages |
| `out-expo` | `cubic-bezier(0.16, 1, 0.3, 1)` | Dropdown open |
| `elastic` | `cubic-bezier(0.68, -0.55, 0.27, 1.55)` | Playful micro-interactions |

### 6.3 Key Animations

| Element | Animation | Duration | Easing |
|---------|-----------|----------|--------|
| Page transition | Opacity 0→1 | 200ms | smooth |
| Button press | scale(0.98) + inset shadow | 150ms | smooth |
| Card hover | scale(1.02) + translateY(-4px) | 200ms | smooth |
| Checkbox check | scale(0→1.15→1) + rotate | 250ms | bounce |
| Form message enter | translateY(-10px→0) + opacity | 250ms | bounce |
| Tooltip | scale(0.95→1) + opacity | 200ms | bounce |
| Toast slide in | translateX(100%→0) + scale | 300ms | smooth |
| Search result | translateY(20px→0) + opacity | 400ms | smooth (staggered) |

### 6.4 Reduced Motion

All animations respect `@media (prefers-reduced-motion: reduce)` — transitions become instant, transforms disabled.

---

## 7. Icon System

- **Library:** Iconify (with custom `elefant` collection)
- **Default prefix:** `ph:` (Phosphor icons)
- **Default size:** 16px (`size-4`) inline, 24px (`size-6`) standalone
- **Color:** always `currentColor` (inherits from parent text)
- **Behavior:** `pointer-events: none`, `shrink-0`
- **Custom brand icons:** SVG files in `assets/icons/` (Anthropic, OpenAI, Gemini, DeepSeek, Meta, Grok, Qwen)
- **Flags:** CDN via `flagcdn.com` (not bundled)

---

## 8. Layouts

### 8.1 User Layout (Marketing + Search)

```
┌────────────────────────────────────────┐
│ Header (sticky, 64px)                  │
│ [Logo] [Nav] [Lang] [Search] [Profile] │
├────────────────────────────────────────┤
│                                        │
│ Page Content (max 1152px centered)     │
│                                        │
├────────────────────────────────────────┤
│ Footer (multi-column links)            │
└────────────────────────────────────────┘
```

### 8.2 Dashboard Layout (Authenticated App)

```
┌──────┬─────────────────────────────────┐
│      │ Header (breadcrumbs, actions)   │
│ Side │─────────────────────────────────│
│ bar  │                                 │
│ 250px│ Page Content (max 1280px)       │
│      │                                 │
│      │                                 │
│      ├─────────────────────────────────│
│      │ Footer                          │
└──────┴─────────────────────────────────┘
```

### 8.3 Empty Layout (Auth Pages)

```
┌────────────────────────────────────────┐
│ (dark semi-transparent background)     │
│                                        │
│        ┌──────────────────┐            │
│        │ Auth Card        │            │
│        │ (centered)       │            │
│        └──────────────────┘            │
│                                        │
│ [Made by Elefant badge, bottom-left]   │
└────────────────────────────────────────┘
```

---

## 9. Platform-Specific Guidance

### 9.1 For Tauri Desktop App

**What ports cleanly (~70% of codebase):**
- All CSS tokens/variables → copy directly
- Vue 3 components → work in Tauri webview as-is
- Pinia stores → swap persistence from `localStorage` to `tauri-plugin-store`
- Tailwind CSS → framework-agnostic, works anywhere
- BetterAuth client → pure HTTP, works as-is
- Orval-generated API clients → portable HTTP calls
- Search adapter functions → pure TypeScript, no framework dependency
- All TypeScript types/interfaces → fully portable

**What needs reimplementation:**
- `localStorage`/`sessionStorage` → `tauri-plugin-store`
- `$fetch` / Nuxt server routes → direct HTTP to backend API
- File downloads (`<a download>`) → `fs.writeBinaryFile()`
- Audio recording (`MediaRecorder`) → `tauri-plugin-audio` or skip
- Nuxt middleware → custom Vue Router guards
- SSR-specific code (`import.meta.server`) → remove entirely
- File-based routing → explicit Vue Router config

**Tauri-specific opportunities:**
- System tray with notification badges
- Native file dialogs for export
- Keyboard shortcuts via Tauri accelerators
- Offline mode with local SQLite cache
- Auto-update via Tauri updater

### 9.2 For Microsoft Word Add-in

**Constraints:**
- Runs in sandboxed iframe (~300px sidebar or dialog modal)
- No `localStorage` → use `Office.context.document.settings`
- Limited DOM access
- Must use Office.js API for document interaction

**Viable feature set (MVP):**
1. **Search** — compact search bar + results list in sidebar
2. **Insert citation** — format + insert text at cursor via `Office.context.document.body.insertText()`
3. **Browse briefcase** — read-only item list
4. **View document notes** — read/write notes
5. **Insert reference** — structured legal reference insertion

**Not viable in Word sidebar:**
- Drag-and-drop reordering
- File export / ZIP generation
- Audio recording
- Complex multi-step wizards (legal doc generator)
- Graph visualization
- Full search filters panel

**Recommended UI approach:**
- Single-page task pane (no routing)
- Tab-based navigation: Search | Briefcase | Settings
- Dialog popups for detail views (Office.js `displayDialogAsync`)
- Compact card design (no hover effects — touch-first)
- Reduced animation set (instant transitions)

**Simplified token set for Word:**
- Core 6 colors only: primary, secondary, accent, destructive, background, foreground
- 2 shadow levels (elevation-1, elevation-2)
- Single border radius (8px)
- Inter font only (no Outfit)
- Min tap target: 40px

---

## 10. Shared Code Extraction Plan

### Recommended Package Structure

```
@elefant/types          → All TypeScript interfaces (already exists in shared/types/)
@elefant/auth           → BetterAuth client + composable
@elefant/api-clients    → Orval-generated clients + customFetch + unwrap()
@elefant/search-mappers → search-adapter.ts + all index adapters
@elefant/utils          → date formatters, logger, base64, auth-errors
@elefant/ui-tokens      → CSS custom properties file, exported as JSON for non-CSS consumers
```

### Storage Abstraction

```typescript
interface StorageAdapter {
  get(key: string): Promise<string | null>
  set(key: string, value: string): Promise<void>
  remove(key: string): Promise<void>
}

// Implementations:
// Web:   LocalStorageAdapter (localStorage)
// Tauri: TauriStoreAdapter (tauri-plugin-store)
// Word:  OfficeSettingsAdapter (Office.context.document.settings)
```

---

## 11. Data Attribute Convention

All components use `data-slot` attributes for semantic identification. Preserve this convention across platforms for consistent testing and styling:

```
data-slot="button"
data-slot="input"
data-slot="card"
data-slot="dialog"
data-slot="dialog-content"
data-slot="form-label"
data-slot="form-message"
data-slot="form-control"
data-slot="input-group"
data-slot="skeleton"
data-slot="accordion-trigger"
data-slot="accordion-content"
data-slot="tooltip-content"
data-slot="popover-content"
data-slot="dropdown-menu-content"
data-slot="select-content"
data-slot="table"
data-slot="progress"
```

---

## 12. Accessibility Requirements

- **Focus rings:** 3px ring, `--ring` color at 50% opacity, on `focus-visible` only
- **ARIA labels:** on all interactive elements
- **Keyboard nav:** full tab order, Enter/Space for activation
- **Color contrast:** WCAG AA minimum (4.5:1 body text, 3:1 large text)
- **Reduced motion:** all animations gated by `prefers-reduced-motion`
- **Touch targets:** minimum 44px (web/Tauri), 40px (Word)
- **Screen reader:** live regions for toasts, status updates

---

## 13. Internationalization

6 locales supported — all UI strings must be externalized:

| Code | Language | Direction |
|------|----------|-----------|
| `en-SG` | Singapore English (default) | LTR |
| `en-US` | US English | LTR |
| `ms-MY` | Bahasa Melayu | LTR |
| `ta-SG` | Tamil | LTR |
| `zh-SG` | Simplified Chinese | LTR |
| `zh-Hant` | Traditional Chinese | LTR |

Locale detection: cookie → browser preference → `en-SG` fallback.
