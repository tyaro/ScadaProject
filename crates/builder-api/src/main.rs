use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use scada_core::service::{print_health, ServiceRole};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodedError {
    code: String,
    path: String,
    detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ConditionErrorMapping {
    code: Option<String>,
    path: Option<String>,
    detail: Option<String>,
    user_message: String,
    known_code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct ErrorMapRequest {
    error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SaveScreenResponse {
    saved_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuilderHttpRequest {
    method: String,
    path: String,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BuilderHttpResponse {
    status_code: u16,
    reason: &'static str,
    content_type: &'static str,
    body: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::BuilderApi);
        return;
    }

    if args.iter().any(|arg| arg == "--serve") {
        let addr = arg_value(&args, "--addr").unwrap_or_else(|| "127.0.0.1:18110".to_string());
        eprintln!("builder-api listening on {addr}");
        if let Err(error) = run_builder_server(&addr) {
            eprintln!("builder-api serve failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(raw) = arg_value(&args, "--map-error") {
        println!("{}", map_condition_error_message(&raw));
        return;
    }

    if let Some(raw) = arg_value(&args, "--map-error-json") {
        let mapped = map_condition_error(&raw);
        println!("{}", serde_json::to_string(&mapped).unwrap_or_else(|_| error_json("failed to serialize error mapping")));
        return;
    }

    println!("builder-api skeleton");
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn run_builder_server(addr: &str) -> Result<(), String> {
    let listener = TcpListener::bind(addr).map_err(|error| error.to_string())?;
    for incoming in listener.incoming() {
        let mut stream = incoming.map_err(|error| error.to_string())?;
        handle_builder_connection(&mut stream)?;
    }

    Ok(())
}

fn handle_builder_connection(stream: &mut TcpStream) -> Result<(), String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;

    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|error| error.to_string())?;

    let response = match parse_http_request(&raw) {
        Ok(request) => handle_builder_request(&request),
        Err(message) => BuilderHttpResponse::json(400, error_json(&message)),
    };

    stream
        .write_all(&response.to_http_bytes())
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn parse_http_request(raw: &str) -> Result<BuilderHttpRequest, String> {
    let (head, body) = raw
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid http request".to_string())?;
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "missing path".to_string())?
        .to_string();

    Ok(BuilderHttpRequest {
        method,
        path,
        body: body.to_string(),
    })
}

fn handle_builder_request(request: &BuilderHttpRequest) -> BuilderHttpResponse {
    let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    handle_builder_request_with_project_root(request, &project_root)
}

fn handle_builder_request_with_project_root(
    request: &BuilderHttpRequest,
    project_root: &Path,
) -> BuilderHttpResponse {
    if request.method == "GET" && request.path_without_query() == "/health" {
        return BuilderHttpResponse::json(
            200,
            r#"{"service":"builder-api","status":"healthy"}"#.to_string(),
        );
    }

    if request.method == "POST" && request.path_without_query() == "/api/v1/errors/map" {
        let payload: ErrorMapRequest = match serde_json::from_str(request.body.as_str()) {
            Ok(payload) => payload,
            Err(error) => {
                return BuilderHttpResponse::json(
                    400,
                    error_json(&format!("invalid error map JSON: {error}")),
                );
            }
        };

        let mapped = map_condition_error(&payload.error);
        return match serde_json::to_string(&mapped) {
            Ok(body) => BuilderHttpResponse::json(200, body),
            Err(error) => BuilderHttpResponse::json(500, error_json(&error.to_string())),
        };
    }

    if let Some(screen_id) = screen_id_from_path(request.path_without_query()) {
        if !is_valid_screen_id(screen_id) {
            return BuilderHttpResponse::json(400, error_json("invalid screen id"));
        }

        if request.method == "GET" {
            let path = screen_file_path(project_root, screen_id);
            return match fs::read_to_string(&path) {
                Ok(body) => BuilderHttpResponse::json(200, body),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    BuilderHttpResponse::json(404, error_json("screen definition not found"))
                }
                Err(error) => BuilderHttpResponse::json(500, error_json(&error.to_string())),
            };
        }

        if request.method == "PUT" {
            let parsed: serde_json::Value = match serde_json::from_str(&request.body) {
                Ok(value) => value,
                Err(error) => {
                    return BuilderHttpResponse::json(
                        400,
                        error_json(&format!("invalid screen-definition JSON: {error}")),
                    );
                }
            };

            if !is_screen_definition_shape(&parsed) {
                return BuilderHttpResponse::json(
                    400,
                    error_json("invalid screen-definition shape"),
                );
            }

            if parsed["screen_id"].as_str() != Some(screen_id) {
                return BuilderHttpResponse::json(
                    400,
                    error_json("screen_id in body must match path"),
                );
            }

            let screens_dir = project_root.join("config").join("screens");
            if let Err(error) = fs::create_dir_all(&screens_dir) {
                return BuilderHttpResponse::json(500, error_json(&error.to_string()));
            }

            let normalized = match serde_json::to_string_pretty(&parsed) {
                Ok(value) => value,
                Err(error) => {
                    return BuilderHttpResponse::json(500, error_json(&error.to_string()));
                }
            };

            let path = screen_file_path(project_root, screen_id);
            if let Err(error) = fs::write(&path, normalized + "\n") {
                return BuilderHttpResponse::json(500, error_json(&error.to_string()));
            }

            let response = SaveScreenResponse {
                saved_path: format!("config/screens/{}.screen.json", screen_id),
            };
            return match serde_json::to_string(&response) {
                Ok(body) => BuilderHttpResponse::json(200, body),
                Err(error) => BuilderHttpResponse::json(500, error_json(&error.to_string())),
            };
        }
    }

    BuilderHttpResponse::json(404, error_json("not found"))
}

fn is_screen_definition_shape(value: &serde_json::Value) -> bool {
    let Some(record) = value.as_object() else {
        return false;
    };

    record.get("schema_version").and_then(|item| item.as_str()).is_some()
        && record.get("screen_id").and_then(|item| item.as_str()).is_some()
        && record.get("project_id").and_then(|item| item.as_str()).is_some()
        && record.get("name").and_then(|item| item.as_str()).is_some()
        && record
            .get("canvas_width")
            .and_then(|item| item.as_i64())
            .is_some()
        && record
            .get("canvas_height")
            .and_then(|item| item.as_i64())
            .is_some()
        && record
            .get("objects")
            .and_then(|item| item.as_array())
            .is_some()
}

fn screen_id_from_path(path: &str) -> Option<&str> {
    const PREFIX: &str = "/api/v1/screens/";
    if !path.starts_with(PREFIX) {
        return None;
    }
    let id = &path[PREFIX.len()..];
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id)
}

fn is_valid_screen_id(screen_id: &str) -> bool {
    screen_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

fn screen_file_path(project_root: &Path, screen_id: &str) -> PathBuf {
    project_root
        .join("config")
        .join("screens")
        .join(format!("{}.screen.json", screen_id))
}

fn error_json(message: &str) -> String {
    format!(r#"{{"error":"{}"}}"#, message.replace('"', "\\\""))
}

fn parse_coded_error(raw: &str) -> Option<CodedError> {
    let code_start = raw.find("code=")? + "code=".len();
    let path_marker = " path=";
    let path_start_marker = raw[code_start..].find(path_marker)? + code_start;
    let code = raw[code_start..path_start_marker].trim().to_string();

    let path_start = path_start_marker + path_marker.len();
    let detail_marker = " detail=";
    let detail_start_marker = raw[path_start..].find(detail_marker)? + path_start;
    let path = raw[path_start..detail_start_marker].trim().to_string();

    let detail_start = detail_start_marker + detail_marker.len();
    let detail = raw[detail_start..].trim().to_string();

    Some(CodedError { code, path, detail })
}

fn map_condition_error_message(raw: &str) -> String {
    map_condition_error(raw).user_message
}

fn map_condition_error(raw: &str) -> ConditionErrorMapping {
    let Some(parsed) = parse_coded_error(raw) else {
        return ConditionErrorMapping {
            code: None,
            path: None,
            detail: None,
            user_message: raw.to_string(),
            known_code: false,
        };
    };

    let message = match parsed.code.as_str() {
        "MODIFY_RULE_CONDITION_MISSING_SELECTOR" => {
            format!("invalid modify rule condition at {}: choose op, all, or any", parsed.path)
        }
        "MODIFY_RULE_CONDITION_VALUE_REQUIRED" => {
            format!("invalid modify rule condition at {}: missing value", parsed.path)
        }
        "MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX" => {
            format!(
                "invalid modify rule condition at {}: between requires min and max",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_BETWEEN_RANGE_INVALID" => {
            format!(
                "invalid modify rule condition at {}: min must be less than or equal to max",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_IN_REQUIRES_VALUES" => {
            format!(
                "invalid modify rule condition at {}: in requires non-empty values",
                parsed.path
            )
        }
        "MODIFY_RULE_CONDITION_UNSUPPORTED_OP" => {
            format!(
                "invalid modify rule condition at {}: unsupported operator",
                parsed.path
            )
        }
        _ => raw.to_string(),
    };

    let known_code = parsed.code.as_str() != "" && !message.eq(raw);

    ConditionErrorMapping {
        code: Some(parsed.code),
        path: Some(parsed.path),
        detail: Some(parsed.detail),
        user_message: message,
        known_code,
    }
}

impl BuilderHttpRequest {
    fn new(method: &str, path: &str, body: &str) -> Self {
        Self {
            method: method.to_string(),
            path: path.to_string(),
            body: body.to_string(),
        }
    }

    fn path_without_query(&self) -> &str {
        self.path.split('?').next().unwrap_or(&self.path)
    }
}

impl BuilderHttpResponse {
    fn json(status_code: u16, body: String) -> Self {
        Self {
            status_code,
            reason: status_reason(status_code),
            content_type: "application/json",
            body,
        }
    }

    fn to_http_bytes(&self) -> Vec<u8> {
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

fn status_reason(status_code: u16) -> &'static str {
    match status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_coded_error_reads_code_path_and_detail() {
        let parsed = parse_coded_error(
            "screen definition error: code=MODIFY_RULE_CONDITION_VALUE_REQUIRED path=object=pump-001 property=color detail=op 'gte' requires value",
        )
        .expect("parsed");

        assert_eq!("MODIFY_RULE_CONDITION_VALUE_REQUIRED", parsed.code);
        assert_eq!("object=pump-001 property=color", parsed.path);
        assert_eq!("op 'gte' requires value", parsed.detail);
    }

    #[test]
    fn map_condition_error_message_returns_user_friendly_text() {
        let mapped = map_condition_error_message(
            "code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op 'between' requires min and max",
        );

        assert_eq!(
            "invalid modify rule condition at object=pump-001 property=color: between requires min and max",
            mapped
        );
    }

    #[test]
    fn map_condition_error_message_keeps_unknown_code_raw() {
        let raw = "code=UNKNOWN_CODE path=object=x property=y detail=unknown";
        assert_eq!(raw, map_condition_error_message(raw));
    }

    #[test]
    fn handle_builder_request_returns_structured_mapping_json() {
        let request = BuilderHttpRequest::new(
            "POST",
            "/api/v1/errors/map",
            r#"{"error":"code=MODIFY_RULE_CONDITION_IN_REQUIRES_VALUES path=object=pump-001 property=color detail=op 'in' requires non-empty values"}"#,
        );
        let response = handle_builder_request(&request);
        let payload: serde_json::Value =
            serde_json::from_str(&response.body).expect("valid json response");

        assert_eq!(200, response.status_code);
        assert_eq!(
            "MODIFY_RULE_CONDITION_IN_REQUIRES_VALUES",
            payload["code"].as_str().expect("code")
        );
        assert_eq!(
            "invalid modify rule condition at object=pump-001 property=color: in requires non-empty values",
            payload["user_message"].as_str().expect("user_message")
        );
        assert_eq!(Some(true), payload["known_code"].as_bool());
    }

    #[test]
    fn handle_builder_request_returns_health_json_contract() {
        let request = BuilderHttpRequest::new("GET", "/health", "");
        let response = handle_builder_request(&request);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(200, response.status_code);
        assert_eq!("application/json", response.content_type);
        assert_eq!(Some("builder-api"), payload["service"].as_str());
        assert_eq!(Some("healthy"), payload["status"].as_str());
    }

    #[test]
    fn handle_builder_request_returns_unknown_code_contract_fields() {
        let raw =
            "code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state";
        let request = BuilderHttpRequest::new(
            "POST",
            "/api/v1/errors/map",
            &format!(r#"{{"error":"{}"}}"#, raw),
        );
        let response = handle_builder_request(&request);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(200, response.status_code);
        assert_eq!("application/json", response.content_type);
        assert_eq!(Some("SOME_NEW_ERROR"), payload["code"].as_str());
        assert_eq!(Some("object=valve-002 property=text"), payload["path"].as_str());
        assert_eq!(Some("unexpected runtime validation state"), payload["detail"].as_str());
        assert_eq!(Some(false), payload["known_code"].as_bool());
        assert_eq!(Some(raw), payload["user_message"].as_str());
    }

    #[test]
    fn handle_builder_request_returns_bad_request_for_invalid_json() {
        let request = BuilderHttpRequest::new("POST", "/api/v1/errors/map", "{not-json");
        let response = handle_builder_request(&request);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(400, response.status_code);
        assert_eq!("application/json", response.content_type);
        assert!(payload["error"]
            .as_str()
            .expect("error")
            .contains("invalid error map JSON"));
    }

    fn test_project_root(test_name: &str) -> PathBuf {
        let unique = format!(
            "{}_{}_{}",
            test_name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(path.join("config").join("screens")).expect("create test dir");
        path
    }

    #[test]
    fn handle_builder_request_get_screen_returns_project_file() {
        let project_root = test_project_root("builder_api_get_screen");
        let file_path = project_root
            .join("config")
            .join("screens")
            .join("mock-main.screen.json");
        std::fs::write(
            &file_path,
            r#"{"schema_version":"1.0.0","screen_id":"mock-main","project_id":"demo","name":"Mock","canvas_width":100,"canvas_height":80,"objects":[]}"#,
        )
        .expect("write screen");

        let request = BuilderHttpRequest::new("GET", "/api/v1/screens/mock-main", "");
        let response = handle_builder_request_with_project_root(&request, &project_root);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(200, response.status_code);
        assert_eq!(Some("mock-main"), payload["screen_id"].as_str());
        let _ = std::fs::remove_dir_all(project_root);
    }

    #[test]
    fn handle_builder_request_put_screen_writes_project_file() {
        let project_root = test_project_root("builder_api_put_screen");
        let request = BuilderHttpRequest::new(
            "PUT",
            "/api/v1/screens/mock-main",
            r#"{"schema_version":"1.0.0","screen_id":"mock-main","project_id":"demo","name":"Mock Main Screen","canvas_width":1280,"canvas_height":720,"objects":[]}"#,
        );

        let response = handle_builder_request_with_project_root(&request, &project_root);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");
        let saved_path = project_root
            .join("config")
            .join("screens")
            .join("mock-main.screen.json");

        assert_eq!(200, response.status_code);
        assert_eq!(Some("config/screens/mock-main.screen.json"), payload["saved_path"].as_str());
        assert!(saved_path.exists());
        let _ = std::fs::remove_dir_all(project_root);
    }

    #[test]
    fn handle_builder_request_put_screen_rejects_mismatched_screen_id() {
        let project_root = test_project_root("builder_api_put_mismatch");
        let request = BuilderHttpRequest::new(
            "PUT",
            "/api/v1/screens/mock-main",
            r#"{"schema_version":"1.0.0","screen_id":"other-screen","project_id":"demo","name":"Mock Main Screen","canvas_width":1280,"canvas_height":720,"objects":[]}"#,
        );

        let response = handle_builder_request_with_project_root(&request, &project_root);
        let payload: serde_json::Value = serde_json::from_str(&response.body).expect("json");

        assert_eq!(400, response.status_code);
        assert!(payload["error"]
            .as_str()
            .expect("error")
            .contains("screen_id in body must match path"));
        let _ = std::fs::remove_dir_all(project_root);
    }
}
