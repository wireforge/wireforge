//! Postman import (v0.3): Collection v2.x and Environment JSON.
//!
//! Two-phase flow: `preview` parses and reports what would be imported plus
//! warnings for anything unsupported; `apply` writes wireforge files through
//! the workspace layer. Unsupported fields are always reported, never
//! silently dropped.

use crate::error::{WfError, WfResult};
use crate::id::new_id;
use crate::model::{
    ApiKeyPlacement, Auth, Body, EnvValue, Environment, HttpMethod, KeyValue, MultipartField,
};
use crate::workspace;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

// --- Public results ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWarning {
    /// Where in the source collection the warning applies (e.g. "Users / List").
    pub path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// "collection" or "environment".
    pub kind: String,
    pub name: String,
    pub requests: u32,
    pub folders: u32,
    pub variables: u32,
    pub warnings: Vec<ImportWarning>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub kind: String,
    pub name: String,
    pub requests: u32,
    pub folders: u32,
    pub variables: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_file: Option<String>,
}

// --- Postman wire format (lenient: defaults everywhere, extra fields ignored) ---

#[derive(Deserialize)]
struct PmCollection {
    info: PmInfo,
    #[serde(default)]
    item: Vec<PmItem>,
    #[serde(default)]
    auth: Option<PmAuth>,
    #[serde(default)]
    variable: Vec<PmVariable>,
    #[serde(default)]
    event: Vec<Value>,
}

