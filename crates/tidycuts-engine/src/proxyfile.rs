//! Reads single pictures straight out of a proxy file, without starting a
//! program. A proxy is a QuickTime file of JPEG pictures, one per frame, so
//! once the table of where each picture sits is read, any frame is a file
//! read and a JPEG decode: a few milliseconds, instead of a quarter of a
//! second to start ffmpeg. That is what makes jumping around instant.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tidycuts_render::RgbaImage;

/// Where every picture of a proxy file is, and when it shows.
pub struct ProxyIndex {
    file: Mutex<File>,
    offsets: Vec<u64>,
    sizes: Vec<u32>,
    /// Start of each picture, in `timescale` units.
    starts: Vec<u64>,
    timescale: f64,
    /// The picture decoded last, in case the same one is asked for again.
    last: Mutex<Option<(usize, Arc<RgbaImage>)>>,
}

/// The tables of the proxy files looked at so far (`None`: not readable
/// this way), shared by the still picture, the scrub bar and playback.
static OPENED: Mutex<Option<HashMap<PathBuf, Option<Arc<ProxyIndex>>>>> = Mutex::new(None);

/// The picture table of `path` when it is one of the app's proxy files.
pub fn shared(path: &Path) -> Option<Arc<ProxyIndex>> {
    if path.parent().and_then(|d| d.file_name()) != Some(std::ffi::OsStr::new("proxies")) {
        return None;
    }
    let mut all = OPENED.lock().unwrap();
    all.get_or_insert_with(HashMap::new)
        .entry(path.to_path_buf())
        .or_insert_with(|| ProxyIndex::open(path).map(Arc::new))
        .clone()
}

fn be32(b: &[u8]) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(..4)?.try_into().ok()?))
}

fn be64(b: &[u8]) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(..8)?.try_into().ok()?))
}

/// The boxes inside `data`: (type, content).
fn boxes(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= data.len() {
        let Some(size) = be32(&data[i..]) else { break };
        let kind: [u8; 4] = data[i + 4..i + 8].try_into().unwrap();
        let (head, total) = match size {
            0 => (8, data.len() - i),
            1 => match data.get(i + 8..).and_then(be64) {
                Some(s) => (16, s as usize),
                None => break,
            },
            s => (8, s as usize),
        };
        if total < head || i + total > data.len() {
            break;
        }
        out.push((kind, &data[i + head..i + total]));
        i += total;
    }
    out
}

fn find<'a>(list: &[([u8; 4], &'a [u8])], kind: &[u8; 4]) -> Option<&'a [u8]> {
    list.iter().find(|(k, _)| k == kind).map(|(_, d)| *d)
}

