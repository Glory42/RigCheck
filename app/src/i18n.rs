//! UI translations with Fluent (https://projectfluent.org). One `.ftl` file per
//! language in `app/i18n/`, built into the binary.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use serde::{Deserialize, Serialize};
use unic_langid::LanguageIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "de")]
    German,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::English, Language::Turkish, Language::German];

    /// The language's name in that language, for the picker.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "ENGLISH",
            Language::Turkish => "TÜRKÇE",
            Language::German => "DEUTSCH",
        }
    }

    /// The IDs of every message in this language's file.
    #[cfg(test)]
    fn message_ids(self) -> impl Iterator<Item = &'static str> {
        self.ftl().lines().filter_map(|line| {
            let (id, _) = line.split_once(" =")?;
            let is_id = !id.is_empty() && !id.starts_with(['#', ' ', '\t']);
            is_id.then_some(id)
        })
    }

    fn ftl(self) -> &'static str {
        match self {
            Language::English => include_str!("../i18n/en.ftl"),
            Language::Turkish => include_str!("../i18n/tr.ftl"),
            Language::German => include_str!("../i18n/de.ftl"),
        }
    }

    fn id(self) -> LanguageIdentifier {
        let tag = match self {
            Language::English => "en",
            Language::Turkish => "tr",
            Language::German => "de",
        };
        tag.parse().expect("valid language tag")
    }

    /// The language for a POSIX locale like `tr_TR.UTF-8`, as found in
    /// `$LC_ALL` / `$LC_MESSAGES` / `$LANG`. English if it's not one we have.
    pub fn from_locale(locale: Option<&str>) -> Language {
        match locale.and_then(|locale| locale.get(..2)) {
            Some("tr") => Language::Turkish,
            Some("de") => Language::German,
            _ => Language::English,
        }
    }
}

/// Looks up UI text in one language.
pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

impl I18n {
    /// Loads a built-in translation. The files ship inside the binary, so a
    /// broken one is a bug the tests catch, not a runtime error.
    pub fn new(language: Language) -> I18n {
        let resource = FluentResource::try_new(language.ftl().to_owned())
            .unwrap_or_else(|(_, errors)| panic!("{language:?} .ftl has errors: {errors:?}"));
        let mut bundle = FluentBundle::new(vec![language.id()]);
        // Fluent wraps arguments in Unicode isolation marks by default; plain
        // left-to-right text doesn't need them.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .expect("message IDs are unique");
        I18n { bundle }
    }

    #[cfg(test)]
    fn has(&self, id: &str) -> bool {
        self.bundle.has_message(id)
    }

    /// The text for `id`.
    pub fn t(&self, id: &str) -> String {
        self.format(id, None)
    }

    /// The text for `id` with `{ $name }` arguments filled in.
    pub fn t_with(&self, id: &str, args: &[(&str, &str)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, *value);
        }
        self.format(id, Some(&fluent_args))
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
        let Some(pattern) = self.bundle.get_message(id).and_then(|m| m.value()) else {
            return id.to_owned();
        };
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_english_message() {
        for language in Language::ALL {
            let i18n = I18n::new(language);
            for id in Language::English.message_ids() {
                assert!(i18n.has(id), "{language:?} is missing `{id}`");
            }
        }
    }

    #[test]
    fn the_system_locale_picks_the_language() {
        let cases = [
            (Some("tr_TR.UTF-8"), Language::Turkish),
            (Some("de_AT.UTF-8"), Language::German),
            (Some("en_US.UTF-8"), Language::English),
            (Some("fr_FR.UTF-8"), Language::English),
            (Some("C"), Language::English),
            (None, Language::English),
        ];
        for (locale, expected) in cases {
            assert_eq!(Language::from_locale(locale), expected, "{locale:?}");
        }
    }
}
