//! Make a qualification copy with the same project and a fresh undo history.
//! Keep it beside the input so relative media references remain unchanged.
use serde::Deserialize;
use shotclutch_format::session::{DocumentId, DocumentState, decimal_u64};
use std::{io::Write, path::PathBuf};

#[derive(Deserialize)]
struct Header {
    version: u32,
    project_id: DocumentId,
    #[serde(with = "decimal_u64")]
    revision: u64,
    #[serde(with = "decimal_u64")]
    next_id: u64,
}
#[derive(Deserialize)]
struct Input {
    format: String,
    schema_version: u32,
    project: shotclutch_project::Project,
    edit_state: Option<Header>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: take_review_seed INPUT OUTPUT (same directory)".into());
    }
    let input = PathBuf::from(&args[1]).canonicalize()?;
    let output = PathBuf::from(&args[2]);
    if input.parent() != Some(output.parent().ok_or("missing output directory")?.canonicalize()?.as_path()) {
        return Err("Keep the copy beside the input to preserve relative media references".into());
    }
    // Deserialize only the header of the old state; legacy undo snapshots are
    // skipped rather than expanded into many copies of the saved transcript.
    let saved: Input = serde_json::from_slice(&std::fs::read(&input)?)?;
    if saved.format != shotclutch_format::FORMAT_ID || saved.schema_version != shotclutch_format::SCHEMA_VERSION {
        return Err("Use a current native project for qualification".into());
    }
    saved.project.preedit.check()?;
    let state = saved.edit_state.map(|h| DocumentState {
        version: h.version,
        project_id: h.project_id,
        revision: h.revision,
        next_id: h.next_id,
        undo: vec![],
        redo: vec![],
        operations: vec![],
    });
    let bytes = shotclutch_format::encode_document(&saved.project, None, state.as_ref(), None, false);
    std::fs::OpenOptions::new().write(true).create_new(true).open(&output)?.write_all(&bytes)?;
    println!("Saved {} bytes with the same project and fresh qualification history", bytes.len());
    Ok(())
}
