use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use scada_core::service::{
    default_local_services, generate_startup_token, LocalRuntimeConfig, LocalServiceSpec,
    LOCAL_BIND_HOST_ENV, LOCAL_TOKEN_ENV,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceHealthResult {
    pub service: String,
    pub healthy: bool,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStartPlan {
    pub service: String,
    pub binary_path: PathBuf,
    pub args: Vec<String>,
    pub envs: Vec<(String, String)>,
    pub restart_on_exit: bool,
    pub restart_max_attempts: Option<u32>,
    pub restart_backoff_ms: Option<u64>,
    pub restart_reset_after_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisedServiceStatus {
    pub service: String,
    pub started: bool,
    pub exited: bool,
    pub exit_code: Option<i32>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PickScreenRelativePathResult {
    pub cancelled: bool,
    pub relative_path: Option<String>,
}

impl PickScreenRelativePathResult {
    pub fn cancelled() -> Self {
        Self {
            cancelled: true,
            relative_path: None,
        }
    }

    pub fn selected(relative_path: String) -> Self {
        Self {
            cancelled: false,
            relative_path: Some(relative_path),
        }
    }
}

#[derive(Debug)]
pub struct SupervisedChild {
    pub service: String,
    child: Child,
}

impl SupervisedChild {
    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn stop(&mut self) -> std::io::Result<()> {
        match self.child.try_wait()? {
            Some(_) => Ok(()),
            None => {
                self.child.kill()?;
                let _ = self.child.wait()?;
                Ok(())
            }
        }
    }
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

pub fn create_local_runtime_config(bind_host: &str) -> LocalRuntimeConfig {
    LocalRuntimeConfig::new(bind_host, generate_startup_token())
}

pub fn service_start_plan(bin_dir: &Path, config: &LocalRuntimeConfig) -> Vec<ServiceStartPlan> {
    config
        .services
        .iter()
        .map(|service| ServiceStartPlan {
            service: service.role.as_str().to_string(),
            binary_path: service_binary_path(bin_dir, service),
            args: Vec::new(),
            envs: vec![
                (LOCAL_BIND_HOST_ENV.to_string(), config.bind_host.clone()),
                (LOCAL_TOKEN_ENV.to_string(), config.startup_token.clone()),
            ],
            restart_on_exit: true,
            restart_max_attempts: None,
            restart_backoff_ms: None,
            restart_reset_after_ms: None,
        })
        .collect()
}

pub fn spawn_service(plan: &ServiceStartPlan) -> Result<SupervisedChild, String> {
    let mut command = Command::new(&plan.binary_path);
    command.args(&plan.args);
    for (key, value) in &plan.envs {
        command.env(key, value);
    }
    let child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;

    Ok(SupervisedChild {
        service: plan.service.clone(),
        child,
    })
}

pub fn poll_child_status(child: &mut SupervisedChild) -> SupervisedServiceStatus {
    match child.child.try_wait() {
        Ok(Some(status)) => {
            let stderr = collect_stderr(child).unwrap_or_default();
            let stdout = collect_stdout(child).unwrap_or_default();
            let message = if !stderr.is_empty() {
                format!("process exited stderr={}", truncate_for_log(&stderr, 240))
            } else if !stdout.is_empty() {
                format!("process exited stdout={}", truncate_for_log(&stdout, 240))
            } else {
                "process exited".to_string()
            };
            SupervisedServiceStatus {
                service: child.service.clone(),
                started: true,
                exited: true,
                exit_code: status.code(),
                message,
            }
        }
        Ok(None) => SupervisedServiceStatus {
            service: child.service.clone(),
            started: true,
            exited: false,
            exit_code: None,
            message: "process running".to_string(),
        },
        Err(error) => SupervisedServiceStatus {
            service: child.service.clone(),
            started: true,
            exited: false,
            exit_code: None,
            message: error.to_string(),
        },
    }
}

fn collect_stderr(child: &mut SupervisedChild) -> Result<String, std::io::Error> {
    let Some(stderr) = child.child.stderr.as_mut() else {
        return Ok(String::new());
    };

    let mut buf = String::new();
    stderr.read_to_string(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn collect_stdout(child: &mut SupervisedChild) -> Result<String, std::io::Error> {
    let Some(stdout) = child.child.stdout.as_mut() else {
        return Ok(String::new());
    };

    let mut buf = String::new();
    stdout.read_to_string(&mut buf)?;
    Ok(buf.trim().to_string())
}

fn truncate_for_log(input: &str, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input.to_string();
    }

    let truncated: String = input.chars().take(max_chars).collect();
    format!("{truncated}...")
}

pub fn supervise_once(bin_dir: &Path, config: &LocalRuntimeConfig) -> Vec<SupervisedServiceStatus> {
    let plans = service_start_plan(bin_dir, config);
    let mut children = Vec::new();
    let mut statuses = Vec::new();

    for plan in plans {
        match spawn_service(&plan) {
            Ok(child) => children.push(child),
            Err(error) => statuses.push(SupervisedServiceStatus {
                service: plan.service,
                started: false,
                exited: false,
                exit_code: None,
                message: error,
            }),
        }
    }

    std::thread::sleep(std::time::Duration::from_millis(100));

    for mut child in children {
        let status = poll_child_status(&mut child);
        let _ = child.stop();
        statuses.push(status);
    }

    statuses
}

pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "****".to_string();
    }

    format!("{}...{}", &token[..6], &token[token.len() - 4..])
}

pub fn normalize_relative_screen_path(
    project_root: &Path,
    absolute_path: &Path,
) -> Result<String, String> {
    if !project_root.is_absolute() {
        return Err("project_root must be absolute".to_string());
    }
    if !absolute_path.is_absolute() {
        return Err("absolute_path must be absolute".to_string());
    }

    let relative = absolute_path
        .strip_prefix(project_root)
        .map_err(|_| "selected path is outside project root".to_string())?;
    let normalized = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => part.to_string_lossy().into_owned(),
            _ => String::new(),
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");

    if !is_valid_screen_relative_path(&normalized) {
        return Err(
            "relative_path must match config/screens/*.screen.json and disallow traversal"
                .to_string(),
        );
    }

    Ok(normalized)
}

pub fn is_valid_screen_relative_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.contains("..") || trimmed.contains("//") || trimmed.contains('\\') {
        return false;
    }

    let Some(without_prefix) = trimmed.strip_prefix("config/screens/") else {
        return false;
    };
    let Some(body) = without_prefix.strip_suffix(".screen.json") else {
        return false;
    };
    if body.is_empty() || body.starts_with('/') || body.ends_with('/') {
        return false;
    }

    body.split('/').all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    })
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

    #[test]
    fn service_plan_sets_bind_host_and_token_env() {
        let config = LocalRuntimeConfig {
            bind_host: "127.0.0.1".to_string(),
            startup_token: "secret-token".to_string(),
            services: vec![service()],
        };

        let plan = service_start_plan(Path::new("/tmp/scada"), &config);

        assert_eq!(1, plan.len());
        assert!(plan[0].args.is_empty());
        assert!(plan[0].restart_on_exit);
        assert_eq!(None, plan[0].restart_max_attempts);
        assert_eq!(None, plan[0].restart_backoff_ms);
        assert_eq!(None, plan[0].restart_reset_after_ms);
        assert_eq!(
            Some("127.0.0.1".to_string()),
            plan[0]
                .envs
                .iter()
                .find(|(key, _)| key == LOCAL_BIND_HOST_ENV)
                .map(|(_, value)| value.clone())
        );
        assert_eq!(
            Some("secret-token".to_string()),
            plan[0]
                .envs
                .iter()
                .find(|(key, _)| key == LOCAL_TOKEN_ENV)
                .map(|(_, value)| value.clone())
        );
    }

    #[test]
    fn token_mask_hides_middle() {
        assert_eq!("local-...abcd", mask_token("local-secret-abcd"));
    }

    #[test]
    fn stop_accepts_short_lived_process() {
        let child = Command::new("/bin/echo")
            .arg("ok")
            .spawn()
            .expect("spawn echo");
        let mut child = SupervisedChild {
            service: "echo".to_string(),
            child,
        };

        child.stop().expect("stop process");
    }

    #[test]
    fn screen_relative_path_validator_accepts_valid_path() {
        assert!(is_valid_screen_relative_path(
            "config/screens/mock-main.screen.json"
        ));
        assert!(is_valid_screen_relative_path(
            "config/screens/custom/mock-main.screen.json"
        ));
    }

    #[test]
    fn screen_relative_path_validator_rejects_invalid_path() {
        assert!(!is_valid_screen_relative_path(""));
        assert!(!is_valid_screen_relative_path("config/screens/.screen.json"));
        assert!(!is_valid_screen_relative_path(
            "config/screens/mock.main.screen.json"
        ));
        assert!(!is_valid_screen_relative_path(
            "config/screens/../mock-main.screen.json"
        ));
        assert!(!is_valid_screen_relative_path(
            "config/screens/mock-main.json"
        ));
    }

    #[test]
    fn normalize_relative_screen_path_returns_project_relative_path() {
        let project_root = Path::new("/tmp/scada-project");
        let absolute = Path::new("/tmp/scada-project/config/screens/mock-main.screen.json");

        let path = normalize_relative_screen_path(project_root, absolute).expect("normalize path");

        assert_eq!("config/screens/mock-main.screen.json", path);
    }

    #[test]
    fn normalize_relative_screen_path_rejects_outside_project_root() {
        let project_root = Path::new("/tmp/scada-project");
        let absolute = Path::new("/tmp/other/config/screens/mock-main.screen.json");

        let error = normalize_relative_screen_path(project_root, absolute).expect_err("must reject");

        assert!(error.contains("outside project root"));
    }
}
