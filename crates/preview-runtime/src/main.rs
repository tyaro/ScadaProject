use std::path::PathBuf;
use std::time::Duration;

use preview_runtime::{
    apply_delta_to_projection, default_snapshot_tags, format_delta_apply_result,
    format_screen_projection_summary, format_snapshot_summary, load_screen_definition,
    parse_tag_value_delta, project_snapshot_to_screen, publish_delta_via_mqtt,
    receive_delta_once_via_mqtt, resolve_tag_ids_from_screen, run_runtime_server,
    subscribe_deltas_via_mqtt, MqttConnectionConfig, RuntimeApi, RuntimeTagValue, TagServerClient,
};
use scada_core::mqtt::{MqttBrokerEndpoint, MqttBrokerTransport, DEFAULT_MQTT_TCP_PORT};
use scada_core::service::{print_health, ServiceRole, LOCAL_TOKEN_ENV};

const TAG_SERVER_URL_ENV: &str = "SCADA_TAG_SERVER_URL";
const DEFAULT_SCREEN_PATH: &str = "config/screens/mock-main.screen.json";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::PreviewRuntime);
        return;
    }
    if args.iter().any(|arg| arg == "--serve") {
        let addr = serve_addr(&args);
        let tag_server_url = arg_value(&args, "--tag-server")
            .or_else(|| std::env::var(TAG_SERVER_URL_ENV).ok())
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string());
        let screen_path = arg_value(&args, "--screen")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SCREEN_PATH));
        let token = arg_value(&args, "--token").or_else(|| std::env::var(LOCAL_TOKEN_ENV).ok());
        let client = match TagServerClient::from_base_url(&tag_server_url) {
            Ok(client) => client.with_token(token.clone()),
            Err(error) => {
                eprintln!("preview-runtime serve failed: {error}");
                std::process::exit(1);
            }
        };
        let api = RuntimeApi::new(client)
            .with_required_token(token)
            .with_default_screen_path(screen_path);

        eprintln!("preview-runtime listening on {addr}");
        if let Err(error) = run_runtime_server(&addr, &api) {
            eprintln!("preview-runtime failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--snapshot") {
        let tag_server_url = arg_value(&args, "--tag-server")
            .or_else(|| std::env::var(TAG_SERVER_URL_ENV).ok())
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string());
        let project_id = arg_value(&args, "--project-id").unwrap_or_else(|| "demo".to_string());
        let tags = {
            let values = arg_values(&args, "--tag");
            if values.is_empty() {
                default_snapshot_tags()
            } else {
                values
            }
        };
        let token = arg_value(&args, "--token").or_else(|| std::env::var(LOCAL_TOKEN_ENV).ok());
        let client = match TagServerClient::from_base_url(&tag_server_url) {
            Ok(client) => client.with_token(token),
            Err(error) => {
                eprintln!("preview-runtime snapshot failed: {error}");
                std::process::exit(1);
            }
        };

        match client.fetch_snapshot(&project_id, &tags) {
            Ok(snapshot) => println!("{}", format_snapshot_summary(&snapshot)),
            Err(error) => {
                eprintln!("preview-runtime snapshot failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--snapshot-screen") {
        let tag_server_url = arg_value(&args, "--tag-server")
            .or_else(|| std::env::var(TAG_SERVER_URL_ENV).ok())
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string());
        let screen_path = arg_value(&args, "--screen")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SCREEN_PATH));
        let definition = match load_screen_definition(&screen_path) {
            Ok(definition) => definition,
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        };
        let tags = resolve_tag_ids_from_screen(&definition);
        if tags.is_empty() {
            eprintln!(
                "preview-runtime snapshot-screen failed: no tags resolved from {}",
                screen_path.display()
            );
            std::process::exit(1);
        }
        let token = arg_value(&args, "--token").or_else(|| std::env::var(LOCAL_TOKEN_ENV).ok());
        let client = match TagServerClient::from_base_url(&tag_server_url) {
            Ok(client) => client.with_token(token),
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        };

        match client.fetch_snapshot(&definition.project_id, &tags) {
            Ok(snapshot) => {
                let mut projection = project_snapshot_to_screen(&definition, &snapshot);
                println!(
                    "screen={} objects={} tags={}",
                    definition.screen_id,
                    definition.objects.len(),
                    tags.len()
                );
                println!("{}", format_snapshot_summary(&snapshot));
                println!("{}", format_screen_projection_summary(&projection));

                if args.iter().any(|arg| arg == "--simulate-delta") {
                    let topic = arg_value(&args, "--delta-topic")
                        .unwrap_or_else(|| "scada/demo/tag/mock.running.001/value".to_string());
                    let payload = arg_value(&args, "--delta-payload").unwrap_or_else(|| {
                        r#"{"tag_id":"mock.running.001","value":true,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:02Z","server_timestamp":"1970-01-01T00:00:02Z","sequence":2,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}"#.to_string()
                    });

                    match parse_tag_value_delta(&definition.project_id, &topic, &payload) {
                        Ok(delta) => {
                            let result = apply_delta_to_projection(&mut projection, &delta.value);
                            println!("{}", format_delta_apply_result(&result));
                            println!("{}", format_screen_projection_summary(&projection));
                        }
                        Err(error) => {
                            eprintln!("preview-runtime simulate-delta failed: {error}");
                            std::process::exit(1);
                        }
                    }
                }

                if args.iter().any(|arg| arg == "--mqtt-subscribe") {
                    let config = mqtt_config_from_args(&args, "preview-runtime-sub");
                    let verbose_projection = args.iter().any(|arg| arg == "--verbose-projection");
                    run_snapshot_screen_mqtt_loop(
                        &client,
                        &definition,
                        &definition.project_id,
                        &tags,
                        &config,
                        &mut projection,
                        verbose_projection,
                    );
                }
            }
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--mqtt-receive-once") {
        let project_id = arg_value(&args, "--project-id").unwrap_or_else(|| "demo".to_string());
        let config = mqtt_config_from_args(&args, "preview-runtime-recv");
        match receive_delta_once_via_mqtt(&config, &project_id) {
            Ok(delta) => {
                println!("mqtt received topic={}", delta.topic);
                println!(
                    "{} {} {} {}",
                    delta.value.tag_id,
                    delta.value.data_type,
                    delta.value.quality,
                    delta.value.value
                );
            }
            Err(error) => {
                eprintln!("preview-runtime mqtt-receive-once failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--mqtt-publish-delta") {
        let project_id = arg_value(&args, "--project-id").unwrap_or_else(|| "demo".to_string());
        let config = mqtt_config_from_args(&args, "preview-runtime-pub");
        let payload = arg_value(&args, "--delta-payload").unwrap_or_else(|| {
            r#"{"tag_id":"mock.running.001","value":true,"data_type":"boolean","quality":"Simulated","source_timestamp":"1970-01-01T00:00:02Z","server_timestamp":"1970-01-01T00:00:02Z","sequence":2,"scan_interval_ms":1000,"stale_after_ms":3000,"driver_id":"mock-driver","endpoint_id":"mock-endpoint","read_status":"ok","write_status":"idle"}"#.to_string()
        });
        let delta: RuntimeTagValue = match serde_json::from_str(&payload) {
            Ok(delta) => delta,
            Err(error) => {
                eprintln!("preview-runtime mqtt-publish-delta failed: invalid payload: {error}");
                std::process::exit(1);
            }
        };

        match publish_delta_via_mqtt(&config, &delta, &project_id) {
            Ok(()) => println!(
                "mqtt published tag={} seq={} project={}",
                delta.tag_id, delta.sequence, project_id
            ),
            Err(error) => {
                eprintln!("preview-runtime mqtt-publish-delta failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    println!("preview-runtime skeleton");
}

fn run_snapshot_screen_mqtt_loop(
    client: &TagServerClient,
    definition: &preview_runtime::ScreenDefinition,
    project_id: &str,
    tags: &[String],
    config: &MqttConnectionConfig,
    projection: &mut preview_runtime::ScreenProjection,
    verbose_projection: bool,
) {
    let mut reconnect_failures: u32 = 0;
    loop {
        match subscribe_deltas_via_mqtt(config, project_id, |delta| {
            let result = apply_delta_to_projection(projection, &delta.value);
            println!("{}", format_delta_apply_result(&result));
            print_projection_log(projection, verbose_projection);
        }) {
            Ok(()) => {}
            Err(error) => {
                reconnect_failures = reconnect_failures.saturating_add(1);
                let backoff_secs = reconnect_backoff_secs(reconnect_failures);
                eprintln!("preview-runtime mqtt-subscribe receive failed: {error}");
                eprintln!("preview-runtime mqtt-subscribe refreshing snapshot");
                match client.fetch_snapshot(project_id, tags) {
                    Ok(snapshot) => {
                        *projection = project_snapshot_to_screen(definition, &snapshot);
                        println!("{}", format_snapshot_summary(&snapshot));
                        print_projection_log(projection, verbose_projection);
                    }
                    Err(refresh_error) => {
                        eprintln!(
                            "preview-runtime mqtt-subscribe snapshot refresh failed: {refresh_error}"
                        );
                    }
                }
                eprintln!("preview-runtime mqtt-subscribe reconnect backoff={backoff_secs}s");
                std::thread::sleep(Duration::from_secs(backoff_secs));
            }
        }
    }
}

fn print_projection_log(projection: &preview_runtime::ScreenProjection, verbose_projection: bool) {
    let summary = format_screen_projection_summary(projection);
    if verbose_projection {
        println!("{summary}");
        return;
    }

    let compact = summary.lines().next().unwrap_or("projection unavailable");
    println!("{compact}");
}

fn reconnect_backoff_secs(failures: u32) -> u64 {
    let capped_failures = failures.min(6);
    (1u64 << (capped_failures - 1)).min(30)
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn arg_values(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}

fn serve_addr(args: &[String]) -> String {
    arg_value(args, "--addr").unwrap_or_else(|| {
        let host = std::env::var("SCADA_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("SCADA_PREVIEW_RUNTIME_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(18090);
        format!("{host}:{port}")
    })
}

fn mqtt_config_from_args(args: &[String], client_prefix: &str) -> MqttConnectionConfig {
    let endpoint = if let Some(url) = arg_value(args, "--mqtt-url") {
        match MqttBrokerEndpoint::parse(&url) {
            Ok(endpoint) => endpoint,
            Err(error) => {
                eprintln!("preview-runtime mqtt config failed: invalid --mqtt-url: {error}");
                std::process::exit(1);
            }
        }
    } else {
        let host = arg_value(args, "--mqtt-host").unwrap_or_else(|| "127.0.0.1".to_string());
        let port = arg_value(args, "--mqtt-port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_MQTT_TCP_PORT);
        let transport = if args.iter().any(|arg| arg == "--mqtt-websocket") {
            MqttBrokerTransport::WebSocket
        } else {
            MqttBrokerTransport::Tcp
        };
        let websocket_path =
            arg_value(args, "--mqtt-ws-path").unwrap_or_else(|| "/mqtt".to_string());
        MqttBrokerEndpoint {
            host,
            port,
            transport,
            websocket_path,
        }
    };
    let timeout_secs = arg_value(args, "--mqtt-timeout-secs")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);
    let client_id = arg_value(args, "--mqtt-client-id")
        .unwrap_or_else(|| format!("{client_prefix}-{}", std::process::id()));

    MqttConnectionConfig {
        endpoint,
        client_id,
        timeout: Duration::from_secs(timeout_secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_backoff_secs_increases_and_caps_at_30() {
        assert_eq!(1, reconnect_backoff_secs(1));
        assert_eq!(2, reconnect_backoff_secs(2));
        assert_eq!(4, reconnect_backoff_secs(3));
        assert_eq!(8, reconnect_backoff_secs(4));
        assert_eq!(16, reconnect_backoff_secs(5));
        assert_eq!(30, reconnect_backoff_secs(6));
        assert_eq!(30, reconnect_backoff_secs(10));
    }
}
