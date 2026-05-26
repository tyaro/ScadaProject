use scada_core::command::{ControlCommand, ControlCommandStatus};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;

use mock_driver::MockDriver;
use scada_core::driver::{
    driver_write_request_from_json_str, driver_write_response_to_json,
    raw_driver_value_from_json_str, DriverWriteResponse, RawDriverValue,
};
use scada_core::tag::{tag_value_to_json, TagValue};

#[derive(Debug, Clone)]
pub struct ValueNormalizer {
    sequence: u64,
    scan_interval_ms: u64,
    stale_after_ms: u64,
}

pub fn apply_driver_write_response(command: &mut ControlCommand, response: &DriverWriteResponse) {
    if response.accepted {
        command.transition_to(ControlCommandStatus::DriverAck);
    } else {
        command.transition_to(ControlCommandStatus::Failed);
    }
}

impl ValueNormalizer {
    pub fn new(scan_interval_ms: u64, stale_after_ms: u64) -> Self {
        Self {
            sequence: 0,
            scan_interval_ms,
            stale_after_ms,
        }
    }

    pub fn normalize(&mut self, raw: RawDriverValue, server_timestamp: &str) -> TagValue {
        self.sequence += 1;

        TagValue {
            tag_id: raw.tag_id,
            value: raw.value,
            quality: raw.quality,
            source_timestamp: raw.source_timestamp,
            server_timestamp: server_timestamp.to_string(),
            sequence: self.sequence,
            scan_interval_ms: self.scan_interval_ms,
            stale_after_ms: self.stale_after_ms,
            driver_id: raw.driver_id,
            endpoint_id: raw.endpoint_id,
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverManagerCycleConfig {
    pub mock_driver_bin: String,
    pub tag_server_url: String,
    pub project_id: String,
    pub token: Option<String>,
    pub scan_interval_ms: u64,
    pub stale_after_ms: u64,
    pub server_timestamp: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverManagerCycleResult {
    pub raw_values: usize,
    pub posted_values: usize,
    pub tag_server_status: u16,
    pub tag_server_body: String,
}

pub fn run_mock_driver_cycle(
    config: &DriverManagerCycleConfig,
) -> Result<DriverManagerCycleResult, String> {
    let mut normalizer = ValueNormalizer::new(config.scan_interval_ms, config.stale_after_ms);
    run_mock_driver_cycle_with_normalizer(config, &mut normalizer)
}

pub fn run_mock_driver_cycle_with_normalizer(
    config: &DriverManagerCycleConfig,
    normalizer: &mut ValueNormalizer,
) -> Result<DriverManagerCycleResult, String> {
    let raw_values = collect_mock_driver_values(&config.mock_driver_bin)?;
    let mut values = Vec::new();

    for raw in raw_values {
        values.push(normalizer.normalize(raw, &config.server_timestamp));
    }

    let (status, body) = post_tag_values(config, &values)?;

    Ok(DriverManagerCycleResult {
        raw_values: values.len(),
        posted_values: values.len(),
        tag_server_status: status,
        tag_server_body: body,
    })
}

pub fn collect_mock_driver_values(mock_driver_bin: &str) -> Result<Vec<RawDriverValue>, String> {
    let output = Command::new(mock_driver_bin)
        .arg("--emit-once")
        .output()
        .map_err(|error| format!("spawn mock driver {mock_driver_bin}: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "mock driver exited with status {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("mock driver stdout is not UTF-8: {error}"))?;
    let mut raw_values = Vec::new();

    for line in stdout.lines().filter(|line| !line.trim().is_empty()) {
        raw_values.push(raw_driver_value_from_json_str(line)?);
    }

    Ok(raw_values)
}

pub fn post_tag_values(
    config: &DriverManagerCycleConfig,
    values: &[TagValue],
) -> Result<(u16, String), String> {
    let endpoint = HttpEndpoint::parse(&config.tag_server_url)?;
    let values_json = values.iter().map(tag_value_to_json).collect::<Vec<_>>();
    let body = format!(
        r#"{{"project_id":{},"values":[{}]}}"#,
        json_string(&config.project_id),
        values_json.join(",")
    );
    let mut request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        endpoint.path_with("/api/v1/driver-values"),
        endpoint.host,
        body.as_bytes().len()
    );
    if let Some(token) = &config.token {
        request.push_str(&format!("x-scada-token: {token}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&body);

    let mut stream =
        TcpStream::connect((endpoint.host.as_str(), endpoint.port)).map_err(|error| {
            format!(
                "connect tag server {}:{}: {error}",
                endpoint.host, endpoint.port
            )
        })?;
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("write tag server request: {error}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| format!("read tag server response: {error}"))?;

    parse_http_response(&response)
}

pub fn run_mock_write_server(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => handle_write_stream(&mut stream)?,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

fn handle_write_stream(stream: &mut TcpStream) -> std::io::Result<()> {
    let request = match read_http_request(stream) {
        Ok(request) => request,
        Err(error) => {
            let response = http_response(400, "Bad Request", &json_error(&error.to_string()));
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }
    };

    let response = if request.method == "POST" && request.path == "/api/v1/driver-writes" {
        match driver_write_request_from_json_str(&request.body) {
            Ok(driver_request) => {
                let driver = MockDriver::new("mock-driver", "mock-endpoint");
                let driver_response = driver.write(&driver_request);
                http_response(
                    202,
                    "Accepted",
                    &driver_write_response_to_json(&driver_response),
                )
            }
            Err(error) => http_response(400, "Bad Request", &json_error(&error)),
        }
    } else if request.method == "GET" && request.path == "/health" {
        http_response(
            200,
            "OK",
            r#"{"service":"driver-manager","status":"healthy"}"#,
        )
    } else {
        http_response(404, "Not Found", &json_error("not found"))
    };

    stream.write_all(response.as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimpleHttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<SimpleHttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 1024];
    let mut header_end = None;

    while header_end.is_none() {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
    }

    let Some(header_end) = header_end else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "incomplete http request",
        ));
    };
    let header = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0);
    let expected_len = header_end + 4 + content_length;

    while buffer.len() < expected_len {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let raw = String::from_utf8_lossy(&buffer[..buffer.len().min(expected_len)]).to_string();
    parse_http_request(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

fn parse_http_request(raw: &str) -> Result<SimpleHttpRequest, String> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| "missing http header terminator".to_string())?;
    let request_line = head
        .lines()
        .next()
        .ok_or_else(|| "missing http request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing http method".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "missing http path".to_string())?;

    Ok(SimpleHttpRequest {
        method: method.to_string(),
        path: path.to_string(),
        body: body.to_string(),
    })
}

fn http_response(status: u16, reason: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    )
}

fn json_error(message: &str) -> String {
    format!(r#"{{"error":{}}}"#, json_string(message))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpEndpoint {
    host: String,
    port: u16,
    base_path: String,
}

impl HttpEndpoint {
    fn parse(input: &str) -> Result<Self, String> {
        let without_scheme = input
            .strip_prefix("http://")
            .ok_or_else(|| "tag server URL must start with http://".to_string())?;
        let (authority, path) = without_scheme
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((without_scheme, String::new()));
        if authority.is_empty() {
            return Err("tag server URL is missing host".to_string());
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let parsed_port = port
                    .parse::<u16>()
                    .map_err(|_| format!("invalid tag server port: {port}"))?;
                (host.to_string(), parsed_port)
            }
            None => (authority.to_string(), 80),
        };
        if host.is_empty() {
            return Err("tag server URL is missing host".to_string());
        }

        Ok(Self {
            host,
            port,
            base_path: path.trim_end_matches('/').to_string(),
        })
    }

    fn path_with(&self, suffix: &str) -> String {
        if self.base_path.is_empty() {
            suffix.to_string()
        } else {
            format!("{}{}", self.base_path, suffix)
        }
    }
}

fn parse_http_response(raw: &str) -> Result<(u16, String), String> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| "malformed HTTP response".to_string())?;
    let status_line = head
        .lines()
        .next()
        .ok_or_else(|| "missing HTTP status line".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "missing HTTP status code".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid HTTP status code: {error}"))?;

    Ok((status, body.to_string()))
}

fn json_string(value: &str) -> String {
    serde_json::Value::String(value.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scada_core::driver::RawDriverValue;
    use scada_core::tag::{QualityCode, TagValueData};

    #[test]
    fn normalizer_assigns_sequence_and_server_timestamp() {
        let mut normalizer = ValueNormalizer::new(1000, 3000);
        let tag_value = normalizer.normalize(
            RawDriverValue {
                tag_id: "mock.temperature.001".to_string(),
                value: TagValueData::Float(25.0),
                quality: QualityCode::Simulated,
                source_timestamp: "1970-01-01T00:00:00Z".to_string(),
                driver_id: "mock-driver".to_string(),
                endpoint_id: "mock-endpoint".to_string(),
            },
            "1970-01-01T00:00:01Z",
        );

        assert_eq!(1, tag_value.sequence);
        assert_eq!("1970-01-01T00:00:01Z", tag_value.server_timestamp);
    }

    #[test]
    fn accepted_driver_write_sets_driver_ack() {
        let mut command = ControlCommand::requested(
            "cmd-1",
            "idem-1",
            "operator",
            "mock.running.001",
            "true",
            "1970-01-01T00:00:00Z",
            3000,
        );

        apply_driver_write_response(&mut command, &DriverWriteResponse::accepted("cmd-1"));

        assert_eq!(ControlCommandStatus::DriverAck, command.status);
    }

    #[test]
    fn http_endpoint_parses_local_tag_server_url() {
        let endpoint = HttpEndpoint::parse("http://127.0.0.1:18080").expect("endpoint");

        assert_eq!("127.0.0.1", endpoint.host);
        assert_eq!(18080, endpoint.port);
        assert_eq!(
            "/api/v1/driver-values",
            endpoint.path_with("/api/v1/driver-values")
        );
    }

    #[test]
    fn parse_http_response_reads_status_and_body() {
        let (status, body) =
            parse_http_response("HTTP/1.1 202 Accepted\r\nContent-Length: 15\r\n\r\n{\"ok\":true}")
                .expect("response");

        assert_eq!(202, status);
        assert_eq!(r#"{"ok":true}"#, body);
    }

    #[test]
    fn write_request_endpoint_accepts_mock_tag() {
        let request = parse_http_request(
            "POST /api/v1/driver-writes HTTP/1.1\r\nContent-Length: 72\r\n\r\n{\"command_id\":\"cmd-1\",\"tag_id\":\"mock.running.001\",\"value\":\"true\"}",
        )
        .expect("parse request");

        assert_eq!("POST", request.method);
        assert_eq!("/api/v1/driver-writes", request.path);
    }
}
