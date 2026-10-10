//! Freeze frames (right-click > Freeze Frame): one moment of a
//! video clip held as a still, inserted into the clip at that point.

use crate::clip::{AudioSettings, Clip, ClipId};
use crate::edit::MIN_CLIP;
use crate::keyframe::Animatable;
use crate::project::{MediaId, MediaKind, Project, TrackKind};
use crate::time::Time;
use uuid::Uuid;

impl Clip {
    /// A clip of the picture `still` (this clip's frame at timeline time
    /// `at`) that looks just like this clip at that moment: the same
    /// transform, crop, colour, effects, mask and blending, with every
    /// keyframed value held at its value then. Lasts `len`.
    pub fn freeze_copy(&self, at: Time, still: MediaId, len: Time) -> Clip {
        let lt = self.local(at).max(Time::ZERO).min(self.duration());
        let mut c = self.clone();
        c.id = Uuid::new_v4();
        c.link = None;
        c.media_id = Some(still);
        c.generator = None;
        c.source_in = Time::ZERO;
        c.source_out = len.max(MIN_CLIP);
        c.speed = 1.0;
        c.reverse = false;
        c.for_each_animatable(|a| *a = Animatable::constant(a.value_at(lt)));
        c.volume = Animatable::constant(100.0);
        c.audio = AudioSettings::default();
        c.transition_in = None;
        c.transition_out = None;
        c.anim_in = None;
        c.anim_out = None;
        // Clean-up that works on movement has nothing to do in a still.
        c.video.stabilize = 0.0;
        c.video.deflicker = false;
        c.video.motion_blur = 0.0;
        for e in &mut c.effects {
            e.id = Uuid::new_v4();
        }
        c
    }
}

