use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::sync::Mutex;
use tempfile::tempdir;
static PDFIUM_LOCK: Mutex<()> = Mutex::new(());

fn get_pdfium_for_test() -> &'static pdfium_render::prelude::Pdfium {
    martini::converter::pdf_conv::get_pdfium().expect("Failed to initialize PDFium for test")
}

#[test]
fn test_list_formats() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .assert()
        .success()
        .stdout(predicate::str::contains("[any] -> favicon"));
}

#[test]
fn test_list_formats_json() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"to\": \"favicon\""));
}

#[test]
fn test_convert_svg_to_ico() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("favicon.ico");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("svg")
        .arg("--to")
        .arg("favicon")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg(&output_path)
        .assert()
        .success();

    assert!(output_path.exists());
    let metadata = fs::metadata(&output_path).unwrap();
    assert!(metadata.len() > 0);
}

#[test]
fn test_convert_svg_to_package() {
    let temp_dir = tempdir().unwrap();
    let output_dir = temp_dir.path().join("icons");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("svg")
        .arg("--to")
        .arg("favicon")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg(&output_dir)
        .arg("--package")
        .assert()
        .success();

    assert!(output_dir.join("favicon.ico").exists());
    assert!(output_dir.join("favicon-16x16.png").exists());
    assert!(output_dir.join("favicon-32x32.png").exists());
    assert!(output_dir.join("apple-touch-icon.png").exists());
    assert!(output_dir.join("site.webmanifest").exists());
    assert!(output_dir.join("favicon-tags.html").exists());
}

#[test]
fn test_convert_missing_input() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("svg")
        .arg("--to")
        .arg("favicon")
        .arg("-i")
        .arg("tests/fixtures/does_not_exist.svg")
        .arg("-o")
        .arg("target/ignored.ico")
        .assert()
        .code(2) // InputFileNotFound exit code
        .stderr(predicate::str::contains("Input file not found"));
}

#[test]
fn test_convert_unsupported_formats() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("png")
        .arg("--to")
        .arg("pdf")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg("target/ignored.ico")
        .assert()
        .code(6) // UnsupportedConversion exit code
        .stderr(predicate::str::contains("Unsupported conversion"));
}

#[test]
fn test_image_conversions() {
    use image::{ImageBuffer, Rgb, Rgba};
    let temp_dir = tempdir().unwrap();
    let input_png = temp_dir.path().join("input.png");
    let input_jpg = temp_dir.path().join("input.jpg");

    // Create test images
    let png_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    png_img.save(&input_png).unwrap();

    let jpg_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    jpg_img.save(&input_jpg).unwrap();

    // 1. Single png -> webp
    let out_webp = temp_dir.path().join("output.webp");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("png")
        .arg("--to")
        .arg("webp")
        .arg("-i")
        .arg(&input_png)
        .arg("-o")
        .arg(&out_webp)
        .assert()
        .success();
    assert!(out_webp.exists());

    // 2. Single png -> avif
    let out_avif = temp_dir.path().join("output.avif");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("png")
        .arg("--to")
        .arg("avif")
        .arg("-i")
        .arg(&input_png)
        .arg("-o")
        .arg(&out_avif)
        .assert()
        .success();
    assert!(out_avif.exists());

    // 3. Batch conversion png/jpg -> both in directory
    let batch_dir = temp_dir.path().join("batch");
    fs::create_dir(&batch_dir).unwrap();
    let batch_png = batch_dir.join("img1.png");
    let batch_jpg = batch_dir.join("img2.jpg");
    png_img.save(&batch_png).unwrap();
    jpg_img.save(&batch_jpg).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--to")
        .arg("both")
        .arg("-i")
        .arg(&batch_dir)
        .arg("--quality")
        .arg("85")
        .assert()
        .success();

    assert!(batch_dir.join("img1.webp").exists());
    assert!(batch_dir.join("img1.avif").exists());
    assert!(batch_dir.join("img2.webp").exists());
    assert!(batch_dir.join("img2.avif").exists());
}

#[test]
fn test_image_conversions_delete_original() {
    use image::{ImageBuffer, Rgb};
    let temp_dir = tempdir().unwrap();
    let input_jpg = temp_dir.path().join("input.jpg");

    let jpg_img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    jpg_img.save(&input_jpg).unwrap();

    let out_webp = temp_dir.path().join("output.webp");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--to")
        .arg("webp")
        .arg("-i")
        .arg(&input_jpg)
        .arg("-o")
        .arg(&out_webp)
        .arg("--delete-original")
        .assert()
        .success();

    assert!(out_webp.exists());
    assert!(!input_jpg.exists());
}

