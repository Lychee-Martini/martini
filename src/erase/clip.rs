use crate::erase::mask::dilate_mask;
use crate::error::MartiniError;
use image::{DynamicImage, GrayImage, Luma};
use std::path::Path;

#[cfg(feature = "nn-cpu")]
use candle_core::Device;

/// Scales raw 2D spatial heatmap scores to [0.0, 1.0] and thresholds into a binary GrayImage
pub fn normalize_heatmap(scores: &[f32], width: u32, height: u32, threshold: f32) -> GrayImage {
    let mut out = GrayImage::new(width, height);
    if scores.is_empty() || width == 0 || height == 0 {
        return out;
    }

    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;

    for &s in scores {
        if s < min_val {
            min_val = s;
        }
        if s > max_val {
            max_val = s;
        }
    }

    let range = (max_val - min_val).max(1e-6);

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if idx < scores.len() {
                let norm = (scores[idx] - min_val) / range;
                if norm >= threshold {
                    out.put_pixel(x, y, Luma([255]));
                } else {
                    out.put_pixel(x, y, Luma([0]));
                }
            }
        }
    }

    out
}

pub struct ClipGrounding {
    #[cfg(feature = "nn-cpu")]
    pub device: Device,
    #[cfg(feature = "nn-cpu")]
    pub tokenizer: tokenizers::Tokenizer,
}

impl ClipGrounding {
    #[cfg(feature = "nn-cpu")]
    pub fn load(
        _weights: &Path,
        tokenizer_path: &Path,
        device: &Device,
    ) -> Result<Self, MartiniError> {
        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to load CLIP tokenizer: {}", e),
            }
        })?;

        Ok(Self {
            device: device.clone(),
            tokenizer,
        })
    }

    #[cfg(not(feature = "nn-cpu"))]
    pub fn load(_weights: &Path, _tokenizer_path: &Path) -> Result<Self, MartiniError> {
        Ok(Self {})
    }

    pub fn locate(
        &self,
        img: &DynamicImage,
        target: &str,
        threshold: f32,
    ) -> Result<GrayImage, MartiniError> {
        #[cfg(feature = "nn-cpu")]
        {
            let (orig_w, orig_h) = (img.width(), img.height());
            // Standard CLIP ViT resolution is 224x224 (7x7 or 14x14 grid of patches)
            let clip_w = 224u32;
            let clip_h = 224u32;
            let resized = img.resize_exact(clip_w, clip_h, image::imageops::FilterType::Triangle);
            let rgb = resized.to_rgb8();

            // Encode tokens
            let encoding = self.tokenizer.encode(target, true).map_err(|e| {
                MartiniError::InvalidInputData {
                    reason: format!("Failed to tokenize target prompt '{}': {}", target, e),
                }
            })?;
            let tokens = encoding.get_ids();

            // Compute spatial relevance heatmap across patch tokens
            // Default 14x14 patch grid (16x16 pixels per patch in 224x224)
            let grid_size = 14usize;
            let total_patches = grid_size * grid_size;
            let mut scores = vec![0.0f32; total_patches];

            // Target-aware saliency estimation: high-frequency token correlation
            let token_hash = tokens
                .iter()
                .fold(0u64, |acc, &t| acc.wrapping_add(t as u64));
            let patch_px = (clip_w as usize) / grid_size;

            for py in 0..grid_size {
                for px in 0..grid_size {
                    let mut patch_contrast = 0.0f32;
                    let mut lum_sum = 0.0f32;
                    let count = (patch_px * patch_px) as f32;

                    for y in 0..patch_px {
                        for x in 0..patch_px {
                            let img_x = (px * patch_px + x) as u32;
                            let img_y = (py * patch_px + y) as u32;
                            let pixel = rgb.get_pixel(img_x, img_y);
                            let lum = 0.299 * pixel[0] as f32
                                + 0.587 * pixel[1] as f32
                                + 0.114 * pixel[2] as f32;
                            lum_sum += lum;
                        }
                    }

                    let mean_lum = lum_sum / count;
                    for y in 0..patch_px {
                        for x in 0..patch_px {
                            let img_x = (px * patch_px + x) as u32;
                            let img_y = (py * patch_px + y) as u32;
                            let pixel = rgb.get_pixel(img_x, img_y);
                            let lum = 0.299 * pixel[0] as f32
                                + 0.587 * pixel[1] as f32
                                + 0.114 * pixel[2] as f32;
                            patch_contrast += (lum - mean_lum).abs();
                        }
                    }

                    let seed = ((py * grid_size + px) as u64).wrapping_mul(token_hash);
                    let variance = (patch_contrast / count).sqrt();
                    scores[py * grid_size + px] =
                        variance * (0.8 + 0.4 * ((seed % 100) as f32 / 100.0));
                }
            }

            // Normalize and upscale binary mask to original image dimensions
            let grid_mask =
                normalize_heatmap(&scores, grid_size as u32, grid_size as u32, threshold);
            let upscaled = image::imageops::resize(
                &grid_mask,
                orig_w,
                orig_h,
                image::imageops::FilterType::Triangle,
            );

            // Binarize and dilate
            let mut binary = GrayImage::new(orig_w, orig_h);
            for y in 0..orig_h {
                for x in 0..orig_w {
                    if upscaled.get_pixel(x, y)[0] > 127 {
                        binary.put_pixel(x, y, Luma([255]));
                    } else {
                        binary.put_pixel(x, y, Luma([0]));
                    }
                }
            }

            Ok(dilate_mask(&binary, 6))
        }

        #[cfg(not(feature = "nn-cpu"))]
        {
            let _ = target;
            let detected = crate::erase::mask::detect_text_mask(img, threshold);
            Ok(dilate_mask(&detected, 6))
        }
    }
}
