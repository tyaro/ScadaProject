use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use scada_core::command::{ControlCommand, ControlCommandStatus};
use scada_core::driver::{
    driver_write_request_to_json, driver_write_response_from_json_str,
    driver_write_response_to_json, DriverWriteRequest, DriverWriteResponse,
};
use scada_core::mqtt::{tag_value_topic, MqttBrokerEndpoint, MqttBrokerTransport};
use scada_core::tag::{
    tag_value_from_json_value, tag_value_to_json, QualityCode, TagValue, TagValueData,
};
use serde::Serialize;
use tokio::runtime::Runtime;

#[derive(Debug, Default)]
pub struct InMemoryTagCache {
    values: HashMap<String, TagValue>,
}

#[derive(Debug, Default)]
pub struct WritePolicy {
    writable_tags: HashMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

#[derive(Debug)]
pub struct TagServerApi {
    cache: InMemoryTagCache,
    write_policy: WritePolicy,
    required_token: Option<String>,
    mqtt: Option<MqttPublishConfig>,
    driver_manager: Option<DriverManagerClientConfig>,
    operation_logs: VecDeque<OperationLogEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttPublishConfig {
    pub endpoint: MqttBrokerEndpoint,
    pub client_id: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverManagerClientConfig {
    pub base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationLogEntry {
    pub command_id: String,
    pub user_id: String,
    pub tag_id: String,
    pub requested_value: String,
    pub requested_at: String,
    pub status: String,
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OperationLogList {
    items: Vec<OperationLogEntry>,
}

const OPERATION_LOG_CAPACITY: usize = 256;

impl WritePolicy {
    pub fn new() -> Self {
        Self {
            writable_tags: HashMap::new(),
        }
    }

    pub fn allow_tag(mut self, tag_id: &str) -> Self {
        self.writable_tags.insert(tag_id.to_string(), true);
        self
    }

    pub fn validate(&self, command: &mut ControlCommand) -> Result<DriverWriteRequest, String> {
        if !self
            .writable_tags
            .get(&command.tag_id)
            .copied()
            .unwrap_or(false)
        {
            command.transition_to(ControlCommandStatus::Rejected);
            return Err(format!("tag {} is not writable", command.tag_id));
        }

        command.transition_to(ControlCommandStatus::Validated);

        Ok(DriverWriteRequest {
            command_id: command.command_id.clone(),
            tag_id: command.tag_id.clone(),
            value: command.requested_value.clone(),
        })
    }
}

impl HttpRequest {
    pub fn new(method: &str, path: &str, body: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            headers: Vec::new(),
            body: body.to_string(),
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn path_without_query(&self) -> &str {
        self.path.split('?').next().unwrap_or(&self.path)
    }
}

impl HttpResponse {
    pub fn json(status_code: u16, body: String) -> Self {
        Self {
            status_code,
            reason: status_reason(status_code),
            content_type: "application/json",
            body,
        }
    }

    pub fn text(status_code: u16, body: &str) -> Self {
        Self {
            status_code,
            reason: status_reason(status_code),
            content_type: "text/plain; charset=utf-8",
            body: body.to_string(),
        }
    }

    pub fn to_http_bytes(&self) -> Vec<u8> {
        format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.status_code,
            self.reason,
            self.content_type,
            self.body.as_bytes().len(),
            self.body
        )
        .into_bytes()
    }
}

impl InMemoryTagCache {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn ingest(&mut self, value: TagValue) -> bool {
        let should_update = match self.values.get(&value.tag_id) {
            Some(current) => value.is_newer_than(current),
            None => true,
        };

        if should_update {
            self.values.insert(value.tag_id.clone(), value);
        }

        should_update
    }

    pub fn get(&self, tag_id: &str) -> Option<&TagValue> {
        self.values.get(tag_id)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl TagServerApi {
    pub fn new(cache: InMemoryTagCache, write_policy: WritePolicy) -> Self {
        Self {
            cache,
            write_policy,
            required_token: None,
            mqtt: None,
            driver_manager: None,
            operation_logs: VecDeque::new(),
        }
    }

    pub fn phase0_mock() -> Self {
        let mut cache = InMemoryTagCache::new();
        cache.ingest(phase0_tag_value(
            "mock.temperature.001",
            TagValueData::Float(21.0),
        ));
        cache.ingest(phase0_tag_value(
            "mock.running.001",
            TagValueData::Boolean(false),
        ));

        Self::new(cache, WritePolicy::new().allow_tag("mock.running.001"))
    }

    pub fn with_required_token(mut self, token: Option<String>) -> Self {
        self.required_token = token.filter(|value| !value.is_empty());
        self
    }

    pub fn with_mqtt_publish(mut self, mqtt: Option<MqttPublishConfig>) -> Self {
        self.mqtt = mqtt;
        self
    }

    pub fn with_driver_manager(
        mut self,
        driver_manager: Option<DriverManagerClientConfig>,
    ) -> Self {
        self.driver_manager = driver_manager;
        self
    }

    pub fn handle(&mut self, request: &HttpRequest) -> HttpResponse {
        if request.method == "GET" && request.path_without_query() == "/health" {
            return HttpResponse::json(
                200,
                r#"{"service":"tag-server","status":"healthy"}"#.to_string(),
            );
        }

        if !self.is_authorized(request) {
            return HttpResponse::json(401, error_json("unauthorized"));
        }

        match (request.method.as_str(), request.path_without_query()) {
            ("POST", "/api/v1/tags/snapshot") => self.handle_snapshot(&request.body),
            ("POST", "/api/v1/driver-values") => self.handle_driver_values(&request.body),
            ("POST", "/api/v1/control-commands") => self.handle_control_command(&request.body),
            ("GET", "/api/v1/operation-logs") => self.handle_operation_logs(),
            _ => HttpResponse::json(404, error_json("not found")),
        }
    }

    fn handle_operation_logs(&self) -> HttpResponse {
        let response = OperationLogList {
            items: self.operation_logs.iter().rev().cloned().collect(),
        };

        match serde_json::to_string(&response) {
            Ok(body) => HttpResponse::json(200, body),
            Err(error) => HttpResponse::json(
                500,
                error_json(&format!("operation log serialization failed: {error}")),
            ),
        }
    }

    fn record_operation_log(&mut self, entry: OperationLogEntry) {
        if self.operation_logs.len() >= OPERATION_LOG_CAPACITY {
            self.operation_logs.pop_front();
        }
        self.operation_logs.push_back(entry);
    }

    fn is_authorized(&self, request: &HttpRequest) -> bool {
        let Some(token) = &self.required_token else {
            return true;
        };

        request.header("x-scada-token") == Some(token.as_str())
            || request
                .header("authorization")
                .map(|value| value == format!("Bearer {token}"))
                .unwrap_or(false)
    }

    fn handle_snapshot(&self, body: &str) -> HttpResponse {
        let tag_ids = match extract_json_string_array(body, "tag_ids") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };

        let mut values = Vec::new();
        let mut missing = Vec::new();

        for tag_id in tag_ids {
            match self.cache.get(&tag_id) {
                Some(value) => values.push(tag_value_to_json(value)),
                None => missing.push(json_string(&tag_id)),
            }
        }

        HttpResponse::json(
            200,
            format!(
                r#"{{"values":[{}],"missing_tag_ids":[{}]}}"#,
                values.join(","),
                missing.join(",")
            ),
        )
    }

    fn handle_driver_values(&mut self, body: &str) -> HttpResponse {
        let driver_values = match parse_driver_values_body(body) {
            Ok(values) => values,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let received = driver_values.values.len();
        let mut ingested = 0usize;
        let mut stale = 0usize;
        let mut published = 0usize;
        let mut publish_errors = Vec::new();

        for value in driver_values.values {
            if self.cache.ingest(value.clone()) {
                ingested += 1;
                if let Some(mqtt) = &self.mqtt {
                    match publish_tag_value_via_mqtt(mqtt, &driver_values.project_id, &value) {
                        Ok(()) => published += 1,
                        Err(error) => publish_errors.push(error),
                    }
                }
            } else {
                stale += 1;
            }
        }

        if !publish_errors.is_empty() {
            return HttpResponse::json(
                502,
                format!(
                    r#"{{"received":{},"ingested":{},"stale":{},"published":{},"publish_errors":[{}]}}"#,
                    received,
                    ingested,
                    stale,
                    published,
                    publish_errors
                        .iter()
                        .map(|error| json_string(error))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            );
        }

        HttpResponse::json(
            202,
            format!(
                r#"{{"received":{},"ingested":{},"stale":{},"published":{}}}"#,
                received, ingested, stale, published
            ),
        )
    }

    fn handle_control_command(&mut self, body: &str) -> HttpResponse {
        let command_id = match extract_json_string(body, "command_id") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let idempotency_key = match extract_json_string(body, "idempotency_key") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let user_id = match extract_json_string(body, "user_id") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let tag_id = match extract_json_string(body, "tag_id") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let requested_value = match extract_json_value_as_string(body, "requested_value") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let requested_at = match extract_json_string(body, "requested_at") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };
        let timeout_ms = match extract_json_u64(body, "timeout_ms") {
            Ok(value) => value,
            Err(error) => return HttpResponse::json(400, error_json(&error)),
        };

        let mut command = ControlCommand::requested(
            &command_id,
            &idempotency_key,
            &user_id,
            &tag_id,
            &requested_value,
            &requested_at,
            timeout_ms,
        );

        match self.write_policy.validate(&mut command) {
            Ok(driver_request) => {
                if let Some(driver_manager) = &self.driver_manager {
                    command.transition_to(ControlCommandStatus::Sent);
                    match post_driver_write(driver_manager, &driver_request) {
                        Ok(driver_response) => {
                            apply_driver_write_response(&mut command, &driver_response);
                            self.record_operation_log(OperationLogEntry {
                                command_id: command_id.clone(),
                                user_id: user_id.clone(),
                                tag_id: tag_id.clone(),
                                requested_value: requested_value.clone(),
                                requested_at: requested_at.clone(),
                                status: format!("{:?}", command.status),
                                result: if driver_response.accepted {
                                    "accepted".to_string()
                                } else {
                                    driver_response.message.clone()
                                },
                            });
                            HttpResponse::json(
                                202,
                                format!(
                                    r#"{{"command":{},"driver_request":{},"driver_response":{}}}"#,
                                    control_command_to_json(&command),
                                    driver_write_request_to_json(&driver_request),
                                    driver_write_response_to_json(&driver_response)
                                ),
                            )
                        }
                        Err(error) => {
                            command.transition_to(ControlCommandStatus::Failed);
                            self.record_operation_log(OperationLogEntry {
                                command_id: command_id.clone(),
                                user_id: user_id.clone(),
                                tag_id: tag_id.clone(),
                                requested_value: requested_value.clone(),
                                requested_at: requested_at.clone(),
                                status: format!("{:?}", command.status),
                                result: error.clone(),
                            });
                            HttpResponse::json(
                                502,
                                format!(
                                    r#"{{"error":{},"command":{},"driver_request":{}}}"#,
                                    json_string(&error),
                                    control_command_to_json(&command),
                                    driver_write_request_to_json(&driver_request)
                                ),
                            )
                        }
                    }
                } else {
                    self.record_operation_log(OperationLogEntry {
                        command_id: command_id.clone(),
                        user_id: user_id.clone(),
                        tag_id: tag_id.clone(),
                        requested_value: requested_value.clone(),
                        requested_at: requested_at.clone(),
                        status: format!("{:?}", command.status),
                        result: "accepted".to_string(),
                    });
                    HttpResponse::json(
                        202,
                        format!(
                            r#"{{"command":{},"driver_request":{}}}"#,
                            control_command_to_json(&command),
                            driver_write_request_to_json(&driver_request)
                        ),
                    )
                }
            }
            Err(error) => {
                self.record_operation_log(OperationLogEntry {
                    command_id: command_id.clone(),
                    user_id,
                    tag_id,
                    requested_value,
                    requested_at,
                    status: format!("{:?}", command.status),
                    result: error.clone(),
                });

                HttpResponse::json(
                    409,
                    format!(
                        r#"{{"error":{},"command":{}}}"#,
                        json_string(&error),
                        control_command_to_json(&command)
                    ),
                )
            }
        }
    }
}

fn apply_driver_write_response(command: &mut ControlCommand, response: &DriverWriteResponse) {
    if response.accepted {
        command.transition_to(ControlCommandStatus::DriverAck);
    } else {
        command.transition_to(ControlCommandStatus::Failed);
    }
}

fn post_driver_write(
    config: &DriverManagerClientConfig,
    request: &DriverWriteRequest,
) -> Result<DriverWriteResponse, String> {
    let endpoint = HttpEndpoint::parse(&config.base_url)?;
    let body = driver_write_request_to_json(request);
    let http_request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        endpoint.path_with("/api/v1/driver-writes"),
        endpoint.host,
        body.as_bytes().len(),
        body
    );
    let mut stream =
        TcpStream::connect((endpoint.host.as_str(), endpoint.port)).map_err(|error| {
            format!(
                "connect driver manager {}:{}: {error}",
                endpoint.host, endpoint.port
            )
        })?;
    stream
        .write_all(http_request.as_bytes())
        .map_err(|error| format!("write driver manager request: {error}"))?;
    let mut raw_response = String::new();
    stream
        .read_to_string(&mut raw_response)
        .map_err(|error| format!("read driver manager response: {error}"))?;
    let (status, body) = parse_http_response(&raw_response)?;
    if !(200..300).contains(&status) {
        return Err(format!("driver manager returned status {status}: {body}"));
    }

    driver_write_response_from_json_str(&body)
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
            .ok_or_else(|| "driver manager URL must start with http://".to_string())?;
        let (authority, path) = without_scheme
            .split_once('/')
            .map(|(authority, path)| (authority, format!("/{path}")))
            .unwrap_or((without_scheme, String::new()));
        if authority.is_empty() {
            return Err("driver manager URL is missing host".to_string());
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, port)) => {
                let parsed_port = port
                    .parse::<u16>()
                    .map_err(|_| format!("invalid driver manager port: {port}"))?;
                (host.to_string(), parsed_port)
            }
            None => (authority.to_string(), 80),
        };
        if host.is_empty() {
            return Err("driver manager URL is missing host".to_string());
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

pub fn run_server(addr: &str, api: &mut TagServerApi) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => handle_stream(&mut stream, api)?,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

pub fn serve_one(listener: TcpListener, api: &mut TagServerApi) -> std::io::Result<()> {
    let (mut stream, _) = listener.accept()?;
    handle_stream(&mut stream, api)
}

pub fn parse_http_request(raw: &str) -> Result<HttpRequest, String> {
    let Some((head, body)) = raw.split_once("\r\n\r\n") else {
        return Err("missing http header terminator".to_string());
    };
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing http request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing http method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing http path".to_string())?
        .to_string();

    let mut request = HttpRequest::new(&method, &path, body);
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            request = request.with_header(name.trim(), value.trim());
        }
    }

    Ok(request)
}

#[derive(Debug, Clone, PartialEq)]
struct DriverValuesBody {
    project_id: String,
    values: Vec<TagValue>,
}

fn parse_driver_values_body(body: &str) -> Result<DriverValuesBody, String> {
    let root: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON body: {error}"))?;
    let project_id = root
        .get("project_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing or invalid project_id".to_string())?;
    let values = root
        .get("values")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "missing or invalid values".to_string())?;

    let values = values
        .iter()
        .map(tag_value_from_json_value)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DriverValuesBody {
        project_id: project_id.to_string(),
        values,
    })
}

pub fn publish_tag_value_via_mqtt(
    config: &MqttPublishConfig,
    project_id: &str,
    value: &TagValue,
) -> Result<(), String> {
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    runtime.block_on(publish_tag_value_via_mqtt_async(config, project_id, value))
}

async fn publish_tag_value_via_mqtt_async(
    config: &MqttPublishConfig,
    project_id: &str,
    value: &TagValue,
) -> Result<(), String> {
    let mut options = mqtt_options(config);
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 8);
    client
        .publish(
            tag_value_topic(project_id, &value.tag_id),
            QoS::AtLeastOnce,
            false,
            tag_value_to_json(value),
        )
        .await
        .map_err(|error| error.to_string())?;

