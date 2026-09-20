//! The actual diff: given the flat package lists from two lockfile
//! snapshots, work out exactly what changed per package name.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use serde::Serialize;

use crate::lockfile::Package;
use crate::version;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    Upgraded,
    Downgraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Change {
    pub name: String,
    pub kind: ChangeKind,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
}

/// Diffs two package lists. Returns changes sorted by package name,
/// then old version, then new version, for stable output.
///
/// Per package name, versions identical on both sides are matched off
/// one-for-one first (a crate resolved at both `1.0.0` and `2.0.0` in
/// both snapshots reports nothing, even though the name appears twice).
/// What's left is exactly one side's *changed* versions for that name.
/// If exactly one version remains on each side, it's a single upgrade
/// or downgrade (real-version-numeric comparison, not string
/// comparison — see `version.rs`); otherwise each leftover version is
/// reported independently as added/removed, since there's no reliable
/// way to say *which* old version a given new one replaced when
/// several changed at once for the same name.
pub fn diff(old: &[Package], new: &[Package]) -> Vec<Change> {
    let mut old_by_name: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for p in old {
        old_by_name
            .entry(p.name.as_str())
            .or_default()
            .push(p.version.as_str());
    }
    let mut new_by_name: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for p in new {
        new_by_name
            .entry(p.name.as_str())
            .or_default()
            .push(p.version.as_str());
    }

    let mut all_names: BTreeSet<&str> = BTreeSet::new();
    all_names.extend(old_by_name.keys());
    all_names.extend(new_by_name.keys());

    let mut changes = Vec::new();
    for name in all_names {
        let mut remaining_old: Vec<&str> = old_by_name.get(name).cloned().unwrap_or_default();
        let new_versions: Vec<&str> = new_by_name.get(name).cloned().unwrap_or_default();
        let mut remaining_new: Vec<&str> = Vec::new();

        for v in new_versions {
            if let Some(pos) = remaining_old.iter().position(|ov| *ov == v) {
                remaining_old.remove(pos);
            } else {
                remaining_new.push(v);
            }
        }

        if remaining_old.len() == 1 && remaining_new.len() == 1 {
            let old_v = remaining_old[0];
            let new_v = remaining_new[0];
            let kind = match version::compare(old_v, new_v) {
                Ordering::Less => ChangeKind::Upgraded,
                Ordering::Greater => ChangeKind::Downgraded,
                Ordering::Equal => continue, // different strings, same effective version
            };
            changes.push(Change {
                name: name.to_string(),
                kind,
                old_version: Some(old_v.to_string()),
                new_version: Some(new_v.to_string()),
            });
        } else {
            for old_v in remaining_old {
                changes.push(Change {
                    name: name.to_string(),
                    kind: ChangeKind::Removed,
                    old_version: Some(old_v.to_string()),
                    new_version: None,
                });
            }
            for new_v in remaining_new {
                changes.push(Change {
                    name: name.to_string(),
                    kind: ChangeKind::Added,
                    old_version: None,
                    new_version: Some(new_v.to_string()),
                });
            }
        }
    }

    changes.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.old_version.cmp(&b.old_version))
            .then(a.new_version.cmp(&b.new_version))
    });
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, version: &str) -> Package {
        Package {
            name: name.to_string(),
            version: version.to_string(),
        }
    }

    #[test]
    fn unchanged_package_produces_no_change() {
        let old = vec![pkg("anyhow", "1.0.75")];
        let new = vec![pkg("anyhow", "1.0.75")];
        assert!(diff(&old, &new).is_empty());
    }

    #[test]
    fn detects_an_added_package() {
        let old = vec![];
        let new = vec![pkg("tokio", "1.35.0")];
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].name, "tokio");
        assert_eq!(changes[0].old_version, None);
        assert_eq!(changes[0].new_version.as_deref(), Some("1.35.0"));
    }

    #[test]
    fn detects_a_removed_package() {
        let old = vec![pkg("libc", "0.2.150")];
        let new = vec![];
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Removed);
        assert_eq!(changes[0].old_version.as_deref(), Some("0.2.150"));
        assert_eq!(changes[0].new_version, None);
    }

    #[test]
    fn detects_an_upgrade() {
        let old = vec![pkg("regex", "1.9.0")];
        let new = vec![pkg("regex", "1.10.2")];
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Upgraded);
        assert_eq!(changes[0].old_version.as_deref(), Some("1.9.0"));
        assert_eq!(changes[0].new_version.as_deref(), Some("1.10.2"));
    }

    #[test]
    fn detects_a_downgrade() {
        let old = vec![pkg("memchr", "2.6.0")];
        let new = vec![pkg("memchr", "2.5.0")];
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Downgraded);
        assert_eq!(changes[0].old_version.as_deref(), Some("2.6.0"));
        assert_eq!(changes[0].new_version.as_deref(), Some("2.5.0"));
    }

    #[test]
    fn a_crate_resolved_at_two_versions_where_only_one_side_changes() {
        // syn stays at 1.0.109 on both sides, but a second 2.0.0 shows
        // up new — that should read as a plain Added for the 2.0.0
        // entry, not confuse the unrelated 1.0.109 copy.
        let old = vec![pkg("syn", "1.0.109")];
        let new = vec![pkg("syn", "1.0.109"), pkg("syn", "2.0.38")];
        let changes = diff(&old, &new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[0].new_version.as_deref(), Some("2.0.38"));
    }

    #[test]
    fn full_realistic_scenario_one_added_one_removed_two_bumped() {
        let old = vec![
            pkg("anyhow", "1.0.75"),
            pkg("serde", "1.0.190"),
            pkg("libc", "0.2.150"),
            pkg("regex", "1.9.0"),
            pkg("memchr", "2.6.0"),
        ];
        let new = vec![
            pkg("anyhow", "1.0.75"),
            pkg("serde", "1.0.195"),
            pkg("regex", "1.10.2"),
            pkg("memchr", "2.6.0"),
            pkg("tokio", "1.35.0"),
        ];
        let changes = diff(&old, &new);
        assert_eq!(
            changes.len(),
            4,
            "expected exactly 4 changes, got: {changes:?}"
        );

        let added: Vec<&Change> = changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Added)
            .collect();
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].name, "tokio");

        let removed: Vec<&Change> = changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Removed)
            .collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].name, "libc");

        let upgraded: Vec<&Change> = changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Upgraded)
            .collect();
        assert_eq!(upgraded.len(), 2);
        assert!(upgraded.iter().any(|c| c.name == "serde"
            && c.old_version.as_deref() == Some("1.0.190")
            && c.new_version.as_deref() == Some("1.0.195")));
        assert!(upgraded.iter().any(|c| c.name == "regex"
            && c.old_version.as_deref() == Some("1.9.0")
            && c.new_version.as_deref() == Some("1.10.2")));

        assert!(
            !changes
                .iter()
                .any(|c| c.name == "anyhow" || c.name == "memchr"),
            "unchanged packages must not appear"
        );
    }

    #[test]
    fn changes_are_sorted_by_name() {
        let old = vec![pkg("zeta", "1.0.0"), pkg("alpha", "1.0.0")];
        let new = vec![pkg("zeta", "2.0.0"), pkg("alpha", "2.0.0")];
        let changes = diff(&old, &new);
        assert_eq!(changes[0].name, "alpha");
        assert_eq!(changes[1].name, "zeta");
    }
}
