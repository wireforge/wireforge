//! CollectionStore: file-based collection persistence with canonical
//! serialization (stable diffs, opaque IDs, Git-friendly moves).

use crate::error::WfError;
use crate::model::RequestFile;
use std::path::Path;

pub trait CollectionStore {
    fn load_request(&self, path: &Path) -> Result<RequestFile, WfError>;
    fn save_request(&self, path: &Path, request: &RequestFile) -> Result<(), WfError>;
    // TODO (v0.2): opaque ID assignment, folder ordering with merge repair,
    // example response-body spillover, deterministic migrations.
}