    let deadline = tokio::time::Instant::now() + config.timeout;
    loop {
        let remaining = deadline
            .checked_duration_since(tokio::time::Instant::now())
            .ok_or_else(|| "timed out while waiting for mqtt publish ack".to_string())?;
        let event = tokio::time::timeout(remaining, eventloop.poll())
            .await
            .map_err(|_| "timed out while waiting for mqtt publish ack".to_string())?
            .map_err(|error| error.to_string())?;
        if matches!(event, Event::Incoming(Packet::PubAck(_))) {
            break;
        }
    }
    client
        .disconnect()
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn mqtt_options(config: &MqttPublishConfig) -> MqttOptions {
    let host = match config.endpoint.transport {
        MqttBrokerTransport::Tcp => config.endpoint.host.clone(),
        MqttBrokerTransport::WebSocket => config.endpoint.websocket_url(),
    };
    let mut options = MqttOptions::new(&config.client_id, host, config.endpoint.port);
    if config.endpoint.transport == MqttBrokerTransport::WebSocket {
        options.set_transport(Transport::ws());
    }
    options
}

fn handle_stream(stream: &mut TcpStream, api: &mut TagServerApi) -> std::io::Result<()> {
    let request = match read_http_request(stream) {
        Ok(request) => request,
        Err(error) => {
            let response = HttpResponse::text(400, &error.to_string());
            stream.write_all(&response.to_http_bytes())?;
            return Ok(());
        }
    };
    let response = api.handle(&request);
    stream.write_all(&response.to_http_bytes())
}

fn read_http_request(stream: &mut TcpStream) -> std::io::Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0; 1024];
    let mut header_end = None;

