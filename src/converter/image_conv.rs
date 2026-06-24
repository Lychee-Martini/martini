use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::imageops::FilterType;
use image::{ColorType, DynamicImage, ImageFormat};
use resvg::tiny_skia;
use resvg::usvg;
use std::fs;
use std::io::Cursor;

use crate::converter::{
    ConversionResult, ConvertOptions, Converter, EncodedFile, Format, OutputFileMetadata,
};
use crate::error::MartiniError;

pub struct ImageConverter;

impl ImageConverter {
    pub fn convert_image(
        &self,
        from: Format,
        to: Format,
        input_data: &[u8],
        options: &ConvertOptions,
    ) -> Result<ConversionResult, MartiniError> {
        // 1. Loading & Decoding Phase
        let (tree, mut target_img) = if from == Format::Svg {
            let opt = usvg::Options::default();
            let svg_tree = usvg::Tree::from_data(input_data, &opt)?;

            // If we're not converting to Favicon, render it at natural size to target_img
            if to != Format::Favicon {
                let w = svg_tree.size().width().round() as u32;
                let h = svg_tree.size().height().round() as u32;
                let rgba = render_svg_to_rgba(&svg_tree, w, h)?;
                let img_buffer =
                    image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(w, h, rgba)
                        .ok_or_else(|| {
                            MartiniError::Rendering(
                                "Failed to create ImageBuffer from SVG".to_string(),
                            )
                        })?;
                (Some(svg_tree), Some(DynamicImage::ImageRgba8(img_buffer)))
            } else {
                (Some(svg_tree), None)
            }
        } else {
            let img = image::load_from_memory(input_data)?;
            (None, Some(img))
        };

        // 2. Perform color format standardisation for raster outputs if needed
        if to != Format::Favicon
            && let Some(ref img) = target_img
        {
            let color_type = img.color();
            let converted = match color_type {
                ColorType::La8 | ColorType::La16 | ColorType::Rgba8 | ColorType::Rgba16 => {
                    DynamicImage::ImageRgba8(img.to_rgba8())
                }
                _ => DynamicImage::ImageRgb8(img.to_rgb8()),
            };
            target_img = Some(converted);
        }

        // 3. Dispatch to specific format strategy
        let encoder: Box<dyn Encoder> = match to {
            Format::Webp => Box::new(WebpEncoder),
            Format::Avif => Box::new(AvifEncoder),
            Format::Png => Box::new(PngEncoder),
            Format::Jpg => Box::new(JpegEncoder),
            Format::Favicon => Box::new(FaviconEncoder),
            _ => {
                return Err(MartiniError::UnsupportedConversion {
                    from: from.to_string(),
                    to: to.to_string(),
                });
            }
        };

        let encoded_files =
            encoder.encode(from, to, tree.as_ref(), target_img.as_ref(), options)?;

        let mut output_files = Vec::new();
        for file in encoded_files {
            if let Some(parent) = file.path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent)?;
            }
            fs::write(&file.path, &file.bytes)?;
            output_files.push(OutputFileMetadata {
                path: file.path.to_string_lossy().to_string(),
                size_bytes: file.bytes.len() as u64,
                description: file.description,
            });
        }

        Ok(ConversionResult {
            from,
            to,
            output_files,
        })
    }
}

// Converter implementation for backwards compatibility / generics
impl Converter for ImageConverter {
    fn convert(
        &self,
        input_data: &[u8],
        options: &ConvertOptions,
    ) -> Result<ConversionResult, MartiniError> {
        // Fallback default conversion
        self.convert_image(Format::Png, Format::Webp, input_data, options)
    }
}

// --- Strategy Pattern for Image Format Encoders ---