#[derive(Deserialize)]
struct PmInfo {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct PmItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    item: Option<Vec<PmItem>>,
    #[serde(default)]
    request: Option<PmRequestDef>,
    #[serde(default)]
    event: Vec<Value>,
    #[serde(default)]
    auth: Option<PmAuth>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmRequestDef {
    Url(String),
    Full(Box<PmRequest>),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmRequest {
    method: Option<String>,
    url: Option<PmUrl>,
    header: Vec<PmKv>,
    body: Option<PmBody>,
    auth: Option<PmAuth>,
    description: Option<Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum PmUrl {
    Raw(String),
    Full(PmUrlObj),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmUrlObj {
    raw: Option<String>,
    protocol: Option<String>,
    host: Option<Value>,
    path: Option<Value>,
    query: Vec<PmKv>,
    variable: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmKv {
    key: Option<String>,
    value: Option<Value>,
    disabled: bool,
    description: Option<Value>,
    #[serde(rename = "type")]
    kind: Option<String>,
    src: Option<Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmBody {
    mode: Option<String>,
    raw: Option<String>,
    urlencoded: Vec<PmKv>,
    formdata: Vec<PmKv>,
    graphql: Option<Value>,
    options: Option<Value>,
    disabled: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmAuth {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    params: BTreeMap<String, Value>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmVariable {
    key: String,
    value: Value,
    #[serde(rename = "type")]
    kind: Option<String>,
    disabled: bool,
}

#[derive(Deserialize)]
struct PmEnvironment {
    #[serde(default)]
    name: String,
    #[serde(default)]
    values: Vec<PmEnvValue>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PmEnvValue {
    key: String,
    value: Value,
    enabled: Option<bool>,
    #[serde(rename = "type")]
    kind: Option<String>,
}

// --- Conversion plan ---

enum PlanNode {
    Folder {
        name: String,
        children: Vec<PlanNode>,
    },
    Request {
        name: String,
        description: Option<String>,
        method: HttpMethod,
        url: String,
        params: Vec<KeyValue>,
        headers: Vec<KeyValue>,
        auth: Auth,
        body: Body,
    },
}

struct CollectionPlan {
    name: String,
    nodes: Vec<PlanNode>,
    variables: Vec<(String, String)>,
    warnings: Vec<ImportWarning>,
}

struct EnvironmentPlan {
    name: String,
    values: BTreeMap<String, EnvValue>,
    warnings: Vec<ImportWarning>,
}

enum Parsed {
    Collection(CollectionPlan),
    Environment(EnvironmentPlan),
}

// --- Helpers ---

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn desc_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Object(m) => m
            .get("content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn kv_to_key_value(kv: &PmKv) -> KeyValue {
    KeyValue {
        enabled: !kv.disabled,
        key: kv.key.clone().unwrap_or_default(),
        value: kv.value.as_ref().map(value_to_string).unwrap_or_default(),
        description: kv.description.as_ref().and_then(desc_to_string),
    }
}

/// Look up a named parameter in a Postman auth section, which may be either an
/// array of `{key, value}` entries (v2.1) or a plain object (older exports).
fn auth_param(auth: &PmAuth, name: &str) -> Option<String> {
    match auth.params.get(&auth.kind)? {
        Value::Array(items) => items.iter().find_map(|it| {
            let key = it.get("key").and_then(|k| k.as_str())?;
            if key == name {
                Some(value_to_string(it.get("value").unwrap_or(&Value::Null)))
            } else {
                None
            }
        }),
        Value::Object(map) => map.get(name).map(value_to_string),
        _ => None,
    }
}

fn split_query(url: &str) -> (String, Vec<KeyValue>) {
    match url.split_once('?') {
        None => (url.to_string(), vec![]),
        Some((base, q)) => {
            let params = q
                .split('&')
                .filter(|p| !p.is_empty())
                .map(|p| {
                    let (k, v) = p.split_once('=').unwrap_or((p, ""));
                    KeyValue {
                        enabled: true,
                        key: k.to_string(),
                        value: v.to_string(),
                        description: None,
                    }
                })
                .collect();
            (base.to_string(), params)
        }
    }
}

fn join_value_parts(v: &Value, sep: &str) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .filter_map(|x| x.as_str())
            .collect::<Vec<_>>()
            .join(sep),
        _ => String::new(),
    }
}

fn url_from_parts(o: &PmUrlObj) -> String {
    let host = o
        .host
        .as_ref()
        .map(|h| join_value_parts(h, "."))
        .unwrap_or_default();
    let path = o
        .path
        .as_ref()
        .map(|p| join_value_parts(p, "/"))
        .unwrap_or_default();
    let proto = o.protocol.clone().unwrap_or_else(|| "https".to_string());
    if host.is_empty() {
        path
    } else if path.is_empty() {
        format!("{proto}://{host}")
    } else {
        format!("{proto}://{host}/{path}")
    }
}

fn has_script(events: &[Value]) -> bool {
    events.iter().any(|e| e.get("script").is_some())
}

// --- Parse + convert ---

fn parse(text: &str) -> WfResult<Parsed> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| WfError::new("WF_IMPORT_PARSE_FAILED", format!("invalid JSON: {e}")))?;

    if value.get("info").is_some() && value.get("item").is_some() {
        let pm: PmCollection = serde_json::from_value(value).map_err(|e| {
            WfError::new(
                "WF_IMPORT_PARSE_FAILED",
                format!("not a Postman collection: {e}"),
            )
        })?;
        Ok(Parsed::Collection(convert_collection(pm)))
    } else if value.get("values").is_some() {
        let pm: PmEnvironment = serde_json::from_value(value).map_err(|e| {
            WfError::new(
                "WF_IMPORT_PARSE_FAILED",
                format!("not a Postman environment: {e}"),
            )
        })?;
        Ok(Parsed::Environment(convert_environment(pm)))
    } else {
        Err(Box::new(WfError::new(
            "WF_IMPORT_UNSUPPORTED_FILE",
            "file is neither a Postman collection nor a Postman environment",
        )))
    }
}

fn convert_collection(pm: PmCollection) -> CollectionPlan {
    let mut warnings = vec![];
    let name = if pm.info.name.is_empty() {
        "Imported collection".to_string()
    } else {
        pm.info.name.clone()
    };

    if has_script(&pm.event) {
        warnings.push(ImportWarning {
            path: name.clone(),
            message: "collection-level scripts are not imported".to_string(),
        });
    }

    let mut variables = vec![];
    for v in &pm.variable {
        if v.disabled {
            warnings.push(ImportWarning {
                path: name.clone(),
                message: format!("disabled variable '{}' was skipped", v.key),
            });
            continue;
        }
        if v.kind.as_deref() == Some("secret") {
            warnings.push(ImportWarning {
                path: name.clone(),
                message: format!(
                    "variable '{}' is marked secret; its value was not imported (use environment secrets)",
                    v.key
                ),
            });
            continue;
        }
        variables.push((v.key.clone(), value_to_string(&v.value)));
    }

    let nodes = convert_items(&pm.item, pm.auth.as_ref(), "", &mut warnings);
    CollectionPlan {
        name,
        nodes,
        variables,
        warnings,
    }
}

fn convert_items(
    items: &[PmItem],
    inherited_auth: Option<&PmAuth>,
    parent_path: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Vec<PlanNode> {
    let mut nodes = vec![];
    for item in items {
        let item_name = if item.name.is_empty() {
            "Untitled"
        } else {
            &item.name
        };
        let path = if parent_path.is_empty() {
            item_name.to_string()
        } else {
            format!("{parent_path} / {item_name}")
        };

        if has_script(&item.event) {
            warnings.push(ImportWarning {
                path: path.clone(),
                message: "pre-request/test scripts are not imported".to_string(),
            });
        }

        if let Some(children) = &item.item {
            let auth = item.auth.as_ref().or(inherited_auth);
            let kids = convert_items(children, auth, &path, warnings);
            nodes.push(PlanNode::Folder {
                name: item_name.to_string(),
                children: kids,
            });
        } else if let Some(req) = &item.request {
            nodes.push(convert_request(
                item_name,
                &path,
                req,
                inherited_auth,
                warnings,
            ));
        } else {
            warnings.push(ImportWarning {
                path,
                message: "item has neither a request nor children; skipped".to_string(),
            });
        }
    }
    nodes
}

fn convert_request(
    name: &str,
    path: &str,
    def: &PmRequestDef,
    inherited_auth: Option<&PmAuth>,
    warnings: &mut Vec<ImportWarning>,
) -> PlanNode {
    let owned;
    let req: &PmRequest = match def {
        PmRequestDef::Url(url) => {
            owned = PmRequest {
                url: Some(PmUrl::Raw(url.clone())),
                ..Default::default()
            };
            &owned
        }
        PmRequestDef::Full(r) => r,
    };

    // Method
    let method_str = req.method.clone().unwrap_or_else(|| "GET".to_string());
    let method = match method_str.to_uppercase().as_str() {
        "GET" => HttpMethod::Get,
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        other => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!("method '{other}' is not supported; imported as GET"),
            });
            HttpMethod::Get
        }
    };

    // URL + params
    let (url, params) = match &req.url {
        None => (String::new(), vec![]),
        Some(PmUrl::Raw(s)) => split_query(s),
        Some(PmUrl::Full(o)) => {
            if !o.variable.is_empty() {
                warnings.push(ImportWarning {
                    path: path.to_string(),
                    message: "path variables (:var) are kept as-is in the URL".to_string(),
                });
            }
            let raw = o.raw.clone().unwrap_or_else(|| url_from_parts(o));
            let (base, raw_params) = split_query(&raw);
            let params = if o.query.is_empty() {
                raw_params
            } else {
                o.query.iter().map(kv_to_key_value).collect()
            };
            (base, params)
        }
    };

    // Headers
    let headers: Vec<KeyValue> = req.header.iter().map(kv_to_key_value).collect();

    // Auth (request-level wins; otherwise nearest folder/collection auth)
    let auth = convert_auth(req.auth.as_ref().or(inherited_auth), path, warnings);

    // Body
    let body = convert_body(req.body.as_ref(), &headers, path, warnings);

    PlanNode::Request {
        name: name.to_string(),
        description: req.description.as_ref().and_then(desc_to_string),
        method,
        url,
        params,
        headers,
        auth,
        body,
    }
}

fn convert_auth(auth: Option<&PmAuth>, path: &str, warnings: &mut Vec<ImportWarning>) -> Auth {
    let Some(a) = auth else { return Auth::None };
    match a.kind.as_str() {
        "" | "noauth" => Auth::None,
        "bearer" => Auth::Bearer {
            token: auth_param(a, "token").unwrap_or_default(),
        },
        "basic" => Auth::Basic {
            username: auth_param(a, "username").unwrap_or_default(),
            password: auth_param(a, "password").unwrap_or_default(),
        },
        "apikey" => Auth::ApiKey {
            placement: if auth_param(a, "in").as_deref() == Some("query") {
                ApiKeyPlacement::Query
            } else {
                ApiKeyPlacement::Header
            },
            key: auth_param(a, "key").unwrap_or_default(),
            value: auth_param(a, "value").unwrap_or_default(),
        },
        other => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!("auth type '{other}' is not supported; set to none"),
            });
            Auth::None
        }
    }
}

