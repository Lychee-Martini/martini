use crate::error::MartiniError;

#[cfg(feature = "erase")]
use candle_core::Device;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceTarget {
    Auto,
    Cuda,
    Cpu,
}

pub fn resolve_device_target(target: &str) -> Result<DeviceTarget, MartiniError> {
    match target.trim().to_lowercase().as_str() {
        "auto" => Ok(DeviceTarget::Auto),
        "cuda" => Ok(DeviceTarget::Cuda),
        "cpu" => Ok(DeviceTarget::Cpu),
        other => Err(MartiniError::InvalidInputData {
            reason: format!(
                "Unknown device target '{}': expected 'auto', 'cuda', or 'cpu'",
                other
            ),
        }),
    }
}

#[cfg(feature = "erase")]
pub fn select_candle_device(target: DeviceTarget) -> (Device, String) {
    match target {
        DeviceTarget::Cpu => (Device::Cpu, "CPU".to_string()),
        DeviceTarget::Cuda => {
            #[cfg(feature = "cuda")]
            {
                match Device::new_cuda(0) {
                    Ok(dev) => (dev, "CUDA (GPU 0)".to_string()),
                    Err(e) => {
                        tracing::warn!(
                            "CUDA requested but unavailable ({}), falling back to CPU",
                            e
                        );
                        (Device::Cpu, "CPU (fallback)".to_string())
                    }
                }
            }
            #[cfg(not(feature = "cuda"))]
            {
                tracing::warn!(
                    "CUDA requested but binary was built without 'cuda' feature flag; falling back to CPU"
                );
                (Device::Cpu, "CPU".to_string())
            }
        }
        DeviceTarget::Auto => {
            #[cfg(feature = "cuda")]
            {
                if let Ok(dev) = Device::new_cuda(0) {
                    return (dev, "CUDA (GPU 0)".to_string());
                }
            }
            (Device::Cpu, "CPU".to_string())
        }
    }
}

/// Returns estimated peak allocated VRAM in MB if running on CUDA
pub fn get_peak_vram_mb() -> Option<usize> {
    None
}
