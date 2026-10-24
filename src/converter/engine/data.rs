use super::FormatEngine;
use crate::converter::{ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub struct DataEngine;

impl FormatEngine for DataEngine {
    fn name(&self) -> &'static str {
        "data"
    }

    fn supported_pairs(&self) -> Vec<(Format, Format)> {
        let formats = [
            Format::Json,
            Format::Yaml,
            Format::Toml,
            Format::Csv,
            Format::Tsv,
        ];
        let mut pairs = Vec::new();
        for &from in &formats {
            for &to in &formats {
                if from != to {
                    pairs.push((from, to));
                }
            }
        }
        pairs
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError> {
        let from = detect_format(input)?;
        let to = detect_format(output)?;

        let value = read_data(input, from)?;
        write_data(&value, output, to)?;

        let metadata = fs::metadata(output).map_err(MartiniError::Io)?;

        Ok(OutputFileMetadata {
            path: output.to_string_lossy().to_string(),
            size_bytes: metadata.len(),
            description: format!("Converted from {} to {} using DataEngine", from, to),
        })
    }
}

fn detect_format(path: &Path) -> Result<Format, MartiniError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    Format::from_str(&ext).map_err(|_| MartiniError::InvalidInputData {
        reason: format!("Unsupported data format extension: '{}'", ext),
    })
}

fn read_data(path: &Path, format: Format) -> Result<Value, MartiniError> {
    let file = fs::File::open(path).map_err(MartiniError::Io)?;

    match format {
        Format::Json => serde_json::from_reader(file).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to parse JSON: {}", e),
        }),
        Format::Yaml => serde_yaml::from_reader(file).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to parse YAML: {}", e),
        }),
        Format::Toml => {
            let content = fs::read_to_string(path).map_err(MartiniError::Io)?;
            let toml_val: toml::Value =
                toml::from_str(&content).map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to parse TOML: {}", e),
                })?;
            serde_json::to_value(toml_val).map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to convert TOML to JSON: {}", e),
            })
        }
        Format::Csv | Format::Tsv => {
            let delimiter = if format == Format::Tsv { b'\t' } else { b',' };
            let mut rdr = csv::ReaderBuilder::new()
                .delimiter(delimiter)
                .from_reader(file);

            let headers = rdr
                .headers()
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to read CSV headers: {}", e),
                })?
                .clone();

            let mut rows = Vec::new();
            for result in rdr.records() {
                let record = result.map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to read CSV record: {}", e),
                })?;
                let mut map = serde_json::Map::new();
                for (header, field) in headers.iter().zip(record.iter()) {
                    let val = if let Ok(n) = field.parse::<i64>() {
                        Value::Number(n.into())
                    } else if let Ok(f) = field.parse::<f64>() {
                        if let Some(num) = serde_json::Number::from_f64(f) {
                            Value::Number(num)
                        } else {
                            Value::String(field.to_string())
                        }
                    } else if let Ok(b) = field.parse::<bool>() {
                        Value::Bool(b)
                    } else {
                        Value::String(field.to_string())
                    };
                    map.insert(header.to_string(), val);
                }
                rows.push(Value::Object(map));
            }
            Ok(Value::Array(rows))
        }
        _ => Err(MartiniError::UnsupportedConversion {
            from: format.to_string(),
            to: "intermediate data".to_string(),
        }),
    }
}

