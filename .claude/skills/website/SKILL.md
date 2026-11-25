---
name: website
description: Change rigcheck's Astro showcase site in website/. Use when editing site copy, layout, styles, download links, or the site's English/Turkish/German pages.
---

# Website

`website/` is a separate Astro project (its own `package.json` and
`node_modules`, not part of the Cargo workspace). It serves one page in
English (`/`), Turkish (`/tr/`) and German (`/de/`).

## Commands

```sh
cd website
npm install
npm run dev      # http://localhost:4321
npm run build    # static output in dist/
npm run preview
```

Node version comes from `website/.node-version`. Build before opening a PR.

## Where things are

| What | Where |
|---|---|
| All text, per language | `website/src/i18n.ts` (three objects) |
| Page markup/sections | `website/src/components/Home.astro` |
| Layout, language detection | `website/src/layouts/Base.astro` |
| Header/language picker | `website/src/components/Header.astro` |
| Styles | `website/src/styles/global.css` |
| Release links/version | `website/src/release.ts` (GitHub latest release at build time) |
| Routes | `website/src/pages/index.astro` |

## Rules

- **Keep the three languages in sync.** Adding a key means adding it to the
  `en`, `tr` and `de` objects. The `Strings` type and `Record<Lang, Strings>`
  map catch missing keys.
- **Match the app's wording.** Stamp and table words (`READY`, `TOO WEAK`,
  `BLOCKED`, `PROTONDB`, …) must match `app/i18n/*.ftl`. When you change the
  app's translation, update the site too.
- **No tracking, no cookies, no external fonts.** Fonts are served from
  Fontsource; the site has no analytics.
- **Images come from the repo's `assets/`** via imports (Vite allows the
  parent directory). Don't copy files into `website/public/`.
- **Download links are generated** from the latest release; don't hardcode a
  version. Without network the links fall back to the releases page.
- **Look and feel follows the app**: colors, borders and shadows come from
  `app/src/style.rs` (Paper theme).

## Before/after

```sh
cd website && npm run build
```

CI (`.github/workflows/website.yml`) builds the site on PRs that touch it. A PR
that only changes `website/` skips the Rust checks, so still run the build
yourself. Don't commit `dist/` or `node_modules/`.
