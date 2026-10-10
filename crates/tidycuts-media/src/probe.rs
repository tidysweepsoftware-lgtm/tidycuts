use crate::ffmpeg::{FfmpegError, Tools};
use serde::Deserialize;
use std::path::Path;
use tidycuts_core::{FrameRate, MediaItem, MediaKind, Time};

#[derive(Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    duration: Option<String>,
    pix_fmt: Option<String>,
    color_space: Option<String>,
    color_range: Option<String>,
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    side_data_list: Vec<SideData>,
    #[serde(default)]
    disposition: Disposition,
}

#[derive(Deserialize, Default)]
struct SideData {
    rotation: Option<f64>,
}

#[derive(Deserialize, Default)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Deserialize)]
struct ProbeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

const IMAGE_CODECS: &[&str] = &[
    "png",
    "mjpeg",
    "jpegls",
    "bmp",
    "tiff",
    "webp",
    "gif_still",
    "jpeg2000",
    "qoi",
];

/// Reads a file's type, length and video size with ffprobe.
pub fn probe(path: &Path) -> Result<MediaItem, FfmpegError> {
    let tools = Tools::get()?;
    let out = tools
        .ffprobe_cmd()
        .args(["-print_format", "json", "-show_format", "-show_streams"])
        .arg(path)
        .output()
        .map_err(|e| FfmpegError::Spawn("ffprobe", e))?;
    if !out.status.success() {
        return Err(FfmpegError::Failed {
            tool: "ffprobe",
            message: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    let json = String::from_utf8_lossy(&out.stdout);
    media_from_probe_json(path, &json)
}

pub(crate) fn media_from_probe_json(path: &Path, json: &str) -> Result<MediaItem, FfmpegError> {
    let parsed: ProbeOutput =
        serde_json::from_str(json).map_err(|e| FfmpegError::BadOutput(e.to_string()))?;
    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video") && s.disposition.attached_pic == 0);
    let audio_streams = parsed
        .streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .count() as u32;
    let has_audio = audio_streams > 0;
    let format_name = parsed
        .format
        .as_ref()
        .and_then(|f| f.format_name.clone())
        .unwrap_or_default();
    let secs = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| video.and_then(|v| v.duration.as_deref()))
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0);

    let is_image = video.is_some_and(|v| {
        let codec = v.codec_name.as_deref().unwrap_or("");
        format_name.contains("image2")
            || format_name.ends_with("_pipe")
            || (IMAGE_CODECS.contains(&codec) && secs.is_none())
    });

    let kind = match (video, is_image, has_audio) {
        (Some(_), true, _) => MediaKind::Image,
        (Some(_), false, _) => MediaKind::Video,
        (None, _, true) => MediaKind::Audio,
        (None, _, false) => {
            return Err(FfmpegError::BadOutput(
                "the file has no video or audio".into(),
            ))
        }
    };
    let duration = match kind {
        MediaKind::Image => MediaItem::DEFAULT_IMAGE_DURATION,
        _ => Time::from_secs_f64(
            secs.ok_or_else(|| FfmpegError::BadOutput("unknown duration".into()))?,
        ),
    };

    let mut item = MediaItem::new(path.to_path_buf(), kind, duration);
    item.has_audio = has_audio && kind != MediaKind::Image;
    item.audio_streams = if item.has_audio { audio_streams } else { 0 };
    if let Some(v) = video {
        item.width = v.width.unwrap_or(0);
        item.height = v.height.unwrap_or(0);
        // Phones store portrait video sideways plus a rotation; ffmpeg turns
        // it upright when decoding, so report the upright size.
        let rotation = v
            .side_data_list
            .iter()
            .find_map(|d| d.rotation)
            .or_else(|| v.tags.get("rotate").and_then(|r| r.parse().ok()))
            .unwrap_or(0.0);
        if ((rotation.abs() as i64) / 90) % 2 == 1 {
            std::mem::swap(&mut item.width, &mut item.height);
        }
        let pix = v.pix_fmt.as_deref().unwrap_or("");
        item.has_alpha = pixel_format_has_alpha(pix);
        item.full_range = v.color_range.as_deref() == Some("pc") || pix.starts_with("yuvj");
        item.color_matrix = v.color_space.as_deref().and_then(|c| match c {
            "bt709" => Some("bt709".to_string()),
            "bt470bg" | "smpte170m" | "bt601" | "fcc" => Some("bt601".to_string()),
            "bt2020nc" | "bt2020c" => Some("bt2020".to_string()),
            _ => None,
        });
        if kind == MediaKind::Video {
            item.frame_rate = v
                .avg_frame_rate
                .as_deref()
                .and_then(FrameRate::parse)
                .or_else(|| v.r_frame_rate.as_deref().and_then(FrameRate::parse));
        }
    }
    Ok(item)
}

/// Whether an ffmpeg pixel format name carries transparency.
pub fn pixel_format_has_alpha(pix: &str) -> bool {
    pix.starts_with("yuva")
        || pix.starts_with("rgba")
        || pix.starts_with("bgra")
        || pix.starts_with("argb")
        || pix.starts_with("abgr")
        || pix.starts_with("gbrap")
        || pix.starts_with("ya")
        || pix == "pal8"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_video_with_audio() {
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001","r_frame_rate":"30000/1001"},
            {"codec_type":"audio","codec_name":"aac"}],
            "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"12.500000"}}"#;
        let m = media_from_probe_json(Path::new("a.mp4"), json).unwrap();
        assert_eq!(m.kind, MediaKind::Video);
        assert!(m.has_audio);
        assert_eq!((m.width, m.height), (1920, 1080));
        assert_eq!(m.duration, Time::from_secs_f64(12.5));
        assert_eq!(m.frame_rate, Some(FrameRate::new(30000, 1001)));
    }

    #[test]
    fn classifies_still_images() {
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"png","width":800,"height":600}],
            "format":{"format_name":"png_pipe"}}"#;
        let m = media_from_probe_json(Path::new("a.png"), json).unwrap();
        assert_eq!(m.kind, MediaKind::Image);
        assert_eq!(m.duration, MediaItem::DEFAULT_IMAGE_DURATION);
        assert!(!m.has_audio);
    }

    #[test]
    fn reads_rotation_alpha_and_color() {
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"hevc","width":1920,"height":1080,"avg_frame_rate":"30/1",
             "pix_fmt":"yuv420p10le","color_space":"bt2020nc","color_range":"tv",
             "side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]},
            {"codec_type":"audio"},{"codec_type":"audio"}],
            "format":{"format_name":"mov,mp4","duration":"3.0"}}"#;
        let m = media_from_probe_json(Path::new("phone.mov"), json).unwrap();
        assert_eq!((m.width, m.height), (1080, 1920));
        assert_eq!(m.color_matrix.as_deref(), Some("bt2020"));
        assert!(!m.full_range && !m.has_alpha);
        assert_eq!(m.audio_streams, 2);
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"vp9","width":640,"height":360,
            "avg_frame_rate":"25/1","pix_fmt":"yuva420p","tags":{"rotate":"180"}}],
            "format":{"format_name":"matroska,webm","duration":"2.0"}}"#;
        let m = media_from_probe_json(Path::new("a.webm"), json).unwrap();
        assert!(m.has_alpha);
        assert_eq!((m.width, m.height), (640, 360));
    }

    #[test]
    fn music_with_cover_art_is_audio() {
        let json = r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"},
            {"codec_type":"video","codec_name":"mjpeg","width":500,"height":500,"disposition":{"attached_pic":1}}],
            "format":{"format_name":"mp3","duration":"180.0"}}"#;
        let m = media_from_probe_json(Path::new("song.mp3"), json).unwrap();
        assert_eq!(m.kind, MediaKind::Audio);
        assert_eq!(m.duration, Time::from_secs_f64(180.0));
    }
}
