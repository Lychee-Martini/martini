use super::FormatEngine;
use crate::converter::image_conv::convert_image;
use crate::converter::{ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use std::fs;
use std::path::Path;
use std::str::FromStr;

pub struct ImageEngine;

impl FormatEngine for ImageEngine {
    fn name(&self) -> &'static str {
        "image"
    }

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

    fn supported_pairs(&self) -> Vec<(Format, Format)> {
        let sources = [
            Format::Svg,
            Format::Png,
            Format::Jpg,
            Format::Webp,
            Format::Avif,
            Format::Bmp,
            Format::Tiff,
            Format::Qoi,
        ];
        let targets = [
            Format::Png,
            Format::Jpg,
            Format::Webp,
            Format::Avif,
            Format::Favicon,
            Format::Bmp,
            Format::Tiff,
            Format::Qoi,
        ];

        let mut pairs = Vec::new();
        for &from in &sources {
            for &to in &targets {
                if from != to {
                    pairs.push((from, to));
                }
            }
        }
        pairs
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
