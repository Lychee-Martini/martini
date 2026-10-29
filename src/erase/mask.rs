use image::{GenericImageView, GrayImage, Luma};

/// Computes a proportional resolution scale factor relative to ~1024px (1 for <=1535px, 4 for 4032x3024)
pub fn resolution_scale(width: u32, height: u32) -> u32 {
    ((width.max(height) as f32) / 1024.0)
        .round()
        .clamp(1.0, 8.0) as u32
}

/// Dilates positive mask pixels (255) by `radius` pixels using a Euclidean distance kernel
pub fn dilate_mask(mask: &GrayImage, radius: u32) -> GrayImage {
    if radius == 0 {
        return mask.clone();
    }

    let (width, height) = mask.dimensions();
    let mut dilated = mask.clone();
    let r_i32 = radius as i32;

    for y in 0..height {
        for x in 0..width {
            if mask.get_pixel(x, y)[0] == 255 {
                let x_min = (x as i32 - r_i32).max(0) as u32;
                let x_max = (x as i32 + r_i32).min(width as i32 - 1) as u32;
                let y_min = (y as i32 - r_i32).max(0) as u32;
                let y_max = (y as i32 + r_i32).min(height as i32 - 1) as u32;

                for ny in y_min..=y_max {
                    for nx in x_min..=x_max {
                        let dx = nx as i32 - x as i32;
                        let dy = ny as i32 - y as i32;
                        if dx * dx + dy * dy <= r_i32 * r_i32 {
                            dilated.put_pixel(nx, ny, Luma([255]));
                        }
                    }
                }
            }
        }
    }

    dilated
}

/// Applies a separable 2D box blur to soften/feather mask edges
pub fn feather_mask(mask: &GrayImage, radius: u32) -> GrayImage {
    if radius == 0 {
        return mask.clone();
    }

    let (width, height) = mask.dimensions();
    if width == 0 || height == 0 {
        return GrayImage::new(width, height);
    }

    let r_i32 = radius as i32;
    let w_usize = width as usize;
    let mut h_sum = vec![0u32; (width * height) as usize];
    let mut h_cnt = vec![0u32; w_usize];

    for x in 0..width {
        let x_min = (x as i32 - r_i32).max(0) as u32;
        let x_max = (x as i32 + r_i32).min(width as i32 - 1) as u32;
        h_cnt[x as usize] = x_max - x_min + 1;
    }

    for y in 0..height {
        let row_off = y as usize * w_usize;
        for x in 0..width {
            let x_min = (x as i32 - r_i32).max(0) as u32;
            let x_max = (x as i32 + r_i32).min(width as i32 - 1) as u32;
            let mut sum = 0u32;
            for nx in x_min..=x_max {
                sum += mask.get_pixel(nx, y)[0] as u32;
            }
            h_sum[row_off + x as usize] = sum;
        }
    }

    let mut result = GrayImage::new(width, height);
    for y in 0..height {
        let y_min = (y as i32 - r_i32).max(0) as u32;
        let y_max = (y as i32 + r_i32).min(height as i32 - 1) as u32;
        let v_cnt = y_max - y_min + 1;
        for x in 0..width {
            let mut sum = 0u32;
            for ny in y_min..=y_max {
                sum += h_sum[ny as usize * w_usize + x as usize];
            }
            let count = h_cnt[x as usize] * v_cnt;
            let avg = sum.checked_div(count).unwrap_or(0) as u8;
            result.put_pixel(x, y, Luma([avg]));
        }
    }

    result
}

/// Computes a tight bounding box around positive mask pixels with padding: (x, y, width, height)
pub fn compute_bounding_box(mask: &GrayImage, padding: u32) -> Option<(u32, u32, u32, u32)> {
    let (img_w, img_h) = mask.dimensions();
    let mut min_x = u32::MAX;
    let mut max_x = 0;
    let mut min_y = u32::MAX;
    let mut max_y = 0;
    let mut found = false;

    for y in 0..img_h {
        for x in 0..img_w {
            if mask.get_pixel(x, y)[0] > 0 {
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                found = true;
            }
        }
    }

    if !found {
        return None;
    }

    let x = min_x.saturating_sub(padding);
    let y = min_y.saturating_sub(padding);
    let w = (max_x + padding + 1).min(img_w) - x;
    let h = (max_y + padding + 1).min(img_h) - y;

    Some((x, y, w, h))
}

