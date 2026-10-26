# User-confirmed clean installation and core workflow

Date: 2026-10-26, Asia/Bangkok. Evidence type: direct user confirmation in the implementation conversation; no new automated test run is represented by this record.

The user reported:

> Clean installation success, proceed to plan the deployment of remote CI and runtime qualification

On clarification, the user confirmed:

> 0.13.0 msi file, app launched, import/edit/save/reopen/export video is okay

Accepted result: clean MSI installation, desktop launch, import/edit, save/reopen and video export **passed by manual user confirmation** for version 0.13.0. This resolves the reported installation symptom and the previously unconfirmed clean-install/core-workflow acceptance.

Exact Windows edition/build, machine profile, tested MSI SHA-256, offline conditions, output frame/sample counts, Finish-checkbox behavior, uninstall and extended playback/audio performance were not separately supplied. These remain metadata or additional test coverage, not failed tests. The candidate MSI from the preceding delivery is identified in [the wizard package validation](msi-wizard/validation.json); the user did not independently confirm its hash.

Keep the earlier automated reports as historical records of what was known when those runs occurred. Next work: [remote CI and runtime rollout](../CI_RUNTIME_ROLLOUT.md), using CI-produced artifacts with recorded hashes and declared environments.