fn convert_body(
    body: Option<&PmBody>,
    headers: &[KeyValue],
    path: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Body {
    let Some(b) = body else { return Body::None };
    if b.disabled {
        warnings.push(ImportWarning {
            path: path.to_string(),
            message: "body is disabled in Postman; imported as none".to_string(),
        });
        return Body::None;
    }
    match b.mode.as_deref() {
        None | Some("raw") => {
            let Some(raw) = &b.raw else { return Body::None };
            let language = b
                .options
                .as_ref()
                .and_then(|o| o.pointer("/raw/language"))
                .and_then(|l| l.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    headers
                        .iter()
                        .find(|h| h.key.eq_ignore_ascii_case("content-type"))
                        .map(|h| {
                            if h.value.contains("json") {
                                "json".to_string()
                            } else {
                                "text".to_string()
                            }
                        })
                });
            match language.as_deref() {
                Some("json") => Body::Json { text: raw.clone() },
                Some("xml") => Body::Raw {
                    content_type: "application/xml".to_string(),
                    text: raw.clone(),
                },
                Some("html") => Body::Raw {
                    content_type: "text/html".to_string(),
                    text: raw.clone(),
                },
                Some("javascript") => Body::Raw {
                    content_type: "application/javascript".to_string(),
                    text: raw.clone(),
                },
                _ => Body::Raw {
                    content_type: "text/plain".to_string(),
                    text: raw.clone(),
                },
            }
        }
        Some("urlencoded") => Body::FormUrlEncoded {
            fields: b.urlencoded.iter().map(kv_to_key_value).collect(),
        },
        Some("formdata") => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: "multipart form-data imported; the editor cannot edit it yet".to_string(),
            });
            Body::Multipart {
                fields: b
                    .formdata
                    .iter()
                    .map(|f| {
                        if f.kind.as_deref() == Some("file") {
                            MultipartField::File {
                                enabled: !f.disabled,
                                key: f.key.clone().unwrap_or_default(),
                                path: f
                                    .src
                                    .as_ref()
                                    .map(|s| join_value_parts(s, ","))
                                    .unwrap_or_default(),
                            }
                        } else {
                            MultipartField::Text {
                                enabled: !f.disabled,
                                key: f.key.clone().unwrap_or_default(),
                                value: f.value.as_ref().map(value_to_string).unwrap_or_default(),
                            }
                        }
                    })
                    .collect(),
            }
        }
        Some("graphql") => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: "GraphQL editing arrives in v2; body imported for fidelity".to_string(),
            });
            let g = b.graphql.as_ref();
            Body::Graphql {
                query: g
                    .and_then(|g| g.get("query"))
                    .map(value_to_string)
                    .unwrap_or_default(),
                variables: g
                    .and_then(|g| g.get("variables"))
                    .map(value_to_string)
                    .unwrap_or_default(),
            }
        }
        Some(other) => {
            warnings.push(ImportWarning {
                path: path.to_string(),
                message: format!("body mode '{other}' is not supported; imported as none"),
            });
            Body::None
        }
    }
}

