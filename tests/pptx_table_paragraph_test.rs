use martini::converter::pptx::extract::{RawPathItem, RawTextItem};
use martini::converter::pptx::ir::RectEmu;
use martini::converter::pptx::paragraph::cluster_paragraphs;
use martini::converter::pptx::table::detect_tables;

#[test]
fn test_paragraph_clustering_merges_adjacent_lines() {
    let line1 = RawTextItem {
        text: "First line of a paragraph ".to_string(),
        rect: RectEmu::new(100_000, 100_000, 300_000, 20_000),
        font_name: "Arial".to_string(),
        font_size_pt: 12.0,
        color_rgba: (0, 0, 0, 255),
        bold: false,
        italic: false,
        rotation_degrees: 0.0,
        render_mode_invisible: false,
    };
    let line2 = RawTextItem {
        text: "second line continuing.".to_string(),
        rect: RectEmu::new(100_000, 125_000, 280_000, 20_000),
        font_name: "Arial".to_string(),
        font_size_pt: 12.0,
        color_rgba: (0, 0, 0, 255),
        bold: false,
        italic: false,
        rotation_degrees: 0.0,
        render_mode_invisible: false,
    };
    let textboxes = cluster_paragraphs(vec![line1, line2]);
    assert_eq!(textboxes.len(), 1);
    assert_eq!(textboxes[0].paragraphs[0].runs.len(), 2);
    assert_eq!(
        textboxes[0].paragraphs[0].runs[0].text,
        "First line of a paragraph "
    );
    assert_eq!(
        textboxes[0].paragraphs[0].runs[1].text,
        "second line continuing."
    );
}

#[test]
fn test_detect_ruled_table() {
    // 2x2 grid of horizontal and vertical line paths
    let mut paths = vec![
        RawPathItem::horizontal_line(100_000, 100_000, 400_000),
        RawPathItem::horizontal_line(100_000, 200_000, 400_000),
        RawPathItem::horizontal_line(100_000, 300_000, 400_000),
        RawPathItem::vertical_line(100_000, 100_000, 200_000),
        RawPathItem::vertical_line(300_000, 100_000, 200_000),
        RawPathItem::vertical_line(500_000, 100_000, 200_000),
    ];
    let mut text_items = vec![
        RawTextItem::simple("Cell A", 120_000, 120_000),
        RawTextItem::simple("Cell B", 320_000, 120_000),
        RawTextItem::simple("Cell C", 120_000, 220_000),
        RawTextItem::simple("Cell D", 320_000, 220_000),
    ];
    let tables = detect_tables(&mut paths, &mut text_items);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].rows.len(), 2);
    assert_eq!(tables[0].col_widths.len(), 2);
    assert!(
        text_items.is_empty(),
        "Consumed table text items should be removed"
    );
}
