//! ThemeStore: loads and validates `wireforge.theme` JSON files.

use crate::error::WfResult;
use std::path::Path;

pub trait ThemeStore {
    fn load(&self, path: &Path) -> WfResult<()>;
    // TODO (v1.0): typed Theme model, required-token validation, contrast and
    // semantic-color warnings, fallback to the selected base theme.
}
