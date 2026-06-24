use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "martini")]
#[command(author = "Lychee Martini <info@lycheemartini.com>")]
#[command(version = "0.1.0")]
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
pub enum Commands {
    /// Convert a file or directory from one format to another
    Convert {
        /// Source format (e.g. svg, png, jpg, webp, avif). Defaults to 'auto' for auto-detection.
        #[arg(long, default_value = "auto")]
        from: String,

        /// Target format (e.g. favicon, webp, avif, png, jpg, both) (optional)
        #[arg(long)]
        to: Option<String>,

        /// Path to the input file or directory
        #[arg(short, long)]
        input: PathBuf,

        /// Path to the output file or directory (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,

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
    },

    /// List all supported format conversions and their parameters
    ListFormats,
}
