use martini::converter::{ConvertOptions, Format, convert};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_bmp_and_qoi_conversions() {
    let dir = tempdir().unwrap();
    let png_file = dir.path().join("pixel.png");
    let bmp_file = dir.path().join("pixel.bmp");
    let qoi_file = dir.path().join("pixel.qoi");

    // 1x1 red PNG
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    img.save(&png_file).unwrap();

    let png_bytes = fs::read(&png_file).unwrap();
    let opt = ConvertOptions {
        output_path: bmp_file.clone(),
        ..Default::default()
    };
    convert(Format::Png, Format::Bmp, &png_bytes, &opt).unwrap();
    assert!(bmp_file.exists());

    let bmp_bytes = fs::read(&bmp_file).unwrap();
    let opt_qoi = ConvertOptions {
        output_path: qoi_file.clone(),
        ..Default::default()
    };
    convert(Format::Bmp, Format::Qoi, &bmp_bytes, &opt_qoi).unwrap();
    assert!(qoi_file.exists());
}
