use assert_cmd::Command;
use martini::converter::typst_engine::compile_typst_to_pdf;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn test_convert_pdf_to_pptx_full_pipeline() {
    let typst_src = r#"
#set page(width: 160mm, height: 90mm, margin: 10mm)
= Project Roadmap
- Phase 1: Planning
- Phase 2: Delivery

#rect(width: 40mm, height: 20mm, fill: blue)
"#;
    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("deck.pdf");
    let pptx_path = temp_dir.path().join("deck.pptx");

    let pdf_bytes = compile_typst_to_pdf(typst_src, Path::new(".")).unwrap();
    fs::write(&pdf_path, pdf_bytes).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&pdf_path)
        .arg("-o")
        .arg(&pptx_path)
        .assert()
        .success();

    assert!(pptx_path.exists());
    let pptx_bytes = fs::read(&pptx_path).unwrap();
    let mut zip = ZipArchive::new(Cursor::new(pptx_bytes)).unwrap();
    assert!(zip.by_name("ppt/slides/slide1.xml").is_ok());
}

#[test]
fn test_image_only_page_raster_fallback() {
    // Generate PDF containing only an image with no text
    let temp_dir = tempdir().unwrap();
    let img_path = temp_dir.path().join("sample.png");
    image::RgbImage::new(100, 100).save(&img_path).unwrap();
    let typst_src = "#set page(margin: 0mm)\n#image(\"sample.png\")";
    let pdf_bytes = compile_typst_to_pdf(typst_src, temp_dir.path()).unwrap();
    let pdf_path = temp_dir.path().join("scan.pdf");
    let pptx_path = temp_dir.path().join("scan.pptx");
    fs::write(&pdf_path, pdf_bytes).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&pdf_path)
        .arg("-o")
        .arg(&pptx_path)
        .arg("--json")
        .assert()
        .success();

    assert!(pptx_path.exists());
}
