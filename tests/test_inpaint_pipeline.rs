use image::{DynamicImage, GrayImage, RgbaImage};
use martini::erase::pipeline::ClassicalInpainter;

#[test]
fn test_classical_inpainter_smoke() {
    let img = DynamicImage::ImageRgba8(RgbaImage::new(32, 32));
    let mask = GrayImage::new(32, 32);
    let inpainter = ClassicalInpainter::new();
    let res = inpainter.inpaint(&img, &mask, 10).unwrap();
    assert_eq!(res.width(), 32);
    assert_eq!(res.height(), 32);
}
