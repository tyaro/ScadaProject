pub const ROOT_TOPIC: &str = "scada";
pub const DEFAULT_MQTT_TCP_PORT: u16 = 1883;
pub const DEFAULT_MQTT_WS_PORT: u16 = 8083;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MqttBrokerTransport {
    Tcp,
    WebSocket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttBrokerEndpoint {
    pub host: String,
    pub port: u16,
    pub transport: MqttBrokerTransport,
    pub websocket_path: String,
}

impl MqttBrokerEndpoint {
    pub fn parse(input: &str) -> Result<Self, String> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err("mqtt endpoint is empty".to_string());
        }

        if let Some(without) = raw.strip_prefix("ws://") {
            return parse_ws_endpoint(without);
        }
        if let Some(without) = raw.strip_prefix("mqtt://") {
            return parse_tcp_endpoint(without);
        }
        if let Some(without) = raw.strip_prefix("tcp://") {
            return parse_tcp_endpoint(without);
        }

        parse_tcp_endpoint(raw)
    }

    pub fn websocket_url(&self) -> String {
        format!("ws://{}:{}{}", self.host, self.port, self.websocket_path)
    }
}

fn parse_tcp_endpoint(authority: &str) -> Result<MqttBrokerEndpoint, String> {
    let (host, port) = split_host_port(authority, DEFAULT_MQTT_TCP_PORT)?;
    Ok(MqttBrokerEndpoint {
        host,
        port,
        transport: MqttBrokerTransport::Tcp,
        websocket_path: "/mqtt".to_string(),
    })
}

fn parse_ws_endpoint(without_scheme: &str) -> Result<MqttBrokerEndpoint, String> {
    let (authority, path) = match without_scheme.split_once('/') {
        Some((authority, tail)) => (authority, format!("/{}", tail)),
        None => (without_scheme, "/mqtt".to_string()),
    };
    let (host, port) = split_host_port(authority, DEFAULT_MQTT_WS_PORT)?;
    Ok(MqttBrokerEndpoint {
        host,
        port,
        transport: MqttBrokerTransport::WebSocket,
        websocket_path: path,
    })
}

fn split_host_port(input: &str, default_port: u16) -> Result<(String, u16), String> {
    let trimmed = input.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("missing host".to_string());
    }

    match trimmed.rsplit_once(':') {
        Some((host, port_str)) => {
            if host.is_empty() {
                return Err("missing host".to_string());
            }
            let port = port_str
                .parse::<u16>()
                .map_err(|_| format!("invalid port: {port_str}"))?;
            Ok((host.to_string(), port))
        }
        None => Ok((trimmed.to_string(), default_port)),
    }
}

pub fn tag_value_topic(project_id: &str, tag_id: &str) -> String {
    format!("{}/{}/tag/{}/value", ROOT_TOPIC, project_id, tag_id)
}

pub fn tag_quality_topic(project_id: &str, tag_id: &str) -> String {
    format!("{}/{}/tag/{}/quality", ROOT_TOPIC, project_id, tag_id)
}

pub fn alarm_state_topic(project_id: &str, alarm_id: &str) -> String {
    format!("{}/{}/alarm/{}/state", ROOT_TOPIC, project_id, alarm_id)
}

pub fn driver_status_topic(project_id: &str, driver_id: &str) -> String {
    format!("{}/{}/driver/{}/status", ROOT_TOPIC, project_id, driver_id)
}

pub fn system_event_topic(project_id: &str) -> String {
    format!("{}/{}/event/system", ROOT_TOPIC, project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_value_topic_matches_design() {
        assert_eq!("scada/demo/tag/t1/value", tag_value_topic("demo", "t1"));
    }

    #[test]
    fn driver_status_topic_matches_design() {
        assert_eq!(
            "scada/demo/driver/mock-driver/status",
            driver_status_topic("demo", "mock-driver")
        );
    }

    #[test]
    fn parse_mqtt_endpoint_with_defaults() {
        let endpoint = MqttBrokerEndpoint::parse("127.0.0.1").expect("endpoint");
        assert_eq!("127.0.0.1", endpoint.host);
        assert_eq!(DEFAULT_MQTT_TCP_PORT, endpoint.port);
        assert_eq!(MqttBrokerTransport::Tcp, endpoint.transport);
    }

    #[test]
    fn parse_mqtt_endpoint_with_ws_url() {
        let endpoint =
            MqttBrokerEndpoint::parse("ws://broker.example.local:9001/ws").expect("endpoint");
        assert_eq!("broker.example.local", endpoint.host);
        assert_eq!(9001, endpoint.port);
        assert_eq!(MqttBrokerTransport::WebSocket, endpoint.transport);
        assert_eq!("/ws", endpoint.websocket_path);
        assert_eq!(
            "ws://broker.example.local:9001/ws",
            endpoint.websocket_url()
        );
    }
}
