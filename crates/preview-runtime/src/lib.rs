use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use scada_core::mqtt::{tag_value_topic, MqttBrokerEndpoint, MqttBrokerTransport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use tokio::runtime::Runtime;
use tokio::time::{self, Instant};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeControlCommandRequest {
    pub command_id: String,
    pub idempotency_key: String,
    pub user_id: String,
    pub tag_id: String,
    pub requested_value: Value,
    pub status: String,
    pub requested_at: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeScreenProjectionRequest {
    #[serde(default)]
    pub screen_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenProjection {
    pub screen_id: String,
    pub project_id: String,
    pub object_states: Vec<ScreenObjectState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenObjectState {
    pub object_id: String,
    pub svg_asset_id: String,
    pub bindings: Vec<ObjectBindingState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectBindingState {
    pub key: String,
    pub tag_id: String,
    pub value: Option<Value>,
    pub quality: Option<String>,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeDeltaMessage {
    pub topic: String,
    pub value: RuntimeTagValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaApplyResult {
    pub tag_id: String,
    pub matched_bindings: usize,
    pub applied_bindings: usize,
    pub ignored_stale_bindings: usize,
    pub affected_object_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttConnectionConfig {
    pub endpoint: MqttBrokerEndpoint,
    pub client_id: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpClientResponse {
    pub status_code: u16,
    pub reason: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHttpRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHttpResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub content_type: &'static str,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeApi {
    tag_server: TagServerClient,
    required_token: Option<String>,
    default_screen_path: PathBuf,
}

#[derive(Debug)]
pub enum RuntimeClientError {
    InvalidEndpoint(String),
    Io(String),
    Http(String),
    Json(String),
    MqttDelta(String),
    ScreenDefinition(String),
    Runtime(String),
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

    pub fn forward_control_command(
        &self,
        command: &RuntimeControlCommandRequest,
    ) -> Result<HttpClientResponse, RuntimeClientError> {
        validate_control_command(command).map_err(RuntimeClientError::Runtime)?;
        let body = serde_json::to_string(command)
            .map_err(|error| RuntimeClientError::Json(error.to_string()))?;
        self.forward_control_command_body(&body)
    }

    pub fn forward_control_command_body(
        &self,
        body: &str,
    ) -> Result<HttpClientResponse, RuntimeClientError> {
        let raw_request = build_http_request(
            "POST",
            "/api/v1/control-commands",
            &self.endpoint.authority(),
            self.token.as_deref(),
            body,
        );
        let raw_response = self.send_raw(&raw_request)?;
        parse_http_response(&raw_response)
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

impl RuntimeHttpRequest {
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

impl RuntimeHttpResponse {
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

impl RuntimeApi {
    pub fn new(tag_server: TagServerClient) -> Self {
        Self {
            tag_server,
            required_token: None,
            default_screen_path: PathBuf::from("config/screens/mock-main.screen.json"),
        }
    }

    pub fn with_required_token(mut self, token: Option<String>) -> Self {
        self.required_token = token.filter(|value| !value.is_empty());
        self
    }

    pub fn with_default_screen_path(mut self, path: PathBuf) -> Self {
        self.default_screen_path = path;
        self
    }

    pub fn handle(&self, request: &RuntimeHttpRequest) -> RuntimeHttpResponse {
        if request.method == "GET" && request.path_without_query() == "/health" {
            return RuntimeHttpResponse::json(
                200,
                r#"{"service":"preview-runtime","status":"healthy"}"#.to_string(),
            );
        }

        if !self.is_authorized(request) {
            return RuntimeHttpResponse::json(401, error_json("unauthorized"));
        }

        match (request.method.as_str(), request.path_without_query()) {
            ("POST", "/api/v1/control-commands") => self.handle_control_command(&request.body),
            ("POST", "/api/v1/screens/projection") => self.handle_screen_projection(&request.body),
            _ => RuntimeHttpResponse::json(404, error_json("not found")),
        }
    }

    fn is_authorized(&self, request: &RuntimeHttpRequest) -> bool {
        let Some(token) = &self.required_token else {
            return true;
        };

        request.header("x-scada-token") == Some(token.as_str())
            || request
                .header("authorization")
                .map(|value| value == format!("Bearer {token}"))
                .unwrap_or(false)
    }

    fn handle_control_command(&self, body: &str) -> RuntimeHttpResponse {
        let command: RuntimeControlCommandRequest = match serde_json::from_str(body) {
            Ok(command) => command,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    400,
                    error_json(&format!("invalid control command JSON: {error}")),
                );
            }
        };

        if let Err(error) = validate_control_command(&command) {
            return RuntimeHttpResponse::json(400, error_json(&error.to_string()));
        }

        match self.tag_server.forward_control_command(&command) {
            Ok(response) => RuntimeHttpResponse::json(response.status_code, response.body),
            Err(error) => RuntimeHttpResponse::json(
                502,
                error_json(&format!("tag server forwarding failed: {error}")),
            ),
        }
    }

    fn handle_screen_projection(&self, body: &str) -> RuntimeHttpResponse {
        let projection_request: RuntimeScreenProjectionRequest = if body.trim().is_empty() {
            RuntimeScreenProjectionRequest { screen_path: None }
        } else {
            match serde_json::from_str(body) {
                Ok(request) => request,
                Err(error) => {
                    return RuntimeHttpResponse::json(
                        400,
                        error_json(&format!("invalid screen projection JSON: {error}")),
                    );
                }
            }
        };
        let screen_path = projection_request
            .screen_path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_screen_path.clone());
        let definition = match load_screen_definition(&screen_path) {
            Ok(definition) => definition,
            Err(error) => return RuntimeHttpResponse::json(400, error_json(&error.to_string())),
        };
        let tag_ids = resolve_tag_ids_from_screen(&definition);
        if tag_ids.is_empty() {
            return RuntimeHttpResponse::json(
                400,
                error_json("screen definition does not contain tag bindings"),
            );
        }

        let snapshot = match self
            .tag_server
            .fetch_snapshot(&definition.project_id, &tag_ids)
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return RuntimeHttpResponse::json(
                    502,
                    error_json(&format!("tag server snapshot failed: {error}")),
                );
            }
        };
        let projection = project_snapshot_to_screen(&definition, &snapshot);
        match serde_json::to_string(&projection) {
            Ok(body) => RuntimeHttpResponse::json(200, body),
            Err(error) => RuntimeHttpResponse::json(500, error_json(&error.to_string())),
        }
    }
}

impl fmt::Display for RuntimeClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEndpoint(message) => write!(formatter, "invalid endpoint: {message}"),
            Self::Io(message) => write!(formatter, "io error: {message}"),
            Self::Http(message) => write!(formatter, "http error: {message}"),
            Self::Json(message) => write!(formatter, "json error: {message}"),
            Self::MqttDelta(message) => write!(formatter, "mqtt delta error: {message}"),
            Self::ScreenDefinition(message) => {
                write!(formatter, "screen definition error: {message}")
            }
            Self::Runtime(message) => write!(formatter, "runtime error: {message}"),
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

    resolved.sort();
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

pub fn project_snapshot_to_screen(
    definition: &ScreenDefinition,
    snapshot: &TagSnapshot,
) -> ScreenProjection {
    let values_by_tag: HashMap<&str, &RuntimeTagValue> = snapshot
        .values
        .iter()
        .map(|value| (value.tag_id.as_str(), value))
        .collect();
    let mut object_states = Vec::new();

    for object in &definition.objects {
        let mut bindings = Vec::new();
        let mut binding_keys: Vec<_> = object.tag_bindings.keys().cloned().collect();
        binding_keys.sort();

        for key in binding_keys {
            let tag_id = object
                .tag_bindings
                .get(&key)
                .cloned()
                .unwrap_or_else(|| "".to_string());
            let resolved = values_by_tag.get(tag_id.as_str()).copied();
            bindings.push(ObjectBindingState {
                key,
                tag_id,
                value: resolved.map(|value| value.value.clone()),
                quality: resolved.map(|value| value.quality.clone()),
                sequence: resolved.map(|value| value.sequence),
            });
        }

        object_states.push(ScreenObjectState {
            object_id: object.object_id.clone(),
            svg_asset_id: object.svg_asset_id.clone(),
            bindings,
        });
    }

    ScreenProjection {
        screen_id: definition.screen_id.clone(),
        project_id: definition.project_id.clone(),
        object_states,
    }
}

pub fn format_screen_projection_summary(projection: &ScreenProjection) -> String {
    let total_bindings: usize = projection
        .object_states
        .iter()
        .map(|object| object.bindings.len())
        .sum();
    let missing_bindings: usize = projection
        .object_states
        .iter()
        .flat_map(|object| object.bindings.iter())
        .filter(|binding| binding.value.is_none())
        .count();
    let mut lines = vec![format!(
        "projection screen={} objects={} bindings={} missing={}",
        projection.screen_id,
        projection.object_states.len(),
        total_bindings,
        missing_bindings
    )];

    for object in &projection.object_states {
        lines.push(format!(
            "object {} asset={}",
            object.object_id, object.svg_asset_id
        ));
        for binding in &object.bindings {
            let value = binding
                .value
                .as_ref()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string());
            let quality = binding
                .quality
                .clone()
                .unwrap_or_else(|| "Missing".to_string());
            let sequence = binding
                .sequence
                .map(|seq| seq.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "binding {} tag={} value={} quality={} seq={}",
                binding.key, binding.tag_id, value, quality, sequence
            ));
        }
    }

    lines.join("\n")
}

pub fn parse_tag_value_delta(
    project_id: &str,
    topic: &str,
    payload: &str,
) -> Result<RuntimeDeltaMessage, RuntimeClientError> {
    let value: RuntimeTagValue = serde_json::from_str(payload)
        .map_err(|error| RuntimeClientError::Json(error.to_string()))?;
    let expected_topic = tag_value_topic(project_id, &value.tag_id);

    if topic != expected_topic {
        return Err(RuntimeClientError::MqttDelta(format!(
            "topic {topic} does not match expected {expected_topic}"
        )));
    }

    Ok(RuntimeDeltaMessage {
        topic: topic.to_string(),
        value,
    })
}

pub fn apply_delta_to_projection(
    projection: &mut ScreenProjection,
    delta: &RuntimeTagValue,
) -> DeltaApplyResult {
    let mut result = DeltaApplyResult {
        tag_id: delta.tag_id.clone(),
        matched_bindings: 0,
        applied_bindings: 0,
        ignored_stale_bindings: 0,
        affected_object_ids: Vec::new(),
    };

    for object in &mut projection.object_states {
        let mut object_changed = false;
        for binding in &mut object.bindings {
            if binding.tag_id != delta.tag_id {
                continue;
            }

            result.matched_bindings += 1;
            if binding
                .sequence
                .map(|sequence| delta.sequence <= sequence)
                .unwrap_or(false)
            {
                result.ignored_stale_bindings += 1;
                continue;
            }

            binding.value = Some(delta.value.clone());
            binding.quality = Some(delta.quality.clone());
            binding.sequence = Some(delta.sequence);
            result.applied_bindings += 1;
            object_changed = true;
        }

        if object_changed {
            result.affected_object_ids.push(object.object_id.clone());
        }
    }

    result
}

pub fn format_delta_apply_result(result: &DeltaApplyResult) -> String {
    format!(
        "delta tag={} matched={} applied={} stale={} affected={}",
        result.tag_id,
        result.matched_bindings,
        result.applied_bindings,
        result.ignored_stale_bindings,
        result.affected_object_ids.join(",")
    )
}

pub fn mqtt_tag_value_topic_filter(project_id: &str) -> String {
    format!("scada/{project_id}/tag/+/value")
}

pub fn receive_delta_once_via_mqtt(
    config: &MqttConnectionConfig,
    project_id: &str,
) -> Result<RuntimeDeltaMessage, RuntimeClientError> {
    let runtime = Runtime::new().map_err(|error| RuntimeClientError::Runtime(error.to_string()))?;
    runtime.block_on(receive_delta_once_via_mqtt_async(config, project_id))
}

pub fn subscribe_deltas_via_mqtt<F>(
    config: &MqttConnectionConfig,
    project_id: &str,
    mut on_delta: F,
) -> Result<(), RuntimeClientError>
where
    F: FnMut(RuntimeDeltaMessage),
{
    let runtime = Runtime::new().map_err(|error| RuntimeClientError::Runtime(error.to_string()))?;
    runtime.block_on(async move {
        subscribe_deltas_via_mqtt_async(config, project_id, &mut on_delta).await
    })
}

pub fn publish_delta_via_mqtt(
    config: &MqttConnectionConfig,
    delta: &RuntimeTagValue,
    project_id: &str,
) -> Result<(), RuntimeClientError> {
    let runtime = Runtime::new().map_err(|error| RuntimeClientError::Runtime(error.to_string()))?;
    runtime.block_on(publish_delta_via_mqtt_async(config, delta, project_id))
}

pub fn run_runtime_server(addr: &str, api: &RuntimeApi) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => handle_runtime_stream(&mut stream, api)?,
            Err(error) => return Err(error),
        }
    }

    Ok(())
}

pub fn serve_runtime_once(listener: TcpListener, api: &RuntimeApi) -> std::io::Result<()> {
    let (mut stream, _) = listener.accept()?;
    handle_runtime_stream(&mut stream, api)
}

async fn receive_delta_once_via_mqtt_async(
    config: &MqttConnectionConfig,
    project_id: &str,
) -> Result<RuntimeDeltaMessage, RuntimeClientError> {
    let mut options = mqtt_options(config);
    options.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(options, 16);
    client
        .subscribe(mqtt_tag_value_topic_filter(project_id), QoS::AtMostOnce)
        .await
        .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;

    let deadline = Instant::now() + config.timeout;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err(RuntimeClientError::MqttDelta(
                "timed out while waiting for delta".to_string(),
            ));
        };
        let event = time::timeout(remaining, eventloop.poll())
            .await
            .map_err(|_| {
                RuntimeClientError::MqttDelta("timed out while waiting for delta".to_string())
            })?
            .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;

        if let Event::Incoming(Packet::Publish(publish)) = event {
            let payload = String::from_utf8(publish.payload.to_vec())
                .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;
            return parse_tag_value_delta(project_id, &publish.topic, &payload);
        }
    }
}

async fn subscribe_deltas_via_mqtt_async<F>(
    config: &MqttConnectionConfig,
    project_id: &str,
    on_delta: &mut F,
) -> Result<(), RuntimeClientError>
where
    F: FnMut(RuntimeDeltaMessage),
{
    let mut options = mqtt_options(config);
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 16);
    client
        .subscribe(mqtt_tag_value_topic_filter(project_id), QoS::AtMostOnce)
        .await
        .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;

    loop {
        let event = time::timeout(config.timeout, eventloop.poll())
            .await
            .map_err(|_| {
                RuntimeClientError::MqttDelta("timed out while waiting for delta".to_string())
            })?
            .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;

        if let Event::Incoming(Packet::Publish(publish)) = event {
            let payload = String::from_utf8(publish.payload.to_vec())
                .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;
            let delta = parse_tag_value_delta(project_id, &publish.topic, &payload)?;
            on_delta(delta);
        }
    }
}

