# ADR 0011: Web UI stack and design system

Status: accepted

## Context

The web UI is the one non-Rust part. It must look like the chosen
reference dashboard, run under a strict Content Security Policy, load
nothing from other origins, and be buildable by small models that know
mainstream tools best.

## Decision

- React, TypeScript in strict mode, Vite, Tailwind CSS, React Router,
  TanStack Query, `lucide-react`. Tests with Vitest and Playwright.
- The built files are embedded in the `riehub` binary. There is no
  separate web server and no server-side rendering.
- Design tokens are CSS variables defined once (spec, web, section 2).
  Components use tokens, never raw colours.
- Charts are small SVG components written in the repository.
- The font is self-hosted. No CDN, no analytics.
- API types are generated from the committed OpenAPI document.
- Any further runtime dependency needs a new ADR.

## Alternatives considered

- **A Rust web framework compiled to WebAssembly:** keeps one
  language, but the ecosystem for accessible components is thin and
  small models write it poorly.
- **A component library:** faster start, large dependency tree, and a
  look that fights the reference design.
- **A chart library:** tens of transitive packages for three chart
  types.

## Consequences

Some components are written by hand. The dependency tree stays small
enough to audit. The UI cannot use libraries that inject inline styles
or scripts.
