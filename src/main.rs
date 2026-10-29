use clap::{CommandFactory, Parser};
use indicatif::{ProgressBar, ProgressStyle};
use std::process;
use std::str::FromStr;
use tracing::{Level, info};
use tracing_subscriber::FmtSubscriber;

use martini::cli::ui::{print_report_table, print_setup_panel};
use martini::cli::{CliArgs, Commands};
use martini::converter::{self, ConversionResult, ConvertOptions, Format, OutputFileMetadata};
use martini::error::MartiniError;
use martini::{BatchConvertOptions, batch_convert};

fn normalize_args<I, T>(raw_args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let args: Vec<std::ffi::OsString> = raw_args.into_iter().map(Into::into).collect();
    if args.len() <= 1 {
        return args;
    }
    let has_subcommand_or_root_help = args.iter().skip(1).any(|arg| {
        arg.to_str().is_some_and(|s| {
            matches!(
                s,
                "convert"
                    | "list-formats"
                    | "preset"
                    | "watch"
                    | "erase"
                    | "help"
                    | "--help"
                    | "-h"
                    | "--version"
                    | "-V"
            )
        })
    });

    if !has_subcommand_or_root_help {
        let mut modified = args;
        modified.insert(1, std::ffi::OsString::from("convert"));
        modified
    } else {
        args
    }
}

