# M0 CPU benchmark comparison

Inherited revision: `3a5f217b348bf0c9dc1678f305c34b6c88617f37`. Milestone revision: `f35fac68e9ebca6e8d8503210d2f7f750c9058bb`.

Software decode and CPU composition, quick workloads, identical fixture hashes. Fixture preparation is excluded from measurement. OS filesystem caches are uncontrolled. These are descriptive baselines, not qualification against the release budgets in document 14.

Repeated rows are summarized by the median of the harness-reported values; this is not a pooled percentile. Decode/export FPS retain the inherited best-of-repeats scalar; individual FPS/time runs remain in the raw JSON. Quick seek percentiles have small sample counts. FPS is higher-is-better; latency, timeouts, and drop percentage are lower-is-better. A zero-frame playback case or high drop rate remains a failed real-time workload even when the benchmark process completes.

| Section / case | Metric | Upstream | ShotClutch | Change |
|---|---|---:|---:|---:|
| decode: fixture=dec_h264_1080.mp4 | fps | 128.627 | 118.148 | -8.1% |
| decode: fixture=dec_h264_2160.mp4 | fps | 37.251 | 32.499 | -12.8% |
| decode: fixture=dec_hevc_1080.mp4 | fps | 116.369 | 93.019 | -20.1% |
| decode: fixture=dec_hevc_2160.mp4 | fps | 38.593 | 34.453 | -10.7% |
| decode: fixture=dec_hevc10_2160.mp4 | fps | 26.119 | 28.093 | +7.6% |
| decode: fixture=dec_vp9_1080.webm | fps | 102.561 | 110.126 | +7.4% |
| decode: fixture=dec_vp9_2160.webm | fps | 30.492 | 29.843 | -2.1% |
| decode: fixture=dec_av1_1080.mp4 | fps | 83.757 | 76.371 | -8.8% |
| decode: fixture=dec_av1_2160.mp4 | fps | 21.764 | 19.584 | -10.0% |
| decode: fixture=dec_prores_1080.mov | fps | 155.608 | 156.655 | +0.7% |
| decode: fixture=dec_prores_2160.mov | fps | 40.535 | 39.722 | -2.0% |
| seek: fixture=a1080.mp4, mode=full | p50_ms | 510.380 | 428.292 | -16.1% |
| seek: fixture=a1080.mp4, mode=full | p95_ms | 571.920 | 548.613 | -4.1% |
| seek: fixture=a1080.mp4, mode=keep 2 s | p50_ms | 451.080 | 428.067 | -5.1% |
| seek: fixture=a1080.mp4, mode=keep 2 s | p95_ms | 532.069 | 530.827 | -0.2% |
| seek: fixture=a1080.mp4, mode=late | p50_ms | 306.168 | 316.046 | +3.2% |
| seek: fixture=a1080.mp4, mode=late | p95_ms | 389.080 | 380.600 | -2.2% |
| seek: fixture=a2160.mp4, mode=full | p50_ms | 5557.129 | 5454.569 | -1.8% |
| seek: fixture=a2160.mp4, mode=full | p95_ms | 5610.021 | 6422.011 | +14.5% |
| seek: fixture=a2160.mp4, mode=keep 2 s | p50_ms | 5315.818 | 5180.120 | -2.6% |
| seek: fixture=a2160.mp4, mode=keep 2 s | p95_ms | 5486.145 | 5348.318 | -2.5% |
| seek: fixture=a2160.mp4, mode=late | p50_ms | 4954.502 | 5216.266 | +5.3% |
| seek: fixture=a2160.mp4, mode=late | p95_ms | 5600.493 | 5924.049 | +5.8% |
| seek: fixture=hevc2160.mp4, mode=full | p50_ms | 4404.633 | 4772.100 | +8.3% |
| seek: fixture=hevc2160.mp4, mode=full | p95_ms | 4859.351 | 5567.305 | +14.6% |
| seek: fixture=hevc2160.mp4, mode=keep 2 s | p50_ms | 3506.062 | 3508.570 | +0.1% |
| seek: fixture=hevc2160.mp4, mode=keep 2 s | p95_ms | 3704.009 | 3876.747 | +4.7% |
| seek: fixture=hevc2160.mp4, mode=late | p50_ms | 3357.523 | 3176.964 | -5.4% |
| seek: fixture=hevc2160.mp4, mode=late | p95_ms | 3855.430 | 3449.180 | -10.5% |
| playback: scenario=h264-1080, res=full | drop_pct | 0.000 | 0.000 | n/a |
| playback: scenario=stack3, res=full | drop_pct | 2.083 | 7.292 | +250.0% |
| playback: scenario=stack3-blend, res=full | drop_pct | 44.792 | 26.042 | -41.9% |
| playback: scenario=stack3-fx, res=full | drop_pct | 94.792 | 96.875 | +2.2% |
| playback: scenario=h264-2160, res=full | drop_pct | 98.958 | 96.875 | -2.1% |
| playback: scenario=h264-2160, res=half | drop_pct | 64.583 | 57.292 | -11.3% |
| playback: scenario=h264-2160, res=half draft | drop_pct | 72.917 | 88.542 | +21.4% |
| playback: scenario=h264-2160, res=quarter | drop_pct | 31.250 | 70.833 | +126.7% |
| playback: scenario=h264-2160, res=quarter draft | drop_pct | 26.042 | 27.083 | +4.0% |
| playback: scenario=hevc-2160, res=full | drop_pct | 98.958 | 100.000 | +1.1% |
| playback: scenario=hevc-2160, res=half | drop_pct | 44.792 | 94.792 | +111.6% |
| playback: scenario=hevc-2160, res=half draft | drop_pct | 15.625 | 18.750 | +20.0% |
| playback: scenario=vp9-2160, res=full | drop_pct | 87.500 | 91.667 | +4.8% |
| playback: scenario=vp9-2160, res=half | drop_pct | 73.958 | 79.167 | +7.0% |
| playback: scenario=vp9-2160, res=half draft | drop_pct | 64.583 | 69.792 | +8.1% |
| playback: scenario=av1-2160, res=full | drop_pct | 98.958 | 100.000 | +1.1% |
| playback: scenario=av1-2160, res=half | drop_pct | 95.833 | 92.708 | -3.3% |
| playback: scenario=av1-2160, res=half draft | drop_pct | 98.958 | 96.875 | -2.1% |
| scrub: scenario=h264-1080 | seek_p95_ms | 577.287 | 486.921 | -15.7% |
| scrub: scenario=h264-1080 | timeouts | 0.000 | 0.000 | n/a |
| scrub: scenario=h264-2160 | seek_p95_ms | 4563.704 | 4641.567 | +1.7% |
| scrub: scenario=h264-2160 | timeouts | 0.000 | 0.000 | n/a |
| scrub: scenario=hevc-2160 | seek_p95_ms | 3362.711 | 3502.293 | +4.2% |
| scrub: scenario=hevc-2160 | timeouts | 0.000 | 0.000 | n/a |
| timeline: view=fit: all 1000 clips, clips=1000 | update_p95_ms | 3.767 | 3.846 | +2.1% |
| timeline: view=zoomed, scrolling, clips=1000 | update_p95_ms | 2.768 | 2.724 | -1.6% |
| export: scene=h264, format=H.264 + AAC (MP4) | fps | 16.325 | 15.954 | -2.3% |
| export: scene=h264_fx, format=H.264 + AAC, 3 layers + effects | fps | 6.551 | 6.478 | -1.1% |
| export: scene=prores, format=ProRes 422 (MOV) | fps | 17.236 | 16.986 | -1.5% |
| project: what=save (file.saveAs), clips=5000 | p50_ms | 16.899 | 15.959 | -5.6% |
| project: what=open (file.open), clips=5000 | p50_ms | 40.154 | 37.438 | -6.8% |

Raw section results, process resource measurements, hardware/software profiles, and corpus hashes are retained beside each manifest. No regression pass/fail is inferred from a single quick comparison.