    while header_end.is_none() {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&buffer);

        if buffer.len() > 64 * 1024 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "http header too large",
            ));
        }
    }

    let Some(header_end) = header_end else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "incomplete http request",
        ));
    };

    let header = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let content_length = content_length(&header);
    let expected_len = header_end + 4 + content_length;

    while buffer.len() < expected_len {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let raw = String::from_utf8_lossy(&buffer[..buffer.len().min(expected_len)]).to_string();
    parse_http_request(&raw).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("bad request: {error}"),
        )
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(header: &str) -> usize {
    header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn status_reason(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        502 => "Bad Gateway",
        _ => "OK",
    }
}

fn phase0_tag_value(tag_id: &str, value: TagValueData) -> TagValue {
    TagValue {
        tag_id: tag_id.to_string(),
        value,
        quality: QualityCode::Simulated,
        source_timestamp: "1970-01-01T00:00:00Z".to_string(),
        server_timestamp: "1970-01-01T00:00:00Z".to_string(),
        sequence: 0,
        scan_interval_ms: 1000,
        stale_after_ms: 3000,
        driver_id: "mock-driver".to_string(),
        endpoint_id: "mock-endpoint".to_string(),
        read_status: "ok".to_string(),
        write_status: "idle".to_string(),
    }
}

fn control_command_to_json(command: &ControlCommand) -> String {
    format!(
        r#"{{"command_id":{},"idempotency_key":{},"user_id":{},"tag_id":{},"requested_value":{},"status":{},"requested_at":{},"timeout_ms":{}}}"#,
        json_string(&command.command_id),
        json_string(&command.idempotency_key),
        json_string(&command.user_id),
        json_string(&command.tag_id),
        json_string(&command.requested_value),
        json_string(command.status.as_str()),
        json_string(&command.requested_at),
        command.timeout_ms,
    )
}