fn grayscale_morph(img: &GrayImage, radius: u32, init: u8, op: fn(u8, u8) -> u8) -> GrayImage {
    if radius == 0 {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let r = radius as i32;

    let mut tmp = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let x_start = (x as i32 - r).max(0) as u32;
            let x_end = (x as i32 + r).min(w as i32 - 1) as u32;
            let val = (x_start..=x_end).fold(init, |acc, nx| op(acc, img.get_pixel(nx, y)[0]));
            tmp.put_pixel(x, y, Luma([val]));
        }
    }

    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        let y_start = (y as i32 - r).max(0) as u32;
        let y_end = (y as i32 + r).min(h as i32 - 1) as u32;
        for x in 0..w {
            let val = (y_start..=y_end).fold(init, |acc, ny| op(acc, tmp.get_pixel(x, ny)[0]));
            out.put_pixel(x, y, Luma([val]));
        }
    }
    out
}

pub fn grayscale_dilate(img: &GrayImage, radius: u32) -> GrayImage {
    grayscale_morph(img, radius, 0, u8::max)
}

pub fn grayscale_erode(img: &GrayImage, radius: u32) -> GrayImage {
    grayscale_morph(img, radius, 255, u8::min)
}

type ClusterGroup = ((u32, u32, u32, u32), Vec<(u32, u32)>);

/// Connected cluster of positive mask pixels and its tight bounding box `(x, y, w, h)`
#[derive(Debug, Clone)]
pub struct MaskCluster {
    pub bbox: (u32, u32, u32, u32),
    pub pixels: Vec<(u32, u32)>,
}

