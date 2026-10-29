use crate::erase::benchmark::BenchmarkReport;
use crate::erase::mask::{feather_mask, resolution_scale};
use crate::erase::models::{InpaintModelPreset, ModelPaths};
use crate::error::MartiniError;
use image::{DynamicImage, GrayImage, Rgba, imageops};
use indicatif::ProgressBar;
#[cfg(feature = "nn-cpu")]
use std::time::Instant;

#[cfg(feature = "nn-cpu")]
use candle_core::{DType, Device, Tensor};
#[cfg(feature = "nn-cpu")]
use candle_nn::Module;
#[cfg(feature = "nn-cpu")]
use candle_transformers::models::stable_diffusion::{
    self, StableDiffusionConfig,
    ddim::DDIMSchedulerConfig,
    schedulers::{PredictionType, SchedulerConfig, TimestepSpacing},
};

/// Classical heuristic inpainter using Telea Fast Marching propagation and Laplacian harmonic smoothing
pub fn inpaint_classical(
    image: &DynamicImage,
    mask: &GrayImage,
    steps: usize,
) -> Result<DynamicImage, MartiniError> {
    let (width, height) = mask.dimensions();
    if image.width() != width || image.height() != height {
        return Err(MartiniError::InvalidInputData {
            reason: format!(
                "Inpaint dimension mismatch: image is {}x{}, but mask is {}x{}",
                image.width(),
                image.height(),
                width,
                height
            ),
        });
    }

    let mut rgba = image.to_rgba8();
    let total_pixels = (width * height) as usize;

    // Check if there are any masked pixels (> 0)
    let mut masked_indices = Vec::new();
    for y in 0..height {
        for x in 0..width {
            if mask.get_pixel(x, y)[0] > 0 {
                masked_indices.push((x, y));
            }
        }
    }

    if masked_indices.is_empty() {
        return Ok(image.clone());
    }

    // Distance transform from unmasked (known) pixels
    let mut dist = vec![f32::INFINITY; total_pixels];
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if mask.get_pixel(x, y)[0] == 0 {
                dist[idx] = 0.0;
            }
        }
    }

    // Forward pass
    let sqrt2 = std::f32::consts::SQRT_2;
    let w_usize = width as usize;
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let mut d = dist[idx];
            if d > 0.0 {
                if x > 0 {
                    d = d.min(dist[idx - 1] + 1.0);
                }
                if y > 0 {
                    d = d.min(dist[idx - w_usize] + 1.0);
                    if x > 0 {
                        d = d.min(dist[idx - w_usize - 1] + sqrt2);
                    }
                    if x + 1 < width {
                        d = d.min(dist[idx - w_usize + 1] + sqrt2);
                    }
                }
                dist[idx] = d;
            }
        }
    }

    // Backward pass
    for y in (0..height).rev() {
        for x in (0..width).rev() {
            let idx = (y * width + x) as usize;
            let mut d = dist[idx];
            if d > 0.0 {
                if x + 1 < width {
                    d = d.min(dist[idx + 1] + 1.0);
                }
                if y + 1 < height {
                    d = d.min(dist[idx + w_usize] + 1.0);
                    if x + 1 < width {
                        d = d.min(dist[idx + w_usize + 1] + sqrt2);
                    }
                    if x > 0 {
                        d = d.min(dist[idx + w_usize - 1] + sqrt2);
                    }
                }
                dist[idx] = d;
            }
        }
    }

    // Sort masked pixels in ascending order of distance from boundary
    masked_indices.sort_unstable_by(|&(x1, y1), &(x2, y2)| {
        let d1 = dist[(y1 * width + x1) as usize];
        let d2 = dist[(y2 * width + x2) as usize];
        d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
    });

    // Fast Marching Inpainting (Telea-style propagation)
    let radius = 5i32;
    let mut known = vec![false; total_pixels];
    for y in 0..height {
        for x in 0..width {
            if mask.get_pixel(x, y)[0] == 0 {
                known[(y * width + x) as usize] = true;
            }
        }
    }

    for &(x, y) in &masked_indices {
        let p_idx = (y * width + x) as usize;
        let d_p = dist[p_idx];

        let mut sum_r = 0.0f32;
        let mut sum_g = 0.0f32;
        let mut sum_b = 0.0f32;
        let mut sum_a = 0.0f32;
        let mut sum_w = 0.0f32;

        let y_min = (y as i32 - radius).max(0) as u32;
        let y_max = (y as i32 + radius).min(height as i32 - 1) as u32;
        let x_min = (x as i32 - radius).max(0) as u32;
        let x_max = (x as i32 + radius).min(width as i32 - 1) as u32;

        for qy in y_min..=y_max {
            for qx in x_min..=x_max {
                let q_idx = (qy * width + qx) as usize;
                if known[q_idx] {
                    let dx = x as f32 - qx as f32;
                    let dy = y as f32 - qy as f32;
                    let d_sq = dx * dx + dy * dy;
                    if d_sq > 0.0 && d_sq <= (radius * radius) as f32 {
                        let d_q = dist[q_idx];
                        let w = (1.0 / d_sq) / (1.0 + (d_p - d_q).abs());
                        let q_px = rgba.get_pixel(qx, qy);
                        sum_r += w * q_px[0] as f32;
                        sum_g += w * q_px[1] as f32;
                        sum_b += w * q_px[2] as f32;
                        sum_a += w * q_px[3] as f32;
                        sum_w += w;
                    }
                }
            }
        }

        if sum_w > 0.0 {
            let r = (sum_r / sum_w).round() as u8;
            let g = (sum_g / sum_w).round() as u8;
            let b = (sum_b / sum_w).round() as u8;
            let a = (sum_a / sum_w).round() as u8;
            rgba.put_pixel(x, y, Rgba([r, g, b, a]));
        }
        known[p_idx] = true;
    }

    // Harmonic Relaxation (Laplacian smoothing passes)
    let num_smooth_steps = steps.max(20);
    let mut buffer = rgba.clone();

    for _ in 0..num_smooth_steps {
        for &(x, y) in &masked_indices {
            let mut sum_r = 0.0f32;
            let mut sum_g = 0.0f32;
            let mut sum_b = 0.0f32;
            let mut sum_a = 0.0f32;
            let mut count = 0.0f32;

            if x > 0 {
                let px = rgba.get_pixel(x - 1, y);
                sum_r += px[0] as f32;
                sum_g += px[1] as f32;
                sum_b += px[2] as f32;
                sum_a += px[3] as f32;
                count += 1.0;
            }
            if x + 1 < width {
                let px = rgba.get_pixel(x + 1, y);
                sum_r += px[0] as f32;
                sum_g += px[1] as f32;
                sum_b += px[2] as f32;
                sum_a += px[3] as f32;
                count += 1.0;
            }
            if y > 0 {
                let px = rgba.get_pixel(x, y - 1);
                sum_r += px[0] as f32;
                sum_g += px[1] as f32;
                sum_b += px[2] as f32;
                sum_a += px[3] as f32;
                count += 1.0;
            }
            if y + 1 < height {
                let px = rgba.get_pixel(x, y + 1);
                sum_r += px[0] as f32;
                sum_g += px[1] as f32;
                sum_b += px[2] as f32;
                sum_a += px[3] as f32;
                count += 1.0;
            }

            if count > 0.0 {
                buffer.put_pixel(
                    x,
                    y,
                    Rgba([
                        (sum_r / count).round() as u8,
                        (sum_g / count).round() as u8,
                        (sum_b / count).round() as u8,
                        (sum_a / count).round() as u8,
                    ]),
                );
            }
        }
        std::mem::swap(&mut rgba, &mut buffer);
    }

    Ok(DynamicImage::ImageRgba8(rgba))
}