#[test]
fn test_svg_to_raster_and_raster_to_favicon() {
    let temp_dir = tempdir().unwrap();

    // 1. Convert SVG to PNG
    let out_png = temp_dir.path().join("rendered.png");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("svg")
        .arg("--to")
        .arg("png")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg(&out_png)
        .assert()
        .success();
    assert!(out_png.exists());
    let metadata = fs::metadata(&out_png).unwrap();
    assert!(metadata.len() > 0);

    // 2. Convert PNG to Favicon
    let out_favicon = temp_dir.path().join("favicon.ico");
    let mut cmd2 = Command::cargo_bin("martini").unwrap();
    cmd2.arg("convert")
        .arg("--from")
        .arg("png")
        .arg("--to")
        .arg("favicon")
        .arg("-i")
        .arg(&out_png)
        .arg("-o")
        .arg(&out_favicon)
        .assert()
        .success();
    assert!(out_favicon.exists());
}

#[test]
fn test_auto_detect_target_format() {
    use image::{ImageBuffer, Rgba};
    let temp_dir = tempdir().unwrap();
    let input_png = temp_dir.path().join("input.png");
    let png_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    png_img.save(&input_png).unwrap();

    // 1. Auto-detect from output extension (e.g. .webp)
    let out_webp = temp_dir.path().join("output.webp");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_png)
        .arg("-o")
        .arg(&out_webp)
        .assert()
        .success();
    assert!(out_webp.exists());

    // 2. Default target format when output is omitted (in-place WebP)
    let mut cmd2 = Command::cargo_bin("martini").unwrap();
    cmd2.arg("convert")
        .arg("-i")
        .arg(&input_png)
        .assert()
        .success();
    assert!(temp_dir.path().join("input.webp").exists());
}

#[test]
fn test_convert_pdf_to_images() {
    let _lock = PDFIUM_LOCK.lock().unwrap();
    use pdfium_render::prelude::*;

    let temp_dir = tempdir().unwrap();
    let input_pdf = temp_dir.path().join("test_input.pdf");

    // 1. Create a simple PDF file with 2 blank pages
    let pdfium = get_pdfium_for_test();
    let mut document = pdfium.create_new_pdf().expect("Failed to create new PDF");
    document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::a4())
        .expect("Failed to create page 1");
    document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::a4())
        .expect("Failed to create page 2");

    // Save to file
    let pdf_bytes = document
        .save_to_bytes()
        .expect("Failed to save PDF to bytes");
    fs::write(&input_pdf, &pdf_bytes).expect("Failed to write test PDF");

    // 2. Convert PDF to PNG via CLI (all pages by default)
    let output_png = temp_dir.path().join("output.png");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("pdf")
        .arg("--to")
        .arg("png")
        .arg("-i")
        .arg(&input_pdf)
        .arg("-o")
        .arg(&output_png)
        .assert()
        .success();

    // Verify output files are created with correct suffix
    let out_page1 = temp_dir.path().join("output_page_1.png");
    let out_page2 = temp_dir.path().join("output_page_2.png");
    assert!(out_page1.exists());
    assert!(out_page2.exists());

    // 3. Convert specific page with custom DPI
    let output_jpg = temp_dir.path().join("output_single.jpg");
    let mut cmd2 = Command::cargo_bin("martini").unwrap();
    cmd2.arg("convert")
        .arg("-i")
        .arg(&input_pdf)
        .arg("-o")
        .arg(&output_jpg)
        .arg("--pages")
        .arg("2")
        .arg("--dpi")
        .arg("100")
        .assert()
        .success();

    let out_single = temp_dir.path().join("output_single_page_2.jpg");
    assert!(out_single.exists());
}

