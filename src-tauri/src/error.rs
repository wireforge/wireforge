//! The `wireforge.error` envelope: a structured, coded error type.
//!
//! Errors carry a stable string `code` (`WF_<DOMAIN>_<SPECIFIC>`) so callers
//! branch on the code, not on the human-readable message.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorTarget {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// `kind` is a closed enum of UI affordance hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedAction {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WfError {
    pub format: String,
    pub version: u32,
    pub code: String,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ErrorTarget>,
    pub retryable: bool,
    pub confirmation_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_action: Option<SuggestedAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl WfError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            format: "wireforge.error".to_string(),
            version: 1,
            code: code.to_string(),
            severity: Severity::Error,
            message: message.into(),
            target: None,
            retryable: false,
            confirmation_required: false,
            suggested_action: None,
            details: None,
        }
    }

    /// Attach a UI affordance hint. `kind` is a closed identifier (e.g.
    /// `openSettings`, `setSecret`); the button label is derived from it.
    pub fn with_suggested_action(mut self, kind: &str) -> Self {
        let text = match kind {
            "openSettings" => "Open settings",
            "setSecret" => "Set secrets",
            "retry" => "Retry",
            other => other,
        };
        self.suggested_action = Some(SuggestedAction {
            kind: kind.to_string(),
            text: text.to_string(),
        });
        self
    }

    /// Attach structured machine-readable detail (never secret values).
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl std::fmt::Display for WfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for WfError {}

/// Convenience result type. The error is boxed so the `Ok` path stays small and
/// `Result<T, _>` does not balloon with the rich error envelope.
pub type WfResult<T> = Result<T, Box<WfError>>;
