use crate::erase::mask::feather_mask;
use crate::erase::models::ModelPaths;
use crate::error::MartiniError;
use image::{DynamicImage, GrayImage, Rgba};

pub struct InpaintPipeline {}

impl InpaintPipeline {
    pub fn load(_paths: &ModelPaths) -> Result<Self, MartiniError> {
        Ok(Self {})
    }

    pub fn inpaint(
        &self,
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

    /// Blends an inpainted crop patch back into the original image using the feathered mask
    pub fn blend_result(
        original: &DynamicImage,
        inpainted_patch: &DynamicImage,
        mask: &GrayImage,
        crop: (u32, u32, u32, u32),
    ) -> Result<DynamicImage, MartiniError> {
        let (crop_x, crop_y, crop_w, crop_h) = crop;
        let mut result = original.to_rgba8();
        let patch_rgba = inpainted_patch.to_rgba8();

        // Feather mask slightly to ensure smooth boundaries
        let feathered = feather_mask(mask, 2);

        for py in 0..crop_h {
            for px in 0..crop_w {
                let ox = crop_x + px;
                let oy = crop_y + py;

                if ox < result.width()
                    && oy < result.height()
                    && px < patch_rgba.width()
                    && py < patch_rgba.height()
                {
                    let mask_val = feathered.get_pixel(ox, oy)[0] as f32 / 255.0;

                    if mask_val > 0.0 {
                        let orig_px = result.get_pixel(ox, oy);
                        let patch_px = patch_rgba.get_pixel(px, py);

                        let r = ((1.0 - mask_val) * orig_px[0] as f32
                            + mask_val * patch_px[0] as f32)
                            .round() as u8;
                        let g = ((1.0 - mask_val) * orig_px[1] as f32
                            + mask_val * patch_px[1] as f32)
                            .round() as u8;
                        let b = ((1.0 - mask_val) * orig_px[2] as f32
                            + mask_val * patch_px[2] as f32)
                            .round() as u8;
                        let a = ((1.0 - mask_val) * orig_px[3] as f32
                            + mask_val * patch_px[3] as f32)
                            .round() as u8;

                        result.put_pixel(ox, oy, Rgba([r, g, b, a]));
                    }
                }
            }
        }

        Ok(DynamicImage::ImageRgba8(result))
    }
}
