//! HttpEngine: converts a [`UnifiedRequest`] into a [`UnifiedResponse`].
//! Initial implementation target: `reqwest` (v0.1).

use crate::error::WfError;
use crate::model::{UnifiedRequest, UnifiedResponse};

pub trait HttpEngine {
    fn send(&self, request: UnifiedRequest) -> Result<UnifiedResponse, WfError>;
}
