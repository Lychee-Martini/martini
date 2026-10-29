pub mod benchmark;
pub mod clip;
pub mod device;
pub mod mask;
pub mod models;
pub mod pipeline;

use crate::erase::benchmark::BenchmarkReport;
use crate::erase::clip::ClipGrounding;
use crate::erase::device::resolve_device_target;
use crate::erase::mask::{
    InpaintBatch, compute_inpaint_roi_masks, dilate_mask, plan_neural_inpaint_batches,
    resolution_scale,
};
use crate::erase::models::{InpaintModelPreset, ModelManager};
use crate::erase::pipeline::{
    CandleInpainter, blend_patch_in_place, inpaint_classical, scrub_foreign_masked_pixels,
};
use crate::error::MartiniError;
use clap::Args;
use image::{DynamicImage, GenericImageView, GrayImage, imageops};
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Args)]
pub struct EraseArgs {
    /// Path to the source image to edit
    #[arg(value_name = "INPUT", required_unless_present = "download_models")]
    pub input: Option<PathBuf>,

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
    if let Some(ref download_opt) = args.download_models {
        let model_mgr = ModelManager::new(args.model_dir.as_deref())?;
        let download_val = download_opt.as_deref();
        if download_val == Some("all") {
            for preset in [
                InpaintModelPreset::Sd15,
                InpaintModelPreset::Lcm,
                InpaintModelPreset::SdTurbo,
            ] {
                println!(
                    "🚀 Pre-downloading weights for preset '{}'...",
                    preset.as_str()
                );
                model_mgr.download_preset(preset)?;
            }
        } else {
            let target_preset = match download_val {
                Some("lcm") => InpaintModelPreset::Lcm,
                Some("sd-turbo") => InpaintModelPreset::SdTurbo,
                Some("sd-1.5") => InpaintModelPreset::Sd15,
                _ => args.model,
            };
            println!(
                "🚀 Pre-downloading weights for preset '{}'...",
                target_preset.as_str()
            );
            model_mgr.download_preset(target_preset)?;
        }
        println!("✅ Download complete.");
        return Ok(());
    }

    let input_path = args
        .input
        .as_ref()
        .ok_or_else(|| MartiniError::InvalidInputData {
            reason: "Input file must be provided unless --download-models is used".to_string(),
        })?;

    if args.target.is_none() && args.mask.is_none() {
        return Err(MartiniError::InvalidInputData {
            reason: "Either --target or --mask must be specified for erase".to_string(),
        });
    }

    if !input_path.exists() {
        return Err(MartiniError::InputFileNotFound {
            path: input_path.to_string_lossy().to_string(),
        });
    }

