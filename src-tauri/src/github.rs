//! GitHub authentication via OAuth device flow.
//!
//! The device flow needs no client secret: the app posts its (public) client id,
//! shows the user a code to enter at a verification URL, then polls for an access
//! token. The token is stored in the OS keychain (never on disk) and used by the
//! Git transport in later v0.6 chunks. github.com and GitHub Enterprise are both
//! supported via a configurable host.

use crate::error::{WfError, WfResult};
use serde::Serialize;
use serde_json::Value;

const KEYCHAIN_SERVICE: &str = "wireforge";
const USER_AGENT: &str = "wireforge";

/// Web and API base URLs for a host. github.com splits these (api.github.com);
/// Enterprise serves the API under `/api/v3`.
fn bases(host: &str) -> (String, String) {
    let host = host.trim();
    if host.is_empty() || host == "github.com" {
        (
            "https://github.com".to_string(),
            "https://api.github.com".to_string(),
        )
    } else {
        (format!("https://{host}"), format!("https://{host}/api/v3"))
    }
}

fn keychain_account(host: &str) -> String {
    let host = host.trim();
    let host = if host.is_empty() { "github.com" } else { host };
    format!("github/{host}")
}

fn net_err(msg: impl std::fmt::Display) -> Box<WfError> {
    Box::new(WfError::new(
        "WF_GITHUB_REQUEST_FAILED",
        format!("GitHub request failed: {msg}"),
    ))
}

fn http_client() -> WfResult<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(net_err)
}

// --- Keychain token storage ---

pub fn store_token(host: &str, token: &str) -> WfResult<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &keychain_account(host))
        .map_err(|e| net_err(format!("keychain: {e}")))?;
    entry
        .set_password(token)
        .map_err(|e| net_err(format!("keychain: {e}")))
}

pub fn load_token(host: &str) -> WfResult<Option<String>> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &keychain_account(host))
        .map_err(|e| net_err(format!("keychain: {e}")))?;
    match entry.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(net_err(format!("keychain: {e}"))),
    }
}

pub fn logout(host: &str) -> WfResult<()> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, &keychain_account(host))
        .map_err(|e| net_err(format!("keychain: {e}")))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(net_err(format!("keychain: {e}"))),
    }
}

// --- Public command results ---

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollOutcome {
    /// "authorized" | "pending" | "slowDown" | "denied"
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub authenticated: bool,
    pub host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

// --- Device flow ---

pub async fn device_start(host: &str, client_id: &str) -> WfResult<DeviceStart> {
    if client_id.trim().is_empty() {
        return Err(Box::new(WfError::new(
            "WF_GITHUB_TOKEN_INVALID",
            "a GitHub OAuth client id is required to sign in",
        )));
    }
    let (web, _) = bases(host);
    let resp = http_client()?
        .post(format!("{web}/login/device/code"))
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[("client_id", client_id), ("scope", "repo")])
        .send()
        .await
        .map_err(net_err)?;
    let v: Value = resp.json().await.map_err(net_err)?;

    if let Some(err) = v.get("error").and_then(Value::as_str) {
        return Err(net_err(format!("device code request rejected: {err}")));
    }
    let get = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Ok(DeviceStart {
        device_code: get("device_code").ok_or_else(|| net_err("missing device_code"))?,
        user_code: get("user_code").ok_or_else(|| net_err("missing user_code"))?,
        verification_uri: get("verification_uri")
            .ok_or_else(|| net_err("missing verification_uri"))?,
        interval: v.get("interval").and_then(Value::as_u64).unwrap_or(5),
        expires_in: v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
    })
}

/// One step of the device-flow result, parsed from the token endpoint body.
#[derive(Debug, PartialEq)]
enum PollStep {
    Authorized(String),
    Pending,
    SlowDown,
    Denied,
}

/// Pure interpretation of the token endpoint response (separated for testing).
fn interpret_poll(v: &Value) -> WfResult<PollStep> {
    if let Some(token) = v.get("access_token").and_then(Value::as_str) {
        return Ok(PollStep::Authorized(token.to_string()));
    }
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => Ok(PollStep::Pending),
        Some("slow_down") => Ok(PollStep::SlowDown),
        Some("access_denied") => Ok(PollStep::Denied),
        Some("expired_token") => Err(Box::new(WfError::new(
            "WF_GITHUB_DEVICE_FLOW_EXPIRED",
            "the device-flow code expired; start sign-in again",
        ))),
        Some(other) => Err(net_err(format!("device flow error: {other}"))),
        None => Err(net_err("unexpected token response")),
    }
}

