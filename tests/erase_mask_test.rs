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

#[test]
fn test_detect_text_rejects_colored_stems_and_background_gaps() {
    let mut img = image::RgbImage::from_pixel(240, 180, image::Rgb([218, 218, 215]));

    // Two dark green leaves/stems (high contrast > 160 against white cloth, but chromatic)
    // separated by a 6px white cloth gap at y = 76..82
    for y in 68..76 {
        for x in 40..110 {
            img.put_pixel(x, y, image::Rgb([26, 56, 24]));
        }
    }
    for y in 82..90 {
        for x in 40..110 {
            img.put_pixel(x, y, image::Rgb([26, 56, 24]));
        }
    }

    // Neutral black text stroke at x = 160..163, y = 120..145
    for y in 120..145 {
        for x in 160..163 {
            img.put_pixel(x, y, image::Rgb([12, 12, 12]));
        }
    }

    let dyn_img = image::DynamicImage::ImageRgb8(img);
    let mask = martini::erase::mask::detect_text_mask(&dyn_img, 0.35);

    // True black text stroke must be detected
    assert_eq!(mask.get_pixel(161, 130)[0], 255);
    // Dark green leaves must NOT be detected despite high contrast against cloth
    assert_eq!(mask.get_pixel(75, 72)[0], 0);
    assert_eq!(mask.get_pixel(75, 86)[0], 0);
    // Narrow white cloth gap between the two dark green leaves must NOT be detected
    assert_eq!(mask.get_pixel(75, 79)[0], 0);
}

#[test]
fn test_compute_inpaint_rois_splits_scattered_regions() {
    let mut mask = GrayImage::new(1024, 768);
    // Bottom-left label (like "01")
    for y in 660..688 {
        for x in 136..170 {
            mask.put_pixel(x, y, Luma([255]));
        }
    }
    // Top-right label (like Chinese title)
    for y in 60..90 {
        for x in 883..975 {
            mask.put_pixel(x, y, Luma([255]));
        }
    }

    // Neural 512x512 context windows
    let sd_rois = martini::erase::mask::compute_inpaint_rois(&mask, 512, 64);
    assert_eq!(sd_rois.len(), 2);
    for &(rx, ry, rw, rh) in &sd_rois {
        assert_eq!((rw, rh), (512, 512));
        assert!(rx + rw <= 1024);
        assert!(ry + rh <= 768);
    }

    // Tight local boxes for fast classical mode
    let fast_rois = martini::erase::mask::compute_inpaint_rois(&mask, 0, 16);
    assert_eq!(fast_rois.len(), 2);
    for &(_rx, _ry, rw, rh) in &fast_rois {
        assert!(rw < 160 && rh < 100);
    }
}

#[test]
fn test_detect_text_high_resolution_thick_strokes() {
    // High-resolution 2400x1800 canvas (simulating 4K/12MP product photography)
    // with a 20px-wide neutral black digit stroke (> 10px old morphological limit)
    // alongside chromatic dark green leaves separated by a 12px bright cloth gap.
    let mut img = image::RgbImage::from_pixel(2400, 1800, image::Rgb([218, 218, 215]));

    // Chromatic dark green leaves with a 12px cloth gap at y = 720..732
    for y in 690..720 {
        for x in 400..850 {
            img.put_pixel(x, y, image::Rgb([26, 56, 24]));
        }
    }
    for y in 732..762 {
        for x in 400..850 {
            img.put_pixel(x, y, image::Rgb([26, 56, 24]));
        }
    }

    // Thick 20px-wide neutral black digit stroke at x = 1500..1520, y = 1200..1310
    for y in 1200..1310 {
        for x in 1500..1520 {
            img.put_pixel(x, y, image::Rgb([14, 14, 14]));
        }
    }

    // Thin 4px-wide neutral black hairline stroke at x = 1520..1560, y = 1270..1274
    for y in 1270..1274 {
        for x in 1520..1560 {
            img.put_pixel(x, y, image::Rgb([16, 16, 16]));
        }
    }

    let dyn_img = image::DynamicImage::ImageRgb8(img);
    let mask = martini::erase::mask::detect_text_mask(&dyn_img, 0.35);

    // Both thick 20px vertical stem and thin 4px crossbar must be detected
    assert_eq!(mask.get_pixel(1510, 1255)[0], 255);
    assert_eq!(mask.get_pixel(1540, 1272)[0], 255);

    // Chromatic green leaves and the cloth gap between them must remain 0
    assert_eq!(mask.get_pixel(600, 705)[0], 0);
    assert_eq!(mask.get_pixel(600, 726)[0], 0);
    assert_eq!(mask.get_pixel(600, 745)[0], 0);
}

#[test]
fn test_plan_neural_inpaint_batches_packs_4k_scattered_labels() {
    use martini::erase::mask::{InpaintBatch, plan_neural_inpaint_batches};

    // Simulate 7 scattered text labels on a 4032x3024 image (like 01..06 + top-right caption)
    let mut mask = GrayImage::new(4032, 3024);
    let boxes = [
        (685, 1210, 150, 115),
        (1625, 1210, 150, 115),
        (2635, 1210, 150, 115),
        (525, 2600, 140, 115),
        (1845, 2600, 150, 115),
        (3165, 2600, 150, 115),
        (3470, 230, 380, 130),
    ];
    for (bx, by, bw, bh) in boxes {
        for y in by..(by + bh) {
            for x in bx..(bx + bw) {
                mask.put_pixel(x, y, Luma([255]));
            }
        }
    }

    let batches = plan_neural_inpaint_batches(&mask, true);
    // 7 scattered labels pack into just 2 diffusion passes (4 + 3 quadrants) instead of 7+ passes
    assert_eq!(batches.len(), 2);
    let total_packed: usize = batches
        .iter()
        .map(|b| match b {
            InpaintBatch::Quadrants(q) => q.len(),
            InpaintBatch::Single(_) => 1,
        })
        .sum();
    assert_eq!(total_packed, 7);
}
