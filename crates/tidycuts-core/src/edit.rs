//! Editing operations on a project: adding, moving, trimming, splitting and
//! deleting clips, with a magnetic main track.
//!
//! The main track is the lowest video track. While
//! [`ProjectSettings::magnetic`](crate::ProjectSettings) is on it never has
//! gaps: deleting a clip closes the hole, dropping a clip between two others
//! pushes the rest along, and trimming slides everything after the edit.
//! Clips on other tracks that sit after an edit on the main track move by
//! the same amount so titles, overlays and music stay in sync.

use crate::clip::{Clip, ClipId, Generator};
use crate::keyframe::Animatable;
use crate::project::{MediaId, MediaKind, Project, Track, TrackId, TrackKind};
use crate::speed::SpeedRamp;
use crate::time::Time;
use uuid::Uuid;

/// Which end of a clip a trim moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Start,
    End,
}

/// The shortest a clip can be trimmed to.
pub const MIN_CLIP: Time = Time(40_000);

pub const MIN_SPEED: f64 = 0.1;
pub const MAX_SPEED: f64 = 16.0;

impl Track {
    /// The empty span containing `t` that has a clip after it, if any.
    pub fn gap_at(&self, t: Time) -> Option<(Time, Time)> {
        if self.clip_at(t).is_some() {
            return None;
        }
        let next = self.clips.iter().find(|c| c.start > t)?;
        let prev_end = self
            .clips
            .iter()
            .filter(|c| c.end() <= t)
            .map(Clip::end)
            .max()
            .unwrap_or(Time::ZERO);
        Some((prev_end, next.start))
    }

    /// Whether `start..start+len` is free (ignoring one clip).
    pub fn is_free(&self, start: Time, len: Time, ignore: Option<ClipId>) -> bool {
        self.clips
            .iter()
            .filter(|c| Some(c.id) != ignore)
            .all(|c| c.end() <= start || c.start >= start + len)
    }

    fn sort_clips(&mut self) {
        self.clips.sort_by_key(|c| c.start);
    }

    /// Lays the clips end to end from zero, keeping their order in the list.
    fn pack(&mut self) {
        let mut at = Time::ZERO;
        for c in &mut self.clips {
            c.start = at;
            at = c.end();
        }
    }

    /// Index a clip dropped at `at` takes on a magnetic track: before the
    /// first clip whose middle is after the drop point.
    fn insert_index(&self, at: Time) -> usize {
        self.clips
            .iter()
            .filter(|c| c.start + c.duration().scale(0.5) <= at)
            .count()
    }

    /// Moves every clip starting at or after `at` by `delta`. A move to the
    /// left stops at the end of the clip before them.
    fn shift_from(&mut self, at: Time, delta: Time) {
        if delta == Time::ZERO || self.locked {
            return;
        }
        let Some(first) = self.clips.iter().position(|c| c.start >= at) else {
            return;
        };
        let mut d = delta;
        if d < Time::ZERO {
            let prev_end = self.clips[..first]
                .iter()
                .map(Clip::end)
                .max()
                .unwrap_or(Time::ZERO);
            d = d.max(prev_end - self.clips[first].start);
        }
        for c in &mut self.clips[first..] {
            c.start += d;
        }
    }
}

/// Moves `clip`'s keyframes so each stays on the moment of the source it
/// had in `old`, after a change of speed.
fn follow_timing(old: &Clip, clip: &mut Clip) {
    let new = clip.clone();
    clip.for_each_animatable(|a| {
        a.remap_time(|at| Time::from_secs_f64(new.local_secs(old.source_secs(at.as_secs_f64()))))
    });
}

fn fits(track: TrackKind, media: MediaKind, has_audio: bool) -> bool {
    match track {
        TrackKind::Video => media != MediaKind::Audio,
        TrackKind::Audio => media == MediaKind::Audio || has_audio,
    }
}

impl Project {
    /// The lowest video track ("Video 1"), the main track.
    pub fn main_track(&self) -> Option<TrackId> {
        self.tracks
            .iter()
            .rev()
            .find(|t| t.kind == TrackKind::Video)
            .map(|t| t.id)
    }

    /// Whether clips on this track are kept together with no gaps.
    pub fn is_magnetic(&self, track: TrackId) -> bool {
        self.settings.magnetic && self.main_track() == Some(track)
    }

    /// Turns the magnetic main track on or off, closing its gaps when on.
    pub fn set_magnetic(&mut self, on: bool) {
        self.settings.magnetic = on;
        if on {
            if let Some(main) = self.main_track() {
                if let Some(t) = self.track_mut(main) {
                    t.pack();
                }
            }
        }
    }

    /// The track a media item lands on when it isn't dropped on a specific one.
    pub fn default_track_for(&self, kind: MediaKind) -> Option<TrackId> {
        let want = match kind {
            MediaKind::Audio => TrackKind::Audio,
            MediaKind::Video | MediaKind::Image => TrackKind::Video,
        };
        if want == TrackKind::Video {
            if let Some(main) = self
                .main_track()
                .filter(|m| !self.track(*m).unwrap().locked)
            {
                return Some(main);
            }
        }
        self.tracks
            .iter()
            .find(|t| t.kind == want && !t.locked)
            .or_else(|| self.tracks.iter().find(|t| t.kind == want))
            .map(|t| t.id)
    }

    /// A video track above the main track with room for `len` at `at`,
    /// adding one if every overlay track is busy there. `None` when all
    /// [`crate::MAX_VIDEO_TRACKS`] are in use at that spot.
    pub fn overlay_track_for(&mut self, at: Time, len: Time) -> Option<TrackId> {
        let main = self.main_track();
        let found = self
            .tracks
            .iter()
            .rev()
            .filter(|t| t.kind == TrackKind::Video && Some(t.id) != main && !t.locked)
            .find(|t| t.is_free(at, len, None))
            .map(|t| t.id);
        found.or_else(|| self.add_track(TrackKind::Video))
    }

