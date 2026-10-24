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
