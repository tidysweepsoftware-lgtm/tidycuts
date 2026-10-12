//! End-to-end check with real files: `cargo run -p tidycuts-engine --example e2e -- video.mp4 other.mp4 audio.wav out_dir`.
//! Builds a small project (two clips with a transition, a title and a
//! picture-in-picture), renders frames through the streaming decoders and
//! the preview path, times the scrub cache and mixes the sound.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tidycuts_core::*;
use tidycuts_engine::mixer::Mixer;
use tidycuts_engine::*;
use tidycuts_render::*;

fn save(path: &str, w: u32, h: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgba).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = &args[4];
    let a = tidycuts_media::probe(std::path::Path::new(&args[1])).unwrap();
    let b = tidycuts_media::probe(std::path::Path::new(&args[2])).unwrap();
    let music = tidycuts_media::probe(std::path::Path::new(&args[3])).unwrap();
    println!(
        "probed {} {}x{} {:?}",
        a.name, a.width, a.height, a.frame_rate
    );
    let mut p = Project::new("e2e");
    let (ma, mb, mm) = (
        p.add_media(a.clone()),
        p.add_media(b.clone()),
        p.add_media(music),
    );
    let c1 = p.append_clip(ma).unwrap();
    let c2 = p.append_clip(mb).unwrap();
    p.clip_mut(c2).unwrap().transition_in =
        Some(Transition::new("dissolve", Time::from_secs_f64(1.0)));
    let v2 = p.add_track(TrackKind::Video).unwrap();
    let pip = p.add_clip(mb, v2, Time::from_secs_f64(2.0)).unwrap();
    {
        let c = p.clip_mut(pip).unwrap();
        c.transform.scale_x = Animatable::constant(35.0);
        c.transform.scale_y = Animatable::constant(35.0);
        c.transform.position_x = Animatable::constant(560.0);
        c.transform.position_y = Animatable::constant(-280.0);
        c.video.corner_radius = 24.0;
        c.video.shadow.enabled = true;
        c.audio.muted = true;
    }
    let v3 = p.add_track(TrackKind::Video).unwrap();
    p.add_generated(
        Generator::Title(TitleSpec::default()),
        Time::from_secs_f64(4.0),
        v3,
        Time::from_secs_f64(1.0),
    )
    .unwrap();
    let at = p
        .audio_track_for(Time::ZERO, Time::from_secs_f64(10.0))
        .unwrap();
    let mc = p.add_clip(mm, at, Time::ZERO).unwrap();
    p.clip_mut(mc).unwrap().audio.fade_in = Time::from_secs_f64(2.0);
    p.clip_mut(c1).unwrap().audio.ducking = 0.0;

    let renderer = gpu::headless().expect("gpu");
    let mut comp = Compositor::new(renderer);
    let (w, h) = (960u32, 540u32);
    let target = comp.make_target(w, h, wgpu::TextureFormat::Rgba8Unorm, true);
    let statics = Statics::default();
    let cut = p.clip(c2).unwrap().1.start.as_secs_f64();

    // Playback path.
    let mut sf = StreamFrames::new(Duration::from_secs(5), false, true);
    let started = Instant::now();
    let mut n = 0;
    for (i, t) in [0.5, 1.0, 1.5, 2.5, 3.0, cut - 0.25, cut, cut + 0.4]
        .iter()
        .enumerate()
    {
        let mut pl = Planner {
            project: &p,
            out_scale: w as f32 / 1920.0,
            frames: &mut sf,
            statics: &statics,
            auto_grade: &|_| None,
            levels: &Default::default(),
            show_empty_cells: false,
            used_statics: Vec::new(),
        };
        let f = pl.build(Time::from_secs_f64(*t));
        for (k, img) in &f.statics {
            if !comp.has_static(*k) {
                comp.store_static(*k, img);
            }
        }
        let px = comp.render_to_cpu(&f.frame, &target);
        save(&format!("{out}/play_{i}.png"), w, h, &px);
        sf.end_frame();
        n += 1;
    }
    println!("rendered {n} playback frames in {:?}", started.elapsed());

    // Sequential throughput like real playback.
    let started = Instant::now();
    let mut frames = 0;
    let mut t = 0.0;
    while t < 4.0 {
        let mut pl = Planner {
            project: &p,
            out_scale: w as f32 / 1920.0,
            frames: &mut sf,
            statics: &statics,
            auto_grade: &|_| None,
            levels: &Default::default(),
            show_empty_cells: false,
            used_statics: Vec::new(),
        };
        let f = pl.build(Time::from_secs_f64(t));
        for (k, img) in &f.statics {
            if !comp.has_static(*k) {
                comp.store_static(*k, img);
            }
        }
        let _ = comp.render_to_cpu(&f.frame, &target);
        sf.end_frame();
        frames += 1;
        t += 1.0 / 25.0;
    }
    let el = started.elapsed();
    println!(
        "sequential: {frames} frames in {el:?} = {:.1} fps (software GPU)",
        frames as f64 / el.as_secs_f64()
    );

    // Decoding and planning alone (no GPU work).
    let mut sf2 = StreamFrames::new(Duration::from_secs(5), false, true);
    let started = Instant::now();
    let mut t = 0.0;
    let mut frames = 0;
    while t < 4.0 {
        let mut pl = Planner {
            project: &p,
            out_scale: w as f32 / 1920.0,
            frames: &mut sf2,
            statics: &statics,
            auto_grade: &|_| None,
            levels: &Default::default(),
            show_empty_cells: false,
            used_statics: Vec::new(),
        };
        let _ = pl.build(Time::from_secs_f64(t));
        sf2.end_frame();
        frames += 1;
        t += 1.0 / 25.0;
    }
    let el = started.elapsed();
    println!(
        "decode+plan only: {frames} frames in {el:?} = {:.1} fps",
        frames as f64 / el.as_secs_f64()
    );

    // Scrub cache.
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
    let scrub = ScrubCache::new(
        std::path::PathBuf::from(format!("{out}/cache")),
        3,
        notify.clone(),
    );
    let server = FrameServer::new(scrub.clone(), 256 << 20, 2, notify);
    let started = Instant::now();
    let cache = scrub.get(&a).unwrap();
    while !cache.is_complete() {
        std::thread::sleep(Duration::from_millis(50));
    }
    println!(
        "scrub cache for {:.0}s of video built in {:?}",
        a.duration.as_secs_f64(),
        started.elapsed()
    );
    let started = Instant::now();
    let mut got = 0;
    for i in 0..60 {
        if server.scrub_frame(&a, i as f64 * 0.3, 0.1).is_some() {
            got += 1;
        }
    }
    println!("{got}/60 scrub frames in {:?}", started.elapsed());
    let mut pf = PreviewFrames {
        server: &server,
        scrubbing: true,
        incomplete: false,
        hwaccel: true,
    };
    let mut pl = Planner {
        project: &p,
        out_scale: w as f32 / 1920.0,
        frames: &mut pf,
        statics: &statics,
        auto_grade: &|_| None,
        levels: &Default::default(),
        show_empty_cells: false,
        used_statics: Vec::new(),
    };
    let f = pl.build(Time::from_secs_f64(1.7));
    let px = comp.render_to_cpu(&f.frame, &target);
    save(&format!("{out}/scrub.png"), w, h, &px);

    // Sound.
    let mut mixer = Mixer::new(48000, Duration::from_secs(5));
    let mut buf = vec![0f32; 48000 * 2];
    let mut rms = Vec::new();
    for s in 0..6 {
        mixer.mix(&p, Time::from_secs_f64(s as f64), &mut buf);
        let r = (buf.iter().map(|x| x * x).sum::<f32>() / buf.len() as f32).sqrt();
        rms.push(r);
    }
    println!("mix rms per second: {rms:.3?}");

    // Export a few seconds around the cut.
    println!(
        "working encoders: {:?}",
        tidycuts_engine::export::working_encoders()
    );
    let mut es = tidycuts_engine::export::ExportSettings::for_project(
        &p,
        std::path::PathBuf::from(format!("{out}/export.mp4")),
    );
    es.width = 960;
    es.height = 540;
    es.range = Some((
        Time::from_secs_f64(cut - 2.0),
        Time::from_secs_f64(cut + 1.0),
    ));
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let started = Instant::now();
    let r = tidycuts_engine::export::export(
        &p,
        &es,
        comp.renderer().clone(),
        &statics,
        &|_| None,
        &cancel,
        |_| {},
    );
    println!("export: {r:?} in {:?}", started.elapsed());
}
