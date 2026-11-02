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
    /// Hardware-table names the user picked instead of what was detected.
    pub gpu_override: Option<String>,
    pub cpu_override: Option<String>,
    /// Only check for a new games.db when asked, not at every start.
    /// `default`: files saved before this existed would otherwise fail to
    /// load, and every setting would reset.
    #[serde(default)]
    pub manual_updates: bool,
    /// The most space downloaded covers may take; `None` for the default.
    #[serde(default)]
    pub image_cache_mb: Option<u32>,
}

/// The app's color theme, picked under Settings > Appearance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Paper when the system is light, Ink when it's dark.
    #[default]
    System,
    Ink,
    Midnight,
    Plum,
    Forest,
    Paper,
}

impl Appearance {
    /// The appearance to draw with, given the system's light or dark mode.
    pub fn resolve(self, system: iced::theme::Mode) -> Appearance {
        match (self, system) {
            (Appearance::System, iced::theme::Mode::Dark) => Appearance::Ink,
            (Appearance::System, _) => Appearance::Paper,
            (picked, _) => picked,
        }
    }

    /// In the order Settings shows them.
    pub const ALL: [Appearance; 6] = [
        Appearance::System,
        Appearance::Paper,
        Appearance::Ink,
        Appearance::Midnight,
        Appearance::Plum,
        Appearance::Forest,
    ];
}

impl Settings {
    /// The image cache limit in bytes: 500 MB unless set (ROADMAP §7.1).
    pub fn image_cache_bytes(&self) -> u64 {
        u64::from(self.image_cache_mb.unwrap_or(500)) * 1_000_000
    }

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
    fn a_fresh_install_follows_the_system() {
        let dir = TempDir::new().unwrap();

        let settings = Settings::load(dir.path());

        assert_eq!(settings.appearance, Appearance::System);
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
                Appearance::System,
                "{broken}"
            );
        }
    }

    /// Written by v0.5, before `manual_updates` existed.
    #[test]
    fn settings_from_an_older_version_keep_their_choices() {
        let dir = TempDir::new().unwrap();
        let old = r#"{"appearance": "ink", "language": "tr", "gpu_override": "GeForce RTX 3060", "cpu_override": null}"#;
        fs::write(dir.path().join("settings.json"), old).unwrap();

        let settings = Settings::load(dir.path());

        assert_eq!(settings.appearance, Appearance::Ink);
        assert_eq!(settings.gpu_override.as_deref(), Some("GeForce RTX 3060"));
        assert!(!settings.manual_updates);
    }
}
