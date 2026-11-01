//! Golden-image tests for ShotClutch's renderer (test-only crate; the library is empty).
//!
//! `tests/golden.rs` builds small procedural projects (no media files), renders one frame of each
//! through the CPU reference compositor (`shotclutch-render`) and compares it with a committed PNG
//! in `goldens/` using `shotclutch_testkit::golden`. The same scenes are composited on the GPU
//! (`shotclutch-gpu`) when an adapter is available and compared with the CPU result.
//!
//! Regenerate references with `SHOTCLUTCH_BLESS=1 cargo test -p shotclutch-golden`.
