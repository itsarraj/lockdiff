pub mod diff;
pub mod lockfile;
pub mod version;

pub use diff::{Change, ChangeKind};
pub use lockfile::Package;

/// Which lockfile format to parse. Detected from the filename by the
/// CLI (`main.rs`), but exposed here so callers (and tests) can force
/// it explicitly — useful for a fixture file that isn't literally named
/// `Cargo.lock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Cargo,
    Npm,
}

impl Format {
    /// Guesses a format from a file name: anything ending in `.json` is
    /// treated as a `package-lock.json`; everything else (`Cargo.lock`,
    /// or a fixture file with no matching extension at all) is treated
    /// as a `Cargo.lock`. There are only two supported formats, so this
    /// is deliberately a two-way default rather than a strict
    /// allowlist of exact file names.
    pub fn detect(file_name: &str) -> Format {
        if file_name.ends_with(".json") {
            Format::Npm
        } else {
            Format::Cargo
        }
    }
}

/// Parses lockfile text of a given format into its package list.
pub fn parse(content: &str, format: Format) -> Result<Vec<Package>, String> {
    match format {
        Format::Cargo => lockfile::parse_cargo_lock(content),
        Format::Npm => lockfile::parse_npm_lock(content),
    }
}

/// Parses both snapshots and diffs them in one call — the entry point
/// both the CLI and the fixture tests below use.
pub fn diff_lockfiles(
    old_content: &str,
    new_content: &str,
    format: Format,
) -> Result<Vec<Change>, String> {
    let old = parse(old_content, format)?;
    let new = parse(new_content, format)?;
    Ok(diff::diff(&old, &new))
}

#[cfg(test)]
mod fixture_tests {
    use super::*;

    const OLD_CARGO_LOCK: &str = include_str!("../fixtures/old-Cargo.lock");
    const NEW_CARGO_LOCK: &str = include_str!("../fixtures/new-Cargo.lock");

    #[test]
    fn realistic_cargo_lock_snapshots_diff_exactly_as_expected() {
        let changes = diff_lockfiles(OLD_CARGO_LOCK, NEW_CARGO_LOCK, Format::Cargo).unwrap();

        // one added (tokio), one removed (libc), two upgraded (serde, regex).
        assert_eq!(changes.len(), 4, "got: {changes:#?}");

        let by_name = |n: &str| changes.iter().find(|c| c.name == n).cloned();

        let tokio = by_name("tokio").expect("tokio should be a change");
        assert_eq!(tokio.kind, ChangeKind::Added);
        assert_eq!(tokio.new_version.as_deref(), Some("1.35.1"));

        let libc = by_name("libc").expect("libc should be a change");
        assert_eq!(libc.kind, ChangeKind::Removed);
        assert_eq!(libc.old_version.as_deref(), Some("0.2.150"));

        let serde = by_name("serde").expect("serde should be a change");
        assert_eq!(serde.kind, ChangeKind::Upgraded);
        assert_eq!(serde.old_version.as_deref(), Some("1.0.190"));
        assert_eq!(serde.new_version.as_deref(), Some("1.0.195"));

        let regex = by_name("regex").expect("regex should be a change");
        assert_eq!(regex.kind, ChangeKind::Upgraded);
        assert_eq!(regex.old_version.as_deref(), Some("1.9.0"));
        assert_eq!(regex.new_version.as_deref(), Some("1.10.2"));

        // anyhow and memchr are identical in both snapshots and must not appear.
        assert!(by_name("anyhow").is_none());
        assert!(by_name("memchr").is_none());
    }

    #[test]
    fn diffing_a_snapshot_against_itself_produces_no_changes() {
        let changes = diff_lockfiles(OLD_CARGO_LOCK, OLD_CARGO_LOCK, Format::Cargo).unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn format_detection_from_file_name() {
        assert_eq!(Format::detect("Cargo.lock"), Format::Cargo);
        assert_eq!(Format::detect("package-lock.json"), Format::Npm);
        assert_eq!(Format::detect("old.lock"), Format::Cargo);
    }
}
