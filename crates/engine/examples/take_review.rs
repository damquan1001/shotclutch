use shotclutch_engine::preedit::take_review::{self, ReviewProgress};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicBool},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: take_review INPUT OUTPUT MODEL".into());
    }
    let input: serde_json::Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
    let map = serde_json::from_value(input["map"].clone())?;
    let analysis = serde_json::from_value(input["analysis"].clone())?;
    let checkpoint = input["checkpoint"].as_str().ok_or("missing checkpoint")?.parse()?;
    let progress = Arc::new(Mutex::new(ReviewProgress::default()));
    let watch = progress.clone();
    let done = Arc::new(AtomicBool::new(false));
    let worker_done = done.clone();
    let output = PathBuf::from(&args[2]);
    let status = output.with_extension("progress.json");
    let reporter = std::thread::spawn(move || {
        while !worker_done.load(std::sync::atomic::Ordering::Relaxed) {
            let p = watch.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
            let _ = std::fs::write(&status, serde_json::to_vec(&p).unwrap());
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
    });
    let result = take_review::run(checkpoint, &map, &analysis, &args[3], &AtomicBool::new(false), &progress).map_err(std::io::Error::other);
    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = reporter.join();
    let mut result = result?;
    // Bind the worker's response to the same source snapshot captured by the editor.
    // The model sees transcript evidence only; it must never create this stamp.
    result.source_stamp = input.get("source_stamp").filter(|stamp| !stamp.is_null()).cloned().map(serde_json::from_value).transpose()?;
    std::fs::write(output, serde_json::to_vec_pretty(&result)?)?;
    println!("{} groups, {} failures, {:.1} seconds", result.report.completed_groups, result.report.failures.len(), result.report.elapsed_seconds);
    Ok(())
}
