pub mod mask;
pub mod models;
pub mod pipeline;

use crate::erase::mask::{compute_bounding_box, dilate_mask};
use crate::erase::models::{InpaintModelPreset, ModelManager};
use crate::erase::pipeline::InpaintPipeline;
use crate::error::MartiniError;
use clap::Args;
use image::{GenericImageView, GrayImage, imageops};
use std::path::{Path, PathBuf};

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

    /// English description of the object to generate in place of the target
    #[arg(short = 'r', long = "replace")]
    pub replace: Option<String>,

    /// Path to an optional pre-existing binary mask image
    #[arg(long = "mask")]
    pub mask: Option<PathBuf>,

    /// Inpainting model preset to use
    #[arg(long = "model", default_value = "sd-1.5")]
    pub model: InpaintModelPreset,

    /// Pre-download model weights for the selected preset (or 'all') and exit
    #[arg(long = "download-models", num_args = 0..=1)]
    pub download_models: Option<Option<String>>,

    /// Instrument and print stage-by-stage latency and peak memory
    #[arg(long)]
    pub benchmark: bool,

    /// Use classical Fast Marching inpainting (instant, no model download needed)
    #[arg(long)]
    pub fast: bool,

    /// Compute device target: auto, cuda, or cpu
    #[arg(long = "device", default_value = "auto")]
    pub device: String,

    /// Number of diffusion inference steps (default: 20)
    #[arg(long, default_value_t = 20)]
    pub steps: usize,

    /// Mask sensitivity threshold for localization (default: 0.35)
    #[arg(long, default_value_t = 0.35)]
    pub threshold: f32,

    /// Mask boundary dilation radius in pixels (default: 3)
    #[arg(long, default_value_t = 3)]
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

    // 1. Resolve output path
    let output_path = match &args.output {
        Some(p) => p.clone(),
        None => {
            let stem = args
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let ext = args
                .input
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let parent = args.input.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!("{}_erased.{}", stem, ext))
        }
    };

    // 2. Load input image
    let img = image::open(&args.input).map_err(|e| MartiniError::InvalidInputData {
        reason: format!(
            "Failed to read input image '{}': {}",
            args.input.display(),
            e
        ),
    })?;

    let (img_w, img_h) = img.dimensions();

    // 3. Resolve or load mask
    let mut mask = if let Some(ref mask_file) = args.mask {
        if !mask_file.exists() {
            return Err(MartiniError::InputFileNotFound {
                path: mask_file.to_string_lossy().to_string(),
            });
        }
        let mask_img = image::open(mask_file).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to read mask image '{}': {}", mask_file.display(), e),
        })?;
        let luma = mask_img.to_luma8();
        if luma.dimensions() != (img_w, img_h) {
            imageops::resize(&luma, img_w, img_h, imageops::FilterType::Nearest)
        } else {
            luma
        }
    } else if let Some(ref target) = args.target {
        // Text/watermark/annotation mask generation
        let detected = mask::detect_text_mask(&img, args.threshold);
        let mut count = 0;
        for p in detected.pixels() {
            if p[0] > 0 {
                count += 1;
            }
        }
        if count == 0 {
            tracing::warn!(
                "No target objects found matching '{}' with threshold {}",
                target,
                args.threshold
            );
        }
        detected
    } else {
        GrayImage::new(img_w, img_h)
    };

    // 4. Apply dilation
    if args.dilate > 0 {
        mask = dilate_mask(&mask, args.dilate);
    }

    // 5. Save intermediate mask if requested
    if args.save_mask {
        let mask_output_path = {
            let stem = args
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let parent = args.input.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!("{}_mask.png", stem))
        };
        mask.save(&mask_output_path)
            .map_err(|e| MartiniError::Io(std::io::Error::other(e)))?;
    }

    // 6. Inpainting execution
    let model_mgr = ModelManager::new(args.model_dir.as_deref())?;
    let model_paths = model_mgr.ensure_models()?;
    let pipeline = InpaintPipeline::load(&model_paths)?;

    let result_img = if let Some(bbox) = compute_bounding_box(&mask, 16) {
        let (crop_x, crop_y, crop_w, crop_h) = bbox;
        let crop_patch = img.crop_imm(crop_x, crop_y, crop_w, crop_h);
        let crop_mask = imageops::crop_imm(&mask, crop_x, crop_y, crop_w, crop_h).to_image();
        let inpainted_patch = pipeline.inpaint(&crop_patch, &crop_mask, args.steps)?;
        InpaintPipeline::blend_result(&img, &inpainted_patch, &mask, bbox)?
    } else {
        // If mask is empty, output is original image
        img
    };

    // 7. Save output
    result_img
        .save(&output_path)
        .map_err(|e| MartiniError::Io(std::io::Error::other(e)))?;

    // 8. Output status
    if args.json {
        let json_meta = serde_json::json!({
            "input": args.input.to_string_lossy(),
            "output": output_path.to_string_lossy(),
            "target": args.target,
            "steps": args.steps,
            "success": true,
        });
        println!("{}", serde_json::to_string_pretty(&json_meta).unwrap());
    } else {
        println!("✨ Object erasure complete!");
        println!("📄 Output saved to: {}", output_path.display());
    }

    Ok(())
}
