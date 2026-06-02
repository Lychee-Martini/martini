use crate::error::MartiniError;
use std::path::PathBuf;
use std::str::FromStr;

pub mod svg2favicon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Svg,
    Favicon,
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "svg" => Ok(Format::Svg),
            "favicon" => Ok(Format::Favicon),
            _ => Err(format!("Unsupported format: '{}'", s)),
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Svg => write!(f, "svg"),
            Format::Favicon => write!(f, "favicon"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub package: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputFileMetadata {
    pub path: String,
    pub size_bytes: u64,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversionResult {
    pub from: Format,
    pub to: Format,
    pub output_files: Vec<OutputFileMetadata>,
}

/// The main trait that all format converters must implement.
pub trait Converter {
    fn convert(
        &self,
        input_data: &[u8],
        options: &ConvertOptions,
    ) -> Result<ConversionResult, MartiniError>;
}

/// Dispatches conversion requests to the appropriate converter implementation.
pub fn convert(
    from: Format,
    to: Format,
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    match (from, to) {
        (Format::Svg, Format::Favicon) => {
            let converter = svg2favicon::SvgToFaviconConverter;
            converter.convert(input_data, options)
        }
        _ => Err(MartiniError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        }),
    }
}