#[test]
fn test_pdfium_thread_safety() {
    let _lock = PDFIUM_LOCK.lock().unwrap();
    let _ = get_pdfium_for_test();

    let threads: Vec<_> = (0..3)
        .map(|_| {
            std::thread::spawn(|| {
                let mut cmd = Command::cargo_bin("martini").unwrap();
                cmd.arg("list-formats").assert().success();
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
}

#[test]
fn test_favicon_package_smart_path() {
    let temp_dir = tempdir().unwrap();
    let output_file_path = temp_dir.path().join("sub_folder").join("my_icon.ico");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg(&output_file_path)
        .arg("--to")
        .arg("favicon")
        .arg("--package")
        .assert()
        .success();

    // Ensure primary ICO is at the exact path
    assert!(output_file_path.exists());
    assert!(output_file_path.is_file());

    // Companion files should be in the same folder as my_icon.ico
    let parent_dir = output_file_path.parent().unwrap();
    assert!(parent_dir.join("favicon-16x16.png").exists());
    assert!(parent_dir.join("site.webmanifest").exists());
}

#[test]
fn test_convert_md_to_docx() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("output.docx");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("md")
        .arg("--to")
        .arg("docx")
        .arg("-i")
        .arg("tests/fixtures/sample.md")
        .arg("-o")
        .arg(&output_path)
        .assert()
        .success();

    assert!(output_path.exists());
    let metadata = fs::metadata(&output_path).unwrap();
    assert!(metadata.len() > 0);
}

#[test]
fn test_convert_md_to_docx_auto_detect() {
    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("output.docx");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg("tests/fixtures/sample.md")
        .arg("-o")
        .arg(&output_path)
        .assert()
        .success();

    assert!(output_path.exists());
    let metadata = fs::metadata(&output_path).unwrap();
    assert!(metadata.len() > 0);
}

#[test]
fn test_convert_glob_pattern() {
    use image::{ImageBuffer, Rgba};
    let temp_dir = tempdir().unwrap();
    let dir_path = temp_dir.path();

    // Create valid test images
    let png_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    png_img.save(dir_path.join("file1.png")).unwrap();
    png_img.save(dir_path.join("file2.png")).unwrap();
    fs::write(dir_path.join("file3.txt"), b"dummy txt").unwrap();

    let pattern = dir_path.join("*.png");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(pattern.to_string_lossy().to_string())
        .arg("--to")
        .arg("avif")
        .assert()
        .success();

    // Verify converted files exist
    assert!(dir_path.join("file1.avif").exists());
    assert!(dir_path.join("file2.avif").exists());
    assert!(!dir_path.join("file3.avif").exists());
}

#[test]
fn test_convert_glob_pattern_invalid_output() {
    use image::{ImageBuffer, Rgba};
    let temp_dir = tempdir().unwrap();
    let dir_path = temp_dir.path();

    let png_img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
    png_img.save(dir_path.join("file1.png")).unwrap();
    png_img.save(dir_path.join("file2.png")).unwrap();

    let pattern = dir_path.join("*.png");
    let invalid_output = dir_path.join("single_output.avif");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(pattern.to_string_lossy().to_string())
        .arg("-o")
        .arg(invalid_output)
        .arg("--to")
        .arg("avif")
        .assert()
        .code(3) // InvalidInputData
        .stderr(predicate::str::contains("Output path must be a directory"));
}

#[test]
fn test_cli_parsing_resizing_flags() {
    let temp_dir = tempdir().unwrap();
    let input_png = temp_dir.path().join("input.png");
    let png_img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = image::ImageBuffer::new(10, 10);
    png_img.save(&input_png).unwrap();

    let out_png = temp_dir.path().join("output.png");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_png)
        .arg("-o")
        .arg(&out_png)
        .arg("--width")
        .arg("200")
        .arg("--height")
        .arg("150")
        .arg("--no-upscale")
        .assert()
        .success();
}

#[test]
fn test_image_resize_width_only() {
    let temp_dir = tempdir().unwrap();
    let input_png = temp_dir.path().join("input.png");
    let png_img = image::ImageBuffer::from_pixel(100, 50, image::Rgba([0u8, 0u8, 0u8, 255u8]));
    png_img.save(&input_png).unwrap();

    let out_png = temp_dir.path().join("output.png");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_png)
        .arg("-o")
        .arg(&out_png)
        .arg("--width")
        .arg("50")
        .assert()
        .success();

    let output_img = image::open(&out_png).unwrap();
    assert_eq!(output_img.width(), 50);
    assert_eq!(output_img.height(), 25);
}

#[test]
fn test_svg_resize() {
    let temp_dir = tempdir().unwrap();
    let out_png = temp_dir.path().join("output.png");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .arg("-o")
        .arg(&out_png)
        .arg("--width")
        .arg("400")
        .assert()
        .success();

    let output_img = image::open(&out_png).unwrap();
    assert_eq!(output_img.width(), 400);
}

#[test]
fn test_pdf_resize() {
    let _lock = PDFIUM_LOCK.lock().unwrap();
    use pdfium_render::prelude::*;

    let temp_dir = tempdir().unwrap();
    let input_pdf = temp_dir.path().join("test_input.pdf");

    let pdfium = get_pdfium_for_test();
    let mut document = pdfium.create_new_pdf().expect("Failed to create new PDF");
    document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::a4())
        .expect("Failed to create page 1");

    let pdf_bytes = document
        .save_to_bytes()
        .expect("Failed to save PDF to bytes");
    fs::write(&input_pdf, &pdf_bytes).expect("Failed to write test PDF");

    let output_png = temp_dir.path().join("output.png");
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_pdf)
        .arg("-o")
        .arg(&output_png)
        .arg("--width")
        .arg("300")
        .assert()
        .success();

    let out_page = temp_dir.path().join("output_page_1.png");
    assert!(out_page.exists());
    let output_img = image::open(&out_page).unwrap();
    assert_eq!(output_img.width(), 300);
}