    /// An audio track with room for `len` at `at`, adding one if needed.
    /// `None` when all [`crate::MAX_AUDIO_TRACKS`] are busy there.
    pub fn audio_track_for(&mut self, at: Time, len: Time) -> Option<TrackId> {
        let found = self
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio && !t.locked)
            .find(|t| t.is_free(at, len, None))
            .map(|t| t.id);
        found.or_else(|| self.add_track(TrackKind::Audio))
    }

    /// Moves clips that start at or after `at` on every other track, and
    /// the markers after it.
    pub(crate) fn ripple_others(&mut self, except: TrackId, at: Time, delta: Time) {
        for t in self.tracks.iter_mut().filter(|t| t.id != except) {
            t.shift_from(at, delta);
        }
        self.shift_markers(at, delta);
    }

    fn pack_track(&mut self, id: TrackId) {
        if let Some(t) = self.track_mut(id) {
            t.pack();
        }
    }

    /// Puts a ready-made clip on a track at (or near) `at`. On the magnetic
    /// main track it's inserted between clips; elsewhere it takes the first
    /// free spot at or after `at`.
    pub fn place_clip(&mut self, mut clip: Clip, track_id: TrackId, at: Time) -> Option<ClipId> {
        let magnetic = self.is_magnetic(track_id);
        let track = self.track_mut(track_id)?;
        if track.locked {
            return None;
        }
        let id = clip.id;
        let len = clip.duration();
        if magnetic {
            let idx = track.insert_index(at);
            let ripple = idx < track.clips.len();
            clip.start = if idx == 0 {
                Time::ZERO
            } else {
                track.clips[idx - 1].end()
            };
            let insert_at = clip.start;
            track.clips.insert(idx, clip);
            track.pack();
            if ripple {
                self.ripple_others(track_id, insert_at, len);
            }
        } else {
            clip.start = track.free_slot(at, len, None);
            track.clips.push(clip);
            track.sort_clips();
        }
        Some(id)
    }

    /// Places media on a track. Returns `None` if the media or track doesn't
    /// exist, the track is locked, or the media type doesn't belong there.
    pub fn add_clip(&mut self, media_id: MediaId, track_id: TrackId, at: Time) -> Option<ClipId> {
        let media = self.media(media_id)?.clone();
        let track = self.track(track_id)?;
        if !fits(track.kind, media.kind, media.has_audio) {
            return None;
        }
        self.place_clip(Clip::new(&media, at), track_id, at)
    }

    /// Places a title, sticker, color or adjustment clip on a video track.
    pub fn add_generated(
        &mut self,
        generator: Generator,
        len: Time,
        track_id: TrackId,
        at: Time,
    ) -> Option<ClipId> {
        if self.track(track_id)?.kind != TrackKind::Video {
            return None;
        }
        self.place_clip(Clip::generated(generator, at, len), track_id, at)
    }

    /// Appends media to the end of its default track.
    pub fn append_clip(&mut self, media_id: MediaId) -> Option<ClipId> {
        let kind = self.media(media_id)?.kind;
        let track_id = self.default_track_for(kind)?;
        let end = self.track(track_id)?.end();
        self.add_clip(media_id, track_id, end)
    }

    /// Moves a clip, possibly to another track of the same kind. On the
    /// magnetic main track the clip is re-inserted between clips; elsewhere
    /// it lands at the first free spot at or after `at`.
    pub fn move_clip(&mut self, clip_id: ClipId, to_track: TrackId, at: Time) -> bool {
        let Some((from, _)) = self.clip(clip_id) else {
            return false;
        };
        let (from_id, from_kind, from_locked) = (from.id, from.kind, from.locked);
        let Some(dest) = self.track(to_track) else {
            return false;
        };
        if dest.kind != from_kind || dest.locked || from_locked {
            return false;
        }
        let from_track = self.track_mut(from_id).unwrap();
        let idx = from_track
            .clips
            .iter()
            .position(|c| c.id == clip_id)
            .unwrap();
        let clip = from_track.clips.remove(idx);
        if self.is_magnetic(from_id) {
            self.pack_track(from_id);
        }
        let magnetic = self.is_magnetic(to_track);
        let dest = self.track_mut(to_track).unwrap();
        if magnetic {
            let idx = dest.insert_index(at);
            dest.clips.insert(idx, clip);
            dest.pack();
        } else {
            let mut clip = clip;
            clip.start = dest.free_slot(at, clip.duration(), None);
            dest.clips.push(clip);
            dest.sort_clips();
        }
        true
    }

    /// The other clips linked to this one (a video and its detached audio).
    pub fn linked_clips(&self, clip_id: ClipId) -> Vec<ClipId> {
        let Some(link) = self.clip(clip_id).and_then(|(_, c)| c.link) else {
            return Vec::new();
        };
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.link == Some(link) && c.id != clip_id)
            .map(|c| c.id)
            .collect()
    }

    /// Cuts the link between a clip and the clips it was linked to, so each
    /// can be edited on its own.
    pub fn unlink_clip(&mut self, clip_id: ClipId) -> bool {
        let group = self.linked_clips(clip_id);
        let had = self.clip(clip_id).is_some_and(|(_, c)| c.link.is_some());
        for id in group.iter().chain(std::iter::once(&clip_id)) {
            if let Some(c) = self.clip_mut(*id) {
                c.link = None;
            }
        }
        had
    }

    /// Splits a clip in two at timeline time `at`, and the clips linked to it
    /// at the same moment. Returns the new right half of `clip_id`.
    pub fn split_clip(&mut self, clip_id: ClipId, at: Time) -> Option<ClipId> {
        let others = self.linked_clips(clip_id);
        let right = self.split_one(clip_id, at)?;
        if !others.is_empty() {
            // Each side keeps its own pair: the right halves get a new link.
            let new_link = Uuid::new_v4();
            let mut any = false;
            for other in others {
                if let Some(r) = self.split_one(other, at) {
                    if let Some(c) = self.clip_mut(r) {
                        c.link = Some(new_link);
                    }
                    any = true;
                }
            }
            if let Some(c) = self.clip_mut(right) {
                c.link = any.then_some(new_link);
            }
        }
        Some(right)
    }

    fn split_one(&mut self, clip_id: ClipId, at: Time) -> Option<ClipId> {
        let track_id = self.clip(clip_id)?.0.id;
        let track = self.track_mut(track_id)?;
        if track.locked {
            return None;
        }
        let clip = track.clips.iter_mut().find(|c| c.id == clip_id)?;
        if at < clip.start + MIN_CLIP || at > clip.end() - MIN_CLIP {
            return None;
        }
        let offset = at - clip.start;
        let total = clip.duration();
        let cut = clip.source_time(at);
        let mut right = clip.clone();
        right.id = Uuid::new_v4();
        if clip.reverse {
            clip.source_in = cut;
            right.source_out = cut;
        } else {
            clip.source_out = cut;
            right.source_in = cut;
        }
        right.start = clip.end();
        clip.for_each_animatable(|a| a.rebase(Time::ZERO, offset));
        right.for_each_animatable(|a| a.rebase(offset, total - offset));
        // The cut gets no transition; the outer ends keep theirs.
        clip.transition_out = None;
        right.transition_in = None;
        clip.anim_out = None;
        right.anim_in = None;
        clip.audio.fade_out = Time::ZERO;
        right.audio.fade_in = Time::ZERO;
        for e in &mut right.effects {
            e.id = Uuid::new_v4();
        }
        let id = right.id;
        track.clips.push(right);
        track.sort_clips();
        Some(id)
    }

    /// Deletes a clip, and the clips linked to it. On the magnetic main track
    /// the gap closes and later clips on other tracks follow.
    pub fn delete_clip(&mut self, clip_id: ClipId) -> bool {
        self.delete_group(clip_id, false)
    }

    fn remove_clip(&mut self, clip_id: ClipId) -> Option<(TrackId, Clip)> {
        for t in &mut self.tracks {
            if let Some(i) = t.clips.iter().position(|c| c.id == clip_id) {
                if t.locked {
                    return None;
                }
                return Some((t.id, t.clips.remove(i)));
            }
        }
        None
    }

    /// Deletes a clip (and its linked clips) and closes the gap it leaves
    /// (Shift+Delete). On the main track, later clips on other tracks move
    /// too.
    pub fn ripple_delete_clip(&mut self, clip_id: ClipId) -> bool {
        self.delete_group(clip_id, true)
    }

    /// Removes a clip and the clips linked to it. With `ripple`, or on the
    /// magnetic main track, the gaps close.
    fn delete_group(&mut self, clip_id: ClipId, ripple: bool) -> bool {
        let others = self.linked_clips(clip_id);
        let Some(first) = self.remove_clip(clip_id) else {
            return false;
        };
        let mut removed = vec![first];
        for other in others {
            if let Some(x) = self.remove_clip(other) {
                removed.push(x);
            }
        }
        let main = self.main_track();
        let closes = |p: &Project, t: TrackId| ripple || p.is_magnetic(t);
        // When the main track closes its gap, every other track follows, so
        // the others need no gap closing of their own.
        let main_closes = removed
            .iter()
            .any(|(t, _)| Some(*t) == main && closes(self, *t));
        for (track_id, clip) in &removed {
            let len = clip.duration();
            if self.is_magnetic(*track_id) {
                self.pack_track(*track_id);
            } else if ripple && !(main_closes && Some(*track_id) != main) {
                if let Some(t) = self.track_mut(*track_id) {
                    t.shift_from(clip.end(), -len);
                }
            }
            if Some(*track_id) == main && closes(self, *track_id) {
                self.ripple_others(*track_id, clip.end(), -len);
            }
        }
        true
    }

    /// Closes the empty space at `at` on a track by pulling later clips left.
    pub fn delete_gap(&mut self, track_id: TrackId, at: Time) -> bool {
        let Some(track) = self.track(track_id) else {
            return false;
        };
        let Some((a, b)) = track.gap_at(at) else {
            return false;
        };
        if track.locked {
            return false;
        }
        self.track_mut(track_id).unwrap().shift_from(b, a - b);
        if self.main_track() == Some(track_id) {
            self.ripple_others(track_id, b, a - b);
        }
        true
    }

    /// Moves one edge of a clip to timeline time `to`, within what the
    /// source and neighbouring clips allow. On the magnetic main track the
    /// clip keeps its place and the clips after it slide instead.
    /// Returns how far the edge actually moved.
    pub fn trim_clip(&mut self, clip_id: ClipId, edge: Edge, to: Time) -> Option<Time> {
        let (track, clip) = self.clip(clip_id)?;
        if track.locked {
            return None;
        }
        let track_id = track.id;
        let magnetic = self.is_magnetic(track_id);
        let media = clip.media_id.and_then(|m| self.media(m));
        let fixed = clip.has_fixed_length(media);
        let media_len = media.map(|m| m.duration).unwrap_or(Time::ZERO);
        let (start, end, reverse) = (clip.start, clip.end(), clip.reverse);
        let (src_in, src_out) = (clip.source_in, clip.source_out);
        let prev_end = track
            .clips
            .iter()
            .filter(|c| c.id != clip_id && c.end() <= start)
            .map(Clip::end)
            .max()
            .unwrap_or(Time::ZERO);
        let next_start = track
            .clips
            .iter()
            .filter(|c| c.id != clip_id && c.start >= end)
            .map(|c| c.start)
            .min();

        let delta = match edge {
            Edge::Start => {
                let mut new_start = to.min(end - MIN_CLIP);
                if fixed {
                    let room = if reverse {
                        clip.play_time(src_out, media_len)
                    } else {
                        clip.play_time(Time::ZERO, src_in)
                    };
                    new_start = new_start.max(start - room);
                }
                if !magnetic {
                    new_start = new_start.max(prev_end);
                }
                new_start - start
            }
            Edge::End => {
                let mut new_end = to.max(start + MIN_CLIP);
                if fixed {
                    let room = if reverse {
                        clip.play_time(Time::ZERO, src_in)
                    } else {
                        clip.play_time(src_out, media_len)
                    };
                    new_end = new_end.min(end + room);
                }
                if !magnetic {
                    if let Some(n) = next_start {
                        new_end = new_end.min(n);
                    }
                }
                new_end - end
            }
        };
        if delta == Time::ZERO {
            return Some(Time::ZERO);
        }
        let clip = self.clip_mut(clip_id)?;
        // Rounding must not carry a fixed-length source past its ends.
        let keep = |t: Time| {
            if fixed {
                t.max(Time::ZERO).min(media_len)
            } else {
                t
            }
        };
        match edge {
            Edge::Start => {
                if reverse {
                    clip.source_out = keep(clip.source_step(src_out, delta));
                } else {
                    clip.source_in = keep(clip.source_step(src_in, delta));
                }
                let len = clip.duration();
                clip.for_each_animatable(|a| a.rebase(delta, len));
                if !magnetic {
                    clip.start += delta;
                }
            }
            Edge::End => {
                if reverse {
                    clip.source_in = keep(clip.source_step(src_in, delta));
                } else {
                    clip.source_out = keep(clip.source_step(src_out, delta));
                }
                let len = clip.duration();
                clip.for_each_animatable(|a| a.rebase(Time::ZERO, len));
            }
        }
        if magnetic {
            let change = match edge {
                Edge::Start => -delta,
                Edge::End => delta,
            };
            self.pack_track(track_id);
            self.ripple_others(track_id, end, change);
        }
        Some(delta)
    }

    /// Changes a video or audio clip's playback speed; the clip gets
    /// shorter or longer and later clips on its track move to make room.
    /// A speed ramp on the clip is removed.
    pub fn set_speed(&mut self, clip_id: ClipId, speed: f64) -> bool {
        let speed = speed.clamp(MIN_SPEED, MAX_SPEED);
        self.retime_clip(clip_id, |clip| {
            if clip.ramp.is_none() {
                let old_speed = clip.speed;
                clip.speed = speed;
                clip.for_each_animatable(|a| a.scale_time(old_speed / speed));
            } else {
                let old = clip.clone();
                clip.speed = speed;
                clip.ramp = None;
                follow_timing(&old, clip);
            }
        })
    }

    /// Gives a video or audio clip a speed ramp (or removes it with `None`).
    /// The clip's length follows the curve, keyframes stay on the same
    /// moments of the source, and later clips on its track move.
    pub fn set_speed_ramp(&mut self, clip_id: ClipId, ramp: Option<SpeedRamp>) -> bool {
        self.retime_clip(clip_id, |clip| {
            let old = clip.clone();
            clip.speed = 1.0;
            clip.ramp = ramp.map(|mut r| {
                r.tidy();
                r
            });
            follow_timing(&old, clip);
        })
    }

    /// Applies a change to a clip's timing, then makes room for (or closes
    /// up behind) its new length.
    fn retime_clip(&mut self, clip_id: ClipId, change: impl FnOnce(&mut Clip)) -> bool {
        let Some((track, clip)) = self.clip(clip_id) else {
            return false;
        };
        if track.locked || !clip.has_fixed_length(clip.media_id.and_then(|m| self.media(m))) {
            return false;
        }
        let track_id = track.id;
        let old_end = clip.end();
        let magnetic = self.is_magnetic(track_id);
        let clip = self.clip_mut(clip_id).unwrap();
        change(clip);
        let new_end = clip.end();
        let delta = new_end - old_end;
        if magnetic {
            self.pack_track(track_id);
            self.ripple_others(track_id, old_end, delta);
        } else if delta != Time::ZERO {
            // Later clips follow the end in both directions, so slower then
            // faster then 1x puts everything back where it was.
            let t = self.track_mut(track_id).unwrap();
            for c in t
                .clips
                .iter_mut()
                .filter(|c| c.id != clip_id && c.start >= old_end)
            {
                c.start += delta;
            }
        }
        true
    }

    /// Changes how long an image or generated clip lasts.
    pub fn set_duration(&mut self, clip_id: ClipId, len: Time) -> bool {
        let Some((_, clip)) = self.clip(clip_id) else {
            return false;
        };
        let end = clip.start + len.max(MIN_CLIP);
        self.trim_clip(clip_id, Edge::End, end).is_some()
    }

    /// Copies a clip and places the copy right after it.
    pub fn duplicate_clip(&mut self, clip_id: ClipId) -> Option<ClipId> {
        let (track, clip) = self.clip(clip_id)?;
        let track_id = track.id;
        let mut copy = clip.clone();
        copy.id = Uuid::new_v4();
        copy.link = None;
        for e in &mut copy.effects {
            e.id = Uuid::new_v4();
        }
        let at = clip.end();
        self.place_clip(copy, track_id, at)
    }

    /// Moves a video clip's sound to its own clip on an audio track and
    /// silences the original, so the two can be edited separately.
    pub fn detach_audio(&mut self, clip_id: ClipId) -> Option<ClipId> {
        let (track, clip) = self.clip(clip_id)?;
        let media = self.media(clip.media_id?)?;
        if track.kind != TrackKind::Video || !media.has_audio || clip.audio.muted {
            return None;
        }
        let mut sound = clip.clone();
        sound.id = Uuid::new_v4();
        // The sound stays tied to its picture for cutting and deleting.
        let link = clip.link.unwrap_or_else(Uuid::new_v4);
        sound.link = Some(link);
        sound.effects.clear();
        sound.transition_in = None;
        sound.transition_out = None;
        sound.anim_in = None;
        sound.anim_out = None;
        let (at, len) = (clip.start, clip.duration());
        let audio_track = self.audio_track_for(at, len)?;
        let id = sound.id;
        let t = self.track_mut(audio_track)?;
        t.clips.push(sound);
        t.sort_clips();
        let original = self.clip_mut(clip_id)?;
        original.audio.muted = true;
        original.link = Some(link);
        Some(id)
    }
}