fn convert_environment(pm: PmEnvironment) -> EnvironmentPlan {
    let mut warnings = vec![];
    let name = if pm.name.is_empty() {
        "Imported environment".to_string()
    } else {
        pm.name.clone()
    };

    let mut values = BTreeMap::new();
    for v in &pm.values {
        if v.enabled == Some(false) {
            warnings.push(ImportWarning {
                path: name.clone(),
                message: format!("disabled variable '{}' was skipped", v.key),
            });
            continue;
        }
        if v.kind.as_deref() == Some("secret") {
            warnings.push(ImportWarning {
                path: name.clone(),
                message: format!(
                    "secret '{}': value not imported; wireforge resolves secrets from the keychain",
                    v.key
                ),
            });
            values.insert(v.key.clone(), EnvValue::Secret { secret: true });
        } else {
            values.insert(v.key.clone(), EnvValue::Plain(value_to_string(&v.value)));
        }
    }

    EnvironmentPlan {
        name,
        values,
        warnings,
    }
}

fn count_nodes(nodes: &[PlanNode]) -> (u32, u32) {
    let mut folders = 0;
    let mut requests = 0;
    for n in nodes {
        match n {
            PlanNode::Folder { children, .. } => {
                folders += 1;
                let (f, r) = count_nodes(children);
                folders += f;
                requests += r;
            }
            PlanNode::Request { .. } => requests += 1,
        }
    }
    (folders, requests)
}

