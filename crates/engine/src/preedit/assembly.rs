use std::sync::Arc;

use shotclutch_edit::transcript::{CaptionRules, SeqWord, caption_blocks};
use shotclutch_project::preedit::*;
use shotclutch_project::{
    Caption, CaptionTrack, ClipId, ItemId, ItemKind, Project, SequenceSettings, TrackId, TrackKind, Transition, TransitionAlign, TransitionId,
};
use shotclutch_time::{Tick, TimeRange};

use super::fail;
use crate::Result;

/// Snap edit decisions before persisting them, so the recorded map and the assembly agree.
pub(super) fn snap_decision(
    video: Option<&dyn shotclutch_media::MediaSource>,
    sample_rate: u32,
    available: TimeRange,
    range: TimeRange,
    inward: bool,
) -> Result<TimeRange> {
    if !encloses(available, range) {
        return Err(fail("INVALID_ARGUMENT: decision exceeds the selected source range"));
    }
    let (start, end) = if let Some(video) = video {
        let snap = |tick, after| {
            if tick == available.start || tick == available.end() {
                return Ok(tick);
            }
            video.video_edit_boundary(tick, after).ok_or_else(|| fail("CAPABILITY_UNAVAILABLE: no indexed frame boundary in the requested direction"))
        };
        (snap(range.start, inward)?, snap(range.end(), !inward)?)
    } else {
        let sample_rate = i64::from(sample_rate.max(1));
        let snap = |t: Tick, after: bool| {
            let sample = t.to_units_floor(sample_rate);
            let boundary = Tick::from_units(sample, sample_rate);
            if after && boundary < t { Tick::from_units(sample + 1, sample_rate) } else { boundary }
        };
        (snap(range.start, inward), snap(range.end(), !inward))
    };
    let (start, end) = (start.max(available.start), end.min(available.end()));
    if start >= end {
        return Err(fail("INVALID_ARGUMENT: range has no complete edit interval"));
    }
    Ok(TimeRange::from_bounds(start, end))
}

