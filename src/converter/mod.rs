use crate::error::MartiniError;
use std::path::PathBuf;
use std::str::FromStr;

pub mod batch;
pub mod engine {
    pub mod archive;
    pub mod data;
}
pub mod image_conv;
pub mod md_conv;
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
    let img_from = "svg, png, jpg, jpeg, webp, avif, bmp, tiff, qoi";
    let batch_base = &[
        ("overwrite", "boolean (default false)"),
        ("delete_original", "boolean (default false)"),
        ("recursive", "boolean (default false)"),
        ("workers", "integer (optional)"),
    ];

    vec![
        FormatCapability::new(
            img_from,
            "favicon",
            "Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package)",
            &[(
                "package",
                "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)",
            )],
        ),
        FormatCapability::new(img_from, "png", "Convert images to PNG format", batch_base),
        FormatCapability::new(
            img_from,
            "jpg",
            "Convert images to JPEG format",
            &[
                ("quality", "integer (1-100, default 80)"),
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
                ("recursive", "boolean (default false)"),
                ("workers", "integer (optional)"),
            ],
        ),
        FormatCapability::new(
            img_from,
            "webp",
            "Convert images to WebP format",
            &[
                ("quality", "integer (1-100, default 80)"),
                ("lossless", "boolean (default false)"),
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
                ("recursive", "boolean (default false)"),
                ("workers", "integer (optional)"),
            ],
        ),
        FormatCapability::new(
            img_from,
            "avif",
            "Convert images to AVIF format",
            &[
                ("quality", "integer (1-100, default 80)"),
                ("lossless", "boolean (default false)"),
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
                ("recursive", "boolean (default false)"),
                ("workers", "integer (optional)"),
            ],
        ),
        FormatCapability::new(
            img_from,
            "both",
            "Convert images to both WebP and AVIF formats",
            &[
                ("quality", "integer (1-100, default 80)"),
                ("lossless", "boolean (default false)"),
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
                ("recursive", "boolean (default false)"),
                ("workers", "integer (optional)"),
            ],
        ),
        FormatCapability::new(img_from, "bmp", "Convert images to BMP format", batch_base),
        FormatCapability::new(
            img_from,
            "tiff",
            "Convert images to TIFF format",
            batch_base,
        ),
        FormatCapability::new(img_from, "qoi", "Convert images to QOI format", batch_base),
        FormatCapability::new(
            "pdf",
            "png, jpg, jpeg, webp, avif",
            "Convert PDF pages to images",
            &[
                (
                    "pages",
                    "string (comma-separated page numbers or ranges, e.g. '1,3-5')",
                ),
                ("dpi", "integer (rendering DPI, default 150)"),
                ("quality", "integer (1-100, default 80)"),
                ("lossless", "boolean (default false)"),
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
            ],
        ),
        FormatCapability::new(
            "md",
            "docx",
            "Convert Markdown documents to Microsoft Word (DOCX) files",
            &[("overwrite", "boolean (default false)")],
        ),
        FormatCapability::new(
            "md",
            "pdf",
            "Convert Markdown documents to PDF files with Pure Rust engine",
            &[
                (
                    "theme",
                    "string ('github', 'academic', 'modern', 'minimal', default 'github')",
                ),
                (
                    "paper_size",
                    "string ('a4', 'letter', 'a3', 'a5', default 'a4')",
                ),
                ("margin", "string (e.g. '20mm', '1in', default '20mm')"),
                ("font_family", "string (custom font family name)"),
                ("header", "string (custom header text)"),
                ("footer", "string (custom footer text)"),
                ("no_page_numbers", "boolean (default false)"),
                (
                    "toc",
                    "boolean (automatically generate table of contents, default false)",
                ),
                ("overwrite", "boolean (default false)"),
            ],
        ),
        FormatCapability::new(
            "json, yaml, toml, csv, tsv",
            "json, yaml, toml, csv, tsv",
            "Convert structured data between JSON, YAML, TOML, CSV, and TSV formats",
            &[
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
            ],
        ),
        FormatCapability::new(
            "zip, tar, gz, tar.gz",
            "zip, tar, gz, tar.gz",
            "Convert compressed archives between ZIP, TAR, and TAR.GZ formats",
            &[
                ("overwrite", "boolean (default false)"),
                ("delete_original", "boolean (default false)"),
            ],
        ),
    ]
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
        md_to_typst::convert_markdown_to_pdf(input_data, options)
    } else if from == Format::Md && to == Format::Docx {
        md_conv::convert_markdown(input_data, options)
    } else if from == Format::Pdf {
        pdf_conv::convert_pdf(to, input_data, options)
    } else if to == Format::Pdf {
        Err(MartiniError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        })
    } else if engine::data::can_convert(from, to) {
        let meta = engine::data::convert_slice(input_data, &options.output_path, from, to)?;
        Ok(ConversionResult {
            from,
            to,
            output_files: vec![meta],
        })
    } else if engine::archive::can_convert(from, to) {
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
        let meta = engine::archive::convert_archive(&input_path, &options.output_path, from, to)?;
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
