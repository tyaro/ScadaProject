use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOCAL_TOKEN_ENV: &str = "SCADA_LOCAL_TOKEN";
pub const LOCAL_BIND_HOST_ENV: &str = "SCADA_BIND_HOST";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceRole {
    BuilderApi,
    PreviewRuntime,
    TagServer,
    DriverManager,
    MockDriver,
    TauriShell,
}

impl ServiceRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuilderApi => "builder-api",
            Self::PreviewRuntime => "preview-runtime",
            Self::TagServer => "tag-server",
            Self::DriverManager => "driver-manager",
            Self::MockDriver => "mock-driver",
            Self::TauriShell => "tauri-shell",
        }
    }
}

pub fn print_health(role: ServiceRole) {
    println!("{}: healthy", role.as_str());
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalServiceSpec {
    pub role: ServiceRole,
    pub binary: &'static str,
    pub health_arg: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRuntimeConfig {
    pub bind_host: String,
    pub startup_token: String,
    pub services: Vec<LocalServiceSpec>,
}

impl LocalRuntimeConfig {
    pub fn new(bind_host: &str, startup_token: String) -> Self {
        Self {
            bind_host: bind_host.to_string(),
            startup_token,
            services: default_local_services(),
        }
    }
}

pub fn generate_startup_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("local-{:x}-{:x}", process::id(), nanos)
}

pub fn default_local_services() -> Vec<LocalServiceSpec> {
    vec![
        LocalServiceSpec {
            role: ServiceRole::BuilderApi,
            binary: "builder-api",
            health_arg: "--health",
        },
        LocalServiceSpec {
            role: ServiceRole::TagServer,
            binary: "tag-server",
            health_arg: "--health",
        },
        LocalServiceSpec {
            role: ServiceRole::DriverManager,
            binary: "driver-manager",
            health_arg: "--health",
        },
        LocalServiceSpec {
            role: ServiceRole::PreviewRuntime,
            binary: "preview-runtime",
            health_arg: "--health",
        },
        LocalServiceSpec {
            role: ServiceRole::MockDriver,
            binary: "mock-driver",
            health_arg: "--health",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_services_include_tag_server_before_driver_manager() {
        let services = default_local_services();
        let tag_server_index = services
            .iter()
            .position(|service| service.role == ServiceRole::TagServer)
            .expect("tag server service");
        let driver_manager_index = services
            .iter()
            .position(|service| service.role == ServiceRole::DriverManager)
            .expect("driver manager service");

        assert!(tag_server_index < driver_manager_index);
    }

    #[test]
    fn startup_token_is_non_empty() {
        assert!(!generate_startup_token().is_empty());
    }

    #[test]
    fn runtime_config_uses_default_services() {
        let config = LocalRuntimeConfig::new("127.0.0.1", "token".to_string());

        assert_eq!("127.0.0.1", config.bind_host);
        assert_eq!("token", config.startup_token);
        assert!(!config.services.is_empty());
    }
}
