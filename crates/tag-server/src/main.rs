use std::time::Duration;

use scada_core::mqtt::MqttBrokerEndpoint;
use scada_core::service::{print_health, ServiceRole, LOCAL_BIND_HOST_ENV, LOCAL_TOKEN_ENV};
use tag_server::{run_server, MqttPublishConfig, TagServerApi};

const TAG_SERVER_PORT_ENV: &str = "SCADA_TAG_SERVER_PORT";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::TagServer);
        return;
    }
    if args.iter().any(|arg| arg == "--serve") {
        let addr = serve_addr(&args);
        let required_token = std::env::var(LOCAL_TOKEN_ENV).ok();
        let mqtt = mqtt_config_from_args(&args);
        let mut api = TagServerApi::phase0_mock()
            .with_required_token(required_token)
            .with_mqtt_publish(mqtt);

        eprintln!("tag-server listening on {addr}");
        if let Err(error) = run_server(&addr, &mut api) {
            eprintln!("tag-server failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    println!("tag-server skeleton");
}

fn mqtt_config_from_args(args: &[String]) -> Option<MqttPublishConfig> {
    let url = arg_value(args, "--mqtt-url")?;
    let endpoint = match MqttBrokerEndpoint::parse(&url) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("tag-server mqtt config failed: invalid --mqtt-url: {error}");
            std::process::exit(1);
        }
    };
    let client_id = arg_value(args, "--mqtt-client-id")
        .unwrap_or_else(|| format!("tag-server-{}", std::process::id()));
    let timeout_secs = arg_value(args, "--mqtt-timeout-secs")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(5);

    Some(MqttPublishConfig {
        endpoint,
        client_id,
        timeout: Duration::from_secs(timeout_secs),
    })
}

fn serve_addr(args: &[String]) -> String {
    if let Some(addr) = arg_value(args, "--addr") {
        return addr;
    }

    let host = arg_value(args, "--bind-host")
        .or_else(|| std::env::var(LOCAL_BIND_HOST_ENV).ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = arg_value(args, "--port")
        .or_else(|| std::env::var(TAG_SERVER_PORT_ENV).ok())
        .unwrap_or_else(|| "18080".to_string());

    format!("{host}:{port}")
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}
