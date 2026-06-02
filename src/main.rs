use clap::Parser;
use std::process;
use std::str::FromStr;
use tracing::{Level, debug, info};
use tracing_subscriber::FmtSubscriber;

use martini::cli::{CliArgs, Commands};
use martini::converter::{self, ConvertOptions, Format};
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
            let formats = vec![serde_json::json!({
                "from": "svg",
                "to": "favicon",
                "description": "Convert an SVG vector image to a Chrome favicon (.ico or full favicon package)",
                "parameters": {
                    "package": "boolean (generates a package of optimized PNGs, manifest, and HTML copy-paste snippets alongside the .ico file)"
                }
            })];

            if args.json {
                println!("{}", serde_json::to_string_pretty(&formats).unwrap());
            } else {
                println!("Supported Conversions:");
                println!(
                    "- svg -> favicon: Convert SVG to Chrome favicon (single .ico or package). Options: --package"
                );
            }
            Ok(0)
        }
        Commands::Convert {
            from,
            to,
            input,
            output,
            package,
        } => {
            let from_fmt =
                Format::from_str(&from).map_err(|_| MartiniError::UnsupportedConversion {
                    from: from.clone(),
                    to: to.clone(),
                })?;
            let to_fmt =
                Format::from_str(&to).map_err(|_| MartiniError::UnsupportedConversion {
                    from: from.clone(),
                    to: to.clone(),
                })?;

            debug!("Reading input file: {:?}", input);
            if !input.exists() {
                return Err(MartiniError::InputFileNotFound {
                    path: input.to_string_lossy().to_string(),
                });
            }

            let input_data = std::fs::read(&input)?;
            if input_data.is_empty() {
                return Err(MartiniError::InvalidInputData {
                    reason: "Input file is empty".to_string(),
                });
            }

            let options = ConvertOptions {
                input_path: input,
                output_path: output,
                package,
            };

            info!("Starting conversion from {} to {}...", from_fmt, to_fmt);
            let result = converter::convert(from_fmt, to_fmt, &input_data, &options)?;
            info!("Conversion completed successfully.");

            if args.json {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            } else {
                println!("\n✨ Conversion successful!");
                println!("----------------------------------");
                for file in &result.output_files {
                    println!("📄 Path:        {}", file.path);
                    println!("   Size:        {} bytes", file.size_bytes);
                    println!("   Description: {}", file.description);
                    println!();
                }

                if package {
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