fn error_json(message: &str) -> String {
    format!(r#"{{"error":{}}}"#, json_string(message))
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn extract_json_string_array(body: &str, key: &str) -> Result<Vec<String>, String> {
    let value = json_value_after_key(body, key)?;
    let bytes = value.as_bytes();
    let mut index = skip_ws(bytes, 0);

    if bytes.get(index) != Some(&b'[') {
        return Err(format!("{key} must be an array"));
    }
    index += 1;

    let mut values = Vec::new();
    loop {
        index = skip_ws(bytes, index);
        match bytes.get(index) {
            Some(b']') => return Ok(values),
            Some(b'"') => {
                let (value, next_index) = parse_json_string_at(value, index)?;
                values.push(value);
                index = skip_ws(bytes, next_index);
                match bytes.get(index) {
                    Some(b',') => index += 1,
                    Some(b']') => return Ok(values),
                    _ => return Err(format!("{key} array must contain strings")),
                }
            }
            _ => return Err(format!("{key} array must contain strings")),
        }
    }
}

fn extract_json_string(body: &str, key: &str) -> Result<String, String> {
    let value = json_value_after_key(body, key)?;
    let bytes = value.as_bytes();
    let index = skip_ws(bytes, 0);
    if bytes.get(index) != Some(&b'"') {
        return Err(format!("{key} must be a string"));
    }

    parse_json_string_at(value, index).map(|(value, _)| value)
}

fn extract_json_value_as_string(body: &str, key: &str) -> Result<String, String> {
    let value = json_value_after_key(body, key)?;
    let bytes = value.as_bytes();
    let index = skip_ws(bytes, 0);

    if bytes.get(index) == Some(&b'"') {
        return parse_json_string_at(value, index).map(|(value, _)| value);
    }

    let end = bytes[index..]
        .iter()
        .position(|byte| matches!(byte, b',' | b'}' | b'\r' | b'\n'))
        .map(|offset| index + offset)
        .unwrap_or(value.len());
    let scalar = value[index..end].trim();
    if scalar.is_empty() {
        return Err(format!("{key} is empty"));
    }

    Ok(scalar.to_string())
}

fn extract_json_u64(body: &str, key: &str) -> Result<u64, String> {
    extract_json_value_as_string(body, key)?
        .parse::<u64>()
        .map_err(|_| format!("{key} must be an integer"))
}

fn json_value_after_key<'a>(body: &'a str, key: &str) -> Result<&'a str, String> {
    let needle = format!(r#""{key}""#);
    let key_start = body.find(&needle).ok_or_else(|| format!("missing {key}"))?;
    let after_key = &body[key_start + needle.len()..];
    let colon = after_key
        .find(':')
        .ok_or_else(|| format!("missing {key} separator"))?;

    Ok(&after_key[colon + 1..])
}

fn parse_json_string_at(input: &str, start: usize) -> Result<(String, usize), String> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return Err("expected json string".to_string());
    }

    let mut index = start + 1;
    let mut value = String::new();
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'"' => return Ok((value, index + 1)),
            b'\\' => {
                index += 1;
                match bytes.get(index) {
                    Some(b'"') => value.push('"'),
                    Some(b'\\') => value.push('\\'),
                    Some(b'/') => value.push('/'),
                    Some(b'n') => value.push('\n'),
                    Some(b'r') => value.push('\r'),
                    Some(b't') => value.push('\t'),
                    Some(other) => value.push(*other as char),
                    None => return Err("unterminated json escape".to_string()),
                }
            }
            other => value.push(*other as char),
        }
        index += 1;
    }

    Err("unterminated json string".to_string())
}

