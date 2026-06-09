//! Importer / Exporter: converts external formats to and from wireforge
//! collection files. Postman v2.1 (v0.3), cURL (v1.0), OpenAPI and HAR (v1.x).

use crate::error::WfError;
use std::path::Path;

pub trait Importer {
    fn import(&self, source: &Path) -> Result<(), WfError>;
}

pub trait Exporter {
    fn export(&self, destination: &Path) -> Result<(), WfError>;
}
