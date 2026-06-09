//! VariableResolver: resolves `{{variable}}` usages before send across the
//! global / collection / environment / local scopes. Secret-classified
//! variables are delegated to the SecretResolver and never written to files.

use crate::error::WfResult;

pub trait VariableResolver {
    fn resolve(&self, input: &str, environment: &str) -> WfResult<String>;
}