async fn publish_delta_via_mqtt_async(
    config: &MqttConnectionConfig,
    delta: &RuntimeTagValue,
    project_id: &str,
) -> Result<(), RuntimeClientError> {
    let mut options = mqtt_options(config);
    options.set_keep_alive(Duration::from_secs(5));
    let (client, mut eventloop) = AsyncClient::new(options, 16);
    let topic = tag_value_topic(project_id, &delta.tag_id);
    let payload =
        serde_json::to_vec(delta).map_err(|error| RuntimeClientError::Json(error.to_string()))?;

    client
        .publish(topic, QoS::AtMostOnce, false, payload)
        .await
        .map_err(|error| RuntimeClientError::MqttDelta(error.to_string()))?;

    // Drive the eventloop briefly so the queued publish request is sent.
    for _ in 0..5 {
        match time::timeout(Duration::from_millis(200), eventloop.poll()).await {
            Ok(Ok(_)) => continue,
            Ok(Err(error)) => return Err(RuntimeClientError::MqttDelta(error.to_string())),
            Err(_) => break,
        }
    }
    Ok(())
}

fn mqtt_options(config: &MqttConnectionConfig) -> MqttOptions {
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

pub fn parse_runtime_http_request(raw: &str) -> Result<RuntimeHttpRequest, String> {
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
        .ok_or_else(|| "missing http method".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "missing http path".to_string())?;

    let mut request = RuntimeHttpRequest::new(method, path, body);
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            request = request.with_header(name.trim(), value.trim());
        }
    }

    Ok(request)
}

