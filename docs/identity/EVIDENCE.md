# Post-M0 identity qualification

This is identity work after completed M0, on `codex/shotclutch-identity`. The starting revision is
`9114bb6ea976bacbaac071aa364c86e6a3515d91`. M0 records, archived upstream documentation, original
project fixtures, root license notices and the upstream contribution snapshot remain unchanged.
The workspace version stays 0.13.0. Main promotion, installed applications, signing and publishing
are outside this change.

Implementation commits are `17af382d` (rename/assets/project compatibility), `2ed27bcd` (browser
aliases and harnesses), and `317e9ad2` (recovery protection and source provenance). The project
payload and schema version 15 are retained. See [the compatibility policy](../SHOTCLUTCH_IDENTITY.md).

## Checks and repairs

- All 655 external Cargo package version/source/checksum tuples match the M0 lockfile; internal
  package identities and workspace edges changed without external dependency upgrades.
- Formatting, dependency layers, asset attribution checks, seven CI helper/identity regression
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

## Remaining gates

The repaired source still needs a complete green three-platform CI run, completed installation
qualification, and an exact packaged desktop import/trim/undo/redo/save/reopen/export exercise.
Export acceptance is 50 video frames and 80,000 stereo sample frames. A retained upstream reader
must reject the new envelope, and final package inspection must confirm native associations, icon
resources, notices, installer identity and source hashes. IDENTITY-001 remains In Progress until
these results are recorded here. Hosted checks establish build/test coverage, not consumer
playback qualification on macOS or Linux.