#[test]
fn test_convert_md_to_pdf() {
    let temp_dir = tempdir().unwrap();
    let output_pdf = temp_dir.path().join("output.pdf");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("--from")
        .arg("md")
        .arg("--to")
        .arg("pdf")
        .arg("-i")
        .arg("tests/fixtures/sample.md")
        .arg("-o")
        .arg(&output_pdf)
        .assert()
        .success();

    assert!(output_pdf.exists());
    let bytes = fs::read(&output_pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    assert!(bytes.len() > 100);
}

#[test]
fn test_convert_md_to_pdf_default_target() {
    let temp_dir = tempdir().unwrap();
    let input_md = temp_dir.path().join("document.md");
    fs::copy("tests/fixtures/sample.md", &input_md).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_md)
        .assert()
        .success();

    let expected_pdf = temp_dir.path().join("document.pdf");
    assert!(expected_pdf.exists());
    let bytes = fs::read(&expected_pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn test_convert_md_to_pdf_with_options() {
    let temp_dir = tempdir().unwrap();
    let output_pdf = temp_dir.path().join("styled.pdf");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg("tests/fixtures/sample.md")
        .arg("-o")
        .arg(&output_pdf)
        .arg("--theme")
        .arg("modern")
        .arg("--paper-size")
        .arg("letter")
        .arg("--margin")
        .arg("15mm")
        .arg("--header")
        .arg("Confidential Document")
        .arg("--footer")
        .arg("Company Internal")
        .arg("--toc")
        .assert()
        .success();

    assert!(output_pdf.exists());
    let bytes = fs::read(&output_pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}

#[test]
fn test_convert_md_to_pdf_all_themes() {
    let temp_dir = tempdir().unwrap();
    for theme in ["github", "academic", "modern", "minimal"] {
        let output_pdf = temp_dir.path().join(format!("{}.pdf", theme));
        let mut cmd = Command::cargo_bin("martini").unwrap();
        cmd.arg("convert")
            .arg("-i")
            .arg("tests/fixtures/sample.md")
            .arg("-o")
            .arg(&output_pdf)
            .arg("--theme")
            .arg(theme)
            .assert()
            .success();

        assert!(output_pdf.exists());
        let bytes = fs::read(&output_pdf).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }
}

#[test]
fn test_batch_convert_md_to_pdf() {
    let temp_dir = tempdir().unwrap();
    let input_dir = temp_dir.path().join("docs");
    let output_dir = temp_dir.path().join("out_pdfs");
    fs::create_dir_all(&input_dir).unwrap();

    fs::copy("tests/fixtures/sample.md", input_dir.join("doc1.md")).unwrap();
    fs::copy("tests/fixtures/sample.md", input_dir.join("doc2.md")).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg(&input_dir)
        .arg("-o")
        .arg(&output_dir)
        .arg("--from")
        .arg("md")
        .arg("--to")
        .arg("pdf")
        .assert()
        .success();

    assert!(output_dir.join("doc1.pdf").exists());
    assert!(output_dir.join("doc2.pdf").exists());
    let bytes1 = fs::read(output_dir.join("doc1.pdf")).unwrap();
    assert!(bytes1.starts_with(b"%PDF"));
}

#[test]
fn test_convert_positional_pdf_default_output() {
    let _lock = PDFIUM_LOCK.lock().unwrap();
    use pdfium_render::prelude::*;

    let temp_dir = tempdir().unwrap();
    let input_pdf = temp_dir.path().join("test_doc.pdf");

    let pdfium = get_pdfium_for_test();
    let mut document = pdfium.create_new_pdf().expect("Failed to create new PDF");
    document
        .pages_mut()
        .create_page_at_end(PdfPagePaperSize::a4())
        .expect("Failed to create page 1");

    let pdf_bytes = document
        .save_to_bytes()
        .expect("Failed to save PDF to bytes");
    fs::write(&input_pdf, &pdf_bytes).expect("Failed to write test PDF");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert").arg(&input_pdf).assert().success();

    let out_page = temp_dir.path().join("test_doc_page_1.png");
    assert!(out_page.exists());
    let output_img = image::open(&out_page).unwrap();
    assert!(output_img.width() > 0);
}

#[test]
fn test_convert_positional_input_and_output() {
    let temp_dir = tempdir().unwrap();
    let out_png = temp_dir.path().join("positional_output.png");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("tests/fixtures/sample.svg")
        .arg(&out_png)
        .assert()
        .success();

    assert!(out_png.exists());
    let output_img = image::open(&out_png).unwrap();
    assert!(output_img.width() > 0);
}

#[test]
fn test_default_subcommand_positional() {
    let temp_dir = tempdir().unwrap();
    let out_png = temp_dir.path().join("default_subcmd_out.png");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("tests/fixtures/sample.svg")
        .arg(&out_png)
        .assert()
        .success();

    assert!(out_png.exists());
}

#[test]
fn test_positional_and_flag_input_conflict() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("tests/fixtures/sample.svg")
        .arg("-i")
        .arg("tests/fixtures/sample.svg")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_positional_and_flag_output_conflict() {
    let temp_dir = tempdir().unwrap();
    let out1 = temp_dir.path().join("out1.png");
    let out2 = temp_dir.path().join("out2.png");

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("tests/fixtures/sample.svg")
        .arg(&out1)
        .arg("-o")
        .arg(&out2)
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with"));
}

#[test]
fn test_no_args_exits_zero() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Usage: martini"));
}

#[test]
fn test_list_formats_shows_data_and_archive() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "[json/yaml/toml/csv/tsv] -> [json/yaml/toml/csv/tsv]",
        ))
        .stdout(predicate::str::contains(
            "[zip/tar/gz/tar.gz] -> [zip/tar/gz/tar.gz]",
        ));
}

