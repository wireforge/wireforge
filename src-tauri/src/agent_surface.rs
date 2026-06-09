//! AgentSurface: structured, redacted inspection for local tools and agents.
//! This is not an autonomous runtime.

use crate::error::WfResult;

pub trait AgentSurface {
    fn inspect_workspace(&self) -> WfResult<serde_json::Value>;
}