fn write_data(value: &Value, path: &Path, format: Format) -> Result<(), MartiniError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(MartiniError::Io)?;
    }

    match format {
        Format::Json => {
            let json_str =
                serde_json::to_string_pretty(value).map_err(|e| MartiniError::OutputWrite {
                    reason: format!("Failed to serialize JSON: {}", e),
                })?;
            fs::write(path, json_str).map_err(MartiniError::Io)?;
        }
        Format::Yaml => {
            let yaml_str = serde_yaml::to_string(value).map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to serialize YAML: {}", e),
            })?;
            fs::write(path, yaml_str).map_err(MartiniError::Io)?;
        }
        Format::Toml => {
            let toml_val: toml::Value = match value {
                Value::Object(_) => serde_json::from_value(value.clone()).map_err(|e| {
                    MartiniError::OutputWrite {
                        reason: format!("Failed to convert JSON to TOML value: {}", e),
                    }
                })?,
                _ => {
                    let mut wrapper = serde_json::Map::new();
                    wrapper.insert("data".to_string(), value.clone());
                    serde_json::from_value(Value::Object(wrapper)).map_err(|e| {
                        MartiniError::OutputWrite {
                            reason: format!("Failed to convert JSON to TOML table: {}", e),
                        }
                    })?
                }
            };
            let toml_str =
                toml::to_string_pretty(&toml_val).map_err(|e| MartiniError::OutputWrite {
                    reason: format!("Failed to serialize TOML: {}", e),
                })?;
            fs::write(path, toml_str).map_err(MartiniError::Io)?;
        }
        Format::Csv | Format::Tsv => {
            let delimiter = if format == Format::Tsv { b'\t' } else { b',' };
            let file = fs::File::create(path).map_err(MartiniError::Io)?;
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(delimiter)
                .from_writer(file);

            match value {
                Value::Array(items) => {
                    let mut all_keys: Vec<String> = Vec::new();
                    let mut seen = HashSet::new();
                    for item in items {
                        if let Value::Object(map) = item {
                            for key in map.keys() {
                                if seen.insert(key.clone()) {
                                    all_keys.push(key.clone());
                                }
                            }
                        }
                    }

                    if all_keys.is_empty() {
                        wtr.write_record(["value"])
                            .map_err(|e| MartiniError::OutputWrite {
                                reason: format!("Failed to write CSV header: {}", e),
                            })?;
                        for item in items {
                            let str_val = match item {
                                Value::String(s) => s.clone(),
                                other => other.to_string(),
                            };
                            wtr.write_record([str_val])
                                .map_err(|e| MartiniError::OutputWrite {
                                    reason: format!("Failed to write CSV record: {}", e),
                                })?;
                        }
                    } else {
                        let headers: Vec<&str> = all_keys.iter().map(|s| s.as_str()).collect();
                        wtr.write_record(&headers)
                            .map_err(|e| MartiniError::OutputWrite {
                                reason: format!("Failed to write CSV header: {}", e),
                            })?;

                        for item in items {
                            let mut row = Vec::new();
                            if let Value::Object(map) = item {
                                for key in &all_keys {
                                    let field_val = match map.get(key) {
                                        Some(Value::String(s)) => s.clone(),
                                        Some(Value::Null) | None => String::new(),
                                        Some(other) => other.to_string(),
                                    };
                                    row.push(field_val);
                                }
                            }
                            wtr.write_record(&row)
                                .map_err(|e| MartiniError::OutputWrite {
                                    reason: format!("Failed to write CSV record: {}", e),
                                })?;
                        }
                    }
                }
                Value::Object(map) => {
                    wtr.write_record(["key", "value"])
                        .map_err(|e| MartiniError::OutputWrite {
                            reason: format!("Failed to write CSV header: {}", e),
                        })?;
                    for (k, v) in map {
                        let str_val = match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        wtr.write_record([k.as_str(), str_val.as_str()])
                            .map_err(|e| MartiniError::OutputWrite {
                                reason: format!("Failed to write CSV record: {}", e),
                            })?;
                    }
                }
                _ => {
                    wtr.write_record(["value"])
                        .map_err(|e| MartiniError::OutputWrite {
                            reason: format!("Failed to write CSV header: {}", e),
                        })?;
                    wtr.write_record([value.to_string()]).map_err(|e| {
                        MartiniError::OutputWrite {
                            reason: format!("Failed to write CSV record: {}", e),
                        }
                    })?;
                }
            }
            wtr.flush().map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to flush CSV writer: {}", e),
            })?;
        }
        _ => {
            return Err(MartiniError::UnsupportedConversion {
                from: "intermediate data".to_string(),
                to: format.to_string(),
            });
        }
    }

    Ok(())
}
