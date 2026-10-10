//! Getting a GPU device when there's no window (exports from the command
//! line, tests, the MCP server).

use crate::Renderer;
use std::sync::{Arc, OnceLock};

/// Who made the graphics card that draws the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Other,
}

impl Vendor {
    /// From the PCI vendor id, falling back on the adapter's name.
    pub fn of(info: &wgpu::AdapterInfo) -> Vendor {
        let name = info.name.to_lowercase();
        match info.vendor {
            0x10DE => Vendor::Nvidia,
            0x1002 | 0x1022 => Vendor::Amd,
            0x8086 => Vendor::Intel,
            0x106B => Vendor::Apple,
            _ if name.contains("nvidia") || name.contains("geforce") => Vendor::Nvidia,
            _ if name.contains("radeon") || name.contains("amd") => Vendor::Amd,
            _ if name.contains("intel") || name.contains("arc") => Vendor::Intel,
            _ if name.contains("apple") => Vendor::Apple,
            _ => Vendor::Other,
        }
    }
}

/// The graphics card in use, as noted by [`note_adapter`].
#[derive(Debug, Clone)]
pub struct Adapter {
    pub name: String,
    pub vendor: Vendor,
    pub backend: String,
    pub discrete: bool,
}

static ADAPTER: OnceLock<Adapter> = OnceLock::new();

/// Remembers which graphics card draws the picture, so that exports can use
/// the video encoder that sits on the same card.
pub fn note_adapter(info: &wgpu::AdapterInfo) {
    let _ = ADAPTER.set(Adapter {
        name: info.name.clone(),
        vendor: Vendor::of(info),
        backend: format!("{:?}", info.backend),
        discrete: info.device_type == wgpu::DeviceType::DiscreteGpu,
    });
}

/// The graphics card in use, once one has been opened.
pub fn adapter() -> Option<&'static Adapter> {
    ADAPTER.get()
}

/// Opens the most capable GPU, preferring a discrete one.
pub fn headless() -> Result<Arc<Renderer>, String> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
        ..Default::default()
    }))
    .map_err(|e| format!("no usable graphics adapter: {e}"))?;
    let info = adapter.get_info();
    log::info!("rendering on {} ({:?})", info.name, info.backend);
    note_adapter(&info);
    let limits = if info.backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("tidycuts headless"),
        required_limits: wgpu::Limits {
            max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d.min(16384),
            ..limits
        },
        ..Default::default()
    }))
    .map_err(|e| format!("could not open the graphics device: {e}"))?;
    Ok(Renderer::new(device, queue))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(vendor: u32, name: &str) -> wgpu::AdapterInfo {
        wgpu::AdapterInfo {
            name: name.into(),
            vendor,
            ..wgpu::AdapterInfo::new(wgpu::DeviceType::Other, wgpu::Backend::Vulkan)
        }
    }

    #[test]
    fn cards_are_told_apart_by_id_then_by_name() {
        assert_eq!(Vendor::of(&info(0x10DE, "RTX 4070")), Vendor::Nvidia);
        assert_eq!(Vendor::of(&info(0x1002, "RX 7800 XT")), Vendor::Amd);
        assert_eq!(Vendor::of(&info(0x8086, "Arc A770")), Vendor::Intel);
        assert_eq!(Vendor::of(&info(0x106B, "Apple M2")), Vendor::Apple);
        assert_eq!(
            Vendor::of(&info(0, "NVIDIA GeForce GTX 1060")),
            Vendor::Nvidia
        );
        assert_eq!(Vendor::of(&info(0, "AMD Radeon Pro")), Vendor::Amd);
        assert_eq!(Vendor::of(&info(0, "llvmpipe")), Vendor::Other);
    }
}
