use assert_cmd::Command;
use image::{Rgba, RgbaImage};
use tempfile::tempdir;

#[test]
fn test_erase_fast_mode_end_to_end() {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("test_input.png");
    let output_path = dir.path().join("test_output.png");

    let img = RgbaImage::from_pixel(64, 64, Rgba([200, 200, 200, 255]));
    img.save(&input_path).unwrap();

    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args([
        "erase",
        input_path.to_str().unwrap(),
        "-o",
        output_path.to_str().unwrap(),
        "-t",
        "watermark",
        "--fast",
        "--json",
    ]);
    cmd.assert().success();
    assert!(output_path.exists());
}
