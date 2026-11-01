# Second-PC core workflow: verified PASS

Readable replacements were supplied at the same F: results folder on 2026-11-01. These supersede the earlier zero-filled files for project/output verification; previous integrity reports are preserved, and their cause remains unknown.

The package/fixture hashes match run 40834967572, attempt 4, source a7fbcf2ffba3156bef1e175d30880670d7ff3bfc. The independent second profile is Windows 10 Enterprise x64 build 19045, Intel i5-4690 (4C/4T), Radeon RX 570 driver 31.0.21925.1001, 25,675,120,640 memory bytes (approximately 24 GiB).

The user reported portable launch, import/playback, trim/undo/redo, save/reopen, export and MSI wizard/install/uninstall PASS, with no external editing application required. They separately confirmed that both original files open, and that the test was network-connected.

Independent verification passed:

- Test.fcproj parses as the inherited filmcraft.project schema 15, generator ShotClutch 0.13.0, and loads through the exact packaged CLI without saving or editing it.
- Both stored sequences are 1920x1080 at 30 fps with 50-frame edits. Original media references use the second-PC drive path; no original was relinked or modified for inspection.
- Test.mov is ProRes, 1920x1080 at 30/1 fps, exactly 50 decoded video frames. Full video decoding completed without error.
- Audio is 48 kHz stereo PCM; independent decoding produces exactly 80,000 stereo sample frames (640,000 float-PCM bytes).

[Verification and hashes](verification.json), [reproduction](verify-return.py), [project](Test.fcproj), [manual observations](observations.md). The large returned video remains in target/qualification/second-pc-return-recheck-20261101, outside Git. Profile and observations retain their original content; network clarification is linked in the historical record.

Result: bounded second-PC core correctness PASS. The user accepted this verified result as done and passing; RT-003 is Done for this bounded qualification. [Acceptance](user-acceptance.json). This is a Windows 10 profile, not Windows 11 second-profile coverage, and it does not qualify sustained performance, dropped frames, memory, physical A/V drift or strict offline consumer installation. Those expanded gates remain open; no extra hosted CI run is required for this result.
