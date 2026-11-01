//! User settings, saved as `settings.json` in the config dir (ROADMAP §7.1).

use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

use crate::i18n::Language;

const FILE: &str = "settings.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub appearance: Appearance,
    /// `None` follows the system language. Files saved before languages
    /// existed lack this field, which serde reads as `None`.
    pub language: Option<Language>,
}

/// The app's color theme, picked under Settings > Appearance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Ink,
    Midnight,
    Plum,
    Forest,
    #[default]
    Paper,
}

impl Appearance {
    /// In the order Settings shows them.
    pub const ALL: [Appearance; 5] = [
        Appearance::Paper,
        Appearance::Ink,
        Appearance::Midnight,
        Appearance::Plum,
        Appearance::Forest,
    ];
}

impl Settings {
    /// Reads the settings in `dir`.
    pub fn load(dir: &Path) -> Settings {
        let Ok(json) = fs::read_to_string(dir.join(FILE)) else {
            return Settings::default();
        };
        serde_json::from_str(&json).unwrap_or_default()
    }

    /// Writes the settings to `dir`, creating it if needed.
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)?;
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(dir.join(FILE), json + "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_fresh_install_uses_paper() {
        let dir = TempDir::new().unwrap();

        let settings = Settings::load(dir.path());

        assert_eq!(settings.appearance, Appearance::Paper);
    }

    #[test]
    fn a_saved_appearance_comes_back() {
        let dir = TempDir::new().unwrap();
        let settings = Settings {
            appearance: Appearance::Ink,
            ..Settings::default()
        };

        settings.save(dir.path()).unwrap();

        assert_eq!(Settings::load(dir.path()).appearance, Appearance::Ink);
    }

    #[test]
    fn a_broken_file_falls_back_to_the_defaults() {
        let dir = TempDir::new().unwrap();
        for broken in ["{not json", r#"{"appearance": "neon"}"#] {
            fs::write(dir.path().join("settings.json"), broken).unwrap();

            assert_eq!(
                Settings::load(dir.path()).appearance,
                Appearance::Paper,
                "{broken}"
            );
        }
    }
}
