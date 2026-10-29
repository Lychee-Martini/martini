use crate::error::MartiniError;

#[cfg(feature = "nn-cpu")]
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

#[cfg(feature = "cuda")]
fn configure_cuda_device(dev: Device) -> (Device, String) {
    candle_core::cuda_backend::set_gemm_reduced_precision_f16(true);
    candle_core::cuda_backend::set_gemm_reduced_precision_bf16(true);
    candle_core::cuda_backend::set_gemm_reduced_precision_f32(true);

    let label = dev
        .as_cuda_device()
        .ok()
        .and_then(|c| c.cuda_stream().context().name().ok())
        .map(|name| format!("CUDA (GPU 0: {})", name.trim()))
        .unwrap_or_else(|| "CUDA (GPU 0)".to_string());
    (dev, label)
}

#[cfg(feature = "nn-cpu")]
pub fn select_candle_device(target: DeviceTarget) -> (Device, String) {
    match target {
        DeviceTarget::Cpu => (Device::Cpu, "CPU".to_string()),
        DeviceTarget::Cuda => {
            #[cfg(feature = "cuda")]
            {
                match Device::new_cuda(0) {
                    Ok(dev) => configure_cuda_device(dev),
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
                    return configure_cuda_device(dev);
                }
            }
            (Device::Cpu, "CPU".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_device_target_parsing() {
        assert_eq!(resolve_device_target("auto").unwrap(), DeviceTarget::Auto);
        assert_eq!(resolve_device_target("cuda").unwrap(), DeviceTarget::Cuda);
        assert_eq!(resolve_device_target("cpu").unwrap(), DeviceTarget::Cpu);
        assert!(resolve_device_target("invalid").is_err());
    }
}
