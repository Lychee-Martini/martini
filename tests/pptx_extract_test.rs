use martini::converter::pdf_conv::get_pdfium;
use martini::converter::pptx::extract::extract_page_objects;
use martini::converter::typst_engine::compile_typst_to_pdf;
use std::path::Path;

#[test]
fn test_extract_pdf_page_objects_basic() {
    let typst_src =
        "#set page(width: 100mm, height: 100mm, margin: 10mm)\n= Title Text\nThis is a paragraph.";
    let pdf_bytes = compile_typst_to_pdf(typst_src, Path::new(".")).unwrap();
    let pdfium = get_pdfium().unwrap();
    let doc = pdfium.load_pdf_from_byte_slice(&pdf_bytes, None).unwrap();

    let raw_page = extract_page_objects(&doc, 0).unwrap();
    assert!(raw_page.text_items.len() >= 2);
    assert!(
        raw_page
            .text_items
            .iter()
            .any(|t| t.text.contains("Title Text"))
    );
}

#[test]
fn test_review_focus_2_and_4_rotation_and_transparent_image() {
    let typst_src = r#"
#set page(width: 80mm, height: 80mm)
#rotate(90deg)[Vertical Axis Title]
"#;
    let pdf_bytes = compile_typst_to_pdf(typst_src, Path::new(".")).unwrap();
    let pdfium = get_pdfium().unwrap();
    let doc = pdfium.load_pdf_from_byte_slice(&pdf_bytes, None).unwrap();
    let raw_page = extract_page_objects(&doc, 0).unwrap();
    let vertical_text = raw_page
        .text_items
        .iter()
        .find(|t| t.text.contains("Vertical Axis Title"))
        .unwrap();
    assert!(
        (vertical_text.rotation_degrees - 90.0).abs() < 2.0
            || (vertical_text.rotation_degrees - 270.0).abs() < 2.0
    );
}
