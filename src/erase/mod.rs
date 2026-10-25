pub mod mask;
pub mod models;
pub mod pipeline;

use crate::error::MartiniError;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Clone, Args)]
pub struct EraseArgs {
    /// Path to the source image to edit
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

    /// Path to save the edited image (defaults to <INPUT_STEM>_erased.<EXT>)
    #[arg(short = 'o', long = "output", value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// English description of the object/watermark to erase (e.g. 'watermark', 'person')
    #[arg(short = 't', long = "target")]
    pub target: Option<String>,

    /// Path to an optional pre-existing binary mask image
    #[arg(long = "mask")]
    pub mask: Option<PathBuf>,

    /// Number of diffusion inference steps (default: 20)
    #[arg(long, default_value_t = 20)]
    pub steps: usize,

    /// Mask sensitivity threshold for localization (default: 0.35)
    #[arg(long, default_value_t = 0.35)]
    pub threshold: f32,

    /// Mask boundary dilation radius in pixels (default: 8)
    #[arg(long, default_value_t = 8)]
    pub dilate: u32,

    /// Save the intermediate generated binary mask for verification
    #[arg(long)]
    pub save_mask: bool,

    /// Custom directory for downloaded model weights
    #[arg(long = "model-dir")]
    pub model_dir: Option<PathBuf>,

    /// Output execution metadata as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run_erase(args: &EraseArgs) -> Result<(), MartiniError> {
    if args.target.is_none() && args.mask.is_none() {
        return Err(MartiniError::InvalidInputData {
            reason: "Either --target or --mask must be specified for erase".to_string(),
        });
    }

    if !args.input.exists() {
        return Err(MartiniError::InputFileNotFound {
            path: args.input.to_string_lossy().to_string(),
        });
    }

    Ok(())
}
