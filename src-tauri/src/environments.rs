//! Environment files and the variable-resolution context.
//!
//! Environments live in `environments/<slug>.wf.env.json`. A personal,
//! git-ignored override may sit beside one as `<slug>.local.wf.env.json`; its
//! plain values win at resolution time. `secrets.manifest.json` is the secret
//! classification authority and lives in the same directory.

use crate::error::{WfError, WfResult};
use crate::id::new_id;
use crate::model::{EnvValue, Environment, SecretScope, SecretsManifest, Workspace};
use crate::variable_resolver::VarScopes;
use crate::workspace::{self, read_json, write_json};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const ENV_EXT: &str = ".wf.env.json";
const LOCAL_EXT: &str = ".local.wf.env.json";
const MANIFEST_FILE: &str = "secrets.manifest.json";

/// A reserved environment segment for workspace-scoped secrets (keychain
/// namespacing in Chunk B).
pub const WORKSPACE_SEGMENT: &str = "_workspace";

fn environments_dir(workspace: &Path) -> PathBuf {
    workspace.join("environments")
}

fn env_path(workspace: &Path, slug: &str) -> PathBuf {
    environments_dir(workspace).join(format!("{slug}{ENV_EXT}"))
}

fn local_path(workspace: &Path, slug: &str) -> PathBuf {
    environments_dir(workspace).join(format!("{slug}{LOCAL_EXT}"))
}

/// A row for the environment switcher.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvSummary {
    pub slug: String,
    pub name: String,
    pub id: String,
    pub has_local: bool,
}

