use image::{GenericImageView, GrayImage, Luma};

#[test]
fn test_dilate_mask_expands_boundaries() {
    let mut mask = GrayImage::new(10, 10);
    mask.put_pixel(5, 5, Luma([255]));
    let dilated = martini::erase::mask::dilate_mask(&mask, 1);
    assert_eq!(dilated.get_pixel(5, 5)[0], 255);
    assert_eq!(dilated.get_pixel(4, 5)[0], 255);
    assert_eq!(dilated.get_pixel(6, 5)[0], 255);
    assert_eq!(dilated.get_pixel(5, 4)[0], 255);
    assert_eq!(dilated.get_pixel(5, 6)[0], 255);
    assert_eq!(dilated.get_pixel(0, 0)[0], 0);
}

#[test]
fn test_bounding_box_empty_mask() {
    let mask = GrayImage::new(10, 10);
    assert!(martini::erase::mask::compute_bounding_box(&mask, 2).is_none());
}

#[test]
fn test_bounding_box_with_padding() {
    let mut mask = GrayImage::new(20, 20);
    mask.put_pixel(10, 10, Luma([255]));
    let bbox = martini::erase::mask::compute_bounding_box(&mask, 3).unwrap();
    assert_eq!(bbox, (7, 7, 7, 7)); // x, y, width, height
}

#[test]
fn test_feather_mask_smooths_edges() {
    let mut mask = GrayImage::new(10, 10);
    for x in 3..7 {
        for y in 3..7 {
            mask.put_pixel(x, y, Luma([255]));
        }
    }
    let feathered = martini::erase::mask::feather_mask(&mask, 1);
    // Center pixel should be near max
    assert!(feathered.get_pixel(5, 5)[0] > 200);
    // Boundary pixel at 3,3 should be intermediate
    let edge_val = feathered.get_pixel(3, 3)[0];
    assert!(edge_val > 0 && edge_val < 255);
}

#[test]
fn test_detect_text_synthetic_stroke() {
    // 50x50 white background with black thin stroke in center
    let mut img = image::RgbImage::from_pixel(50, 50, image::Rgb([240, 240, 240]));
    for y in 20..30 {
        // 2-pixel wide stroke
        img.put_pixel(25, y, image::Rgb([10, 10, 10]));
        img.put_pixel(26, y, image::Rgb([10, 10, 10]));
    }
    let dyn_img = image::DynamicImage::ImageRgb8(img);
    let mask = martini::erase::mask::detect_text_mask(&dyn_img, 0.35);

    // Pixels on the stroke should be detected
    assert_eq!(mask.get_pixel(25, 25)[0], 255);
    assert_eq!(mask.get_pixel(26, 25)[0], 255);
    // Background pixels far away should be 0
    assert_eq!(mask.get_pixel(5, 5)[0], 0);
    assert_eq!(mask.get_pixel(45, 45)[0], 0);
}

#[test]
fn test_detect_text_on_sample_image() {
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("sample_text_overlay.jpg");
    assert!(
        fixture_path.exists(),
        "sample fixture not found at {}",
        fixture_path.display()
    );

    let img = image::open(&fixture_path).expect("failed to open sample_text_overlay.jpg");
    let mask = martini::erase::mask::detect_text_mask(&img, 0.35);

    let mut positive_count = 0;
    for pixel in mask.pixels() {
        if pixel[0] > 0 {
            positive_count += 1;
        }
    }
    assert!(positive_count > 100);

    // Inpaint in memory and verify text area was replaced with clean background
    let inpainted = martini::erase::pipeline::inpaint_classical(&img, &mask, 15).unwrap();

    let (w, h) = img.dimensions();
    let mut diff_count = 0;
    let mut sum_orig_luma: u64 = 0;
    let mut sum_erased_luma: u64 = 0;
    for y in 0..h {
        for x in 0..w {
            let p1 = img.get_pixel(x, y);
            let p2 = inpainted.get_pixel(x, y);
            let diff = (p1[0] as i32 - p2[0] as i32).abs()
                + (p1[1] as i32 - p2[1] as i32).abs()
                + (p1[2] as i32 - p2[2] as i32).abs();
            if diff > 30 {
                diff_count += 1;
                sum_orig_luma += (p1[0] as u64 + p1[1] as u64 + p1[2] as u64) / 3;
                sum_erased_luma += (p2[0] as u64 + p2[1] as u64 + p2[2] as u64) / 3;
            }
        }
    }
    let avg_orig = sum_orig_luma.checked_div(diff_count).unwrap_or(0);
    let avg_erased = sum_erased_luma.checked_div(diff_count).unwrap_or(0);
    assert!(diff_count > 1000);
    assert!(avg_erased > avg_orig + 50);
}
