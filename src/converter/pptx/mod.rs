use crate::converter::pdf_conv::{get_pdfium, parse_pages};
use crate::converter::pptx::chart::detect_charts;
use crate::converter::pptx::extract::extract_page_objects;
use crate::converter::pptx::ir::{
    ColorRgb, Element, ImageFormat, Picture, RectEmu, Shape, ShapeGeom, Slide, SlideIR,
};
use crate::converter::pptx::paragraph::cluster_paragraphs;
use crate::converter::pptx::table::detect_tables;
use crate::converter::pptx::writer::write_pptx;
use crate::converter::{ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use pdfium_render::prelude::*;
use std::io::Cursor;

pub mod chart;
pub mod extract;
pub mod font;
pub mod ir;
pub mod paragraph;
pub mod table;
pub mod writer;

pub fn convert_pdf_to_pptx(
    input_data: &[u8],
    options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    let pdfium = get_pdfium()?;
    let document = pdfium
        .load_pdf_from_byte_slice(input_data, None)
        .map_err(|e| MartiniError::PdfRender(format!("Failed to load PDF document: {:?}", e)))?;

    let total_pages = document.pages().len();
    if total_pages == 0 {
        return Err(MartiniError::PdfRender(
            "PDF document has no pages".to_string(),
        ));
    }

    let pages_to_render = if let Some(ref range_str) = options.pages {
        parse_pages(range_str, total_pages).map_err(|e| MartiniError::InvalidInputData {
            reason: format!("Invalid page selection: {}", e),
        })?
    } else {
        (0..total_pages).collect()
    };

    // Determine default canvas size from the first page
    let first_page = document
        .pages()
        .get(pages_to_render[0] as PdfPageIndex)
        .map_err(|e| {
            MartiniError::PdfRender(format!(
                "Failed to get page {}: {:?}",
                pages_to_render[0] + 1,
                e
            ))
        })?;
    let default_width_emu = (first_page.width().value * 12700.0).round() as i64;
    let default_height_emu = (first_page.height().value * 12700.0).round() as i64;

    let mut ir = SlideIR::new(default_width_emu, default_height_emu);

    let dpi = options.dpi.max(72);
    let render_config = PdfRenderConfig::new()
        .scale_page_by_factor(dpi as f32 / 72.0)
        .render_form_data(true)
        .render_annotations(true);

    for &page_index in &pages_to_render {
        let page = document
            .pages()
            .get(page_index as PdfPageIndex)
            .map_err(|e| {
                MartiniError::PdfRender(format!("Failed to get page {}: {:?}", page_index + 1, e))
            })?;

        let page_width_emu = (page.width().value * 12700.0).round() as i64;
        let page_height_emu = (page.height().value * 12700.0).round() as i64;

        let mut slide = Slide::new(page_width_emu, page_height_emu);

        // Try extracting vector/text objects
        let raw_page_res = extract_page_objects(&document, page_index as u16);

        // Check if raster fallback is needed (error, or scan with 0 text and 1 large image or 0 elements)
        let is_raster_fallback = match &raw_page_res {
            Err(_) => true,
            Ok(raw) => {
                let has_visible_text = raw.text_items.iter().any(|t| !t.render_mode_invisible);
                if !has_visible_text {
                    if raw.image_items.len() == 1 {
                        let img_area = raw.image_items[0].rect.w * raw.image_items[0].rect.h;
                        let page_area = raw.width_emu * raw.height_emu;
                        img_area as f64 >= (page_area as f64) * 0.60
                    } else {
                        raw.text_items.is_empty()
                    }
                } else {
                    false
                }
            }
        };

        if is_raster_fallback {
            // Render full page to bitmap image
            let rendered_img = page.render_with_config(&render_config).map_err(|e| {
                MartiniError::PdfRender(format!(
                    "Failed to render page {}: {:?}",
                    page_index + 1,
                    e
                ))
            })?;
            let dyn_img = rendered_img.as_image().map_err(|e| {
                MartiniError::PdfRender(format!(
                    "Failed to convert rendered page {} to dynamic image: {:?}",
                    page_index + 1,
                    e
                ))
            })?;

            let mut png_bytes = Vec::new();
            dyn_img
                .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
                .map_err(|e| {
                    MartiniError::PdfRender(format!("Failed to encode rendered image: {:?}", e))
                })?;

            slide.elements.push(Element::Picture(Picture {
                rect: RectEmu::new(0, 0, page_width_emu, page_height_emu),
                image_bytes: png_bytes,
                format: ImageFormat::Png,
                rotation_degrees: 0.0,
            }));
        } else {
            let mut raw_page = raw_page_res.unwrap();

            // 1. Detect Tables
            let tables = detect_tables(&mut raw_page.path_items, &mut raw_page.text_items);

            // 2. Detect Charts
            let charts = detect_charts(&mut raw_page.path_items, &mut raw_page.text_items);

            // 3. Cluster Paragraphs
            let textboxes = cluster_paragraphs(raw_page.text_items);

            // 4. Convert remaining paths to Shapes
            let mut shapes = Vec::new();
            for path in raw_page.path_items {
                if path.fill_color.is_some() || path.stroke_color.is_some() {
                    let fill = path.fill_color.map(|c| ColorRgb::new(c.0, c.1, c.2, c.3));
                    let stroke = path
                        .stroke_color
                        .map(|c| (ColorRgb::new(c.0, c.1, c.2, c.3), path.stroke_width_pt));

                    shapes.push(Element::Shape(Shape {
                        rect: path.rect,
                        geom: ShapeGeom::Preset("rect".to_string()),
                        fill,
                        stroke,
                        rotation_degrees: 0.0,
                    }));
                }
            }

            // 5. Convert extracted images to Pictures
            let mut pictures = Vec::new();
            for img in raw_page.image_items {
                pictures.push(Element::Picture(Picture {
                    rect: img.rect,
                    image_bytes: img.image_bytes,
                    format: img.format,
                    rotation_degrees: img.rotation_degrees,
                }));
            }

            // Assemble slide elements (Background shapes -> Pictures -> Tables -> Charts -> TextBoxes)
            slide.elements.extend(shapes);
            slide.elements.extend(pictures);
            for tbl in tables {
                slide.elements.push(Element::Table(tbl));
            }
            slide.elements.extend(charts);
            for tb in textboxes {
                slide.elements.push(Element::TextBox(tb));
            }
        }

        ir.add_slide(slide);
    }

    // Write PPTX presentation package
    let pptx_bytes = write_pptx(&ir)?;

    // Ensure parent directory exists
    if let Some(parent) = options
        .output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(MartiniError::Io)?;
    }

    std::fs::write(&options.output_path, &pptx_bytes).map_err(MartiniError::Io)?;

    let size_bytes = pptx_bytes.len() as u64;
    let meta = OutputFileMetadata {
        path: options.output_path.to_string_lossy().to_string(),
        size_bytes,
        description: format!("PowerPoint presentation with {} slide(s)", ir.slides.len()),
    };

    Ok(ConversionResult {
        from: Format::Pdf,
        to: Format::Pptx,
        output_files: vec![meta],
    })
}
