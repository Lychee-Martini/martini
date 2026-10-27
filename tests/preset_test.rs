use martini::preset::PresetManager;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_preset_load_from_toml() {
    let dir = tempdir().unwrap();
    let toml_path = dir.path().join("martini-presets.toml");
    fs::write(
        &toml_path,
        r#"
[presets.web-optimize]
to = "webp"
quality = 82
lossless = false

[presets.data-sync]
from = "csv"
to = "json"
"#,
    )
    .unwrap();

    let manager = PresetManager::load_from_path(&toml_path).unwrap();
    let preset = manager.get("web-optimize").expect("preset should exist");
    assert_eq!(preset.to, "webp");
    assert_eq!(preset.quality, Some(82));
    assert_eq!(preset.lossless, Some(false));

    let data_preset = manager.get("data-sync").expect("data-sync should exist");
    assert_eq!(data_preset.from.as_deref(), Some("csv"));
    assert_eq!(data_preset.to, "json");
}

#[test]
fn test_preset_apply_to_convert_and_watch() {
    use martini::Preset;
    use martini::cli::Commands;
    use std::path::PathBuf;

    let preset = Preset {
        from: Some("png".to_string()),
        to: "webp".to_string(),
        quality: Some(85),
        lossless: Some(true),
        recursive: Some(true),
        overwrite: Some(true),
        delete_original: Some(true),
        move_originals: Some(PathBuf::from("/backup")),
        dpi: Some(300),
        theme: Some("modern".to_string()),
        paper_size: Some("a4".to_string()),
    };

    // Test Convert with auto from and None to
    let mut convert_cmd = Commands::Convert {
        from: "auto".to_string(),
        to: None,
        input: Some(PathBuf::from("in.png")),
        input_flag: None,
        output: None,
        output_flag: None,
        package: false,
        quality: 80,
        lossless: false,
        recursive: false,
        delete_original: false,
        overwrite: false,
        workers: None,
        pages: None,
        dpi: 150,
        width: None,
        height: None,
        no_upscale: false,
        theme: None,
        paper_size: None,
        margin: None,
        font_family: None,
        header: None,
        footer: None,
        no_page_numbers: false,
        toc: false,
        preset: Some("test".to_string()),
    };

    assert_eq!(convert_cmd.preset_name(), Some("test"));
    preset.apply_to(&mut convert_cmd);

    if let Commands::Convert {
        from,
        to,
        quality,
        lossless,
        recursive,
        delete_original,
        theme,
        paper_size,
        ..
    } = convert_cmd
    {
        assert_eq!(from, "png");
        assert_eq!(to, Some("webp".to_string()));
        assert_eq!(quality, 85);
        assert!(lossless);
        assert!(recursive);
        assert!(delete_original);
        assert_eq!(theme, Some("modern".to_string()));
        assert_eq!(paper_size, Some("a4".to_string()));
    } else {
        panic!("expected Commands::Convert");
    }

    // Test Watch
    let mut watch_cmd = Commands::Watch {
        input: PathBuf::from("in"),
        output: PathBuf::from("out"),
        preset: Some("test".to_string()),
        to: None,
        settle_ms: 500,
        recursive: false,
        initial: false,
        delete_original: false,
        move_originals: None,
        overwrite: false,
    };

    assert_eq!(watch_cmd.preset_name(), Some("test"));
    preset.apply_to(&mut watch_cmd);

    if let Commands::Watch {
        to,
        recursive,
        delete_original,
        move_originals,
        ..
    } = watch_cmd
    {
        assert_eq!(to, Some("webp".to_string()));
        assert!(recursive);
        assert!(delete_original);
        assert_eq!(move_originals, Some(PathBuf::from("/backup")));
    } else {
        panic!("expected Commands::Watch");
    }
}
