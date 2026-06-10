//! Secret resolution.
//!
//! A `SecretProvider` chain resolves secret values at send time without ever
//! writing them to disk. The default order is environment variables first (so
//! CI and headless runs override without a keychain) then the OS keychain.
//!
//! This module also owns send-time request resolution: it expands every
//! `{{variable}}` in a request, fails before sending if any plain variable is
//! unresolved (`WF_VAR_UNRESOLVED`) or any required secret is missing
//! (`WF_SECRET_MISSING`), and otherwise returns the fully resolved request.

use crate::environments::{self, WORKSPACE_SEGMENT};
use crate::error::{WfError, WfResult};
use crate::model::{
    Auth, Body, KeyValue, MultipartField, SecretScope, SecretsManifest, UnifiedRequest,
};
use crate::variable_resolver::{resolve, VarScopes};
use crate::workspace;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const SERVICE: &str = "wireforge";

/// Identifies one secret for a provider: workspace-namespaced and scoped to an
/// environment segment (`WORKSPACE_SEGMENT` for workspace-scoped secrets).
pub struct SecretRef<'a> {
    pub workspace_id: &'a str,
    pub environment: &'a str,
    pub name: &'a str,
}

pub trait SecretProvider {
    fn get(&self, r: &SecretRef) -> WfResult<Option<String>>;
}

/// Normalize a token for an environment-variable name: uppercase, non-alnum to
/// underscore.
fn env_token(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

/// Reads `WIREFORGE_SECRET_<ENV>_<NAME>` then falls back to
/// `WIREFORGE_SECRET_<NAME>`.
pub struct EnvVarProvider;

impl SecretProvider for EnvVarProvider {
    fn get(&self, r: &SecretRef) -> WfResult<Option<String>> {
        let name = env_token(r.name);
        if r.environment != WORKSPACE_SEGMENT {
            let scoped = format!("WIREFORGE_SECRET_{}_{}", env_token(r.environment), name);
            if let Ok(v) = std::env::var(&scoped) {
                return Ok(Some(v));
            }
        }
        Ok(std::env::var(format!("WIREFORGE_SECRET_{name}")).ok())
    }
}

/// OS keychain provider. Account is `<workspaceId>/<environment>/<name>`.
pub struct KeychainProvider;

fn keychain_account(r: &SecretRef) -> String {
    format!("{}/{}/{}", r.workspace_id, r.environment, r.name)
}

fn keychain_error(e: keyring::Error) -> Box<WfError> {
    match e {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => Box::new(
            WfError::new("WF_SECRET_KEYCHAIN_DENIED", "keychain access was denied"),
        ),
        other => Box::new(WfError::new(
            "WF_SECRET_PROVIDER_UNAVAILABLE",
            format!("keychain provider error: {other}"),
        )),
    }
}

impl SecretProvider for KeychainProvider {
    fn get(&self, r: &SecretRef) -> WfResult<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, &keychain_account(r)).map_err(keychain_error)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain_error(e)),
        }
    }
}

/// An ordered chain of providers; the first to return a value wins.
pub struct ProviderChain {
    providers: Vec<Box<dyn SecretProvider>>,
}

impl ProviderChain {
    /// The default chain: environment variables, then the OS keychain.
    pub fn default_chain() -> Self {
        ProviderChain {
            providers: vec![Box::new(EnvVarProvider), Box::new(KeychainProvider)],
        }
    }

    pub fn get(&self, r: &SecretRef) -> WfResult<Option<String>> {
        for p in &self.providers {
            if let Some(v) = p.get(r)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }
}

/// Write a secret to the OS keychain.
pub fn set_secret(workspace: &Path, environment: &str, name: &str, value: &str) -> WfResult<()> {
    let ws_id = workspace::ensure_workspace_id(workspace)?;
    let r = SecretRef {
        workspace_id: &ws_id,
        environment,
        name,
    };
    let entry = keyring::Entry::new(SERVICE, &keychain_account(&r)).map_err(keychain_error)?;
    entry.set_password(value).map_err(keychain_error)
}

/// Delete a secret from the OS keychain. A missing entry is not an error.
pub fn delete_secret(workspace: &Path, environment: &str, name: &str) -> WfResult<()> {
    let ws_id = workspace::ensure_workspace_id(workspace)?;
    let r = SecretRef {
        workspace_id: &ws_id,
        environment,
        name,
    };
    let entry = keyring::Entry::new(SERVICE, &keychain_account(&r)).map_err(keychain_error)?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_error(e)),
    }
}

