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
}