/// Extracts connected components of positive (`> 0`) mask pixels and merges components within `merge_gap` pixels
pub fn extract_component_clusters(mask: &GrayImage, merge_gap: u32) -> Vec<MaskCluster> {
    let (w, h) = mask.dimensions();
    let mut visited = vec![false; (w * h) as usize];
    let mut raw_clusters: Vec<ClusterGroup> = Vec::new();

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if mask.get_pixel(x, y)[0] > 0 && !visited[idx] {
                let mut pixels = Vec::new();
                let mut queue = std::collections::VecDeque::new();
                queue.push_back((x, y));
                visited[idx] = true;

                let mut min_x = x;
                let mut max_x = x;
                let mut min_y = y;
                let mut max_y = y;

                while let Some((cx, cy)) = queue.pop_front() {
                    pixels.push((cx, cy));
                    min_x = min_x.min(cx);
                    max_x = max_x.max(cx);
                    min_y = min_y.min(cy);
                    max_y = max_y.max(cy);

                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = cx as i32 + dx;
                            let ny = cy as i32 + dy;
                            if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                                let n_idx = (ny as u32 * w + nx as u32) as usize;
                                if mask.get_pixel(nx as u32, ny as u32)[0] > 0 && !visited[n_idx] {
                                    visited[n_idx] = true;
                                    queue.push_back((nx as u32, ny as u32));
                                }
                            }
                        }
                    }
                }

                raw_clusters.push(((min_x, min_y, max_x, max_y), pixels));
            }
        }
    }

    // Iteratively merge clusters whose bounding boxes are within `merge_gap` pixels
    loop {
        let mut merged_any = false;
        let mut i = 0;
        while i < raw_clusters.len() {
            let mut j = i + 1;
            while j < raw_clusters.len() {
                let (ax0, ay0, ax1, ay1) = raw_clusters[i].0;
                let (bx0, by0, bx1, by1) = raw_clusters[j].0;
                let x_close =
                    ax0 <= bx1.saturating_add(merge_gap) && bx0 <= ax1.saturating_add(merge_gap);
                let y_close =
                    ay0 <= by1.saturating_add(merge_gap) && by0 <= ay1.saturating_add(merge_gap);

                if x_close && y_close {
                    let removed = raw_clusters.swap_remove(j);
                    raw_clusters[i].0 = (
                        ax0.min(removed.0.0),
                        ay0.min(removed.0.1),
                        ax1.max(removed.0.2),
                        ay1.max(removed.0.3),
                    );
                    raw_clusters[i].1.extend(removed.1);
                    merged_any = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
        if !merged_any {
            break;
        }
    }

    raw_clusters
        .into_iter()
        .map(|((x0, y0, x1, y1), pixels)| MaskCluster {
            bbox: (x0, y0, x1 - x0 + 1, y1 - y0 + 1),
            pixels,
        })
        .collect()
}

/// Computes local inpainting ROIs `(x, y, w, h)` and each ROI's isolated local mask.
/// - When `context_size == 0`, returns tight bounding boxes padded by `padding` (for fast classical inpainting).
/// - When `context_size > 0` (e.g. `512`), groups nearby components into native `context_size x context_size` windows.
pub fn compute_inpaint_roi_masks(
    mask: &GrayImage,
    context_size: u32,
    padding: u32,
) -> Vec<((u32, u32, u32, u32), GrayImage)> {
    let (img_w, img_h) = mask.dimensions();
    if img_w == 0 || img_h == 0 {
        return Vec::new();
    }

    let initial_gap = if context_size == 0 {
        padding.saturating_mul(2)
    } else {
        (context_size / 32).max(16)
    };
    let clusters = extract_component_clusters(mask, initial_gap);
    if clusters.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<ClusterGroup> = clusters
        .into_iter()
        .map(|c| {
            let (x, y, w, h) = c.bbox;
            ((x, y, x + w - 1, y + h - 1), c.pixels)
        })
        .collect();

    if context_size > 0 {
        let win_w = context_size.min(img_w);
        let win_h = context_size.min(img_h);
        let max_span_w = win_w.saturating_sub(padding * 2).max(win_w / 2);
        let max_span_h = win_h.saturating_sub(padding * 2).max(win_h / 2);

        // Greedily merge closest groups that jointly fit inside one context window with padding
        loop {
            let mut best_pair: Option<(usize, usize, u64)> = None;
            for i in 0..groups.len() {
                for j in (i + 1)..groups.len() {
                    let (ax0, ay0, ax1, ay1) = groups[i].0;
                    let (bx0, by0, bx1, by1) = groups[j].0;
                    let ux0 = ax0.min(bx0);
                    let uy0 = ay0.min(by0);
                    let ux1 = ax1.max(bx1);
                    let uy1 = ay1.max(by1);

                    if ux1 - ux0 < max_span_w && uy1 - uy0 < max_span_h {
                        let acx = (ax0 + ax1) as i64 / 2;
                        let acy = (ay0 + ay1) as i64 / 2;
                        let bcx = (bx0 + bx1) as i64 / 2;
                        let bcy = (by0 + by1) as i64 / 2;
                        let dist_sq = ((acx - bcx).pow(2) + (acy - bcy).pow(2)) as u64;
                        if best_pair.is_none_or(|(_, _, best_d)| dist_sq < best_d) {
                            best_pair = Some((i, j, dist_sq));
                        }
                    }
                }
            }

            if let Some((i, j, _)) = best_pair {
                let removed = groups.swap_remove(j);
                let (ax0, ay0, ax1, ay1) = groups[i].0;
                groups[i].0 = (
                    ax0.min(removed.0.0),
                    ay0.min(removed.0.1),
                    ax1.max(removed.0.2),
                    ay1.max(removed.0.3),
                );
                groups[i].1.extend(removed.1);
            } else {
                break;
            }
        }
    }

    groups
        .into_iter()
        .map(|((x0, y0, x1, y1), pixels)| {
            let (rx, ry, rw, rh) = if context_size == 0 {
                let rx = x0.saturating_sub(padding);
                let ry = y0.saturating_sub(padding);
                let rw = (x1 + padding + 1).min(img_w) - rx;
                let rh = (y1 + padding + 1).min(img_h) - ry;
                (rx, ry, rw, rh)
            } else {
                let win_w = context_size.min(img_w);
                let win_h = context_size.min(img_h);
                let span_w = x1 - x0 + 1;
                let span_h = y1 - y0 + 1;
                let rw = (span_w + padding * 2).max(win_w).min(img_w);
                let rh = (span_h + padding * 2).max(win_h).min(img_h);
                let cx = (x0 + x1) / 2;
                let cy = (y0 + y1) / 2;
                let rx = cx.saturating_sub(rw / 2).min(img_w - rw);
                let ry = cy.saturating_sub(rh / 2).min(img_h - rh);
                (rx, ry, rw, rh)
            };

            let mut local_mask = GrayImage::new(rw, rh);
            for (px, py) in pixels {
                if px >= rx && px < rx + rw && py >= ry && py < ry + rh {
                    local_mask.put_pixel(px - rx, py - ry, Luma([mask.get_pixel(px, py)[0]]));
                }
            }
            ((rx, ry, rw, rh), local_mask)
        })
        .collect()
}

/// Computes local inpainting ROI rectangles `(x, y, width, height)` for scattered mask regions
pub fn compute_inpaint_rois(
    mask: &GrayImage,
    context_size: u32,
    padding: u32,
) -> Vec<(u32, u32, u32, u32)> {
    compute_inpaint_roi_masks(mask, context_size, padding)
        .into_iter()
        .map(|(roi, _)| roi)
        .collect()
}

/// A single local ROI crop `(x, y, w, h)` and its isolated local mask
pub type RoiMask = ((u32, u32, u32, u32), GrayImage);

/// Execution batch for neural 512x512 diffusion:
/// - `Single`: one full 512x512 diffusion pass
/// - `Quadrants`: 2..=4 disjoint square ROIs packed into a single 2x2 512x512 pass
#[derive(Debug, Clone)]
pub enum InpaintBatch {
    Single(RoiMask),
    Quadrants(Vec<RoiMask>),
}

/// Plans neural diffusion batches, packing small/medium scattered ROIs into 2x2 512x512 quadrants
/// when `allow_quadrants` is true and every packed tile maintains >= 24px (3 latent cells) of
/// unmasked border margin inside its 256x256 quadrant.
pub fn plan_neural_inpaint_batches(mask: &GrayImage, allow_quadrants: bool) -> Vec<InpaintBatch> {
    let (img_w, img_h) = mask.dimensions();
    if img_w == 0 || img_h == 0 {
        return Vec::new();
    }

    let res_scale = resolution_scale(img_w, img_h);
    let min_img_dim = img_w.min(img_h);
    if !allow_quadrants || min_img_dim < 256 {
        return compute_inpaint_roi_masks(mask, 512 * res_scale, 64 * res_scale)
            .into_iter()
            .map(InpaintBatch::Single)
            .collect();
    }

    let clusters = extract_component_clusters(mask, 16 * res_scale);
    if clusters.is_empty() {
        return Vec::new();
    }

    let mut groups: Vec<ClusterGroup> = clusters
        .into_iter()
        .map(|c| {
            let (x, y, w, h) = c.bbox;
            ((x, y, x + w - 1, y + h - 1), c.pixels)
        })
        .collect();

    // Greedily merge nearby groups that fit comfortably inside a single quadrant tile
    let max_tile_span = (144 * res_scale).min((min_img_dim * 9) / 16);
    loop {
        let mut best_pair: Option<(usize, usize, u64)> = None;
        for i in 0..groups.len() {
            for j in (i + 1)..groups.len() {
                let (ax0, ay0, ax1, ay1) = groups[i].0;
                let (bx0, by0, bx1, by1) = groups[j].0;
                let ux0 = ax0.min(bx0);
                let uy0 = ay0.min(by0);
                let ux1 = ax1.max(bx1);
                let uy1 = ay1.max(by1);

                if ux1 - ux0 < max_tile_span && uy1 - uy0 < max_tile_span {
                    let acx = (ax0 + ax1) as i64 / 2;
                    let acy = (ay0 + ay1) as i64 / 2;
                    let bcx = (bx0 + bx1) as i64 / 2;
                    let bcy = (by0 + by1) as i64 / 2;
                    let dist_sq = ((acx - bcx).pow(2) + (acy - bcy).pow(2)) as u64;
                    if best_pair.is_none_or(|(_, _, best_d)| dist_sq < best_d) {
                        best_pair = Some((i, j, dist_sq));
                    }
                }
            }
        }

        if let Some((i, j, _)) = best_pair {
            let removed = groups.swap_remove(j);
            let (ax0, ay0, ax1, ay1) = groups[i].0;
            groups[i].0 = (
                ax0.min(removed.0.0),
                ay0.min(removed.0.1),
                ax1.max(removed.0.2),
                ay1.max(removed.0.3),
            );
            groups[i].1.extend(removed.1);
        } else {
            break;
        }
    }

    let min_tile_side = (128 * res_scale).max(256).min(min_img_dim);
    let max_tile_side = (256 * res_scale).max(384).min(min_img_dim);
    let pad = 48 * res_scale;

    let mut packable: Vec<RoiMask> = Vec::new();
    let mut oversized_pixels: Vec<(u32, u32)> = Vec::new();

    for ((x0, y0, x1, y1), pixels) in groups {
        let span_w = x1 - x0 + 1;
        let span_h = y1 - y0 + 1;
        let side = (span_w.max(span_h) + pad * 2).max(min_tile_side);

        let mut packed = false;
        if side <= max_tile_side {
            let cx = (x0 + x1) / 2;
            let cy = (y0 + y1) / 2;
            let rx = cx.saturating_sub(side / 2).min(img_w - side);
            let ry = cy.saturating_sub(side / 2).min(img_h - side);
            let min_margin = (side * 24).div_ceil(256);

            if x0 >= rx + min_margin
                && x1 + min_margin < rx + side
                && y0 >= ry + min_margin
                && y1 + min_margin < ry + side
            {
                let mut local_mask = GrayImage::new(side, side);
                for &(px, py) in &pixels {
                    if px >= rx && px < rx + side && py >= ry && py < ry + side {
                        local_mask.put_pixel(px - rx, py - ry, Luma([mask.get_pixel(px, py)[0]]));
                    }
                }
                packable.push(((rx, ry, side, side), local_mask));
                packed = true;
            }
        }

        if !packed {
            oversized_pixels.extend(pixels);
        }
    }

    let mut batches = Vec::new();
    let mut pack_iter = packable.into_iter();
    loop {
        let mut chunk = Vec::with_capacity(4);
        for _ in 0..4 {
            if let Some(item) = pack_iter.next() {
                chunk.push(item);
            } else {
                break;
            }
        }
        match chunk.len() {
            0 => break,
            1 => batches.push(InpaintBatch::Single(chunk.pop().unwrap())),
            _ => batches.push(InpaintBatch::Quadrants(chunk)),
        }
    }

    if !oversized_pixels.is_empty() {
        let mut over_mask = GrayImage::new(img_w, img_h);
        for (px, py) in oversized_pixels {
            over_mask.put_pixel(px, py, Luma([mask.get_pixel(px, py)[0]]));
        }
        for roi in compute_inpaint_roi_masks(&over_mask, 512 * res_scale, 64 * res_scale) {
            batches.push(InpaintBatch::Single(roi));
        }
    }

    batches
}

#[inline]
fn is_neutral_rgb(px: image::Rgb<u8>) -> bool {
    let max_c = px[0].max(px[1]).max(px[2]);
    let min_c = px[0].min(px[1]).min(px[2]);
    let chroma = max_c - min_c;
    if max_c <= 25 {
        chroma <= 12
    } else {
        chroma <= 16 && (chroma as u32 * 6 <= max_c as u32)
    }
}

struct ScaleTextDetection {
    dark_mask: GrayImage,
    bright_mask: GrayImage,
    closed: GrayImage,
    opened: GrayImage,
}

fn detect_text_at_scale(
    img: &image::DynamicImage,
    thresh_u8: u8,
    radius: u32,
) -> ScaleTextDetection {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    let dilated = grayscale_dilate(&gray, radius);
    let closed = grayscale_erode(&dilated, radius);

    let eroded = grayscale_erode(&gray, radius);
    let opened = grayscale_dilate(&eroded, radius);

    let rgb = img.to_rgb8();
    let mut dark_raw = GrayImage::new(w, h);
    let mut bright_raw = GrayImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let px = *rgb.get_pixel(x, y);
            if !is_neutral_rgb(px) {
                continue;
            }

            let orig = gray.get_pixel(x, y)[0];
            let c_val = closed.get_pixel(x, y)[0];
            let o_val = opened.get_pixel(x, y)[0];

            if c_val.saturating_sub(orig) >= thresh_u8 {
                dark_raw.put_pixel(x, y, Luma([255]));
            }
            if orig >= 180 && orig.saturating_sub(o_val) >= thresh_u8 {
                bright_raw.put_pixel(x, y, Luma([255]));
            }
        }
    }

    let mut dark_mask = GrayImage::new(w, h);
    let mut bright_mask = GrayImage::new(w, h);
    filter_stroke_components(&dark_raw, &gray, &opened, 12, false, &mut dark_mask);
    filter_stroke_components(&bright_raw, &gray, &opened, 12, true, &mut bright_mask);

    ScaleTextDetection {
        dark_mask,
        bright_mask,
        closed,
        opened,
    }
}

