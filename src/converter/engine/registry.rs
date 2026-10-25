use super::FormatEngine;
use crate::converter::{Format, FormatCapability};

pub struct EngineRegistry {
    engines: Vec<Box<dyn FormatEngine>>,
}

impl Default for EngineRegistry {
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register(Box::new(super::image::ImageEngine));
        registry.register(Box::new(super::document::DocumentEngine));
        registry.register(Box::new(super::data::DataEngine));
        registry.register(Box::new(super::archive::ArchiveEngine));
        registry
    }
}

impl EngineRegistry {
    pub fn empty() -> Self {
        Self {
            engines: Vec::new(),
        }
    }

    pub fn register(&mut self, engine: Box<dyn FormatEngine>) {
        self.engines.push(engine);
    }

    pub fn find_engine(&self, from: Format, to: Format) -> Option<&dyn FormatEngine> {
        self.engines
            .iter()
            .find(|e| e.can_convert(from, to))
            .map(|e| e.as_ref())
    }

    pub fn all_capabilities(&self) -> Vec<FormatCapability> {
        let mut caps = Vec::new();
        for engine in &self.engines {
            caps.extend(engine.capabilities());
        }
        caps
    }
}