/// Replaces pixels belonging to other (foreign) mask clusters inside a local context crop
/// with the crop's unmasked background mean so neighboring text never contaminates diffusion context.
pub(crate) fn scrub_foreign_masked_pixels(
    patch: &mut image::RgbImage,
    full_mask: &GrayImage,
    local_mask: &GrayImage,
    crop_x: u32,
    crop_y: u32,
) {
    let (pw, ph) = patch.dimensions();
    let (fw, fh) = full_mask.dimensions();
    let mut has_foreign = false;

    'scan: for py in 0..ph {
        let fy = crop_y + py;
        if fy >= fh {
            break;
        }
        for px in 0..pw {
            let fx = crop_x + px;
            if fx < fw && full_mask.get_pixel(fx, fy)[0] > 0 && local_mask.get_pixel(px, py)[0] == 0
            {
                has_foreign = true;
                break 'scan;
            }
        }
    }

    if !has_foreign {
        return;
    }

    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;
    for py in 0..ph {
        let fy = crop_y + py;
        for px in 0..pw {
            let fx = crop_x + px;
            if fx < fw && fy < fh && full_mask.get_pixel(fx, fy)[0] == 0 {
                let p = patch.get_pixel(px, py);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
        }
    }

    if count == 0 {
        return;
    }
    let fill = image::Rgb([
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
    ]);

    for py in 0..ph {
        let fy = crop_y + py;
        if fy >= fh {
            break;
        }
        for px in 0..pw {
            let fx = crop_x + px;
            if fx < fw && full_mask.get_pixel(fx, fy)[0] > 0 && local_mask.get_pixel(px, py)[0] == 0
            {
                patch.put_pixel(px, py, fill);
            }
        }
    }
}

