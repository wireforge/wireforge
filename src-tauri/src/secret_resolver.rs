//! Secret resolution.
//!
//! `SecretResolver` orchestrates a chain of `SecretProvider`s with environment
//! scoping and redaction; each provider is one backend.

use crate::error::WfError;

pub trait SecretProvider {
    /// `account` is the namespaced key `<workspaceId>/<environmentId>/<name>`.
    fn get(&self, account: &str) -> Result<Option<String>, WfError>;
}

pub trait SecretResolver {
    fn resolve(&self, name: &str, environment: &str) -> Result<Option<String>, WfError>;
}
