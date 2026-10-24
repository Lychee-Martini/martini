use pdfium_render::prelude::*;
use std::fs;
use std::sync::OnceLock;

use crate::converter::image_conv::encode_image;
use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

pub fn get_pdfium() -> Result<&'static Pdfium, MartiniError> {
    if let Some(pdfium) = PDFIUM.get() {
        return Ok(pdfium);
    }

    let lib_path = pdfium_auto::ensure_pdfium_library(None).map_err(|e| {
        MartiniError::PdfRender(format!("Failed to locate/download PDFium library: {:?}", e))
    })?;

    let bindings = match Pdfium::bind_to_library(&lib_path) {
        Ok(b) => b,
        Err(e) => {
            if lib_path.is_file() {
                let _ = fs::remove_file(&lib_path);
            }
            let fresh_path = pdfium_auto::ensure_pdfium_library(None).map_err(|e2| {
                MartiniError::PdfRender(format!("Failed to re-download PDFium library: {:?}", e2))
            })?;
            Pdfium::bind_to_library(&fresh_path).map_err(|e2| {
                MartiniError::PdfRender(format!(
                    "Failed to load PDFium library: {:?} (retry: {:?})",
                    e, e2
                ))
            })?
        }
    };

    Ok(PDFIUM.get_or_init(|| Pdfium::new(bindings)))
}

pub fn convert_pdf(
    to: Format,
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    // 1. Bind to PDFium library
    let pdfium = get_pdfium()?;

    // 2. Load the PDF document
    let document = pdfium
        .load_pdf_from_byte_slice(input_data, None)
        .map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Failed to parse PDF document: {:?}", e),
        })?;

    let total_pages = document.pages().len();
    if total_pages <= 0 {
        return Err(MartiniError::InvalidInputData {
            reason: "PDF document has no pages".to_string(),
        });
    }

    // 3. Resolve page selection
    let pages_to_render = if let Some(ref range_str) = options.pages {
        parse_pages(range_str, total_pages).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Invalid page selection: {}", e),
        })?
    } else {
        (0..total_pages).collect()
    };

    if pages_to_render.is_empty() {
        return Err(MartiniError::InvalidInputData {
            reason: "No pages selected for conversion".to_string(),
        });
    }

    // 4. Validate output format
    if !matches!(to, Format::Webp | Format::Avif | Format::Png | Format::Jpg) {
        return Err(MartiniError::UnsupportedConversion {
            from: "pdf".to_string(),
            to: to.to_string(),
        });
    }

    // 5. Setup rendering configuration (DPI)
    let dpi = options.dpi.max(1);
    let render_config = PdfRenderConfig::new()
        .scale_page_by_factor(dpi as f32 / 72.0)
        .render_form_data(true)
        .render_annotations(true);

    let mut output_files = Vec::new();

    for &page_index in &pages_to_render {
        // Get page
        let page = document.pages().get(page_index).map_err(|e| {
            MartiniError::PdfRender(format!(
                "Failed to get PDF page {}: {:?}",
                page_index + 1,
                e
            ))
        })?;

        // Render page to dynamic image
        let img = page.render_with_config(&render_config).map_err(|e| {
            MartiniError::PdfRender(format!("Failed to render page {}: {:?}", page_index + 1, e))
        })?;
        let mut dynamic_img = img.as_image().map_err(|e| {
            MartiniError::PdfRender(format!(
                "Failed to convert rendered page {} to image: {:?}",
                page_index + 1,
                e
            ))
        })?;
        if options.width.is_some() || options.height.is_some() {
            let (w, h) = crate::converter::image_conv::calculate_dimensions(
                dynamic_img.width(),
                dynamic_img.height(),
                options.width,
                options.height,
                options.no_upscale,
            );
            if (w, h) != (dynamic_img.width(), dynamic_img.height()) {
                dynamic_img = dynamic_img.resize_exact(w, h, image::imageops::FilterType::Lanczos3);
            }
        }

        // Setup page-specific output path
        let page_num = page_index + 1;
        let stem = options
            .output_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let ext = options
            .output_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let page_filename = format!("{}_page_{}.{}", stem, page_num, ext);
        let page_output_path = options.output_path.with_file_name(page_filename);

        let page_options = ConvertOptions {
            input_path: options.input_path.clone(),
            output_path: page_output_path.clone(),
            package: false,
            quality: options.quality,
            lossless: options.lossless,
            overwrite: options.overwrite,
            pages: None,
            dpi: options.dpi,
            width: options.width,
            height: options.height,
            no_upscale: options.no_upscale,
            theme: options.theme.clone(),
            paper_size: options.paper_size.clone(),
            margin: options.margin.clone(),
            font_family: options.font_family.clone(),
            header: options.header.clone(),
            footer: options.footer.clone(),
            no_page_numbers: options.no_page_numbers,
            toc: options.toc,
        };

        // Encode to target format
        let encoded_files = encode_image(Format::Pdf, to, None, Some(&dynamic_img), &page_options)?;

        for file in encoded_files {
            if let Some(parent) = file.path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file.path, &file.bytes)?;
            output_files.push(OutputFileMetadata {
                path: file.path.to_string_lossy().to_string(),
                size_bytes: file.bytes.len() as u64,
                description: format!("Page {} of PDF", page_num),
            });
        }
    }

    Ok(ConversionResult {
        from: Format::Pdf,
        to,
        output_files,
    })
}

pub fn parse_pages(pages_str: &str, total_pages: i32) -> Result<Vec<i32>, String> {
    let mut pages = Vec::new();
    for part in pages_str.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').collect();
            if bounds.len() != 2 {
                return Err(format!("Invalid page range: {}", part));
            }
            let start = bounds[0]
                .trim()
                .parse::<i32>()
                .map_err(|_| format!("Invalid page number: {}", bounds[0]))?;
            let end = bounds[1]
                .trim()
                .parse::<i32>()
                .map_err(|_| format!("Invalid page number: {}", bounds[1]))?;
            if start <= 0 || end <= 0 {
                return Err("Page numbers must be 1-indexed and greater than 0".to_string());
            }
            if start > end {
                return Err(format!(
                    "Start page {} is greater than end page {}",
                    start, end
                ));
            }
            for p in start..=end {
                if p <= total_pages {
                    pages.push(p - 1); // 0-indexed internally
                }
            }
        } else {
            let p = part
                .parse::<i32>()
                .map_err(|_| format!("Invalid page number: {}", part))?;
            if p <= 0 {
                return Err("Page numbers must be 1-indexed and greater than 0".to_string());
            }
            if p <= total_pages {
                pages.push(p - 1); // 0-indexed internally
            }
        }
    }
    pages.sort();
    pages.dedup();
    Ok(pages)
}
