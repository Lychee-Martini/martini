use crate::error::MartiniError;
use std::path::PathBuf;
use std::str::FromStr;

pub mod batch;
pub mod image_conv;
pub mod pdf_conv;

#[derive(Debug, Clone)]
pub struct EncodedFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Svg,
    Favicon,
    Png,
    Jpg,
    Webp,
    Avif,
    Pdf,
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "svg" => Ok(Format::Svg),
            "favicon" => Ok(Format::Favicon),
            "png" => Ok(Format::Png),
            "jpg" | "jpeg" => Ok(Format::Jpg),
            "webp" => Ok(Format::Webp),
            "avif" => Ok(Format::Avif),
            "pdf" => Ok(Format::Pdf),
            _ => Err(format!("Unsupported format: '{}'", s)),
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Format::Svg => write!(f, "svg"),
            Format::Favicon => write!(f, "favicon"),
            Format::Png => write!(f, "png"),
            Format::Jpg => write!(f, "jpg"),
            Format::Webp => write!(f, "webp"),
            Format::Avif => write!(f, "avif"),
            Format::Pdf => write!(f, "pdf"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub package: bool,
    pub quality: u8,
    pub lossless: bool,
    pub overwrite: bool,
    pub pages: Option<String>,
    pub dpi: u16,
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
    if to == Format::Pdf && from != Format::Pdf {
        return Err(MartiniError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        });
    }

    if from == Format::Pdf {
        let converter = pdf_conv::PdfConverter;
        converter.convert_pdf(to, input_data, options)
    } else {
        let converter = image_conv::ImageConverter;
        converter.convert_image(from, to, input_data, options)
    }
}
