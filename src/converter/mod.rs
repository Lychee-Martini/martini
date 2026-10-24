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
            "gz" | "gzip" => Ok(Format::Gz),
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

pub fn get_supported_formats() -> Vec<FormatCapability> {
    let mut list = Vec::new();

    let mut fav_params = std::collections::HashMap::new();
    fav_params.insert("package".to_string(), "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "favicon".to_string(),
        description:
            "Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package)"
                .to_string(),
        parameters: fav_params,
    });

    let mut png_params = std::collections::HashMap::new();
    png_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    png_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    png_params.insert(
        "recursive".to_string(),
        "boolean (default false)".to_string(),
    );
    png_params.insert("workers".to_string(), "integer (optional)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "png".to_string(),
        description: "Convert images to PNG format".to_string(),
        parameters: png_params,
    });

    let mut jpg_params = std::collections::HashMap::new();
    jpg_params.insert(
        "quality".to_string(),
        "integer (1-100, default 80)".to_string(),
    );
    jpg_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    jpg_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    jpg_params.insert(
        "recursive".to_string(),
        "boolean (default false)".to_string(),
    );
    jpg_params.insert("workers".to_string(), "integer (optional)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "jpg".to_string(),
        description: "Convert images to JPEG format".to_string(),
        parameters: jpg_params,
    });

    let mut webp_params = std::collections::HashMap::new();
    webp_params.insert(
        "quality".to_string(),
        "integer (1-100, default 80)".to_string(),
    );
    webp_params.insert(
        "lossless".to_string(),
        "boolean (default false)".to_string(),
    );
    webp_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    webp_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    webp_params.insert(
        "recursive".to_string(),
        "boolean (default false)".to_string(),
    );
    webp_params.insert("workers".to_string(), "integer (optional)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "webp".to_string(),
        description: "Convert images to WebP format".to_string(),
        parameters: webp_params,
    });

    let mut avif_params = std::collections::HashMap::new();
    avif_params.insert(
        "quality".to_string(),
        "integer (1-100, default 80)".to_string(),
    );
    avif_params.insert(
        "lossless".to_string(),
        "boolean (default false)".to_string(),
    );
    avif_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    avif_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    avif_params.insert(
        "recursive".to_string(),
        "boolean (default false)".to_string(),
    );
    avif_params.insert("workers".to_string(), "integer (optional)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "avif".to_string(),
        description: "Convert images to AVIF format".to_string(),
        parameters: avif_params,
    });

    let mut both_params = std::collections::HashMap::new();
    both_params.insert(
        "quality".to_string(),
        "integer (1-100, default 80)".to_string(),
    );
    both_params.insert(
        "lossless".to_string(),
        "boolean (default false)".to_string(),
    );
    both_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    both_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    both_params.insert(
        "recursive".to_string(),
        "boolean (default false)".to_string(),
    );
    both_params.insert("workers".to_string(), "integer (optional)".to_string());
    list.push(FormatCapability {
        from: "svg, png, jpg, jpeg, webp, avif".to_string(),
        to: "both".to_string(),
        description: "Convert images to both WebP and AVIF formats".to_string(),
        parameters: both_params,
    });

    let mut pdf_params = std::collections::HashMap::new();
    pdf_params.insert(
        "pages".to_string(),
        "string (comma-separated page numbers or ranges, e.g. '1,3-5')".to_string(),
    );
    pdf_params.insert(
        "dpi".to_string(),
        "integer (rendering DPI, default 150)".to_string(),
    );
    pdf_params.insert(
        "quality".to_string(),
        "integer (1-100, default 80)".to_string(),
    );
    pdf_params.insert(
        "lossless".to_string(),
        "boolean (default false)".to_string(),
    );
    pdf_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    pdf_params.insert(
        "delete_original".to_string(),
        "boolean (default false)".to_string(),
    );
    list.push(FormatCapability {
        from: "pdf".to_string(),
        to: "png, jpg, jpeg, webp, avif".to_string(),
        description: "Convert PDF pages to images".to_string(),
        parameters: pdf_params,
    });

    let mut docx_params = std::collections::HashMap::new();
    docx_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    list.push(FormatCapability {
        from: "md".to_string(),
        to: "docx".to_string(),
        description: "Convert Markdown documents to Microsoft Word (DOCX) files".to_string(),
        parameters: docx_params,
    });

    let mut pdf_md_params = std::collections::HashMap::new();
    pdf_md_params.insert(
        "theme".to_string(),
        "string ('github', 'academic', 'modern', 'minimal', default 'github')".to_string(),
    );
    pdf_md_params.insert(
        "paper_size".to_string(),
        "string ('a4', 'letter', 'a3', 'a5', default 'a4')".to_string(),
    );
    pdf_md_params.insert(
        "margin".to_string(),
        "string (e.g. '20mm', '1in', default '20mm')".to_string(),
    );
    pdf_md_params.insert(
        "font_family".to_string(),
        "string (custom font family name)".to_string(),
    );
    pdf_md_params.insert(
        "header".to_string(),
        "string (custom header text)".to_string(),
    );
    pdf_md_params.insert(
        "footer".to_string(),
        "string (custom footer text)".to_string(),
    );
    pdf_md_params.insert(
        "no_page_numbers".to_string(),
        "boolean (default false)".to_string(),
    );
    pdf_md_params.insert(
        "toc".to_string(),
        "boolean (automatically generate table of contents, default false)".to_string(),
    );
    pdf_md_params.insert(
        "overwrite".to_string(),
        "boolean (default false)".to_string(),
    );
    list.push(FormatCapability {
        from: "md".to_string(),
        to: "pdf".to_string(),
        description: "Convert Markdown documents to PDF files with Pure Rust engine".to_string(),
        parameters: pdf_md_params,
    });

    list
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
    if from == Format::Md && to == Format::Pdf {
        let converter = md_pdf::MarkdownPdfConverter;
        converter.convert_markdown_to_pdf(input_data, options)
    } else if from == Format::Md && to == Format::Docx {
        let converter = md_conv::MarkdownConverter;
        converter.convert_markdown(input_data, options)
    } else if from == Format::Pdf {
        let converter = pdf_conv::PdfConverter;
        converter.convert_pdf(to, input_data, options)
    } else if to == Format::Pdf {
        Err(MartiniError::UnsupportedConversion {
            from: from.to_string(),
            to: to.to_string(),
        })
    } else {
        let converter = image_conv::ImageConverter;
        converter.convert_image(from, to, input_data, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_supported_formats() {
        let formats = get_supported_formats();
        assert!(!formats.is_empty());
        assert!(formats.iter().any(|f| f.to == "favicon"));
    }
}
