use std::path::{Path, PathBuf};
use std::process::Command;

use scada_core::service::{default_local_services, LocalServiceSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHealthResult {
    pub service: String,
    pub healthy: bool,
    pub output: String,
}

pub fn default_service_bin_dir(current_exe: &Path) -> PathBuf {
    current_exe
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn service_binary_path(bin_dir: &Path, service: &LocalServiceSpec) -> PathBuf {
    bin_dir.join(service.binary)
}

pub fn parse_health_output(service: &LocalServiceSpec, output: &str) -> ServiceHealthResult {
    let expected = format!("{}: healthy", service.role.as_str());
    let trimmed = output.trim().to_string();

    ServiceHealthResult {
        service: service.role.as_str().to_string(),
        healthy: trimmed == expected,
        output: trimmed,
    }
}

pub fn check_service_health(bin_dir: &Path, service: &LocalServiceSpec) -> ServiceHealthResult {
    let binary = service_binary_path(bin_dir, service);
    let output = Command::new(&binary).arg(service.health_arg).output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                parse_health_output(service, &stdout)
            } else {
                ServiceHealthResult {
                    service: service.role.as_str().to_string(),
                    healthy: false,
                    output: stderr.trim().to_string(),
                }
            }
        }
        Err(error) => ServiceHealthResult {
            service: service.role.as_str().to_string(),
            healthy: false,
            output: error.to_string(),
        },
    }
}

pub fn check_default_services(bin_dir: &Path) -> Vec<ServiceHealthResult> {
    default_local_services()
        .iter()
        .map(|service| check_service_health(bin_dir, service))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scada_core::service::{LocalServiceSpec, ServiceRole};

    fn service() -> LocalServiceSpec {
        LocalServiceSpec {
            role: ServiceRole::TagServer,
            binary: "tag-server",
            health_arg: "--health",
        }
    }

    #[test]
    fn parser_accepts_expected_health_output() {
        let result = parse_health_output(&service(), "tag-server: healthy\n");

        assert!(result.healthy);
        assert_eq!("tag-server", result.service);
    }

    #[test]
    fn parser_rejects_wrong_health_output() {
        let result = parse_health_output(&service(), "tag-server: broken\n");

        assert!(!result.healthy);
    }

    #[test]
    fn binary_path_uses_supplied_directory() {
        let path = service_binary_path(Path::new("/tmp/scada"), &service());

        assert_eq!(PathBuf::from("/tmp/scada/tag-server"), path);
    }
}
