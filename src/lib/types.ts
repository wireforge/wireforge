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
  | { type: 'bearer'; token: string };

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
