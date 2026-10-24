use martini::converter::engine::EngineRegistry;
use martini::converter::{ConvertOptions, Format};

use tempfile::tempdir;

#[test]
fn test_bmp_and_qoi_conversions() {
    let registry = EngineRegistry::default();
    let dir = tempdir().unwrap();
    let png_file = dir.path().join("pixel.png");
    let bmp_file = dir.path().join("pixel.bmp");
    let qoi_file = dir.path().join("pixel.qoi");

    // 1x1 red PNG
    let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
    img.save(&png_file).unwrap();

    let opt = ConvertOptions::default();
    let engine = registry
        .find_engine(Format::Png, Format::Bmp)
        .expect("ImageEngine must support PNG -> BMP");
    engine.convert(&png_file, &bmp_file, &opt).unwrap();
    assert!(bmp_file.exists());

    let qoi_engine = registry
        .find_engine(Format::Bmp, Format::Qoi)
        .expect("ImageEngine must support BMP -> QOI");
    qoi_engine.convert(&bmp_file, &qoi_file, &opt).unwrap();
    assert!(qoi_file.exists());
}
