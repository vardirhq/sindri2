//! Shared `wgpu` device and presentation foundations.
//!
//! Window creation and event loops belong to platform hosts. This crate owns
//! only target-independent adapter/device negotiation and surface policy.

use thiserror::Error;

mod surface;

pub use surface::{SurfaceAction, SurfaceProfile, SurfaceSource, SurfaceStatus, WindowSurface};
pub use wgpu;

#[derive(Clone, Debug)]
pub struct GpuRequestOptions {
    pub power_preference: wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
    pub required_features: wgpu::Features,
    pub required_limits: wgpu::Limits,
    pub memory_hints: wgpu::MemoryHints,
}

impl Default for GpuRequestOptions {
    fn default() -> Self {
        Self {
            power_preference: wgpu::PowerPreference::default(),
            force_fallback_adapter: false,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GpuCapabilities {
    pub adapter_name: String,
    pub backend: wgpu::Backend,
    pub device_type: wgpu::DeviceType,
    pub features: wgpu::Features,
    pub limits: wgpu::Limits,
}

#[derive(Debug)]
pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub capabilities: GpuCapabilities,
}

impl GpuContext {
    pub async fn request(
        instance: &wgpu::Instance,
        compatible_surface: Option<&wgpu::Surface<'_>>,
        options: &GpuRequestOptions,
    ) -> Result<Self, GpuError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: options.power_preference,
                force_fallback_adapter: options.force_fallback_adapter,
                compatible_surface,
                ..Default::default()
            })
            .await?;

        let available_features = adapter.features();
        let missing_features = options.required_features.difference(available_features);
        if !missing_features.is_empty() {
            return Err(GpuError::MissingFeatures(missing_features));
        }

        let adapter_limits = adapter.limits();
        let required_limits = options
            .required_limits
            .clone()
            .using_resolution(adapter_limits.clone());
        let info = adapter.get_info();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Sindri device"),
                required_features: options.required_features,
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: options.memory_hints.clone(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        Ok(Self {
            capabilities: GpuCapabilities {
                adapter_name: info.name,
                backend: info.backend,
                device_type: info.device_type,
                features: available_features,
                limits: adapter_limits,
            },
            adapter,
            device,
            queue,
        })
    }
}

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("no compatible GPU adapter was available: {0}")]
    Adapter(#[from] wgpu::RequestAdapterError),
    #[error("failed to create a logical GPU device: {0}")]
    Device(#[from] wgpu::RequestDeviceError),
    #[error("GPU adapter does not support required features: {0:?}")]
    MissingFeatures(wgpu::Features),
    #[error("surface has no supported presentation configuration")]
    UnsupportedSurface,
    #[error("the presentation surface was lost and could not be created again: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error(
        "surface offers no sRGB format, so rendered colours could not be encoded and every frame \
         would display too dark"
    )]
    NoSrgbSurfaceFormat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_target_format_matches_what_surfaces_must_offer() {
        // A surface is accepted only if it can encode sRGB, which is the same
        // requirement the offscreen and editor targets satisfy by constant.
        assert!(sindri_render::COLOR_TARGET_FORMAT.is_srgb());
    }

    #[test]
    fn request_defaults_require_only_webgpu_baseline_limits() {
        let options = GpuRequestOptions::default();
        assert!(options.required_features.is_empty());
        assert_eq!(options.required_limits, wgpu::Limits::default());
    }
}
