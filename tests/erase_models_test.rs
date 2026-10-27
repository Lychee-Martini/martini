use std::fs;
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
fn test_verify_checksum_valid() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("test.bin");
    fs::write(&file_path, b"martini-model-bytes").unwrap();
    // SHA256 of b"martini-model-bytes"
    let expected = "91e7d1e46e4bb64d0b9ea40628571bc8c305fda625179052759670b2fdff3831";
    assert!(martini::erase::models::ModelManager::verify_checksum(&file_path, expected).unwrap());
}

#[test]
fn test_verify_checksum_invalid() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("corrupted.bin");
    fs::write(&file_path, b"wrong-bytes").unwrap();
    let expected = "91e7d1e46e4bb64d0b9ea40628571bc8c305fda625179052759670b2fdff3831";
    assert!(!martini::erase::models::ModelManager::verify_checksum(&file_path, expected).unwrap());
}

#[test]
fn test_model_manager_cache_paths() {
    let tmp = tempdir().unwrap();
    let mgr = martini::erase::models::ModelManager::new(Some(tmp.path())).unwrap();
    let paths = mgr.get_expected_paths(martini::erase::models::InpaintModelPreset::Sd15);
    assert!(paths.unet_weights.to_string_lossy().contains("sd-1.5"));
    assert!(paths.clip_weights.to_string_lossy().contains("clip"));
}
