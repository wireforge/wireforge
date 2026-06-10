//! cURL command import.
//!
//! Parses a `curl ...` command (including Chrome/Firefox "Copy as cURL" output)
//! into a `RequestFile`. The frontend opens the result as a new request the user
//! can send or save. Best-effort: unknown flags are ignored rather than failing.

use crate::error::{WfError, WfResult};
use crate::id::new_id;
use crate::model::{Auth, Body, HttpMethod, KeyValue, MultipartField, RequestFile};

/// Shell-style tokenizer honoring single/double quotes, backslash escapes, and
/// backslash-newline line continuations.
fn tokenize(input: &str) -> Vec<String> {
    let cleaned = input.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut tokens = vec![];
    let mut cur = String::new();
    let mut has = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = cleaned.chars().peekable();

    while let Some(c) = chars.next() {
        if in_single {
            if c == '\'' {
                in_single = false;
            } else {
                cur.push(c);
            }
        } else if in_double {
            match c {
                '"' => in_double = false,
                '\\' => match chars.peek() {
                    Some(&n) if matches!(n, '"' | '\\' | '$' | '`') => {
                        cur.push(n);
                        chars.next();
                    }
                    _ => cur.push('\\'),
                },
                _ => cur.push(c),
            }
        } else {
            match c {
                '\'' => {
                    in_single = true;
                    has = true;
                }
                '"' => {
                    in_double = true;
                    has = true;
                }
                '\\' => {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                        has = true;
                    }
                }
                c if c.is_whitespace() => {
                    if has {
                        tokens.push(std::mem::take(&mut cur));
                        has = false;
                    }
                }
                _ => {
                    cur.push(c);
                    has = true;
                }
            }
        }
    }
    if has {
        tokens.push(cur);
    }
    tokens
}

fn to_method(s: &str) -> HttpMethod {
    match s.to_uppercase().as_str() {
        "POST" => HttpMethod::Post,
        "PUT" => HttpMethod::Put,
        "PATCH" => HttpMethod::Patch,
        "DELETE" => HttpMethod::Delete,
        "HEAD" => HttpMethod::Head,
        "OPTIONS" => HttpMethod::Options,
        _ => HttpMethod::Get,
    }
}

fn split_query(url: &str) -> (String, Vec<KeyValue>) {
    match url.split_once('?') {
        None => (url.to_string(), vec![]),
        Some((base, q)) => (base.to_string(), parse_pairs(q)),
    }
}

fn parse_pairs(s: &str) -> Vec<KeyValue> {
    s.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            KeyValue {
                enabled: true,
                key: k.to_string(),
                value: v.to_string(),
                description: None,
            }
        })
        .collect()
}

fn name_from_url(url: &str) -> String {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let path = after_scheme.split('?').next().unwrap_or(after_scheme);
    path.split('/')
        .rfind(|s| !s.is_empty())
        .or_else(|| after_scheme.split('/').next())
        .unwrap_or("Imported request")
        .to_string()
}

