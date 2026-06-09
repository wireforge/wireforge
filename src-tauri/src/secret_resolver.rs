//! Secret resolution.
//!
//! `SecretResolver` orchestrates a chain of `SecretProvider`s with environment
//! scoping and redaction; each provider is one backend.

use crate::error::WfResult;

pub trait SecretProvider {
    /// `account` is the namespaced key `<workspaceId>/<environmentId>/<name>`.
    fn get(&self, account: &str) -> WfResult<Option<String>>;
}

pub trait SecretResolver {
    fn resolve(&self, name: &str, environment: &str) -> WfResult<Option<String>>;
}
