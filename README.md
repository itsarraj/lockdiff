# lockdiff

Diffs two snapshots of the same lockfile — exactly what a `cargo update`
or `npm update` actually changed, before you merge it. GitHub's own
dependency-update PRs give you this for a single package at a time;
nothing gives you a clean "here's everything that moved" summary for a
whole lockfile diff without reading a multi-thousand-line raw text diff.

## Usage

```bash
lockdiff old-Cargo.lock new-Cargo.lock
lockdiff old-package-lock.json new-package-lock.json
lockdiff old.lock new.lock --format npm   # force format instead of guessing from the filename
lockdiff old.lock new.lock --json
```

Exit code `1` if anything changed, `0` if the two snapshots are
identical. Format is auto-detected from the "old" file's name
(`Cargo.lock` vs. `package-lock.json`) unless `--format` overrides it.

## What it reports

Four categories, each only printed if non-empty: **Upgraded** and
**Downgraded** (a real semver comparison, not a string comparison — a
package moving from `1.9.0` to `1.10.0` is correctly an upgrade even
though `"1.10.0" < "1.9.0"` as plain text), **Added**, and **Removed**.

## Status: built and verified against realistic before/after snapshots

- **26 unit tests** (`cargo test --lib`) across `lockfile` (parsing both
  formats), `version` (real semver comparison — including the
  string-vs-numeric ordering trap above, and pre-release ordering), and
  `diff` (categorizing every combination: same version untouched and
  correctly excluded from the report, upgraded, downgraded, added,
  removed, and a package present in both with an identical version
  producing no entry at all).
- **Live-verified against the actual compiled binary and two realistic
  Cargo.lock snapshots**: one with a genuine upgrade (`serde` `1.0.200`
  → `1.0.229`), one package removed, and one new package added — ran the
  binary and confirmed the report correctly categorized all three
  changes under the right heading with exit code `1`, and that an
  unchanged package included in both files produced no output at all.

**Not done / deliberately deferred**: Yarn/pnpm/Go/Ruby lockfiles (the
same four formats `depaudit` already added in this workspace — a natural
follow-up, not attempted here to keep this pass scoped to the two most
common formats); and dependency-source changes (a package moving from
crates.io to a git fork at the same version number) — this compares
`name`+`version` pairs only, not the full resolved source/checksum.
