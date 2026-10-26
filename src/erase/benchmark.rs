use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub localization_ms: u64,
    pub vae_encode_ms: u64,
    pub denoise_ms: u64,
    pub denoise_steps: usize,
    pub denoise_step_avg_ms: f64,
    pub vae_decode_ms: u64,
    pub alpha_blend_ms: u64,
    pub total_ms: u64,
    pub peak_vram_mb: Option<usize>,
}

impl BenchmarkReport {
    pub fn new(
        localization_ms: u64,
        vae_encode_ms: u64,
        denoise_ms: u64,
        denoise_steps: usize,
        vae_decode_ms: u64,
        alpha_blend_ms: u64,
        peak_vram_mb: Option<usize>,
    ) -> Self {
        let denoise_step_avg_ms = if denoise_steps > 0 {
            denoise_ms as f64 / denoise_steps as f64
        } else {
            0.0
        };
        let total_ms =
            localization_ms + vae_encode_ms + denoise_ms + vae_decode_ms + alpha_blend_ms;

        Self {
            localization_ms,
            vae_encode_ms,
            denoise_ms,
            denoise_steps,
            denoise_step_avg_ms,
            vae_decode_ms,
            alpha_blend_ms,
            total_ms,
            peak_vram_mb,
        }
    }

    pub fn print_table(&self, model_name: &str, device_name: &str) {
        println!(
            "\nBenchmark Results [Model: {} | Device: {}]",
            model_name, device_name
        );
        println!("+---------------------------+------------+");
        println!("| Stage                     | Time (ms)  |");
        println!("+---------------------------+------------+");
        println!(
            "| 1. CLIP Localization      | {:>6} ms  |",
            self.localization_ms
        );
        println!(
            "| 2. VAE Encoding           | {:>6} ms  |",
            self.vae_encode_ms
        );
        println!(
            "| 3. UNet Denoise ({} st.)  | {:>6} ms  |",
            self.denoise_steps, self.denoise_ms
        );
        println!(
            "|    - Avg per step         | {:>6.1} ms  |",
            self.denoise_step_avg_ms
        );
        println!(
            "| 4. VAE Decoding           | {:>6} ms  |",
            self.vae_decode_ms
        );
        println!(
            "| 5. Alpha Blending         | {:>6} ms  |",
            self.alpha_blend_ms
        );
        println!("+---------------------------+------------+");
        println!("| Total Latency             | {:>6} ms  |", self.total_ms);
        if let Some(vram) = self.peak_vram_mb {
            println!("| Peak GPU VRAM             | {:>6} MB  |", vram);
        }
        println!("+---------------------------+------------+\n");
    }
}
