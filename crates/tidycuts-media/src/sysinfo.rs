//! Facts about this computer that the program (and a person reading a problem report) can use:
//! the graphics cards with their memory, and how much room is left on a disk.

use std::path::Path;

/// One graphics card, as Windows lists it.
#[derive(Debug, Clone)]
pub struct GraphicsCard {
    pub name: String,
    /// Memory on the card itself.
    pub dedicated_bytes: u64,
    /// System memory the card can also use (all it has, on a processor's built-in graphics).
    pub shared_bytes: u64,
    /// Windows' own software drawing, not a real card.
    pub software: bool,
}

/// The graphics cards of this computer (empty where this cannot be asked).
#[cfg(windows)]
pub fn graphics_cards() -> Vec<GraphicsCard> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1};
    // SAFETY: plain DXGI enumeration; every interface is released when it goes out of scope.
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let Ok(d) = adapter.GetDesc1() else {
                continue;
            };
            let len = d
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(d.Description.len());
            out.push(GraphicsCard {
                name: String::from_utf16_lossy(&d.Description[..len]),
                dedicated_bytes: d.DedicatedVideoMemory as u64,
                shared_bytes: d.SharedSystemMemory as u64,
                // DXGI_ADAPTER_FLAG_SOFTWARE
                software: d.Flags & 2 != 0,
            });
        }
        out
    }
}

#[cfg(not(windows))]
pub fn graphics_cards() -> Vec<GraphicsCard> {
    Vec::new()
}

/// Free bytes on the disk that holds `path` (the folder need not exist yet).
#[cfg(windows)]
pub fn free_disk_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    let wide: Vec<u16> = p
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut free = 0u64;
    // SAFETY: `wide` is a nul-ended string and `free` outlives the call.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(wide.as_ptr()),
            Some(&mut free as *mut u64),
            None,
            None,
        )
        .ok()?;
    }
    Some(free)
}

#[cfg(not(windows))]
pub fn free_disk_bytes(_path: &Path) -> Option<u64> {
    None
}