/// Detects high-contrast text and thin overlay annotations (strokes, watermarks, dimensions)
pub fn detect_text_mask(img: &image::DynamicImage, threshold: f32) -> GrayImage {
    const NORM_MAX_DIM: u32 = 1280;

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return GrayImage::new(w, h);
    }

    let thresh_u8 = ((threshold.clamp(0.05, 0.95)) * 255.0).round() as u8;
    let max_dim = w.max(h);

    if max_dim <= NORM_MAX_DIM {
        let det = detect_text_at_scale(img, thresh_u8, 5);
        let mut out = det.dark_mask;
        for y in 0..h {
            for x in 0..w {
                if det.bright_mask.get_pixel(x, y)[0] == 255 {
                    out.put_pixel(x, y, Luma([255]));
                }
            }
        }
        return out;
    }

    // For high-resolution images (> 1280px), detect candidate stroke regions at normalized 1280px scale
    // (capturing thick 20-50px strokes in <40ms) and snap stroke edges at full native pixel resolution.
    let scale = NORM_MAX_DIM as f32 / max_dim as f32;
    let det_w = ((w as f32 * scale).round() as u32).max(1);
    let det_h = ((h as f32 * scale).round() as u32).max(1);
    let small = img.resize_exact(det_w, det_h, image::imageops::FilterType::Triangle);

    let small_thresh = ((thresh_u8 as f32) * 0.85).round().max(12.0) as u8;
    let det = detect_text_at_scale(&small, small_thresh, 8);

    let dark_env = dilate_mask(&det.dark_mask, 2);
    let bright_env = dilate_mask(&det.bright_mask, 2);

    let mut union_env = dark_env.clone();
    for sy in 0..det_h {
        for sx in 0..det_w {
            if bright_env.get_pixel(sx, sy)[0] == 255 {
                union_env.put_pixel(sx, sy, Luma([255]));
            }
        }
    }

    let mut out = GrayImage::new(w, h);
    let Some((sbx, sby, sbw, sbh)) = compute_bounding_box(&union_env, 1) else {
        return out;
    };

    let fx0 = ((sbx as u64 * w as u64) / det_w as u64) as u32;
    let fy0 = ((sby as u64 * h as u64) / det_h as u64) as u32;
    let fx1 = (((sbx + sbw) as u64 * w as u64).div_ceil(det_w as u64) as u32).min(w);
    let fy1 = (((sby + sbh) as u64 * h as u64).div_ceil(det_h as u64) as u32).min(h);
    let snap_thresh = ((thresh_u8 as u16 * 3) / 4).max(16) as u8;

    for y in fy0..fy1 {
        let sy = ((y as u64 * det_h as u64) / h as u64).min(det_h as u64 - 1) as u32;
        for x in fx0..fx1 {
            let sx = ((x as u64 * det_w as u64) / w as u64).min(det_w as u64 - 1) as u32;
            let in_dark = dark_env.get_pixel(sx, sy)[0] == 255;
            let in_bright = bright_env.get_pixel(sx, sy)[0] == 255;
            if !in_dark && !in_bright {
                continue;
            }

            let rgba = img.get_pixel(x, y);
            if !is_neutral_rgb(image::Rgb([rgba[0], rgba[1], rgba[2]])) {
                continue;
            }

            let luma = ((299 * rgba[0] as u32 + 587 * rgba[1] as u32 + 114 * rgba[2] as u32 + 500)
                / 1000) as u8;

            let dark_hit =
                in_dark && det.closed.get_pixel(sx, sy)[0].saturating_sub(luma) >= snap_thresh;
            let bright_hit = in_bright
                && luma >= 175
                && luma.saturating_sub(det.opened.get_pixel(sx, sy)[0]) >= snap_thresh;
            if dark_hit || bright_hit {
                out.put_pixel(x, y, Luma([255]));
            }
        }
    }

    out
}

