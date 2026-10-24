use crate::error::MartiniError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub from: Option<String>,
    pub to: String,
    pub quality: Option<u8>,
    pub lossless: Option<bool>,
    pub recursive: Option<bool>,
    pub overwrite: Option<bool>,
    pub delete_original: Option<bool>,
    pub move_originals: Option<PathBuf>,
    pub dpi: Option<u16>,
    pub theme: Option<String>,
    pub paper_size: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct PresetConfigFile {
    #[serde(default)]
    presets: HashMap<String, Preset>,
}

pub struct PresetManager {
    presets: HashMap<String, Preset>,
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::load()
    }
}

impl PresetManager {
    pub fn empty() -> Self {
        Self {
            presets: HashMap::new(),
        }
    }

    pub fn load() -> Self {
        let mut manager = Self::empty();
        // 1. User config directory: %APPDATA%/martini/presets.toml or ~/.config/martini/presets.toml
        if let Some(user_config) = get_user_config_path() {
            if user_config.exists() {
                let _ = manager.merge_from_path(&user_config);
            }
        }
        // 2. Project local: ./martini-presets.toml
        let local_path = Path::new("martini-presets.toml");
        if local_path.exists() {
            let _ = manager.merge_from_path(local_path);
        }
        manager
    }

    pub fn load_from_path(path: &Path) -> Result<Self, MartiniError> {
        let mut manager = Self::empty();
        manager.merge_from_path(path)?;
        Ok(manager)
    }

    pub fn merge_from_path(&mut self, path: &Path) -> Result<(), MartiniError> {
        let content = fs::read_to_string(path).map_err(MartiniError::Io)?;
        let file: PresetConfigFile = toml::from_str(&content).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to parse presets file at '{}': {}", path.display(), e),
            }
        })?;
        for (name, preset) in file.presets {
            self.presets.insert(name, preset);
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.presets.get(name)
    }

    pub fn list(&self) -> Vec<(&str, &Preset)> {
        let mut list: Vec<_> = self.presets.iter().map(|(k, v)| (k.as_str(), v)).collect();
        list.sort_by_key(|(k, _)| *k);
        list
    }
}

fn get_user_config_path() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        Some(PathBuf::from(appdata).join("martini").join("presets.toml"))
    } else if let Ok(home) = std::env::var("HOME") {
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("martini")
                .join("presets.toml"),
        )
    } else {
        None
    }
}
