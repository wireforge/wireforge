//! Variable resolution.
//!
//! Resolves `{{variable}}` references across the global / collection /
//! environment / local-override scopes (local wins), expands nested references
//! with cycle and depth protection, and supports dynamic variables
//! (`{{$uuid}}`, `{{$timestamp}}`, `{{$isoTimestamp}}`, `{{$randomInt}}`).
//!
//! Secret-classified variables are never expanded from a scope map. Their value
//! comes from the secret layer (v0.4 Chunk B); here they are either redacted for
//! previews or substituted from a caller-provided resolved-secret map at send
//! time. Either way the resolver reports which names were secret so the caller
//! can validate before sending.

use crate::error::{WfError, WfResult};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPTH: usize = 25;
const REDACTED: &str = "••••••";

/// The four resolution scopes, lowest to highest precedence.
#[derive(Debug, Default, Clone)]
pub struct VarScopes {
    pub global: BTreeMap<String, String>,
    pub collection: BTreeMap<String, String>,
    pub environment: BTreeMap<String, String>,
    pub local: BTreeMap<String, String>,
}

impl VarScopes {
    fn lookup(&self, name: &str) -> Option<&String> {
        self.local
            .get(name)
            .or_else(|| self.environment.get(name))
            .or_else(|| self.collection.get(name))
            .or_else(|| self.global.get(name))
    }
}

/// The result of resolving a string: the expanded text plus a classification of
/// every referenced variable. `used` is every name seen; `unresolved` is plain
/// names with no value; `secrets` is names classified as secret.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveOutcome {
    pub text: String,
    pub used: Vec<String>,
    pub unresolved: Vec<String>,
    pub secrets: Vec<String>,
}

/// Resolve `input`. `secret_names` classifies which references are secret.
/// `secret_values` supplies resolved secret values (empty during previews).
/// When `redact` is true, secret references render as a fixed mask regardless of
/// any supplied value.
pub fn resolve(
    input: &str,
    scopes: &VarScopes,
    secret_names: &BTreeSet<String>,
    secret_values: &BTreeMap<String, String>,
    redact: bool,
) -> WfResult<ResolveOutcome> {
    let mut ex = Expander {
        scopes,
        secret_names,
        secret_values,
        redact,
        used: BTreeSet::new(),
        unresolved: BTreeSet::new(),
        secrets: BTreeSet::new(),
        visiting: Vec::new(),
    };
    let text = ex.expand(input, 0)?;
    Ok(ResolveOutcome {
        text,
        used: ex.used.into_iter().collect(),
        unresolved: ex.unresolved.into_iter().collect(),
        secrets: ex.secrets.into_iter().collect(),
    })
}

struct Expander<'a> {
    scopes: &'a VarScopes,
    secret_names: &'a BTreeSet<String>,
    secret_values: &'a BTreeMap<String, String>,
    redact: bool,
    used: BTreeSet<String>,
    unresolved: BTreeSet<String>,
    secrets: BTreeSet<String>,
    visiting: Vec<String>,
}

