use martini::erase::models::{InpaintModelPreset, ModelManager};
use std::path::Path;

#[test]
fn test_model_manager_cache_paths() {
    let mgr = ModelManager::new(Some(Path::new("/tmp/test_models"))).unwrap();
    let paths = mgr.get_expected_paths(InpaintModelPreset::Sd15);
    assert!(paths.unet_weights.to_string_lossy().contains("sd-1.5"));
    assert!(paths.clip_weights.to_string_lossy().contains("clip"));
}
