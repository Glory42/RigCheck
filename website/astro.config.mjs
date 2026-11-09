import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://rigcheck.gorkemkaryol.dev",
  // English at /, the others under their language code.
  i18n: {
    locales: ["en", "tr", "de"],
    defaultLocale: "en",
    routing: { prefixDefaultLocale: false },
  },
  // Screenshots and icons are imported from the repo's assets/.
  vite: { server: { fs: { allow: [".."] } } },
});
