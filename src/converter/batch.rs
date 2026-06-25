use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use crate::converter::Format;
use crate::error::MartiniError;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskResult {
    pub input_path: String,
    pub output_path: Option<String>,
    pub status: String, // "success", "skipped", "failed"
    pub original_size: u64,
    pub converted_size: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BatchConvertOptions {
    pub input_dir: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub from_filter: String,
    pub targets: Vec<Format>,
    pub quality: u8,
    pub lossless: bool,
    pub recursive: bool,
    pub overwrite: bool,
    pub delete_original: bool,
    pub workers: Option<usize>,
    pub pages: Option<String>,
    pub dpi: u16,
    pub files: Option<Vec<PathBuf>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchResult {
    pub tasks: Vec<TaskResult>,
    pub deleted_count: usize,
    pub deletion_errors: Vec<(String, String)>,
}

pub trait ProgressTracker: Send + Sync {
    fn set_message(&self, msg: &str);
    fn inc(&self, delta: u64);
}

pub fn get_all_images(directory: &Path, recursive: bool, from_filter: &str) -> Vec<PathBuf> {
    let mut image_files = Vec::new();
    let extensions: HashSet<&str> = match from_filter.to_lowercase().as_str() {
        "png" => ["png"].iter().copied().collect(),
        "jpg" | "jpeg" => ["jpg", "jpeg"].iter().copied().collect(),
        "webp" => ["webp"].iter().copied().collect(),
        "avif" => ["avif"].iter().copied().collect(),
        "svg" => ["svg"].iter().copied().collect(),
        "pdf" => ["pdf"].iter().copied().collect(),
        _ => ["png", "jpg", "jpeg"].iter().copied().collect(), // default/auto
    };

    let mut dirs_to_visit = vec![directory.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                if let Some(name) = path.file_name().and_then(|n| n.to_str())
                    && name.starts_with('.')
                    && name != "."
                {
                    continue;
                }

                if path.is_dir() {
                    if recursive {
                        dirs_to_visit.push(path);
                    }
                } else if path.is_file()
                    && let Some(ext) = path.extension().and_then(|e| e.to_str())
                    && extensions.contains(ext.to_lowercase().as_str())
                {
                    image_files.push(path);
                }
            }
        }
    }

    image_files.sort();
    image_files
}

