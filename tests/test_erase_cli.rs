use clap::Parser;
use martini::erase::EraseArgs;
use martini::erase::models::InpaintModelPreset;

#[derive(Parser, Debug)]
struct TestCli {
    #[command(flatten)]
    erase: EraseArgs,
}

#[test]
fn test_cli_parse_replace_and_model_flags() {
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
    assert_eq!(args.erase.model, InpaintModelPreset::Lcm);
    assert!(args.erase.benchmark);
    assert!(args.erase.fast);
    assert_eq!(args.erase.device, "cuda");
}

#[test]
fn test_cli_parse_download_models_without_input() {
    let args = TestCli::parse_from(["test", "--download-models", "sd-1.5"]);
    assert_eq!(args.erase.download_models, Some(Some("sd-1.5".to_string())));
    assert_eq!(args.erase.input, None);
}
