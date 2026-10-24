use crate::converter::md_to_typst::{MarkdownToTypst, MdToTypstOptions};
use crate::converter::typst_engine::compile_typst_to_pdf;
use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use std::fs;
use std::path::Path;

pub fn convert_markdown_to_pdf(
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    let markdown_str =
        std::str::from_utf8(input_data).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Input is not valid UTF-8: {}", e),
        })?;

    let base_dir = options
        .input_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let md_options = MdToTypstOptions {
        theme: options.theme.clone(),
        paper_size: options.paper_size.clone(),
        margin: options.margin.clone(),
        font_family: options.font_family.clone(),
        header: options.header.clone(),
        footer: options.footer.clone(),
        no_page_numbers: options.no_page_numbers,
        toc: options.toc,
    };

    let typst_markup = MarkdownToTypst::translate(markdown_str, &md_options, base_dir);

    let pdf_bytes = compile_typst_to_pdf(&typst_markup, base_dir)?;

    if let Some(parent) = options
        .output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(&options.output_path, &pdf_bytes)?;

    let size = options
        .output_path
        .metadata()
        .map(|m| m.len())
        .unwrap_or(pdf_bytes.len() as u64);

    Ok(ConversionResult {
        from: Format::Md,
        to: Format::Pdf,
        output_files: vec![OutputFileMetadata {
            path: options.output_path.to_string_lossy().to_string(),
            size_bytes: size,
            description: "Converted from Markdown to PDF".to_string(),
        }],
    })
}