impl Project {
    /// Rotates a clip a quarter turn clockwise (keyframes included).
    pub fn rotate_clip_90(&mut self, clip_id: ClipId) -> bool {
        let Some(c) = self.clip_mut(clip_id) else {
            return false;
        };
        c.transform.rotation.offset(90.0);
        let r = c.transform.rotation.value.rem_euclid(360.0);
        if !c.transform.rotation.is_animated() {
            c.transform.rotation.value = r;
        }
        true
    }

    /// Scales and centres a clip so its visible (cropped) part fills the
    /// whole frame with no bars ("Crop to Fit").
    ///
    /// This follows the renderer: 100% scale is the uncropped picture
    /// fitted inside the frame, and a crop keeps the remaining part where it
    /// was, so an off-centre crop also needs moving back to the centre.
    pub fn crop_to_fit(&mut self, clip_id: ClipId) -> bool {
        let (pw, ph) = (self.settings.width as f64, self.settings.height as f64);
        let Some((_, clip)) = self.clip(clip_id) else {
            return false;
        };
        let Some(media) = clip.media_id.and_then(|m| self.media(m)) else {
            return false;
        };
        let (mw, mh) = (media.width.max(1) as f64, media.height.max(1) as f64);
        let fit = (pw / mw).min(ph / mh);
        let base = (mw * fit, mh * fit);
        let crop = clip.crop.clamped();
        let (w0, h0) = (
            (1.0 - crop.left - crop.right).max(0.01),
            (1.0 - crop.top - crop.bottom).max(0.01),
        );
        let (mut vw, mut vh) = (base.0 * w0, base.1 * h0);
        let rot = clip.transform.rotation.value;
        if (rot.rem_euclid(180.0) - 90.0).abs() < 1.0 {
            std::mem::swap(&mut vw, &mut vh);
        }
        let scale = (pw / vw).max(ph / vh);
        // Where the visible part's centre sits relative to the picture's
        // centre, then turned and scaled like the renderer does.
        let offset = (
            (crop.left + w0 * 0.5 - 0.5) * base.0,
            (crop.top + h0 * 0.5 - 0.5) * base.1,
        );
        let anchor = clip.video.anchor;
        let rel = (
            (offset.0 - anchor[0]) * scale,
            (offset.1 - anchor[1]) * scale,
        );
        let (sin, cos) = rot.to_radians().sin_cos();
        let turned = (rel.0 * cos - rel.1 * sin, rel.0 * sin + rel.1 * cos);
        let c = self.clip_mut(clip_id).unwrap();
        c.transform.scale_x = Animatable::constant(scale * 100.0);
        c.transform.scale_y = Animatable::constant(scale * 100.0);
        c.transform.position_x = Animatable::constant(-(anchor[0] + turned.0));
        c.transform.position_y = Animatable::constant(-(anchor[1] + turned.1));
        true
    }

