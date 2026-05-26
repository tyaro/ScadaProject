use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagServerEndpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagServerClient {
    endpoint: TagServerEndpoint,
    token: Option<String>,
    timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagSnapshotRequest {
    pub project_id: String,
    pub tag_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TagSnapshot {
    pub values: Vec<RuntimeTagValue>,
    pub missing_tag_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RuntimeTagValue {
    pub tag_id: String,
    pub value: Value,
    pub data_type: String,
    pub quality: String,
    pub source_timestamp: String,
    pub server_timestamp: String,
    pub sequence: u64,
    pub scan_interval_ms: u64,
    pub stale_after_ms: u64,
    pub driver_id: String,
    pub endpoint_id: String,
    pub read_status: String,
    pub write_status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScreenDefinition {
    pub schema_version: String,
    pub screen_id: String,
    pub project_id: String,
    pub name: String,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub objects: Vec<ScreenObjectDefinition>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ScreenObjectDefinition {
    pub object_id: String,
    pub svg_asset_id: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub tag_bindings: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientResponse {
    pub status_code: u16,
    pub reason: String,
    pub body: String,
}

#[derive(Debug)]
pub enum RuntimeClientError {
    InvalidEndpoint(String),
    Io(String),
    Http(String),
    Json(String),
    ScreenDefinition(String),
    ServerStatus {
        status_code: u16,
        reason: String,
        body: String,
    },
}

impl TagServerEndpoint {
    pub fn parse(input: &str) -> Result<Self, RuntimeClientError> {
        let without_scheme = input
            .strip_prefix("http://")
            .unwrap_or(input)
            .trim_end_matches('/');

        if input.starts_with("https://") {
            return Err(RuntimeClientError::InvalidEndpoint(
                "https is not supported in the phase0 local runtime client".to_string(),
            ));
        }

        let (host, port) = without_scheme.rsplit_once(':').ok_or_else(|| {
            RuntimeClientError::InvalidEndpoint(format!("missing port in {input}"))
        })?;
        let port = port
            .parse::<u16>()
            .map_err(|_| RuntimeClientError::InvalidEndpoint(format!("invalid port in {input}")))?;

        if host.is_empty() {
            return Err(RuntimeClientError::InvalidEndpoint(format!(
                "missing host in {input}"
            )));
        }

        Ok(Self {
            host: host.to_string(),
            port,
        })
    }

    pub fn authority(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl TagServerClient {
    pub fn new(endpoint: TagServerEndpoint) -> Self {
        Self {
            endpoint,
            token: None,
            timeout: Duration::from_secs(3),
        }
    }

    pub fn from_base_url(base_url: &str) -> Result<Self, RuntimeClientError> {
        Ok(Self::new(TagServerEndpoint::parse(base_url)?))
    }

    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token.filter(|value| !value.is_empty());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn fetch_snapshot(
        &self,
        project_id: &str,
        tag_ids: &[String],
    ) -> Result<TagSnapshot, RuntimeClientError> {
        let request = TagSnapshotRequest {
            project_id: project_id.to_string(),
            tag_ids: tag_ids.to_vec(),
        };
        let body = serde_json::to_string(&request)
            .map_err(|error| RuntimeClientError::Json(error.to_string()))?;
        let raw_request = build_http_request(
            "POST",
            "/api/v1/tags/snapshot",
            &self.endpoint.authority(),
            self.token.as_deref(),
            &body,
        );
        let raw_response = self.send_raw(&raw_request)?;
        let response = parse_http_response(&raw_response)?;

        if response.status_code != 200 {
            return Err(RuntimeClientError::ServerStatus {
                status_code: response.status_code,
                reason: response.reason,
                body: response.body,
            });
        }

        serde_json::from_str(&response.body)
            .map_err(|error| RuntimeClientError::Json(error.to_string()))
    }

    fn send_raw(&self, request: &str) -> Result<String, RuntimeClientError> {
        let mut stream = TcpStream::connect(self.endpoint.authority())
            .map_err(|error| RuntimeClientError::Io(error.to_string()))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| RuntimeClientError::Io(error.to_string()))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| RuntimeClientError::Io(error.to_string()))?;
        stream
            .write_all(request.as_bytes())
            .map_err(|error| RuntimeClientError::Io(error.to_string()))?;

        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .map_err(|error| RuntimeClientError::Io(error.to_string()))?;
        Ok(response)
    }
}

impl fmt::Display for RuntimeClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(formatter, "invalid endpoint: {message}"),
            Self::Io(message) => write!(formatter, "io error: {message}"),
            Self::Http(message) => write!(formatter, "http error: {message}"),
            Self::Json(message) => write!(formatter, "json error: {message}"),
            Self::ScreenDefinition(message) => {
                write!(formatter, "screen definition error: {message}")
            }
            Self::ServerStatus {
                status_code,
                reason,
                body,
            } => write!(
                formatter,
                "tag server returned {status_code} {reason}: {body}"
            ),
        }
    }
}

impl std::error::Error for RuntimeClientError {}

pub fn default_snapshot_tags() -> Vec<String> {
    vec![
        "mock.temperature.001".to_string(),
        "mock.running.001".to_string(),
    ]
}

pub fn load_screen_definition(path: &Path) -> Result<ScreenDefinition, RuntimeClientError> {
    let raw = fs::read_to_string(path).map_err(|error| {
        RuntimeClientError::ScreenDefinition(format!("failed to read {}: {error}", path.display()))
    })?;

    serde_json::from_str(&raw).map_err(|error| {
        RuntimeClientError::ScreenDefinition(format!("failed to parse {}: {error}", path.display()))
    })
}

pub fn resolve_tag_ids_from_screen(definition: &ScreenDefinition) -> Vec<String> {
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();

    for object in &definition.objects {
        for tag_id in object.tag_bindings.values() {
            if seen.insert(tag_id.clone()) {
                resolved.push(tag_id.clone());
            }
        }
    }

    resolved
}

pub fn format_snapshot_summary(snapshot: &TagSnapshot) -> String {
    let mut lines = vec![format!(
        "snapshot values={} missing={}",
        snapshot.values.len(),
        snapshot.missing_tag_ids.len()
    )];

    for value in &snapshot.values {
        lines.push(format!(
            "{} {} {} {}",
            value.tag_id, value.data_type, value.quality, value.value
        ));
    }

    if !snapshot.missing_tag_ids.is_empty() {
        lines.push(format!("missing {}", snapshot.missing_tag_ids.join(",")));
    }

    lines.join("\n")
}

fn build_http_request(
    method: &str,
    path: &str,
    host: &str,
    token: Option<&str>,
    body: &str,
) -> String {
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.as_bytes().len()
    );

    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }

    request.push_str("\r\n");
    request.push_str(body);
    request
}

fn parse_http_response(raw: &str) -> Result<HttpClientResponse, RuntimeClientError> {
    let (head, body) = raw.split_once("\r\n\r\n").ok_or_else(|| {
        RuntimeClientError::Http("missing response header terminator".to_string())
    })?;
    let status_line = head
        .lines()
        .next()
        .ok_or_else(|| RuntimeClientError::Http("missing status line".to_string()))?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts
        .next()
        .ok_or_else(|| RuntimeClientError::Http("missing http version".to_string()))?;
    if !version.starts_with("HTTP/") {
        return Err(RuntimeClientError::Http(format!(
            "invalid http version {version}"
        )));
    }

    let status_code = parts
        .next()
        .ok_or_else(|| RuntimeClientError::Http("missing status code".to_string()))?
        .parse::<u16>()
        .map_err(|_| RuntimeClientError::Http("invalid status code".to_string()))?;
    let reason = parts.next().unwrap_or_default().to_string();

    Ok(HttpClientResponse {
        status_code,
        reason,
        body: body.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_parses_http_url() {
        let endpoint = TagServerEndpoint::parse("http://127.0.0.1:18080").expect("endpoint");

        assert_eq!("127.0.0.1", endpoint.host);
        assert_eq!(18080, endpoint.port);
        assert_eq!("127.0.0.1:18080", endpoint.authority());
    }

    #[test]
    fn http_request_includes_auth_and_content_length() {
        let request = build_http_request(
            "POST",
            "/api/v1/tags/snapshot",
            "127.0.0.1:18080",
            Some("secret"),
            r#"{"tag_ids":["mock.running.001"]}"#,
        );

        assert!(request.contains("Authorization: Bearer secret\r\n"));
        assert!(request.contains("Content-Length: 32\r\n"));
    }

    #[test]
    fn response_parser_reads_status_and_body() {
        let response = parse_http_response(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"values\":[],\"missing_tag_ids\":[]}",
        )
        .expect("response");

        assert_eq!(200, response.status_code);
        assert_eq!("OK", response.reason);
        assert_eq!(r#"{"values":[],"missing_tag_ids":[]}"#, response.body);
    }

    #[test]
    fn summary_formats_values_and_missing_tags() {
        let snapshot = TagSnapshot {
            values: vec![RuntimeTagValue {
                tag_id: "mock.running.001".to_string(),
                value: Value::Bool(true),
                data_type: "boolean".to_string(),
                quality: "Simulated".to_string(),
                source_timestamp: "1970-01-01T00:00:00Z".to_string(),
                server_timestamp: "1970-01-01T00:00:00Z".to_string(),
                sequence: 1,
                scan_interval_ms: 1000,
                stale_after_ms: 3000,
                driver_id: "mock-driver".to_string(),
                endpoint_id: "mock-endpoint".to_string(),
                read_status: "ok".to_string(),
                write_status: "idle".to_string(),
            }],
            missing_tag_ids: vec!["missing".to_string()],
        };

        let summary = format_snapshot_summary(&snapshot);

        assert!(summary.contains("snapshot values=1 missing=1"));
        assert!(summary.contains("mock.running.001 boolean Simulated true"));
        assert!(summary.contains("missing missing"));
    }

    #[test]
    fn resolve_screen_tag_ids_deduplicates_preserving_first_seen() {
        let definition = ScreenDefinition {
            schema_version: "1.0.0".to_string(),
            screen_id: "main".to_string(),
            project_id: "demo".to_string(),
            name: "Main".to_string(),
            canvas_width: 1280,
            canvas_height: 720,
            objects: vec![
                ScreenObjectDefinition {
                    object_id: "obj-1".to_string(),
                    svg_asset_id: "pump".to_string(),
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                    tag_bindings: std::collections::HashMap::from([
                        ("value".to_string(), "mock.temperature.001".to_string()),
                        ("state".to_string(), "mock.running.001".to_string()),
                    ]),
                },
                ScreenObjectDefinition {
                    object_id: "obj-2".to_string(),
                    svg_asset_id: "label".to_string(),
                    x: 120.0,
                    y: 0.0,
                    width: 100.0,
                    height: 40.0,
                    tag_bindings: std::collections::HashMap::from([(
                        "value".to_string(),
                        "mock.temperature.001".to_string(),
                    )]),
                },
            ],
        };

        let tags = resolve_tag_ids_from_screen(&definition);
        assert_eq!(
            vec![
                "mock.temperature.001".to_string(),
                "mock.running.001".to_string()
            ],
            tags
        );
    }
}
