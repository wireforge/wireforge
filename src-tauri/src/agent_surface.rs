//! AgentSurface: structured, redacted inspection for local tools and agents.
//! This is not an autonomous runtime.

use crate::error::WfError;

pub trait AgentSurface {
    fn inspect_workspace(&self) -> Result<serde_json::Value, WfError>;
}
