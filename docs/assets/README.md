# riehub brand assets

The SVGs here are generated placeholders (an initial in a rounded
square). Replace them with the real logo when it exists, keeping the
same file names so the README, the docs site and the favicon pick them up
without other changes.

## Files

| File | Use |
|------|-----|
| `logo.svg` | Primary logo (mark + wordmark) for light backgrounds, used by `README.md` and the docs home page |
| `logo-dark.svg` | Same, for dark backgrounds |
| `mark.svg` | Mark only, also the docs site favicon |
| `mark-dark.svg` | Mark only, for dark backgrounds, used in the docs site header |
| `site.css` | Maps the brand colours onto the MkDocs Material theme |

When the real logo lands, also consider adding:

- `favicon.ico` (16-256 px in one file) and point `theme.favicon` in
  `mkdocs.yml` at it,
- `apple-touch-icon.png` (180×180),
- `social.png` (1280×640), uploaded as the GitHub repository's social
  preview (Settings > General > Social preview).

## Colours

| Name | Hex |
|------|-----|
| Primary | `#14532D` |
| Accent | `#A3E635` |

Keep clear space around the logo, and don't recolour, stretch or
rearrange the mark and wordmark.
