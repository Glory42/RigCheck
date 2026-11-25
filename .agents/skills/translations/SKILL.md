---
name: translations
description: Add or fix rigcheck's user-facing text, in the Fluent files under app/i18n or the Astro site's copy. Use when changing a label, adding a message, syncing app and website wording, or adding a language.
---

# Translations

UI text lives in `app/i18n/` as Fluent files: `en.ftl`, `tr.ftl`, `de.ftl`.
They're compiled into the binary; a missing ID or an invalid file fails tests.

## Change or add a message

1. Add or edit the ID in `en.ftl` first. IDs are kebab-case and descriptive
   (`library-search`, `vram-dedicated`).
2. Mirror the same ID in `tr.ftl` and `de.ftl`. Every English message must
   exist in every language; `app/src/i18n.rs` has a test for this.
3. Copy placeholders exactly: `{ $size }` in one language must be `{ $size }`
   in the others (order and surrounding words can change).
4. Write uppercase labels uppercase in the file (`AYARLAR`, `EINSTELLUNGEN`).
   Never uppercase in code: Turkish needs `İ`/`ı`, and Rust's uppercasing gets
   it wrong.
5. Don't translate game names or Steam's requirement text; those stay English.

Run:

```sh
cargo test -p rigcheck every_language_has_every_english_message
```

Check the screen that uses the text:

```sh
cargo run -p rigcheck
```

## Add a language

1. Copy `app/i18n/en.ftl` to `<code>.ftl` and translate every line.
2. In `app/src/i18n.rs`, add a `Language` variant, then update `ALL`,
   `native_name`, `ftl()`, `id()` and `from_locale()`. The picker gets the
   language from `Language::ALL`.
3. If the site should offer it too, update `website/src/i18n.ts`
   (`languages`, `nativeNames`, and the new copy object) and the locales in
   `website/astro.config.mjs`.
4. Run the workspace tests: the parity test catches a missed ID.

## Website copy

`website/src/i18n.ts` holds three objects (en/tr/de). Stamp and table words
must match the app's translations, so sync them when the app's wording
changes. Keep the page structure the same across languages.

```sh
cd website && npm run build
```

## Don't

- Don't hardcode user-visible strings in Rust; route them through `I18n`.
- Don't reorder or rename existing IDs without updating every call site.
- Don't machine-translate a whole file without checking the app's domain
  words (`CONTEXT.md`) and the existing tone.
