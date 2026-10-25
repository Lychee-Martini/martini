use crate::error::MartiniError;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub clip_weights: PathBuf,
    pub tokenizer: PathBuf,
    pub unet_weights: PathBuf,
    pub vae_weights: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ModelManager {
    pub cache_dir: PathBuf,
}

impl ModelManager {
    pub fn new(custom_dir: Option<&Path>) -> Result<Self, MartiniError> {
        let cache_dir = match custom_dir {
            Some(d) => d.join("erase"),
            None => {
                let base = dirs::cache_dir().unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join(".cache"))
                        .unwrap_or_else(|| PathBuf::from("."))
                });
                base.join("martini").join("models").join("erase")
            }
        };

        Ok(Self { cache_dir })
    }

    pub fn ensure_models(&self) -> Result<ModelPaths, MartiniError> {
        fs::create_dir_all(&self.cache_dir).map_err(MartiniError::Io)?;

        let clip_weights = self.cache_dir.join("clip_model.safetensors");
        let tokenizer = self.cache_dir.join("tokenizer.json");
        let unet_weights = self.cache_dir.join("unet.safetensors");
        let vae_weights = self.cache_dir.join("vae.safetensors");

        Ok(ModelPaths {
            clip_weights,
            tokenizer,
            unet_weights,
            vae_weights,
        })
    }

    pub fn verify_checksum(path: &Path, expected_sha256: &str) -> Result<bool, MartiniError> {
        let mut file = fs::File::open(path).map_err(MartiniError::Io)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer).map_err(MartiniError::Io)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        let hex_str = format!("{:x}", result);
        Ok(hex_str.eq_ignore_ascii_case(expected_sha256))
    }
}
