use crate::error::MartiniError;
use std::path::PathBuf;
use std::str::FromStr;

pub mod batch;
pub mod image_conv;
pub mod pdf_conv;
pub mod md_conv;

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
    Md,
    Docx,
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
            "md" | "markdown" => Ok(Format::Md),
            "docx" => Ok(Format::Docx),
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
