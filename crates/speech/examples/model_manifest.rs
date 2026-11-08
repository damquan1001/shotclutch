//! Export the canonical pinned catalogue for offline qualification tools.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(shotclutch_speech::models::catalogue())?);
    Ok(())
}
