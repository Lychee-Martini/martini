use martini::converter::Format;
use martini::converter::engine::data::convert_slice;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_json_to_yaml_and_back() {
    let dir = tempdir().unwrap();
    let yaml_file = dir.path().join("output.yaml");
    let meta = convert_slice(
        br#"{"name": "martini", "awesome": true}"#,
        &yaml_file,
        Format::Json,
        Format::Yaml,
    )
    .unwrap();
    assert!(meta.size_bytes > 0);
    let yaml_bytes = fs::read(&yaml_file).unwrap();
    let yaml_str = String::from_utf8_lossy(&yaml_bytes);
    assert!(yaml_str.contains("name: martini"));
    assert!(yaml_str.contains("awesome: true"));

    // Back to JSON
    let back_json_file = dir.path().join("back.json");
    convert_slice(&yaml_bytes, &back_json_file, Format::Yaml, Format::Json).unwrap();
    let back_str = fs::read_to_string(&back_json_file).unwrap();
    assert!(back_str.contains("martini"));
}

#[test]
fn test_json_to_toml_and_back() {
    let dir = tempdir().unwrap();
    let toml_file = dir.path().join("output.toml");
    convert_slice(
        br#"{"title": "Martini", "version": "1.0"}"#,
        &toml_file,
        Format::Json,
        Format::Toml,
    )
    .unwrap();
    let toml_bytes = fs::read(&toml_file).unwrap();
    assert!(String::from_utf8_lossy(&toml_bytes).contains("title = \"Martini\""));

    let back_json = dir.path().join("back.json");
    convert_slice(&toml_bytes, &back_json, Format::Toml, Format::Json).unwrap();
    let back_str = fs::read_to_string(&back_json).unwrap();
    assert!(back_str.contains("Martini"));
}

#[test]
fn test_json_to_csv_with_key_union() {
    let dir = tempdir().unwrap();
    let csv_file = dir.path().join("output.csv");
    convert_slice(
        br#"[{"a": 1, "b": 2}, {"b": 3, "c": 4}]"#,
        &csv_file,
        Format::Json,
        Format::Csv,
    )
    .unwrap();
    let csv_bytes = fs::read(&csv_file).unwrap();
    let csv_str = String::from_utf8_lossy(&csv_bytes);
    assert!(csv_str.contains("a") && csv_str.contains("b") && csv_str.contains("c"));

    // Convert CSV back to JSON
    let back_json = dir.path().join("back.json");
    convert_slice(&csv_bytes, &back_json, Format::Csv, Format::Json).unwrap();
    let back_str = fs::read_to_string(&back_json).unwrap();
    assert!(back_str.contains("\"b\""));
}

#[test]
fn test_csv_to_tsv() {
    let dir = tempdir().unwrap();
    let tsv_file = dir.path().join("output.tsv");
    convert_slice(
        b"col1,col2\nval1,val2\n",
        &tsv_file,
        Format::Csv,
        Format::Tsv,
    )
    .unwrap();
    let tsv_str = fs::read_to_string(&tsv_file).unwrap();
    assert!(tsv_str.contains("col1\tcol2"));
    assert!(tsv_str.contains("val1\tval2"));
}
