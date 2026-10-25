use martini::converter::engine::{EngineRegistry, FormatEngine};
use martini::converter::{ConvertOptions, Format, FormatCapability, OutputFileMetadata};
use martini::error::MartiniError;
use std::path::Path;

struct DummyEngine;
impl FormatEngine for DummyEngine {
    fn can_convert(&self, from: Format, to: Format) -> bool {
        (from, to) == (Format::Json, Format::Yaml)
    }
    fn capabilities(&self) -> Vec<FormatCapability> {
        vec![FormatCapability::new("json", "yaml", "dummy", &[])]
    }
    fn convert(
        &self,
        _in: &Path,
        _out: &Path,
        _opt: &ConvertOptions,
    ) -> Result<OutputFileMetadata, MartiniError> {
        Ok(OutputFileMetadata {
            path: "test.yaml".into(),
            size_bytes: 42,
            description: "ok".into(),
        })
    }
}

#[test]
fn test_registry_registration_and_lookup() {
    let mut registry = EngineRegistry::empty();
    assert!(registry.find_engine(Format::Json, Format::Yaml).is_none());
    assert!(registry.all_capabilities().is_empty());
    registry.register(Box::new(DummyEngine));
    assert!(registry.find_engine(Format::Json, Format::Yaml).is_some());
    assert!(registry.find_engine(Format::Png, Format::Jpg).is_none());
    assert_eq!(registry.all_capabilities().len(), 1);
}