/// List environments (base files only), sorted by slug. Local overrides and the
/// manifest are excluded.
pub fn list_environments(workspace: &Path) -> WfResult<Vec<EnvSummary>> {
    let dir = environments_dir(workspace);
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = vec![];
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
    for entry in entries {
        let entry = entry.map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name == MANIFEST_FILE
            || file_name.ends_with(LOCAL_EXT)
            || !file_name.ends_with(ENV_EXT)
        {
            continue;
        }
        let slug = file_name.trim_end_matches(ENV_EXT).to_string();
        let env: Environment = read_json(&entry.path())?;
        out.push(EnvSummary {
            slug: slug.clone(),
            name: env.name,
            id: env.id,
            has_local: local_path(workspace, &slug).exists(),
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(out)
}

/// Create a new, empty environment. Returns its slug.
pub fn create_environment(workspace: &Path, name: &str) -> WfResult<String> {
    let dir = environments_dir(workspace);
    std::fs::create_dir_all(&dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    let path = workspace::unique_path(&dir, &workspace::slugify(name), ENV_EXT);
    let env = Environment {
        format: "wireforge.environment".to_string(),
        version: 1,
        id: new_id("env"),
        name: name.to_string(),
        values: Default::default(),
    };
    write_json(&path, &env)?;
    Ok(path
        .file_name()
        .map(|n| n.to_string_lossy().trim_end_matches(ENV_EXT).to_string())
        .unwrap_or_default())
}

pub fn load_environment(workspace: &Path, slug: &str) -> WfResult<Environment> {
    read_json(&env_path(workspace, slug))
}

pub fn save_environment(workspace: &Path, slug: &str, env: &Environment) -> WfResult<()> {
    write_json(&env_path(workspace, slug), env)
}

pub fn load_manifest(workspace: &Path) -> WfResult<Option<SecretsManifest>> {
    let path = environments_dir(workspace).join(MANIFEST_FILE);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_json(&path)?))
}

fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> WfResult<Option<T>> {
    if path.exists() {
        Ok(Some(read_json(path)?))
    } else {
        Ok(None)
    }
}

/// Build the resolution scopes and the set of secret-classified names for an
/// optional active environment. Secret classification combines the manifest
/// (authoritative) with environment `{ "secret": true }` markers.
pub fn build_scopes(
    workspace: &Path,
    env_slug: Option<&str>,
) -> WfResult<(VarScopes, BTreeSet<String>)> {
    let mut scopes = VarScopes::default();
    let mut secrets = BTreeSet::new();

    // Global scope from the workspace file.
    if let Some(ws) = read_optional::<Workspace>(&workspace.join("wireforge.json"))? {
        scopes.global = ws.variables;
    }

    // Collection scope.
    let col_path = workspace::collection_dir(workspace).join("collection.json");
    if let Some(col) = read_optional::<crate::model::Collection>(&col_path)? {
        scopes.collection = col.variables;
    }

    // Environment + local override scopes and their secret markers.
    if let Some(slug) = env_slug {
        if let Some(env) = read_optional::<Environment>(&env_path(workspace, slug))? {
            for (k, v) in env.values {
                match v {
                    EnvValue::Plain(s) => {
                        scopes.environment.insert(k, s);
                    }
                    EnvValue::Secret { .. } => {
                        secrets.insert(k);
                    }
                }
            }
        }
        if let Some(local) = read_optional::<Environment>(&local_path(workspace, slug))? {
            for (k, v) in local.values {
                match v {
                    EnvValue::Plain(s) => {
                        scopes.local.insert(k, s);
                    }
                    EnvValue::Secret { .. } => {
                        secrets.insert(k);
                    }
                }
            }
        }
    }

    // Manifest classification (authoritative).
    if let Some(manifest) = load_manifest(workspace)? {
        for (name, decl) in manifest.secrets {
            let applies = match decl.scope {
                SecretScope::Workspace => true,
                SecretScope::Environment => match env_slug {
                    Some(slug) => {
                        decl.environments.is_empty() || decl.environments.iter().any(|e| e == slug)
                    }
                    None => decl.environments.is_empty(),
                },
            };
            if applies {
                secrets.insert(name);
            }
        }
    }

    Ok((scopes, secrets))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SecretDecl;
    use crate::variable_resolver::resolve;
    use std::collections::{BTreeMap, BTreeSet};

    fn temp_ws() -> PathBuf {
        std::env::temp_dir().join(new_id("wf_env"))
    }

    fn write(path: &Path, json: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn create_list_load_save_round_trip() {
        let ws = temp_ws();
        let slug = create_environment(&ws, "Staging Server").unwrap();
        assert_eq!(slug, "staging-server");

        let list = list_environments(&ws).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Staging Server");
        assert!(!list[0].has_local);

        let mut env = load_environment(&ws, &slug).unwrap();
        env.values.insert(
            "baseUrl".to_string(),
            EnvValue::Plain("https://s.example.com".to_string()),
        );
        save_environment(&ws, &slug, &env).unwrap();
        assert!(matches!(
            load_environment(&ws, &slug).unwrap().values.get("baseUrl"),
            Some(EnvValue::Plain(v)) if v == "https://s.example.com"
        ));

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn build_scopes_layers_precedence_and_classifies_secrets() {
        let ws = temp_ws();

        write(
            &ws.join("wireforge.json"),
            r#"{"format":"wireforge.workspace","version":1,"name":"W","variables":{"x":"global","g":"only-global"}}"#,
        );
        write(
            &workspace::collection_dir(&ws).join("collection.json"),
            r#"{"format":"wireforge.collection","version":1,"id":"col_1","name":"M","variables":{"x":"collection"},"order":[]}"#,
        );
        write(
            &env_path(&ws, "staging"),
            r#"{"format":"wireforge.environment","version":1,"id":"env_1","name":"Staging","values":{"x":"env","baseUrl":"https://s","apiToken":{"secret":true}}}"#,
        );
        write(
            &local_path(&ws, "staging"),
            r#"{"format":"wireforge.environment","version":1,"id":"env_1","name":"Staging","values":{"x":"local"}}"#,
        );
        write(
            &ws.join("environments").join(MANIFEST_FILE),
            r#"{"format":"wireforge.secrets","version":1,"secrets":{"webhookKey":{"description":"d","scope":"workspace"},"prodOnly":{"description":"d","scope":"environment","environments":["production"]}}}"#,
        );

        let (scopes, secrets) = build_scopes(&ws, Some("staging")).unwrap();

        // Local override wins; global-only still reachable.
        let out = resolve(
            "{{x}} {{g}} {{baseUrl}}",
            &scopes,
            &secrets,
            &BTreeMap::new(),
            true,
        )
        .unwrap();
        assert_eq!(out.text, "local only-global https://s");

        // apiToken (env marker) + webhookKey (workspace manifest) are secret.
        // prodOnly is environment-scoped to "production", so not here.
        assert!(secrets.contains("apiToken"));
        assert!(secrets.contains("webhookKey"));
        assert!(!secrets.contains("prodOnly"));

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn manifest_only_classification_without_active_environment() {
        let ws = temp_ws();
        let mut secrets_map = BTreeMap::new();
        secrets_map.insert(
            "wsSecret".to_string(),
            SecretDecl {
                description: Some("d".to_string()),
                required: true,
                scope: SecretScope::Workspace,
                environments: vec![],
                provider: None,
                doc_url: None,
            },
        );
        let manifest = SecretsManifest {
            format: "wireforge.secrets".to_string(),
            version: 1,
            secrets: secrets_map,
        };
        write_json(&ws.join("environments").join(MANIFEST_FILE), &manifest).unwrap();

        let (_, secrets) = build_scopes(&ws, None).unwrap();
        let expected: BTreeSet<String> = ["wsSecret".to_string()].into_iter().collect();
        assert_eq!(secrets, expected);

        let _ = std::fs::remove_dir_all(&ws);
    }
}