/// Parse a cURL command into a request.
pub fn parse(input: &str) -> WfResult<RequestFile> {
    let tokens = tokenize(input);
    let mut iter = tokens.iter().peekable();
    if iter.peek().map(|s| s.as_str()) == Some("curl") {
        iter.next();
    }

    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut headers: Vec<KeyValue> = vec![];
    let mut data: Vec<String> = vec![];
    let mut json_flag = false;
    let mut form: Vec<MultipartField> = vec![];
    let mut user: Option<String> = None;
    let mut get_flag = false;

    // Flags that consume the following token but that we ignore.
    let value_flags_ignored = [
        "-o",
        "--output",
        "-b",
        "--cookie",
        "-A",
        "--user-agent",
        "-e",
        "--referer",
    ];

    while let Some(tok) = iter.next() {
        let t = tok.as_str();
        match t {
            "-X" | "--request" => method = iter.next().cloned(),
            _ if t.starts_with("-X") && t.len() > 2 => method = Some(t[2..].to_string()),
            "-H" | "--header" => {
                if let Some(h) = iter.next() {
                    if let Some((k, v)) = h.split_once(':') {
                        headers.push(KeyValue {
                            enabled: true,
                            key: k.trim().to_string(),
                            value: v.trim().to_string(),
                            description: None,
                        });
                    }
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-ascii" | "--data-binary"
            | "--data-urlencode" => {
                if let Some(d) = iter.next() {
                    data.push(d.clone());
                }
            }
            "--json" => {
                if let Some(d) = iter.next() {
                    data.push(d.clone());
                    json_flag = true;
                }
            }
            "-F" | "--form" => {
                if let Some(f) = iter.next() {
                    if let Some((k, v)) = f.split_once('=') {
                        if let Some(path) = v.strip_prefix('@') {
                            form.push(MultipartField::File {
                                enabled: true,
                                key: k.to_string(),
                                path: path.to_string(),
                            });
                        } else {
                            form.push(MultipartField::Text {
                                enabled: true,
                                key: k.to_string(),
                                value: v.to_string(),
                            });
                        }
                    }
                }
            }
            "-u" | "--user" => user = iter.next().cloned(),
            "--url" => url = iter.next().cloned(),
            "-G" | "--get" => get_flag = true,
            _ if value_flags_ignored.contains(&t) => {
                iter.next();
            }
            _ if t.starts_with('-') => { /* ignore unknown/value-less flags */ }
            _ => {
                if url.is_none() {
                    url = Some(tok.clone());
                }
            }
        }
    }

    let url = url.ok_or_else(|| {
        Box::new(WfError::new(
            "WF_IMPORT_PARSE_FAILED",
            "no URL found in the cURL command",
        ))
    })?;

    // Add an implicit content-type for --json.
    if json_flag
        && !headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("content-type"))
    {
        headers.push(KeyValue {
            enabled: true,
            key: "Content-Type".to_string(),
            value: "application/json".to_string(),
            description: None,
        });
    }

    let content_type = headers
        .iter()
        .find(|h| h.key.eq_ignore_ascii_case("content-type"))
        .map(|h| h.value.to_lowercase());
    let is_json = json_flag || content_type.as_deref().is_some_and(|c| c.contains("json"));

    let (mut base, mut params) = split_query(&url);
    let joined = data.join("&");

    // -G turns data into query parameters.
    let body = if get_flag && !data.is_empty() {
        params.extend(parse_pairs(&joined));
        Body::None
    } else if !form.is_empty() {
        Body::Multipart { fields: form }
    } else if !data.is_empty() {
        if is_json {
            Body::Json { text: joined }
        } else if content_type
            .as_deref()
            .map(|c| c.contains("x-www-form-urlencoded"))
            .unwrap_or(true)
        {
            // curl -d defaults to form-urlencoded.
            Body::FormUrlEncoded {
                fields: parse_pairs(&joined),
            }
        } else {
            Body::Raw {
                content_type: content_type.unwrap_or_else(|| "text/plain".to_string()),
                text: joined,
            }
        }
    } else {
        Body::None
    };

    let method = method.unwrap_or_else(|| {
        if get_flag {
            "GET".to_string()
        } else if !data.is_empty() || matches!(body, Body::Multipart { .. }) {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });

    let auth = match user {
        Some(u) => {
            let (username, password) = u.split_once(':').unwrap_or((u.as_str(), ""));
            Auth::Basic {
                username: username.to_string(),
                password: password.to_string(),
            }
        }
        None => Auth::None,
    };

    let name = name_from_url(&base);
    if base.is_empty() {
        base = url.clone();
    }

    Ok(RequestFile {
        format: "wireforge.request".to_string(),
        version: 1,
        id: new_id("req"),
        name,
        description: None,
        method: to_method(&method),
        url: base,
        params,
        headers,
        auth,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_copy_as_curl_json_post() {
        let req = parse(
            r#"curl 'https://api.example.com/users?page=2' \
              -H 'Accept: application/json' \
              -H 'content-type: application/json' \
              -H 'Authorization: Bearer abc' \
              --data-raw '{"name":"joe"}'"#,
        )
        .unwrap();
        assert!(matches!(req.method, HttpMethod::Post));
        assert_eq!(req.url, "https://api.example.com/users");
        assert_eq!(req.params.len(), 1);
        assert_eq!(req.params[0].key, "page");
        assert!(req
            .headers
            .iter()
            .any(|h| h.key == "Authorization" && h.value == "Bearer abc"));
        assert!(matches!(req.body, Body::Json { text } if text.contains("joe")));
    }

    #[test]
    fn method_flag_and_attached_form() {
        let g = parse("curl -X DELETE https://x.test/items/1").unwrap();
        assert!(matches!(g.method, HttpMethod::Delete));
        assert_eq!(g.url, "https://x.test/items/1");

        let attached = parse("curl -XPUT https://x.test/a").unwrap();
        assert!(matches!(attached.method, HttpMethod::Put));
    }

    #[test]
    fn plain_get_and_basic_auth() {
        let g = parse("curl https://x.test/ping").unwrap();
        assert!(matches!(g.method, HttpMethod::Get));
        assert!(matches!(g.auth, Auth::None));

        let a = parse("curl -u alice:secret https://x.test/me").unwrap();
        assert!(
            matches!(a.auth, Auth::Basic { username, password } if username == "alice" && password == "secret")
        );
    }

    #[test]
    fn data_defaults_to_form_urlencoded_post() {
        let req = parse("curl -d 'a=1' -d 'b=2' https://x.test/form").unwrap();
        assert!(matches!(req.method, HttpMethod::Post));
        let Body::FormUrlEncoded { fields } = req.body else {
            panic!("expected form-urlencoded body");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].key, "a");
        assert_eq!(fields[1].value, "2");
    }

    #[test]
    fn json_flag_adds_content_type() {
        let req = parse(r#"curl --json '{"x":1}' https://x.test/j"#).unwrap();
        assert!(matches!(req.method, HttpMethod::Post));
        assert!(matches!(req.body, Body::Json { .. }));
        assert!(req
            .headers
            .iter()
            .any(|h| h.key.eq_ignore_ascii_case("content-type") && h.value.contains("json")));
    }

    #[test]
    fn multipart_form_with_file() {
        let req =
            parse("curl -F 'avatar=@/tmp/a.png' -F 'name=joe' https://x.test/upload").unwrap();
        let Body::Multipart { fields } = req.body else {
            panic!("expected multipart body");
        };
        assert!(matches!(&fields[0], MultipartField::File { path, .. } if path == "/tmp/a.png"));
        assert!(matches!(&fields[1], MultipartField::Text { value, .. } if value == "joe"));
    }

    #[test]
    fn get_flag_moves_data_to_query() {
        let req = parse("curl -G -d 'q=rust' -d 'n=5' https://x.test/search").unwrap();
        assert!(matches!(req.method, HttpMethod::Get));
        assert!(matches!(req.body, Body::None));
        assert_eq!(req.params.len(), 2);
        assert_eq!(req.params[0].key, "q");
    }

    #[test]
    fn empty_or_urlless_is_an_error() {
        assert_eq!(
            parse("curl -X POST").unwrap_err().code,
            "WF_IMPORT_PARSE_FAILED"
        );
    }
}
