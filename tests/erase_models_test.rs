use tempfile::tempdir;

#[test]
fn test_model_manager_custom_cache_path() {
    let tmp = tempdir().unwrap();
    let mgr = martini::erase::models::ModelManager::new(Some(tmp.path())).unwrap();
    assert_eq!(mgr.cache_dir, tmp.path().join("erase"));
}

#[test]
fn test_model_manager_default_cache_path() {
    let mgr = martini::erase::models::ModelManager::new(None).unwrap();
    let path_str = mgr.cache_dir.to_string_lossy();
    assert!(path_str.contains("martini") && path_str.contains("erase"));
}

#[test]
fn test_model_manager_cache_paths() {
    let tmp = tempdir().unwrap();
    let mgr = martini::erase::models::ModelManager::new(Some(tmp.path())).unwrap();
    let paths = mgr.get_expected_paths(martini::erase::models::InpaintModelPreset::Sd15);
    assert!(paths.unet_weights.to_string_lossy().contains("sd-1.5"));
    assert!(paths.clip_weights.to_string_lossy().contains("clip"));
}
