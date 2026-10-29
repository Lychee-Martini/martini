use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_list_formats_includes_pptx() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("list-formats")
        .assert()
        .success()
        .stdout(predicate::str::contains("pptx"));
}

#[test]
fn test_convert_pdf_to_pptx_cli_invocation_recognizes_target() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.arg("convert")
        .arg("-i")
        .arg("tests/fixtures/nonexistent.pdf")
        .arg("-o")
        .arg("output.pptx")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Input file not found"));
}
