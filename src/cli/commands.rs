use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "martini")]
#[command(author = "Lychee Martini <info@lycheemartini.com>")]
#[command(version)]
#[command(about = "A modern, extensible CLI format converter optimized for Agent Skills", long_about = None)]
pub struct CliArgs {
    /// Enable verbose logging (debug level)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress all logging output except CLI results
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Format output as structured JSON
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Convert a file or directory from one format to another
    Convert {
        /// Source format (e.g. svg, png, jpg, webp, avif). Defaults to 'auto' for auto-detection.
        #[arg(long, default_value = "auto")]
        from: String,

        /// Target format (e.g. favicon, webp, avif, png, jpg, both) (optional)
        #[arg(long)]
        to: Option<String>,

        /// Path to the input file or directory (positional)
        #[arg(
            value_name = "INPUT",
            conflicts_with = "input_flag",
            required_unless_present = "input_flag"
        )]
        input: Option<PathBuf>,

        /// Path to the input file or directory (flag)
        #[arg(
            short = 'i',
            long = "input",
            value_name = "INPUT",
            conflicts_with = "input",
            required_unless_present = "input"
        )]
        input_flag: Option<PathBuf>,

        /// Path to the output file or directory (positional, optional)
        #[arg(value_name = "OUTPUT", conflicts_with = "output_flag")]
        output: Option<PathBuf>,

        /// Path to the output file or directory (flag, optional)
        #[arg(
            short = 'o',
            long = "output",
            value_name = "OUTPUT",
            conflicts_with = "output"
        )]
        output_flag: Option<PathBuf>,

        /// Generate a full favicon package instead of a single .ico file (only for SVG -> favicon)
        #[arg(long)]
        package: bool,

        /// Compression quality (1-100, default 80)
        #[arg(long, default_value_t = 80)]
        quality: u8,

        /// Enable lossless compression
        #[arg(long)]
        lossless: bool,

        /// Search directory and its subdirectories recursively
        #[arg(short, long)]
        recursive: bool,

        /// Delete original images after successful conversion
        #[arg(short, long)]
        delete_original: bool,

        /// Overwrite output files if they already exist
        #[arg(long)]
        overwrite: bool,

        /// Number of parallel worker threads. Defaults to CPU count.
        #[arg(short, long)]
        workers: Option<usize>,

        /// Page range to convert from PDF (e.g. '1,3-5'). Defaults to all pages.
        #[arg(long)]
        pages: Option<String>,

        /// Resolution in DPI for PDF rendering (default: 150)
        #[arg(long, default_value_t = 150)]
        dpi: u16,

        /// Target width in pixels (optional)
        #[arg(long)]
        width: Option<u32>,

        /// Target height in pixels (optional)
        #[arg(long)]
        height: Option<u32>,

        /// Prevent upscaling of images smaller than the target dimensions
        #[arg(long)]
        no_upscale: bool,

        /// Document styling theme for Markdown to PDF (github, academic, modern, minimal)
        #[arg(long)]
        theme: Option<String>,

        /// Paper size for PDF rendering (a4, letter, a3, a5)
        #[arg(long)]
        paper_size: Option<String>,

        /// Page margins for PDF (e.g. '20mm', '1in')
        #[arg(long)]
        margin: Option<String>,

        /// Custom font family for PDF rendering
        #[arg(long)]
        font_family: Option<String>,

        /// Custom header text for PDF
        #[arg(long)]
        header: Option<String>,

        /// Custom footer text for PDF
        #[arg(long)]
        footer: Option<String>,

        /// Disable page numbers in PDF footer
        #[arg(long)]
        no_page_numbers: bool,

        /// Automatically generate Table of Contents in PDF
        #[arg(long)]
        toc: bool,

        /// Apply a named preset from martini-presets.toml
        #[arg(long)]
        preset: Option<String>,
    },

    /// List all supported format conversions and their parameters
    ListFormats,

    /// Manage and inspect conversion presets
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },

    /// Watch a directory and automatically convert incoming files
    Watch {
        /// Path to the input directory to watch (inbox)
        #[arg(short = 'i', long = "input", value_name = "INPUT")]
        input: PathBuf,

        /// Path to the output directory (outbox)
        #[arg(short = 'o', long = "output", value_name = "OUTPUT")]
        output: PathBuf,

        /// Apply a named preset from martini-presets.toml
        #[arg(long)]
        preset: Option<String>,

        /// Target format to convert into (e.g. webp, yaml, json)
        #[arg(long)]
        to: Option<String>,

        /// Settle time in milliseconds before processing a newly written file (default: 500)
        #[arg(long, default_value_t = 500)]
        settle_ms: u64,

        /// Watch subdirectories recursively
        #[arg(short, long)]
        recursive: bool,

        /// Process existing files in the directory on startup before watching
        #[arg(long)]
        initial: bool,

        /// Delete original files after successful conversion
        #[arg(long)]
        delete_original: bool,

        /// Move original files to a directory after conversion
        #[arg(long, value_name = "DIR")]
        move_originals: Option<PathBuf>,

        /// Overwrite output files if they already exist
        #[arg(long)]
        overwrite: bool,
    },

    /// Erase objects or watermarks from an image using natural language or a mask
    Erase(crate::erase::EraseArgs),
}

impl Commands {
    pub fn preset_name(&self) -> Option<&str> {
        match self {
            Commands::Convert { preset, .. } | Commands::Watch { preset, .. } => preset.as_deref(),
            _ => None,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum PresetAction {
    /// List all available presets
    List,
    /// Show detailed configuration of a preset
    Show {
        /// Name of the preset
        name: String,
    },
}
