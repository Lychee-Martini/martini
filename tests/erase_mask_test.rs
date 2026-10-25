use image::{GrayImage, Luma};

#[test]
fn test_dilate_mask_expands_boundaries() {
    let mut mask = GrayImage::new(10, 10);
    mask.put_pixel(5, 5, Luma([255]));
    let dilated = martini::erase::mask::dilate_mask(&mask, 1);
    assert_eq!(dilated.get_pixel(5, 5)[0], 255);
    assert_eq!(dilated.get_pixel(4, 5)[0], 255);
    assert_eq!(dilated.get_pixel(6, 5)[0], 255);
    assert_eq!(dilated.get_pixel(5, 4)[0], 255);
    assert_eq!(dilated.get_pixel(5, 6)[0], 255);
    assert_eq!(dilated.get_pixel(0, 0)[0], 0);
}

#[test]
fn test_bounding_box_empty_mask() {
    let mask = GrayImage::new(10, 10);
    assert!(martini::erase::mask::compute_bounding_box(&mask, 2).is_none());
}

#[test]
fn test_bounding_box_with_padding() {
    let mut mask = GrayImage::new(20, 20);
    mask.put_pixel(10, 10, Luma([255]));
    let bbox = martini::erase::mask::compute_bounding_box(&mask, 3).unwrap();
    assert_eq!(bbox, (7, 7, 7, 7)); // x, y, width, height
}

#[test]
fn test_feather_mask_smooths_edges() {
    let mut mask = GrayImage::new(10, 10);
    for x in 3..7 {
        for y in 3..7 {
            mask.put_pixel(x, y, Luma([255]));
        }
    }
    let feathered = martini::erase::mask::feather_mask(&mask, 1);
    // Center pixel should be near max
    assert!(feathered.get_pixel(5, 5)[0] > 200);
    // Boundary pixel at 3,3 should be intermediate
    let edge_val = feathered.get_pixel(3, 3)[0];
    assert!(edge_val > 0 && edge_val < 255);
}
