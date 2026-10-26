use martini::erase::benchmark::BenchmarkReport;

#[test]
fn test_benchmark_report_calculations() {
    let report = BenchmarkReport {
        localization_ms: 100,
        vae_encode_ms: 50,
        denoise_ms: 800,
        denoise_steps: 20,
        denoise_step_avg_ms: 40.0,
        vae_decode_ms: 60,
        alpha_blend_ms: 10,
        total_ms: 1020,
        peak_vram_mb: Some(2400),
    };
    assert_eq!(report.denoise_step_avg_ms, 40.0);
    assert_eq!(report.total_ms, 1020);
}