    // 1. Resolve output path
    let output_path = match &args.output {
        Some(p) => p.clone(),
        None => {
            let stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let ext = input_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!("{}_erased.{}", stem, ext))
        }
    };

    // 2. Load input image
    let img = image::open(input_path).map_err(|e| MartiniError::InvalidInputData {
        reason: format!(
            "Failed to read input image '{}': {}",
            input_path.display(),
            e
        ),
    })?;

    let t_run_start = Instant::now();
    let (img_w, img_h) = img.dimensions();
    let res_scale = resolution_scale(img_w, img_h);

    // 3. Resolve device & benchmark instrumentation
    let _dev_target = resolve_device_target(&args.device)?;
    #[cfg(feature = "nn-cpu")]
    let (candle_dev, dev_name) = crate::erase::device::select_candle_device(_dev_target);
    #[cfg(not(feature = "nn-cpu"))]
    let dev_name = "CPU".to_string();

    if !args.json {
        if args.fast {
            println!("⚡ Device: CPU (fast mode) | Image: {}x{}", img_w, img_h);
        } else {
            println!(
                "⚡ Device: {} | Model: {} | Image: {}x{}",
                dev_name,
                args.model.as_str(),
                img_w,
                img_h
            );
        }
    }

    let progress = if args.json {
        None
    } else {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        pb.set_message("Locating target mask...");
        Some(pb)
    };

    let mut benchmark_report = args.benchmark.then(BenchmarkReport::default);

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
            #[cfg(feature = "nn-cpu")]
            let clip_grounding = ClipGrounding::load(
                &model_paths.clip_weights,
                &model_paths.tokenizer,
                &candle_dev,
            )?;
            #[cfg(not(feature = "nn-cpu"))]
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

    // 5. Apply dilation (scaled proportionally with resolution when auto-detecting target)
    if args.dilate > 0 {
        let eff_dilate = if args.mask.is_some() {
            args.dilate
        } else {
            args.dilate.saturating_mul(res_scale)
        };
        mask = dilate_mask(&mask, eff_dilate);
    }

    // 6. Save intermediate mask if requested
    if args.save_mask {
        let mask_output_path = {
            let stem = input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("image");
            let parent = input_path.parent().unwrap_or_else(|| Path::new("."));
            parent.join(format!("{}_mask.png", stem))
        };
        mask.save(&mask_output_path)
            .map_err(|e| MartiniError::Io(std::io::Error::other(e)))?;
    }

    // 7. Inpainting execution across clustered local ROIs
    let mut working_img = img;
    let mut total_blend_ms = 0u64;

    if args.fast {
        let roi_masks = compute_inpaint_roi_masks(&mask, 0, 16 * res_scale);
        for (roi, roi_mask) in roi_masks {
            let (crop_x, crop_y, crop_w, crop_h) = roi;
            let crop_patch = working_img.crop_imm(crop_x, crop_y, crop_w, crop_h);
            let inpainted_patch = inpaint_classical(&crop_patch, &roi_mask, args.steps)?;
            let t_blend = Instant::now();
            blend_patch_in_place(&mut working_img, &inpainted_patch, &roi_mask, roi)?;
            total_blend_ms += t_blend.elapsed().as_millis() as u64;
        }
    } else {
        let batches = plan_neural_inpaint_batches(&mask, args.replace.is_none());
        if !batches.is_empty() {
            if let Some(ref pb) = progress {
                pb.set_message("Loading inpainting model...");
            }
            let model_paths = model_mgr.ensure_models(args.model)?;
            #[cfg(feature = "nn-cpu")]
            let inpainter = CandleInpainter::load(&model_paths, args.model, &candle_dev)?;
            #[cfg(not(feature = "nn-cpu"))]
            let inpainter = CandleInpainter::load(&model_paths, args.model)?;

            let eff_steps = if args.steps == 0 {
                args.model.default_steps()
            } else {
                args.steps
            };
            let total_passes = batches.len();
            if let Some(ref pb) = progress {
                pb.set_length((total_passes * eff_steps) as u64);
                pb.set_position(0);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "{spinner:.green} [{elapsed_precise}] [{bar:28.cyan/blue}] {pos}/{len} {msg}",
                        )
                        .unwrap()
                        .progress_chars("=>-"),
                );
            }

            const QUAD: u32 = 256;
            const OFFSETS: [(u32, u32); 4] = [(0, 0), (QUAD, 0), (0, QUAD), (QUAD, QUAD)];

            for (pass_idx, batch) in batches.into_iter().enumerate() {
                if let Some(ref pb) = progress {
                    pb.set_message(format!(
                        "Inpainting pass {}/{} on {}...",
                        pass_idx + 1,
                        total_passes,
                        dev_name
                    ));
                }

                match batch {
                    InpaintBatch::Single((roi, roi_mask)) => {
                        let (crop_x, crop_y, crop_w, crop_h) = roi;
                        let mut crop_rgb = working_img
                            .crop_imm(crop_x, crop_y, crop_w, crop_h)
                            .to_rgb8();
                        scrub_foreign_masked_pixels(
                            &mut crop_rgb,
                            &mask,
                            &roi_mask,
                            crop_x,
                            crop_y,
                        );
                        let inpainted_patch = inpainter.inpaint_with_progress(
                            &DynamicImage::ImageRgb8(crop_rgb),
                            &roi_mask,
                            args.replace.as_deref(),
                            args.steps,
                            benchmark_report.as_mut(),
                            progress.as_ref(),
                        )?;
                        let t_blend = Instant::now();
                        blend_patch_in_place(&mut working_img, &inpainted_patch, &roi_mask, roi)?;
                        total_blend_ms += t_blend.elapsed().as_millis() as u64;
                    }
                    InpaintBatch::Quadrants(rois) => {
                        let mut atlas_img =
                            image::RgbImage::from_pixel(512, 512, image::Rgb([128, 128, 128]));
                        let mut atlas_mask = GrayImage::new(512, 512);

                        for (idx, &((crop_x, crop_y, crop_w, crop_h), ref roi_mask)) in
                            rois.iter().enumerate()
                        {
                            let (ox, oy) = OFFSETS[idx];
                            let mut crop_rgb = working_img
                                .crop_imm(crop_x, crop_y, crop_w, crop_h)
                                .to_rgb8();
                            scrub_foreign_masked_pixels(
                                &mut crop_rgb,
                                &mask,
                                roi_mask,
                                crop_x,
                                crop_y,
                            );
                            let tile_rgb = if (crop_w, crop_h) == (QUAD, QUAD) {
                                crop_rgb
                            } else {
                                imageops::resize(
                                    &crop_rgb,
                                    QUAD,
                                    QUAD,
                                    imageops::FilterType::Lanczos3,
                                )
                            };
                            let tile_mask = if (crop_w, crop_h) == (QUAD, QUAD) {
                                roi_mask.clone()
                            } else {
                                imageops::resize(
                                    roi_mask,
                                    QUAD,
                                    QUAD,
                                    imageops::FilterType::Triangle,
                                )
                            };
                            imageops::replace(&mut atlas_img, &tile_rgb, ox as i64, oy as i64);
                            imageops::replace(&mut atlas_mask, &tile_mask, ox as i64, oy as i64);
                        }

                        let inpainted_atlas = inpainter.inpaint_with_progress(
                            &DynamicImage::ImageRgb8(atlas_img),
                            &atlas_mask,
                            None,
                            args.steps,
                            benchmark_report.as_mut(),
                            progress.as_ref(),
                        )?;

                        for (idx, (roi, roi_mask)) in rois.into_iter().enumerate() {
                            let (ox, oy) = OFFSETS[idx];
                            let (_, _, crop_w, crop_h) = roi;
                            let quad_out = inpainted_atlas.crop_imm(ox, oy, QUAD, QUAD);
                            let patch_out = if (crop_w, crop_h) == (QUAD, QUAD) {
                                quad_out
                            } else {
                                quad_out.resize_exact(
                                    crop_w,
                                    crop_h,
                                    imageops::FilterType::Lanczos3,
                                )
                            };
                            let t_blend = Instant::now();
                            blend_patch_in_place(&mut working_img, &patch_out, &roi_mask, roi)?;
                            total_blend_ms += t_blend.elapsed().as_millis() as u64;
                        }
                    }
                }
            }
        }
    }

    if let Some(ref mut rep) = benchmark_report {
        rep.alpha_blend_ms = total_blend_ms;
        rep.total_ms = rep.localization_ms
            + rep.vae_encode_ms
            + rep.denoise_ms
            + rep.vae_decode_ms
            + rep.alpha_blend_ms;
    }

    if let Some(ref pb) = progress {
        pb.set_message("Saving output image...");
    }

    // 8. Save output
    working_img
        .save(&output_path)
        .map_err(|e| MartiniError::Io(std::io::Error::other(e)))?;

    if let Some(pb) = progress {
        pb.finish_and_clear();
    }

    // 9. Output status & benchmark reports
    if let (Some(rep), false) = (&benchmark_report, args.json) {
        rep.print_table(args.model.as_str(), &dev_name);
    }

    if args.json {
        let json_meta = serde_json::json!({
            "input": input_path.to_string_lossy(),
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
        println!(
            "✨ Object erasure complete! ({:.1}s)",
            t_run_start.elapsed().as_secs_f64()
        );
        println!("📄 Output saved to: {}", output_path.display());
    }

    Ok(())
}