fn handle_runtime_stream(stream: &mut TcpStream, api: &RuntimeApi) -> std::io::Result<()> {
    let request = match read_runtime_http_request(stream) {
        Ok(request) => request,
        Err(error) => {
            let response = RuntimeHttpResponse::text(400, &error.to_string());
            stream.write_all(&response.to_http_bytes())?;
            return Ok(());
        }
    };
    let response = api.handle(&request);
    stream.write_all(&response.to_http_bytes())
}

fn read_runtime_http_request(stream: &mut TcpStream) -> std::io::Result<RuntimeHttpRequest> {
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
    parse_runtime_http_request(&raw).map_err(|error| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("bad request: {error}"),
        )
    })
}

fn validate_control_command(command: &RuntimeControlCommandRequest) -> Result<(), String> {
    if command.command_id.is_empty() {
        return Err("command_id must not be empty".to_string());
    }
    if command.idempotency_key.is_empty() {
        return Err("idempotency_key must not be empty".to_string());
    }
    if command.user_id.is_empty() {
        return Err("user_id must not be empty".to_string());
    }
    if command.tag_id.is_empty() {
        return Err("tag_id must not be empty".to_string());
    }
    if command.status != "Requested" {
        return Err("status must be Requested for a new runtime command".to_string());
    }
    if command.requested_at.is_empty() {
        return Err("requested_at must not be empty".to_string());
    }
    if command.timeout_ms == 0 {
        return Err("timeout_ms must be greater than zero".to_string());
    }

    Ok(())
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
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    }
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
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
    fn runtime_api_requires_token_for_control_commands() {
        let api = RuntimeApi::new(
            TagServerClient::from_base_url("http://127.0.0.1:18080").expect("client"),
        )
        .with_required_token(Some("secret".to_string()));
        let request = RuntimeHttpRequest::new("POST", "/api/v1/control-commands", "{}");

        let response = api.handle(&request);

        assert_eq!(401, response.status_code);
        assert_eq!(r#"{"error":"unauthorized"}"#, response.body);
    }

    #[test]
    fn runtime_api_rejects_invalid_control_command_before_forwarding() {
        let api = RuntimeApi::new(
            TagServerClient::from_base_url("http://127.0.0.1:18080").expect("client"),
        );
        let request = RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"","idempotency_key":"key","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:05Z","timeout_ms":1000}"#,
        );

        let response = api.handle(&request);

        assert_eq!(400, response.status_code);
        assert!(response.body.contains("command_id must not be empty"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn tag_server_client_forwards_control_command_with_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/control-commands", request.path);
            assert_eq!(Some("Bearer secret"), request.header("authorization"));
            assert!(request.body.contains(r#""tag_id":"mock.running.001""#));

            let body = r#"{"command":{"status":"DriverAck"},"driver_request":{"command_id":"cmd-1","tag_id":"mock.running.001","value":"true"}}"#;
            let response = RuntimeHttpResponse::json(202, body.to_string());
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let client = TagServerClient::from_base_url(&format!("http://{addr}"))
            .expect("client")
            .with_token(Some("secret".to_string()));

        let response = client
            .forward_control_command(&RuntimeControlCommandRequest {
                command_id: "cmd-1".to_string(),
                idempotency_key: "key-1".to_string(),
                user_id: "operator".to_string(),
                tag_id: "mock.running.001".to_string(),
                requested_value: Value::Bool(true),
                status: "Requested".to_string(),
                requested_at: "1970-01-01T00:00:05Z".to_string(),
                timeout_ms: 1000,
            })
            .expect("response");
        server.join().expect("server");

        assert_eq!(202, response.status_code);
        assert!(response.body.contains("DriverAck"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_forwards_control_command_json_error_response() {
        let response = runtime_api_handle_control_command_with_tag_server_response(
            409,
            r#"{"error":"command already in progress"}"#,
        );

        assert_eq!(409, response.status_code);
        assert_eq!(r#"{"error":"command already in progress"}"#, response.body);
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_forwards_control_command_non_json_error_response() {
        let response =
            runtime_api_handle_control_command_with_tag_server_response(502, "bad gateway");

        assert_eq!(502, response.status_code);
        assert_eq!("bad gateway", response.body);
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_forwards_control_command_empty_success_response() {
        let response = runtime_api_handle_control_command_with_tag_server_response(202, "");

        assert_eq!(202, response.status_code);
        assert_eq!("", response.body);
    }

    #[test]
    fn runtime_api_returns_bad_gateway_when_control_command_connection_is_refused() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        drop(listener);

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(100)),
        );

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            valid_control_command_json(),
        ));

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("io error"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_control_command_response_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept");
            std::thread::sleep(std::time::Duration::from_millis(300));
        });

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(50)),
        );

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            valid_control_command_json(),
        ));
        server.join().expect("server");

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("io error"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_control_command_http_header_terminator_is_missing() {
        let response = runtime_api_handle_control_command_with_raw_response(
            "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\n",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("missing response header terminator"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_control_command_http_status_line_is_missing() {
        let response = runtime_api_handle_control_command_with_raw_response(
            "\r\n\r\n{\"command\":{\"status\":\"DriverAck\"}}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("missing status line"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_control_command_http_version_is_invalid() {
        let response = runtime_api_handle_control_command_with_raw_response(
            "HTTX/1.1 202 Accepted\r\nContent-Type: application/json\r\n\r\n{\"command\":{\"status\":\"DriverAck\"}}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("invalid http version"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_control_command_http_status_code_is_invalid() {
        let response = runtime_api_handle_control_command_with_raw_response(
            "HTTP/1.1 xyz OOPS\r\nContent-Type: application/json\r\n\r\n{\"command\":{\"status\":\"DriverAck\"}}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server forwarding failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("invalid status code"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_screen_projection_from_tag_snapshot() {
        let screen_path = std::env::temp_dir().join(format!(
            "scada-preview-runtime-screen-{}.json",
            std::process::id()
        ));
        fs::write(
            &screen_path,
            r#"{"schema_version":"1.0.0","screen_id":"main","project_id":"demo","name":"Main","canvas_width":1280,"canvas_height":720,"objects":[{"object_id":"pump-001","svg_asset_id":"pump","x":0.0,"y":0.0,"width":100.0,"height":100.0,"tag_bindings":{"state":"mock.running.001"}}]}"#,
        )
        .expect("write screen");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);
            assert!(request.body.contains(r#""tag_ids":["mock.running.001"]"#));

            let body = r#"{"values":[{"tag_id":"mock.running.001","value":true,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:01Z","server_timestamp":"1970-01-01T00:00:01Z","sequence":3,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}],"missing_tag_ids":[]}"#;
            let response = RuntimeHttpResponse::json(200, body.to_string());
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        )
        .with_default_screen_path(screen_path.clone());
        let request = RuntimeHttpRequest::new("POST", "/api/v1/screens/projection", r#"{}"#);

        let response = api.handle(&request);
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(200, response.status_code);
        assert!(response.body.contains(r#""screen_id":"main""#));
        assert!(response.body.contains(r#""object_id":"pump-001""#));
        assert!(response.body.contains(r#""value":true"#));
        assert!(response.body.contains(r#""sequence":3"#));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_handles_projection_and_control_command_with_same_client() {
        let screen_path = std::env::temp_dir().join(format!(
            "scada-preview-runtime-screen-dual-{}.json",
            std::process::id()
        ));
        fs::write(
            &screen_path,
            r#"{"schema_version":"1.0.0","screen_id":"main","project_id":"demo","name":"Main","canvas_width":1280,"canvas_height":720,"objects":[{"object_id":"pump-001","svg_asset_id":"pump","x":0.0,"y":0.0,"width":100.0,"height":100.0,"tag_bindings":{"state":"mock.running.001"}}]}"#,
        )
        .expect("write screen");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut snapshot_stream, _) = listener.accept().expect("snapshot accept");
            let snapshot_request =
                read_runtime_http_request(&mut snapshot_stream).expect("snapshot request");
            assert_eq!("POST", snapshot_request.method);
            assert_eq!("/api/v1/tags/snapshot", snapshot_request.path);
            let snapshot_body = r#"{"values":[{"tag_id":"mock.running.001","value":false,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:01Z","server_timestamp":"1970-01-01T00:00:01Z","sequence":3,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}],"missing_tag_ids":[]}"#;
            let snapshot_response = RuntimeHttpResponse::json(200, snapshot_body.to_string());
            snapshot_stream
                .write_all(&snapshot_response.to_http_bytes())
                .expect("snapshot write");
            drop(snapshot_stream);

            let (mut command_stream, _) = listener.accept().expect("command accept");
            let command_request =
                read_runtime_http_request(&mut command_stream).expect("command request");
            assert_eq!("POST", command_request.method);
            assert_eq!("/api/v1/control-commands", command_request.path);
            assert!(command_request.body.contains(r#""command_id":"cmd-1""#));
            let command_body = r#"{"command":{"status":"DriverAck"},"driver_request":{"command_id":"cmd-1","tag_id":"mock.running.001","value":"true"},"driver_response":{"command_id":"cmd-1","accepted":true,"message":"accepted"}}"#;
            let command_response = RuntimeHttpResponse::json(202, command_body.to_string());
            command_stream
                .write_all(&command_response.to_http_bytes())
                .expect("command write");
            drop(command_stream);
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        )
        .with_default_screen_path(screen_path.clone());

        let projection_response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        let command_response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            r#"{"command_id":"cmd-1","idempotency_key":"key-1","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:05Z","timeout_ms":1000}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(200, projection_response.status_code);
        assert!(projection_response.body.contains(r#""screen_id":"main""#));
        assert_eq!(202, command_response.status_code);
        assert!(command_response.body.contains("DriverAck"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_fetch_fails() {
        let screen_path = write_test_screen_definition("snapshot-fail");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);

            let response =
                RuntimeHttpResponse::json(503, r#"{"error":"snapshot unavailable"}"#.to_string());
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("503"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_json_is_invalid() {
        let screen_path = write_test_screen_definition("snapshot-invalid-json");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);

            let response = RuntimeHttpResponse::json(200, r#"{"values":"#.to_string());
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("json error"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_body_is_empty() {
        let screen_path = write_test_screen_definition("snapshot-empty-body");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);

            let response = RuntimeHttpResponse::json(200, String::new());
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("json error"));
    }

    #[test]
    fn runtime_api_rejects_invalid_projection_json() {
        let api = RuntimeApi::new(
            TagServerClient::from_base_url("http://127.0.0.1:18080").expect("client"),
        );

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{"screen_path":"#,
        ));

        assert_eq!(400, response.status_code);
        assert!(response.body.contains("invalid screen projection JSON"));
    }

    #[test]
    fn runtime_api_rejects_missing_projection_screen_path() {
        let api = RuntimeApi::new(
            TagServerClient::from_base_url("http://127.0.0.1:18080").expect("client"),
        );
        let missing_path = std::env::temp_dir().join(format!(
            "scada-preview-runtime-missing-screen-{}.json",
            std::process::id()
        ));

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            &format!(r#"{{"screen_path":"{}"}}"#, missing_path.display()),
        ));

        assert_eq!(400, response.status_code);
        assert!(response.body.contains("screen definition error"));
    }

    #[test]
    fn runtime_api_returns_bad_gateway_when_snapshot_connection_is_refused() {
        let screen_path = write_test_screen_definition("snapshot-connection-refused");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        drop(listener);

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(100)),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        let _ = fs::remove_file(screen_path);

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("io error"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_response_times_out() {
        let screen_path = write_test_screen_definition("snapshot-timeout");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept");
            std::thread::sleep(std::time::Duration::from_millis(300));
        });

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(50)),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("io error"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_projection_accepts_bearer_and_forwards_auth_to_tag_server() {
        let screen_path = write_test_screen_definition("projection-auth-forward");

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);
            assert_eq!(Some("Bearer secret"), request.header("authorization"));

            let response = RuntimeHttpResponse::json(
                200,
                r#"{"values":[{"tag_id":"mock.running.001","value":true,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:01Z","server_timestamp":"1970-01-01T00:00:01Z","sequence":3,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}],"missing_tag_ids":[]}"#
                    .to_string(),
            );
            stream.write_all(&response.to_http_bytes()).expect("write");
        });

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_token(Some("secret".to_string())),
        )
        .with_required_token(Some("secret".to_string()))
        .with_default_screen_path(screen_path.clone());

        let request = RuntimeHttpRequest::new("POST", "/api/v1/screens/projection", r#"{}"#)
            .with_header("authorization", "Bearer secret");
        let response = api.handle(&request);

        server.join().expect("server");
        let _ = fs::remove_file(screen_path);

        assert_eq!(200, response.status_code);
        assert!(response.body.contains(r#""screen_id":"main""#));
        assert!(response.body.contains(r#""value":true"#));
    }

    fn write_test_screen_definition(name: &str) -> PathBuf {
        let screen_path = std::env::temp_dir().join(format!(
            "scada-preview-runtime-screen-{name}-{}.json",
            std::process::id()
        ));
        fs::write(
            &screen_path,
            r#"{"schema_version":"1.0.0","screen_id":"main","project_id":"demo","name":"Main","canvas_width":1280,"canvas_height":720,"objects":[{"object_id":"pump-001","svg_asset_id":"pump","x":0.0,"y":0.0,"width":100.0,"height":100.0,"tag_bindings":{"state":"mock.running.001"}}]}"#,
        )
        .expect("write screen");
        screen_path
    }

    fn runtime_api_handle_control_command_with_tag_server_response(
        status_code: u16,
        body: &str,
    ) -> RuntimeHttpResponse {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let response_body = body.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/control-commands", request.path);
            assert!(request.body.contains(r#""command_id":"cmd-1""#));

            let response = RuntimeHttpResponse::json(status_code, response_body);
            stream.write_all(&response.to_http_bytes()).expect("write");
        });
        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}")).expect("client"),
        );

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            valid_control_command_json(),
        ));
        server.join().expect("server");
        response
    }

    fn runtime_api_handle_control_command_with_raw_response(raw_response: &str) -> RuntimeHttpResponse {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let raw = raw_response.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/control-commands", request.path);
            assert!(request.body.contains(r#""command_id":"cmd-1""#));
            stream.write_all(raw.as_bytes()).expect("write raw response");
        });

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(200)),
        );

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/control-commands",
            valid_control_command_json(),
        ));
        server.join().expect("server");
        response
    }

    fn runtime_api_handle_projection_with_raw_snapshot_response(
        raw_response: &str,
    ) -> RuntimeHttpResponse {
        let screen_path = write_test_screen_definition("snapshot-raw-http-response");
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let addr = listener.local_addr().expect("addr");
        let raw = raw_response.to_string();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let request = read_runtime_http_request(&mut stream).expect("request");
            assert_eq!("POST", request.method);
            assert_eq!("/api/v1/tags/snapshot", request.path);
            stream.write_all(raw.as_bytes()).expect("write raw response");
        });

        let api = RuntimeApi::new(
            TagServerClient::from_base_url(&format!("http://{addr}"))
                .expect("client")
                .with_timeout(std::time::Duration::from_millis(200)),
        )
        .with_default_screen_path(screen_path.clone());

        let response = api.handle(&RuntimeHttpRequest::new(
            "POST",
            "/api/v1/screens/projection",
            r#"{}"#,
        ));
        server.join().expect("server");
        let _ = fs::remove_file(screen_path);
        response
    }

    fn valid_control_command_json() -> &'static str {
        r#"{"command_id":"cmd-1","idempotency_key":"key-1","user_id":"operator","tag_id":"mock.running.001","requested_value":true,"status":"Requested","requested_at":"1970-01-01T00:00:05Z","timeout_ms":1000}"#
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_http_header_terminator_is_missing() {
        let response = runtime_api_handle_projection_with_raw_snapshot_response(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("missing response header terminator"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_http_status_line_is_missing() {
        let response = runtime_api_handle_projection_with_raw_snapshot_response(
            "\r\n\r\n{\"values\":[],\"missing_tag_ids\":[]}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("missing status line"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_http_version_is_invalid() {
        let response = runtime_api_handle_projection_with_raw_snapshot_response(
            "HTTX/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"values\":[],\"missing_tag_ids\":[]}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("invalid http version"));
    }

    #[test]
    #[ignore = "requires local TCP listener"]
    fn runtime_api_returns_bad_gateway_when_snapshot_http_status_code_is_invalid() {
        let response = runtime_api_handle_projection_with_raw_snapshot_response(
            "HTTP/1.1 xyz OOPS\r\nContent-Type: application/json\r\n\r\n{\"values\":[],\"missing_tag_ids\":[]}",
        );

        assert_eq!(502, response.status_code);
        assert!(response.body.contains("tag server snapshot failed"));
        assert!(response.body.contains("http error"));
        assert!(response.body.contains("invalid status code"));
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
    fn resolve_screen_tag_ids_deduplicates_with_stable_order() {
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
                "mock.running.001".to_string(),
                "mock.temperature.001".to_string()
            ],
            tags
        );
    }

    #[test]
    fn projection_maps_values_to_object_bindings() {
        let definition = ScreenDefinition {
            schema_version: "1.0.0".to_string(),
            screen_id: "main".to_string(),
            project_id: "demo".to_string(),
            name: "Main".to_string(),
            canvas_width: 1280,
            canvas_height: 720,
            objects: vec![ScreenObjectDefinition {
                object_id: "obj-1".to_string(),
                svg_asset_id: "pump".to_string(),
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                tag_bindings: HashMap::from([
                    ("value".to_string(), "mock.temperature.001".to_string()),
                    ("state".to_string(), "missing.tag".to_string()),
                ]),
            }],
        };
        let snapshot = TagSnapshot {
            values: vec![RuntimeTagValue {
                tag_id: "mock.temperature.001".to_string(),
                value: Value::from(21.0),
                data_type: "float".to_string(),
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
            missing_tag_ids: vec!["missing.tag".to_string()],
        };

        let projection = project_snapshot_to_screen(&definition, &snapshot);

        assert_eq!("main", projection.screen_id);
        assert_eq!(1, projection.object_states.len());
        assert_eq!(2, projection.object_states[0].bindings.len());
        assert_eq!(
            Some(Value::from(21.0)),
            projection.object_states[0].bindings[1].value
        );
        assert_eq!(None, projection.object_states[0].bindings[0].value);
    }

    #[test]
    fn delta_parser_requires_matching_topic() {
        let payload = r#"{"tag_id":"mock.running.001","value":true,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:01Z","server_timestamp":"1970-01-01T00:00:01Z","sequence":2,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}"#;

        let delta = parse_tag_value_delta("demo", "scada/demo/tag/mock.running.001/value", payload)
            .expect("delta");

        assert_eq!("mock.running.001", delta.value.tag_id);
        assert!(parse_tag_value_delta("demo", "scada/demo/tag/other/value", payload).is_err());
    }

    #[test]
    fn delta_apply_updates_newer_bindings_and_ignores_stale() {
        let mut projection = ScreenProjection {
            screen_id: "main".to_string(),
            project_id: "demo".to_string(),
            object_states: vec![ScreenObjectState {
                object_id: "pump".to_string(),
                svg_asset_id: "pump-symbol".to_string(),
                bindings: vec![
                    ObjectBindingState {
                        key: "state".to_string(),
                        tag_id: "mock.running.001".to_string(),
                        value: Some(Value::Bool(false)),
                        quality: Some("Simulated".to_string()),
                        sequence: Some(1),
                    },
                    ObjectBindingState {
                        key: "old".to_string(),
                        tag_id: "mock.temperature.001".to_string(),
                        value: Some(Value::from(21.0)),
                        quality: Some("Simulated".to_string()),
                        sequence: Some(5),
                    },
                ],
            }],
        };
        let newer = RuntimeTagValue {
            tag_id: "mock.running.001".to_string(),
            value: Value::Bool(true),
            data_type: "boolean".to_string(),
            quality: "Simulated".to_string(),
            source_timestamp: "1970-01-01T00:00:02Z".to_string(),
            server_timestamp: "1970-01-01T00:00:02Z".to_string(),
            sequence: 2,
            scan_interval_ms: 1000,
            stale_after_ms: 3000,
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        };
        let stale = RuntimeTagValue {
            tag_id: "mock.temperature.001".to_string(),
            value: Value::from(19.0),
            data_type: "float".to_string(),
            quality: "Simulated".to_string(),
            source_timestamp: "1970-01-01T00:00:01Z".to_string(),
            server_timestamp: "1970-01-01T00:00:01Z".to_string(),
            sequence: 4,
            scan_interval_ms: 1000,
            stale_after_ms: 3000,
            driver_id: "mock-driver".to_string(),
            endpoint_id: "mock-endpoint".to_string(),
            read_status: "ok".to_string(),
            write_status: "idle".to_string(),
        };

        let newer_result = apply_delta_to_projection(&mut projection, &newer);
        let stale_result = apply_delta_to_projection(&mut projection, &stale);

        assert_eq!(1, newer_result.applied_bindings);
        assert_eq!(
            Some(Value::Bool(true)),
            projection.object_states[0].bindings[0].value
        );
        assert_eq!(1, stale_result.ignored_stale_bindings);
        assert_eq!(
            Some(Value::from(21.0)),
            projection.object_states[0].bindings[1].value
        );
    }

    #[test]
    fn mqtt_topic_filter_matches_design() {
        assert_eq!(
            "scada/demo/tag/+/value",
            mqtt_tag_value_topic_filter("demo")
        );
    }
}
