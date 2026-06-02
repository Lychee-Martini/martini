use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_list_formats() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .assert()
        .success()
        .stdout(predicate::str::contains("svg -> favicon"));
}

#[test]
fn test_list_formats_json() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .arg("--json")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"from\": \"svg\""));
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