// --- Public API ---

pub fn preview(text: &str) -> WfResult<ImportPreview> {
    Ok(match parse(text)? {
        Parsed::Collection(plan) => {
            let (folders, requests) = count_nodes(&plan.nodes);
            ImportPreview {
                kind: "collection".to_string(),
                name: plan.name,
                requests,
                folders,
                variables: plan.variables.len() as u32,
                warnings: plan.warnings,
            }
        }
        Parsed::Environment(plan) => ImportPreview {
            kind: "environment".to_string(),
            name: plan.name,
            requests: 0,
            folders: 0,
            variables: plan.values.len() as u32,
            warnings: plan.warnings,
        },
    })
}

pub fn apply(workspace_root: &Path, text: &str) -> WfResult<ImportResult> {
    match parse(text)? {
        Parsed::Collection(plan) => apply_collection(workspace_root, plan),
        Parsed::Environment(plan) => apply_environment(workspace_root, plan),
    }
}

fn apply_collection(ws: &Path, plan: CollectionPlan) -> WfResult<ImportResult> {
    let root_rel = workspace::create_folder(ws, "", &plan.name)?;
    write_nodes(ws, &root_rel, &plan.nodes)?;
    let variables = workspace::merge_collection_variables(ws, &plan.variables)?;
    let (folders, requests) = count_nodes(&plan.nodes);
    Ok(ImportResult {
        kind: "collection".to_string(),
        name: plan.name,
        requests,
        folders,
        variables,
        root_path: Some(root_rel),
        environment_file: None,
    })
}

fn write_nodes(ws: &Path, parent_rel: &str, nodes: &[PlanNode]) -> WfResult<()> {
    for node in nodes {
        match node {
            PlanNode::Folder { name, children } => {
                let rel = workspace::create_folder(ws, parent_rel, name)?;
                write_nodes(ws, &rel, children)?;
            }
            PlanNode::Request {
                name,
                description,
                method,
                url,
                params,
                headers,
                auth,
                body,
            } => {
                let rel = workspace::create_request(ws, parent_rel, name)?;
                let mut rf = workspace::load_request_file(ws, &rel)?;
                rf.description = description.clone();
                rf.method = *method;
                rf.url = url.clone();
                rf.params = params.clone();
                rf.headers = headers.clone();
                rf.auth = auth.clone();
                rf.body = body.clone();
                workspace::save_request_file(ws, &rel, &rf)?;
            }
        }
    }
    Ok(())
}

