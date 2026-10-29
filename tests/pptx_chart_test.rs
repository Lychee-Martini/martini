use martini::converter::pptx::chart::detect_charts;
use martini::converter::pptx::extract::{RawPathItem, RawTextItem};
use martini::converter::pptx::ir::{ChartType, Element};

#[test]
fn test_detect_column_chart_with_ticks() {
    // Synthetic vertical axis with ticks: 0, 50, 100
    // Two columns with heights corresponding to values 40 and 80
    // Categories: "Q1", "Q2"
    let mut paths = vec![
        RawPathItem::vertical_line(100_000, 100_000, 300_000), // axis
        RawPathItem::horizontal_line(100_000, 400_000, 400_000), // baseline
        RawPathItem::rect(150_000, 280_000, 50_000, 120_000),  // col 1
        RawPathItem::rect(250_000, 160_000, 50_000, 240_000),  // col 2
    ];
    let mut text_items = vec![
        RawTextItem::simple("100", 70_000, 100_000),
        RawTextItem::simple("50", 70_000, 250_000),
        RawTextItem::simple("0", 70_000, 400_000),
        RawTextItem::simple("Q1", 160_000, 420_000),
        RawTextItem::simple("Q2", 260_000, 420_000),
    ];

    let elements = detect_charts(&mut paths, &mut text_items);
    assert_eq!(elements.len(), 1);
    match &elements[0] {
        Element::Chart(c) => {
            assert_eq!(c.chart_type, ChartType::Column);
            assert_eq!(c.categories, vec!["Q1", "Q2"]);
            assert_eq!(c.series.len(), 1);
            assert!((c.series[0].values[0] - 40.0).abs() < 2.0);
            assert!((c.series[0].values[1] - 80.0).abs() < 2.0);
            assert!(!c.xlsx_bytes.is_empty());
        }
        _ => panic!("Expected native chart element"),
    }
}

#[test]
fn test_review_focus_3_negative_values_interpolation() {
    // Tests axis with negative ticks: -50, 0, 50
    // Ensure negative values are computed correctly
    let mut paths = vec![
        RawPathItem::vertical_line(100_000, 100_000, 300_000),
        RawPathItem::horizontal_line(100_000, 250_000, 400_000), // baseline at 0
        RawPathItem::rect(150_000, 250_000, 50_000, 75_000),     // col pointing downwards
    ];
    let mut text_items = vec![
        RawTextItem::simple("50", 70_000, 100_000),
        RawTextItem::simple("0", 70_000, 250_000),
        RawTextItem::simple("-50", 70_000, 400_000),
        RawTextItem::simple("Loss", 160_000, 340_000),
    ];
    let elements = detect_charts(&mut paths, &mut text_items);
    assert!(!elements.is_empty());
    if let Element::Chart(c) = &elements[0] {
        assert!(c.series[0].values[0] < 0.0);
    } else {
        panic!("Expected native chart element");
    }
}
