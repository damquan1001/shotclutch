# Upstream maintenance policy

Upstream: `https://github.com/storytold/filmcraft`. Fork: `https://github.com/damquan1001/shotclutch`. The M0 evidence records the exact inherited revision; upstream/main is not assumed to match it.

1. Fetch explicitly and record the candidate upstream commit and its relationship to the last imported revision. Never reset the fork to upstream or apply an unattended sync.
2. Review incoming changes and select a merge or individual cherry-picks on a separate `codex/upstream-*` branch. Inspect licenses, dependency changes, schema migrations, and any security fixes.
3. Preserve ShotClutch binary names, app/installer/data identities, scoped permissions, specifications, and future schema extensions. Resolve conflicts manually and keep existing project fixtures readable. Do not import upstream branding or overwrite retained provenance.
4. Keep changes to core engine code small and attributable. The M0 patch inventory comprises product-surface naming, environment compatibility, branding/attribution, Windows packaging, fork-owned CI, and benchmark portability. Internal crate names are intentionally retained.
5. Run locked builds, formatting, layers/assets checks, relevant tests, package validation, project round trips, and comparable benchmarks after an import. Expand tests for changed media/render paths. Changes to qualified platforms or output must attach matching runtime evidence.
6. Record imported revisions, conflicts, intentional omissions, test results, benchmark changes and rollback instructions in the tracker. Roll back using a reviewed revert that preserves later user data and migrations; never downgrade saved projects silently.

Dependencies and CI actions advance through reviewed commits with exact revisions. Upstream published feature counts and benchmark claims are context, not ShotClutch acceptance evidence.
