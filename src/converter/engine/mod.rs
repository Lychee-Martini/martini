use crate::converter::{ConvertOptions, Format, OutputFileMetadata};
use crate::error::MartiniError;
use std::path::Path;

pub mod archive;
pub mod data;
pub mod image;
pub mod registry;
pub use registry::EngineRegistry;

/// A format conversion engine capable of handling specific (from, to) format pairs.
pub trait FormatEngine: Send + Sync {
    /// Unique identifier for this engine (e.g. "image", "document", "data", "archive")
    fn name(&self) -> &'static str;

    /// Check if this engine can convert between `from` and `to`
    fn can_convert(&self, from: Format, to: Format) -> bool;

    /// All (from, to) pairs supported by this engine
    fn supported_pairs(&self) -> Vec<(Format, Format)>;

    /// Perform conversion of a file
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError>;
}
