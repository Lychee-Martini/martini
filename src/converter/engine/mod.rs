use crate::converter::{ConvertOptions, Format, FormatCapability, OutputFileMetadata};
use crate::error::MartiniError;
use std::path::Path;

pub mod archive;
pub mod data;
pub mod document;
pub mod image;
pub mod registry;
pub use registry::EngineRegistry;

/// A format conversion engine capable of handling specific (from, to) format pairs.
pub trait FormatEngine: Send + Sync {
    /// Check if this engine can convert between `from` and `to`
    fn can_convert(&self, from: Format, to: Format) -> bool;

    /// Human/agent-facing description of the conversions this engine supports
    fn capabilities(&self) -> Vec<FormatCapability>;

    /// Perform conversion of a file
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError>;
}