pub trait Encoder {
    fn encode(
        &self,
        from: Format,
        to: Format,
        tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError>;
}

pub(crate) struct WebpEncoder;
impl Encoder for WebpEncoder {
    fn encode(
        &self,
        from: Format,
        to: Format,
        _tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError> {
        let target_img = img.ok_or_else(|| MartiniError::InvalidInputData {
            reason: "Failed to decode/render input image".to_string(),
        })?;

        let webp_encoder = webp::Encoder::from_image(target_img).map_err(|e| {
            MartiniError::Rendering(format!("Failed to create WebP encoder: {}", e))
        })?;
        let webp_data = if options.lossless {
            webp_encoder.encode_lossless()
        } else {
            webp_encoder.encode(options.quality as f32)
        };

        let bytes = webp_data.to_vec();
        let description = format!(
            "Converted image from {} to {} (quality: {}, lossless: {})",
            from, to, options.quality, options.lossless
        );
        Ok(vec![EncodedFile {
            path: options.output_path.clone(),
            bytes,
            description,
        }])
    }
}

pub(crate) struct AvifEncoder;
impl Encoder for AvifEncoder {
    fn encode(
        &self,
        from: Format,
        to: Format,
        _tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError> {
        let target_img = img.ok_or_else(|| MartiniError::InvalidInputData {
            reason: "Failed to decode/render input image".to_string(),
        })?;

        let avif_bytes = encode_avif(target_img, options.quality, options.lossless)?;
        let description = format!(
            "Converted image from {} to {} (quality: {}, lossless: {})",
            from, to, options.quality, options.lossless
        );
        Ok(vec![EncodedFile {
            path: options.output_path.clone(),
            bytes: avif_bytes,
            description,
        }])
    }
}

pub(crate) struct PngEncoder;
impl Encoder for PngEncoder {
    fn encode(
        &self,
        from: Format,
        to: Format,
        _tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError> {
        let target_img = img.ok_or_else(|| MartiniError::InvalidInputData {
            reason: "Failed to decode/render input image".to_string(),
        })?;

        let mut png_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        target_img.write_to(&mut cursor, ImageFormat::Png)?;

        let description = format!(
            "Converted image from {} to {} (quality: {}, lossless: {})",
            from, to, options.quality, options.lossless
        );
        Ok(vec![EncodedFile {
            path: options.output_path.clone(),
            bytes: png_bytes,
            description,
        }])
    }
}

pub(crate) struct JpegEncoder;
impl Encoder for JpegEncoder {
    fn encode(
        &self,
        from: Format,
        to: Format,
        _tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError> {
        let target_img = img.ok_or_else(|| MartiniError::InvalidInputData {
            reason: "Failed to decode/render input image".to_string(),
        })?;

        let mut jpeg_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut jpeg_bytes);
        target_img.write_to(&mut cursor, ImageFormat::Jpeg)?;

        let description = format!(
            "Converted image from {} to {} (quality: {}, lossless: {})",
            from, to, options.quality, options.lossless
        );
        Ok(vec![EncodedFile {
            path: options.output_path.clone(),
            bytes: jpeg_bytes,
            description,
        }])
    }
}

pub(crate) struct FaviconEncoder;
impl Encoder for FaviconEncoder {
    fn encode(
        &self,
        _from: Format,
        _to: Format,
        tree: Option<&usvg::Tree>,
        img: Option<&DynamicImage>,
        options: &ConvertOptions,
    ) -> Result<Vec<EncodedFile>, MartiniError> {
        generate_favicon(tree, img, options)
    }
}

fn encode_avif(img: &DynamicImage, quality: u8, lossless: bool) -> Result<Vec<u8>, MartiniError> {
    let rgba_img = img.to_rgba8();
    let width = rgba_img.width() as usize;
    let height = rgba_img.height() as usize;
    let raw_pixels = rgba_img.as_raw();

    let pixels: Vec<ravif::RGBA8> = raw_pixels
        .chunks_exact(4)
        .map(|c| ravif::RGBA8 {
            r: c[0],
            g: c[1],
            b: c[2],
            a: c[3],
        })
        .collect();

    let imgref = imgref::Img::new(pixels.as_slice(), width, height);

    let qual = if lossless { 100.0 } else { quality as f32 };

    let res = ravif::Encoder::new()
        .with_quality(qual)
        .with_speed(4)
        .encode_rgba(imgref)
        .map_err(|e| MartiniError::Rendering(format!("AVIF encoding failed: {:?}", e)))?;

    Ok(res.avif_file)
}

pub(crate) fn render_svg_to_rgba(
    tree: &usvg::Tree,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, MartiniError> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        MartiniError::Rendering(format!(
            "Failed to create tiny-skia Pixmap of size {}x{}",
            width, height
        ))
    })?;

    let svg_w = tree.size().width();
    let svg_h = tree.size().height();

    let scale = (width as f32 / svg_w).min(height as f32 / svg_h);
    let dx = (width as f32 - (svg_w * scale)) / 2.0;
    let dy = (height as f32 - (svg_h * scale)) / 2.0;

    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);

    resvg::render(tree, transform, &mut pixmap.as_mut());

