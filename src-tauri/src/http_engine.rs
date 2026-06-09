//! HttpEngine: converts a [`UnifiedRequest`] into a [`UnifiedResponse`].
//! v0.1 implementation: `reqwest` (async).

use crate::error::{WfError, WfResult};
use crate::model::{
    ApiKeyPlacement, Auth, Body, HttpMethod, KeyValue, UnifiedRequest, UnifiedResponse,
};
use reqwest::header::CONTENT_TYPE;
use std::time::Instant;

#[allow(async_fn_in_trait)]
pub trait HttpEngine {
    async fn send(&self, request: UnifiedRequest) -> WfResult<UnifiedResponse>;
}

/// HttpEngine backed by `reqwest`.
pub struct ReqwestEngine {
    client: reqwest::Client,
}

impl ReqwestEngine {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpEngine for ReqwestEngine {
    async fn send(&self, request: UnifiedRequest) -> WfResult<UnifiedResponse> {
        let mut url = reqwest::Url::parse(&request.url)
            .map_err(|e| WfError::new("WF_REQUEST_INVALID_URL", e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            for p in request.params.iter().filter(|p| p.enabled) {
                pairs.append_pair(&p.key, &p.value);
            }
        }

        let mut builder = self.client.request(map_method(request.method), url);
        for h in request.headers.iter().filter(|h| h.enabled) {
            builder = builder.header(h.key.as_str(), h.value.as_str());
        }
        builder = apply_auth(builder, &request.auth);
        builder = apply_body(builder, &request.body)?;

        let started = Instant::now();
        let resp = builder.send().await.map_err(map_transport_error)?;

        let status = resp.status().as_u16();
        let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
        let http_version = Some(format!("{:?}", resp.version()));
        let remote_ip = resp.remote_addr().map(|addr| addr.ip().to_string());
        let headers = resp
            .headers()
            .iter()
            .map(|(name, value)| KeyValue {
                enabled: true,
                key: name.to_string(),
                value: value.to_str().unwrap_or_default().to_string(),
                description: None,
            })
            .collect();

        let bytes = resp.bytes().await.map_err(map_transport_error)?;
        let duration_ms = started.elapsed().as_millis() as u64;

        Ok(UnifiedResponse {
            status,
            status_text,
            headers,
            size: bytes.len() as u64,
            duration_ms,
            http_version,
            remote_ip,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        })
    }
}

fn map_method(method: HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
        HttpMethod::Head => reqwest::Method::HEAD,
        HttpMethod::Options => reqwest::Method::OPTIONS,
    }
}

fn apply_auth(builder: reqwest::RequestBuilder, auth: &Auth) -> reqwest::RequestBuilder {
    match auth {
        Auth::None => builder,
        Auth::Bearer { token } => builder.bearer_auth(token),
        Auth::Basic { username, password } => builder.basic_auth(username, Some(password)),
        Auth::ApiKey {
            placement,
            key,
            value,
        } => match placement {
            ApiKeyPlacement::Header => builder.header(key.as_str(), value.as_str()),
            ApiKeyPlacement::Query => builder.query(&[(key.as_str(), value.as_str())]),
        },
    }
}

fn apply_body(builder: reqwest::RequestBuilder, body: &Body) -> WfResult<reqwest::RequestBuilder> {
    let b = match body {
        Body::None => builder,
        Body::Raw { content_type, text } => builder
            .header(CONTENT_TYPE, content_type.as_str())
            .body(text.clone()),
        Body::Json { text } => builder
            .header(CONTENT_TYPE, "application/json")
            .body(text.clone()),
        Body::FormUrlEncoded { fields } => {
            let pairs: Vec<(&str, &str)> = fields
                .iter()
                .filter(|f| f.enabled)
                .map(|f| (f.key.as_str(), f.value.as_str()))
                .collect();
            builder.form(&pairs)
        }
        Body::Multipart { .. } => {
            return Err(Box::new(WfError::new(
                "WF_APP_NOT_IMPLEMENTED",
                "multipart bodies are not implemented in v0.1",
            )));
        }
        Body::Graphql { query, variables } => {
            // GraphQL is a v2 feature; send the standard POST envelope for now.
            let envelope = serde_json::json!({ "query": query, "variables": variables });
            builder
                .header(CONTENT_TYPE, "application/json")
                .body(envelope.to_string())
        }
    };
    Ok(b)
}

fn map_transport_error(e: reqwest::Error) -> Box<WfError> {
    let code = if e.is_timeout() {
        "WF_NET_TIMEOUT"
    } else if e.is_connect() {
        "WF_NET_CONNECTION_REFUSED"
    } else {
        "WF_NET_REQUEST_FAILED"
    };
    let mut err = WfError::new(code, e.to_string());
    err.retryable = true;
    Box::new(err)
}
