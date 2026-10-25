use crate::error::MartiniError;
use std::path::PathBuf;
use std::str::FromStr;

pub mod batch;
pub mod engine;
pub mod image_conv;
pub mod md_conv;
pub mod md_pdf;
pub mod md_to_typst;
pub mod pdf_conv;
pub mod typst_engine;

#[derive(Debug, Clone)]
pub struct EncodedFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Svg,
    Favicon,
    Png,
    Jpg,
    Webp,
    Avif,
    Pdf,
    Md,
    Docx,
    // Data formats
    Json,
    Yaml,
    Toml,
    Csv,
    Tsv,
    // Archive formats
    Zip,
    Tar,
    Gz,
    // Extended Image formats
    Bmp,
    Tiff,
    Qoi,
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "svg" => Ok(Format::Svg),
            "favicon" | "ico" => Ok(Format::Favicon),
            "png" => Ok(Format::Png),
            "jpg" | "jpeg" => Ok(Format::Jpg),
            "webp" => Ok(Format::Webp),
            "avif" => Ok(Format::Avif),
            "pdf" => Ok(Format::Pdf),
            "md" | "markdown" => Ok(Format::Md),
            "docx" => Ok(Format::Docx),
            "json" => Ok(Format::Json),
            "yaml" | "yml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            "csv" => Ok(Format::Csv),
            "tsv" => Ok(Format::Tsv),
            "zip" => Ok(Format::Zip),
            "tar" => Ok(Format::Tar),
            "gz" | "gzip" | "tar.gz" | "targz" | "tgz" => Ok(Format::Gz),
            "bmp" => Ok(Format::Bmp),
            "tiff" | "tif" => Ok(Format::Tiff),
            "qoi" => Ok(Format::Qoi),
            _ => Err(format!("Unsupported format: '{}'", s)),
        }
    }
}

impl Format {
    pub fn from_path(path: &std::path::Path) -> Result<Self, MartiniError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        Self::from_str(&ext).map_err(|_| MartiniError::InvalidInputData {
            reason: format!("Unsupported format extension: '{}'", ext),
        })
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
            Format::Md => write!(f, "md"),
            Format::Docx => write!(f, "docx"),
            Format::Json => write!(f, "json"),
            Format::Yaml => write!(f, "yaml"),
            Format::Toml => write!(f, "toml"),
            Format::Csv => write!(f, "csv"),
            Format::Tsv => write!(f, "tsv"),
            Format::Zip => write!(f, "zip"),
            Format::Tar => write!(f, "tar"),
            Format::Gz => write!(f, "gz"),
            Format::Bmp => write!(f, "bmp"),
            Format::Tiff => write!(f, "tiff"),
            Format::Qoi => write!(f, "qoi"),
        }
    }
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            input_path: PathBuf::new(),
            output_path: PathBuf::new(),
            package: false,
            quality: 80,
            lossless: false,
            overwrite: false,
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
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub no_upscale: bool,
    pub theme: Option<String>,
    pub paper_size: Option<String>,
    pub margin: Option<String>,
    pub font_family: Option<String>,
    pub header: Option<String>,
    pub footer: Option<String>,
    pub no_page_numbers: bool,
    pub toc: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputFileMetadata {
    pub path: String,
    pub size_bytes: u64,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FormatCapability {
    pub from: String,
    pub to: String,
    pub description: String,
    pub parameters: std::collections::HashMap<String, String>,
}

impl FormatCapability {
    pub fn new(from: &str, to: &str, desc: &str, params: &[(&str, &str)]) -> Self {
        Self {
            from: from.to_string(),
            to: to.to_string(),
            description: desc.to_string(),
            parameters: params
                .iter()
                .map(|&(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

pub fn all_capabilities() -> Vec<FormatCapability> {
    engine::EngineRegistry::default().all_capabilities()
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConversionResult {
    pub from: Format,
    pub to: Format,
    pub output_files: Vec<OutputFileMetadata>,
}

/// Dispatches conversion requests to the appropriate converter implementation.
pub fn convert(
    from: Format,
    to: Format,
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    if from == Format::Md && to == Format::Pdf {
        md_pdf::convert_markdown_to_pdf(input_data, options)
    } else if from == Format::Md && to == Format::Docx {
        md_conv::convert_markdown(input_data, options)
    } else if from == Format::Pdf {
        pdf_conv::convert_pdf(to, input_data, options)
    } else if to == Format::Pdf {
        Err(MartiniError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        })
    } else if engine::FormatEngine::can_convert(&engine::data::DataEngine, from, to) {
        let meta =
            engine::data::DataEngine.convert_slice(input_data, &options.output_path, from, to)?;
        Ok(ConversionResult {
            from,
            to,
            output_files: vec![meta],
        })
    } else if engine::FormatEngine::can_convert(&engine::archive::ArchiveEngine, from, to) {
        let (input_path, _temp_file) = if options.input_path.exists() {
            (options.input_path.clone(), None)
        } else {
            let suffix = if from == Format::Gz {
                ".tar.gz"
            } else {
                &format!(".{}", from)
            };
            let mut temp = tempfile::Builder::new()
                .suffix(suffix)
                .tempfile()
                .map_err(MartiniError::Io)?;
            std::io::Write::write_all(&mut temp, input_data).map_err(MartiniError::Io)?;
            let path = temp.path().to_path_buf();
            (path, Some(temp))
        };
        let meta = engine::archive::ArchiveEngine.convert_with_formats(
            &input_path,
            &options.output_path,
            from,
            to,
        )?;
        Ok(ConversionResult {
            from,
            to,
            output_files: vec![meta],
        })
    } else {
        image_conv::convert_image(from, to, input_data, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_all_capabilities() {
        let formats = all_capabilities();
        assert!(!formats.is_empty());
        assert!(formats.iter().any(|f| f.to == "favicon"));
        assert!(
            formats
                .iter()
                .any(|f| f.description.contains("structured data"))
        );
        assert!(
            formats
                .iter()
                .any(|f| f.description.contains("compressed archives"))
        );
    }
}
