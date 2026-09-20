//! A small, deliberately pragmatic version comparator — enough to tell
//! an upgrade from a downgrade for the overwhelming majority of
//! real-world Cargo/npm version strings, without pulling in a full
//! semver crate for what's ultimately a one-function need.
//!
//! Numeric dot-separated components compare numerically (`"1.9.0"` <
//! `"1.10.2"`, unlike a plain string compare); a `-prerelease` suffix
//! (`"1.0.0-beta.1"`) sorts before the same core version without one,
//! matching semver's own rule that a prerelease is "less than" its
//! final release; build metadata (`+...`) is stripped and ignored
//! entirely, as semver itself specifies. Two different prerelease
//! suffixes on the same core version fall back to a plain lexicographic
//! compare rather than semver's full prerelease-identifier algorithm —
//! a real, documented simplification.

use std::cmp::Ordering;

pub fn compare(a: &str, b: &str) -> Ordering {
    let (a_core, a_pre) = split_prerelease(a);
    let (b_core, b_pre) = split_prerelease(b);

    let a_parts = numeric_parts(a_core);
    let b_parts = numeric_parts(b_core);
    let len = a_parts.len().max(b_parts.len());
    for i in 0..len {
        let av = a_parts.get(i).copied().unwrap_or(0);
        let bv = b_parts.get(i).copied().unwrap_or(0);
        match av.cmp(&bv) {
            Ordering::Equal => continue,
            other => return other,
        }
    }

    match (a_pre.is_empty(), b_pre.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater, // a is a final release, b is a prerelease of the same core
        (false, true) => Ordering::Less,
        (false, false) => a_pre.cmp(b_pre),
    }
}

fn split_prerelease(v: &str) -> (&str, &str) {
    let v = v.split('+').next().unwrap_or(v); // drop build metadata
    match v.split_once('-') {
        Some((core, pre)) => (core, pre),
        None => (v, ""),
    }
}

fn numeric_parts(s: &str) -> Vec<u64> {
    s.split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_versions_compare_equal() {
        assert_eq!(compare("1.2.3", "1.2.3"), Ordering::Equal);
    }

    #[test]
    fn simple_patch_upgrade() {
        assert_eq!(compare("1.0.75", "1.0.76"), Ordering::Less);
    }

    #[test]
    fn numeric_not_lexicographic_comparison() {
        // A plain string compare would say "1.9.0" > "1.10.2" (since
        // '9' > '1' at the first differing character) — wrong.
        assert_eq!(compare("1.9.0", "1.10.2"), Ordering::Less);
    }

    #[test]
    fn downgrade_is_detected() {
        assert_eq!(compare("2.6.0", "2.5.0"), Ordering::Greater);
    }

    #[test]
    fn missing_trailing_components_default_to_zero() {
        assert_eq!(compare("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(compare("1.0", "1.0.1"), Ordering::Less);
    }

    #[test]
    fn prerelease_sorts_before_its_own_final_release() {
        assert_eq!(compare("1.0.0-beta.1", "1.0.0"), Ordering::Less);
        assert_eq!(compare("1.0.0", "1.0.0-beta.1"), Ordering::Greater);
    }

    #[test]
    fn build_metadata_is_ignored() {
        assert_eq!(compare("1.0.0+build1", "1.0.0+build2"), Ordering::Equal);
    }
}
