//! Timeline markers: named, coloured points on the ruler (M key)
//! for finding places again, with an optional note.

use crate::project::Project;
use crate::time::Time;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type MarkerId = Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub id: MarkerId,
    /// Where it sits on the timeline.
    pub time: Time,
    #[serde(default)]
    pub name: String,
    /// Index into the editor's colour labels (the same palette as clip marks).
    #[serde(default)]
    pub color: u8,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Marker {
    /// The colour new markers get (the editor's mint accent).
    pub const DEFAULT_COLOR: u8 = 4;

    pub fn new(time: Time, name: impl Into<String>) -> Self {
        Marker {
            id: Uuid::new_v4(),
            time: time.max(Time::ZERO),
            name: name.into(),
            color: Marker::DEFAULT_COLOR,
            note: String::new(),
        }
    }
}

impl Project {
    fn sort_markers(&mut self) {
        self.markers.sort_by_key(|m| m.time);
    }

    /// Adds a marker at `at`, named "Marker 1", "Marker 2"… Returns its id.
    pub fn add_marker(&mut self, at: Time) -> MarkerId {
        let mut n = 1;
        while self.markers.iter().any(|m| m.name == format!("Marker {n}")) {
            n += 1;
        }
        let marker = Marker::new(at, format!("Marker {n}"));
        let id = marker.id;
        self.markers.push(marker);
        self.sort_markers();
        id
    }

    pub fn marker(&self, id: MarkerId) -> Option<&Marker> {
        self.markers.iter().find(|m| m.id == id)
    }

    pub fn marker_mut(&mut self, id: MarkerId) -> Option<&mut Marker> {
        self.markers.iter_mut().find(|m| m.id == id)
    }

    /// The marker within `tolerance` of `at`, if any.
    pub fn marker_near(&self, at: Time, tolerance: Time) -> Option<&Marker> {
        self.markers
            .iter()
            .filter(|m| (m.time - at).abs() <= tolerance)
            .min_by_key(|m| (m.time - at).abs())
    }

    pub fn remove_marker(&mut self, id: MarkerId) -> bool {
        let n = self.markers.len();
        self.markers.retain(|m| m.id != id);
        self.markers.len() != n
    }

    /// Moves a marker to `to` (never before the start).
    pub fn move_marker(&mut self, id: MarkerId, to: Time) -> bool {
        let Some(m) = self.marker_mut(id) else {
            return false;
        };
        m.time = to.max(Time::ZERO);
        self.sort_markers();
        true
    }

    /// The nearest marker after `from` (or before it, going backwards).
    pub fn next_marker(&self, from: Time, forward: bool) -> Option<&Marker> {
        let eps = Time::from_millis(1);
        if forward {
            self.markers.iter().find(|m| m.time > from + eps)
        } else {
            self.markers.iter().rev().find(|m| m.time < from - eps)
        }
    }

    /// Moves every marker at or after `at` by `delta`, following a ripple
    /// edit on the main track.
    pub(crate) fn shift_markers(&mut self, at: Time, delta: Time) {
        if delta == Time::ZERO {
            return;
        }
        for m in self.markers.iter_mut().filter(|m| m.time >= at) {
            m.time = (m.time + delta).max(Time::ZERO);
        }
        self.sort_markers();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{MediaItem, MediaKind};
    use std::path::PathBuf;

    fn s(secs: f64) -> Time {
        Time::from_secs_f64(secs)
    }

    #[test]
    fn markers_stay_sorted_and_get_unique_names() {
        let mut p = Project::default();
        let b = p.add_marker(s(4.0));
        let a = p.add_marker(s(1.0));
        assert_eq!(p.markers[0].id, a);
        assert_eq!(p.markers[0].name, "Marker 2");
        assert!(p.move_marker(a, s(9.0)));
        assert_eq!(p.markers[0].id, b);
        assert_eq!(p.next_marker(s(4.0), true).map(|m| m.id), Some(a));
        assert_eq!(p.next_marker(s(9.0), false).map(|m| m.id), Some(b));
        assert!(p.next_marker(s(9.0), true).is_none());
        assert!(p.remove_marker(b));
        assert_eq!(p.add_marker(s(0.0)), p.markers[0].id);
        assert_eq!(p.markers[0].name, "Marker 1");
    }

    #[test]
    fn main_track_ripples_move_later_markers() {
        let mut p = Project::default();
        let mut m = MediaItem::new(PathBuf::from("/v.mp4"), MediaKind::Video, s(4.0));
        m.has_audio = true;
        let m = p.add_media(m);
        let first = p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        let early = p.add_marker(s(1.0));
        let late = p.add_marker(s(6.0));
        assert!(p.ripple_delete_clip(first));
        assert_eq!(p.marker(early).unwrap().time, s(1.0));
        assert_eq!(p.marker(late).unwrap().time, s(2.0));
        // An overlay track edit leaves markers alone.
        let v2 = p.add_track(crate::TrackKind::Video).unwrap();
        let t = p
            .add_generated(crate::Generator::Solid([1.0; 4]), s(1.0), v2, s(0.0))
            .unwrap();
        assert!(p.ripple_delete_clip(t));
        assert_eq!(p.marker(late).unwrap().time, s(2.0));
    }

    #[test]
    fn old_projects_load_without_markers() {
        let p = Project::default();
        let mut json: serde_json::Value = serde_json::to_value(&p).unwrap();
        json.as_object_mut().unwrap().remove("markers");
        let loaded: Project = serde_json::from_value(json).unwrap();
        assert!(loaded.markers.is_empty());
    }
}