    /// Swaps the media a clip plays while keeping its place, length (as far
    /// as the new media allows) and every setting.
    pub fn replace_clip_media(&mut self, clip_id: ClipId, media_id: MediaId) -> bool {
        let Some(media) = self.media(media_id).cloned() else {
            return false;
        };
        let Some((track, clip)) = self.clip(clip_id) else {
            return false;
        };
        if !fits(track.kind, media.kind, media.has_audio) || clip.is_generated() {
            return false;
        }
        let track_id = track.id;
        let old_len = clip.duration();
        let magnetic = self.is_magnetic(track_id);
        let fixed = media.kind != MediaKind::Image;
        let c = self.clip_mut(clip_id).unwrap();
        let want = c.source_len();
        c.media_id = Some(media_id);
        c.source_in = Time::ZERO;
        c.source_out = if fixed {
            want.min(media.duration)
        } else {
            want
        };
        let delta = c.duration() - old_len;
        let end = c.start + old_len;
        if delta != Time::ZERO && magnetic {
            if let Some(t) = self.track_mut(track_id) {
                t.pack();
            }
            self.ripple_others(track_id, end, delta);
        }
        true
    }

    /// Puts media into an empty split-screen cell: the placeholder becomes
    /// an ordinary clip of the media, covering the cell, as long as the
    /// cell was (or as the media, when that's shorter).
    pub fn fill_cell(&mut self, clip_id: ClipId, media_id: MediaId) -> bool {
        let Some(media) = self.media(media_id).cloned() else {
            return false;
        };
        let Some((track, clip)) = self.clip(clip_id) else {
            return false;
        };
        if track.locked
            || !clip.is_placeholder()
            || !fits(track.kind, media.kind, media.has_audio)
            || track.kind != TrackKind::Video
        {
            return false;
        }
        let track_id = track.id;
        let old_len = clip.duration();
        let magnetic = self.is_magnetic(track_id);
        let c = self.clip_mut(clip_id).unwrap();
        c.generator = None;
        c.media_id = Some(media_id);
        c.name = None;
        c.speed = 1.0;
        c.source_in = Time::ZERO;
        c.source_out = if media.kind == MediaKind::Image {
            old_len
        } else {
            old_len.min(media.duration)
        };
        let delta = c.duration() - old_len;
        let end = c.start + old_len;
        if delta != Time::ZERO && magnetic {
            if let Some(t) = self.track_mut(track_id) {
                t.pack();
            }
            self.ripple_others(track_id, end, delta);
        }
        true
    }

