use image::{DynamicImage, GenericImageView, GrayImage, Luma, Rgba, RgbaImage};

#[test]
fn test_blend_result_preserves_unmasked_pixels() {
    let original =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(100, 100, Rgba([255, 0, 0, 255])));
    let patch = DynamicImage::ImageRgba8(RgbaImage::from_pixel(50, 50, Rgba([0, 255, 0, 255])));
    let mut mask = GrayImage::new(100, 100);
    // Mask only center region: from (40, 40) to (60, 60)
    for x in 40..60 {
        for y in 40..60 {
            mask.put_pixel(x, y, Luma([255]));
        }
    }

    let crop = (25, 25, 50, 50);
    let blended =
        martini::erase::pipeline::InpaintPipeline::blend_result(&original, &patch, &mask, crop)
            .unwrap();

    // Pixel far outside the crop should remain original red: [255, 0, 0, 255]
    assert_eq!(blended.get_pixel(10, 10), Rgba([255, 0, 0, 255]));

    // Pixel fully inside the masked center (e.g. 50, 50) should be inpainted green: [0, 255, 0, 255]
    assert_eq!(blended.get_pixel(50, 50), Rgba([0, 255, 0, 255]));

    // Pixel inside the crop but unmasked (e.g. 30, 30) should remain original red: [255, 0, 0, 255]
    assert_eq!(blended.get_pixel(30, 30), Rgba([255, 0, 0, 255]));
}

#[test]
fn test_blend_result_dimensions_match_original() {
    let original =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(120, 80, Rgba([100, 100, 100, 255])));
    let patch = DynamicImage::ImageRgba8(RgbaImage::from_pixel(40, 40, Rgba([200, 200, 200, 255])));
    let mask = GrayImage::new(120, 80);
    let crop = (10, 10, 40, 40);

    let blended =
        martini::erase::pipeline::InpaintPipeline::blend_result(&original, &patch, &mask, crop)
            .unwrap();

    assert_eq!(blended.width(), 120);
    assert_eq!(blended.height(), 80);
}

#[test]
fn test_inpaint_replaces_masked_pixels_with_background() {
    // 60x60 white image with black box in center
    let mut img = RgbaImage::from_pixel(60, 60, Rgba([255, 255, 255, 255]));
    let mut mask = GrayImage::new(60, 60);

    for y in 25..35 {
        for x in 25..35 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            mask.put_pixel(x, y, Luma([255]));
        }
    }

    let dyn_img = DynamicImage::ImageRgba8(img);
    let paths = martini::erase::models::ModelPaths {
        clip_weights: std::path::PathBuf::new(),
        tokenizer: std::path::PathBuf::new(),
        unet_weights: std::path::PathBuf::new(),
        vae_weights: std::path::PathBuf::new(),
        text_encoder_weights: std::path::PathBuf::new(),
    };
    let pipeline = martini::erase::pipeline::InpaintPipeline::load(&paths).unwrap();
    let inpainted = pipeline.inpaint(&dyn_img, &mask, 15).unwrap();

    // Center pixel should now be filled with surrounding white (> 250)
    let center_px = inpainted.get_pixel(30, 30);
    assert!(center_px[0] > 250);
    assert!(center_px[1] > 250);
    assert!(center_px[2] > 250);
}