pub fn blend_patch_in_place(
    target: &mut DynamicImage,
    inpainted_patch: &DynamicImage,
    mask: &GrayImage,
    crop: (u32, u32, u32, u32),
) -> Result<(), MartiniError> {
    let (crop_x, crop_y, crop_w, crop_h) = crop;
    let (img_w, img_h) = (target.width(), target.height());
    let feather_r = (resolution_scale(img_w, img_h) * 2).clamp(2, 10);

    // Support both full-image masks and crop-local ROI masks while only feathering the local crop
    let (feathered, offset_x, offset_y) = if mask.dimensions() == (img_w, img_h) {
        let sub_x = crop_x.saturating_sub(feather_r);
        let sub_y = crop_y.saturating_sub(feather_r);
        let sub_end_x = (crop_x + crop_w + feather_r).min(mask.width());
        let sub_end_y = (crop_y + crop_h + feather_r).min(mask.height());
        let sub = imageops::crop_imm(
            mask,
            sub_x,
            sub_y,
            sub_end_x.saturating_sub(sub_x),
            sub_end_y.saturating_sub(sub_y),
        )
        .to_image();
        (
            feather_mask(&sub, feather_r),
            crop_x - sub_x,
            crop_y - sub_y,
        )
    } else {
        (feather_mask(mask, feather_r), 0, 0)
    };

    if !target.color().has_alpha() {
        let patch_rgb = inpainted_patch.to_rgb8();
        if !matches!(target, DynamicImage::ImageRgb8(_)) {
            *target = DynamicImage::ImageRgb8(target.to_rgb8());
        }
        let Some(result_rgb) = target.as_mut_rgb8() else {
            return Ok(());
        };

        for py in 0..crop_h {
            for px in 0..crop_w {
                let ox = crop_x + px;
                let oy = crop_y + py;
                let mx = px + offset_x;
                let my = py + offset_y;

                if ox < img_w
                    && oy < img_h
                    && px < patch_rgb.width()
                    && py < patch_rgb.height()
                    && mx < feathered.width()
                    && my < feathered.height()
                {
                    let mask_val = feathered.get_pixel(mx, my)[0] as f32 / 255.0;
                    if mask_val > 0.0 {
                        let orig_px = result_rgb.get_pixel(ox, oy);
                        let patch_px = patch_rgb.get_pixel(px, py);
                        let r = ((1.0 - mask_val) * orig_px[0] as f32
                            + mask_val * patch_px[0] as f32)
                            .round() as u8;
                        let g = ((1.0 - mask_val) * orig_px[1] as f32
                            + mask_val * patch_px[1] as f32)
                            .round() as u8;
                        let b = ((1.0 - mask_val) * orig_px[2] as f32
                            + mask_val * patch_px[2] as f32)
                            .round() as u8;
                        result_rgb.put_pixel(ox, oy, image::Rgb([r, g, b]));
                    }
                }
            }
        }
        return Ok(());
    }

    let patch_rgba = inpainted_patch.to_rgba8();
    if !matches!(target, DynamicImage::ImageRgba8(_)) {
        *target = DynamicImage::ImageRgba8(target.to_rgba8());
    }
    let Some(result) = target.as_mut_rgba8() else {
        return Ok(());
    };

    for py in 0..crop_h {
        for px in 0..crop_w {
            let ox = crop_x + px;
            let oy = crop_y + py;
            let mx = px + offset_x;
            let my = py + offset_y;

            if ox < img_w
                && oy < img_h
                && px < patch_rgba.width()
                && py < patch_rgba.height()
                && mx < feathered.width()
                && my < feathered.height()
            {
                let mask_val = feathered.get_pixel(mx, my)[0] as f32 / 255.0;

                if mask_val > 0.0 {
                    let orig_px = result.get_pixel(ox, oy);
                    let patch_px = patch_rgba.get_pixel(px, py);

                    let r = ((1.0 - mask_val) * orig_px[0] as f32 + mask_val * patch_px[0] as f32)
                        .round() as u8;
                    let g = ((1.0 - mask_val) * orig_px[1] as f32 + mask_val * patch_px[1] as f32)
                        .round() as u8;
                    let b = ((1.0 - mask_val) * orig_px[2] as f32 + mask_val * patch_px[2] as f32)
                        .round() as u8;
                    let a = ((1.0 - mask_val) * orig_px[3] as f32 + mask_val * patch_px[3] as f32)
                        .round() as u8;

                    result.put_pixel(ox, oy, Rgba([r, g, b, a]));
                }
            }
        }
    }

    Ok(())
}

