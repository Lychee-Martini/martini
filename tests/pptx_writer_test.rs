use martini::converter::pptx::ir::*;
use martini::converter::pptx::writer::write_pptx;
use std::io::Cursor;
use zip::ZipArchive;

#[test]
fn test_write_pptx_produces_valid_zip_package() {
    let mut ir = SlideIR::new(9_144_000, 6_858_000); // 4:3 in EMU
    let mut slide = Slide::new(9_144_000, 6_858_000);

    // Add text box
    slide.elements.push(Element::TextBox(TextBox {
        rect: RectEmu::new(914_400, 914_400, 4_572_000, 914_400),
        paragraphs: vec![Paragraph {
            runs: vec![TextRun {
                text: "Hello Martini PPTX".to_string(),
                font_latin: "Arial".to_string(),
                font_ea: "Microsoft YaHei".to_string(),
                size_pt: 24.0,
                color: ColorRgb::new(0, 0, 0, 255),
                bold: true,
                italic: false,
                underline: false,
            }],
            line_spacing_pt: None,
            align: TextAlign::Left,
        }],
        rotation_degrees: 0.0,
    }));

    // Add basic shape
    slide.elements.push(Element::Shape(Shape {
        rect: RectEmu::new(914_400, 2_000_000, 1_000_000, 1_000_000),
        geom: ShapeGeom::Preset("rect".to_string()),
        fill: Some(ColorRgb::new(0, 120, 215, 255)),
        stroke: Some((ColorRgb::new(0, 0, 0, 255), 1.0)),
        rotation_degrees: 0.0,
    }));

    ir.slides.push(slide);

    let bytes = write_pptx(&ir).expect("Failed to write PPTX");
    assert!(bytes.len() > 1000);

    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("Must be valid ZIP");
    assert!(archive.by_name("[Content_Types].xml").is_ok());
    assert!(archive.by_name("_rels/.rels").is_ok());
    assert!(archive.by_name("ppt/presentation.xml").is_ok());
    assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
}

#[test]
fn test_review_focus_1_mixed_page_sizes_scaling() {
    let mut ir = SlideIR::new(9_144_000, 5_143_500); // Base 16:9
    let mut portrait_slide = Slide::new(5_143_500, 9_144_000); // Portrait
    portrait_slide.elements.push(Element::TextBox(TextBox {
        rect: RectEmu::new(100_000, 100_000, 500_000, 200_000),
        paragraphs: vec![],
        rotation_degrees: 0.0,
    }));
    ir.slides.push(portrait_slide);

    let bytes = write_pptx(&ir).expect("Failed to write PPTX with mixed page sizes");
    assert!(!bytes.is_empty());
}