pub async fn device_poll(host: &str, client_id: &str, device_code: &str) -> WfResult<PollOutcome> {
    let (web, _) = bases(host);
    let resp = http_client()?
        .post(format!("{web}/login/oauth/access_token"))
        .header(reqwest::header::ACCEPT, "application/json")
        .form(&[
            ("client_id", client_id),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(net_err)?;
    let v: Value = resp.json().await.map_err(net_err)?;

    match interpret_poll(&v)? {
        PollStep::Authorized(token) => {
            store_token(host, &token)?;
            let login = fetch_login(host, &token).await.ok();
            Ok(PollOutcome {
                status: "authorized".to_string(),
                login,
            })
        }
        PollStep::Pending => Ok(PollOutcome {
            status: "pending".to_string(),
            login: None,
        }),
        PollStep::SlowDown => Ok(PollOutcome {
            status: "slowDown".to_string(),
            login: None,
        }),
        PollStep::Denied => Ok(PollOutcome {
            status: "denied".to_string(),
            login: None,
        }),
    }
}

async fn fetch_login(host: &str, token: &str) -> WfResult<String> {
    let (_, api) = bases(host);
    let resp = http_client()?
        .get(format!("{api}/user"))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .bearer_auth(token)
        .send()
        .await
        .map_err(net_err)?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Box::new(WfError::new(
            "WF_GITHUB_TOKEN_INVALID",
            "the stored GitHub token is invalid",
        )));
    }
    let v: Value = resp.json().await.map_err(net_err)?;
    v.get("login")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| net_err("user response missing login"))
}

/// Report whether a token is stored and still valid; an invalid token is cleared.
pub async fn auth_status(host: &str) -> WfResult<AuthStatus> {
    let host_label = {
        let h = host.trim();
        if h.is_empty() { "github.com" } else { h }.to_string()
    };
    let Some(token) = load_token(host)? else {
        return Ok(AuthStatus {
            authenticated: false,
            host: host_label,
            login: None,
        });
    };
    match fetch_login(host, &token).await {
        Ok(login) => Ok(AuthStatus {
            authenticated: true,
            host: host_label,
            login: Some(login),
        }),
        Err(e) if e.code == "WF_GITHUB_TOKEN_INVALID" => {
            let _ = logout(host);
            Ok(AuthStatus {
                authenticated: false,
                host: host_label,
                login: None,
            })
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bases_for_dotcom_and_enterprise() {
        assert_eq!(
            bases("github.com"),
            (
                "https://github.com".to_string(),
                "https://api.github.com".to_string()
            )
        );
        assert_eq!(
            bases(""),
            (
                "https://github.com".to_string(),
                "https://api.github.com".to_string()
            )
        );
        assert_eq!(
            bases("ghe.corp.example"),
            (
                "https://ghe.corp.example".to_string(),
                "https://ghe.corp.example/api/v3".to_string()
            )
        );
    }

    #[test]
    fn interpret_poll_covers_all_states() {
        assert_eq!(
            interpret_poll(&json!({ "access_token": "tok123" })).unwrap(),
            PollStep::Authorized("tok123".to_string())
        );
        assert_eq!(
            interpret_poll(&json!({ "error": "authorization_pending" })).unwrap(),
            PollStep::Pending
        );
        assert_eq!(
            interpret_poll(&json!({ "error": "slow_down" })).unwrap(),
            PollStep::SlowDown
        );
        assert_eq!(
            interpret_poll(&json!({ "error": "access_denied" })).unwrap(),
            PollStep::Denied
        );
        assert_eq!(
            interpret_poll(&json!({ "error": "expired_token" }))
                .unwrap_err()
                .code,
            "WF_GITHUB_DEVICE_FLOW_EXPIRED"
        );
        assert!(interpret_poll(&json!({ "weird": 1 })).is_err());
    }

    #[test]
    fn keychain_account_namespaces_by_host() {
        assert_eq!(keychain_account("github.com"), "github/github.com");
        assert_eq!(keychain_account(""), "github/github.com");
        assert_eq!(keychain_account("ghe.corp"), "github/ghe.corp");
    }
}
