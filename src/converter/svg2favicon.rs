use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::{ImageBuffer, ImageFormat, Rgba};
use resvg::tiny_skia;
use resvg::usvg;
use std::fs;
use std::io::Cursor;

use crate::converter::{ConversionResult, ConvertOptions, Converter, Format, OutputFileMetadata};
use crate::error::MartiniError;

pub struct SvgToFaviconConverter;

impl Converter for SvgToFaviconConverter {
    fn convert(
        &self,
        input_data: &[u8],
        options: &ConvertOptions,
    ) -> Result<ConversionResult, MartiniError> {
        // 1. Parse SVG data
        let opt = usvg::Options::default();
        let tree = usvg::Tree::from_data(input_data, &opt)?;

        let mut output_files = Vec::new();

        if options.package {
            // A package: generate target directory and create ICO, PNGs, manifest, and HTML tags snippet.
            let dir_path = &options.output_path;
            fs::create_dir_all(dir_path)?;

            // Render PNG sizes
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
                let rgba = render_to_rgba(&tree, size, size)?;
                let png_data = rgba_to_png(rgba, size, size)?;

                // Cache 16, 32 and 48 for ICO generation later (ICO doesn't strictly need 180/192/512)
                if size == 16 || size == 32 {
                    png_buffers.push((png_data.clone(), size));
                }

                let file_path = dir_path.join(filename);
                fs::write(&file_path, &png_data)?;

                output_files.push(OutputFileMetadata {
                    path: file_path.to_string_lossy().to_string(),
                    size_bytes: png_data.len() as u64,
                    description: desc.to_string(),
                });
            }

            // Render a 48x48 icon specifically for the ICO file
            let rgba_48 = render_to_rgba(&tree, 48, 48)?;
            let png_48 = rgba_to_png(rgba_48, 48, 48)?;
            png_buffers.push((png_48, 48));

            // Generate favicon.ico in the package dir
            let ico_bytes = build_ico(&png_buffers)?;
            let ico_path = dir_path.join("favicon.ico");
            fs::write(&ico_path, &ico_bytes)?;

            output_files.push(OutputFileMetadata {
                path: ico_path.to_string_lossy().to_string(),
                size_bytes: ico_bytes.len() as u64,
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
            fs::write(&manifest_path, manifest_content)?;

            output_files.push(OutputFileMetadata {
                path: manifest_path.to_string_lossy().to_string(),
                size_bytes: manifest_content.len() as u64,
                description: "Web App Manifest containing icons definitions".to_string(),
            });

            // Write HTML snippet
            let html_content = r#"<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="manifest" href="/site.webmanifest">
"#;
            let html_path = dir_path.join("favicon-tags.html");
            fs::write(&html_path, html_content)?;

            output_files.push(OutputFileMetadata {
                path: html_path.to_string_lossy().to_string(),
                size_bytes: html_content.len() as u64,
                description: "HTML header tags to copy-paste into index.html".to_string(),
            });
        } else {
            // Single ICO file output
            let sizes = [16, 32, 48];
            let mut png_buffers = Vec::new();
            for &size in &sizes {
                let rgba = render_to_rgba(&tree, size, size)?;
                let png = rgba_to_png(rgba, size, size)?;
                png_buffers.push((png, size));
            }

            let ico_bytes = build_ico(&png_buffers)?;

            // Ensure parent directory of output exists
            if let Some(parent) = options.output_path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }

            fs::write(&options.output_path, &ico_bytes)?;

            output_files.push(OutputFileMetadata {
                path: options.output_path.to_string_lossy().to_string(),
                size_bytes: ico_bytes.len() as u64,
                description: "Multi-resolution Windows favicon (16x16, 32x32, 48x48)".to_string(),
            });
        }

        Ok(ConversionResult {
            from: Format::Svg,
            to: Format::Favicon,
            output_files,
        })
    }
}

/// Renders SVG to straight RGBA pixels at a specific size, preserving aspect ratio.
fn render_to_rgba(tree: &usvg::Tree, width: u32, height: u32) -> Result<Vec<u8>, MartiniError> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        MartiniError::Rendering(format!(
            "Failed to create tiny-skia Pixmap of size {}x{}",
            width, height
        ))
    })?;

    let svg_w = tree.size().width();
    let svg_h = tree.size().height();

    // Choose scaling factor to fit image in target dimensions
    let scale = (width as f32 / svg_w).min(height as f32 / svg_h);
    let dx = (width as f32 - (svg_w * scale)) / 2.0;
    let dy = (height as f32 - (svg_h * scale)) / 2.0;

    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);

    resvg::render(tree, transform, &mut pixmap.as_mut());

    // Demultiply alpha from tiny-skia's premultiplied RGBA to straight RGBA
    let raw_pixels = demultiply_alpha(pixmap.data());
    Ok(raw_pixels)
}

/// Helper function to demultiply premultiplied alpha channels.
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

/// Converts a straight RGBA raw pixel buffer into PNG bytes.
fn rgba_to_png(rgba: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, MartiniError> {
    let img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba).ok_or_else(|| {
        MartiniError::Rendering("Failed to create ImageBuffer from raw pixels".to_string())
    })?;

    let mut png_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(png_bytes)
}

/// Encodes a list of PNG buffers into a single ICO file.
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
