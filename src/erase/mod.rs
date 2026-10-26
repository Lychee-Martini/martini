pub mod benchmark;
pub mod clip;
pub mod device;
pub mod mask;
pub mod models;
pub mod pipeline;

use crate::erase::benchmark::BenchmarkReport;
use crate::erase::clip::ClipGrounding;
use crate::erase::device::resolve_device_target;
use crate::erase::mask::{compute_bounding_box, dilate_mask};
use crate::erase::models::{InpaintModelPreset, ModelManager};
use crate::erase::pipeline::{CandleInpainter, ClassicalInpainter, InpaintPipeline};
use crate::error::MartiniError;
use clap::Args;
use image::{GenericImageView, GrayImage, imageops};
use std::path::{Path, PathBuf};
use std::time::Instant;

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
    // 0. Handle model pre-download if requested
    if let Some(ref download_arg) = args.download_models {
        let model_mgr = ModelManager::new(args.model_dir.as_deref())?;
        let target_preset = match download_arg.as_deref() {
            Some("lcm") => InpaintModelPreset::Lcm,
            Some("sd-turbo") => InpaintModelPreset::SdTurbo,
            _ => args.model,
        };
        println!(
            "🚀 Pre-downloading weights for preset '{}'...",
            target_preset.as_str()
        );
        model_mgr.download_preset(target_preset)?;
        println!("✅ Download complete.");
        return Ok(());
    }

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

    // 3. Resolve device & benchmark instrumentation
    let _dev_target = resolve_device_target(&args.device)?;
    #[cfg(feature = "erase")]
    let (candle_dev, dev_name) = crate::erase::device::select_candle_device(_dev_target);
    #[cfg(not(feature = "erase"))]
    let dev_name = "CPU".to_string();

    let mut benchmark_report = if args.benchmark {
        Some(BenchmarkReport::new(0, 0, 0, 0, 0, 0, None))
    } else {
        None
    };

    let model_mgr = ModelManager::new(args.model_dir.as_deref())?;

    // 4. Resolve or load mask
    let t_loc_start = Instant::now();
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
        if args.fast {
            mask::detect_text_mask(&img, args.threshold)
        } else {
            let model_paths = model_mgr.ensure_models(args.model)?;
            #[cfg(feature = "erase")]
            let clip_grounding = ClipGrounding::load(
                &model_paths.clip_weights,
                &model_paths.tokenizer,
                &candle_dev,
            )?;
            #[cfg(not(feature = "erase"))]
            let clip_grounding =
                ClipGrounding::load(&model_paths.clip_weights, &model_paths.tokenizer)?;

            clip_grounding.locate(&img, target, args.threshold)?
        }
    } else {
        GrayImage::new(img_w, img_h)
    };

    let loc_ms = t_loc_start.elapsed().as_millis() as u64;
    if let Some(ref mut rep) = benchmark_report {
        rep.localization_ms = loc_ms;
    }

    // 5. Apply dilation
    if args.dilate > 0 {
        mask = dilate_mask(&mask, args.dilate);
    }

    // 6. Save intermediate mask if requested
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

    // 7. Inpainting execution
    let result_img = if let Some(bbox) = compute_bounding_box(&mask, 16) {
        let (crop_x, crop_y, crop_w, crop_h) = bbox;
        let crop_patch = img.crop_imm(crop_x, crop_y, crop_w, crop_h);
        let crop_mask = imageops::crop_imm(&mask, crop_x, crop_y, crop_w, crop_h).to_image();

        let inpainted_patch = if args.fast {
            let classical = ClassicalInpainter::new();
            classical.inpaint(&crop_patch, &crop_mask, args.steps)?
        } else {
            let model_paths = model_mgr.ensure_models(args.model)?;
            #[cfg(feature = "erase")]
            let inpainter = CandleInpainter::load(&model_paths, args.model, &candle_dev)?;
            #[cfg(not(feature = "erase"))]
            let inpainter = CandleInpainter::load(&model_paths, args.model)?;

            inpainter.inpaint(
                &crop_patch,
                &crop_mask,
                args.replace.as_deref(),
                args.steps,
                benchmark_report.as_mut(),
            )?
        };

        let t_blend = Instant::now();
        let blended = InpaintPipeline::blend_result(&img, &inpainted_patch, &mask, bbox)?;
        let blend_ms = t_blend.elapsed().as_millis() as u64;

        if let Some(ref mut rep) = benchmark_report {
            rep.alpha_blend_ms = blend_ms;
            rep.total_ms = rep.localization_ms
                + rep.vae_encode_ms
                + rep.denoise_ms
                + rep.vae_decode_ms
                + rep.alpha_blend_ms;
        }

        blended
    } else {
        img
    };

    // 8. Save output
    result_img
        .save(&output_path)
        .map_err(|e| MartiniError::Io(std::io::Error::other(e)))?;

    // 9. Output status & benchmark reports
    if let (Some(rep), false) = (&benchmark_report, args.json) {
        rep.print_table(args.model.as_str(), &dev_name);
    }

    if args.json {
        let json_meta = serde_json::json!({
            "input": args.input.to_string_lossy(),
            "output": output_path.to_string_lossy(),
            "target": args.target,
            "replace": args.replace,
            "model": args.model.as_str(),
            "device": dev_name,
            "steps": args.steps,
            "success": true,
            "benchmark": benchmark_report,
        });
        println!("{}", serde_json::to_string_pretty(&json_meta).unwrap());
    } else {
        println!("✨ Object erasure complete!");
        println!("📄 Output saved to: {}", output_path.display());
    }

    Ok(())
}
