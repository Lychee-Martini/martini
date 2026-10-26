#[test]
fn test_features_compilation_smoke() {
    let feature_erase_active = cfg!(feature = "erase");
    assert!(feature_erase_active || !feature_erase_active);
}
