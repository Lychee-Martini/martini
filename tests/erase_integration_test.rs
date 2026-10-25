use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn test_erase_with_predefined_mask_and_json_output() {
    let tmp = tempdir().unwrap();
    let in_file = tmp.path().join("input.png");
    let mask_file = tmp.path().join("mask.png");
    let out_file = tmp.path().join("output.png");

    // Create synthetic input and mask
    image::RgbaImage::from_pixel(64, 64, image::Rgba([120, 120, 120, 255]))
        .save(&in_file)
        .unwrap();
    image::GrayImage::from_pixel(64, 64, image::Luma([0]))
        .save(&mask_file)
        .unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args([
        "erase",
        in_file.to_str().unwrap(),
        "-o",
        out_file.to_str().unwrap(),
        "--mask",
        mask_file.to_str().unwrap(),
        "--json",
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("\"success\": true"));

    assert!(out_file.exists());
}

#[test]
fn test_erase_default_output_naming() {
    let tmp = tempdir().unwrap();
    let in_file = tmp.path().join("photo.png");
    let mask_file = tmp.path().join("mask.png");

    image::RgbaImage::from_pixel(32, 32, image::Rgba([200, 200, 200, 255]))
        .save(&in_file)
        .unwrap();
    image::GrayImage::from_pixel(32, 32, image::Luma([0]))
        .save(&mask_file)
        .unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args([
        "erase",
        in_file.to_str().unwrap(),
        "--mask",
        mask_file.to_str().unwrap(),
    ])
    .assert()
    .success();

    let default_out = tmp.path().join("photo_erased.png");
    assert!(default_out.exists());
}

#[test]
fn test_erase_save_mask_flag() {
    let tmp = tempdir().unwrap();
    let in_file = tmp.path().join("item.png");
    let mask_file = tmp.path().join("mask.png");
    let out_file = tmp.path().join("out.png");

    image::RgbaImage::from_pixel(32, 32, image::Rgba([50, 50, 50, 255]))
        .save(&in_file)
        .unwrap();
    image::GrayImage::from_pixel(32, 32, image::Luma([255]))
        .save(&mask_file)
        .unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args([
        "erase",
        in_file.to_str().unwrap(),
        "-o",
        out_file.to_str().unwrap(),
        "--mask",
        mask_file.to_str().unwrap(),
        "--save-mask",
    ])
    .assert()
    .success();

    let saved_mask = tmp.path().join("item_mask.png");
    assert!(saved_mask.exists());
}
