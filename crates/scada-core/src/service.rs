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
