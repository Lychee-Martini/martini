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

impl Preset {
    pub fn load(name: &str) -> Result<Self, MartiniError> {
        PresetManager::load()
            .get(name)
            .cloned()
            .ok_or_else(|| MartiniError::InvalidInputData {
                reason: format!("Preset '{}' not found", name),
            })
    }

    pub fn apply_to(&self, cmd: &mut crate::cli::Commands) {
        match cmd {
            crate::cli::Commands::Convert {
                from,
                to,
                quality,
                lossless,
                recursive,
                delete_original,
                theme,
                paper_size,
                ..
            } => {
                if *from == "auto"
                    && let Some(ref f) = self.from
                {
                    *from = f.clone();
                }
                if to.is_none() {
                    *to = Some(self.to.clone());
                }
                if let Some(q) = self.quality {
                    *quality = q;
                }
                if let Some(l) = self.lossless {
                    *lossless = l;
                }
                if let Some(r) = self.recursive {
                    *recursive = r;
                }
                if let Some(d) = self.delete_original {
                    *delete_original = d;
                }
                if theme.is_none() && self.theme.is_some() {
                    *theme = self.theme.clone();
                }
                if paper_size.is_none() && self.paper_size.is_some() {
                    *paper_size = self.paper_size.clone();
                }
            }
            crate::cli::Commands::Watch {
                to,
                recursive,
                delete_original,
                move_originals,
                ..
            } => {
                if to.is_none() {
                    *to = Some(self.to.clone());
                }
                if let Some(r) = self.recursive {
                    *recursive = r;
                }
                if let Some(d) = self.delete_original {
                    *delete_original = d;
                }
                if move_originals.is_none() && self.move_originals.is_some() {
                    *move_originals = self.move_originals.clone();
                }
            }
            _ => {}
        }
    }
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
        if let Some(user_config) = dirs::config_dir()
            .map(|p| p.join("martini").join("presets.toml"))
            .filter(|p| p.exists())
        {
            let _ = manager.merge_from_path(&user_config);
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
        let file: PresetConfigFile =
            toml::from_str(&content).map_err(|e| MartiniError::InvalidInputData {
                reason: format!(
                    "Failed to parse presets file at '{}': {}",
                    path.display(),
                    e
                ),
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
