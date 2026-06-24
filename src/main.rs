use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::process;
use std::str::FromStr;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use martini::cli::{CliArgs, Commands};
use martini::converter::{self, ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use martini::error::MartiniError;
use martini::{BatchConvertOptions, ProgressTracker, TaskResult, batch_convert};
use std::sync::Arc;

struct CliProgressTracker {
    pb: ProgressBar,
}

impl ProgressTracker for CliProgressTracker {
    fn set_message(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    fn inc(&self, delta: u64) {
        self.pb.inc(delta);
    }
}

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
            if !input.exists() {
                return Err(MartiniError::InputFileNotFound {
                    path: input.to_string_lossy().to_string(),
                });
            }

            // Determine if we're doing a directory conversion
            let is_dir = input.is_dir();

            // Set up rayon thread pool if custom workers requested
            if let Some(w) = workers {
                let _ = rayon::ThreadPoolBuilder::new()
                    .num_threads(w)
                    .build_global();
            }

            // Resolve output formats (targets)
            let is_svg = input
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase() == "svg")
                .unwrap_or(false);
            let is_pdf = input
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase() == "pdf")
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
                            } else {
                                "webp".to_string()
                            }
                        }
                    } else {
                        if is_svg {
                            "favicon".to_string()
                        } else if is_pdf {
                            "png".to_string()
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

                let files = martini::converter::batch::get_all_images(&input, recursive, &from);
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
                    input_dir: input.clone(),
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
                    let file_ext = input
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

                let input_data = std::fs::read(&input)?;
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
                                let filename = input.file_name().ok_or_else(|| {
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
                        None => input.with_extension(target_fmt.to_string()),
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
                        input_path: input.clone(),
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
                    std::fs::remove_file(&input)?;
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

// Local TaskResult and get_all_images removed (moved to library)

fn format_size(size_bytes: u64) -> String {
    if size_bytes == 0 {
        return "0 B".to_string();
    }
    let size_name = ["B", "KB", "MB", "GB", "TB"];
    let i = (size_bytes as f64).log(1024.0).floor() as usize;
    if i >= size_name.len() {
        return format!("{} TB", (size_bytes as f64 / 1024.0f64.powi(4)));
    }
    let p = 1024.0f64.powi(i as i32);
    let s = (size_bytes as f64 / p * 100.0).round() / 100.0;
    format!("{} {}", s, size_name[i])
}

fn print_setup_panel(
    input: &Path,
    to: &str,
    quality: u8,
    lossless: bool,
    recursive: bool,
    delete_original: bool,
    overwrite: bool,
) {
    println!("┌────────────────────────────────────────────────────────┐");
    println!("│                Image Converter Setup                   │");
    println!("├────────────────────────────────────────────────────────┤");
    println!(
        "│ Target:            {:35} │",
        truncate_str(&input.to_string_lossy(), 35)
    );
    println!("│ Formats:           {:35} │", to);
    println!(
        "│ Quality:           {:35} │",
        format!("{} (lossless={})", quality, lossless)
    );
    println!("│ Recursive:         {:35} │", recursive.to_string());
    println!("│ Delete Original:   {:35} │", delete_original.to_string());
    println!("│ Overwrite Existing:{:35} │", overwrite.to_string());
    println!("└────────────────────────────────────────────────────────┘\n");
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("...{}", &s[s.len() - max_len + 3..])
    }
}

fn print_report_table(
    results: &[TaskResult],
    delete_original: bool,
    deleted_count: usize,
    del_errors: &[(String, String)],
) {
    let total_tasks = results.len();
    let successful = results.iter().filter(|r| r.status == "success").count();
    let skipped = results.iter().filter(|r| r.status == "skipped").count();
    let failed = results.iter().filter(|r| r.status == "failed").count();

    let mut orig_size = 0;
    let mut new_size = 0;
    for r in results {
        if r.status == "success" || r.status == "skipped" {
            orig_size += r.original_size;
            new_size += r.converted_size;
        }
    }

    let saved_bytes = orig_size as i64 - new_size as i64;
    let savings_str = if orig_size == 0 {
        "0 B (0% saved)".to_string()
    } else if saved_bytes < 0 {
        format!(
            "-{} (size increased)",
            format_size(saved_bytes.unsigned_abs())
        )
    } else {
        let pct = (saved_bytes as f64 / orig_size as f64) * 100.0;
        format!("{} ({:.1}% saved)", format_size(saved_bytes as u64), pct)
    };

    println!("\n✨ Conversion complete!\n");
    println!("┌────────────────────────────────────────┐");
    println!("│           Conversion Summary           │");
    println!("├────────────────────────────────────────┤");
    println!("│ Total Tasks:       {:20} │", total_tasks);
    println!("│ Successful:        \x1b[32m{:20}\x1b[0m │", successful);
    println!("│ Skipped:           \x1b[33m{:20}\x1b[0m │", skipped);
    println!("│ Failed:            \x1b[31m{:20}\x1b[0m │", failed);
    if delete_original {
        println!("│ Originals Deleted: \x1b[31m{:20}\x1b[0m │", deleted_count);
        if !del_errors.is_empty() {
            println!(
                "│ Deletion Failures: \x1b[31m{:20}\x1b[0m │",
                del_errors.len()
            );
        }
    }
    println!("│ Original Size:     {:20} │", format_size(orig_size));
    println!("│ New Size:          {:20} │", format_size(new_size));
    println!("│ Space Savings:     {:20} │", savings_str);
    println!("└────────────────────────────────────────┘");

    // Print error details if any
    let failures: Vec<&TaskResult> = results.iter().filter(|r| r.status == "failed").collect();
    if !failures.is_empty() {
        println!("\n❌ Failed Conversions Details:");
        println!("--------------------------------------------------");
        for fail in failures {
            println!("📄 File: {}", fail.input_path);
            println!(
                "   Error: {}",
                fail.error_message.as_deref().unwrap_or("Unknown error")
            );
            println!();
        }
    }

    if !del_errors.is_empty() {
        println!("\n❌ Failed Deletions Details:");
        println!("--------------------------------------------------");
        for (file, err_msg) in del_errors {
            println!("📄 File: {}", file);
            println!("   Error: {}", err_msg);
            println!();
        }
    }
}