#[test]
fn test_cli_convert_data_json_to_yaml() {
    let temp_dir = tempdir().unwrap();
    let json_file = temp_dir.path().join("input.json");
    let yaml_file = temp_dir.path().join("output.yaml");
    fs::write(&json_file, r#"{"name": "martini", "awesome": true}"#).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg(&json_file)
        .arg(&yaml_file)
        .assert()
        .success();

    assert!(yaml_file.exists());
    let content = fs::read_to_string(&yaml_file).unwrap();
    assert!(content.contains("name: martini"));
    assert!(content.contains("awesome: true"));
}

#[test]
fn test_cli_convert_archive_tar_to_zip() {
    let temp_dir = tempdir().unwrap();
    let tar_file = temp_dir.path().join("archive.tar");
    let zip_file = temp_dir.path().join("archive.zip");

    {
        let mut tar_builder = tar::Builder::new(fs::File::create(&tar_file).unwrap());
        let mut header = tar::Header::new_gnu();
        let data = b"martini archive test";
        header.set_size(data.len() as u64);
        header.set_cksum();
        tar_builder
            .append_data(&mut header, "test.txt", &data[..])
            .unwrap();
        tar_builder.finish().unwrap();
    }

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg(&tar_file)
        .arg(&zip_file)
        .assert()
        .success();

    assert!(zip_file.exists());
    let file = fs::File::open(&zip_file).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut entry = zip.by_name("test.txt").expect("test.txt must exist in zip");
    use std::io::Read;
    let mut content = String::new();
    entry.read_to_string(&mut content).unwrap();
    assert_eq!(content, "martini archive test");
}

#[test]
fn test_cli_convert_pdf_to_pptx() {
    let _guard = PDFIUM_LOCK.lock().unwrap();
    let _pdfium = get_pdfium_for_test();

    let temp_dir = tempdir().unwrap();
    let pdf_path = temp_dir.path().join("presentation.pdf");
    let pptx_path = temp_dir.path().join("presentation.pptx");

    let typst_src = "#set page(width: 120mm, height: 80mm)\n= Martini PPTX\nHello slides!";
    let pdf_bytes = martini::converter::typst_engine::compile_typst_to_pdf(
        typst_src,
        std::path::Path::new("."),
    )
    .unwrap();
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
    let metadata = fs::metadata(&pptx_path).unwrap();
    assert!(metadata.len() > 0);
}
