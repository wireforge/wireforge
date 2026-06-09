//! CollectionStore: file-based collection persistence with canonical
//! serialization (stable diffs, opaque IDs, Git-friendly moves).

use crate::canonical::to_canonical_json;
use crate::error::{WfError, WfResult};
use crate::model::RequestFile;
use std::path::Path;

pub trait CollectionStore {
    fn load_request(&self, path: &Path) -> WfResult<RequestFile>;
    fn save_request(&self, path: &Path, request: &RequestFile) -> WfResult<()>;
}

/// Filesystem-backed store: one canonical `.wf.json` per request, written
/// atomically (temp file + rename) to avoid partial writes.
pub struct FsCollectionStore;

impl CollectionStore for FsCollectionStore {
    fn load_request(&self, path: &Path) -> WfResult<RequestFile> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| WfError::new("WF_STORE_FILE_NOT_FOUND", e.to_string()))?;
        let request = serde_json::from_str(&text)
            .map_err(|e| WfError::new("WF_STORE_VALIDATION_FAILED", e.to_string()))?;
        Ok(request)
    }

    fn save_request(&self, path: &Path, request: &RequestFile) -> WfResult<()> {
        let json = to_canonical_json(request)
            .map_err(|e| WfError::new("WF_SERIALIZE_FAILED", e.to_string()))?;
        write_atomic(path, json.as_bytes())
    }
}

/// Write bytes to `path` atomically: write a sibling temp file, then rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> WfResult<()> {
    let dir = path
        .parent()
        .ok_or_else(|| WfError::new("WF_STORE_WRITE_FAILED", "path has no parent directory"))?;
    std::fs::create_dir_all(dir)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    let tmp = path.with_extension("wf-tmp");
    std::fs::write(&tmp, bytes)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| WfError::new("WF_STORE_WRITE_FAILED", e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Auth, Body, HttpMethod, KeyValue};

    fn sample() -> RequestFile {
        RequestFile {
            format: "wireforge.request".to_string(),
            version: 1,
            id: "req_sample".to_string(),
            name: "List users".to_string(),
            description: None,
            method: HttpMethod::Get,
            url: "{{baseUrl}}/v1/users".to_string(),
            params: vec![],
            headers: vec![KeyValue {
                enabled: true,
                key: "Accept".to_string(),
                value: "application/json".to_string(),
                description: None,
            }],
            auth: Auth::Bearer {
                token: "{{apiToken}}".to_string(),
            },
            body: Body::None,
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let store = FsCollectionStore;
        let dir = std::env::temp_dir().join(crate::id::new_id("wf_test"));
        let path = dir.join("list-users.wf.json");

        store.save_request(&path, &sample()).unwrap();
        let loaded = store.load_request(&path).unwrap();

        assert_eq!(loaded.id, "req_sample");
        assert_eq!(loaded.method, HttpMethod::Get);
        assert_eq!(loaded.headers.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_is_byte_stable() {
        let store = FsCollectionStore;
        let dir = std::env::temp_dir().join(crate::id::new_id("wf_test"));
        let path = dir.join("r.wf.json");

        store.save_request(&path, &sample()).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let loaded = store.load_request(&path).unwrap();
        store.save_request(&path, &loaded).unwrap();
        let second = std::fs::read_to_string(&path).unwrap();

        assert_eq!(first, second);
        assert!(first.ends_with('\n'));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