fn main() {
    let raw_args: Vec<_> = std::env::args_os().collect();
    if raw_args.len() <= 1 {
        let mut cmd = CliArgs::command();
        let _ = cmd.print_help();
        println!();
        process::exit(0);
    }

    let args = CliArgs::parse_from(normalize_args(raw_args));
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

fn run(mut args: CliArgs) -> Result<i32, MartiniError> {
    if let Some(p_name) = args.command.preset_name() {
        martini::Preset::load(p_name)?.apply_to(&mut args.command);
    }

    match args.command {
        Commands::ListFormats => {
            let formats = martini::all_capabilities();
            if args.json {
                println!("{}", serde_json::to_string_pretty(&formats).unwrap());
            } else {
                println!("Supported Conversions:");
                for f in formats {
                    let display_from = if f.from.starts_with("svg, png") {
                        "[any]".to_string()
                    } else if f.from == "json, yaml, toml, csv, tsv" {
                        "[json/yaml/toml/csv/tsv]".to_string()
                    } else if f.from == "zip, tar, gz, tar.gz" {
                        "[zip/tar/gz/tar.gz]".to_string()
                    } else {
                        f.from.clone()
                    };
                    let display_to = if f.to == "png, jpg, jpeg, webp, avif" {
                        "[png/jpg/webp/avif]".to_string()
                    } else if f.to == "json, yaml, toml, csv, tsv" {
                        "[json/yaml/toml/csv/tsv]".to_string()
                    } else if f.to == "zip, tar, gz, tar.gz" {
                        "[zip/tar/gz/tar.gz]".to_string()
                    } else {
                        f.to.clone()
                    };

                    let options_str = match f.to.as_str() {
                        "favicon" => ". Options: --package",
                        "png" | "bmp" | "tiff" | "qoi" => "",
                        "jpg" => ". Options: --quality",
                        "webp" | "avif" | "both" => ". Options: --quality, --lossless",
                        "docx" => ". Options: --overwrite",
                        "pptx" => ". Options: --pages, --overwrite",
                        "pdf" if f.from == "md" => {
                            ". Options: --theme, --paper-size, --margin, --font-family, --header, --footer, --no-page-numbers, --toc, --overwrite"
                        }
                        _ if f.from == "pdf" => ". Options: --pages, --dpi, --quality, --lossless",
                        _ if f.parameters.contains_key("overwrite") => ". Options: --overwrite",
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
        Commands::Preset { action } => {
            let manager = martini::preset::PresetManager::load();
            match action {
                martini::cli::commands::PresetAction::List => {
                    let presets = manager.list();
                    if args.json {
                        println!("{}", serde_json::to_string_pretty(&presets).unwrap());
                    } else {
                        println!("Available Presets:");
                        if presets.is_empty() {
                            println!(
                                "  (No presets found. Create martini-presets.toml to add presets)"
                            );
                        } else {
                            for (name, preset) in presets {
                                println!(
                                    "- {}: to={}, quality={:?}, lossless={:?}",
                                    name, preset.to, preset.quality, preset.lossless
                                );
                            }
                        }
                    }
                }
                martini::cli::commands::PresetAction::Show { name } => {
                    let preset = martini::Preset::load(&name)?;
                    if args.json {
                        println!("{}", serde_json::to_string_pretty(&preset).unwrap());
                    } else {
                        println!("Preset '{}':\n{:#?}", name, preset);
                    }
                }
            }
            Ok(0)
        }
        Commands::Convert {
            from,
            to,
            input,
            input_flag,
            output,
            output_flag,
            package,
            quality,
            lossless,
            recursive,
            delete_original,
            overwrite,
            workers,
            pages,
            dpi,
            width,
            height,
            no_upscale,
            theme,
            paper_size,
            margin,
            font_family,
            header,
            footer,
            no_page_numbers,
            toc,
            preset: _,
        } => {
            let input = input
                .or(input_flag)
                .expect("Input must be provided either as positional argument or via --input");
            let output = output.or(output_flag);
            let input_str = input.to_string_lossy();
            let is_glob = is_glob_pattern(&input);

            let (glob_files, is_dir, input_resolved) = if is_glob {
                let pattern_normalized = input_str.replace('\\', "/");
                let mut matches = Vec::new();
                for entry in
                    glob::glob(&pattern_normalized).map_err(|e| MartiniError::InvalidInputData {
                        reason: format!("Invalid glob pattern: {}", e),
                    })?
                {
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
                    if output.as_ref().is_some_and(|out_path| {
                        out_path.is_file() || (!out_path.exists() && out_path.extension().is_some())
                    }) {
                        return Err(MartiniError::InvalidInputData {
                            reason:
                                "Output path must be a directory when converting multiple files"
                                    .to_string(),
                        });
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
            let input_name_lower = input_resolved
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            let input_ext = input_resolved
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let to_resolved = match to {
                Some(ref t) => t.clone(),
                None => {
                    if let Some(ref out_path) = output {
                        let out_name = out_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_lowercase();
                        if out_path.is_dir() {
                            default_target_for_ext(&input_ext, &input_name_lower).to_string()
                        } else if out_name.ends_with(".tar.gz") || out_name.ends_with(".tgz") {
                            "tar.gz".to_string()
                        } else if let Some(ext) = out_path.extension().and_then(|e| e.to_str()) {
                            let ext_lower = ext.to_lowercase();
                            if ext_lower == "ico" || ext_lower == "html" {
                                "favicon".to_string()
                            } else {
                                ext_lower
                            }
                        } else {
                            default_target_for_ext(&input_ext, &input_name_lower).to_string()
                        }
                    } else {
                        default_target_for_ext(&input_ext, &input_name_lower).to_string()
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

            let base_options = ConvertOptions {
                input_path: input_resolved.clone(),
                output_path: std::path::PathBuf::new(),
                package,
                quality,
                lossless,
                overwrite,
                pages,
                dpi,
                width,
                height,
                no_upscale,
                theme,
                paper_size,
                margin,
                font_family,
                header,
                footer,
                no_page_numbers,
                toc,
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
                    None => {
                        martini::converter::batch::get_all_images(&input_resolved, recursive, &from)
                    }
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

                let batch_options = BatchConvertOptions {
                    input_dir: input_resolved.clone(),
                    output_dir: output.clone(),
                    from_filter: from.clone(),
                    targets: targets.clone(),
                    recursive,
                    delete_original,
                    workers,
                    files: glob_files,
                    base: base_options,
                };

                let batch_result = batch_convert(batch_options, Some(&pb))?;

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
                    let file_name = input_resolved
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let file_ext = if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz")
                    {
                        "tar.gz".to_string()
                    } else {
                        input_resolved
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase()
                    };
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

                let filename =
                    input_resolved
                        .file_name()
                        .ok_or_else(|| MartiniError::InvalidInputData {
                            reason: "Input path has no filename".to_string(),
                        })?;
                let filename_str = filename.to_string_lossy();
                let base_stem = if filename_str.to_lowercase().ends_with(".tar.gz") {
                    &filename_str[..filename_str.len() - 7]
                } else if let Some(dot_idx) = filename_str.rfind('.') {
                    &filename_str[..dot_idx]
                } else {
                    &filename_str
                };
                let format_output_name = |fmt: &Format| -> String {
                    let ext = if *fmt == Format::Gz {
                        "tar.gz"
                    } else {
                        &fmt.to_string()
                    };
                    format!("{}.{}", base_stem, ext)
                };

                for target_fmt in &targets {
                    let out_path = match &output {
                        Some(out) if out.is_dir() => out.join(format_output_name(target_fmt)),
                        Some(out) if targets.len() > 1 => {
                            out.with_extension(target_fmt.to_string())
                        }
                        Some(out) => out.clone(),
                        None => input_resolved.with_file_name(format_output_name(target_fmt)),
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
                        output_path: out_path.clone(),
                        ..base_options.clone()
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
        Commands::Watch {
            input,
            output,
            preset: _,
            to,
            settle_ms,
            recursive,
            initial,
            delete_original,
            move_originals,
            overwrite,
        } => {
            let format_override = if let Some(ref s) = to {
                Some(
                    Format::from_str(s)
                        .map_err(|e| MartiniError::InvalidInputData { reason: e })?,
                )
            } else {
                None
            };

            let original_action = if let Some(dest) = move_originals {
                martini::watcher::OriginalAction::MoveTo(dest)
            } else if delete_original {
                martini::watcher::OriginalAction::Delete
            } else {
                martini::watcher::OriginalAction::Keep
            };

            let rule = martini::watcher::WatchRule {
                input_dir: input,
                output_dir: output,
                recursive,
                settle_duration: std::time::Duration::from_millis(settle_ms),
                original_action,
                overwrite,
                format_override,
                initial_scan: initial,
            };

            let watcher = martini::watcher::DirectoryWatcher::new(rule);

            let (_tx, rx) = std::sync::mpsc::channel();

            if !args.quiet && !args.json {
                println!(
                    "👀 Martini watching inbox: {} -> {}",
                    watcher.rule.input_dir.display(),
                    watcher.rule.output_dir.display()
                );
                println!("Press Ctrl+C to stop.");
            }

            watcher.run(rx)?;
            Ok(0)
        }
        Commands::Erase(erase_args) => {
            martini::erase::run_erase(&erase_args)?;
            Ok(0)
        }
    }
}

fn default_target_for_ext(ext: &str, file_name: &str) -> &'static str {
    if ext == "svg" {
        "favicon"
    } else if ext == "pdf" {
        "png"
    } else if ext == "md" || ext == "markdown" {
        "pdf"
    } else if matches!(ext, "json" | "csv" | "tsv") {
        "yaml"
    } else if matches!(ext, "yaml" | "yml" | "toml") {
        "json"
    } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") || ext == "tar" {
        "zip"
    } else if ext == "zip" {
        "tar.gz"
    } else {
        "webp"
    }
}

fn is_glob_pattern(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s.contains('*') || s.contains('?') || (s.contains('[') && s.contains(']'))
}

fn get_glob_base(pattern: &str) -> std::path::PathBuf {
    let base_str = pattern
        .split_once(['*', '?', '['])
        .map_or(pattern, |(b, _)| b);
    let base_path = std::path::PathBuf::from(base_str);
    if base_path.is_dir() {
        base_path
    } else if let Some(parent) = base_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        parent.to_path_buf()
    } else {
        std::path::PathBuf::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_args_default_subcommand() {
        let args = vec!["martini", "test.pdf"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini", "convert", "test.pdf"]);
    }

    #[test]
    fn test_normalize_args_already_convert() {
        let args = vec!["martini", "convert", "test.pdf"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini", "convert", "test.pdf"]);
    }

    #[test]
    fn test_normalize_args_list_formats() {
        let args = vec!["martini", "list-formats"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini", "list-formats"]);
    }

    #[test]
    fn test_normalize_args_erase() {
        let args = vec!["martini", "erase", "test.png", "--target", "dog"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(
            strs,
            vec!["martini", "erase", "test.png", "--target", "dog"]
        );
    }

    #[test]
    fn test_normalize_args_help_flags() {
        let args = vec!["martini", "--help"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini", "--help"]);

        let args_h = vec!["martini", "-h"];
        let normalized_h = normalize_args(args_h);
        let strs_h: Vec<_> = normalized_h.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs_h, vec!["martini", "-h"]);
    }

    #[test]
    fn test_normalize_args_version_flags() {
        let args = vec!["martini", "--version"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini", "--version"]);
    }

    #[test]
    fn test_normalize_args_empty_or_single() {
        let args = vec!["martini"];
        let normalized = normalize_args(args);
        let strs: Vec<_> = normalized.iter().map(|s| s.to_str().unwrap()).collect();
        assert_eq!(strs, vec!["martini"]);
    }

    #[test]
    fn test_default_target_for_ext() {
        assert_eq!(default_target_for_ext("svg", "logo.svg"), "favicon");
        assert_eq!(default_target_for_ext("pdf", "doc.pdf"), "png");
        assert_eq!(default_target_for_ext("md", "readme.md"), "pdf");
        assert_eq!(default_target_for_ext("json", "data.json"), "yaml");
        assert_eq!(default_target_for_ext("yaml", "data.yaml"), "json");
        assert_eq!(default_target_for_ext("gz", "archive.tar.gz"), "zip");
        assert_eq!(default_target_for_ext("zip", "archive.zip"), "tar.gz");
        assert_eq!(default_target_for_ext("png", "image.png"), "webp");
    }
}
