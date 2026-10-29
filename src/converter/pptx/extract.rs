use crate::converter::pptx::ir::{ImageFormat, RectEmu};
use crate::error::MartiniError;
use pdfium_render::prelude::*;
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq)]
pub struct RawTextItem {
    pub text: String,
    pub rect: RectEmu,
    pub font_name: String,
    pub font_size_pt: f32,
    pub color_rgba: (u8, u8, u8, u8),
    pub bold: bool,
    pub italic: bool,
    pub rotation_degrees: f32,
    pub render_mode_invisible: bool,
}

impl RawTextItem {
    pub fn simple(text: &str, x: i64, y: i64) -> Self {
        Self {
            text: text.to_string(),
            rect: RectEmu::new(x, y, 200_000, 30_000),
            font_name: "Arial".to_string(),
            font_size_pt: 12.0,
            color_rgba: (0, 0, 0, 255),
            bold: false,
            italic: false,
            rotation_degrees: 0.0,
            render_mode_invisible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawPathSegment {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierTo(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawPathItem {
    pub rect: RectEmu,
    pub segments: Vec<RawPathSegment>,
    pub stroke_color: Option<(u8, u8, u8, u8)>,
    pub stroke_width_pt: f32,
    pub fill_color: Option<(u8, u8, u8, u8)>,
    pub is_even_odd: bool,
}

impl RawPathItem {
    pub fn horizontal_line(x: i64, y: i64, len: i64) -> Self {
        Self {
            rect: RectEmu::new(x, y, len, 12700),
            segments: vec![
                RawPathSegment::MoveTo((x as f32) / 12700.0, (y as f32) / 12700.0),
                RawPathSegment::LineTo(((x + len) as f32) / 12700.0, (y as f32) / 12700.0),
            ],
            stroke_color: Some((0, 0, 0, 255)),
            stroke_width_pt: 1.0,
            fill_color: None,
            is_even_odd: false,
        }
    }

    pub fn vertical_line(x: i64, y: i64, len: i64) -> Self {
        Self {
            rect: RectEmu::new(x, y, 12700, len),
            segments: vec![
                RawPathSegment::MoveTo((x as f32) / 12700.0, (y as f32) / 12700.0),
                RawPathSegment::LineTo((x as f32) / 12700.0, ((y + len) as f32) / 12700.0),
            ],
            stroke_color: Some((0, 0, 0, 255)),
            stroke_width_pt: 1.0,
            fill_color: None,
            is_even_odd: false,
        }
    }

    pub fn rect(x: i64, y: i64, w: i64, h: i64) -> Self {
        Self {
            rect: RectEmu::new(x, y, w, h),
            segments: vec![
                RawPathSegment::MoveTo((x as f32) / 12700.0, (y as f32) / 12700.0),
                RawPathSegment::LineTo(((x + w) as f32) / 12700.0, (y as f32) / 12700.0),
                RawPathSegment::LineTo(((x + w) as f32) / 12700.0, ((y + h) as f32) / 12700.0),
                RawPathSegment::LineTo((x as f32) / 12700.0, ((y + h) as f32) / 12700.0),
                RawPathSegment::Close,
            ],
            stroke_color: None,
            stroke_width_pt: 0.0,
            fill_color: Some((68, 114, 196, 255)),
            is_even_odd: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawImageItem {
    pub rect: RectEmu,
    pub image_bytes: Vec<u8>,
    pub format: ImageFormat,
    pub rotation_degrees: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawPdfPage {
    pub width_emu: i64,
    pub height_emu: i64,
    pub text_items: Vec<RawTextItem>,
    pub path_items: Vec<RawPathItem>,
    pub image_items: Vec<RawImageItem>,
}

pub fn extract_page_objects(
    document: &PdfDocument,
    page_index: u16,
) -> Result<RawPdfPage, MartiniError> {
    let page = document
        .pages()
        .get(page_index as PdfPageIndex)
        .map_err(|e| {
            MartiniError::PdfRender(format!("Failed to load page {}: {:?}", page_index + 1, e))
        })?;

    let page_width_pt = page.width().value;
    let page_height_pt = page.height().value;
    let width_emu = (page_width_pt * 12700.0).round() as i64;
    let height_emu = (page_height_pt * 12700.0).round() as i64;

    let mut text_items = Vec::new();
    let mut path_items = Vec::new();
    let mut image_items = Vec::new();

    for obj in page.objects().iter() {
        if let Some(text_obj) = obj.as_text_object() {
            let text = text_obj.text();
            if text.trim().is_empty() {
                continue;
            }

            let quad = text_obj.bounds().map_err(|e| {
                MartiniError::PdfRender(format!("Failed to get text bounds: {:?}", e))
            })?;

            let x1 = quad.x1().value;
            let y1 = quad.y1().value;
            let x2 = quad.x2().value;
            let y2 = quad.y2().value;
            let x3 = quad.x3().value;
            let y3 = quad.y3().value;
            let x4 = quad.x4().value;
            let y4 = quad.y4().value;

            let x_min = x1.min(x2).min(x3).min(x4);
            let x_max = x1.max(x2).max(x3).max(x4);
            let y_min = y1.min(y2).min(y3).min(y4);
            let y_max = y1.max(y2).max(y3).max(y4);

            let rect = RectEmu::new(
                (x_min * 12700.0).round() as i64,
                ((page_height_pt - y_max) * 12700.0).round() as i64,
                ((x_max - x_min).max(0.1) * 12700.0).round() as i64,
                ((y_max - y_min).max(0.1) * 12700.0).round() as i64,
            );

            // Compute baseline rotation angle
            let dx = x2 - x1;
            let dy = y2 - y1;
            let mut angle_deg = dy.atan2(dx).to_degrees();
            if angle_deg < 0.0 {
                angle_deg += 360.0;
            }
            let rotation_degrees = (360.0 - angle_deg) % 360.0;

            let font = text_obj.font();
            let font_name = font.name();
            let is_bold_weight = match font.weight() {
                Ok(
                    PdfFontWeight::Weight700Bold
                    | PdfFontWeight::Weight800
                    | PdfFontWeight::Weight900,
                ) => true,
                Ok(PdfFontWeight::Custom(w)) if w >= 700 => true,
                _ => false,
            };
            let bold = is_bold_weight
                || font.is_bold_reenforced()
                || font_name.to_lowercase().contains("bold");
            let italic = font.is_italic() || font_name.to_lowercase().contains("italic");
            let font_size_pt = text_obj.scaled_font_size().value;

            let color = text_obj.fill_color().unwrap_or(PdfColor::new(0, 0, 0, 255));
            let color_rgba = (color.red(), color.green(), color.blue(), color.alpha());

            let render_mode_invisible = matches!(
                text_obj.render_mode(),
                PdfPageTextRenderMode::Invisible | PdfPageTextRenderMode::InvisibleClipping
            );

            text_items.push(RawTextItem {
                text,
                rect,
                font_name,
                font_size_pt,
                color_rgba,
                bold,
                italic,
                rotation_degrees,
                render_mode_invisible,
            });
        } else if let Some(path_obj) = obj.as_path_object() {
            let quad = match path_obj.bounds() {
                Ok(q) => q,
                Err(_) => continue,
            };

            let x1 = quad.x1().value;
            let y1 = quad.y1().value;
            let x2 = quad.x2().value;
            let y2 = quad.y2().value;
            let x3 = quad.x3().value;
            let y3 = quad.y3().value;
            let x4 = quad.x4().value;
            let y4 = quad.y4().value;

            let x_min = x1.min(x2).min(x3).min(x4);
            let x_max = x1.max(x2).max(x3).max(x4);
            let y_min = y1.min(y2).min(y3).min(y4);
            let y_max = y1.max(y2).max(y3).max(y4);

            let rect = RectEmu::new(
                (x_min * 12700.0).round() as i64,
                ((page_height_pt - y_max) * 12700.0).round() as i64,
                ((x_max - x_min).max(0.1) * 12700.0).round() as i64,
                ((y_max - y_min).max(0.1) * 12700.0).round() as i64,
            );

            let stroke_color = if path_obj.is_stroked().unwrap_or(false) {
                path_obj
                    .stroke_color()
                    .ok()
                    .map(|c| (c.red(), c.green(), c.blue(), c.alpha()))
            } else {
                None
            };

            let stroke_width_pt = path_obj.stroke_width().map(|pt| pt.value).unwrap_or(1.0);

            let fill_color = match path_obj.fill_mode() {
                Ok(mode) if mode != PdfPathFillMode::None => path_obj
                    .fill_color()
                    .ok()
                    .map(|c| (c.red(), c.green(), c.blue(), c.alpha())),
                _ => None,
            };

            let is_even_odd = matches!(path_obj.fill_mode(), Ok(PdfPathFillMode::EvenOdd));

            let mut segments = Vec::new();
            for seg in path_obj.segments().iter() {
                let (px, py) = seg.point();
                let x_pt = px.value;
                let y_pt = page_height_pt - py.value;
                match seg.segment_type() {
                    PdfPathSegmentType::MoveTo => {
                        segments.push(RawPathSegment::MoveTo(x_pt, y_pt));
                    }
                    PdfPathSegmentType::LineTo => {
                        segments.push(RawPathSegment::LineTo(x_pt, y_pt));
                    }
                    PdfPathSegmentType::BezierTo => {
                        segments.push(RawPathSegment::BezierTo(x_pt, y_pt, x_pt, y_pt, x_pt, y_pt));
                    }
                    _ => {}
                }
                if seg.is_close() {
                    segments.push(RawPathSegment::Close);
                }
            }

            path_items.push(RawPathItem {
                rect,
                segments,
                stroke_color,
                stroke_width_pt,
                fill_color,
                is_even_odd,
            });
        } else if let Some(image_obj) = obj.as_image_object() {
            let quad = match image_obj.bounds() {
                Ok(q) => q,
                Err(_) => continue,
            };

            let x1 = quad.x1().value;
            let y1 = quad.y1().value;
            let x2 = quad.x2().value;
            let y2 = quad.y2().value;
            let x3 = quad.x3().value;
            let y3 = quad.y3().value;
            let x4 = quad.x4().value;
            let y4 = quad.y4().value;

            let x_min = x1.min(x2).min(x3).min(x4);
            let x_max = x1.max(x2).max(x3).max(x4);
            let y_min = y1.min(y2).min(y3).min(y4);
            let y_max = y1.max(y2).max(y3).max(y4);

            let rect = RectEmu::new(
                (x_min * 12700.0).round() as i64,
                ((page_height_pt - y_max) * 12700.0).round() as i64,
                ((x_max - x_min).max(0.1) * 12700.0).round() as i64,
                ((y_max - y_min).max(0.1) * 12700.0).round() as i64,
            );

            // Review Focus 4: Try getting processed image (preserving alpha/transparency mask)
            let (image_bytes, format) =
                if let Ok(dyn_img) = image_obj.get_processed_image(document) {
                    let mut png_bytes = Vec::new();
                    if dyn_img
                        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
                        .is_ok()
                    {
                        (png_bytes, ImageFormat::Png)
                    } else if let Ok(raw) = image_obj.get_raw_image_data() {
                        (raw, ImageFormat::Jpeg)
                    } else {
                        continue;
                    }
                } else if let Ok(raw) = image_obj.get_raw_image_data() {
                    (raw, ImageFormat::Jpeg)
                } else {
                    continue;
                };

            image_items.push(RawImageItem {
                rect,
                image_bytes,
                format,
                rotation_degrees: 0.0,
            });
        }
    }

    Ok(RawPdfPage {
        width_emu,
        height_emu,
        text_items,
        path_items,
        image_items,
    })
}
