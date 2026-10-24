use martini::converter::engine::data::DataEngine;
use martini::converter::engine::FormatEngine;
use martini::converter::ConvertOptions;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_json_to_yaml_and_back() {
    let engine = DataEngine;
    let dir = tempdir().unwrap();
    let json_file = dir.path().join("input.json");
    let yaml_file = dir.path().join("output.yaml");
    fs::write(&json_file, r#"{"name": "martini", "awesome": true}"#).unwrap();

    let opt = ConvertOptions::default();
    let meta = engine.convert(&json_file, &yaml_file, &opt).unwrap();
    assert!(meta.size_bytes > 0);
    let yaml_str = fs::read_to_string(&yaml_file).unwrap();
    assert!(yaml_str.contains("name: martini"));
    assert!(yaml_str.contains("awesome: true"));

    // Back to JSON
    let back_json_file = dir.path().join("back.json");
    engine.convert(&yaml_file, &back_json_file, &opt).unwrap();
    let back_str = fs::read_to_string(&back_json_file).unwrap();
    assert!(back_str.contains("martini"));
}

#[test]
fn test_json_to_toml_and_back() {
    let engine = DataEngine;
    let dir = tempdir().unwrap();
    let json_file = dir.path().join("input.json");
    let toml_file = dir.path().join("output.toml");
    fs::write(&json_file, r#"{"title": "Martini", "version": "1.0"}"#).unwrap();

    let opt = ConvertOptions::default();
    engine.convert(&json_file, &toml_file, &opt).unwrap();
    let toml_str = fs::read_to_string(&toml_file).unwrap();
    assert!(toml_str.contains("title = \"Martini\""));

    let back_json = dir.path().join("back.json");
    engine.convert(&toml_file, &back_json, &opt).unwrap();
    let back_str = fs::read_to_string(&back_json).unwrap();
    assert!(back_str.contains("Martini"));
}

#[test]
fn test_json_to_csv_with_key_union() {
    let engine = DataEngine;
    let dir = tempdir().unwrap();
    let json_file = dir.path().join("input.json");
    let csv_file = dir.path().join("output.csv");
    fs::write(&json_file, r#"[{"a": 1, "b": 2}, {"b": 3, "c": 4}]"#).unwrap();

    let opt = ConvertOptions::default();
    engine.convert(&json_file, &csv_file, &opt).unwrap();
    let csv_str = fs::read_to_string(&csv_file).unwrap();
    assert!(csv_str.contains("a") && csv_str.contains("b") && csv_str.contains("c"));

    // Convert CSV back to JSON
    let back_json = dir.path().join("back.json");
    engine.convert(&csv_file, &back_json, &opt).unwrap();
    let back_str = fs::read_to_string(&back_json).unwrap();
    assert!(back_str.contains("\"b\""));
}

#[test]
fn test_csv_to_tsv() {
    let engine = DataEngine;
    let dir = tempdir().unwrap();
    let csv_file = dir.path().join("input.csv");
    let tsv_file = dir.path().join("output.tsv");
    fs::write(&csv_file, "col1,col2\nval1,val2\n").unwrap();

    let opt = ConvertOptions::default();
    engine.convert(&csv_file, &tsv_file, &opt).unwrap();
    let tsv_str = fs::read_to_string(&tsv_file).unwrap();
    assert!(tsv_str.contains("col1\tcol2"));
    assert!(tsv_str.contains("val1\tval2"));
}
