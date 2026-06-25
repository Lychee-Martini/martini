use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::process;
use std::str::FromStr;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use martini::cli::ui::{CliProgressTracker, print_report_table, print_setup_panel};
use martini::cli::{CliArgs, Commands};
use martini::converter::{self, ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use martini::error::MartiniError;
use martini::{BatchConvertOptions, batch_convert};
use std::sync::Arc;

fn main() {
    let args = CliArgs::parse();
    let is_json = args.json;

    // 1. Initialize logging
    let log_level = if args.quiet {
        None
    } else if args.verbose {
        Some(Level::DEBUG)
    } else {
        Some(Level::INFO)
    };

    if let Some(level) = log_level {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(level)
            .with_writer(std::io::stderr) // Write logs to stderr so stdout remains clean for JSON data
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");
    }

    // 2. Run application
    match run(args) {
        Ok(code) => process::exit(code),
        Err(err) => {
            let code = err.exit_code();
            if is_json {
                let err_json = serde_json::json!({
                    "error": err.to_string(),
                    "exit_code": code,
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap());
            } else {
                eprintln!("❌ Error: {}", err);
            }
            process::exit(code);
        }
    }
}

fn run(args: CliArgs) -> Result<i32, MartiniError> {
    match args.command {
        Commands::ListFormats => {
            let formats = martini::get_supported_formats();
            if args.json {
                println!("{}", serde_json::to_string_pretty(&formats).unwrap());
            } else {
                println!("Supported Conversions:");
                for f in formats {
                    let display_from = if f.from == "svg, png, jpg, jpeg, webp, avif" {
                        "[any]".to_string()
                    } else {
                        f.from.clone()
                    };
                    let display_to = if f.to == "png, jpg, jpeg, webp, avif" {
                        "[png/jpg/webp/avif]".to_string()
                    } else {
                        f.to.clone()
                    };

                    let options_str = match f.to.as_str() {
                        "favicon" => ". Options: --package",
                        "png" => "",
                        "jpg" => ". Options: --quality",
                        "webp" | "avif" | "both" => ". Options: --quality, --lossless",
                        "docx" => ". Options: --overwrite",
                        _ if f.from == "pdf" => ". Options: --pages, --dpi, --quality, --lossless",
                        _ => "",
                    };

                    println!(
                        "- {} -> {}: {}{}",
                        display_from, display_to, f.description, options_str
                    );
                }
            }
            Ok(0)
        }
        Commands::Convert {
            from,
            to,
            input,
            output,
            package,
            quality,
            lossless,
            recursive,
            delete_original,
            overwrite,
            workers,
            pages,
            dpi,
        } => {
            let input_str = input.to_string_lossy();
            let is_glob = is_glob_pattern(&input);

            let (glob_files, is_dir, input_resolved) = if is_glob {
                let pattern_normalized = input_str.replace('\\', "/");
                let mut matches = Vec::new();
                for entry in glob::glob(&pattern_normalized).map_err(|e| MartiniError::InvalidInputData {
                    reason: format!("Invalid glob pattern: {}", e),
                })? {
                    let path = entry.map_err(|e| MartiniError::Io(e.into_error()))?;
                    if path.is_file() {
                        matches.push(path);
                    }
                }

                if matches.is_empty() {
                    return Err(MartiniError::InputFileNotFound {
                        path: input_str.into_owned(),
                    });
                }

                if matches.len() == 1 {
                    (None, false, matches[0].clone())
                } else {
                    if let Some(ref out_path) = output {
                        if out_path.is_file() || (!out_path.exists() && out_path.extension().is_some()) {
                            return Err(MartiniError::InvalidInputData {
                                reason: "Output path must be a directory when converting multiple files".to_string(),
                            });
                        }
                    }

                    let base_dir = get_glob_base(&pattern_normalized);
                    (Some(matches), true, base_dir)
                }
            } else {
                if !input.exists() {
                    return Err(MartiniError::InputFileNotFound {
                        path: input.to_string_lossy().to_string(),
                    });
                }
                let is_dir = input.is_dir();
                (None, is_dir, input.clone())
            };

            // Set up rayon thread pool if custom workers requested
            if let Some(w) = workers {
                let _ = rayon::ThreadPoolBuilder::new()
                    .num_threads(w)
                    .build_global();
            }

            // Resolve output formats (targets)
            let is_svg = input_resolved
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase() == "svg")
                .unwrap_or(false);
            let is_pdf = input_resolved
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase() == "pdf")
                .unwrap_or(false);
            let is_md = input_resolved
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| {
                    let ext = s.to_lowercase();
                    ext == "md" || ext == "markdown"
                })
                .unwrap_or(false);

            let to_resolved = match to {
                Some(ref t) => t.clone(),
                None => {
                    if let Some(ref out_path) = output {
                        if out_path.is_dir() {
                            if is_svg {
                                "favicon".to_string()
                            } else if is_pdf {
                                "png".to_string()
                            } else if is_md {
                                "docx".to_string()
                            } else {
                                "webp".to_string()
                            }
                        } else if let Some(ext) = out_path.extension().and_then(|e| e.to_str()) {
                            let ext_lower = ext.to_lowercase();
                            if ext_lower == "ico" || ext_lower == "html" {
                                "favicon".to_string()
                            } else {
                                ext_lower
                            }
                        } else {
                            if is_svg {
                                "favicon".to_string()
                            } else if is_pdf {
                                "png".to_string()
                            } else if is_md {
                                "docx".to_string()
                            } else {
                                "webp".to_string()
                            }
                        }
                    } else {
                        if is_svg {
                            "favicon".to_string()
                        } else if is_pdf {
                            "png".to_string()
                        } else if is_md {
                            "docx".to_string()
                        } else {
                            "webp".to_string()
                        }
                    }
                }
            };

            // Parse targets
            let targets: Vec<Format> = if to_resolved.to_lowercase() == "both" {
                vec![Format::Webp, Format::Avif]
            } else {
                vec![Format::from_str(&to_resolved).map_err(|_| {
                    MartiniError::UnsupportedConversion {
                        from: from.clone(),
                        to: to_resolved.clone(),
                    }
                })?]
            };

            if is_dir {
                // Batch directory conversion
                if !args.json && !args.quiet {
                    print_setup_panel(
                        &input,
                        &to_resolved,
                        quality,
                        lossless,
                        recursive,
                        delete_original,
                        overwrite,
                    );
                    println!("🔍 Scanning for images...");
                }

                let files = match glob_files {
                    Some(ref f) => f.clone(),
                    None => martini::converter::batch::get_all_images(&input_resolved, recursive, &from),
                };
                if files.is_empty() {
                    if args.json {
                        println!("[]");
                    } else {
                        println!("⚠️ No PNG or JPG/JPEG images found in the target directory.");
                    }
                    return Ok(0);
                }

                if !args.json && !args.quiet {
                    println!("✨ Found {} images to process.\n", files.len());
                }

                let total_tasks = files.len() * targets.len();

                let pb = if !args.json && !args.quiet {
                    let pb = ProgressBar::new(total_tasks as u64);
                    pb.set_style(
                        ProgressStyle::default_bar()
                            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                            .unwrap()
                            .progress_chars("#>-"),
                    );
                    pb
                } else {
                    ProgressBar::hidden()
                };

                let tracker = Arc::new(CliProgressTracker { pb: pb.clone() });

                let batch_options = BatchConvertOptions {
                    input_dir: input_resolved.clone(),
                    output_dir: output.clone(),
                    from_filter: from.clone(),
                    targets: targets.clone(),
                    quality,
                    lossless,
                    recursive,
                    overwrite,
                    delete_original,
                    workers,
                    pages: pages.clone(),
                    dpi,
                    files: glob_files,
                };

                let batch_result = batch_convert(batch_options, Some(tracker))?;

                pb.finish_and_clear();

                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&batch_result.tasks).unwrap()
                    );
                } else {
                    print_report_table(
                        &batch_result.tasks,
                        delete_original,
                        batch_result.deleted_count,
                        &batch_result.deletion_errors,
                    );
                }

                let any_failed = batch_result.tasks.iter().any(|r| r.status == "failed")
                    || !batch_result.deletion_errors.is_empty();
                if any_failed { Ok(1) } else { Ok(0) }
            } else {
                // Single file conversion
                let from_fmt = if from == "auto" {
                    let file_ext = input_resolved
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    Format::from_str(&file_ext).map_err(|_| {
                        MartiniError::UnsupportedConversion {
                            from: file_ext,
                            to: to_resolved.clone(),
                        }
                    })?
                } else {
                    Format::from_str(&from).map_err(|_| MartiniError::UnsupportedConversion {
                        from: from.clone(),
                        to: to_resolved.clone(),
                    })?
                };

                let input_data = std::fs::read(&input_resolved)?;
                if input_data.is_empty() {
                    return Err(MartiniError::InvalidInputData {
                        reason: "Input file is empty".to_string(),
                    });
                }

                let mut output_files = Vec::new();

                for target_fmt in &targets {
                    let out_path = match &output {
                        Some(out) => {
                            if out.is_dir() {
                                let filename = input_resolved.file_name().ok_or_else(|| {
                                    MartiniError::InvalidInputData {
                                        reason: "Input path has no filename".to_string(),
                                    }
                                })?;
                                out.join(filename).with_extension(target_fmt.to_string())
                            } else if targets.len() > 1 {
                                out.with_extension(target_fmt.to_string())
                            } else {
                                out.clone()
                            }
                        }
                        None => input_resolved.with_extension(target_fmt.to_string()),
                    };

                    if out_path.exists() && !overwrite {
                        let size = out_path.metadata().map(|m| m.len()).unwrap_or(0);
                        output_files.push(OutputFileMetadata {
                            path: out_path.to_string_lossy().to_string(),
                            size_bytes: size,
                            description: "Skipped (already exists)".to_string(),
                        });
                        continue;
                    }

                    let options = ConvertOptions {
                        input_path: input_resolved.clone(),
                        output_path: out_path.clone(),
                        package,
                        quality,
                        lossless,
                        overwrite,
                        pages: pages.clone(),
                        dpi,
                    };

                    info!("Starting conversion to {}...", target_fmt);
                    let res = converter::convert(from_fmt, *target_fmt, &input_data, &options)?;
                    output_files.extend(res.output_files);
                }

                // Safely delete original if requested
                if delete_original
                    && output_files
                        .iter()
                        .all(|f| !f.description.contains("failed"))
                {
                    std::fs::remove_file(&input_resolved)?;
                }

                let final_result = ConversionResult {
                    from: from_fmt,
                    to: targets[0], // primary target
                    output_files,
                };

                if args.json {
                    println!("{}", serde_json::to_string_pretty(&final_result).unwrap());
                } else {
                    println!("\n✨ Conversion successful!");
                    println!("----------------------------------");
                    for file in &final_result.output_files {
                        println!("📄 Path:        {}", file.path);
                        println!("   Size:        {} bytes", file.size_bytes);
                        println!("   Description: {}", file.description);
                        println!();
                    }

                    if package && from_fmt == Format::Svg && targets[0] == Format::Favicon {
                        println!("HTML Tags (copy-paste into your index.html head):");
                        println!("--------------------------------------------------");
                        let html_tags = r#"<link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
<link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
<link rel="manifest" href="/site.webmanifest">"#;
                        println!("{}", html_tags);
                        println!("--------------------------------------------------");
                    }
                }

                Ok(0)
            }
        }
    }
}

fn is_glob_pattern(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s.contains('*') || s.contains('?') || (s.contains('[') && s.contains(']'))
}

fn get_glob_base(pattern: &str) -> std::path::PathBuf {
    let mut base_str = String::new();
    for char in pattern.chars() {
        if char == '*' || char == '?' || char == '[' {
            break;
        }
        base_str.push(char);
    }
    let base_path = std::path::PathBuf::from(base_str);
    if base_path.is_dir() {
        base_path
    } else if let Some(parent) = base_path.parent() {
        if parent.as_os_str().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            parent.to_path_buf()
        }
    } else {
        std::path::PathBuf::from(".")
    }
}
