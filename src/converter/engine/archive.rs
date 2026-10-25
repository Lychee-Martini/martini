use super::FormatEngine;
use crate::converter::{ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

pub struct ArchiveEngine;

impl FormatEngine for ArchiveEngine {
    fn name(&self) -> &'static str {
        "archive"
    }

    fn can_convert(&self, from: Format, to: Format) -> bool {
        from != to
            && matches!(from, Format::Zip | Format::Tar | Format::Gz)
            && matches!(to, Format::Zip | Format::Tar | Format::Gz)
    }

    fn supported_pairs(&self) -> Vec<(Format, Format)> {
        vec![
            (Format::Zip, Format::Tar),
            (Format::Tar, Format::Zip),
            (Format::Zip, Format::Gz),
            (Format::Gz, Format::Zip),
            (Format::Tar, Format::Gz),
            (Format::Gz, Format::Tar),
        ]
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        _options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError> {
        let from = detect_archive_format(input);
        let to = detect_archive_format(output);
        self.convert_with_formats(input, output, from, to)
    }
}

impl ArchiveEngine {
    pub fn convert_with_formats(
        &self,
        input: &Path,
        output: &Path,
        from: Format,
        to: Format,
    ) -> Result<OutputFileMetadata, MartiniError> {
        let temp_dir = tempdir().map_err(MartiniError::Io)?;
        extract_archive(input, temp_dir.path(), from)?;

        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(MartiniError::Io)?;
        }

        create_archive(temp_dir.path(), output, to)?;

        let metadata = fs::metadata(output).map_err(MartiniError::Io)?;

        Ok(OutputFileMetadata {
            path: output.to_string_lossy().to_string(),
            size_bytes: metadata.len(),
            description: format!("Converted from {} to {} using ArchiveEngine", from, to),
        })
    }
}

pub fn detect_archive_format(path: &Path) -> Format {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") || name.ends_with(".gz") {
        Format::Gz
    } else if name.ends_with(".tar") {
        Format::Tar
    } else {
        Format::Zip
    }
}

fn extract_archive(
    archive_path: &Path,
    dest_dir: &Path,
    format: Format,
) -> Result<(), MartiniError> {
    let file = fs::File::open(archive_path).map_err(MartiniError::Io)?;

    match format {
        Format::Zip => {
            let mut zip =
                zip::ZipArchive::new(file).map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to read ZIP archive: {}", e),
                })?;

            for i in 0..zip.len() {
                let mut entry = zip
                    .by_index(i)
                    .map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Failed to read ZIP entry: {}", e),
                    })?;

                let enclosed = match entry.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue, // Skip unsafe path traversal entries
                };

                let out_path = dest_dir.join(enclosed);
                if entry.is_dir() {
                    fs::create_dir_all(&out_path).map_err(MartiniError::Io)?;
                } else {
                    if let Some(p) = out_path.parent() {
                        fs::create_dir_all(p).map_err(MartiniError::Io)?;
                    }
                    let mut outfile = fs::File::create(&out_path).map_err(MartiniError::Io)?;
                    io::copy(&mut entry, &mut outfile).map_err(MartiniError::Io)?;
                }
            }
        }
        Format::Tar => {
            let mut tar = tar::Archive::new(file);
            tar.unpack(dest_dir)
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to unpack TAR archive: {}", e),
                })?;
        }
        Format::Gz => {
            let gz = GzDecoder::new(file);
            let mut tar = tar::Archive::new(gz);
            tar.unpack(dest_dir)
                .map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Failed to unpack TAR.GZ archive: {}", e),
                })?;
        }
        _ => {
            return Err(MartiniError::UnsupportedConversion {
                from: format.to_string(),
                to: "extracted archive".to_string(),
            });
        }
    }

    Ok(())
}

fn create_archive(src_dir: &Path, out_file: &Path, format: Format) -> Result<(), MartiniError> {
    match format {
        Format::Zip => {
            let file = fs::File::create(out_file).map_err(MartiniError::Io)?;
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);

            let mut files = Vec::new();
            collect_files(src_dir, src_dir, &mut files)?;

            for (rel_path, abs_path) in files {
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                zip.start_file(rel_str, options)
                    .map_err(|e| MartiniError::OutputWrite {
                        reason: format!("Failed to write ZIP entry header: {}", e),
                    })?;
                let mut f = fs::File::open(&abs_path).map_err(MartiniError::Io)?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).map_err(MartiniError::Io)?;
                zip.write_all(&buf).map_err(MartiniError::Io)?;
            }

            zip.finish().map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to finalize ZIP: {}", e),
            })?;
        }
        Format::Tar => {
            let file = fs::File::create(out_file).map_err(MartiniError::Io)?;
            let mut tar = tar::Builder::new(file);
            tar.append_dir_all(".", src_dir)
                .map_err(|e| MartiniError::OutputWrite {
                    reason: format!("Failed to write TAR archive: {}", e),
                })?;
            tar.finish().map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to finalize TAR: {}", e),
            })?;
        }
        Format::Gz => {
            let file = fs::File::create(out_file).map_err(MartiniError::Io)?;
            let gz = GzEncoder::new(file, Compression::default());
            let mut tar = tar::Builder::new(gz);
            tar.append_dir_all(".", src_dir)
                .map_err(|e| MartiniError::OutputWrite {
                    reason: format!("Failed to write TAR.GZ archive: {}", e),
                })?;
            tar.finish().map_err(|e| MartiniError::OutputWrite {
                reason: format!("Failed to finalize TAR.GZ: {}", e),
            })?;
        }
        _ => {
            return Err(MartiniError::UnsupportedConversion {
                from: "directory".to_string(),
                to: format.to_string(),
            });
        }
    }

    Ok(())
}

fn collect_files(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), MartiniError> {
    if let Ok(entries) = fs::read_dir(current_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(base_dir, &path, files)?;
            } else if path.is_file() {
                let rel = path.strip_prefix(base_dir).unwrap_or(&path);
                files.push((rel.to_path_buf(), path));
            }
        }
    }
    Ok(())
}
