---
name: frontend-product-design
description: "Trigger: UI, UX, frontend design, layout, maquetado, redesign. Design product-specific interfaces before coding visuals."
license: Apache-2.0
metadata:
  author: gentleman-programming
  version: "1.0"
---

# Frontend Product Design

## Activation Contract

Use this skill before creating or changing visible UI, layout, component styling, app shells, dashboards, editors, launchers, empty states, or interaction flows.

## Hard Rules

- Do not ship generic AI dashboard/card layouts. Design from the product's purpose, primary task, and operating context.
- Start with the user's job-to-be-done, then choose layout, density, hierarchy, and controls.
- For this app, prioritize a Linux desktop notes launcher: fast capture, keyboard-first search, calm writing surface, Markdown readability, and protected-note clarity.
- Treat visual design as product behavior: every color, spacing choice, and panel must support capture, search, reading, editing, or privacy.
- Prefer concrete interaction states over decoration: empty, loading, selected, dirty, saved, archived, protected, locked, error.
- Maintain accessibility: readable contrast, visible focus, semantic controls, keyboard navigation, and no color-only status.

## Decision Gates

| Situation | Action |
| --- | --- |
| New screen | Define primary user action, secondary actions, and what must be visually quiet. |
| Redesign request | Remove generic visuals first; preserve working behavior unless explicitly changing UX. |
| Desktop launcher | Optimize for quick open, search, create, edit, and hide; avoid marketing-site hero layouts. |
| Notes/editor UI | Give writing area priority; keep metadata compact and non-intrusive. |
| Protected content | Make lock state obvious without exposing content or using scary noise. |

## Execution Steps

1. State the UI intent in one sentence before editing.
2. Sketch the information architecture: shell, navigation/list, work area, preview, command area.
3. Define a small design system: colors, spacing scale, typography, radius, elevation, focus states.
4. Implement layout with real states and responsive desktop sizing.
5. Verify the UI against the product goal, not against trendiness.

## Output Contract

Return:
- UI intent.
- Key layout decisions.
- Files changed.
- Accessibility/keyboard notes.
- What was deliberately left out.

## References

- `docs/product-requirements.md` — product goals and MVP scope.
- `docs/technical-design.md` — architecture and UI boundary.
