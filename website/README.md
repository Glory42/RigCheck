# Website

The showcase site at https://rigcheck.gorkemkaryol.dev: one static page in English (`/`), Turkish (`/tr/`) and German (`/de/`), built with [Astro](https://astro.build) in the app's Paper appearance (colors, borders and shadows from `app/src/style.rs`).

```sh
cd website
npm install
npm run dev      # http://localhost:4321
npm run build    # static files in dist/
```

- **Text:** `src/i18n.ts`, one object per language. Stamp and table words match the app's translations in `app/i18n/`.
- **Language:** the English page follows the browser's language on a first visit (`src/layouts/Base.astro`); a language picked in the header is saved in the browser and always wins.
- **Downloads:** the version and the package links come from GitHub's latest release at build time (`src/release.ts`). Without network the buttons link to the releases page.
- **Images:** screenshots and icons are imported from the repo's `assets/`, so they're never copied.
- **No tracking:** no analytics, no cookies, fonts served from the site itself (Fira Sans and Fira Mono, via Fontsource).

## Cloudflare Pages

Connect the GitHub repository in Cloudflare Pages with:

| Setting | Value |
|---|---|
| Production branch | `master` |
| Framework preset | Astro |
| Root directory | `website` |
| Build command | `npm run build` |
| Build output directory | `dist` |

Node comes from `website/.node-version`. Then add the custom domain `rigcheck.gorkemkaryol.dev` under the project's Custom domains.

The download buttons show the version from the last build, so the site needs a rebuild after each release. The follow-up pull request that moves `packaging/` to the new version does that when it merges to `master`; to limit builds to changes that matter, set the build watch paths to `website/**` and `packaging/aur/**`.
