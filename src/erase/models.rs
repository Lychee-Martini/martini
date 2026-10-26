use crate::error::MartiniError;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    clap::ValueEnum,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum InpaintModelPreset {
    #[default]
    #[value(name = "sd-1.5")]
    Sd15,
    #[value(name = "lcm")]
    Lcm,
    #[value(name = "sd-turbo")]
    SdTurbo,
}

impl InpaintModelPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sd15 => "sd-1.5",
            Self::Lcm => "lcm",
            Self::SdTurbo => "sd-turbo",
        }
    }

    pub fn default_steps(&self) -> usize {
        match self {
            Self::Sd15 => 20,
            Self::Lcm => 4,
            Self::SdTurbo => 1,
        }
    }

    pub fn repo_id(&self) -> &'static str {
        match self {
            Self::Sd15 => "runwayml/stable-diffusion-inpainting",
            Self::Lcm => "latent-consistency/lcm-sd-1.5",
            Self::SdTurbo => "stabilityai/sd-turbo",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub clip_weights: PathBuf,
    pub tokenizer: PathBuf,
    pub unet_weights: PathBuf,
    pub vae_weights: PathBuf,
    pub text_encoder_weights: PathBuf,
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

    pub fn get_expected_paths(&self, preset: InpaintModelPreset) -> ModelPaths {
        let clip_dir = self.cache_dir.join("clip");
        let preset_dir = self.cache_dir.join(preset.as_str());

        ModelPaths {
            clip_weights: clip_dir.join("model.safetensors"),
            tokenizer: clip_dir.join("tokenizer.json"),
            unet_weights: preset_dir.join("unet.safetensors"),
            vae_weights: preset_dir.join("vae.safetensors"),
            text_encoder_weights: preset_dir.join("text_encoder.safetensors"),
        }
    }

    pub fn ensure_models(&self, preset: InpaintModelPreset) -> Result<ModelPaths, MartiniError> {
        let paths = self.get_expected_paths(preset);
        let all_exist = paths.clip_weights.exists()
            && paths.tokenizer.exists()
            && paths.unet_weights.exists()
            && paths.vae_weights.exists()
            && paths.text_encoder_weights.exists();

        if all_exist {
            return Ok(paths);
        }

        #[cfg(feature = "nn-cpu")]
        {
            self.download_preset(preset)?;
            Ok(paths)
        }

        #[cfg(not(feature = "nn-cpu"))]
        {
            Ok(paths)
        }
    }

    pub fn download_preset(&self, preset: InpaintModelPreset) -> Result<(), MartiniError> {
        let paths = self.get_expected_paths(preset);

        if let Some(parent) = paths.clip_weights.parent() {
            fs::create_dir_all(parent).map_err(MartiniError::Io)?;
        }
        if let Some(parent) = paths.unet_weights.parent() {
            fs::create_dir_all(parent).map_err(MartiniError::Io)?;
        }

        #[cfg(feature = "nn-cpu")]
        {
            use indicatif::{ProgressBar, ProgressStyle};

            println!(
                "📥 Downloading inpainting weights for preset '{}'...",
                preset.as_str()
            );

            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .unwrap(),
            );

            let api = hf_hub::api::sync::ApiBuilder::new()
                .with_cache_dir(self.cache_dir.clone())
                .build()
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to initialize Hugging Face API client: {}", e),
                })?;

            // 1. Download CLIP localization weights
            if !paths.clip_weights.exists() || !paths.tokenizer.exists() {
                pb.set_message("Downloading CLIP visual grounding weights...");
                let clip_repo = api.model("openai/clip-vit-base-patch32".to_string());
                let downloaded_clip = clip_repo.get("model.safetensors").map_err(|e| {
                    MartiniError::InvalidInputData {
                        reason: format!(
                            "Failed to download CLIP model: {}. Tip: Use --fast for model-free inpainting.",
                            e
                        ),
                    }
                })?;
                let downloaded_tok = clip_repo.get("tokenizer.json").map_err(|e| {
                    MartiniError::InvalidInputData {
                        reason: format!("Failed to download CLIP tokenizer: {}", e),
                    }
                })?;
                fs::copy(&downloaded_clip, &paths.clip_weights).map_err(MartiniError::Io)?;
                fs::copy(&downloaded_tok, &paths.tokenizer).map_err(MartiniError::Io)?;
            }

            // 2. Download Inpainting UNet, VAE, and Text Encoder
            let model_repo = api.model(preset.repo_id().to_string());

            if !paths.unet_weights.exists() {
                pb.set_message(format!(
                    "Downloading UNet weights from {}...",
                    preset.repo_id()
                ));
                let downloaded = model_repo
                    .get("unet/diffusion_pytorch_model.safetensors")
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Failed to download UNet weights: {}", e),
                    })?;
                fs::copy(&downloaded, &paths.unet_weights).map_err(MartiniError::Io)?;
            }

            if !paths.vae_weights.exists() {
                pb.set_message("Downloading VAE weights...");
                let downloaded = model_repo
                    .get("vae/diffusion_pytorch_model.safetensors")
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Failed to download VAE weights: {}", e),
                    })?;
                fs::copy(&downloaded, &paths.vae_weights).map_err(MartiniError::Io)?;
            }

            if !paths.text_encoder_weights.exists() {
                pb.set_message("Downloading Text Encoder weights...");
                let downloaded = model_repo
                    .get("text_encoder/model.safetensors")
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Failed to download text encoder weights: {}", e),
                    })?;
                fs::copy(&downloaded, &paths.text_encoder_weights).map_err(MartiniError::Io)?;
            }

            pb.finish_with_message("✨ Model weights downloaded successfully!");
        }

        #[cfg(not(feature = "nn-cpu"))]
        {
            let _ = paths;
        }

        Ok(())
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