impl Expander<'_> {
    fn expand(&mut self, input: &str, depth: usize) -> WfResult<String> {
        if depth > MAX_DEPTH {
            return Err(Box::new(WfError::new(
                "WF_VAR_DEPTH_EXCEEDED",
                "variable nesting exceeded the resolution limit",
            )));
        }

        let mut out = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else {
                // No closing braces: emit the remainder literally.
                out.push_str(&rest[start..]);
                return Ok(out);
            };
            let name = after[..end].trim().to_string();
            let tail_at = start + 2 + end + 2;

            let replacement = self.resolve_name(&name, depth)?;
            out.push_str(&replacement);
            rest = &rest[tail_at..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn resolve_name(&mut self, name: &str, depth: usize) -> WfResult<String> {
        // Empty or malformed: keep literal, do not classify.
        if name.is_empty() {
            return Ok("{{}}".to_string());
        }
        self.used.insert(name.to_string());

        // Secret classification wins over any scope value.
        if self.secret_names.contains(name) {
            self.secrets.insert(name.to_string());
            if self.redact {
                return Ok(REDACTED.to_string());
            }
            // Send-time: substitute the resolved secret literally (never recurse,
            // to avoid expanding anything contained in a secret value).
            return Ok(self
                .secret_values
                .get(name)
                .cloned()
                .unwrap_or_else(|| REDACTED.to_string()));
        }

        // Dynamic variables.
        if let Some(stripped) = name.strip_prefix('$') {
            if let Some(v) = dynamic_value(stripped) {
                return Ok(v);
            }
            // Unknown dynamic: unresolved, keep literal.
            self.unresolved.insert(name.to_string());
            return Ok(format!("{{{{{name}}}}}"));
        }

        // Plain scope lookup with cycle detection.
        match self.scopes.lookup(name) {
            Some(value) => {
                if self.visiting.iter().any(|n| n == name) {
                    return Err(Box::new(WfError::new(
                        "WF_VAR_CYCLE",
                        format!("variable references form a cycle at '{name}'"),
                    )));
                }
                let value = value.clone();
                self.visiting.push(name.to_string());
                let expanded = self.expand(&value, depth + 1)?;
                self.visiting.pop();
                Ok(expanded)
            }
            None => {
                self.unresolved.insert(name.to_string());
                Ok(format!("{{{{{name}}}}}"))
            }
        }
    }
}

fn dynamic_value(name: &str) -> Option<String> {
    match name {
        "uuid" | "guid" => Some(uuid::Uuid::new_v4().to_string()),
        "timestamp" => Some(unix_secs().to_string()),
        "isoTimestamp" => Some(iso_timestamp(unix_secs())),
        "randomInt" => Some((random_u32() % 1000).to_string()),
        _ => None,
    }
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_u32() -> u32 {
    let b = uuid::Uuid::new_v4().into_bytes();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Format a Unix timestamp as `YYYY-MM-DDTHH:MM:SSZ` (UTC) without a date crate,
/// using Howard Hinnant's `civil_from_days` algorithm.
fn iso_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn civil_from_days(z0: i64) -> (i64, u32, u32) {
    let z = z0 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(pairs: &[(&str, &str, &str)]) -> VarScopes {
        let mut s = VarScopes::default();
        for (scope, k, v) in pairs {
            let map = match *scope {
                "global" => &mut s.global,
                "collection" => &mut s.collection,
                "environment" => &mut s.environment,
                _ => &mut s.local,
            };
            map.insert(k.to_string(), v.to_string());
        }
        s
    }

    fn resolve_plain(input: &str, s: &VarScopes) -> ResolveOutcome {
        resolve(input, s, &BTreeSet::new(), &BTreeMap::new(), true).unwrap()
    }

    #[test]
    fn precedence_local_beats_environment_beats_collection_beats_global() {
        let s = scopes(&[
            ("global", "x", "g"),
            ("collection", "x", "c"),
            ("environment", "x", "e"),
            ("local", "x", "l"),
        ]);
        assert_eq!(resolve_plain("{{x}}", &s).text, "l");

        let s = scopes(&[("global", "x", "g"), ("collection", "x", "c")]);
        assert_eq!(resolve_plain("{{x}}", &s).text, "c");
    }

    #[test]
    fn nested_references_expand() {
        let s = scopes(&[
            ("environment", "base", "https://{{host}}/v1"),
            ("environment", "host", "api.example.com"),
        ]);
        let out = resolve_plain("{{base}}/users", &s);
        assert_eq!(out.text, "https://api.example.com/v1/users");
        assert!(out.unresolved.is_empty());
    }

    #[test]
    fn unresolved_is_kept_literal_and_reported() {
        let out = resolve_plain("a/{{missing}}/b", &VarScopes::default());
        assert_eq!(out.text, "a/{{missing}}/b");
        assert_eq!(out.unresolved, vec!["missing".to_string()]);
    }

    #[test]
    fn cycle_is_detected() {
        let s = scopes(&[("global", "a", "{{b}}"), ("global", "b", "{{a}}")]);
        let err = resolve("{{a}}", &s, &BTreeSet::new(), &BTreeMap::new(), true).unwrap_err();
        assert_eq!(err.code, "WF_VAR_CYCLE");
    }

    #[test]
    fn secrets_are_redacted_in_preview_and_substituted_at_send() {
        let secret_names: BTreeSet<String> = ["apiToken".to_string()].into_iter().collect();

        let out = resolve(
            "Bearer {{apiToken}}",
            &VarScopes::default(),
            &secret_names,
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert_eq!(out.text, format!("Bearer {REDACTED}"));
        assert_eq!(out.secrets, vec!["apiToken".to_string()]);
        assert!(out.unresolved.is_empty());

        let values: BTreeMap<String, String> = [("apiToken".to_string(), "t0p".to_string())]
            .into_iter()
            .collect();
        let out = resolve(
            "Bearer {{apiToken}}",
            &VarScopes::default(),
            &secret_names,
            &values,
            false,
        )
        .unwrap();
        assert_eq!(out.text, "Bearer t0p");
    }

    #[test]
    fn dynamic_variables_render() {
        let s = VarScopes::default();
        let uuid = resolve_plain("{{$uuid}}", &s).text;
        assert_eq!(uuid.len(), 36);
        assert_eq!(uuid.matches('-').count(), 4);

        assert!(resolve_plain("{{$timestamp}}", &s)
            .text
            .parse::<u64>()
            .is_ok());

        let iso = resolve_plain("{{$isoTimestamp}}", &s).text;
        assert!(iso.contains('T') && iso.ends_with('Z'), "iso: {iso}");

        let n: u32 = resolve_plain("{{$randomInt}}", &s).text.parse().unwrap();
        assert!(n < 1000);
    }

    #[test]
    fn iso_timestamp_known_value() {
        // 2021-01-01T00:00:00Z
        assert_eq!(iso_timestamp(1_609_459_200), "2021-01-01T00:00:00Z");
    }
}