    /// Pastes copies of `clips` (keeping their spacing) with the earliest
    /// one starting at `at`. Video clips go to `video_track` (or overlay
    /// tracks when it's busy), audio clips to free audio tracks. Returns the
    /// new clip ids.
    pub fn paste_clips(
        &mut self,
        clips: &[(TrackKind, Clip)],
        video_track: Option<TrackId>,
        at: Time,
    ) -> Vec<ClipId> {
        let Some(first) = clips.iter().map(|(_, c)| c.start).min() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        // A link survives only when its whole pair is pasted together, under
        // a new link of its own.
        let mut links: std::collections::HashMap<Uuid, (usize, Uuid)> = Default::default();
        for (_, c) in clips {
            if let Some(l) = c.link {
                links.entry(l).or_insert((0, Uuid::new_v4())).0 += 1;
            }
        }
        for (kind, clip) in clips {
            let mut c = clip.clone();
            c.id = Uuid::new_v4();
            c.link = clip
                .link
                .and_then(|l| links.get(&l))
                .filter(|(n, _)| *n > 1)
                .map(|(_, new)| *new);
            for e in &mut c.effects {
                e.id = Uuid::new_v4();
            }
            let start = at + (clip.start - first);
            c.start = start;
            let len = c.duration();
            let track = match kind {
                TrackKind::Video => video_track
                    .filter(|t| self.track(*t).is_some_and(|tr| tr.kind == TrackKind::Video))
                    .or_else(|| self.overlay_track_for(start, len)),
                TrackKind::Audio => self.audio_track_for(start, len),
            };
            if let Some(track) = track {
                if let Some(id) = self.place_clip(c, track, start) {
                    out.push(id);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::MediaItem;
    use crate::speed::RampPreset;
    use std::path::PathBuf;

    fn video(secs: f64) -> MediaItem {
        let mut m = MediaItem::new(
            PathBuf::from("/clips/a.mp4"),
            MediaKind::Video,
            Time::from_secs_f64(secs),
        );
        m.has_audio = true;
        m
    }

    fn s(secs: f64) -> Time {
        Time::from_secs_f64(secs)
    }

    fn starts(p: &Project, track: TrackId) -> Vec<f64> {
        p.track(track)
            .unwrap()
            .clips
            .iter()
            .map(|c| c.start.as_secs_f64())
            .collect()
    }

    #[test]
    fn appending_places_clips_back_to_back() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let a = p.append_clip(m).unwrap();
        let b = p.append_clip(m).unwrap();
        assert_eq!(p.clip(a).unwrap().1.start, s(0.0));
        assert_eq!(p.clip(b).unwrap().1.start, s(4.0));
        assert_eq!(p.duration(), s(8.0));
    }

    #[test]
    fn overlay_tracks_never_overlap() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let v2 = p.add_track(TrackKind::Video).unwrap();
        p.add_clip(m, v2, s(0.0)).unwrap();
        let b = p.add_clip(m, v2, s(2.0)).unwrap();
        assert_eq!(p.clip(b).unwrap().1.start, s(4.0));
    }

    #[test]
    fn main_track_closes_gaps_after_cutting_a_piece_out() {
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        let middle = p.split_clip(a, s(3.0)).unwrap();
        let last = p.split_clip(middle, s(6.0)).unwrap();
        assert!(p.delete_clip(middle));
        assert_eq!(starts(&p, main), vec![0.0, 3.0]);
        assert_eq!(p.clip(last).unwrap().1.source_in, s(6.0));
        assert_eq!(p.duration(), s(7.0));
    }

    #[test]
    fn other_tracks_follow_main_track_edits() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let main = p.main_track().unwrap();
        let first = p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let title = p
            .add_generated(Generator::Solid([1.0; 4]), s(1.0), v2, s(5.0))
            .unwrap();
        // Something spanning the deleted clip stays put.
        let early = p
            .add_generated(Generator::Solid([1.0; 4]), s(1.0), v2, s(0.5))
            .unwrap();
        assert!(p.delete_clip(first));
        assert_eq!(starts(&p, main), vec![0.0]);
        assert_eq!(p.clip(title).unwrap().1.start, s(1.5));
        assert_eq!(p.clip(early).unwrap().1.start, s(0.5));
    }

    #[test]
    fn dropping_between_clips_inserts_on_the_main_track() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let main = p.main_track().unwrap();
        p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        let mid = p.add_clip(m, main, s(3.5)).unwrap();
        assert_eq!(p.clip(mid).unwrap().1.start, s(4.0));
        assert_eq!(starts(&p, main), vec![0.0, 4.0, 8.0]);
        // Dropping far past the end appends without a gap.
        let end = p.add_clip(m, main, s(100.0)).unwrap();
        assert_eq!(p.clip(end).unwrap().1.start, s(12.0));
    }

