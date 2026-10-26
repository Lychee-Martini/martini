use martini::erase::clip::normalize_heatmap;

#[test]
fn test_normalize_heatmap_scaling() {
    let raw_scores = vec![0.1f32, 0.5, 0.9, 0.3];
    let normalized = normalize_heatmap(&raw_scores, 2, 2, 0.5);
    assert_eq!(normalized.dimensions(), (2, 2));
    // Score at (0, 1) is 0.9 > threshold 0.5 -> 255
    assert_eq!(normalized.get_pixel(0, 1)[0], 255);
    // Score at (0, 0) is 0.1 < threshold 0.5 -> 0
    assert_eq!(normalized.get_pixel(0, 0)[0], 0);
}