fn apply_environment(ws: &Path, plan: EnvironmentPlan) -> WfResult<ImportResult> {
    let dir = ws.join("environments");
    std::fs::create_dir_all(&dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;

    let path = workspace::unique_path(&dir, &workspace::slugify(&plan.name), ".wf.env.json");
    let env = Environment {
        format: "wireforge.environment".to_string(),
        version: 1,
        id: new_id("env"),
        name: plan.name.clone(),
        values: plan.values.clone(),
    };
    workspace::write_json(&path, &env)?;

    let file = path
        .file_name()
        .map(|n| format!("environments/{}", n.to_string_lossy()))
        .unwrap_or_default();
    Ok(ImportResult {
        kind: "environment".to_string(),
        name: plan.name,
        requests: 0,
        folders: 0,
        variables: env.values.len() as u32,
        root_path: None,
        environment_file: Some(file),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{load_request_file, load_tree, Node};
    use std::path::PathBuf;

    fn temp_ws() -> PathBuf {
        std::env::temp_dir().join(new_id("wf_imp"))
    }

    const SAMPLE_COLLECTION: &str = r##"{
      "info": { "name": "Demo API", "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json" },
      "auth": { "type": "bearer", "bearer": [{ "key": "token", "value": "{{apiToken}}", "type": "string" }] },
      "variable": [
        { "key": "baseUrl", "value": "https://api.example.com" },
        { "key": "secretVar", "value": "x", "type": "secret" }
      ],
      "item": [
        {
          "name": "Users",
          "item": [
            {
              "name": "List users",
              "request": {
                "method": "GET",
                "header": [{ "key": "Accept", "value": "application/json" }],
                "url": {
                  "raw": "{{baseUrl}}/users?limit=10&debug=1",
                  "query": [
                    { "key": "limit", "value": "10" },
                    { "key": "debug", "value": "1", "disabled": true }
                  ]
                }
              }
            },
            {
              "name": "Create user",
              "request": {
                "method": "POST",
                "auth": { "type": "noauth" },
                "url": "{{baseUrl}}/users",
                "body": {
                  "mode": "raw",
                  "raw": "{\"name\":\"a\"}",
                  "options": { "raw": { "language": "json" } }
                }
              }
            }
          ]
        },
        {
          "name": "Login",
          "request": {
            "method": "POST",
            "auth": { "type": "basic", "basic": [
              { "key": "username", "value": "u1" },
              { "key": "password", "value": "p1" }
            ]},
            "url": "https://api.example.com/login",
            "body": { "mode": "urlencoded", "urlencoded": [
              { "key": "user", "value": "a" },
              { "key": "pass", "value": "b", "disabled": true }
            ]}
          }
        },
        {
          "name": "Upload",
          "request": {
            "method": "POST",
            "url": "https://api.example.com/upload",
            "body": { "mode": "formdata", "formdata": [
              { "key": "file", "type": "file", "src": "/tmp/a.png" },
              { "key": "note", "value": "hi" }
            ]}
          }
        },
        {
          "name": "Weird",
          "event": [{ "listen": "test", "script": { "exec": ["x"] } }],
          "request": {
            "method": "PROPFIND",
            "auth": { "type": "oauth2" },
            "url": "https://api.example.com/dav"
          }
        }
      ]
    }"##;

    const SAMPLE_ENVIRONMENT: &str = r##"{
      "name": "Staging",
      "values": [
        { "key": "baseUrl", "value": "https://staging.example.com", "enabled": true },
        { "key": "apiToken", "value": "shh", "type": "secret", "enabled": true },
        { "key": "old", "value": "gone", "enabled": false }
      ]
    }"##;

    #[test]
    fn preview_reports_counts_and_warnings() {
        let p = preview(SAMPLE_COLLECTION).unwrap();
        assert_eq!(p.kind, "collection");
        assert_eq!(p.name, "Demo API");
        assert_eq!(p.folders, 1);
        assert_eq!(p.requests, 5);
        assert_eq!(p.variables, 1); // secretVar skipped

        let all = p
            .warnings
            .iter()
            .map(|w| w.message.clone())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all.contains("secret"), "secret variable warning: {all}");
        assert!(all.contains("multipart"), "multipart warning: {all}");
        assert!(all.contains("PROPFIND"), "method warning: {all}");
        assert!(all.contains("oauth2"), "auth warning: {all}");
        assert!(all.contains("scripts"), "script warning: {all}");
    }

    #[test]
    fn convert_maps_requests_faithfully() {
        let Parsed::Collection(plan) = parse(SAMPLE_COLLECTION).unwrap() else {
            panic!("expected collection");
        };

        let PlanNode::Folder { name, children } = &plan.nodes[0] else {
            panic!("expected Users folder first");
        };
        assert_eq!(name, "Users");

        // List users: query split from URL, disabled param kept, inherited bearer.
        let PlanNode::Request {
            url, params, auth, ..
        } = &children[0]
        else {
            panic!("expected request");
        };
        assert_eq!(url, "{{baseUrl}}/users");
        assert_eq!(params.len(), 2);
        assert!(!params[1].enabled);
        assert!(matches!(auth, Auth::Bearer { token } if token == "{{apiToken}}"));

        // Create user: noauth overrides inherited; json body.
        let PlanNode::Request { auth, body, .. } = &children[1] else {
            panic!("expected request");
        };
        assert!(matches!(auth, Auth::None));
        assert!(matches!(body, Body::Json { text } if text.contains("name")));

        // Login: basic auth + urlencoded body.
        let PlanNode::Request { auth, body, .. } = &plan.nodes[1] else {
            panic!("expected request");
        };
        assert!(matches!(auth, Auth::Basic { username, .. } if username == "u1"));
        let Body::FormUrlEncoded { fields } = body else {
            panic!("expected urlencoded body");
        };
        assert_eq!(fields.len(), 2);
        assert!(!fields[1].enabled);

        // Upload: multipart with file path.
        let PlanNode::Request { body, .. } = &plan.nodes[2] else {
            panic!("expected request");
        };
        let Body::Multipart { fields } = body else {
            panic!("expected multipart body");
        };
        assert!(matches!(&fields[0], MultipartField::File { path, .. } if path == "/tmp/a.png"));

        // Weird: PROPFIND downgraded, oauth2 dropped.
        let PlanNode::Request { method, auth, .. } = &plan.nodes[3] else {
            panic!("expected request");
        };
        assert!(matches!(method, HttpMethod::Get));
        assert!(matches!(auth, Auth::None));
    }

    #[test]
    fn apply_writes_collection_files() {
        let ws = temp_ws();
        let res = apply(&ws, SAMPLE_COLLECTION).unwrap();
        assert_eq!(res.requests, 5);
        assert_eq!(res.folders, 1);
        assert_eq!(res.variables, 1);
        let root = res.root_path.unwrap();

        let tree = load_tree(&ws).unwrap();
        let Node::Folder { name, children, .. } = &tree[0] else {
            panic!("expected imported root folder");
        };
        assert_eq!(name, "Demo API");
        assert_eq!(children.len(), 4);

        let rf = load_request_file(&ws, &format!("{root}/users/list-users.wf.json")).unwrap();
        assert_eq!(rf.name, "List users");
        assert_eq!(rf.url, "{{baseUrl}}/users");
        assert_eq!(rf.params.len(), 2);
        assert!(matches!(rf.auth, Auth::Bearer { token } if token == "{{apiToken}}"));

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn apply_writes_environment_file() {
        let ws = temp_ws();
        let res = apply(&ws, SAMPLE_ENVIRONMENT).unwrap();
        assert_eq!(res.kind, "environment");
        assert_eq!(res.variables, 2); // disabled skipped

        let file = ws.join(res.environment_file.unwrap());
        let env: Environment =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(env.name, "Staging");
        assert!(
            matches!(env.values.get("baseUrl"), Some(EnvValue::Plain(v)) if v == "https://staging.example.com")
        );
        assert!(matches!(
            env.values.get("apiToken"),
            Some(EnvValue::Secret { .. })
        ));
        assert!(!env.values.contains_key("old"));

        let _ = std::fs::remove_dir_all(&ws);
    }

    #[test]
    fn unsupported_file_is_rejected() {
        let err = preview("{\"foo\": 1}").unwrap_err();
        assert_eq!(err.code, "WF_IMPORT_UNSUPPORTED_FILE");
        let err = preview("not json").unwrap_err();
        assert_eq!(err.code, "WF_IMPORT_PARSE_FAILED");
    }
}
