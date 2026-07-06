# Task 6 Report: AppShell Component + Story

## Status

Completed. The AppShell component, its companion CSS, and Storybook story have been created and committed.

## Commits

```
9635b79 feat: add AppShell with dark sidebar and inset content area
```

## Files Created

- `frontend/src/components/AppShell.tsx` — AppShell component with sidebar (nav links to /nodes, /schemas, /plugins, installed apps list), collapse toggle, mobile overlay, and `<Outlet />` for main content
- `frontend/src/components/AppShell.css` — Companion CSS with layout, sidebar, nav, apps list, footer, inset main content, hamburger, and responsive breakpoint styles
- `frontend/src/stories/AppShell.stories.tsx` — Storybook stories (`Default` and `Collapsed` variants) wrapped in `QueryClientProvider` + `ThemeProvider`

## Test Summary

- TypeScript: No errors (`npx tsc --noEmit`)
- Biome: All lint/format checks pass
- Storybook: Both stories (`core-appshell--default`, `core-appshell--collapsed`) registered and render without errors at http://localhost:6006
- Commit hook: All CI checks pass (`cargo fmt --all --check`, `bun x biome format`)

## Fix Round 1

**Status:** Completed

**Fixes applied to `frontend/src/components/AppShell.css`:**

1. **Mobile breakpoint specificity fix**: Added `!important` to `margin`, `border-radius`, `height`, and `box-shadow` in the `@media (max-width: 768px)` `.app-shell-main` block so it overrides the higher-specificity `.app-shell-main:not(.sidebar-collapsed)` selector on mobile.

2. **Mobile sidebar transform transition**: Added `transform 0.25s ease` to the `.app-shell-sidebar` transition so the slide-in/out animation on mobile (`translateX(-100%)` / `translateX(0)`) is animated.

3. **Hardcoded pixel value replacements**:
   - `.app-shell-subtitle` `margin: 4px 0 0 0` to `margin: var(--space-1) 0 0 0`
   - `.app-shell-nav` `gap: 2px` to `gap: var(--space-1)`
   - Left `gap: 1px` on `.app-shell-app-info` as-is (sub-token value)

**Test results:**
- TypeScript: No errors (`npx tsc --noEmit`)

## Concerns

- The AppShell story uses inline static markup rather than the actual `<AppShell>` component because `<Outlet />` requires a TanStack Router context (which Storybook doesn't provide). The visual structure is identical, but storybook testing of the real component requires a full app integration test (Task 7).
- A11y: The mobile overlay is a `<button>` (for semantic correctness per Biome's `useSemanticElements` rule). Default button styles (border, min-height) are reset in CSS to make the overlay behave as a full-screen clickaway backdrop.