pub fn batch_convert(
    options: BatchConvertOptions,
    tracker: Option<Arc<dyn ProgressTracker>>,
) -> Result<BatchResult, MartiniError> {
    if !options.input_dir.exists() {
        return Err(MartiniError::InputFileNotFound {
            path: options.input_dir.to_string_lossy().to_string(),
        });
    }

    if !options.input_dir.is_dir() {
        return Err(MartiniError::InvalidInputData {
            reason: "Input path must be a directory for batch conversions".to_string(),
        });
    }

    let canonical_input_dir = std::fs::canonicalize(&options.input_dir)?;

    let files = match &options.files {
        Some(f) => f.clone(),
        None => get_all_images(&options.input_dir, options.recursive, &options.from_filter),
    };
    if files.is_empty() {
        return Ok(BatchResult {
            tasks: Vec::new(),
            deleted_count: 0,
            deletion_errors: Vec::new(),
        });
    }

    // Configure ThreadPool if custom workers requested
    if let Some(w) = options.workers {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(w)
            .build_global();
    }

    let tracker_arc = tracker;

    // Map each input file to its corresponding tasks and execute in parallel
    let tasks: Vec<TaskResult> = files
        .par_iter()
        .flat_map(|file_path| {
            let mut results = Vec::new();

            if let Some(ref t) = tracker_arc {
                let filename = file_path
                    .file_name()
                    .map(|f| f.to_string_lossy())
                    .unwrap_or_default();
                t.set_message(&format!("Converting {}", filename));
            }

            let input_bytes = match std::fs::read(file_path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    for _target_fmt in &options.targets {
                        results.push(TaskResult {
                            input_path: file_path.to_string_lossy().to_string(),
                            output_path: None,
                            status: "failed".to_string(),
                            original_size: 0,
                            converted_size: 0,
                            error_message: Some(format!("Failed to read source file: {}", e)),
                        });
                        if let Some(ref t) = tracker_arc {
                            t.inc(1);
                        }
                    }
                    return results;
                }
            };

            let original_size = input_bytes.len() as u64;

            let file_ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let from_fmt = match Format::from_str(&file_ext) {
                Ok(fmt) => fmt,
                Err(_) => {
                    for _target_fmt in &options.targets {
                        results.push(TaskResult {
                            input_path: file_path.to_string_lossy().to_string(),
                            output_path: None,
                            status: "failed".to_string(),
                            original_size,
                            converted_size: 0,
                            error_message: Some(format!(
                                "Unsupported source format: '{}'",
                                file_ext
                            )),
                        });
                        if let Some(ref t) = tracker_arc {
                            t.inc(1);
                        }
                    }
                    return results;
                }
            };

            for target_fmt in &options.targets {
                let out_path = match &options.output_dir {
                    Some(out_dir) => {
                        let canonical_file = match std::fs::canonicalize(file_path) {
                            Ok(p) => p,
                            Err(e) => {
                                results.push(TaskResult {
                                    input_path: file_path.to_string_lossy().to_string(),
                                    output_path: None,
                                    status: "failed".to_string(),
                                    original_size,
                                    converted_size: 0,
                                    error_message: Some(format!("Failed to canonicalize path: {}", e)),
                                });
                                if let Some(ref t) = tracker_arc {
                                    t.inc(1);
                                }
                                continue;
                            }
                        };

                        let relative = match canonical_file.strip_prefix(&canonical_input_dir) {
                            Ok(rel) => rel,
                            Err(e) => {
                                results.push(TaskResult {
                                    input_path: file_path.to_string_lossy().to_string(),
                                    output_path: None,
                                    status: "failed".to_string(),
                                    original_size,
                                    converted_size: 0,
                                    error_message: Some(format!("Invalid file path prefix: {}", e)),
                                });
                                if let Some(ref t) = tracker_arc {
                                    t.inc(1);
                                }
                                continue;
                            }
                        };
                        out_dir
                            .join(relative)
                            .with_extension(target_fmt.to_string())
                    }
                    None => file_path.with_extension(target_fmt.to_string()),
                };

                if out_path.exists() && !options.overwrite {
                    let converted_size = out_path.metadata().map(|m| m.len()).unwrap_or(0);
                    results.push(TaskResult {
                        input_path: file_path.to_string_lossy().to_string(),
                        output_path: Some(out_path.to_string_lossy().to_string()),
                        status: "skipped".to_string(),
                        original_size,
                        converted_size,
                        error_message: None,
                    });
                    if let Some(ref t) = tracker_arc {
                        t.inc(1);
                    }
                    continue;
                }

                let convert_opts = crate::converter::ConvertOptions {
                    input_path: file_path.clone(),
                    output_path: out_path.clone(),
                    package: false,
                    quality: options.quality,
                    lossless: options.lossless,
                    overwrite: options.overwrite,
                    pages: options.pages.clone(),
                    dpi: options.dpi,
                };

                match crate::converter::convert(from_fmt, *target_fmt, &input_bytes, &convert_opts)
                {
                    Ok(conv_res) => {
                        let mut converted_size = 0;
                        for out_file in &conv_res.output_files {
                            converted_size += out_file.size_bytes;
                        }

                        results.push(TaskResult {
                            input_path: file_path.to_string_lossy().to_string(),
                            output_path: Some(out_path.to_string_lossy().to_string()),
                            status: "success".to_string(),
                            original_size,
                            converted_size,
                            error_message: None,
                        });
                    }
                    Err(e) => {
                        results.push(TaskResult {
                            input_path: file_path.to_string_lossy().to_string(),
                            output_path: Some(out_path.to_string_lossy().to_string()),
                            status: "failed".to_string(),
                            original_size,
                            converted_size: 0,
                            error_message: Some(e.to_string()),
                        });
                    }
                }

                if let Some(ref t) = tracker_arc {
                    t.inc(1);
                }
            }

            results
        })
        .collect();

    // Collect successes and failures for safe deletion check
    let mut conversion_successes = HashMap::new();
    let mut conversion_failures = HashMap::new();
    for res in &tasks {
        if res.status == "success" || res.status == "skipped" {
            conversion_successes
                .entry(res.input_path.clone())
                .or_insert_with(HashSet::new)
                .insert(res.output_path.clone().unwrap_or_default());
        } else {
            conversion_failures
                .entry(res.input_path.clone())
                .or_insert_with(HashSet::new)
                .insert(res.error_message.clone().unwrap_or_default());
        }
    }

    // Handle delete original
    let mut deleted_count = 0;
    let mut deletion_errors = Vec::new();
    if options.delete_original {
        for file_path in &files {
            let path_str = file_path.to_string_lossy().to_string();
            let successes_count = conversion_successes
                .get(&path_str)
                .map(|s| s.len())
                .unwrap_or(0);
            let failures_count = conversion_failures
                .get(&path_str)
                .map(|f| f.len())
                .unwrap_or(0);

            if successes_count == options.targets.len() && failures_count == 0 {
                if let Err(e) = std::fs::remove_file(file_path) {
                    deletion_errors.push((path_str, e.to_string()));
                } else {
                    deleted_count += 1;
                }
            }
        }
    }

    Ok(BatchResult {
        tasks,
        deleted_count,
        deletion_errors,
    })
}
