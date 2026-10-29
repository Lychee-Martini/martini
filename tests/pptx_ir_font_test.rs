use martini::converter::pptx::font::resolve_font_names;
use martini::converter::pptx::ir::*;

#[test]
fn test_font_resolution_cjk_fallback() {
    let font_pair = resolve_font_names("SimSun-Bold", true);
    assert_eq!(font_pair.east_asian, "SimSun");

    let yahei_pair = resolve_font_names("CustomHeitiFont", true);
    assert!(
        yahei_pair.east_asian == "Microsoft YaHei"
            || yahei_pair.east_asian == "PingFang SC"
            || yahei_pair.east_asian == "Noto Sans CJK SC"
    );
}

#[test]
fn test_slide_ir_dimensions() {
    let mut slide = Slide::new(9_144_000, 5_143_500); // 16:9 in EMU
    let tb = TextBox {
        rect: RectEmu::new(100_000, 100_000, 500_000, 200_000),
        paragraphs: vec![],
        rotation_degrees: 0.0,
    };
    slide.elements.push(Element::TextBox(tb));
    assert_eq!(slide.elements.len(), 1);
}
