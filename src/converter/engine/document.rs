use super::FormatEngine;
use crate::converter::{
    ConvertOptions, Format, FormatCapability, OutputFileMetadata, md_conv, md_pdf, pdf_conv,
};
use crate::error::MartiniError;
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// Markdown -> PDF/DOCX and PDF -> raster images.
pub struct DocumentEngine;

impl FormatEngine for DocumentEngine {
    fn can_convert(&self, from: Format, to: Format) -> bool {
        matches!(
            (from, to),
            (Format::Md, Format::Pdf | Format::Docx)
                | (
                    Format::Pdf,
                    Format::Png | Format::Jpg | Format::Webp | Format::Avif
                )
        )
    }

    fn capabilities(&self) -> Vec<FormatCapability> {
        vec![
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
        ]
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError> {
        let from = detect_format(input)?;
        let to = detect_format(output)?;

        let input_bytes = fs::read(input)?;
        let opts = ConvertOptions {
            input_path: input.to_path_buf(),
            output_path: output.to_path_buf(),
            ..options.clone()
        };

        let res = match (from, to) {
            (Format::Md, Format::Pdf) => md_pdf::convert_markdown_to_pdf(&input_bytes, &opts)?,
            (Format::Md, Format::Docx) => md_conv::convert_markdown(&input_bytes, &opts)?,
            (Format::Pdf, _) => pdf_conv::convert_pdf(to, &input_bytes, &opts)?,
            _ => {
                return Err(MartiniError::UnsupportedConversion {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        };

        // PDF -> images may produce one file per page; report the first.
        res.output_files
            .into_iter()
            .next()
            .ok_or_else(|| MartiniError::InvalidInputData {
                reason: format!("Conversion from {} to {} produced no output", from, to),
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
        reason: format!("Unsupported document format extension: '{}'", ext),
    })
}