    #[test]
    fn moving_on_the_main_track_reorders() {
        let mut p = Project::default();
        let m = p.add_media(video(2.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        let b = p.append_clip(m).unwrap();
        let c = p.append_clip(m).unwrap();
        assert!(p.move_clip(a, main, s(5.5)));
        let order: Vec<ClipId> = p.track(main).unwrap().clips.iter().map(|c| c.id).collect();
        assert_eq!(order, vec![b, c, a]);
        assert_eq!(starts(&p, main), vec![0.0, 2.0, 4.0]);
    }

    #[test]
    fn moving_off_the_main_track_closes_the_gap() {
        let mut p = Project::default();
        let m = p.add_media(video(2.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        assert!(p.move_clip(a, v2, s(1.0)));
        assert_eq!(starts(&p, main), vec![0.0]);
        assert_eq!(p.clip(a).unwrap().1.start, s(1.0));
        let audio = p.tracks.last().unwrap().id;
        assert!(!p.move_clip(a, audio, s(0.0)));
    }

    #[test]
    fn non_magnetic_main_track_keeps_gaps_until_deleted() {
        let mut p = Project::default();
        p.settings.magnetic = false;
        let m = p.add_media(video(2.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        p.add_clip(m, main, s(5.0)).unwrap();
        assert!(p.delete_clip(a));
        assert_eq!(starts(&p, main), vec![5.0]);
        assert!(p.delete_gap(main, s(1.0)));
        assert_eq!(starts(&p, main), vec![0.0]);
        p.add_clip(m, main, s(6.0)).unwrap();
        p.set_magnetic(true);
        assert_eq!(starts(&p, main), vec![0.0, 2.0]);
    }

    #[test]
    fn split_keeps_source_continuous() {
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let a = p.append_clip(m).unwrap();
        let b = p.split_clip(a, s(3.0)).unwrap();
        let (_, left) = p.clip(a).unwrap();
        let (_, right) = p.clip(b).unwrap();
        assert_eq!(left.source_out, s(3.0));
        assert_eq!(right.source_in, s(3.0));
        assert_eq!(right.start, s(3.0));
        assert_eq!(right.end(), s(10.0));
        assert!(p.split_clip(a, s(0.0)).is_none());
    }

    #[test]
    fn split_reversed_and_sped_up_clips() {
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let a = p.append_clip(m).unwrap();
        assert!(p.set_speed(a, 2.0));
        p.clip_mut(a).unwrap().reverse = true;
        assert_eq!(p.clip(a).unwrap().1.duration(), s(5.0));
        let b = p.split_clip(a, s(1.0)).unwrap();
        let (_, left) = p.clip(a).unwrap();
        let (_, right) = p.clip(b).unwrap();
        // Reversed: the left part plays the end of the source.
        assert_eq!((left.source_in, left.source_out), (s(8.0), s(10.0)));
        assert_eq!((right.source_in, right.source_out), (s(0.0), s(8.0)));
        assert_eq!(right.start, s(1.0));
        assert_eq!(right.duration(), s(4.0));
    }

    #[test]
    fn split_carries_keyframes_into_both_halves() {
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let a = p.append_clip(m).unwrap();
        {
            let op = &mut p.clip_mut(a).unwrap().transform.opacity;
            op.set_keyframe(s(0.0), 0.0);
            op.set_keyframe(s(10.0), 100.0);
        }
        let b = p.split_clip(a, s(4.0)).unwrap();
        let right = &p.clip(b).unwrap().1.transform.opacity;
        assert!((right.value_at(s(0.0)) - 40.0).abs() < 1e-6);
        assert!((right.value_at(s(6.0)) - 100.0).abs() < 1e-6);
    }

    #[test]
    fn trimming_respects_source_and_neighbours() {
        let mut p = Project::default();
        p.settings.magnetic = false;
        let m = p.add_media(video(10.0));
        let main = p.main_track().unwrap();
        let a = p.add_clip(m, main, s(0.0)).unwrap();
        let b = p.add_clip(m, main, s(12.0)).unwrap();
        // Can't extend past the source.
        p.trim_clip(a, Edge::End, s(11.0)).unwrap();
        assert_eq!(p.clip(a).unwrap().1.end(), s(10.0));
        p.trim_clip(a, Edge::End, s(6.0)).unwrap();
        assert_eq!(p.clip(a).unwrap().1.source_out, s(6.0));
        // Trim the start of b; can't overlap a.
        p.trim_clip(b, Edge::Start, s(14.0)).unwrap();
        let (_, cb) = p.clip(b).unwrap();
        assert_eq!((cb.start, cb.source_in), (s(14.0), s(2.0)));
        p.trim_clip(b, Edge::Start, s(1.0)).unwrap();
        assert_eq!(p.clip(b).unwrap().1.start, s(12.0));
    }

    #[test]
    fn magnetic_trim_slides_following_clips() {
        let mut p = Project::default();
        let m = p.add_media(video(5.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        p.trim_clip(a, Edge::End, s(3.0)).unwrap();
        assert_eq!(starts(&p, main), vec![0.0, 3.0]);
        p.trim_clip(a, Edge::Start, s(1.0)).unwrap();
        let (_, ca) = p.clip(a).unwrap();
        assert_eq!(
            (ca.start, ca.source_in, ca.source_out),
            (s(0.0), s(1.0), s(3.0))
        );
        assert_eq!(starts(&p, main), vec![0.0, 2.0]);
        // Extending the start back pushes the rest right.
        p.trim_clip(a, Edge::Start, s(-1.0)).unwrap();
        assert_eq!(starts(&p, main), vec![0.0, 3.0]);
    }

    #[test]
    fn images_and_titles_stretch_freely() {
        let mut p = Project::default();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let t = p
            .add_generated(Generator::Title(Default::default()), s(5.0), v2, s(0.0))
            .unwrap();
        p.trim_clip(t, Edge::End, s(20.0)).unwrap();
        assert_eq!(p.clip(t).unwrap().1.duration(), s(20.0));
        assert!(!p.set_speed(t, 2.0));
    }

    #[test]
    fn speed_changes_ripple_the_main_track() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let main = p.main_track().unwrap();
        let a = p.append_clip(m).unwrap();
        p.append_clip(m).unwrap();
        assert!(p.set_speed(a, 0.5));
        assert_eq!(starts(&p, main), vec![0.0, 8.0]);
        assert!(p.set_speed(a, 4.0));
        assert_eq!(starts(&p, main), vec![0.0, 1.0]);
        assert_eq!(p.clip(a).unwrap().1.source_time(s(0.5)), s(2.0));
    }

    #[test]
    fn going_slower_then_faster_then_back_to_1x_restores_the_clip() {
        let presets = [0.1, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 4.0, 8.0, 16.0];
        for first in presets {
            for second in presets {
                let mut p = Project::default();
                let m = p.add_media(video(230.83));
                let main = p.main_track().unwrap();
                let a = p.append_clip(m).unwrap();
                p.append_clip(m).unwrap();
                let before = starts(&p, main);
                let (len, src) = {
                    let c = p.clip(a).unwrap().1;
                    (c.duration(), (c.source_in, c.source_out))
                };
                assert!(p.set_speed(a, first));
                assert!(p.set_speed(a, second));
                assert!(p.set_speed(a, 1.0));
                let c = p.clip(a).unwrap().1;
                assert_eq!(c.speed, 1.0, "{first} {second}");
                assert_eq!(c.duration(), len, "{first} {second}");
                assert_eq!((c.source_in, c.source_out), src, "{first} {second}");
                assert_eq!(starts(&p, main), before, "{first} {second}");
            }
        }
    }

    #[test]
    fn a_free_track_clip_keeps_its_neighbours_in_step_with_its_speed() {
        // On an ordinary (non-magnetic) track a clip that gets shorter leaves
        // a gap; getting longer again must not push the next clip any further
        // than where it started.
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let item = p.media(m).unwrap().clone();
        let a = p.place_clip(Clip::new(&item, s(0.0)), v2, s(0.0)).unwrap();
        let b = p.place_clip(Clip::new(&item, s(0.0)), v2, s(10.0)).unwrap();
        assert!(p.set_speed(a, 0.5));
        assert!(p.set_speed(a, 2.0));
        assert!(p.set_speed(a, 1.0));
        assert_eq!(p.clip(a).unwrap().1.duration(), s(10.0));
        assert_eq!(p.clip(b).unwrap().1.start, s(10.0));
    }

    fn ramped(p: &mut Project, secs: f64, preset: RampPreset) -> ClipId {
        let m = p.add_media(video(secs));
        let a = p.append_clip(m).unwrap();
        let ramp = SpeedRamp::preset(preset, s(0.0), s(secs));
        assert!(p.set_speed_ramp(a, Some(ramp)));
        a
    }

    #[test]
    fn speed_ramps_set_the_length_and_map_time_monotonically() {
        let mut p = Project::default();
        let a = ramped(&mut p, 10.0, RampPreset::BulletTime);
        let c = p.clip(a).unwrap().1.clone();
        let r = c.ramp.as_ref().unwrap();
        // The length is the integral of 1 / speed over the source.
        let want = r.play_secs(0.0, 10.0);
        assert!((c.duration().as_secs_f64() - want).abs() < 1e-5);
        assert!(c.duration() > s(10.0));
        assert_eq!(c.source_time(c.start), s(0.0));
        assert!((c.source_time(c.end()) - s(10.0)).abs() <= Time(2));
        let mut last = Time(-1);
        for i in 0..=400 {
            let t = c.start + c.duration().scale(i as f64 / 400.0);
            let src = c.source_time(t);
            assert!(src > last);
            last = src;
            // And back again.
            let back = c.local_secs(src.as_secs_f64());
            assert!((back - (t - c.start).as_secs_f64()).abs() < 1e-5);
        }
        // The slow middle plays at 0.15x.
        let mid = c.local_secs(5.0);
        assert!((c.speed_at(mid) - 0.15).abs() < 1e-9);
    }

    #[test]
    fn speed_ramps_ripple_and_go_back_to_uniform_speed() {
        let mut p = Project::default();
        let main = p.main_track().unwrap();
        let a = ramped(&mut p, 4.0, RampPreset::FlashIn);
        let m = p.clip(a).unwrap().1.media_id.unwrap();
        p.append_clip(m).unwrap();
        let len = p.clip(a).unwrap().1.duration().as_secs_f64();
        assert!(len < 4.0);
        assert_eq!(starts(&p, main), vec![0.0, len]);
        assert!(p.set_speed(a, 2.0));
        assert!(p.clip(a).unwrap().1.ramp.is_none());
        assert_eq!(starts(&p, main), vec![0.0, 2.0]);
        assert!(p.set_speed_ramp(a, None));
        assert_eq!(starts(&p, main), vec![0.0, 4.0]);
    }

    #[test]
    fn splitting_a_ramped_clip_keeps_the_curve_continuous() {
        for reverse in [false, true] {
            let mut p = Project::default();
            let a = ramped(&mut p, 10.0, RampPreset::Montage);
            p.clip_mut(a).unwrap().reverse = reverse;
            let whole = p.clip(a).unwrap().1.clone();
            let cut = whole.start + whole.duration().scale(0.37);
            let b = p.split_clip(a, cut).unwrap();
            let left = p.clip(a).unwrap().1.clone();
            let right = p.clip(b).unwrap().1.clone();
            // The halves add up to the whole and meet at the cut.
            assert!((left.end() - cut).abs() <= Time(2));
            assert!((right.end() - whole.end()).abs() <= Time(3));
            for i in 0..=50 {
                let t = whole.start + whole.duration().scale(i as f64 / 50.0);
                let half = if t < right.start { &left } else { &right };
                let d = (half.source_time(t) - whole.source_time(t)).abs();
                assert!(d <= Time(10), "reverse {reverse}: {d:?} at {t:?}");
            }
            // The speed doesn't jump at the cut.
            let l = left.speed_at(left.duration().as_secs_f64());
            let r = right.speed_at(0.0);
            assert!((l - r).abs() < 1e-3, "{l} vs {r}");
        }
    }

    #[test]
    fn trimming_a_ramped_clip_follows_the_curve() {
        let mut p = Project::default();
        p.settings.magnetic = false;
        let a = ramped(&mut p, 10.0, RampPreset::HeroMoment);
        let whole = p.clip(a).unwrap().1.clone();
        let at = whole.start + whole.duration().scale(0.5);
        let src = whole.source_time(at);
        p.trim_clip(a, Edge::Start, at).unwrap();
        let c = p.clip(a).unwrap().1.clone();
        // The picture at each moment stays the same after the trim.
        assert!((c.source_in - src).abs() <= Time(2));
        assert!((c.end() - whole.end()).abs() <= Time(3));
        for i in 0..=20 {
            let t = c.start + c.duration().scale(i as f64 / 20.0);
            assert!((c.source_time(t) - whole.source_time(t)).abs() <= Time(10));
        }
        // Dragging the start back out stops at the start of the source.
        p.trim_clip(a, Edge::Start, s(-5.0)).unwrap();
        let c = p.clip(a).unwrap().1.clone();
        assert_eq!(c.source_in, s(0.0));
        assert!((c.duration() - whole.duration()).abs() <= Time(3));
        // Trimming the end of the source short and back.
        let end = c.end();
        p.trim_clip(a, Edge::End, end - s(1.0)).unwrap();
        let c2 = p.clip(a).unwrap().1.clone();
        assert!((c2.end() - (end - s(1.0))).abs() <= Time(2));
        assert!((c2.source_out - whole.source_time(end - s(1.0))).abs() <= Time(10));
        p.trim_clip(a, Edge::End, end + s(5.0)).unwrap();
        assert_eq!(p.clip(a).unwrap().1.source_out, s(10.0));
    }

    #[test]
    fn keyframes_stay_on_their_moment_when_the_ramp_changes() {
        let mut p = Project::default();
        let m = p.add_media(video(10.0));
        let a = p.append_clip(m).unwrap();
        p.clip_mut(a)
            .unwrap()
            .transform
            .opacity
            .set_keyframe(s(6.0), 50.0);
        let ramp = SpeedRamp::preset(RampPreset::FlashIn, s(0.0), s(10.0));
        assert!(p.set_speed_ramp(a, Some(ramp)));
        let c = p.clip(a).unwrap().1.clone();
        let k = c.transform.opacity.keyframes[0].at;
        assert!((c.source_time(c.start + k) - s(6.0)).abs() <= Time(5));
        assert!(k < s(6.0));
    }

    #[test]
    fn locked_tracks_reject_edits() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let a = p.append_clip(m).unwrap();
        p.tracks[0].locked = true;
        assert!(p.split_clip(a, s(1.0)).is_none());
        assert!(!p.delete_clip(a));
        assert!(p.trim_clip(a, Edge::End, s(2.0)).is_none());
    }

    #[test]
    fn detaching_audio_makes_an_audio_clip() {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let a = p.append_clip(m).unwrap();
        let sound = p.detach_audio(a).unwrap();
        let (t, c) = p.clip(sound).unwrap();
        assert_eq!(t.kind, TrackKind::Audio);
        assert_eq!(c.duration(), s(4.0));
        assert!(p.clip(a).unwrap().1.audio.muted);
        assert!(p.detach_audio(a).is_none());
    }

    /// A project with one video and its detached audio (4 s each), and the
    /// ids of both.
    fn linked_pair() -> (Project, ClipId, ClipId) {
        let mut p = Project::default();
        let m = p.add_media(video(4.0));
        let v = p.append_clip(m).unwrap();
        let a = p.detach_audio(v).unwrap();
        (p, v, a)
    }

    #[test]
    fn detached_audio_is_linked_to_its_video() {
        let (p, v, a) = linked_pair();
        assert_eq!(p.linked_clips(v), vec![a]);
        assert_eq!(p.linked_clips(a), vec![v]);
    }

    #[test]
    fn cutting_the_video_cuts_its_audio_too() {
        let (mut p, v, a) = linked_pair();
        let right = p.split_clip(v, s(1.5)).unwrap();
        // The audio was cut at the same moment.
        let audio = &p
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap()
            .clips;
        assert_eq!(audio.len(), 2);
        assert_eq!(audio[0].end(), s(1.5));
        assert_eq!(audio[1].start, s(1.5));
        // Each side keeps its own pair.
        let right_audio = audio[1].id;
        assert_eq!(p.linked_clips(v), vec![a]);
        assert_eq!(p.linked_clips(right), vec![right_audio]);
        assert_ne!(
            p.clip(v).unwrap().1.link,
            p.clip(right).unwrap().1.link,
            "the halves must not share a link"
        );
    }

    #[test]
    fn cutting_the_audio_cuts_its_video_too() {
        let (mut p, v, a) = linked_pair();
        p.split_clip(a, s(2.0)).unwrap();
        assert_eq!(
            p.tracks
                .iter()
                .find(|t| t.kind == TrackKind::Video)
                .unwrap()
                .clips
                .len(),
            2
        );
        assert_eq!(p.clip(v).unwrap().1.end(), s(2.0));
    }

    #[test]
    fn deleting_the_video_deletes_its_audio() {
        let (mut p, v, a) = linked_pair();
        assert!(p.delete_clip(v));
        assert!(p.clip(v).is_none() && p.clip(a).is_none());
    }

    #[test]
    fn deleting_the_audio_deletes_its_video() {
        let (mut p, v, a) = linked_pair();
        assert!(p.ripple_delete_clip(a));
        assert!(p.clip(v).is_none() && p.clip(a).is_none());
    }

    #[test]
    fn deleting_one_half_leaves_the_other_pair_alone() {
        let (mut p, v, a) = linked_pair();
        let right = p.split_clip(v, s(1.5)).unwrap();
        assert!(p.ripple_delete_clip(v));
        assert!(p.clip(a).is_none());
        // The right half and its sound moved to the start and are still there.
        let (_, r) = p.clip(right).unwrap();
        assert_eq!(r.start, Time::ZERO);
        let audio = &p
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Audio)
            .unwrap()
            .clips;
        assert_eq!(audio.len(), 1);
        assert_eq!(
            audio[0].start,
            Time::ZERO,
            "later sound followed the gap closing"
        );
    }

    #[test]
    fn unlinked_clips_are_edited_alone() {
        let (mut p, v, a) = linked_pair();
        assert!(p.unlink_clip(v));
        assert!(p.linked_clips(a).is_empty());
        p.split_clip(v, s(1.0)).unwrap();
        assert_eq!(
            p.tracks
                .iter()
                .find(|t| t.kind == TrackKind::Audio)
                .unwrap()
                .clips
                .len(),
            1
        );
        assert!(p.delete_clip(v));
        assert!(p.clip(a).is_some());
    }

    #[test]
    fn copies_and_duplicates_are_not_linked_to_the_original() {
        let (mut p, v, _a) = linked_pair();
        let copy = p.duplicate_clip(v).unwrap();
        assert!(p.clip(copy).unwrap().1.link.is_none());
        // Pasting the whole pair together keeps them tied, under a new link.
        let pair: Vec<(TrackKind, Clip)> = p
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(|c| (t.kind, c.clone())))
            .filter(|(_, c)| c.id != copy)
            .collect();
        let pasted = p.paste_clips(&pair, None, s(20.0));
        assert_eq!(pasted.len(), 2);
        let (l0, l1) = (
            p.clip(pasted[0]).unwrap().1.link,
            p.clip(pasted[1]).unwrap().1.link,
        );
        assert!(l0.is_some() && l0 == l1);
        assert_ne!(l0, p.clip(v).unwrap().1.link);
    }

    #[test]
    fn audio_only_media_is_refused_on_video_tracks() {
        let mut p = Project::default();
        let m = p.add_media(MediaItem::new(
            PathBuf::from("song.mp3"),
            MediaKind::Audio,
            s(3.0),
        ));
        let v = p.default_track_for(MediaKind::Video).unwrap();
        assert!(p.add_clip(m, v, Time::ZERO).is_none());
        assert!(p.append_clip(m).is_some());
        assert_eq!(p.tracks[1].clips.len(), 1);
    }

    #[test]
    fn overlay_track_is_found_or_added() {
        let mut p = Project::default();
        let t1 = p.overlay_track_for(s(0.0), s(5.0)).unwrap();
        assert_eq!(p.tracks.len(), 3);
        p.add_generated(Generator::Adjustment, s(5.0), t1, s(0.0))
            .unwrap();
        let again = p.overlay_track_for(s(6.0), s(1.0));
        assert_eq!(again, Some(t1));
        let other = p.overlay_track_for(s(1.0), s(1.0));
        assert_ne!(other, Some(t1));
    }
}

#[cfg(test)]
mod menu_tests {
    use super::*;
    use crate::project::{MediaItem, MAX_AUDIO_TRACKS, MAX_VIDEO_TRACKS};
    use std::path::PathBuf;

    fn s(x: f64) -> Time {
        Time::from_secs_f64(x)
    }

    fn video(p: &mut Project, secs: f64, w: u32, h: u32) -> MediaId {
        let mut m = MediaItem::new(PathBuf::from("/clips/v.mp4"), MediaKind::Video, s(secs));
        m.width = w;
        m.height = h;
        m.has_audio = true;
        p.add_media(m)
    }

    #[test]
    fn track_counts_are_capped() {
        let mut p = Project::new("t");
        while p.add_track(TrackKind::Video).is_some() {}
        while p.add_track(TrackKind::Audio).is_some() {}
        assert_eq!(p.track_count(TrackKind::Video), MAX_VIDEO_TRACKS);
        assert_eq!(p.track_count(TrackKind::Audio), MAX_AUDIO_TRACKS);
        // Video tracks stay above audio tracks.
        let first_audio = p
            .tracks
            .iter()
            .position(|t| t.kind == TrackKind::Audio)
            .unwrap();
        assert!(p.tracks[first_audio..]
            .iter()
            .all(|t| t.kind == TrackKind::Audio));
        // Names stay unique after removing one and adding again.
        let v3 = p.tracks.iter().find(|t| t.name == "Video 3").unwrap().id;
        assert!(p.remove_track(v3));
        let again = p.add_track(TrackKind::Video).unwrap();
        let names: Vec<_> = p.tracks.iter().map(|t| t.name.clone()).collect();
        let mut dedup = names.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(names.len(), dedup.len());
        assert!(p.track(again).is_some());
    }

    #[test]
    fn the_main_track_cannot_be_removed() {
        let mut p = Project::new("t");
        let main = p.main_track().unwrap();
        assert!(!p.remove_track(main));
    }

    #[test]
    fn detach_fails_cleanly_when_audio_tracks_are_full() {
        let mut p = Project::new("t");
        let m = video(&mut p, 4.0, 1920, 1080);
        let c = p.append_clip(m).unwrap();
        // Fill every audio track at that spot.
        let a = video(&mut p, 4.0, 1920, 1080);
        while p.add_track(TrackKind::Audio).is_some() {}
        let audio: Vec<_> = p
            .tracks
            .iter()
            .filter(|t| t.kind == TrackKind::Audio)
            .map(|t| t.id)
            .collect();
        for t in audio {
            p.add_clip(a, t, Time::ZERO).unwrap();
        }
        assert_eq!(p.detach_audio(c), None);
        assert!(!p.clip(c).unwrap().1.audio.muted);
    }

    #[test]
    fn crop_to_fit_fills_the_frame() {
        let mut p = Project::new("t");
        let m = video(&mut p, 4.0, 1080, 1920);
        let c = p.append_clip(m).unwrap();
        assert!(p.crop_to_fit(c));
        let sc = p.clip(c).unwrap().1.transform.scale_x.value;
        // A 9:16 picture fitted in 16:9 is 607.5 wide; covering needs 1920.
        assert!((sc - 100.0 * 1920.0 / 607.5).abs() < 0.01, "{sc}");
    }

    #[test]
    fn crop_to_fit_centres_an_off_centre_crop() {
        let mut p = Project::new("t");
        // A 16:9 video in a 16:9 project, keeping only its right half.
        let m = video(&mut p, 4.0, 1920, 1080);
        let c = p.append_clip(m).unwrap();
        p.clip_mut(c).unwrap().crop.left = 0.5;
        assert!(p.crop_to_fit(c));
        let tr = &p.clip(c).unwrap().1.transform;
        // The 960-wide half must grow to cover 1920×1080: twice as big.
        assert!(
            (tr.scale_x.value - 200.0).abs() < 0.01,
            "{}",
            tr.scale_x.value
        );
        // It sat 480 px right of centre; scaled ×2 that's 960, moved back.
        assert!(
            (tr.position_x.value + 960.0).abs() < 0.01,
            "{}",
            tr.position_x.value
        );
        assert!(tr.position_y.value.abs() < 0.01);
    }

    #[test]
    fn rotating_wraps_around() {
        let mut p = Project::new("t");
        let m = video(&mut p, 4.0, 1920, 1080);
        let c = p.append_clip(m).unwrap();
        for _ in 0..5 {
            p.rotate_clip_90(c);
        }
        assert_eq!(p.clip(c).unwrap().1.transform.rotation.value, 90.0);
    }

    #[test]
    fn replacing_media_keeps_settings_and_ripples() {
        let mut p = Project::new("t");
        let long = video(&mut p, 10.0, 1920, 1080);
        let short = video(&mut p, 3.0, 1920, 1080);
        let a = p.append_clip(long).unwrap();
        let b = p.append_clip(long).unwrap();
        p.clip_mut(a).unwrap().transform.opacity = Animatable::constant(50.0);
        assert!(p.replace_clip_media(a, short));
        let (_, ca) = p.clip(a).unwrap();
        assert_eq!(ca.duration(), s(3.0));
        assert_eq!(ca.transform.opacity.value, 50.0);
        assert_eq!(p.clip(b).unwrap().1.start, s(3.0));
    }

    #[test]
    fn filling_a_cell_keeps_its_place_and_shape() {
        let mut p = Project::new("t");
        let short = video(&mut p, 3.0, 1280, 720);
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let id = p
            .add_generated(Generator::Placeholder, s(5.0), v2, s(2.0))
            .unwrap();
        {
            let c = p.clip_mut(id).unwrap();
            c.video.cell = Some(crate::Cell::rect(0.0, 0.0, 0.5, 1.0));
            c.name = Some("Cell 1".into());
        }
        assert!(!p.replace_clip_media(id, short));
        assert!(p.fill_cell(id, short));
        let (_, c) = p.clip(id).unwrap();
        assert_eq!(c.media_id, Some(short));
        assert!(c.generator.is_none() && c.name.is_none());
        assert_eq!(c.start, s(2.0));
        assert_eq!(c.duration(), s(3.0));
        assert!(c.video.cell.is_some());
        // Only empty cells can be filled.
        assert!(!p.fill_cell(id, short));
    }

    #[test]
    fn paste_keeps_spacing() {
        let mut p = Project::new("t");
        let m = video(&mut p, 2.0, 1920, 1080);
        let a = p.append_clip(m).unwrap();
        let b = p.append_clip(m).unwrap();
        let copied: Vec<_> = [a, b]
            .iter()
            .map(|id| (TrackKind::Video, p.clip(*id).unwrap().1.clone()))
            .collect();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let new = p.paste_clips(&copied, Some(v2), s(10.0));
        assert_eq!(new.len(), 2);
        assert_eq!(p.clip(new[0]).unwrap().1.start, s(10.0));
        assert_eq!(p.clip(new[1]).unwrap().1.start, s(12.0));
        assert_ne!(new[0], a);
    }
}
