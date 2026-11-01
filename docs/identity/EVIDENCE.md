# Post-M0 identity qualification

This is identity work after completed M0, on `codex/shotclutch-identity`. The starting revision is
`9114bb6ea976bacbaac071aa364c86e6a3515d91`. M0 records, archived upstream documentation, original
project fixtures, root license notices and the upstream contribution snapshot remain unchanged.
The workspace version stays 0.13.0. Main promotion, installed applications, signing and publishing
are outside this change.

Implementation commits are `17af382d` (rename/assets/project compatibility), `2ed27bcd` (browser
aliases and harnesses), `317e9ad2` (recovery protection and source provenance), and `8f9752c5`
(regression repairs and first-Save dialog coverage). The project
payload and schema version 15 are retained. See [the compatibility policy](../SHOTCLUTCH_IDENTITY.md).

## Checks and repairs

- All 655 external Cargo package version/source/checksum tuples match the M0 lockfile; internal
  package identities and workspace edges changed without external dependency upgrades.
- Formatting, dependency layers, asset attribution checks, eight CI helper/identity regression
  guards and workspace/all-target Clippy passed locally. The format/fixture, file-save and
  autosave tests passed, including recovery of an inherited envelope under a native filename.
- The sandboxed local automation tests could not bind localhost test servers. They were stopped
  and rerun with localhost access; all 23 automation tests then passed, including equivalent
  resource aliases. No production authentication or binding restrictions were relaxed.
- Full-suite validation found stale negative directory assertions, a preview test that attempted
  to replace an occupied destination, and two old pronunciation expectations for the renamed
  product. The tests are repaired to check the platform-specific data identity, use a fresh preview
  destination, and expect the pronunciation of the current product name.
- A UI integration regression checks that Save requests a native destination for an inherited
  envelope and preserves the original bytes. The existing asynchronous dialog tests run with it.
- All 17 CLI integration cases and all five asynchronous/native-destination dialog cases passed
  locally. The new CLI regression found that inspect ignored explicit save flags; inspect now
  uses the existing save-finalization path, so conversion and legacy save refusal are enforced.
- The retained upstream executable rejected a newly written schema-15 native envelope with exit 1
  and left its bytes unchanged. [Executable/hash evidence](evidence/upstream-reader.json) records
  the local debug writer; the exact packaged desktop remains a separate gate.
- Binary asset review found inherited names in the Finder layout. It was regenerated with the
  correct volume alias/application entry and icon positions. The layout bytes repeat exactly;
  installer TIFF pages are 660 x 400/72 dpi and 1320 x 800/144 dpi. The background was visually
  checked after explicitly loading the retained Inter font. The checked binary-name guard covers
  ASCII and both UTF-16 byte orders. This correction leaves all Rust/application source unchanged
  from `8f9752c5`; the superseded CI candidate was stopped before acceptance.

## Cloud candidate record

[Candidate CI 41302003049](https://github.com/damquan1001/shotclutch/actions/runs/41302003049)
binds to `317e9ad2af0eb5606aa07caa981dde05d22558ee`. Linux and macOS release builds passed;
the compiled browser check passed object identity and command-equivalence checks for the API and
diagnostic aliases. Windows unsigned MSI/ZIP build and package inspection passed. Linux/macOS
checks exposed the stale tests described above. The Windows installation runner completed its
installed edit/export exercise but received a shutdown signal during cleanup; independent output
verification and clean uninstall acceptance are not inferred from that interrupted job.

[Packaging lint 41302005760](https://github.com/damquan1001/shotclutch/actions/runs/41302005760)
passed script parsing, helper guards, workflow/shell lint, platform metadata and installer identity
checks. Earlier preliminary CI candidates were stopped when review found harness/recovery repairs;
their partial results are not acceptance evidence.

## Completed qualification

Final implementation source is `a6229303ce34fa45fe1245722ea7346ea4d0d49e`. All 14 local check commands
passed on that clean revision: 2,644 Rust test cases passed, zero failed, and 12 existing cases were
ignored. [The command report](evidence/local-checks.json) records toolchain, applied GPU environment,
timings and actual exit codes. The additional CI helper/identity guard suite passed eight tests.
[Final-source packaging lint](https://github.com/damquan1001/shotclutch/actions/runs/41308407723)
passed. [Final CI](https://github.com/damquan1001/shotclutch/actions/runs/41308404270) completed
successfully, including the required aggregate. Windows, Linux and macOS completed all 14 commands
with 2,644, 2,660 and 2,614 passing Rust cases respectively, zero failed and 12 ignored on each.
Both release builds and the compiled browser alias check passed.
See [Windows checks](evidence/windows-checks.json), [Linux checks](evidence/linux-checks.json), [macOS checks](evidence/macos-checks.json) and
[browser aliases](evidence/browser-aliases.json).

The unsigned MSI/ZIP built and passed package inspection, including PE identity, original icon,
native association, wizard, notices and retained installer upgrade identity. The fresh hosted runner
completed installation/uninstallation with exit codes 0, registered the native extension, preserved
a user-created sentinel and verified the installed workflow's 50 video frames / 80,000 stereo sample
frames. See [installation](evidence/windows-install.json) and [installed export](evidence/installed-export.json).
This is a fresh developer runner, not a bare offline consumer environment.

The exact portable desktop independently passed generated 1920 x 1080 import, trim, undo/redo,
native save/reopen and rendered export with the same frame/sample counts. The application child
had no media oracles on PATH and inherited oracle overrides were removed. Export metadata says
ShotClutch 0.13.0. Its native project was rejected by the retained upstream reader without changing
the file. See [package/runtime hashes](evidence/package-qualification.json),
[hardware/oracle inventory](evidence/windows-runtime-profile.json), and
[upstream reader](evidence/upstream-reader.json). The selected desktop adapter was not captured;
the GPU inventory is not a claim that every listed GPU was used.

The first private desktop attempt was interrupted by the agent after save when its watcher advanced
between status checks. No pass was inferred from that attempt. The complete rerun used a fresh
directory and passed; existing applications/data and earlier artifacts were preserved.

IDENTITY-001 is Done. The [final CI job record](evidence/final-ci.json) retains every job conclusion.
Application/build/validation source is the qualified revision a6229303; subsequent commits contain
qualification documentation only. Hosted checks establish build/test coverage, not consumer playback
qualification on macOS or Linux. Earlier M0 reports remain unchanged. No main promotion, signing,
release publication, installed application modification or user-data migration was performed.
