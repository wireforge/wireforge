//! HttpEngine: converts a [`UnifiedRequest`] into a [`UnifiedResponse`].
//! Initial implementation target: reqwest (v0.1).

use crate::error::WfResult;
use crate::model::{UnifiedRequest, UnifiedResponse};

pub trait HttpEngine {
    fn send(&self, request: UnifiedRequest) -> WfResult<UnifiedResponse>;
}