impl ProxyIndex {
    /// Reads the table of pictures of the proxy at `path`. `None` when the
    /// file is not laid out the way proxies are.
    pub fn open(path: &Path) -> Option<ProxyIndex> {
        let mut file = File::open(path).ok()?;
        let len = file.metadata().ok()?.len();
        // Find the "moov" box among the top-level boxes by hopping over them.
        let mut pos = 0u64;
        let mut moov = None;
        while pos + 8 <= len {
            file.seek(SeekFrom::Start(pos)).ok()?;
            let mut head = [0u8; 16];
            let n = file.read(&mut head).ok()?;
            if n < 8 {
                break;
            }
            let size = be32(&head)? as u64;
            let (hdr, total) = match size {
                0 => (8, len - pos),
                1 => (16, be64(&head[8..])?),
                s => (8, s),
            };
            if total < hdr {
                break;
            }
            if &head[4..8] == b"moov" {
                moov = Some((pos + hdr, total - hdr));
                break;
            }
            pos += total;
        }
        let (at, size) = moov?;
        if size > 256 << 20 {
            return None;
        }
        file.seek(SeekFrom::Start(at)).ok()?;
        let mut data = vec![0u8; size as usize];
        file.read_exact(&mut data).ok()?;

        let top = boxes(&data);
        for (kind, trak) in &top {
            if kind != b"trak" {
                continue;
            }
            let t = boxes(trak);
            let Some(mdia) = find(&t, b"mdia") else {
                continue;
            };
            let m = boxes(mdia);
            let is_video = find(&m, b"hdlr").is_some_and(|h| h.get(8..12) == Some(b"vide"));
            if !is_video {
                continue;
            }
            let mdhd = find(&m, b"mdhd")?;
            let timescale = if mdhd.first() == Some(&1) {
                be32(mdhd.get(20..)?)?
            } else {
                be32(mdhd.get(12..)?)?
            };
            let minf = boxes(find(&m, b"minf")?);
            let stbl = boxes(find(&minf, b"stbl")?);

            // Sizes of the pictures.
            let stsz = find(&stbl, b"stsz")?;
            let fixed = be32(stsz.get(4..)?)?;
            let count = be32(stsz.get(8..)?)? as usize;
            let mut sizes = Vec::with_capacity(count);
            for i in 0..count {
                sizes.push(if fixed != 0 {
                    fixed
                } else {
                    be32(stsz.get(12 + i * 4..)?)?
                });
            }

            // Where each run of pictures (a "chunk") starts in the file.
            let (offsets_of_chunks, wide) = match find(&stbl, b"stco") {
                Some(d) => (d, false),
                None => (find(&stbl, b"co64")?, true),
            };
            let n_chunks = be32(offsets_of_chunks.get(4..)?)? as usize;
            let mut chunk_offsets = Vec::with_capacity(n_chunks);
            for i in 0..n_chunks {
                chunk_offsets.push(if wide {
                    be64(offsets_of_chunks.get(8 + i * 8..)?)?
                } else {
                    u64::from(be32(offsets_of_chunks.get(8 + i * 4..)?)?)
                });
            }

            // How many pictures each chunk holds.
            let stsc = find(&stbl, b"stsc")?;
            let n_runs = be32(stsc.get(4..)?)? as usize;
            let mut runs = Vec::with_capacity(n_runs);
            for i in 0..n_runs {
                let first = be32(stsc.get(8 + i * 12..)?)? as usize;
                let per = be32(stsc.get(12 + i * 12..)?)? as usize;
                runs.push((first, per));
            }
            let mut offsets = Vec::with_capacity(count);
            'chunks: for (ci, base) in chunk_offsets.iter().enumerate() {
                let chunk_no = ci + 1;
                let per = runs
                    .iter()
                    .rev()
                    .find(|(first, _)| *first <= chunk_no)
                    .map(|r| r.1)?;
                let mut off = *base;
                for _ in 0..per {
                    if offsets.len() >= count {
                        break 'chunks;
                    }
                    offsets.push(off);
                    off += u64::from(sizes[offsets.len() - 1]);
                }
            }
            if offsets.len() != count || count == 0 {
                return None;
            }

            // When each picture shows.
            let stts = find(&stbl, b"stts")?;
            let n_entries = be32(stts.get(4..)?)? as usize;
            let mut starts = Vec::with_capacity(count);
            let mut now = 0u64;
            for i in 0..n_entries {
                let c = be32(stts.get(8 + i * 8..)?)? as usize;
                let d = u64::from(be32(stts.get(12 + i * 8..)?)?);
                for _ in 0..c {
                    starts.push(now);
                    now += d;
                }
            }
            if starts.len() < count || timescale == 0 {
                return None;
            }
            starts.truncate(count);
            return Some(ProxyIndex {
                file: Mutex::new(file),
                offsets,
                sizes,
                starts,
                timescale: f64::from(timescale),
                last: Mutex::new(None),
            });
        }
        None
    }

    pub fn frame_count(&self) -> usize {
        self.offsets.len()
    }

    /// The picture showing at `secs` (the last one starting at or before it).
    pub fn frame_at(&self, secs: f64) -> usize {
        let t = (secs.max(0.0) * self.timescale).round() as u64;
        self.starts
            .partition_point(|s| *s <= t)
            .saturating_sub(1)
            .min(self.offsets.len() - 1)
    }

    /// The decoded picture showing at `secs`.
    pub fn picture(&self, secs: f64) -> Option<Arc<RgbaImage>> {
        let n = self.frame_at(secs);
        if let Some((k, img)) = self.last.lock().unwrap().as_ref() {
            if *k == n {
                return Some(img.clone());
            }
        }
        let img = Arc::new(crate::scrub::decode_jpeg(&self.jpeg(n)?)?);
        *self.last.lock().unwrap() = Some((n, img.clone()));
        Some(img)
    }

    /// The JPEG bytes of picture `n`.
    pub fn jpeg(&self, n: usize) -> Option<Vec<u8>> {
        let (off, size) = (*self.offsets.get(n)?, *self.sizes.get(n)?);
        let mut f = self.file.lock().ok()?;
        f.seek(SeekFrom::Start(off)).ok()?;
        let mut buf = vec![0u8; size as usize];
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tidycuts_media::Tools;

    #[test]
    fn reads_the_pictures_of_a_proxy_without_ffmpeg_running() {
        let Ok(tools) = Tools::get() else {
            eprintln!("no ffmpeg here; skipping");
            return;
        };
        let dir = std::env::temp_dir().join(format!("tidycuts-proxyfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("p.mov");
        let ok = tools
            .ffmpeg_cmd()
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:duration=2",
            ])
            .args(["-vf", "format=yuvj420p", "-c:v", "mjpeg", "-q:v", "5"])
            .arg(&out)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let idx = ProxyIndex::open(&out).expect("the table of pictures");
        assert_eq!(idx.frame_count(), 50);
        assert_eq!(idx.frame_at(0.0), 0);
        assert_eq!(idx.frame_at(1.0), 25);
        assert_eq!(idx.frame_at(99.0), 49);
        let jpg = idx.jpeg(25).unwrap();
        let img = crate::scrub::decode_jpeg(&jpg).expect("a picture");
        assert_eq!((img.width, img.height), (320, 180));
        // Different frames are different pictures.
        assert_ne!(jpg, idx.jpeg(26).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
