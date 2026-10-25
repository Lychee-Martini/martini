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
