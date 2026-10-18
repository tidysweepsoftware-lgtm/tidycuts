// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod autoduck;
mod captions;
mod detection;
mod dialogs;
mod easing;
mod engine;
mod inspector;
mod layouts;
mod library;
mod library_thumbs;
mod mcp;
mod mcp_tools;
mod menus;
mod meter;
mod mixer;
mod prefs;
mod preview;
mod previews;
mod proxies;
mod scopes;
mod sounds;
mod stills;
mod templates;
mod theme;
mod thumbnail;
mod thumbs;
mod timeline;
mod voiceover;
mod wave;
mod widgets;

/// The app icon for the window and taskbar.
fn window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../packaging/icons/tidycuts-256.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(&bytes[..]))
        .read_info()
        .ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    buf.truncate(info.buffer_size());
    Some(egui::IconData {
        rgba: buf,
        width: info.width,
        height: info.height,
    })
}

fn main() -> eframe::Result<()> {
    app::app_start();
    prefs::migrate_legacy_folders();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let open_paths: Vec<std::path::PathBuf> = std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .collect();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("TidyCuts")
        .with_inner_size([1600.0, 960.0])
        .with_min_inner_size([1100.0, 680.0])
        .with_drag_and_drop(true);
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "TidyCuts",
        options,
        Box::new(move |cc| Ok(Box::new(app::TidyCutsApp::new(cc, open_paths)))),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_icon_is_a_full_colour_square() {
        let icon = super::window_icon().expect("the icon decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // The rounded corners are see-through; the middle is not.
        assert_eq!(icon.rgba[3], 0);
        let mid = (128 * 256 + 128) * 4;
        assert_eq!(icon.rgba[mid + 3], 255);
    }
}
