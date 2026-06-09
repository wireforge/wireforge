//! Tauri command surface — a thin layer over the backend modules.

use crate::error::WfResult;
use crate::http_engine::{HttpEngine, ReqwestEngine};
use crate::model::{UnifiedRequest, UnifiedResponse};

#[tauri::command]
pub fn app_info() -> String {
    format!("wireforge {}", env!("CARGO_PKG_VERSION"))
}

#[tauri::command]
pub async fn send_request(request: UnifiedRequest) -> WfResult<UnifiedResponse> {
    ReqwestEngine::new().send(request).await
}