/// The onboarding status of one manifest-declared secret.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStatus {
    pub name: String,
    pub environment: String,
    pub required: bool,
    pub set: bool,
    pub scope: SecretScope,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_url: Option<String>,
}

/// The environment segment a secret binds to: the workspace segment for
/// workspace-scoped secrets, otherwise the active environment (falling back to
/// the first declared environment).
fn segment_for(scope: SecretScope, declared: &[String], active_env: Option<&str>) -> String {
    match scope {
        SecretScope::Workspace => WORKSPACE_SEGMENT.to_string(),
        SecretScope::Environment => active_env
            .map(str::to_string)
            .or_else(|| declared.first().cloned())
            .unwrap_or_else(|| WORKSPACE_SEGMENT.to_string()),
    }
}

/// Report each manifest-declared secret's status against the provider chain.
pub fn secret_status(workspace: &Path, active_env: Option<&str>) -> WfResult<Vec<SecretStatus>> {
    let Some(manifest) = environments::load_manifest(workspace)? else {
        return Ok(vec![]);
    };
    let ws_id = workspace::ensure_workspace_id(workspace)?;
    let chain = ProviderChain::default_chain();

    let mut out = vec![];
    for (name, decl) in manifest.secrets {
        // Environment-scoped secrets only apply where declared.
        if decl.scope == SecretScope::Environment && !decl.environments.is_empty() {
            if let Some(env) = active_env {
                if !decl.environments.iter().any(|e| e == env) {
                    continue;
                }
            }
        }
        let environment = segment_for(decl.scope, &decl.environments, active_env);
        let set = chain
            .get(&SecretRef {
                workspace_id: &ws_id,
                environment: &environment,
                name: &name,
            })
            .unwrap_or(None)
            .is_some();
        out.push(SecretStatus {
            name,
            environment,
            required: decl.required,
            set,
            scope: decl.scope,
            description: decl.description,
            doc_url: decl.doc_url,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

// --- Send-time request resolution ---

/// Resolves every string field of a request, threading the classification sets.
struct ReqResolver<'a> {
    scopes: &'a VarScopes,
    secret_names: &'a BTreeSet<String>,
    secret_values: &'a BTreeMap<String, String>,
    redact: bool,
    used: BTreeSet<String>,
    unresolved: BTreeSet<String>,
    secrets: BTreeSet<String>,
}

impl ReqResolver<'_> {
    fn go(&mut self, s: &str) -> WfResult<String> {
        let o = resolve(
            s,
            self.scopes,
            self.secret_names,
            self.secret_values,
            self.redact,
        )?;
        self.used.extend(o.used);
        self.unresolved.extend(o.unresolved);
        self.secrets.extend(o.secrets);
        Ok(o.text)
    }

    fn kvs(&mut self, items: &[KeyValue]) -> WfResult<Vec<KeyValue>> {
        let mut out = Vec::with_capacity(items.len());
        for kv in items {
            if !kv.enabled {
                out.push(kv.clone());
                continue;
            }
            out.push(KeyValue {
                enabled: true,
                key: self.go(&kv.key)?,
                value: self.go(&kv.value)?,
                description: kv.description.clone(),
            });
        }
        Ok(out)
    }

    fn auth(&mut self, auth: &Auth) -> WfResult<Auth> {
        Ok(match auth {
            Auth::None => Auth::None,
            Auth::Bearer { token } => Auth::Bearer {
                token: self.go(token)?,
            },
            Auth::Basic { username, password } => Auth::Basic {
                username: self.go(username)?,
                password: self.go(password)?,
            },
            Auth::ApiKey {
                placement,
                key,
                value,
            } => Auth::ApiKey {
                placement: *placement,
                key: self.go(key)?,
                value: self.go(value)?,
            },
        })
    }

    fn body(&mut self, body: &Body) -> WfResult<Body> {
        Ok(match body {
            Body::None => Body::None,
            Body::Raw { content_type, text } => Body::Raw {
                content_type: content_type.clone(),
                text: self.go(text)?,
            },
            Body::Json { text } => Body::Json {
                text: self.go(text)?,
            },
            Body::FormUrlEncoded { fields } => Body::FormUrlEncoded {
                fields: self.kvs(fields)?,
            },
            Body::Multipart { fields } => {
                let mut out = Vec::with_capacity(fields.len());
                for f in fields {
                    out.push(match f {
                        MultipartField::Text {
                            enabled,
                            key,
                            value,
                        } => MultipartField::Text {
                            enabled: *enabled,
                            key: if *enabled { self.go(key)? } else { key.clone() },
                            value: if *enabled {
                                self.go(value)?
                            } else {
                                value.clone()
                            },
                        },
                        MultipartField::File { enabled, key, path } => MultipartField::File {
                            enabled: *enabled,
                            key: if *enabled { self.go(key)? } else { key.clone() },
                            path: path.clone(),
                        },
                    });
                }
                Body::Multipart { fields: out }
            }
            Body::Graphql { query, variables } => Body::Graphql {
                query: self.go(query)?,
                variables: self.go(variables)?,
            },
        })
    }

    fn request(&mut self, req: &UnifiedRequest) -> WfResult<UnifiedRequest> {
        Ok(UnifiedRequest {
            method: req.method,
            url: self.go(&req.url)?,
            params: self.kvs(&req.params)?,
            headers: self.kvs(&req.headers)?,
            auth: self.auth(&req.auth)?,
            body: self.body(&req.body)?,
        })
    }
}

fn manifest_required(manifest: &Option<SecretsManifest>, name: &str) -> bool {
    manifest
        .as_ref()
        .and_then(|m| m.secrets.get(name))
        .map(|d| d.required)
        // Env-marked secrets without a manifest entry are treated as required.
        .unwrap_or(true)
}

fn manifest_segment(
    manifest: &Option<SecretsManifest>,
    name: &str,
    active_env: Option<&str>,
) -> String {
    match manifest.as_ref().and_then(|m| m.secrets.get(name)) {
        Some(d) => segment_for(d.scope, &d.environments, active_env),
        None => active_env
            .map(str::to_string)
            .unwrap_or_else(|| WORKSPACE_SEGMENT.to_string()),
    }
}

/// Resolve a request for sending: expand variables, fetch secrets through the
/// provider chain, and fail before sending if a plain variable is unresolved or
/// a required secret is missing.
pub fn resolve_request(
    workspace: &Path,
    active_env: Option<&str>,
    req: &UnifiedRequest,
) -> WfResult<UnifiedRequest> {
    let (scopes, secret_names) = environments::build_scopes(workspace, active_env)?;

    // First pass (redacted, no secret values) only to classify references.
    let mut classify = ReqResolver {
        scopes: &scopes,
        secret_names: &secret_names,
        secret_values: &BTreeMap::new(),
        redact: true,
        used: BTreeSet::new(),
        unresolved: BTreeSet::new(),
        secrets: BTreeSet::new(),
    };
    classify.request(req)?;

    if !classify.unresolved.is_empty() {
        let names: Vec<String> = classify.unresolved.into_iter().collect();
        return Err(Box::new(
            WfError::new(
                "WF_VAR_UNRESOLVED",
                format!("unresolved variables: {}", names.join(", ")),
            )
            .with_suggested_action("openSettings")
            .with_details(serde_json::json!({ "names": names })),
        ));
    }

    // Fetch referenced secrets; collect missing required ones.
    let manifest = environments::load_manifest(workspace)?;
    let ws_id = workspace::ensure_workspace_id(workspace)?;
    let chain = ProviderChain::default_chain();

    let mut secret_values = BTreeMap::new();
    let mut missing = vec![];
    for name in &classify.secrets {
        let segment = manifest_segment(&manifest, name, active_env);
        let value = chain.get(&SecretRef {
            workspace_id: &ws_id,
            environment: &segment,
            name,
        })?;
        match value {
            Some(v) => {
                secret_values.insert(name.clone(), v);
            }
            None if manifest_required(&manifest, name) => missing.push(name.clone()),
            None => {
                // Optional and unset: substitute empty so the send proceeds.
                secret_values.insert(name.clone(), String::new());
            }
        }
    }

    if !missing.is_empty() {
        return Err(Box::new(
            WfError::new(
                "WF_SECRET_MISSING",
                format!("missing required secrets: {}", missing.join(", ")),
            )
            .with_suggested_action("setSecret")
            .with_details(serde_json::json!({ "names": missing })),
        ));
    }

    // Second pass: real substitution with the resolved secret values.
    let mut apply = ReqResolver {
        scopes: &scopes,
        secret_names: &secret_names,
        secret_values: &secret_values,
        redact: false,
        used: BTreeSet::new(),
        unresolved: BTreeSet::new(),
        secrets: BTreeSet::new(),
    };
    apply.request(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiKeyPlacement, HttpMethod};

    fn req(url: &str, auth: Auth) -> UnifiedRequest {
        UnifiedRequest {
            method: HttpMethod::Get,
            url: url.to_string(),
            params: vec![],
            headers: vec![],
            auth,
            body: Body::None,
        }
    }

    fn write(path: &Path, json: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    }

    fn temp_ws() -> std::path::PathBuf {
        std::env::temp_dir().join(crate::id::new_id("wf_sec"))
    }

    #[test]
    fn env_var_provider_prefers_scoped_then_falls_back() {
        // Unique names so concurrent tests do not collide on process env.
        let scoped = "WIREFORGE_SECRET_STG_TOK_A";
        let bare = "WIREFORGE_SECRET_TOK_A";
        std::env::set_var(scoped, "scoped-val");
        std::env::set_var(bare, "bare-val");

        let p = EnvVarProvider;
        let got = p
            .get(&SecretRef {
                workspace_id: "ws",
                environment: "stg",
                name: "tok_a",
            })
            .unwrap();
        assert_eq!(got.as_deref(), Some("scoped-val"));

        std::env::remove_var(scoped);
        let got = p
            .get(&SecretRef {
                workspace_id: "ws",
                environment: "stg",
                name: "tok_a",
            })
            .unwrap();
        assert_eq!(got.as_deref(), Some("bare-val"));
        std::env::remove_var(bare);
    }

    #[test]
    fn resolve_request_substitutes_plain_and_secret_via_env_var() {
        let ws = temp_ws();
        write(
            &ws.join("wireforge.json"),
            r#"{"format":"wireforge.workspace","version":1,"id":"ws_test1","name":"W","variables":{"baseUrl":"https://api.example.com"}}"#,
        );
        write(
            &ws.join("environments").join("secrets.manifest.json"),
            r#"{"format":"wireforge.secrets","version":1,"secrets":{"apiToken":{"description":"d","required":true,"scope":"workspace"}}}"#,
        );
        std::env::set_var("WIREFORGE_SECRET_APITOKEN", "live-token");

        let r = req(
            "{{baseUrl}}/users",
            Auth::Bearer {
                token: "{{apiToken}}".to_string(),
            },
        );
        let out = resolve_request(&ws, None, &r).unwrap();
        assert_eq!(out.url, "https://api.example.com/users");
        assert!(matches!(out.auth, Auth::Bearer { token } if token == "live-token"));

        std::env::remove_var("WIREFORGE_SECRET_APITOKEN");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resolve_request_errors_on_unresolved_variable() {
        let ws = temp_ws();
        write(
            &ws.join("wireforge.json"),
            r#"{"format":"wireforge.workspace","version":1,"id":"ws_test2","name":"W","variables":{}}"#,
        );
        let r = req("https://x/{{missing}}", Auth::None);
        let err = resolve_request(&ws, None, &r).unwrap_err();
        assert_eq!(err.code, "WF_VAR_UNRESOLVED");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn resolve_request_errors_on_missing_required_secret() {
        let ws = temp_ws();
        write(
            &ws.join("wireforge.json"),
            r#"{"format":"wireforge.workspace","version":1,"id":"ws_test3","name":"W","variables":{}}"#,
        );
        write(
            &ws.join("environments").join("secrets.manifest.json"),
            r#"{"format":"wireforge.secrets","version":1,"secrets":{"missingSecret":{"description":"d","required":true,"scope":"workspace"}}}"#,
        );
        // Ensure no env var satisfies it.
        std::env::remove_var("WIREFORGE_SECRET_MISSINGSECRET");

        let r = req(
            "https://x",
            Auth::ApiKey {
                placement: ApiKeyPlacement::Header,
                key: "X-Key".to_string(),
                value: "{{missingSecret}}".to_string(),
            },
        );
        let err = resolve_request(&ws, None, &r).unwrap_err();
        assert_eq!(err.code, "WF_SECRET_MISSING");
        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn disabled_fields_do_not_block_on_unresolved() {
        let ws = temp_ws();
        write(
            &ws.join("wireforge.json"),
            r#"{"format":"wireforge.workspace","version":1,"id":"ws_test4","name":"W","variables":{}}"#,
        );
        let mut r = req("https://x", Auth::None);
        r.headers.push(KeyValue {
            enabled: false,
            key: "X-Debug".to_string(),
            value: "{{notSet}}".to_string(),
            description: None,
        });
        // Disabled header's unresolved variable must not fail the send.
        let out = resolve_request(&ws, None, &r).unwrap();
        assert_eq!(out.headers[0].value, "{{notSet}}");
        let _ = std::fs::remove_dir_all(&ws);
    }
}