    let raw_pixels = demultiply_alpha(pixmap.data());
    Ok(raw_pixels)
}

fn demultiply_alpha(data: &[u8]) -> Vec<u8> {
    let mut unpremultiplied = Vec::with_capacity(data.len());
    for chunk in data.chunks_exact(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let a = chunk[3];
        if a == 0 {
            unpremultiplied.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let r_un = ((r as u32 * 255) / a as u32).min(255) as u8;
            let g_un = ((g as u32 * 255) / a as u32).min(255) as u8;
            let b_un = ((b as u32 * 255) / a as u32).min(255) as u8;
            unpremultiplied.extend_from_slice(&[r_un, g_un, b_un, a]);
        }
    }
    unpremultiplied
}

fn get_png_bytes(
    tree: Option<&usvg::Tree>,
    img: Option<&DynamicImage>,
    size: u32,
) -> Result<Vec<u8>, MartiniError> {
    if let Some(t) = tree {
        let rgba = render_svg_to_rgba(t, size, size)?;
        rgba_to_png(rgba, size, size)
    } else if let Some(i) = img {
        let resized = i.resize_exact(size, size, FilterType::Lanczos3);
        let mut png_bytes = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        resized.write_to(&mut cursor, ImageFormat::Png)?;
        Ok(png_bytes)
    } else {
        Err(MartiniError::InvalidInputData {
            reason: "No image source found for resizing".to_string(),
        })
    }
}

fn rgba_to_png(rgba: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, MartiniError> {
    let img = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
        .ok_or_else(|| {
            MartiniError::Rendering("Failed to create ImageBuffer from raw pixels".to_string())
        })?;

    let mut png_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(png_bytes)
}

fn build_ico(png_buffers: &[(Vec<u8>, u32)]) -> Result<Vec<u8>, MartiniError> {
    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for (png_data, _) in png_buffers {
        let image = IconImage::read_png(Cursor::new(png_data))?;
        icon_dir.add_entry(IconDirEntry::encode(&image)?);
    }
    let mut ico_bytes = Vec::new();
    icon_dir.write(&mut ico_bytes)?;
    Ok(ico_bytes)
}

fn generate_favicon(
    tree: Option<&usvg::Tree>,
    img: Option<&DynamicImage>,
    options: &ConvertOptions,
) -> Result<Vec<EncodedFile>, MartiniError> {
    let mut output_files = Vec::new();

    if options.package {
        let (dir_path, ico_name) =
            if options.output_path.is_dir() || options.output_path.extension().is_none() {
                (options.output_path.clone(), "favicon.ico".to_string())
            } else {
                let parent = options
                    .output_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                let name = options
                    .output_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "favicon.ico".to_string());
                (parent, name)
            };

        let sizes = [
            (
                16,
                "favicon-16x16.png",
                "Standard small favicon for browser tabs",
            ),
            (
                32,
                "favicon-32x32.png",
                "Standard medium favicon for desktop browsers",
            ),
            (
                180,
                "apple-touch-icon.png",
                "Apple Touch Icon for iOS home screen",
            ),
            (
                192,
                "android-chrome-192x192.png",
                "Android Chrome icon for web app manifest",
            ),
            (
                512,
                "android-chrome-512x512.png",
                "Android Chrome splash icon for web app manifest",
            ),
        ];

        let mut png_buffers = Vec::new();
        for &(size, filename, desc) in &sizes {
            let png_data = get_png_bytes(tree, img, size)?;

            if size == 16 || size == 32 {
                png_buffers.push((png_data.clone(), size));
            }

            let file_path = dir_path.join(filename);
            output_files.push(EncodedFile {
                path: file_path,
                bytes: png_data,
                description: desc.to_string(),
            });
        }

        // Render a 48x48 specifically for the ICO
        let png_48 = get_png_bytes(tree, img, 48)?;
        png_buffers.push((png_48, 48));

        let ico_bytes = build_ico(&png_buffers)?;
        let ico_path = dir_path.join(ico_name);
        output_files.push(EncodedFile {
            path: ico_path,
            bytes: ico_bytes,
            description: "Multi-resolution Windows favicon (16x16, 32x32, 48x48)".to_string(),
        });

        // Write webmanifest
        let manifest_content = r##"{
    "name": "Lychee Martini Web App",
    "short_name": "Lychee Martini",
    "icons": [
        {
            "src": "/android-chrome-192x192.png",
            "sizes": "192x192",
            "type": "image/png"
        },
        {
            "src": "/android-chrome-512x512.png",
            "sizes": "512x512",
            "type": "image/png"
        }
    ],
    "theme_color": "#ffffff",
    "background_color": "#ffffff",
    "display": "standalone"
}
"##;
        let manifest_path = dir_path.join("site.webmanifest");
        output_files.push(EncodedFile {
            path: manifest_path,
            bytes: manifest_content.as_bytes().to_vec(),
            description: "Web App Manifest containing icons definitions".to_string(),
        });

        // Write HTML snippet
        let html_content = r#"<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="manifest" href="/site.webmanifest">
"#;
        let html_path = dir_path.join("favicon-tags.html");
        output_files.push(EncodedFile {
            path: html_path,
            bytes: html_content.as_bytes().to_vec(),
            description: "HTML header tags to copy-paste into index.html".to_string(),
        });
    } else {
        // Single ICO file output
        let sizes = [16, 32, 48];
        let mut png_buffers = Vec::new();
        for &size in &sizes {
            let png = get_png_bytes(tree, img, size)?;
            png_buffers.push((png, size));
        }

        let ico_bytes = build_ico(&png_buffers)?;
        output_files.push(EncodedFile {
            path: options.output_path.clone(),
            bytes: ico_bytes,
            description: "Multi-resolution Windows favicon (16x16, 32x32, 48x48)".to_string(),
        });
    }

    Ok(output_files)
}
