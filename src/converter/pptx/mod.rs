use crate::converter::{ConversionResult, ConvertOptions};
use crate::error::MartiniError;

pub mod extract;
pub mod font;
pub mod ir;
pub mod writer;

pub fn convert_pdf_to_pptx(
    _input_data: &[u8],
    _options: &ConvertOptions,
) -> Result<ConversionResult, MartiniError> {
    Err(MartiniError::PdfRender(
        "PDF to PPTX conversion is under development".to_string(),
    ))
}
