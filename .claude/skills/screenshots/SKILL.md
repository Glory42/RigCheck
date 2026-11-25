---
name: screenshots
description: Regenerate rigcheck's README and website screenshots after a UI change. Use when a screen, theme, wording or layout looks different, or when the checked-in screenshots are stale.
---

# Screenshots

Screenshots are rendered by the real app, offscreen, from an ignored test:
`app/src/screenshots.rs`. The checked-in images live in `assets/screenshots/`
and are also imported by `website/src/components/Home.astro`.

## 1. Get a real database

The test needs a real `games.db` (and the real `$HOME`, for installed games and
Steam's cover art). Use the app's downloaded copy:

```sh
ls ~/.local/share/rigcheck/games.db
```

If it's missing, open the app once so it downloads the database, or point
`RIGCHECK_SCREENSHOT_DB` at any real `games.db`.

## 2. Render

```sh
RIGCHECK_SCREENSHOT_DB=~/.local/share/rigcheck/games.db \
  cargo test -p rigcheck screenshots -- --ignored
```

PNGs land in `target/screenshots/`. The set includes the Library (Paper and
Ink), My Rig, Catalog, the game page, Compare, Settings, About and the
Owned-games Library. Rendering sleeps between frames so cover images load.

## 3. Keep the ones that matter

Convert only the images used by the repo to WebP, matching existing names:

```sh
magick target/screenshots/library.png -quality 90 assets/screenshots/library.webp
```

Current files: `library`, `library-ink`, `catalog`, `game`, `my-rig`,
`settings`, `about`. Replace the ones whose screen changed; don't add new ones
to the repo unless a doc links them.

## 4. Update the text that describes them

- `README.md` has the screenshot table with `alt` text and captions.
- `packaging/dev.gorkemkaryol.rigcheck.metainfo.xml` may reference screenshots.
- `website/src/components/Home.astro` imports the site's images.

If a screen now shows a different feature, update the alt text too. Alt text
describes what the screenshot proves, not just the page name.

## 5. Check

```sh
git status --short assets/screenshots/
```

Look at the changed images before committing. Don't commit `target/screenshots/`
(it's ignored) and don't leave the ignored test enabled in CI.