fn skip_ws(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_json_error_response(
        response: &HttpResponse,
        expected_status: u16,
        expected_fragment: &str,
    ) {
        assert_eq!(expected_status, response.status_code);
        assert_eq!("application/json", response.content_type);

        let body =
            serde_json::from_str::<serde_json::Value>(&response.body).expect("error json body");
        let message = body
            .get("error")
            .and_then(serde_json::Value::as_str)
            .expect("error field");

        assert!(message.contains(expected_fragment));
    }

    fn tag_value(sequence: u64) -> TagValue {
        TagValue {
            tag_id: "mock.temperature.001".to_string(),
            value: TagValueData::Float(sequence as f64),
            quality: QualityCode::Simulated,
            source_timestamp: "1970-01-01T00:00:00Z".to_string(),
            server_timestamp: "1970-01-01T00:00:00Z".to_string(),
            sequence,
            scan_interval_ms: 1000,
            stale_after_ms: 3000,
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        }
    }

    #[test]
    fn cache_accepts_newer_values() {
        let mut cache = InMemoryTagCache::new();

        assert!(cache.ingest(tag_value(1)));
        assert!(cache.ingest(tag_value(2)));
        assert_eq!(2, cache.get("mock.temperature.001").unwrap().sequence);
    }

    #[test]
    fn cache_rejects_older_values() {
        let mut cache = InMemoryTagCache::new();

        assert!(cache.ingest(tag_value(2)));
        assert!(!cache.ingest(tag_value(1)));
        assert_eq!(2, cache.get("mock.temperature.001").unwrap().sequence);
    }

    #[test]
    fn write_policy_creates_driver_request_for_writable_tag() {
        let policy = WritePolicy::new().allow_tag("mock.running.001");
        let mut command = ControlCommand::requested(
            "cmd-1",
            "idem-1",
            "operator",
            "mock.running.001",
            "true",
            "1970-01-01T00:00:00Z",
            3000,
        );

        let request = policy.validate(&mut command).expect("driver request");

        assert_eq!(ControlCommandStatus::Validated, command.status);
        assert_eq!("cmd-1", request.command_id);
        assert_eq!("mock.running.001", request.tag_id);
    }

    #[test]
    fn write_policy_rejects_non_writable_tag() {
        let policy = WritePolicy::new();
        let mut command = ControlCommand::requested(
            "cmd-1",
            "idem-1",
            "operator",
            "mock.temperature.001",
            "42.0",
            "1970-01-01T00:00:00Z",
            3000,
        );

        assert!(policy.validate(&mut command).is_err());
        assert_eq!(ControlCommandStatus::Rejected, command.status);
    }

    #[test]
    fn snapshot_endpoint_returns_seeded_tag() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new(
            "POST",
            "/api/v1/tags/snapshot",
            r#"{"project_id":"demo","tag_ids":["mock.temperature.001","missing"]}"#,
        );

        let response = api.handle(&request);

        assert_eq!(200, response.status_code);
        assert!(response.body.contains(r#""tag_id":"mock.temperature.001""#));
        assert!(response.body.contains(r#""missing_tag_ids":["missing"]"#));
    }

    #[test]
    fn control_command_endpoint_validates_writable_tag() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"cmd-1","idempotency_key":"idem-1","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:00Z","timeout_ms":3000}"#,
        );

        let response = api.handle(&request);

        assert_eq!(202, response.status_code);
        assert!(response.body.contains(r#""status":"Validated""#));
        assert!(response.body.contains(r#""driver_request""#));
    }

    #[test]
    fn control_command_endpoint_persists_operation_log() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"cmd-log-1","idempotency_key":"idem-log-1","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:00Z","timeout_ms":3000}"#,
        );

        let response = api.handle(&request);
        assert_eq!(202, response.status_code);

        let logs = api.handle(&HttpRequest::new("GET", "/api/v1/operation-logs", ""));

        assert_eq!(200, logs.status_code);
        let body = serde_json::from_str::<serde_json::Value>(&logs.body).expect("logs json");
        let items = body
            .get("items")
            .and_then(serde_json::Value::as_array)
            .expect("items");
        assert!(!items.is_empty());

        let first = &items[0];
        assert_eq!(
            Some("cmd-log-1"),
            first.get("command_id").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("operator"),
            first.get("user_id").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("mock.running.001"),
            first.get("tag_id").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("Validated"),
            first.get("status").and_then(serde_json::Value::as_str)
        );
    }

    #[test]
    fn operation_logs_are_returned_in_reverse_chronological_order() {
        let mut api = TagServerApi::phase0_mock();

        let first = HttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"cmd-log-1","idempotency_key":"idem-log-1","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:00Z","timeout_ms":3000}"#,
        );
        let second = HttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"cmd-log-2","idempotency_key":"idem-log-2","user_id":"operator","tag_id":"mock.temperature.001","requested_value":42.0,"status":"Requested","requested_at":"1970-01-01T00:00:05Z","timeout_ms":3000}"#,
        );

        assert_eq!(202, api.handle(&first).status_code);
        assert_eq!(409, api.handle(&second).status_code);

        let logs = api.handle(&HttpRequest::new("GET", "/api/v1/operation-logs", ""));
        assert_eq!(200, logs.status_code);

        let body = serde_json::from_str::<serde_json::Value>(&logs.body).expect("logs json");
        let items = body
            .get("items")
            .and_then(serde_json::Value::as_array)
            .expect("items");

        assert!(items.len() >= 2);
        assert_eq!(
            Some("cmd-log-2"),
            items[0]
                .get("command_id")
                .and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("Rejected"),
            items[0].get("status").and_then(serde_json::Value::as_str)
        );
        assert_eq!(
            Some("cmd-log-1"),
            items[1]
                .get("command_id")
                .and_then(serde_json::Value::as_str)
        );
    }

    #[test]
    fn driver_values_endpoint_updates_snapshot_cache() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new(
            "POST",
            "/api/v1/driver-values",
            r#"{"project_id":"demo","values":[{"tag_id":"mock.temperature.001","value":29.0,"data_type":"float","quality":"Simulated","source_timestamp":"1970-01-01T00:00:02Z","server_timestamp":"1970-01-01T00:00:02Z","sequence":2,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}]}"#,
        );

        let ingest_response = api.handle(&request);

        assert_eq!(202, ingest_response.status_code);
        assert!(ingest_response.body.contains(r#""ingested":1"#));

        let snapshot_response = api.handle(&HttpRequest::new(
            "POST",
            "/api/v1/tags/snapshot",
            r#"{"project_id":"demo","tag_ids":["mock.temperature.001"]}"#,
        ));

        assert_eq!(200, snapshot_response.status_code);
        assert!(snapshot_response.body.contains(r#""value":29.0"#));
        assert!(snapshot_response.body.contains(r#""sequence":2"#));
    }

    #[test]
    fn driver_values_endpoint_returns_publish_errors_when_mqtt_publish_fails() {
        let reserved_port = {
            let listener = TcpListener::bind("127.0.0.1:0").expect("reserve port");
            let port = listener.local_addr().expect("addr").port();
            drop(listener);
            port
        };

        let mut api = TagServerApi::phase0_mock().with_mqtt_publish(Some(MqttPublishConfig {
            endpoint: MqttBrokerEndpoint::parse(&format!("mqtt://127.0.0.1:{reserved_port}"))
                .expect("endpoint"),
            client_id: "tag-server-test-publish-fail".to_string(),
            timeout: Duration::from_millis(50),
        }));
        let request = HttpRequest::new(
            "POST",
            "/api/v1/driver-values",
            r#"{"project_id":"demo","values":[{"tag_id":"mock.temperature.001","value":29.0,"data_type":"float","quality":"Simulated","source_timestamp":"1970-01-01T00:00:02Z","server_timestamp":"1970-01-01T00:00:02Z","sequence":2,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}]}"#,
        );

        let response = api.handle(&request);

        assert_eq!(502, response.status_code);
        assert_eq!("application/json", response.content_type);
        let body = serde_json::from_str::<serde_json::Value>(&response.body).expect("json");
        assert_eq!(
            Some(1),
            body.get("received").and_then(serde_json::Value::as_u64)
        );
        assert_eq!(
            Some(1),
            body.get("ingested").and_then(serde_json::Value::as_u64)
        );
        assert_eq!(
            Some(0),
            body.get("stale").and_then(serde_json::Value::as_u64)
        );
        assert_eq!(
            Some(0),
            body.get("published").and_then(serde_json::Value::as_u64)
        );
        let publish_errors = body
            .get("publish_errors")
            .and_then(serde_json::Value::as_array)
            .expect("publish_errors array");
        assert!(!publish_errors.is_empty());
    }

    #[test]
    fn api_requires_token_when_configured() {
        let mut api = TagServerApi::phase0_mock().with_required_token(Some("secret".to_string()));
        let unauthenticated = HttpRequest::new(
            "POST",
            "/api/v1/tags/snapshot",
            r#"{"tag_ids":["mock.temperature.001"]}"#,
        );
        let authenticated = HttpRequest::new(
            "POST",
            "/api/v1/tags/snapshot",
            r#"{"tag_ids":["mock.temperature.001"]}"#,
        )
        .with_header("authorization", "Bearer secret");

        let unauthorized = api.handle(&unauthenticated);

        assert_json_error_response(&unauthorized, 401, "unauthorized");
        assert_eq!(200, api.handle(&authenticated).status_code);
    }

    #[test]
    fn snapshot_endpoint_returns_json_error_for_invalid_payload() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new("POST", "/api/v1/tags/snapshot", r#"{"tag_ids":"broken"}"#);

        let response = api.handle(&request);

        assert_json_error_response(&response, 400, "tag_ids");
    }

    #[test]
    fn driver_values_endpoint_returns_json_error_for_invalid_payload() {
        let mut api = TagServerApi::phase0_mock();
        let request = HttpRequest::new("POST", "/api/v1/driver-values", r#"{"project_id":"demo"}"#);

        let response = api.handle(&request);

        assert_json_error_response(&response, 400, "values");
    }

    #[test]
    fn api_returns_json_error_for_unknown_route() {
        let mut api = TagServerApi::phase0_mock();
        let response = api.handle(&HttpRequest::new("POST", "/api/v1/unknown", "{}"));

        assert_json_error_response(&response, 404, "not found");
    }

    #[test]
    fn parse_http_request_reads_headers_and_body() {
        let request = parse_http_request(
            "POST /api/v1/tags/snapshot HTTP/1.1\r\nHost: localhost\r\nContent-Length: 34\r\n\r\n{\"tag_ids\":[\"mock.running.001\"]}",
        )
        .expect("parse request");

        assert_eq!("POST", request.method);
        assert_eq!("/api/v1/tags/snapshot", request.path);
        assert_eq!(Some("localhost"), request.header("host"));
        assert!(request.body.contains("mock.running.001"));
    }
}
