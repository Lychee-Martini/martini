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
