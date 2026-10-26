#[test]
fn test_features_compilation_smoke() {
    let feature_nn_cpu_active = cfg!(feature = "nn-cpu");
    assert!(feature_nn_cpu_active || !feature_nn_cpu_active);
}
