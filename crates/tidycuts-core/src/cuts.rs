//! Cuts that come from analysis: taking out the silent stretches of a clip,
//! splitting it where the scene changes and marking its beats. Each takes
//! times in the clip's *source* seconds, which is what the detectors report.

use crate::clip::{Clip, ClipId};
use crate::edit::MIN_CLIP;
use crate::project::Project;
use crate::time::Time;

/// Spans shorter than this are not worth a cut.
const MIN_SPAN: f64 = 0.04;

impl Clip {
    /// The timeline moment the clip shows source second `s` at, if it shows
    /// it at all.
    pub fn timeline_of_source(&self, s: f64) -> Option<Time> {
        let (a, b) = (self.source_in.as_secs_f64(), self.source_out.as_secs_f64());
        if s < a || s > b {
            return None;
        }
        Some(self.start + Time::from_secs_f64(self.local_secs(s)))
    }
}

impl Project {
    /// Timeline spans for source ranges, clipped to the clip, in order and
    /// with overlaps joined.
    fn spans_of_source_ranges(&self, clip: &Clip, ranges: &[(f64, f64)]) -> Vec<(Time, Time)> {
        let (a, b) = (clip.source_in.as_secs_f64(), clip.source_out.as_secs_f64());
        let mut spans: Vec<(Time, Time)> = ranges
            .iter()
            .filter_map(|&(x, y)| {
                let (x, y) = (x.max(a), y.min(b));
                if y - x <= 0.0 {
                    return None;
                }
                let (t1, t2) = (clip.timeline_of_source(x)?, clip.timeline_of_source(y)?);
                let (s, e) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };
                (e.as_secs_f64() - s.as_secs_f64() >= MIN_SPAN).then_some((s, e))
            })
            .collect();
        spans.sort();
        let mut joined: Vec<(Time, Time)> = Vec::new();
        for (s, e) in spans {
            match joined.last_mut() {
                Some(last) if s <= last.1 => last.1 = last.1.max(e),
                _ => joined.push((s, e)),
            }
        }
        joined
    }

    /// Cuts the given source ranges out of a clip and closes the gaps they
    /// leave, one by one from the end so earlier times stay valid. Returns
    /// how much time was taken out, or `None` when the clip can't be cut
    /// (it is locked or missing).
    pub fn remove_source_ranges(&mut self, clip_id: ClipId, ranges: &[(f64, f64)]) -> Option<Time> {
        let clip = self.clip(clip_id)?.1.clone();
        let mut removed = Time::ZERO;
        for (ta, tb) in self.spans_of_source_ranges(&clip, ranges).into_iter().rev() {
            let c = self.clip(clip_id)?.1;
            let (start, end) = (c.start, c.end());
            if tb < end - MIN_CLIP && self.split_clip(clip_id, tb).is_none() {
                return (removed > Time::ZERO).then_some(removed);
            }
            let mid = if ta <= start + MIN_CLIP {
                clip_id
            } else {
                match self.split_clip(clip_id, ta) {
                    Some(id) => id,
                    None => return (removed > Time::ZERO).then_some(removed),
                }
            };
            let len = self.clip(mid).map(|(_, c)| c.duration())?;
            if !self.ripple_delete_clip(mid) {
                return (removed > Time::ZERO).then_some(removed);
            }
            removed += len;
        }
        Some(removed)
    }

    /// Splits a clip at the given source times. Returns how many cuts were made.
    pub fn split_at_source_times(&mut self, clip_id: ClipId, times: &[f64]) -> usize {
        let Some(clip) = self.clip(clip_id).map(|(_, c)| c.clone()) else {
            return 0;
        };
        let mut at: Vec<Time> = times
            .iter()
            .filter_map(|t| clip.timeline_of_source(*t))
            .collect();
        at.sort();
        at.dedup();
        let mut made = 0;
        // From the end: the clip keeps its id as the left part.
        for t in at.into_iter().rev() {
            if self.split_clip(clip_id, t).is_some() {
                made += 1;
            }
        }
        made
    }

    /// Moves clips so each one starts `lag` seconds after `reference` starts
    /// (clips lined up by their sound). If that would put one before zero,
    /// all of them move later by the same amount. Returns how many moved.
    pub fn place_synced(&mut self, reference: ClipId, others: &[(ClipId, f64)]) -> usize {
        let Some((_, r)) = self.clip(reference) else {
            return 0;
        };
        let ref_start = r.start.as_secs_f64();
        let mut wanted: Vec<(ClipId, f64)> = vec![(reference, ref_start)];
        wanted.extend(others.iter().map(|(id, lag)| (*id, ref_start + lag)));
        let earliest = wanted.iter().map(|w| w.1).fold(f64::MAX, f64::min);
        let shift = (-earliest).max(0.0);
        let mut moved = 0;
        for (id, start) in wanted {
            let Some(track) = self.clip(id).map(|(t, _)| t.id) else {
                continue;
            };
            if self.move_clip(id, track, Time::from_secs_f64(start + shift)) {
                moved += 1;
            }
        }
        moved
    }

    /// Adds a marker at each given source time of a clip. Returns how many.
    pub fn add_markers_at_source_times(&mut self, clip_id: ClipId, times: &[f64]) -> usize {
        let Some(clip) = self.clip(clip_id).map(|(_, c)| c.clone()) else {
            return 0;
        };
        let mut made = 0;
        for t in times {
            if let Some(at) = clip.timeline_of_source(*t) {
                self.add_marker(at);
                made += 1;
            }
        }
        made
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{MediaItem, MediaKind};
    use std::path::PathBuf;

    fn s(x: f64) -> Time {
        Time::from_secs_f64(x)
    }

    fn project(secs: f64) -> (Project, ClipId) {
        let mut p = Project::new("t");
        let mut m = MediaItem::new(PathBuf::from("/a.mp4"), MediaKind::Video, s(secs));
        m.has_audio = true;
        let id = p.add_media(m);
        let c = p.append_clip(id).unwrap();
        (p, c)
    }

    fn pieces(p: &Project) -> Vec<(f64, f64, f64)> {
        let track = p.main_track().unwrap();
        p.track(track)
            .unwrap()
            .clips
            .iter()
            .map(|c| {
                (
                    c.start.as_secs_f64(),
                    c.source_in.as_secs_f64(),
                    c.source_out.as_secs_f64(),
                )
            })
            .collect()
    }

    #[test]
    fn silent_ranges_are_cut_out_and_the_gaps_close() {
        let (mut p, c) = project(10.0);
        let gone = p
            .remove_source_ranges(c, &[(6.0, 7.0), (2.0, 3.0)])
            .unwrap();
        assert!((gone.as_secs_f64() - 2.0).abs() < 1e-3);
        let got = pieces(&p);
        assert_eq!(got.len(), 3, "{got:?}");
        // Everything that stays plays one after the other.
        assert_eq!(got[0], (0.0, 0.0, 2.0));
        assert_eq!(got[1], (2.0, 3.0, 6.0));
        assert_eq!(got[2], (5.0, 7.0, 10.0));
        assert!((p.duration().as_secs_f64() - 8.0).abs() < 1e-3);
    }

    #[test]
    fn ranges_at_the_ends_and_overlapping_ones_work() {
        let (mut p, c) = project(10.0);
        p.remove_source_ranges(c, &[(0.0, 1.0), (4.0, 5.0), (4.5, 6.0), (9.0, 10.0)])
            .unwrap();
        let got = pieces(&p);
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!((got[0].1, got[0].2), (1.0, 4.0));
        assert_eq!((got[1].1, got[1].2), (6.0, 9.0));
        assert!((p.duration().as_secs_f64() - 6.0).abs() < 1e-3);
    }

    #[test]
    fn a_sped_up_clip_maps_source_time_to_timeline_time() {
        let (mut p, c) = project(10.0);
        p.set_speed(c, 2.0);
        // 10 s of source at 2x lasts 5 s; source 4..6 is 1 s on the timeline.
        let gone = p.remove_source_ranges(c, &[(4.0, 6.0)]).unwrap();
        assert!((gone.as_secs_f64() - 1.0).abs() < 1e-3);
        assert!((p.duration().as_secs_f64() - 4.0).abs() < 1e-3);
    }

    #[test]
    fn splitting_at_source_times_and_marking_them() {
        let (mut p, c) = project(10.0);
        assert_eq!(p.split_at_source_times(c, &[7.5, 2.5, 5.0, 99.0]), 3);
        let got = pieces(&p);
        assert_eq!(got.len(), 4);
        assert_eq!(got[1].0, 2.5);
        let (mut p, c) = project(10.0);
        assert_eq!(p.add_markers_at_source_times(c, &[1.0, 3.0, 12.0]), 2);
        assert_eq!(p.markers.len(), 2);
        assert_eq!(p.markers[1].time, s(3.0));
    }

    #[test]
    fn clips_are_placed_by_their_sync_offsets() {
        let mut p = Project::new("t");
        let mut m = MediaItem::new(PathBuf::from("/a.mp4"), MediaKind::Video, s(10.0));
        m.has_audio = true;
        let id = p.add_media(m);
        // The main track would pull a clip placed at 5 s back to 0.
        p.set_magnetic(false);
        let t1 = p.main_track().unwrap();
        let a = p.add_clip(id, t1, s(5.0)).unwrap();
        let t2 = p.add_track(crate::project::TrackKind::Video).unwrap();
        let b = p.add_clip(id, t2, Time::ZERO).unwrap();
        // B's sound is 2.5 s behind A's: it starts 2.5 s after A.
        assert_eq!(p.place_synced(a, &[(b, 2.5)]), 2);
        assert_eq!(p.clip(a).unwrap().1.start, s(5.0));
        assert_eq!(p.clip(b).unwrap().1.start, s(7.5));
        // B 8 s before A would be at -3 s (A is at 5 s): both move 3 s later.
        assert_eq!(p.place_synced(a, &[(b, -8.0)]), 2);
        assert_eq!(p.clip(b).unwrap().1.start, Time::ZERO);
        assert_eq!(p.clip(a).unwrap().1.start, s(8.0));
    }

    #[test]
    fn a_locked_track_is_left_alone() {
        let (mut p, c) = project(10.0);
        let t = p.main_track().unwrap();
        p.track_mut(t).unwrap().locked = true;
        assert!(p
            .remove_source_ranges(c, &[(2.0, 3.0)])
            .is_none_or(|t| t == Time::ZERO));
        assert_eq!(pieces(&p).len(), 1);
    }
}
