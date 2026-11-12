# Web UI

The only part of riehub that is not Rust. Directory: `web/`. Decision:
ADR 0011. The visual reference is the "Finexy" finance dashboard shot
by Sujon Hossain
(<https://me.muz.li/sujon_co/finexy-finance-saas-dashboard-design>).
riehub follows its layout language and palette, with its own content.
It copies no asset from it.

## 1. Stack

| Part | Choice |
|---|---|
| Language | TypeScript, `strict` |
| Framework | React with React Router and TanStack Query |
| Build | Vite, output in `web/dist`, embedded in the `riehub` binary with `rust-embed` |
| Styling | Tailwind CSS, tokens as CSS variables in `web/src/styles/tokens.css` |
| Icons | `lucide-react` |
| Charts | small SVG components in `web/src/components/charts`, no chart library |
| Font | Inter variable, self-hosted in `web/src/assets/fonts` (OFL licence file next to it) |
| API types | generated from `docs/api/openapi.json` with `openapi-typescript` |
| Tests | Vitest with Testing Library for components, Playwright for end-to-end |
| Lint | ESLint with `typescript-eslint`, Prettier |
| Package manager | npm. `npm ci --ignore-scripts` everywhere |

Rules:

- No other runtime dependency without an ADR. Fewer packages is a
  security feature (S10).
- Nothing is loaded from another origin: no CDN, no web fonts, no
  analytics (S12). The CSP in [security](security.md) forbids it and
  the build must work under it: no inline scripts, no inline styles,
  no `eval`.
- The UI never stores a token or content in `localStorage` or
  `sessionStorage`. The session is the `HttpOnly` cookie. Theme
  preference is the only thing kept in `localStorage`.
- Secrets are shown once, in a dialog with a copy button, and are not
  kept in component state after it closes.
- Server state is fetched, not cached across users: the query cache is
  cleared on logout.

## 2. Design tokens

Light theme is the default. Dark theme uses the same accents.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--canvas` | `#F5F5F5` | `#0F110F` | page background |
| `--surface` | `#FFFFFF` | `#181B18` | cards |
| `--subtle` | `#F3F4F3` | `#212521` | inner tiles, inputs, pills |
| `--ink` | `#1A1A18` | `#F2F4F1` | text, active nav pill, dark chart bars |
| `--ink-inverse` | `#FFFFFF` | `#0F110F` | text on `--ink` |
| `--muted` | `#6B6F6B` | `#A1A7A0` | secondary text |
| `--faint` | `#9CA09C` | `#6F756E` | hints, axis labels |
| `--line` | `#E7E8E6` | `#2A2F2A` | hairlines, table header border |
| `--lime` | `#A8EF3D` | `#A8EF3D` | primary action, positive chart bars |
| `--lime-strong` | `#8FDD22` | `#8FDD22` | primary action hover |
| `--forest` | `#14532D` | `#1E7A43` | brand, progress fill |
| `--hero-from`, `--hero-to` | `#3E9E3A`, `#0D4A38` | same | gradient of the hero stat card, top to bottom |
| `--good`, `--good-bg` | `#1F9D57`, `#E4F6EA` | `#4ADE80`, `#143021` | up deltas, completed |
| `--warn`, `--warn-bg` | `#D9A21B`, `#FBF2D8` | `#FACC15`, `#33290B` | in progress |
| `--bad`, `--bad-bg` | `#D93F45`, `#FCE9E9` | `#F87171`, `#3A1517` | down deltas, errors |

| Scale | Values |
|---|---|
| Radius | page shell 32, card 24, tile 16, control and pill 999 |
| Spacing | 4, 8, 12, 16, 24, 32, 48. Card padding 24, grid gap 24 |
| Type | greeting 44/48 semibold, section title 22/28 semibold, card title 18/24 medium, body 15/22, small 13/18, stat value 36/40 semibold. Numbers use `font-variant-numeric: tabular-nums` |
| Controls | height 48 for buttons, 44 for inputs and icon buttons |
| Elevation | none. Cards separate from the canvas by colour only |
| Motion | 150 ms ease-out on hover and focus, none when `prefers-reduced-motion` |

Text on `--lime` is always `--ink` (`#1A1A18`), never white. Focus is a
2 px `--ink` outline with 2 px offset on every interactive element.
Colour is never the only signal: status has a dot and a word, deltas
have an arrow and a number.

## 3. Layout

```
+--------------------------------------------------------------------------+
| (logo pill)      ( Overview | Chat | Router | Tools | Automation | Usage )   (search)(bell)(help) (user pill) |
|                                                                          |
| (theme     Good morning, Name                                            |
|  toggle)   One line of status.                                           |
|                                                                          |
| (section   [ card ] [ card ] [ card ]                                    |
|  icon      [ card        ] [ wide card                         ]         |
|  rail)                                                                   |
| (help,                                                                   |
|  logout)                                                                 |
+--------------------------------------------------------------------------+
```

- **Top bar**: a logo pill on the left. A centred pill group for the
  six areas, where the active one is a filled `--ink` pill. On the
  right a pill with icon buttons and a user pill with avatar initial,
  name, email and a menu.
- **Left rail**: floating pills, 56 px wide. Top pill: light and dark
  toggle. Middle pill: icons for the pages of the current area, the
  active one a filled `--ink` circle. Bottom pill: help and logout.
- **Content**: a 12-column grid, maximum width 1440. Below 1024 px the
  rail becomes a bottom bar and cards stack in one column.

## 4. Components

Built once in `web/src/components`, each with a Vitest test:

`Card`, `StatCard` (label, icon chip, value, delta pill), `HeroStatCard`
(gradient), `Button` (primary lime, secondary subtle, ghost, danger),
`IconButton`, `Pill`, `NavPills`, `Rail`, `Input`, `SearchInput`,
`Select`, `Toggle`, `Dialog`, `Toast`, `ProgressBar` (solid fill,
hatched remainder), `StackedBars`, `Sparkline`, `DataTable` (checkbox
column, status dot, row menu, empty state, pagination), `StatusDot`,
`SecretReveal` (show once, copy), `CodeBlock`, `Editor` (plain textarea
with line numbers, for YAML and markdown), `EmptyState`, `ErrorState`.

## 5. Pages

| Area | Pages | Since |
|---|---|---|
| (signed out) | Setup, Login, Recover, Accept invite, Vault unlock | 0.1.0 |
| Overview | Dashboard | 0.2.0 |
| Chat | Chat list and conversation | 0.5.0 |
| Router | Connections, Models, Routing groups, Hub keys, Privacy | 0.2.0 to 0.6.0 |
| Tools | Connectors, Virtual MCPs | 0.4.0 |
| Automation | Skills, Instructions, Agent profiles, Workflows, Runs | 0.5.0 |
| Usage | Usage and cost, Activity, Limits | 0.2.0, 0.3.0 |
| Account (user menu) | Profile, Security (sessions, keys, grants, recovery), Appearance | 0.1.0 |
| Admin (user menu, needs a permission) | Users, Teams, Catalog, Policies, Usage, Audit, Settings | 0.1.0 to 0.7.0 |

The dashboard maps the reference shot to riehub data:

| Reference block | riehub content |
|---|---|
| Greeting | "Good morning, Name" and one line: requests today and anything needing attention |
| Total Balance with two buttons | Spend this month, delta against last month, buttons "New chat" (primary) and "New hub key" |
| Wallets | The three connections with the least quota left: name, quota left, status |
| Four stat cards, the first as hero | Requests, Tokens, Cost, Average latency, each with a delta for the period |
| Profit and Loss bars | Tokens per day or month, stacked: output on input |
| Monthly Spending Limit | The user's budget: spent and ceiling |
| My Cards | Connections as dark cards: provider, status pill, last four characters of the credential's fingerprint (never of the credential) |
| Recent Activities table | Recent requests: id, model, cost, status, time. Metadata only |

Admin pages show only what section 2.1 of the
[specification](../SPEC.md) allows. They reuse the same components.

## 6. Accessibility and quality bar

- Keyboard reachable everywhere, visible focus, correct roles and
  labels, dialogs trap focus.
- Contrast at least 4.5 to 1 for text in both themes.
- Playwright runs the main journeys in both themes and runs an
  automated accessibility check on each page.
