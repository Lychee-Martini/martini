use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_erase_cli_requires_target_or_mask() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args(["erase", "test.png"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Either --target or --mask must be specified",
        ));
}

#[test]
fn test_erase_cli_missing_input_file() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args(["erase", "non_existent_file_xyz123.png", "--target", "dog"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Input file not found"));
}

#[test]
fn test_erase_cli_help_flags() {
    let mut cmd = Command::cargo_bin("martini").unwrap();
    cmd.args(["erase", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--target"))
        .stdout(predicate::str::contains("--mask"))
        .stdout(predicate::str::contains("--steps"))
        .stdout(predicate::str::contains("--threshold"))
        .stdout(predicate::str::contains("--dilate"))
        .stdout(predicate::str::contains("--save-mask"))
        .stdout(predicate::str::contains("--model-dir"));
}