pub fn blend_patch(
    original: &DynamicImage,
    inpainted_patch: &DynamicImage,
    mask: &GrayImage,
    crop: (u32, u32, u32, u32),
) -> Result<DynamicImage, MartiniError> {
    let mut out = original.clone();
    blend_patch_in_place(&mut out, inpainted_patch, mask, crop)?;
    Ok(out)
}

/// Neural diffusion inpainter using Hugging Face Candle (Stable Diffusion Inpainting)
pub struct CandleInpainter {
    pub preset: InpaintModelPreset,
    #[cfg(feature = "nn-cpu")]
    pub device: Device,
    #[cfg(feature = "nn-cpu")]
    pub tokenizer: tokenizers::Tokenizer,
    #[cfg(feature = "nn-cpu")]
    sd_config: StableDiffusionConfig,
    #[cfg(feature = "nn-cpu")]
    vae: stable_diffusion::vae::AutoEncoderKL,
    #[cfg(feature = "nn-cpu")]
    text_encoder: stable_diffusion::clip::ClipTextTransformer,
    #[cfg(feature = "nn-cpu")]
    unet: stable_diffusion::unet_2d::UNet2DConditionModel,
    #[cfg(feature = "nn-cpu")]
    unet_in_channels: usize,
    #[cfg(feature = "nn-cpu")]
    unet_dtype: DType,
}

