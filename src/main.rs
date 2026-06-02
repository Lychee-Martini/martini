use clap::Parser;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;
use rayon::prelude::*;
use indicatif::{ProgressBar, ProgressStyle};

use martini::cli::{CliArgs, Commands};
use martini::converter::{self, ConvertOptions, Format, OutputFileMetadata, ConversionResult};
use martini::error::MartiniError;

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
            let formats = vec![
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "favicon",
                    "description": "Convert an SVG or raster image to a Chrome favicon (.ico or full favicon package)",
                    "parameters": {
                        "package": "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)"
                    }
                }),
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "png",
                    "description": "Convert images to PNG format",
                    "parameters": {
                        "overwrite": "boolean (default false)",
                        "delete_original": "boolean (default false)",
                        "recursive": "boolean (default false)",
                        "workers": "integer (optional)"
                    }
                }),
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "jpg",
                    "description": "Convert images to JPEG format",
                    "parameters": {
                        "quality": "integer (1-100, default 80)",
                        "overwrite": "boolean (default false)",
                        "delete_original": "boolean (default false)",
                        "recursive": "boolean (default false)",
                        "workers": "integer (optional)"
                    }
                }),
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "webp",
                    "description": "Convert images to WebP format",
                    "parameters": {
                        "quality": "integer (1-100, default 80)",
                        "lossless": "boolean (default false)",
                        "overwrite": "boolean (default false)",
                        "delete_original": "boolean (default false)",
                        "recursive": "boolean (default false)",
                        "workers": "integer (optional)"
                    }
                }),
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "avif",
                    "description": "Convert images to AVIF format",
                    "parameters": {
                        "quality": "integer (1-100, default 80)",
                        "lossless": "boolean (default false)",
                        "overwrite": "boolean (default false)",
                        "delete_original": "boolean (default false)",
                        "recursive": "boolean (default false)",
                        "workers": "integer (optional)"
                    }
                }),
                serde_json::json!({
                    "from": "svg, png, jpg, jpeg, webp, avif",
                    "to": "both",
                    "description": "Convert images to both WebP and AVIF formats",
                    "parameters": {
                        "quality": "integer (1-100, default 80)",
                        "lossless": "boolean (default false)",
                        "overwrite": "boolean (default false)",
                        "delete_original": "boolean (default false)",
                        "recursive": "boolean (default false)",
                        "workers": "integer (optional)"
                    }
                }),
            ];

            if args.json {
                println!("{}", serde_json::to_string_pretty(&formats).unwrap());
            } else {
                println!("Supported Conversions:");
                println!("- [any] -> favicon: Convert image to Chrome favicon (single .ico or package). Options: --package");
                println!("- [any] -> png: Convert image to PNG");
                println!("- [any] -> jpg: Convert image to JPEG. Options: --quality");
                println!("- [any] -> webp: Convert image to WebP. Options: --quality, --lossless");
                println!("- [any] -> avif: Convert image to AVIF. Options: --quality, --lossless");
                println!("- [any] -> both: Convert image to both WebP and AVIF. Options: --quality, --lossless");
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
                let _ = rayon::ThreadPoolBuilder::new().num_threads(w).build_global();
            }

            // Resolve output formats (targets)
            let to_resolved = match to {
                Some(ref t) => t.clone(),
                None => {
                    if let Some(ref out_path) = output {
                        if out_path.is_dir() {
                            let is_svg = input.extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_lowercase() == "svg")
                                .unwrap_or(false);
                            if is_svg { "favicon".to_string() } else { "webp".to_string() }
                        } else if let Some(ext) = out_path.extension().and_then(|e| e.to_str()) {
                            let ext_lower = ext.to_lowercase();
                            if ext_lower == "ico" || ext_lower == "html" {
                                "favicon".to_string()
                            } else {
                                ext_lower
                            }
                        } else {
                            let is_svg = input.extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_lowercase() == "svg")
                                .unwrap_or(false);
                            if is_svg { "favicon".to_string() } else { "webp".to_string() }
                        }
                    } else {
                        let is_svg = input.extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_lowercase() == "svg")
                            .unwrap_or(false);
                        if is_svg { "favicon".to_string() } else { "webp".to_string() }
                    }
                }
            };

            // Parse targets
            let targets: Vec<Format> = if to_resolved.to_lowercase() == "both" {
                vec![Format::Webp, Format::Avif]
            } else {
                vec![Format::from_str(&to_resolved).map_err(|_| MartiniError::UnsupportedConversion {
                    from: from.clone(),
                    to: to_resolved.clone(),
                })?]
            };

            if is_dir {
                // Batch directory conversion
                if !args.json && !args.quiet {
                    print_setup_panel(&input, &to_resolved, quality, lossless, recursive, delete_original, overwrite);
                    println!("🔍 Scanning for images...");
                }

                let files = get_all_images(&input, recursive, &from);
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

                // Create tasks list
                let mut tasks = Vec::new();
                for file_path in &files {
                    for target_fmt in &targets {
                        // Output path resolution
                        let out_path = match &output {
                            Some(out_dir) => {
                                let relative = file_path.strip_prefix(&input).unwrap();
                                out_dir.join(relative).with_extension(target_fmt.to_string())
                            }
                            None => file_path.with_extension(target_fmt.to_string()),
                        };
                        tasks.push((file_path.clone(), target_fmt.clone(), out_path));
                    }
                }

                let pb = if !args.json && !args.quiet {
                    let pb = ProgressBar::new(tasks.len() as u64);
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

                // Track results in parallel
                let results: Vec<TaskResult> = tasks
                    .par_iter()
                    .map(|(img_path, target_fmt, out_path)| {
                        pb.set_message(format!("Converting {}", img_path.file_name().unwrap().to_string_lossy()));

                        let orig_size = img_path.metadata().map(|m| m.len()).unwrap_or(0);

                        // If not overwrite and output exists, skip
                        if out_path.exists() && !overwrite {
                            pb.inc(1);
                            return TaskResult {
                                input_path: img_path.to_string_lossy().to_string(),
                                output_path: Some(out_path.to_string_lossy().to_string()),
                                status: "skipped".to_string(),
                                original_size: orig_size,
                                converted_size: out_path.metadata().map(|m| m.len()).unwrap_or(0),
                                error_message: None,
                            };
                        }

                        // Determine source format by extension
                        let file_ext = img_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                        let file_from_fmt = match Format::from_str(&file_ext) {
                            Ok(fmt) => fmt,
                            Err(_) => {
                                pb.inc(1);
                                return TaskResult {
                                    input_path: img_path.to_string_lossy().to_string(),
                                    output_path: None,
                                    status: "failed".to_string(),
                                    original_size: orig_size,
                                    converted_size: 0,
                                    error_message: Some(format!("Unsupported source file extension: {}", file_ext)),
                                };
                            }
                        };

                        let options = ConvertOptions {
                            input_path: img_path.clone(),
                            output_path: out_path.clone(),
                            package: false,
                            quality,
                            lossless,
                            overwrite,
                        };

                        // Perform conversion
                        let input_data = match std::fs::read(img_path) {
                            Ok(d) => d,
                            Err(e) => {
                                pb.inc(1);
                                return TaskResult {
                                    input_path: img_path.to_string_lossy().to_string(),
                                    output_path: None,
                                    status: "failed".to_string(),
                                    original_size: orig_size,
                                    converted_size: 0,
                                    error_message: Some(format!("Failed to read input file: {}", e)),
                                };
                            }
                        };
                        if let Err(e) = converter::convert(file_from_fmt, *target_fmt, &input_data, &options) {
                            pb.inc(1);
                            return TaskResult {
                                input_path: img_path.to_string_lossy().to_string(),
                                output_path: Some(out_path.to_string_lossy().to_string()),
                                status: "failed".to_string(),
                                original_size: orig_size,
                                converted_size: 0,
                                error_message: Some(e.to_string()),
                            };
                        }

                        let new_size = out_path.metadata().map(|m| m.len()).unwrap_or(0);

                        pb.inc(1);
                        TaskResult {
                            input_path: img_path.to_string_lossy().to_string(),
                            output_path: Some(out_path.to_string_lossy().to_string()),
                            status: "success".to_string(),
                            original_size: orig_size,
                            converted_size: new_size,
                            error_message: None,
                        }
                    })
                    .collect();

                pb.finish_and_clear();

                // Group successes/failures per input file for deletion safety
                let mut conversion_successes = std::collections::HashMap::new();
                let mut conversion_failures = std::collections::HashMap::new();
                for res in &results {
                    if res.status == "success" || res.status == "skipped" {
                        conversion_successes
                            .entry(res.input_path.clone())
                            .or_insert_with(std::collections::HashSet::new)
                            .insert(res.output_path.clone().unwrap_or_default());
                    } else {
                        conversion_failures
                            .entry(res.input_path.clone())
                            .or_insert_with(std::collections::HashSet::new)
                            .insert(res.error_message.clone().unwrap_or_default());
                    }
                }

                // Handle delete original
                let mut deleted_count = 0;
                let mut del_errors = Vec::new();
                if delete_original {
                    for file_path in &files {
                        let path_str = file_path.to_string_lossy().to_string();
                        let successes = conversion_successes.get(&path_str).map(|s| s.len()).unwrap_or(0);
                        let failures = conversion_failures.get(&path_str).map(|f| f.len()).unwrap_or(0);

                        // Did we successfully convert to all requested targets, with no failures?
                        if successes == targets.len() && failures == 0 {
                            if let Err(e) = std::fs::remove_file(file_path) {
                                del_errors.push((path_str, e.to_string()));
                            } else {
                                deleted_count += 1;
                            }
                        }
                    }
                }

                // Reporting
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&results).unwrap());
                } else {
                    print_report_table(
                        &results,
                        delete_original,
                        deleted_count,
                        &del_errors,
                    );
                }

                // If any tasks failed, exit with non-zero code
                let any_failed = results.iter().any(|r| r.status == "failed") || !del_errors.is_empty();
                if any_failed {
                    Ok(1)
                } else {
                    Ok(0)
                }
            } else {
                // Single file conversion
                let from_fmt = if from == "auto" {
                    let file_ext = input.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    Format::from_str(&file_ext).map_err(|_| MartiniError::UnsupportedConversion {
                        from: file_ext,
                        to: to_resolved.clone(),
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
                                out.join(input.file_name().unwrap()).with_extension(target_fmt.to_string())
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
                    };

                    info!("Starting conversion to {}...", target_fmt);
                    let res = converter::convert(from_fmt, *target_fmt, &input_data, &options)?;
                    output_files.extend(res.output_files);
                }

                // Safely delete original if requested
                if delete_original && output_files.iter().all(|f| !f.description.contains("failed")) {
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

#[derive(Debug, Clone, serde::Serialize)]
struct TaskResult {
    input_path: String,
    output_path: Option<String>,
    status: String, // "success", "skipped", "failed"
    original_size: u64,
    converted_size: u64,
    error_message: Option<String>,
}

fn get_all_images(directory: &Path, recursive: bool, from_filter: &str) -> Vec<PathBuf> {
    let mut image_files = Vec::new();
    let extensions: std::collections::HashSet<&str> = match from_filter.to_lowercase().as_str() {
        "png" => ["png"].iter().copied().collect(),
        "jpg" | "jpeg" => ["jpg", "jpeg"].iter().copied().collect(),
        "webp" => ["webp"].iter().copied().collect(),
        "avif" => ["avif"].iter().copied().collect(),
        _ => ["png", "jpg", "jpeg"].iter().copied().collect(), // default/auto
    };

    let mut dirs_to_visit = vec![directory.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') && name != "." {
                        continue;
                    }
                }

                if path.is_dir() {
                    if recursive {
                        dirs_to_visit.push(path);
                    }
                } else if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if extensions.contains(ext.to_lowercase().as_str()) {
                            image_files.push(path);
                        }
                    }
                }
            }
        }
    }

    image_files.sort();
    image_files
}

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
    println!("│ Target:            {:35} │", truncate_str(&input.to_string_lossy(), 35));
    println!("│ Formats:           {:35} │", to);
    println!("│ Quality:           {:35} │", format!("{} (lossless={})", quality, lossless));
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
        format!("-{} (size increased)", format_size(saved_bytes.unsigned_abs()))
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
            println!("│ Deletion Failures: \x1b[31m{:20}\x1b[0m │", del_errors.len());
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
            println!("   Error: {}", fail.error_message.as_deref().unwrap_or("Unknown error"));
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