impl Project {
    /// Freezes the frame of a video clip that shows at timeline time `at`:
    /// the clip is split there and a still of `still` (a picture of that
    /// frame, already in the media library) lasting `len` goes in between.
    /// Later clips on the track move right to make room; on the main track
    /// clips on other tracks and markers after the cut follow, like any
    /// ripple edit. Too close to either end of the clip to split, the still
    /// goes before or after the clip instead. Returns the still's clip.
    pub fn freeze_frame(
        &mut self,
        clip_id: ClipId,
        at: Time,
        still: MediaId,
        len: Time,
    ) -> Option<ClipId> {
        if self.media(still)?.kind != MediaKind::Image {
            return None;
        }
        let (track, clip) = self.clip(clip_id)?;
        let media = clip.media_id.and_then(|m| self.media(m))?;
        if track.locked
            || track.kind != TrackKind::Video
            || media.kind != MediaKind::Video
            || !clip.contains(at)
        {
            return None;
        }
        let track_id = track.id;
        let (start, end) = (clip.start, clip.end());
        let mut frozen = clip.freeze_copy(at, still, len);
        let len = frozen.duration();
        let insert_at = if self.split_clip(clip_id, at).is_some() {
            at
        } else if at - start < end - at {
            start
        } else {
            end
        };
        if self.is_magnetic(track_id) {
            return self.place_clip(frozen, track_id, insert_at);
        }
        let main = self.main_track() == Some(track_id);
        let track = self.track_mut(track_id)?;
        for c in track.clips.iter_mut().filter(|c| c.start >= insert_at) {
            c.start += len;
        }
        frozen.start = insert_at;
        let id = frozen.id;
        track.clips.push(frozen);
        track.clips.sort_by_key(|c| c.start);
        if main {
            self.ripple_others(track_id, insert_at, len);
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::{Effect, Generator, Mask, MaskShape};
    use crate::project::{MediaItem, TrackId};
    use std::path::PathBuf;

    fn s(secs: f64) -> Time {
        Time::from_secs_f64(secs)
    }

    fn media(p: &mut Project, kind: MediaKind, secs: f64) -> MediaId {
        let mut m = MediaItem::new(PathBuf::from("/clips/v.mp4"), kind, s(secs));
        m.width = 1280;
        m.height = 720;
        m.has_audio = kind == MediaKind::Video;
        p.add_media(m)
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
    fn freezing_on_the_main_track_inserts_a_still_and_ripples() {
        let mut p = Project::default();
        let v = media(&mut p, MediaKind::Video, 10.0);
        let still = media(&mut p, MediaKind::Image, 5.0);
        let main = p.main_track().unwrap();
        let a = p.append_clip(v).unwrap();
        let b = p.append_clip(v).unwrap();
        {
            let c = p.clip_mut(a).unwrap();
            c.transform.opacity.set_keyframe(s(0.0), 0.0);
            c.transform.opacity.set_keyframe(s(10.0), 100.0);
            c.transform.scale_x = Animatable::constant(150.0);
            c.crop.left = 0.2;
            c.color.saturation = Animatable::constant(-40.0);
            c.effects.push(Effect::new("blur.gaussian"));
            c.video.mask = Some(Mask::new(MaskShape::Ellipse));
            c.anim_in = Some(crate::ClipAnimation {
                kind: "fade".into(),
                duration: s(1.0),
            });
        }
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let title = p
            .add_generated(Generator::Title(Default::default()), s(2.0), v2, s(12.0))
            .unwrap();
        let marker = p.add_marker(s(11.0));

        let f = p.freeze_frame(a, s(4.0), still, s(5.0)).unwrap();
        assert_eq!(starts(&p, main), vec![0.0, 4.0, 9.0, 15.0]);
        let (track, fc) = p.clip(f).unwrap();
        assert_eq!(track.id, main);
        assert_eq!(fc.media_id, Some(still));
        assert_eq!((fc.start, fc.duration()), (s(4.0), s(5.0)));
        // Looks like the clip at 4 s: keyframes held, settings carried over.
        assert!(!fc.transform.opacity.is_animated());
        assert!((fc.transform.opacity.value - 40.0).abs() < 1e-6);
        assert_eq!(fc.transform.scale_x.value, 150.0);
        assert_eq!(fc.crop.left, 0.2);
        assert_eq!(fc.color.saturation.value, -40.0);
        assert_eq!(fc.effects.len(), 1);
        assert_ne!(fc.effects[0].id, p.clip(a).unwrap().1.effects[0].id);
        assert!(fc.video.mask.is_some());
        assert!(fc.anim_in.is_none());
        // The rest of the clip plays on after the still.
        let right = &p.track(main).unwrap().clips[2];
        assert_eq!(right.source_in, s(4.0));
        assert_eq!(right.end(), s(15.0));
        assert_eq!(p.clip(b).unwrap().1.start, s(15.0));
        assert_eq!(p.clip(title).unwrap().1.start, s(17.0));
        assert_eq!(p.marker(marker).unwrap().time, s(16.0));
    }

    #[test]
    fn freezing_an_overlay_pushes_only_its_own_track() {
        let mut p = Project::default();
        let v = media(&mut p, MediaKind::Video, 10.0);
        let still = media(&mut p, MediaKind::Image, 5.0);
        let main = p.main_track().unwrap();
        p.append_clip(v).unwrap();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let a = p.add_clip(v, v2, s(0.0)).unwrap();
        p.add_clip(v, v2, s(12.0)).unwrap();
        let marker = p.add_marker(s(11.0));
        let f = p.freeze_frame(a, s(3.0), still, s(2.0)).unwrap();
        assert_eq!(starts(&p, v2), vec![0.0, 3.0, 5.0, 14.0]);
        assert_eq!(p.clip(f).unwrap().1.start, s(3.0));
        assert_eq!(starts(&p, main), vec![0.0]);
        assert_eq!(p.marker(marker).unwrap().time, s(11.0));
    }

    #[test]
    fn freezing_at_the_very_end_adds_the_still_after_the_clip() {
        let mut p = Project::default();
        let v = media(&mut p, MediaKind::Video, 4.0);
        let still = media(&mut p, MediaKind::Image, 5.0);
        let main = p.main_track().unwrap();
        let a = p.append_clip(v).unwrap();
        let f = p.freeze_frame(a, s(3.99), still, s(1.0)).unwrap();
        assert_eq!(starts(&p, main), vec![0.0, 4.0]);
        assert_eq!(p.clip(a).unwrap().1.duration(), s(4.0));
        assert_eq!(p.clip(f).unwrap().1.start, s(4.0));
    }

    #[test]
    fn only_video_clips_can_be_frozen() {
        let mut p = Project::default();
        let v = media(&mut p, MediaKind::Video, 4.0);
        let still = media(&mut p, MediaKind::Image, 5.0);
        let a = p.append_clip(v).unwrap();
        let v2 = p.add_track(TrackKind::Video).unwrap();
        let t = p
            .add_generated(Generator::Solid([1.0; 4]), s(2.0), v2, s(0.0))
            .unwrap();
        assert!(p.freeze_frame(t, s(1.0), still, s(1.0)).is_none());
        // The still must be a picture.
        assert!(p.freeze_frame(a, s(1.0), v, s(1.0)).is_none());
        // Not where the clip isn't, nor on a locked track.
        assert!(p.freeze_frame(a, s(5.0), still, s(1.0)).is_none());
        let main = p.main_track().unwrap();
        p.track_mut(main).unwrap().locked = true;
        assert!(p.freeze_frame(a, s(1.0), still, s(1.0)).is_none());
        assert_eq!(p.track(main).unwrap().clips.len(), 1);
    }
}