impl CandleInpainter {
    #[cfg(feature = "nn-cpu")]
    pub fn load(
        paths: &ModelPaths,
        preset: InpaintModelPreset,
        device: &Device,
    ) -> Result<Self, MartiniError> {
        let tokenizer = tokenizers::Tokenizer::from_file(&paths.tokenizer).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to load CLIP tokenizer for inpainting: {}", e),
            }
        })?;

        let sd_config = match preset {
            InpaintModelPreset::Sd15 | InpaintModelPreset::Lcm => {
                StableDiffusionConfig::v1_5(None, Some(512), Some(512))
            }
            InpaintModelPreset::SdTurbo => StableDiffusionConfig::v2_1(None, Some(512), Some(512)),
        };

        // Load VAE and text encoder in F32 to avoid FP16 logvar exp() overflow
        let vae = sd_config
            .build_vae(&paths.vae_weights, device, DType::F32)
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!(
                    "Failed to load VAE weights '{}': {}",
                    paths.vae_weights.display(),
                    e
                ),
            })?;

        let text_encoder = stable_diffusion::build_clip_transformer(
            &sd_config.clip,
            &paths.text_encoder_weights,
            device,
            DType::F32,
        )
        .map_err(|e| MartiniError::InvalidInputData {
            reason: format!(
                "Failed to load text encoder weights '{}': {}",
                paths.text_encoder_weights.display(),
                e
            ),
        })?;

        let unet_dtype = if device.is_cuda() {
            DType::F16
        } else {
            DType::F32
        };

        // Prefer 9-channel dedicated inpainting UNet; fall back to 4-channel UNet (e.g. LCM / SD-Turbo)
        let (unet, unet_in_channels) =
            match sd_config.build_unet(&paths.unet_weights, device, 9, false, unet_dtype) {
                Ok(u) => (u, 9),
                Err(_) => {
                    let u = sd_config
                        .build_unet(&paths.unet_weights, device, 4, false, unet_dtype)
                        .map_err(|e| MartiniError::InvalidInputData {
                            reason: format!(
                                "Failed to load UNet weights '{}': {}",
                                paths.unet_weights.display(),
                                e
                            ),
                        })?;
                    (u, 4)
                }
            };

        Ok(Self {
            preset,
            device: device.clone(),
            tokenizer,
            sd_config,
            vae,
            text_encoder,
            unet,
            unet_in_channels,
            unet_dtype,
        })
    }

    #[cfg(not(feature = "nn-cpu"))]
    pub fn load(_paths: &ModelPaths, preset: InpaintModelPreset) -> Result<Self, MartiniError> {
        Ok(Self { preset })
    }

    #[cfg(feature = "nn-cpu")]
    fn encode_sd_prompt(&self, prompt_text: &str) -> Result<Tensor, MartiniError> {
        const EOS_ID: u32 = 49407;
        let max_len = self.sd_config.clip.max_position_embeddings;
        let pad_id = match &self.sd_config.clip.pad_with {
            Some(pad_str) => *self
                .tokenizer
                .get_vocab(true)
                .get(pad_str.as_str())
                .unwrap_or(&EOS_ID),
            None => EOS_ID,
        };

        let enc = self.tokenizer.encode(prompt_text, true).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to tokenize inpaint prompt '{}': {}", prompt_text, e),
            }
        })?;
        let mut ids = enc.get_ids().to_vec();
        if ids.len() > max_len {
            ids.truncate(max_len);
            ids[max_len - 1] = EOS_ID;
        } else {
            ids.resize(max_len, pad_id);
        }

        let token_tensor = Tensor::new(ids.as_slice(), &self.device)
            .and_then(|t| t.unsqueeze(0))
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to build prompt token tensor: {}", e),
            })?;

        self.text_encoder
            .forward(&token_tensor)
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("SD text encoder forward failed: {}", e),
            })
    }

    pub fn inpaint(
        &self,
        image: &DynamicImage,
        mask: &GrayImage,
        prompt: Option<&str>,
        steps: usize,
        reporter: Option<&mut BenchmarkReport>,
    ) -> Result<DynamicImage, MartiniError> {
        self.inpaint_with_progress(image, mask, prompt, steps, reporter, None)
    }

    pub fn inpaint_with_progress(
        &self,
        image: &DynamicImage,
        mask: &GrayImage,
        prompt: Option<&str>,
        steps: usize,
        reporter: Option<&mut BenchmarkReport>,
        progress: Option<&ProgressBar>,
    ) -> Result<DynamicImage, MartiniError> {
        #[cfg(feature = "nn-cpu")]
        {
            const SD_SIZE: u32 = 512;
            const LATENT_SIZE: usize = (SD_SIZE / 8) as usize;
            const VAE_SCALE: f64 = 0.18215;

            let (orig_w, orig_h) = (image.width(), image.height());
            if mask.dimensions() != (orig_w, orig_h) {
                return Err(MartiniError::InvalidInputData {
                    reason: format!(
                        "Inpaint dimension mismatch: image is {}x{}, mask is {}x{}",
                        orig_w,
                        orig_h,
                        mask.width(),
                        mask.height()
                    ),
                });
            }

            // Skip diffusion if mask has no positive pixels
            if !mask.pixels().any(|p| p[0] > 0) {
                return Ok(image.clone());
            }

            let num_steps = if steps == 0 {
                self.preset.default_steps()
            } else {
                steps
            };

            let guidance_scale = match self.preset {
                InpaintModelPreset::Sd15 => 7.5f64,
                InpaintModelPreset::Lcm => 1.5f64,
                InpaintModelPreset::SdTurbo => 1.0f64,
            };
            let use_cfg = guidance_scale > 1.0;

            let (pos_prompt, neg_prompt) = match prompt {
                Some(p) if !p.trim().is_empty() => (
                    p.trim(),
                    "blurry, low quality, artifact, watermark, text, distorted",
                ),
                _ => (
                    "clean plain background surface texture, seamless natural continuation, high quality",
                    "text, watermark, letters, numbers, digits, characters, logo, writing, signature, stamp, blurry, artifact",
                ),
            };

            // 1. Prompt & VAE Encoding
            let t_vae_enc = Instant::now();

            let cond_emb = self.encode_sd_prompt(pos_prompt)?;
            let text_embeddings = if use_cfg {
                let uncond_emb = self.encode_sd_prompt(neg_prompt)?;
                Tensor::cat(&[&uncond_emb, &cond_emb], 0)
            } else {
                Ok(cond_emb)
            }
            .and_then(|t| t.to_dtype(self.unet_dtype))
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to prepare text embeddings: {}", e),
            })?;

            let resized_img = if (orig_w, orig_h) == (SD_SIZE, SD_SIZE) {
                image.to_rgb8()
            } else {
                image
                    .resize_exact(SD_SIZE, SD_SIZE, imageops::FilterType::Lanczos3)
                    .to_rgb8()
            };
            let resized_mask = if (orig_w, orig_h) == (SD_SIZE, SD_SIZE) {
                mask.clone()
            } else {
                imageops::resize(mask, SD_SIZE, SD_SIZE, imageops::FilterType::Triangle)
            };

            // Build 64x64 latent mask via 8x8 max-pooling so thin text strokes never vanish between grid centers,
            // and align the 512x512 masked_image zeroing to the exact 8x8 latent mask cells.
            let mut latent_mask_vec = vec![0.0f32; LATENT_SIZE * LATENT_SIZE];
            for ly in 0..LATENT_SIZE {
                for lx in 0..LATENT_SIZE {
                    let mut any_masked = false;
                    for dy in 0..8u32 {
                        for dx in 0..8u32 {
                            if resized_mask.get_pixel(lx as u32 * 8 + dx, ly as u32 * 8 + dy)[0]
                                > 64
                            {
                                any_masked = true;
                                break;
                            }
                        }
                        if any_masked {
                            break;
                        }
                    }
                    if any_masked {
                        latent_mask_vec[ly * LATENT_SIZE + lx] = 1.0;
                    }
                }
            }

            let plane = (SD_SIZE * SD_SIZE) as usize;
            let mut full_img_data = vec![0.0f32; 3 * plane];
            let mut masked_img_data = vec![0.0f32; 3 * plane];

            for y in 0..SD_SIZE {
                for x in 0..SD_SIZE {
                    let idx = (y * SD_SIZE + x) as usize;
                    let px = resized_img.get_pixel(x, y);
                    let r = (px[0] as f32 / 127.5) - 1.0;
                    let g = (px[1] as f32 / 127.5) - 1.0;
                    let b = (px[2] as f32 / 127.5) - 1.0;

                    full_img_data[idx] = r;
                    full_img_data[plane + idx] = g;
                    full_img_data[2 * plane + idx] = b;

                    let is_masked_cell =
                        latent_mask_vec[(y as usize / 8) * LATENT_SIZE + (x as usize / 8)] > 0.5;
                    if !is_masked_cell {
                        masked_img_data[idx] = r;
                        masked_img_data[plane + idx] = g;
                        masked_img_data[2 * plane + idx] = b;
                    }
                }
            }

            let masked_img_tensor = Tensor::from_vec(
                masked_img_data,
                (1, 3, SD_SIZE as usize, SD_SIZE as usize),
                &self.device,
            )
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to create masked image tensor: {}", e),
            })?;

            let masked_latents = self
                .vae
                .encode(&masked_img_tensor)
                .and_then(|dist| dist.sample())
                .and_then(|s| s * VAE_SCALE)
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("VAE encode failed: {}", e),
                })?;

            let orig_latents = if self.unet_in_channels == 4 {
                let full_tensor = Tensor::from_vec(
                    full_img_data,
                    (1, 3, SD_SIZE as usize, SD_SIZE as usize),
                    &self.device,
                )
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to create full image tensor: {}", e),
                })?;
                Some(
                    self.vae
                        .encode(&full_tensor)
                        .and_then(|dist| dist.sample())
                        .and_then(|s| s * VAE_SCALE)
                        .map_err(|e| MartiniError::InvalidInputData {
                            reason: format!("VAE encode (full) failed: {}", e),
                        })?,
                )
            } else {
                None
            };

            let masked_cells: Vec<bool> = latent_mask_vec.iter().map(|&v| v > 0.5).collect();
            let inv_latent_mask_vec: Vec<f32> = latent_mask_vec.iter().map(|&v| 1.0 - v).collect();
            let latent_mask_single = Tensor::from_vec(
                latent_mask_vec,
                (1, 1, LATENT_SIZE, LATENT_SIZE),
                &self.device,
            )
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to create latent mask tensor: {}", e),
            })?;
            let inv_latent_mask_single = Tensor::from_vec(
                inv_latent_mask_vec,
                (1, 1, LATENT_SIZE, LATENT_SIZE),
                &self.device,
            )
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to create inverse latent mask tensor: {}", e),
            })?;

            let latent_mask_input = if use_cfg {
                Tensor::cat(&[&latent_mask_single, &latent_mask_single], 0)
            } else {
                Ok(latent_mask_single.clone())
            }
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to batch latent mask: {}", e),
            })?;

            let masked_latents_input = if use_cfg {
                Tensor::cat(&[&masked_latents, &masked_latents], 0)
            } else {
                Ok(masked_latents.clone())
            }
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to batch masked latents: {}", e),
            })?;

            let vae_enc_ms = t_vae_enc.elapsed().as_millis() as u64;

            // 2. UNet Denoising Loop
            let t_denoise = Instant::now();

            let mut scheduler = match self.preset {
                InpaintModelPreset::Sd15 => self.sd_config.build_scheduler(num_steps),
                InpaintModelPreset::Lcm | InpaintModelPreset::SdTurbo => DDIMSchedulerConfig {
                    prediction_type: PredictionType::Epsilon,
                    timestep_spacing: TimestepSpacing::Trailing,
                    ..Default::default()
                }
                .build(num_steps),
            }
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to build diffusion scheduler: {}", e),
            })?;

            let timesteps = scheduler.timesteps().to_vec();
            let mut latents =
                Tensor::randn(0f32, 1f32, (1, 4, LATENT_SIZE, LATENT_SIZE), &self.device)
                    .and_then(|t| t * scheduler.init_noise_sigma())
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Failed to initialize latent noise: {}", e),
                    })?;

            for (step_idx, &timestep) in timesteps.iter().enumerate() {
                let latent_model_input = if use_cfg {
                    Tensor::cat(&[&latents, &latents], 0)
                } else {
                    Ok(latents.clone())
                }
                .and_then(|t| scheduler.scale_model_input(t, timestep))
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to scale latent input: {}", e),
                })?;

                let unet_input = if self.unet_in_channels == 9 {
                    Tensor::cat(
                        &[
                            &latent_model_input,
                            &latent_mask_input,
                            &masked_latents_input,
                        ],
                        1,
                    )
                } else {
                    Ok(latent_model_input)
                }
                .and_then(|t| t.to_dtype(self.unet_dtype))
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to assemble UNet input tensor: {}", e),
                })?;

                let noise_pred = self
                    .unet
                    .forward(&unet_input, timestep as f64, &text_embeddings)
                    .and_then(|t| t.to_dtype(DType::F32))
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("UNet forward failed at step {}: {}", step_idx, e),
                    })?;

                let guided_noise = if use_cfg {
                    let chunks =
                        noise_pred
                            .chunk(2, 0)
                            .map_err(|e| MartiniError::InvalidInputData {
                                reason: format!("Failed to split CFG noise prediction: {}", e),
                            })?;
                    let (uncond, cond) = (&chunks[0], &chunks[1]);
                    (uncond + ((cond - uncond).and_then(|d| d * guidance_scale))).map_err(|e| {
                        MartiniError::InvalidInputData {
                            reason: format!("CFG combination failed: {}", e),
                        }
                    })?
                } else {
                    noise_pred
                };

                latents = scheduler
                    .step(&guided_noise, timestep, &latents)
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Scheduler step failed: {}", e),
                    })?;

                // RePaint latent preservation for 4-channel fallback models
                if let Some(ref orig_lat) = orig_latents {
                    latents = if step_idx + 1 < timesteps.len() {
                        let next_t = timesteps[step_idx + 1];
                        let noise = orig_lat.randn_like(0.0, 1.0).map_err(|e| {
                            MartiniError::InvalidInputData {
                                reason: format!("Failed to sample RePaint noise: {}", e),
                            }
                        })?;
                        let noisy_orig =
                            scheduler.add_noise(orig_lat, noise, next_t).map_err(|e| {
                                MartiniError::InvalidInputData {
                                    reason: format!("Failed to noise original latents: {}", e),
                                }
                            })?;
                        ((&latents * &latent_mask_single).and_then(|a| {
                            (&noisy_orig * &inv_latent_mask_single).and_then(|b| a + b)
                        }))
                        .map_err(|e| MartiniError::InvalidInputData {
                            reason: format!("RePaint latent blend failed: {}", e),
                        })?
                    } else {
                        ((&latents * &latent_mask_single)
                            .and_then(|a| (orig_lat * &inv_latent_mask_single).and_then(|b| a + b)))
                        .map_err(|e| MartiniError::InvalidInputData {
                            reason: format!("Final RePaint latent blend failed: {}", e),
                        })?
                    };
                }

                if let Some(pb) = progress {
                    pb.inc(1);
                }
            }

            let denoise_ms = t_denoise.elapsed().as_millis() as u64;

            // 3. VAE Decoding
            let t_vae_dec = Instant::now();
            let decoded = self
                .vae
                .decode(
                    &(&latents / VAE_SCALE).map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Latent unscale failed: {}", e),
                    })?,
                )
                .and_then(|img_t| ((img_t / 2.0)? + 0.5)?.clamp(0.0, 1.0))
                .and_then(|img_t| img_t.squeeze(0)?.to_dtype(DType::F32)?.to_vec3::<f32>())
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("VAE decode failed: {}", e),
                })?;

            let mut out_rgb = image::RgbImage::new(SD_SIZE, SD_SIZE);
            for y in 0..SD_SIZE {
                for x in 0..SD_SIZE {
                    let r = (decoded[0][y as usize][x as usize] * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    let g = (decoded[1][y as usize][x as usize] * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    let b = (decoded[2][y as usize][x as usize] * 255.0)
                        .round()
                        .clamp(0.0, 255.0) as u8;
                    out_rgb.put_pixel(x, y, image::Rgb([r, g, b]));
                }
            }

            // Calibrate per-cluster local RGB exposure bias against surrounding unmasked pixels for seamless erasure
            if prompt.is_none_or(|p| p.trim().is_empty()) {
                let clusters = crate::erase::mask::extract_component_clusters(&resized_mask, 16);
                let mut calibrated = vec![false; (SD_SIZE * SD_SIZE) as usize];
                for cluster in clusters {
                    let (bx, by, bw, bh) = cluster.bbox;
                    let x0 = bx.saturating_sub(8);
                    let y0 = by.saturating_sub(8);
                    let x1 = (bx + bw + 8).min(SD_SIZE);
                    let y1 = (by + bh + 8).min(SD_SIZE);

                    let mut diff_r = 0.0f32;
                    let mut diff_g = 0.0f32;
                    let mut diff_b = 0.0f32;
                    let mut count = 0.0f32;

                    for cy in y0..y1 {
                        for cx in x0..x1 {
                            let cell_idx = (cy as usize / 8) * LATENT_SIZE + (cx as usize / 8);
                            if masked_cells[cell_idx] && resized_mask.get_pixel(cx, cy)[0] == 0 {
                                let o = resized_img.get_pixel(cx, cy);
                                let p = out_rgb.get_pixel(cx, cy);
                                diff_r += o[0] as f32 - p[0] as f32;
                                diff_g += o[1] as f32 - p[1] as f32;
                                diff_b += o[2] as f32 - p[2] as f32;
                                count += 1.0;
                            }
                        }
                    }

                    if count < 16.0 {
                        for cy in y0..y1 {
                            for cx in x0..x1 {
                                if resized_mask.get_pixel(cx, cy)[0] == 0 {
                                    let o = resized_img.get_pixel(cx, cy);
                                    let p = out_rgb.get_pixel(cx, cy);
                                    diff_r += o[0] as f32 - p[0] as f32;
                                    diff_g += o[1] as f32 - p[1] as f32;
                                    diff_b += o[2] as f32 - p[2] as f32;
                                    count += 1.0;
                                }
                            }
                        }
                    }

                    if count > 0.0 {
                        let dr = (diff_r / count).clamp(-32.0, 32.0);
                        let dg = (diff_g / count).clamp(-32.0, 32.0);
                        let db = (diff_b / count).clamp(-32.0, 32.0);
                        for cy in y0..y1 {
                            for cx in x0..x1 {
                                let idx = (cy * SD_SIZE + cx) as usize;
                                if !calibrated[idx] {
                                    calibrated[idx] = true;
                                    let p = out_rgb.get_pixel(cx, cy);
                                    out_rgb.put_pixel(
                                        cx,
                                        cy,
                                        image::Rgb([
                                            (p[0] as f32 + dr).round().clamp(0.0, 255.0) as u8,
                                            (p[1] as f32 + dg).round().clamp(0.0, 255.0) as u8,
                                            (p[2] as f32 + db).round().clamp(0.0, 255.0) as u8,
                                        ]),
                                    );
                                }
                            }
                        }
                    }
                }
            }

            let out_dyn = DynamicImage::ImageRgb8(out_rgb);
            let final_patch = if (orig_w, orig_h) == (SD_SIZE, SD_SIZE) {
                out_dyn
            } else {
                out_dyn.resize_exact(orig_w, orig_h, imageops::FilterType::Lanczos3)
            };
            let vae_dec_ms = t_vae_dec.elapsed().as_millis() as u64;

            // 4. Accumulate benchmark instrumentation across ROIs
            if let Some(rep) = reporter {
                rep.vae_encode_ms += vae_enc_ms;
                rep.denoise_ms += denoise_ms;
                rep.denoise_steps += num_steps;
                rep.denoise_step_avg_ms = if rep.denoise_steps > 0 {
                    rep.denoise_ms as f64 / rep.denoise_steps as f64
                } else {
                    0.0
                };
                rep.vae_decode_ms += vae_dec_ms;
            }

            Ok(final_patch)
        }

        #[cfg(not(feature = "nn-cpu"))]
        {
            let _ = prompt;
            let _ = reporter;
            let _ = progress;
            inpaint_classical(image, mask, steps)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GrayImage, RgbaImage};

    #[test]
    fn test_classical_inpainter_smoke() {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(32, 32));
        let mask = GrayImage::new(32, 32);
        let res = inpaint_classical(&img, &mask, 10).unwrap();
        assert_eq!(res.width(), 32);
        assert_eq!(res.height(), 32);
    }
}
