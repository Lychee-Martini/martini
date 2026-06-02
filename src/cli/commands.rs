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
    /// Convert a file from one format to another
    Convert {
        /// Source format (e.g. svg)
        #[arg(long)]
        from: String,

        /// Target format (e.g. favicon)
        #[arg(long)]
        to: String,

        /// Path to the input file
        #[arg(short, long)]
        input: PathBuf,

        /// Path to the output file (or directory if generating a package)
        #[arg(short, long)]
        output: PathBuf,

        /// Generate a full favicon package instead of a single .ico file
        #[arg(long)]
        package: bool,
    },

    /// List all supported format conversions and their parameters
    ListFormats,
}
