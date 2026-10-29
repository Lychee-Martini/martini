#[cfg(feature = "nn-cpu")]
use crate::erase::mask::extract_component_clusters;
use crate::erase::mask::{detect_text_mask, dilate_mask};
use crate::error::MartiniError;
use image::{DynamicImage, GrayImage, Luma};
use std::path::Path;

#[cfg(feature = "nn-cpu")]
use candle_core::{D, DType, Device, Tensor};
#[cfg(feature = "nn-cpu")]
use candle_transformers::models::clip::{ClipConfig, ClipModel};

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

/// Checks whether the user's target description refers to text, watermarks, digits, or annotations
pub fn is_text_like_target(target: &str) -> bool {
    let lower = target.trim().to_lowercase();
    const KEYWORDS: &[&str] = &[
        "text",
        "watermark",
        "label",
        "number",
        "digit",
        "word",
        "letter",
        "character",
        "caption",
        "subtitle",
        "title",
        "annotation",
        "dimension",
        "code",
        "logo",
        "stamp",
        "signature",
        "writing",
        "font",
        "typography",
        "文字",
        "水印",
        "数字",
        "字幕",
        "标签",
        "标注",
        "编号",
    ];
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

pub struct ClipGrounding {
    #[cfg(feature = "nn-cpu")]
    pub device: Device,
    #[cfg(feature = "nn-cpu")]
    pub tokenizer: tokenizers::Tokenizer,
    #[cfg(feature = "nn-cpu")]
    model: ClipModel,
}

impl ClipGrounding {
    #[cfg(feature = "nn-cpu")]
    pub fn load(
        weights: &Path,
        tokenizer_path: &Path,
        device: &Device,
    ) -> Result<Self, MartiniError> {
        let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to load CLIP tokenizer: {}", e),
            }
        })?;

        let config = ClipConfig::vit_base_patch32();
        let vb = unsafe {
            candle_nn::VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, device)
        }
        .map_err(|e| MartiniError::InvalidInputData {
            reason: format!(
                "Failed to load CLIP safetensors '{}': {}",
                weights.display(),
                e
            ),
        })?;
        let model = ClipModel::new(vb, &config).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to initialize CLIP grounding model: {}", e),
        })?;

        Ok(Self {
            device: device.clone(),
            tokenizer,
            model,
        })
    }

    #[cfg(not(feature = "nn-cpu"))]
    pub fn load(_weights: &Path, _tokenizer_path: &Path) -> Result<Self, MartiniError> {
        Ok(Self {})
    }

    #[cfg(feature = "nn-cpu")]
    fn encode_prompts(&self, prompts: &[String]) -> Result<Tensor, MartiniError> {
        const EOS_ID: u32 = 49407;
        const MAX_LEN: usize = 77;

        let mut encoded_list = Vec::with_capacity(prompts.len());
        let mut max_seq = 2usize;

        for p in prompts {
            let enc = self.tokenizer.encode(p.as_str(), true).map_err(|e| {
                MartiniError::InvalidInputData {
                    reason: format!("Failed to tokenize CLIP prompt '{}': {}", p, e),
                }
            })?;
            let mut ids = enc.get_ids().to_vec();
            if ids.is_empty() {
                ids = vec![49406, EOS_ID];
            } else if ids.len() > MAX_LEN {
                ids.truncate(MAX_LEN);
                ids[MAX_LEN - 1] = EOS_ID;
            }
            max_seq = max_seq.max(ids.len());
            encoded_list.push(ids);
        }

        // Pad after EOS (49407) with 0 so `input_ids.argmax(D::Minus1)` in Candle's ClipTextTransformer
        // always selects the exact EOS token index for each sequence.
        let mut flat = Vec::with_capacity(encoded_list.len() * max_seq);
        for mut ids in encoded_list {
            ids.resize(max_seq, 0u32);
            flat.extend_from_slice(&ids);
        }

        Tensor::from_vec(flat, (prompts.len(), max_seq), &self.device).map_err(|e| {
            MartiniError::InvalidInputData {
                reason: format!("Failed to create CLIP token tensor: {}", e),
            }
        })
    }

    #[cfg(feature = "nn-cpu")]
    fn crops_to_pixel_values(&self, crops: &[DynamicImage]) -> Result<Tensor, MartiniError> {
        const CLIP_SIZE: u32 = 224;
        const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
        const STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

        let plane = (CLIP_SIZE * CLIP_SIZE) as usize;
        let mut data = Vec::with_capacity(crops.len() * 3 * plane);

        for crop in crops {
            let resized =
                crop.resize_exact(CLIP_SIZE, CLIP_SIZE, image::imageops::FilterType::Triangle);
            let rgb = resized.to_rgb8();
            let mut r_plane = Vec::with_capacity(plane);
            let mut g_plane = Vec::with_capacity(plane);
            let mut b_plane = Vec::with_capacity(plane);

            for px in rgb.pixels() {
                r_plane.push(((px[0] as f32 / 255.0) - MEAN[0]) / STD[0]);
                g_plane.push(((px[1] as f32 / 255.0) - MEAN[1]) / STD[1]);
                b_plane.push(((px[2] as f32 / 255.0) - MEAN[2]) / STD[2]);
            }

            data.extend_from_slice(&r_plane);
            data.extend_from_slice(&g_plane);
            data.extend_from_slice(&b_plane);
        }

        Tensor::from_vec(
            data,
            (crops.len(), 3, CLIP_SIZE as usize, CLIP_SIZE as usize),
            &self.device,
        )
        .map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to build CLIP image batch tensor: {}", e),
        })
    }

    #[cfg(feature = "nn-cpu")]
    fn score_crops_positive_prob(
        &self,
        crops: &[DynamicImage],
        input_ids: &Tensor,
        num_positive: usize,
    ) -> Result<Vec<f32>, MartiniError> {
        let mut probs_out = Vec::with_capacity(crops.len());

        for chunk in crops.chunks(32) {
            let pixel_values = self.crops_to_pixel_values(chunk)?;
            let (_logits_per_text, logits_per_image) = self
                .model
                .forward(&pixel_values, input_ids)
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("CLIP forward pass failed: {}", e),
                })?;

            let probs = candle_nn::ops::softmax(&logits_per_image, D::Minus1).map_err(|e| {
                MartiniError::InvalidInputData {
                    reason: format!("CLIP softmax failed: {}", e),
                }
            })?;
            let rows = probs
                .to_vec2::<f32>()
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to read CLIP probabilities: {}", e),
                })?;

            for row in rows {
                let pos_sum: f32 = row.iter().take(num_positive).sum();
                probs_out.push(pos_sum);
            }
        }

        Ok(probs_out)
    }

    #[cfg(feature = "nn-cpu")]
    fn crop_letterboxed(img: &DynamicImage, x0: u32, y0: u32, x1: u32, y1: u32) -> DynamicImage {
        let cw = x1.saturating_sub(x0).max(1);
        let ch = y1.saturating_sub(y0).max(1);
        let sub = img.crop_imm(x0, y0, cw, ch).to_rgb8();

        // Estimate local perimeter background color for aspect-ratio-preserving letterboxing
        let mut r_sum = 0u64;
        let mut g_sum = 0u64;
        let mut b_sum = 0u64;
        let mut count = 0u64;
        for x in 0..cw {
            for y in [0, ch - 1] {
                let p = sub.get_pixel(x, y);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
        }
        for y in 0..ch {
            for x in [0, cw - 1] {
                let p = sub.get_pixel(x, y);
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                count += 1;
            }
        }
        let bg = image::Rgb([
            r_sum.checked_div(count).unwrap_or(220) as u8,
            g_sum.checked_div(count).unwrap_or(220) as u8,
            b_sum.checked_div(count).unwrap_or(220) as u8,
        ]);

        let side = cw.max(ch).max(64);
        let mut canvas = image::RgbImage::from_pixel(side, side, bg);
        let ox = ((side - cw) / 2) as i64;
        let oy = ((side - ch) / 2) as i64;
        image::imageops::overlay(&mut canvas, &sub, ox, oy);
        DynamicImage::ImageRgb8(canvas)
    }

    #[cfg(feature = "nn-cpu")]
    fn locate_text_candidates(
        &self,
        img: &DynamicImage,
        target: &str,
        threshold: f32,
    ) -> Result<GrayImage, MartiniError> {
        let (orig_w, orig_h) = (img.width(), img.height());
        let candidate_mask = detect_text_mask(img, threshold);
        // Tight word-level merge gap (8px) keeps text labels distinct from long spanning lines
        let clusters = extract_component_clusters(&candidate_mask, 8);
        if clusters.is_empty() {
            return Ok(GrayImage::new(orig_w, orig_h));
        }

        let prompts = vec![
            format!("close up of {}", target),
            "printed text, letters, numbers, digits, or characters overlay".to_string(),
            "text label, watermark, measurement line, or dimension annotation".to_string(),
            "green plant leaves, flower stem, or foliage without text".to_string(),
            "flower petals or natural object without any text".to_string(),
            "plain fabric cloth, wall, or background texture without text".to_string(),
        ];
        let input_ids = self.encode_prompts(&prompts)?;

        let mut crops = Vec::new();
        let mut crop_ranges = Vec::with_capacity(clusters.len());
        let margin = 24u32;

        for cluster in &clusters {
            let (bx, by, bw, bh) = cluster.bbox;
            let start_idx = crops.len();

            if bw.max(bh) > 180 && cluster.pixels.len() >= 4 {
                // Sample tight local patches along elongated lines (e.g. dimension rulers)
                let half = 56u32;
                for frac in [
                    0usize,
                    cluster.pixels.len() / 4,
                    cluster.pixels.len() / 2,
                    (cluster.pixels.len() * 3) / 4,
                ] {
                    let (ax, ay) = cluster.pixels[frac];
                    let x0 = ax.saturating_sub(half);
                    let y0 = ay.saturating_sub(half);
                    let x1 = (ax + half).min(orig_w);
                    let y1 = (ay + half).min(orig_h);
                    crops.push(Self::crop_letterboxed(img, x0, y0, x1, y1));
                }
            } else {
                let x0 = bx.saturating_sub(margin);
                let y0 = by.saturating_sub(margin);
                let x1 = (bx + bw + margin).min(orig_w);
                let y1 = (by + bh + margin).min(orig_h);
                crops.push(Self::crop_letterboxed(img, x0, y0, x1, y1));
            }
            crop_ranges.push(start_idx..crops.len());
        }

        let pos_probs = self.score_crops_positive_prob(&crops, &input_ids, 3)?;
        let clip_cutoff = (threshold * 0.5).clamp(0.12, 0.45);

        let mut verified = GrayImage::new(orig_w, orig_h);
        for (cluster, range) in clusters.iter().zip(crop_ranges) {
            let max_prob = pos_probs[range].iter().copied().fold(0.0f32, f32::max);
            if max_prob >= clip_cutoff {
                for &(px, py) in &cluster.pixels {
                    verified.put_pixel(px, py, Luma([255]));
                }
            }
        }

        Ok(dilate_mask(&verified, 2))
    }

    #[cfg(feature = "nn-cpu")]
    fn locate_object_sliding_window(
        &self,
        img: &DynamicImage,
        target: &str,
        threshold: f32,
    ) -> Result<GrayImage, MartiniError> {
        let (orig_w, orig_h) = (img.width(), img.height());
        if orig_w == 0 || orig_h == 0 {
            return Ok(GrayImage::new(orig_w, orig_h));
        }

        let prompts = vec![
            format!("a photo of {}", target),
            format!("close up of {}", target),
            "empty plain background surface without objects".to_string(),
            "surrounding background scenery".to_string(),
        ];
        let input_ids = self.encode_prompts(&prompts)?;

        let min_dim = orig_w.min(orig_h);
        let scales = [
            (min_dim / 2).clamp(96, 384).min(min_dim),
            (min_dim / 4).clamp(64, 224).min(min_dim),
        ];

        let map_w = (orig_w / 8).clamp(16, 128);
        let map_h = (orig_h / 8).clamp(16, 128);
        let mut prob_sum = vec![0.0f32; (map_w * map_h) as usize];
        let mut weight_sum = vec![0.0f32; (map_w * map_h) as usize];

        let mut windows = Vec::new();
        let mut crops = Vec::new();

        for &win in &scales {
            if win == 0 {
                continue;
            }
            let stride = (win / 2).max(16);
            let mut y = 0u32;
            loop {
                let wy = y.min(orig_h - win);
                let mut x = 0u32;
                loop {
                    let wx = x.min(orig_w - win);
                    windows.push((wx, wy, win, win));
                    crops.push(img.crop_imm(wx, wy, win, win));
                    if wx + win >= orig_w {
                        break;
                    }
                    x += stride;
                }
                if wy + win >= orig_h {
                    break;
                }
                y += stride;
            }
        }

        let probs = self.score_crops_positive_prob(&crops, &input_ids, 2)?;

        for (&(wx, wy, ww, wh), &prob) in windows.iter().zip(probs.iter()) {
            let gx0 = ((wx as u64 * map_w as u64) / orig_w as u64) as u32;
            let gy0 = ((wy as u64 * map_h as u64) / orig_h as u64) as u32;
            let gx1 = (((wx + ww) as u64 * map_w as u64).div_ceil(orig_w as u64) as u32).min(map_w);
            let gy1 = (((wy + wh) as u64 * map_h as u64).div_ceil(orig_h as u64) as u32).min(map_h);

            for gy in gy0..gy1 {
                for gx in gx0..gx1 {
                    let idx = (gy * map_w + gx) as usize;
                    prob_sum[idx] += prob;
                    weight_sum[idx] += 1.0;
                }
            }
        }

        let cutoff = threshold.clamp(0.15, 0.90) + 0.25;
        let mut coarse = GrayImage::new(map_w, map_h);
        for gy in 0..map_h {
            for gx in 0..map_w {
                let idx = (gy * map_w + gx) as usize;
                let avg = if weight_sum[idx] > 0.0 {
                    prob_sum[idx] / weight_sum[idx]
                } else {
                    0.0
                };
                if avg >= cutoff.min(0.85) {
                    coarse.put_pixel(gx, gy, Luma([255]));
                }
            }
        }

        let upscaled = image::imageops::resize(
            &coarse,
            orig_w,
            orig_h,
            image::imageops::FilterType::Triangle,
        );
        let mut binary = GrayImage::new(orig_w, orig_h);
        for y in 0..orig_h {
            for x in 0..orig_w {
                if upscaled.get_pixel(x, y)[0] > 127 {
                    binary.put_pixel(x, y, Luma([255]));
                }
            }
        }

        Ok(dilate_mask(&binary, 4))
    }

    pub fn locate(
        &self,
        img: &DynamicImage,
        target: &str,
        threshold: f32,
    ) -> Result<GrayImage, MartiniError> {
        #[cfg(feature = "nn-cpu")]
        {
            if is_text_like_target(target) {
                self.locate_text_candidates(img, target, threshold)
            } else {
                self.locate_object_sliding_window(img, target, threshold)
            }
        }

        #[cfg(not(feature = "nn-cpu"))]
        {
            let _ = target;
            let detected = detect_text_mask(img, threshold);
            Ok(dilate_mask(&detected, 2))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_heatmap_scaling() {
        let raw_scores = vec![0.1f32, 0.5, 0.9, 0.3];
        let normalized = normalize_heatmap(&raw_scores, 2, 2, 0.5);
        assert_eq!(normalized.dimensions(), (2, 2));
        assert_eq!(normalized.get_pixel(0, 1)[0], 255);
        assert_eq!(normalized.get_pixel(0, 0)[0], 0);
    }

    #[test]
    fn test_is_text_like_target_detection() {
        assert!(is_text_like_target("text"));
        assert!(is_text_like_target("watermark"));
        assert!(is_text_like_target("printed numbers"));
        assert!(is_text_like_target("水印"));
        assert!(!is_text_like_target("rose flower"));
        assert!(!is_text_like_target("person"));
    }
}