/// Validates candidate dark or bright stroke components, discarding tiny speckles, full-image seams,
/// and narrow background gaps peeking between dark foreground objects (e.g. leaves or fingers).
fn filter_stroke_components(
    mask: &GrayImage,
    gray: &GrayImage,
    opened: &GrayImage,
    min_size: usize,
    is_bright: bool,
    out: &mut GrayImage,
) {
    let (w, h) = mask.dimensions();
    let mut visited = vec![false; (w * h) as usize];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if mask.get_pixel(x, y)[0] == 255 && !visited[idx] {
                let mut component = Vec::new();
                let mut queue = std::collections::VecDeque::new();
                queue.push_back((x, y));
                visited[idx] = true;

                let mut min_x = x;
                let mut max_x = x;
                let mut min_y = y;
                let mut max_y = y;
                let mut min_luma = 255u8;
                let mut sum_luma = 0u64;

                let mut boundary_total = 0usize;
                let mut boundary_same_level = 0usize;
                let mut boundary_seen = std::collections::HashSet::new();
                let mut boundary_lumas = Vec::new();

                while let Some((cx, cy)) = queue.pop_front() {
                    component.push((cx, cy));
                    min_x = min_x.min(cx);
                    max_x = max_x.max(cx);
                    min_y = min_y.min(cy);
                    max_y = max_y.max(cy);

                    let g = gray.get_pixel(cx, cy)[0];
                    min_luma = min_luma.min(g);
                    sum_luma += g as u64;

                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = cx as i32 + dx;
                            let ny = cy as i32 + dy;
                            if nx >= 0 && nx < w as i32 && ny >= 0 && ny < h as i32 {
                                let nu = nx as u32;
                                let nv = ny as u32;
                                let n_idx = (nv * w + nu) as usize;
                                if mask.get_pixel(nu, nv)[0] == 255 {
                                    if !visited[n_idx] {
                                        visited[n_idx] = true;
                                        queue.push_back((nu, nv));
                                    }
                                } else if boundary_seen.insert(n_idx) {
                                    boundary_lumas.push(gray.get_pixel(nu, nv)[0]);
                                }
                            }
                        }
                    }
                }

                if component.len() < min_size {
                    continue;
                }

                let comp_w = max_x - min_x + 1;
                let comp_h = max_y - min_y + 1;

                // Discard full-width horizontal seams (> 70% of image width) or full-height image borders (> 90%)
                if comp_w > (w * 7) / 10 || comp_h > (h * 9) / 10 {
                    continue;
                }

                let mean_luma = (sum_luma as f32) / (component.len() as f32);

                // Faint thin horizontal shadows/creases are not ink strokes
                if !is_bright && min_luma > 55 && comp_h <= 4 {
                    continue;
                }

                for bg in boundary_lumas {
                    boundary_total += 1;
                    if is_bright {
                        if (bg as f32) >= mean_luma - 20.0 {
                            boundary_same_level += 1;
                        }
                    } else if (bg as f32) <= mean_luma + 15.0 {
                        boundary_same_level += 1;
                    }
                }

                if boundary_total > 0 {
                    let same_ratio = boundary_same_level as f32 / boundary_total as f32;
                    let max_same = if is_bright { 0.04 } else { 0.25 };
                    if same_ratio > max_same {
                        continue;
                    }
                }

                // For bright candidates, reject enclosed holes of a bright page/cloth background peeking between dark objects
                if is_bright {
                    let margin = 56u32;
                    let bx0 = min_x.saturating_sub(margin);
                    let by0 = min_y.saturating_sub(margin);
                    let bx1 = (max_x + margin + 1).min(w);
                    let by1 = (max_y + margin + 1).min(h);
                    let mut bg_total = 0u32;
                    let mut bg_bright = 0u32;
                    let bright_floor = (mean_luma - 18.0).max(0.0) as u8;
                    for by in by0..by1 {
                        for bx in bx0..bx1 {
                            bg_total += 1;
                            if opened.get_pixel(bx, by)[0] >= bright_floor {
                                bg_bright += 1;
                            }
                        }
                    }
                    if bg_total > 0 && (bg_bright as f32 / bg_total as f32) >= 0.15 {
                        continue;
                    }
                }

                for (cx, cy) in component {
                    out.put_pixel(cx, cy, Luma([255]));
                }
            }
        }
    }
}
