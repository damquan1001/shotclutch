# ShotClutch second-PC qualification kit

Source: a7fbcf2ffba3156bef1e175d30880670d7ff3bfc. Successful CI run 40834967572, package attempt 4. This kit does not install software or start tests automatically.

1. Extract this kit into a writable folder. Compare package and fixture SHA-256 values with manifest.json using Get-FileHash. Keep the manifest with returned results.
2. In Windows PowerShell, run ./collect-profile.ps1 to capture OS, CPU, memory, GPU driver and package hashes. If local script policy blocks it, report the error; do not change system policy. The script does not collect serial numbers or credentials. Profile collection alone is not a workflow pass.
3. Extract the portable ZIP in packages. Launch shotclutch.exe and confirm the ShotClutch name. No Python, FFmpeg, external editing suite or oracle tool is needed on this PC for the manual workflow. Record any missing dependency or launch error.
4. Import fixture-1080p.mp4, create a sequence, and confirm 1920x1080 at 30 fps with audio. The fixture is 2 seconds / 60 frames with stereo audio. Play it and report picture/audio observations; this short clip does not qualify sustained playback or physical A/V drift.
5. Trim ten frames from the end (expected 50 frames), undo (60), redo (50), save a .fcproj file, close the app, reopen the saved project and confirm the edit and linked media. If exact trimming is unclear, record the actual duration instead of declaring a pass.
6. Export the whole edited sequence as ProRes MOV with 48 kHz stereo audio. Save the rendered video and project under results. Record resolution, frame rate, preview resolution, renderer/backend if visible, errors, and whether another application was required. We will independently check the returned render for 50 video frames and 80,000 stereo sample frames if the requested trim was exact.
7. MSI lifecycle is a separate optional manual test: use the supplied MSI only if this PC is suitable for installation testing. Record wizard/Finish launch, installed version, import/save/export and uninstall behavior. Keep your own projects outside the install folder. Do not uninstall FilmCraft or another existing application for this test. Do not call a network-connected test an offline consumer test.
8. Complete observations.md and return it with the profile JSON, project, and rendered video. Note which steps were not tested. Do not include passwords, tokens or private footage. Preserve package hashes so these results remain tied to this exact build.

This kit covers a bounded second-PC correctness subset. Long-GOP/VFR, sustained playback, frame-drop/resource measurements, physical A/V loopback and strict offline VM lifecycle require additional declared workloads and evidence. No pass is assumed from another PC merely being available.
