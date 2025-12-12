//! Hardware detection for embedded LLM inference

#[cfg(feature = "cuda")]
use candle_core::CudaDevice;
use sysinfo::System;
use tracing::{debug, info};

/// Detected hardware capabilities
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    /// Total system RAM in bytes
    pub total_ram_bytes: u64,
    /// Available RAM in bytes
    pub available_ram_bytes: u64,
    /// Whether CUDA is available
    pub cuda_available: bool,
    /// CUDA device memory in bytes (if available)
    pub cuda_memory_bytes: Option<u64>,
    /// Whether Metal is available (macOS)
    pub metal_available: bool,
    /// Number of CPU cores
    pub cpu_cores: usize,
}

impl HardwareCapabilities {
    /// Returns available RAM in gigabytes
    pub fn available_ram_gb(&self) -> f64 {
        self.available_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// Returns total RAM in gigabytes
    pub fn total_ram_gb(&self) -> f64 {
        self.total_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    /// Returns the best available compute device
    pub fn best_device(&self) -> ComputeDevice {
        if self.cuda_available {
            ComputeDevice::Cuda
        } else if self.metal_available {
            ComputeDevice::Metal
        } else {
            ComputeDevice::Cpu
        }
    }
}

/// Available compute devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeDevice {
    Cpu,
    Cuda,
    Metal,
}

impl std::fmt::Display for ComputeDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComputeDevice::Cpu => write!(f, "CPU"),
            ComputeDevice::Cuda => write!(f, "CUDA"),
            ComputeDevice::Metal => write!(f, "Metal"),
        }
    }
}

/// Detects hardware capabilities for LLM inference
pub struct HardwareDetector;

impl HardwareDetector {
    /// Detect current hardware capabilities
    pub fn detect() -> HardwareCapabilities {
        let mut sys = System::new_all();
        sys.refresh_all();

        let total_ram_bytes = sys.total_memory();
        let available_ram_bytes = sys.available_memory();
        let cpu_cores = sys.cpus().len();

        // Check for CUDA availability
        let (cuda_available, cuda_memory_bytes) = Self::detect_cuda();

        // Check for Metal availability (macOS only)
        let metal_available = Self::detect_metal();

        let capabilities = HardwareCapabilities {
            total_ram_bytes,
            available_ram_bytes,
            cuda_available,
            cuda_memory_bytes,
            metal_available,
            cpu_cores,
        };

        info!(
            "Hardware detected: {:.1}GB RAM available ({:.1}GB total), {} cores, device: {}",
            capabilities.available_ram_gb(),
            capabilities.total_ram_gb(),
            cpu_cores,
            capabilities.best_device()
        );

        debug!("Hardware capabilities: {:?}", capabilities);

        capabilities
    }

    #[cfg(feature = "cuda")]
    fn detect_cuda() -> (bool, Option<u64>) {
        use candle_core::backend::BackendDevice;
        use candle_core::cuda::cudarc;
        use std::mem::MaybeUninit;

        match CudaDevice::new(0) {
            Ok(_device) => {
                let context = match cudarc::driver::CudaContext::new(0) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        debug!("Failed to create CUDA context: {}", e);
                        return (true, None);
                    }
                };

                let cu_device = context.cu_device();
                let memory_bytes = unsafe {
                    let mut bytes = MaybeUninit::uninit();
                    match cudarc::driver::sys::cuDeviceTotalMem_v2(bytes.as_mut_ptr(), cu_device) {
                        cudarc::driver::sys::cudaError_enum::CUDA_SUCCESS => {
                            Some(bytes.assume_init() as u64)
                        }
                        _ => None,
                    }
                };

                if let Some(bytes) = memory_bytes {
                    info!(
                        "CUDA device detected with {:.2} GB memory",
                        bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                    );
                } else {
                    info!("CUDA device detected (memory info unavailable)");
                }

                (true, memory_bytes)
            }
            Err(e) => {
                debug!("CUDA not available: {}", e);
                (false, None)
            }
        }
    }

    #[cfg(not(feature = "cuda"))]
    fn detect_cuda() -> (bool, Option<u64>) {
        debug!("CUDA support not compiled (cuda feature not enabled)");
        (false, None)
    }

    #[cfg(feature = "metal")]
    fn detect_metal() -> bool {
        use candle_core::metal_backend::MetalDevice;

        match MetalDevice::new(0) {
            Ok(_) => {
                info!("Metal device detected");
                true
            }
            Err(e) => {
                debug!("Metal not available: {}", e);
                false
            }
        }
    }

    #[cfg(not(feature = "metal"))]
    fn detect_metal() -> bool {
        debug!("Metal support not compiled (metal feature not enabled)");
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_detection() {
        let caps = HardwareDetector::detect();

        // Basic sanity checks
        assert!(caps.total_ram_bytes > 0);
        assert!(caps.available_ram_bytes > 0);
        assert!(caps.available_ram_bytes <= caps.total_ram_bytes);
        assert!(caps.cpu_cores > 0);
    }

    #[test]
    fn test_ram_gb_conversion() {
        let caps = HardwareCapabilities {
            total_ram_bytes: 16 * 1024 * 1024 * 1024,    // 16 GB
            available_ram_bytes: 8 * 1024 * 1024 * 1024, // 8 GB
            cuda_available: false,
            cuda_memory_bytes: None,
            metal_available: false,
            cpu_cores: 8,
        };

        assert!((caps.total_ram_gb() - 16.0).abs() < 0.1);
        assert!((caps.available_ram_gb() - 8.0).abs() < 0.1);
    }

    #[test]
    fn test_best_device_selection() {
        // CPU only
        let cpu_caps = HardwareCapabilities {
            total_ram_bytes: 16 * 1024 * 1024 * 1024,
            available_ram_bytes: 8 * 1024 * 1024 * 1024,
            cuda_available: false,
            cuda_memory_bytes: None,
            metal_available: false,
            cpu_cores: 8,
        };
        assert_eq!(cpu_caps.best_device(), ComputeDevice::Cpu);

        // CUDA available
        let cuda_caps = HardwareCapabilities {
            cuda_available: true,
            cuda_memory_bytes: Some(8 * 1024 * 1024 * 1024),
            ..cpu_caps.clone()
        };
        assert_eq!(cuda_caps.best_device(), ComputeDevice::Cuda);

        // Metal available (but not CUDA)
        let metal_caps = HardwareCapabilities {
            metal_available: true,
            ..cpu_caps.clone()
        };
        assert_eq!(metal_caps.best_device(), ComputeDevice::Metal);
    }
}
