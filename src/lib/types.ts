// Mirrors the Rust `UnifiedRequest` / `UnifiedResponse` (serde camelCase) and
// the `wireforge.error` envelope. Kept minimal for the v0.1 request loop.

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';

export interface KeyValue {
  enabled: boolean;
  key: string;
  value: string;
  description?: string;
}

export type Body =
  | { mode: 'none' }
  | { mode: 'json'; text: string }
  | { mode: 'raw'; contentType: string; text: string }
  | { mode: 'formUrlEncoded'; fields: KeyValue[] };

export type Auth =
  | { type: 'none' }
  | { type: 'bearer'; token: string }
  | { type: 'basic'; username: string; password: string }
  | { type: 'apiKey'; placement: 'header' | 'query'; key: string; value: string };

// A request file on disk (mirrors the Rust RequestFile).
export interface RequestFile {
  format: string;
  version: number;
  id: string;
  name: string;
  description?: string;
  method: HttpMethod;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  auth: Auth;
  body: Body;
}

// A node in the collection tree (mirrors the Rust workspace::Node).
export type TreeNode =
  | { kind: 'folder'; id: string; name: string; path: string; children: TreeNode[] }
  | { kind: 'request'; id: string; name: string; method: HttpMethod; path: string };

export interface UnifiedRequest {
  method: HttpMethod;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  auth: Auth;
  body: Body;
}

export interface UnifiedResponse {
  status: number;
  statusText: string;
  headers: KeyValue[];
  size: number;
  durationMs: number;
  httpVersion?: string;
  remoteIp?: string;
  body: string;
}

export interface WfError {
  code: string;
  message: string;
  severity: string;
  retryable: boolean;
}
