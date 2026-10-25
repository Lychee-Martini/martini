use image::{GrayImage, Luma};

#[derive(Debug, Clone)]
pub struct LocalizedMask {
    pub mask: GrayImage,
    pub bounding_box: Option<(u32, u32, u32, u32)>, // (x, y, width, height)
    pub matched: bool,
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

/// Applies a fast box blur to soften/feather mask edges
pub fn feather_mask(mask: &GrayImage, radius: u32) -> GrayImage {
    if radius == 0 {
        return mask.clone();
    }

    let (width, height) = mask.dimensions();
    let mut result = GrayImage::new(width, height);
    let r_i32 = radius as i32;

    for y in 0..height {
        for x in 0..width {
            let x_min = (x as i32 - r_i32).max(0) as u32;
            let x_max = (x as i32 + r_i32).min(width as i32 - 1) as u32;
            let y_min = (y as i32 - r_i32).max(0) as u32;
            let y_max = (y as i32 + r_i32).min(height as i32 - 1) as u32;

            let mut sum: u32 = 0;
            let mut count: u32 = 0;

            for ny in y_min..=y_max {
                for nx in x_min..=x_max {
                    sum += mask.get_pixel(nx, ny)[0] as u32;
                    count += 1;
                }
            }

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

/// Dilates a grayscale image with a separable square kernel of radius `radius`
pub fn grayscale_dilate(img: &GrayImage, radius: u32) -> GrayImage {
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
            let mut max_val = 0u8;
            for nx in x_start..=x_end {
                let v = img.get_pixel(nx, y)[0];
                if v > max_val {
                    max_val = v;
                }
            }
            tmp.put_pixel(x, y, Luma([max_val]));
        }
    }

    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        let y_start = (y as i32 - r).max(0) as u32;
        let y_end = (y as i32 + r).min(h as i32 - 1) as u32;
        for x in 0..w {
            let mut max_val = 0u8;
            for ny in y_start..=y_end {
                let v = tmp.get_pixel(x, ny)[0];
                if v > max_val {
                    max_val = v;
                }
            }
            out.put_pixel(x, y, Luma([max_val]));
        }
    }
    out
}

/// Erodes a grayscale image with a separable square kernel of radius `radius`
pub fn grayscale_erode(img: &GrayImage, radius: u32) -> GrayImage {
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
            let mut min_val = 255u8;
            for nx in x_start..=x_end {
                let v = img.get_pixel(nx, y)[0];
                if v < min_val {
                    min_val = v;
                }
            }
            tmp.put_pixel(x, y, Luma([min_val]));
        }
    }

    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        let y_start = (y as i32 - r).max(0) as u32;
        let y_end = (y as i32 + r).min(h as i32 - 1) as u32;
        for x in 0..w {
            let mut min_val = 255u8;
            for ny in y_start..=y_end {
                let v = tmp.get_pixel(x, ny)[0];
                if v < min_val {
                    min_val = v;
                }
            }
            out.put_pixel(x, y, Luma([min_val]));
        }
    }
    out
}

/// Detects high-contrast text and thin overlay annotations (strokes, watermarks, dimensions)
pub fn detect_text_mask(img: &image::DynamicImage, threshold: f32) -> GrayImage {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();

    // Structuring element radius for stroke detection (typically 4-6px captures stroke widths up to 10-12px)
    let radius = 5u32;

    // Morphological closing: dilation then erosion (fills thin dark strokes)
    let dilated = grayscale_dilate(&gray, radius);
    let closed = grayscale_erode(&dilated, radius);

    // Morphological opening: erosion then dilation (suppresses thin bright strokes)
    let eroded = grayscale_erode(&gray, radius);
    let opened = grayscale_dilate(&eroded, radius);

    let thresh_u8 = ((threshold.clamp(0.05, 0.95)) * 255.0).round() as u8;

    let rgb = img.to_rgb8();
    let mut raw_mask = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let orig = gray.get_pixel(x, y)[0];
            let c_val = closed.get_pixel(x, y)[0];
            let o_val = opened.get_pixel(x, y)[0];

            let black_tophat = c_val.saturating_sub(orig);
            let white_tophat = orig.saturating_sub(o_val);
            let diff = black_tophat.max(white_tophat);

            let px = rgb.get_pixel(x, y);
            let color_diff = (px[0] as i32 - px[1] as i32).abs()
                + (px[1] as i32 - px[2] as i32).abs()
                + (px[2] as i32 - px[0] as i32).abs();
            let is_neutral = color_diff < 50;

            if diff >= thresh_u8 && (is_neutral || diff > 160) {
                raw_mask.put_pixel(x, y, Luma([255]));
            }
        }
    }

    // Filter tiny isolated speckles and structural image seams
    filter_small_components(&raw_mask, 12)
}

/// Removes connected components with fewer than `min_size` pixels, or components spanning full image width
fn filter_small_components(mask: &GrayImage, min_size: usize) -> GrayImage {
    let (w, h) = mask.dimensions();
    let mut visited = vec![false; (w * h) as usize];
    let mut out = GrayImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if mask.get_pixel(x, y)[0] == 255 && !visited[idx] {
                // BFS to collect component
                let mut component = Vec::new();
                let mut queue = std::collections::VecDeque::new();
                queue.push_back((x, y));
                visited[idx] = true;

                let mut min_x = x;
                let mut max_x = x;
                let mut min_y = y;
                let mut max_y = y;

                while let Some((cx, cy)) = queue.pop_front() {
                    component.push((cx, cy));
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
                                if mask.get_pixel(nx as u32, ny as u32)[0] == 255 && !visited[n_idx]
                                {
                                    visited[n_idx] = true;
                                    queue.push_back((nx as u32, ny as u32));
                                }
                            }
                        }
                    }
                }

                let comp_w = max_x - min_x + 1;
                let comp_h = max_y - min_y + 1;

                // Discard full-width horizontal seams (> 70% of image width) or full-height image borders (> 90%)
                if comp_w > (w * 7) / 10 || comp_h > (h * 9) / 10 {
                    continue;
                }

                if component.len() >= min_size {
                    for (cx, cy) in component {
                        out.put_pixel(cx, cy, Luma([255]));
                    }
                }
            }
        }
    }

    out
}
