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

#[derive(clap::Parser, Debug)]
struct TestCli {
    #[command(flatten)]
    erase: martini::erase::EraseArgs,
}

#[test]
fn test_cli_parse_replace_and_model_flags() {
    use clap::Parser;
    let args = TestCli::parse_from([
        "test",
        "input.png",
        "-t",
        "watermark",
        "-r",
        "flowers",
        "--model",
        "lcm",
        "--benchmark",
        "--fast",
        "--device",
        "cuda",
    ]);
    assert_eq!(args.erase.target.as_deref(), Some("watermark"));
    assert_eq!(args.erase.replace.as_deref(), Some("flowers"));
    assert_eq!(
        args.erase.model,
        martini::erase::models::InpaintModelPreset::Lcm
    );
    assert!(args.erase.benchmark);
    assert!(args.erase.fast);
    assert_eq!(args.erase.device, "cuda");
}

#[test]
fn test_cli_parse_download_models_without_input() {
    use clap::Parser;
    let args = TestCli::parse_from(["test", "--download-models", "sd-1.5"]);
    assert_eq!(args.erase.download_models, Some(Some("sd-1.5".to_string())));
    assert_eq!(args.erase.input, None);
}
