//! Audit rendered join samples without editing a saved Smart Cut project.
use serde_json::json;
use shotclutch_engine::{Session, preedit};
use shotclutch_time::Tick;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: smart_cut_join_audit PROJECT OUTPUT_JSON".into());
    }
    let mut session = Session::default();
    session.execute("file.open", json!({"path":std::path::Path::new(&args[1]).canonicalize()?}))?;
    let (&id, map) = session.project.preedit.maps.last_key_value().ok_or("No Smart Cut map")?;
    let analysis = &session.project.preedit.analyses[&map.analysis];
    let segments = map.segments(analysis.source.range)?;
    let (project, sequence) = preedit::preview(&session, id)?;
    let seq = project.sequence(sequence).ok_or("No preview sequence")?;
    let sr = seq.settings.sample_rate;
    let rate = i64::from(sr);
    let provider = session.media.full_res_provider(project.clone(), session.services.clone());
    let original = session.media.full_res_source(&session.project, analysis.source.item, &*session.services).ok_or("Original unavailable")?;
    let duration = seq.duration();
    let duration_samples = duration.to_units_floor(rate);
    let window = Tick::from_units(10, 1000).to_units_floor(rate);
    let mut joins = Vec::new();
    let mut maximum_step = 0.0f32;
    let mut maximum_raw_step = 0.0f32;
    for pair in segments.windows(2) {
        if pair[0].output.end() != pair[1].output.start {
            return Err("Noncontiguous output".into());
        }
        let seam = pair[1].output.start.to_units_floor(rate);
        let start = (seam - window).max(0);
        let end = (seam + window).min(duration_samples);
        if seam <= start || seam >= end {
            return Err("Join has no samples on both sides".into());
        }
        let rendered = shotclutch_render::audio::mix_sequence(&project, seq, start, (end - start) as usize, &provider);
        let offset = (seam - start) as usize;
        let step = rendered.channels.iter().map(|c| (c[offset] - c[offset - 1]).abs()).fold(0.0f32, f32::max);
        let left = original.audio_stream(analysis.source.audio_stream, pair[0].source.end().to_units_floor(rate) - 1, 1, sr)?;
        let right = original.audio_stream(analysis.source.audio_stream, pair[1].source.start.to_units_floor(rate), 1, sr)?;
        let raw_step = left.channels.iter().zip(&right.channels).map(|(a, b)| (b[0] - a[0]).abs()).fold(0.0f32, f32::max);
        maximum_step = maximum_step.max(step);
        maximum_raw_step = maximum_raw_step.max(raw_step);
        joins.push(json!({"output_sample":seam,"source_left_end":pair[0].source.end().0,"source_right_start":pair[1].source.start.0,
            "rendered_step":step,"unfaded_source_step":raw_step}));
    }
    let report = json!({"map":id,"duration_ticks":duration.0,"duration_seconds":duration.seconds(),"sample_rate":sr,
        "retained_segments":segments.len(),"joins_checked":joins.len(),"maximum_rendered_join_step":maximum_step,
        "maximum_unfaded_source_join_step":maximum_raw_step,"passed_declick":maximum_step<=1e-6,
        "scope":"Sample discontinuity check; editorial meaning and perceptual pacing are separate.","joins":joins});
    std::fs::write(&args[2], serde_json::to_vec_pretty(&report)?)?;
    println!("Checked {} joins: maximum rendered step {maximum_step:.9}; unfaded {maximum_raw_step:.6}; duration {:.3}s", joins.len(), duration.seconds());
    if maximum_step > 1e-6 {
        return Err("Join discontinuity exceeds the de-click limit".into());
    }
    Ok(())
}
