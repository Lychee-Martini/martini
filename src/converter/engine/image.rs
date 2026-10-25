use super::FormatEngine;
use crate::converter::image_conv::convert_image;
use crate::converter::{ConvertOptions, Format, FormatCapability, OutputFileMetadata};
use crate::error::MartiniError;
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub struct ImageEngine;

impl FormatEngine for ImageEngine {
    fn can_convert(&self, from: Format, to: Format) -> bool {
        from != to
            && matches!(
                from,
                Format::Svg
                    | Format::Png
                    | Format::Jpg
                    | Format::Webp
                    | Format::Avif
                    | Format::Bmp
                    | Format::Tiff
                    | Format::Qoi
            )
            && matches!(
                to,
                Format::Png
                    | Format::Jpg
                    | Format::Webp
                    | Format::Avif
                    | Format::Favicon
                    | Format::Bmp
                    | Format::Tiff
                    | Format::Qoi
            )
    }

    fn capabilities(&self) -> Vec<FormatCapability> {
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
        ]
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError> {
        let from = detect_image_format(input)?;
        let to = detect_image_format(output)?;

        let input_bytes = fs::read(input).map_err(MartiniError::Io)?;
        let mut opts = options.clone();
        opts.input_path = input.to_path_buf();
        opts.output_path = output.to_path_buf();

        let res = convert_image(from, to, &input_bytes, &opts)?;

        if let Some(first) = res.output_files.into_iter().next() {
            Ok(first)
        } else {
            let meta = fs::metadata(output).map_err(MartiniError::Io)?;
            Ok(OutputFileMetadata {
                path: output.to_string_lossy().to_string(),
                size_bytes: meta.len(),
                description: format!("Converted from {} to {}", from, to),
            })
        }
    }
}

fn detect_image_format(path: &Path) -> Result<Format, MartiniError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    Format::from_str(&ext).map_err(|_| MartiniError::InvalidInputData {
        reason: format!("Unsupported image format extension: '{}'", ext),
    })
}
