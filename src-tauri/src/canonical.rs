//! Canonical JSON serialization for collection files: 2-space pretty, LF, a
//! trailing newline, and deterministic field/key order, so opening and saving
//! without semantic changes produces no diff.

use serde::Serialize;

pub fn to_canonical_json<T: Serialize>(value: &T) -> serde_json::Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

/// Repair a folder's `order` array against the ids actually present: keep the
/// recorded order, drop ids with no matching child, and append unknown children
/// deterministically (sorted). To merge two orders, concatenate them first.
pub fn repair_order(order: &[String], present: &[String]) -> Vec<String> {
    use std::collections::HashSet;
    let present_set: HashSet<&str> = present.iter().map(String::as_str).collect();
    let mut result: Vec<String> = Vec::new();
    let mut included: HashSet<String> = HashSet::new();
    for id in order {
        if present_set.contains(id.as_str()) && included.insert(id.clone()) {
            result.push(id.clone());
        }
    }
    let mut extra: Vec<&String> = present.iter().filter(|p| !included.contains(*p)).collect();
    extra.sort();
    result.extend(extra.into_iter().cloned());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keeps_order_and_prunes_missing() {
        let got = repair_order(&ids(&["a", "x", "b"]), &ids(&["a", "b", "c"]));
        assert_eq!(got, ids(&["a", "b", "c"]));
    }

    #[test]
    fn preserves_recorded_order() {
        let got = repair_order(&ids(&["b", "a"]), &ids(&["a", "b"]));
        assert_eq!(got, ids(&["b", "a"]));
    }

    #[test]
    fn dedupes_repeated_ids() {
        let got = repair_order(&ids(&["a", "a", "b"]), &ids(&["a", "b"]));
        assert_eq!(got, ids(&["a", "b"]));
    }

    #[test]
    fn json_has_trailing_newline_and_lf_only() {
        let s = to_canonical_json(&serde_json::json!({ "a": 1 })).unwrap();
        assert!(s.ends_with('\n'));
        assert!(!s.contains('\r'));
    }
}