pub(super) fn build(project: &mut Project, map: &PreEditMap, analysis: &PreEditAnalysis) -> Result<(ItemId, Vec<Vec<ClipId>>)> {
    let source = &analysis.source;
    let item = project.item(source.item).ok_or_else(|| fail("MEDIA_CHANGED: original has been removed"))?;
    let ItemKind::Media(media) = &item.kind else { return Err(fail("MEDIA_CHANGED: original is no longer a media asset")) };
    let selected_audio = media.info.audio_streams.get(source.audio_stream).ok_or_else(|| fail("MEDIA_CHANGED: selected audio stream is missing"))?;
    if media.offline || !encloses(TimeRange::from_bounds(Tick::ZERO, media.duration()), source.range) {
        return Err(fail("MEDIA_CHANGED: selected source range is unavailable"));
    }
    let video = media.info.video.as_ref();
    let has_video = video.is_some();
    let mut settings = SequenceSettings { sample_rate: selected_audio.sample_rate, ..SequenceSettings::default() };
    if let Some(video) = video {
        settings.width = video.width;
        settings.height = video.height;
        settings.par = video.par;
        settings.frame_rate = video.frame_rate;
    }
    let rate = settings.frame_rate;
    let frame_size = (settings.width, settings.height);
    let name = format!("{} — Rough Cut", item.name);
    let segments = map.segments(source.range).map_err(fail)?;
    if segments.is_empty() {
        return Err(fail("INVALID_ARGUMENT: retain at least one source interval"));
    }
    let duration = segments.last().map_or(Tick::ZERO, |segment| segment.output.end());
    let sequence = project.new_sequence(&name, settings, usize::from(has_video), 1, None);
    let mut video_items = Vec::new();
    let mut audio_items = Vec::new();
    let mut transitions = Vec::new();
    let mut clips = Vec::new();
    let mut words = Vec::new();
    let transcript: Vec<_> = analysis.chunks.iter().filter_map(|c| c.transcript.as_ref()).flat_map(|t| t.words.iter().map(move |w| (w, t))).collect();
    for segment in segments {
        let link = has_video.then(|| project.alloc_id());
        let mut ids = Vec::new();
        if has_video {
            let mut video = project
                .make_track_item(source.item, TrackKind::Video, segment.output.start, segment.source, rate)
                .ok_or_else(|| fail("MEDIA_CHANGED: could not bind video"))?;
            video.duration = segment.output.duration;
            video.link = link;
            for effect in &mut video.effects {
                shotclutch_project::resolve_auto_points(effect, frame_size, frame_size);
            }
            ids.push(video.id);
            video_items.push(video);
        }
        let mut audio = project
            .make_track_item(source.item, TrackKind::Audio, segment.output.start, segment.source, rate)
            .ok_or_else(|| fail("MEDIA_CHANGED: could not bind audio"))?;
        audio.duration = segment.output.duration;
        audio.link = link;
        audio.audio_stream = source.audio_stream;
        audio.source_channels.clear();
        ids.push(audio.id);
        let fade = Tick::from_units(5, 1000).min(Tick(audio.duration.0 / 2));
        if fade > Tick::ZERO {
            for (start, from, to, align) in
                [(audio.start, None, Some(audio.id), TransitionAlign::StartAtCut), (audio.end() - fade, Some(audio.id), None, TransitionAlign::EndAtCut)]
            {
                transitions.push(Transition {
                    id: TransitionId(project.alloc_id()),
                    effect: shotclutch_project::effect::find_effect("constant_power").ok_or_else(|| fail("INTERNAL_ERROR: audio fade unavailable"))?.instance(),
                    start,
                    duration: fade,
                    from,
                    to,
                    align,
                    reverse: false,
                });
            }
        }
        let start = transcript.partition_point(|(w, _)| w.end <= segment.source.start);
        for (index, (word, transcript)) in transcript.iter().enumerate().skip(start).take_while(|(_, (w, _))| w.start < segment.source.end()) {
            let midpoint = word.start + Tick((word.end - word.start).0 / 2);
            if segment.source.contains(midpoint) {
                let map_tick = |t: Tick| segment.output.start + (t.clamp(segment.source.start, segment.source.end()) - segment.source.start);
                words.push(SeqWord {
                    text: map.corrections.get(&word.start.0).unwrap_or(&word.text).clone(),
                    start: map_tick(word.start),
                    end: map_tick(word.end),
                    clip: audio.id,
                    item: source.item,
                    index,
                    track: 0,
                    speaker: transcript.speaker_name(word),
                    confidence: word.confidence,
                });
            }
        }
        audio_items.push(audio);
        clips.push(ids);
    }
    let captions = if words.is_empty() {
        None
    } else {
        let mut track = CaptionTrack::new(TrackId(project.alloc_id()), "Rough Cut Transcript".into(), Default::default());
        track.language = analysis.settings.language.clone();
        for block in caption_blocks(&words, &CaptionRules::default(), rate) {
            let end = block.end.min(duration);
            if end > block.start {
                track.captions.push(Caption {
                    id: ClipId(project.alloc_id()),
                    start: block.start,
                    duration: end - block.start,
                    text: block.text,
                    speaker: block.speaker,
                    cue_id: None,
                    settings: String::new(),
                });
            }
        }
        Some(track)
    };
    let sequence_item = project.item_mut(sequence).ok_or_else(|| fail("INTERNAL_ERROR: missing assembly sequence"))?;
    let ItemKind::Sequence(sequence_data) = &mut sequence_item.kind else { return Err(fail("INTERNAL_ERROR: invalid assembly sequence")) };
    let seq = Arc::make_mut(sequence_data);
    if let Some(track) = seq.video_tracks.first_mut() {
        track.items = video_items;
    }
    seq.audio_tracks[0].items = audio_items;
    seq.audio_tracks[0].transitions = transitions;
    seq.caption_tracks.extend(captions);
    seq.check().map_err(fail)?;
    Ok((sequence, clips))
}
