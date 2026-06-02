use thiserror::Error;

#[derive(Error, Debug)]
pub enum MartiniError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("SVG parsing error: {0}")]
    SvgParse(#[from] resvg::usvg::Error),

    #[error("Rendering error: {0}")]
    Rendering(String),

    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Unsupported conversion from '{from}' to '{to}'")]
    UnsupportedConversion { from: String, to: String },

    #[error("Input file not found: {path}")]
    InputFileNotFound { path: String },

    #[error("Invalid input data: {reason}")]
    InvalidInputData { reason: String },

    #[error("Output writing error: {reason}")]
    OutputWrite { reason: String },
}

impl MartiniError {
    /// Maps the error to a granular exit code for programmatic consumption.
    pub fn exit_code(&self) -> i32 {
        match self {
            MartiniError::InputFileNotFound { .. } => 2,
            MartiniError::InvalidInputData { .. } | MartiniError::SvgParse(_) => 3,
            MartiniError::Rendering(_) | MartiniError::Image(_) => 4,
            MartiniError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => 2,
            MartiniError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => 5,
            MartiniError::Io(_) => 4, // General system/io processing issue
            MartiniError::OutputWrite { .. } => 5,
            MartiniError::UnsupportedConversion { .. } => 6,
        }
    }
}
