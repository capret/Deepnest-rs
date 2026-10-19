//! Device preferences are independent of cutting projects and their undo history.
use crate::i18n::Language;
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub language: Language,
}

pub fn path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    base.map(|p| p.join("Deepnest").join("preferences.json"))
}

impl Preferences {
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Write beside the destination, then replace atomically on the same filesystem.
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(self)?;
        let mut file = std::fs::File::create(&temporary)?;
        use std::io::Write;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        let result = std::fs::rename(&temporary, path);
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_round_trip_replace_and_recover_from_unknown_or_corrupt_files() {
        let dir = std::env::temp_dir().join(format!(
            "deepnest-locale-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let path = dir.join("preferences.json");
        assert_eq!(Preferences::load(&path).language, Language::English);
        Preferences {
            language: Language::SimplifiedChinese,
        }
        .save(&path)
        .unwrap();
        assert_eq!(
            Preferences::load(&path).language,
            Language::SimplifiedChinese
        );
        Preferences::default().save(&path).unwrap();
        assert_eq!(Preferences::load(&path).language, Language::English);
        std::fs::write(&path, r#"{"language":"future-locale"}"#).unwrap();
        assert_eq!(Preferences::load(&path).language, Language::English);
        std::fs::write(&path, "{").unwrap();
        assert_eq!(Preferences::load(&path).language, Language::English);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
