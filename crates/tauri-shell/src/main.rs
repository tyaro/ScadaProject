use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use scada_core::mqtt::MqttBrokerEndpoint;
use scada_core::service::{default_local_services, print_health, ServiceRole};
use serde::Deserialize;
use serde_json::json;
use tauri_shell::{
    check_default_services, create_local_runtime_config, default_service_bin_dir, mask_token,
    normalize_relative_screen_path, service_start_plan, spawn_service, supervise_once,
    PickScreenRelativePathResult, ServiceStartPlan,
};

const SERVICE_CONFIG_SCHEMA_VERSION: &str = "1.0.0";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return;
    }
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::TauriShell);
        return;
    }
    if args.iter().any(|arg| arg == "--list-services") {
        for service in default_local_services() {
            println!(
                "{} {} {}",
                service.role.as_str(),
                service.binary,
                service.health_arg
            );
        }
        return;
    }
    if args.iter().any(|arg| arg == "--check-services") {
        let bin_dir = match bin_dir_arg(&args) {
            Some(path) => path,
            None => std::env::current_exe()
                .map(|path| default_service_bin_dir(&path))
                .unwrap_or_else(|_| PathBuf::from(".")),
        };
        let results = check_default_services(&bin_dir);
        let mut all_healthy = true;

        for result in results {
            println!(
                "{} {} {}",
                result.service,
                if result.healthy {
                    "healthy"
                } else {
                    "unhealthy"
                },
                result.output
            );
            all_healthy = all_healthy && result.healthy;
        }

        if !all_healthy {
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--print-startup-token") {
        let config = create_local_runtime_config("127.0.0.1");
        println!("{}", config.startup_token);
        return;
    }
    if args.iter().any(|arg| arg == "--pick-screen-relative-path") {
        let result = match pick_screen_relative_path_response(&args) {
            Ok(Some(result)) => result,
            Ok(None) => {
                eprintln!("tauri-shell pick-screen-relative-path failed: command not enabled");
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!("tauri-shell pick-screen-relative-path failed: {error}");
                std::process::exit(1);
            }
        };

        let json = serde_json::to_string(&result).expect("serialize pick path result");
        println!("{json}");
        return;
    }
    if args.iter().any(|arg| arg == "--print-service-plan") {
        let bin_dir = match bin_dir_arg(&args) {
            Some(path) => path,
            None => std::env::current_exe()
                .map(|path| default_service_bin_dir(&path))
                .unwrap_or_else(|_| PathBuf::from(".")),
        };
        let bind_host = bind_host_arg(&args).unwrap_or_else(|| "127.0.0.1".to_string());
        let config = create_local_runtime_config(&bind_host);

        let plans = match build_plans(&args, &bin_dir, &config) {
            Ok(plans) => plans,
            Err(error) => {
                eprintln!("tauri-shell print-service-plan failed: {error}");
                std::process::exit(1);
            }
        };
        for item in plans {
            let token = item
                .envs
                .iter()
                .find(|(key, _)| key == "SCADA_LOCAL_TOKEN")
                .map(|(_, value)| mask_token(value))
                .unwrap_or_else(|| "-".to_string());
            let bind_host = item
                .envs
                .iter()
                .find(|(key, _)| key == "SCADA_BIND_HOST")
                .map(|(_, value)| value.clone())
                .unwrap_or_else(|| "-".to_string());
            let args_summary = if item.args.is_empty() {
                "-".to_string()
            } else {
                item.args.join(" ")
            };
            println!(
                "{} {} args=\"{}\" SCADA_BIND_HOST={} SCADA_LOCAL_TOKEN={}",
                item.service,
                item.binary_path.display(),
                args_summary,
                bind_host,
                token
            );
        }
        return;
    }
    if args.iter().any(|arg| arg == "--supervise-once") {
        let bin_dir = match bin_dir_arg(&args) {
            Some(path) => path,
            None => std::env::current_exe()
                .map(|path| default_service_bin_dir(&path))
                .unwrap_or_else(|_| PathBuf::from(".")),
        };
        let bind_host = bind_host_arg(&args).unwrap_or_else(|| "127.0.0.1".to_string());
        let config = create_local_runtime_config(&bind_host);
        let statuses = if args.iter().any(|arg| arg == "--include-rumqttd")
            || arg_value(&args, "--service-config").is_some()
        {
            match supervise_once_with_optional_rumqttd(&args, &bin_dir, &config) {
                Ok(statuses) => statuses,
                Err(error) => {
                    eprintln!("tauri-shell supervise-once failed: {error}");
                    std::process::exit(1);
                }
            }
        } else {
            supervise_once(&bin_dir, &config)
        };
        let mut ok = true;

        for status in statuses {
            println!(
                "{} started={} exited={} exit_code={:?} {}",
                status.service, status.started, status.exited, status.exit_code, status.message
            );
            ok = ok && status.started;
        }

        if !ok {
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--supervise-loop") {
        let bin_dir = match bin_dir_arg(&args) {
            Some(path) => path,
            None => std::env::current_exe()
                .map(|path| default_service_bin_dir(&path))
                .unwrap_or_else(|_| PathBuf::from(".")),
        };
        let bind_host = bind_host_arg(&args).unwrap_or_else(|| "127.0.0.1".to_string());
        let config = create_local_runtime_config(&bind_host);
        let interval_ms = arg_value(&args, "--supervise-interval-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1000);
        let cycles = arg_value(&args, "--supervise-cycles")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        if let Err(error) = supervise_loop_with_optional_rumqttd(
            &args,
            &bin_dir,
            &config,
            Duration::from_millis(interval_ms),
            cycles,
        ) {
            eprintln!("tauri-shell supervise-loop failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--supervise-log-summary") {
        let supervise_log_dir = match arg_value(&args, "--supervise-log-dir") {
            Some(path) => PathBuf::from(path),
            None => {
                eprintln!(
                    "tauri-shell supervise-log-summary failed: --supervise-log-dir is required"
                );
                std::process::exit(1);
            }
        };
        let summary = match summarize_supervise_log_dir(&supervise_log_dir) {
            Ok(summary) => summary,
            Err(error) => {
                eprintln!("tauri-shell supervise-log-summary failed: {error}");
                std::process::exit(1);
            }
        };
        println!(
            "summary dir={} cycle_summaries={} final_summaries={} parse_errors={}",
            supervise_log_dir.display(),
            summary.cycle_summaries,
            summary.final_summaries,
            summary.parse_errors
        );
        for service in summary.services {
            println!(
                "service={} lines={} exited={} started_false={}",
                service.service, service.lines, service.exited, service.started_false
            );
        }
        return;
    }

    println!("tauri-shell skeleton");
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> String {
    format!(
        r#"tauri-shell options

core:
  --help, -h
  --health
  --list-services
  --check-services [--bin-dir <path>]
  --print-startup-token
    --pick-screen-relative-path --project-root <absolute-path> [--absolute-path <absolute-path>] [--cancel]
  --print-service-plan [--bin-dir <path>] [--bind-host <host>] [--service-config <path>] [--include-rumqttd] [--rumqttd-config <path>] [--rumqttd-bin <path>]
  --supervise-once [--bin-dir <path>] [--bind-host <host>] [--service-config <path>] [--include-rumqttd] [--rumqttd-config <path>] [--rumqttd-bin <path>]

supervise-loop:
  --supervise-loop
  --supervise-interval-ms <ms>        default: 1000
  --supervise-cycles <n>              default: 0 (infinite)
  --restart-exited
  --restart-max-attempts <n>          default: 0 (unlimited)
  --restart-backoff-ms <ms>           default: 0
  --restart-reset-after-ms <ms>       default: 0 (disabled)
  --supervise-verbose                 per-service detailed logs
  --supervise-summary-json            emit cycle_summary/final_summary as JSON lines
    --supervise-log-dir <path>          persist cycle summaries and service statuses to files
  --supervise-fail-on-start-error     exit non-zero if any service start fails
  --supervise-fail-on-exhausted-restart
                                      exit non-zero if restart attempts are exhausted
    --supervise-log-summary --supervise-log-dir <path>
                                                                            summarize persisted supervise logs

service-config:
  --service-config <path>             schema_version must be {}
  root mqtt_url                       auto-injected into preview-runtime as --mqtt-url
"#,
        SERVICE_CONFIG_SCHEMA_VERSION
    )
}

fn pick_screen_relative_path_response(
    args: &[String],
) -> Result<Option<PickScreenRelativePathResult>, String> {
    if !args.iter().any(|arg| arg == "--pick-screen-relative-path") {
        return Ok(None);
    }

    if args.iter().any(|arg| arg == "--cancel") {
        return Ok(Some(PickScreenRelativePathResult::cancelled()));
    }

    let project_root = arg_value(args, "--project-root")
        .ok_or_else(|| "--project-root is required".to_string())?;
    let absolute_path = arg_value(args, "--absolute-path")
        .ok_or_else(|| "--absolute-path is required unless --cancel is set".to_string())?;

    let normalized =
        normalize_relative_screen_path(Path::new(&project_root), Path::new(&absolute_path))?;
    Ok(Some(PickScreenRelativePathResult::selected(normalized)))
}

fn bin_dir_arg(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--bin-dir")
        .map(|pair| PathBuf::from(&pair[1]))
}

fn bind_host_arg(args: &[String]) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == "--bind-host")
        .map(|pair| pair[1].clone())
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn build_plans(
    args: &[String],
    bin_dir: &std::path::Path,
    config: &scada_core::service::LocalRuntimeConfig,
) -> Result<Vec<ServiceStartPlan>, String> {
    let mut plans = service_start_plan(bin_dir, config);
    if let Some(config_path) = arg_value(args, "--service-config") {
        let file_config = load_service_plan_file(&config_path)?;
        apply_service_overrides(&mut plans, &file_config.services);
        apply_shared_mqtt_url(&mut plans, file_config.mqtt_url.as_deref());
    }

    if args.iter().any(|arg| arg == "--include-rumqttd") {
        let rumqttd_bin = arg_value(args, "--rumqttd-bin").unwrap_or_else(|| "rumqttd".to_string());
        let rumqttd_config = arg_value(args, "--rumqttd-config")
            .unwrap_or_else(|| "config/rumqttd.toml".to_string());
        plans.push(ServiceStartPlan {
            service: "rumqttd".to_string(),
            binary_path: PathBuf::from(rumqttd_bin),
            args: vec!["-c".to_string(), rumqttd_config, "-q".to_string()],
            envs: Vec::new(),
            restart_on_exit: true,
            restart_max_attempts: None,
            restart_backoff_ms: None,
            restart_reset_after_ms: None,
        });
    }
    Ok(plans)
}

fn apply_shared_mqtt_url(plans: &mut [ServiceStartPlan], mqtt_url: Option<&str>) {
    let Some(mqtt_url) = mqtt_url else {
        return;
    };
    if mqtt_url.is_empty() {
        return;
    }

    for plan in plans {
        if plan.service != "preview-runtime" {
            continue;
        }
        if plan.args.iter().any(|arg| arg == "--mqtt-url") {
            continue;
        }
        plan.args.push("--mqtt-url".to_string());
        plan.args.push(mqtt_url.to_string());
    }
}

fn supervise_once_with_optional_rumqttd(
    args: &[String],
    bin_dir: &std::path::Path,
    config: &scada_core::service::LocalRuntimeConfig,
) -> Result<Vec<tauri_shell::SupervisedServiceStatus>, String> {
    let plans = build_plans(args, bin_dir, config)?;
    let mut children = Vec::new();
    let mut statuses = Vec::new();

    for plan in plans {
        match spawn_service(&plan) {
            Ok(child) => children.push(child),
            Err(error) => statuses.push(tauri_shell::SupervisedServiceStatus {
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
        let status = tauri_shell::poll_child_status(&mut child);
        let _ = child.stop();
        statuses.push(status);
    }

    Ok(statuses)
}

fn supervise_loop_with_optional_rumqttd(
    args: &[String],
    bin_dir: &std::path::Path,
    config: &scada_core::service::LocalRuntimeConfig,
    interval: Duration,
    cycles: u64,
) -> Result<(), String> {
    let plans = build_plans(args, bin_dir, config)?;
    let supervise_log_dir = arg_value(args, "--supervise-log-dir").map(PathBuf::from);
    if let Some(dir) = &supervise_log_dir {
        fs::create_dir_all(dir)
            .map_err(|error| format!("create supervise log dir {}: {error}", dir.display()))?;
    }
    let restart_exited = args.iter().any(|arg| arg == "--restart-exited");
    let supervise_verbose = args.iter().any(|arg| arg == "--supervise-verbose");
    let supervise_summary_json = args.iter().any(|arg| arg == "--supervise-summary-json");
    let fail_on_start_error = args
        .iter()
        .any(|arg| arg == "--supervise-fail-on-start-error");
    let fail_on_exhausted_restart = args
        .iter()
        .any(|arg| arg == "--supervise-fail-on-exhausted-restart");
    let default_restart_max_attempts = arg_value(args, "--restart-max-attempts")
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    let default_restart_backoff_ms = arg_value(args, "--restart-backoff-ms")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let default_restart_reset_after_ms = arg_value(args, "--restart-reset-after-ms")
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    let mut children: Vec<(ServiceStartPlan, tauri_shell::SupervisedChild)> = Vec::new();
    let mut restart_attempts: HashMap<String, u32> = HashMap::new();
    let mut running_since: HashMap<String, Instant> = HashMap::new();
    let mut saw_start_error = false;
    let mut saw_restart_exhausted = false;
    let mut startup_error_count = 0u64;
    let mut total_running_count = 0u64;
    let mut total_exited_count = 0u64;
    let mut total_restarted_count = 0u64;
    let mut total_restart_exhausted_count = 0u64;
    let mut total_restart_failed_count = 0u64;
    let mut total_restart_reset_count = 0u64;

    for plan in &plans {
        match spawn_service(plan) {
            Ok(child) => {
                running_since.insert(plan.service.clone(), Instant::now());
                children.push((plan.clone(), child));
            }
            Err(error) => {
                if supervise_verbose {
                    println!(
                        "{} started=false exited=false exit_code=None {}",
                        plan.service, error
                    );
                }
                startup_error_count = startup_error_count.saturating_add(1);
                if fail_on_start_error {
                    saw_start_error = true;
                }
            }
        }
    }

    let mut cycle = 0u64;
    loop {
        cycle = cycle.saturating_add(1);
        let mut running_count = 0u64;
        let mut exited_count = 0u64;
        let mut restarted_count = 0u64;
        let mut restart_exhausted_count = 0u64;
        let mut restart_failed_count = 0u64;
        let mut restart_reset_count = 0u64;
        for (plan, child) in &mut children {
            let status = tauri_shell::poll_child_status(child);
            if let Some(dir) = &supervise_log_dir {
                append_service_status_log(dir, cycle, &status)?;
            }
            if supervise_verbose {
                println!(
                    "cycle={} {} started={} exited={} exit_code={:?} {}",
                    cycle,
                    status.service,
                    status.started,
                    status.exited,
                    status.exit_code,
                    status.message
                );
            }
            if status.exited {
                exited_count = exited_count.saturating_add(1);
            } else {
                running_count = running_count.saturating_add(1);
            }

            let restart_max_attempts = plan
                .restart_max_attempts
                .unwrap_or(default_restart_max_attempts);
            let restart_backoff_ms = plan
                .restart_backoff_ms
                .unwrap_or(default_restart_backoff_ms);
            let restart_reset_after_ms = plan
                .restart_reset_after_ms
                .unwrap_or(default_restart_reset_after_ms);

            if restart_reset_after_ms > 0 && !status.exited {
                let attempts = restart_attempts.entry(status.service.clone()).or_insert(0);
                if *attempts > 0 {
                    let started_at = running_since
                        .entry(status.service.clone())
                        .or_insert_with(Instant::now);
                    if started_at.elapsed() >= Duration::from_millis(restart_reset_after_ms) {
                        *attempts = 0;
                        if supervise_verbose {
                            println!(
                                "cycle={} {} restart-reset=true message=restart attempts cleared after stable run",
                                cycle, status.service
                            );
                        }
                        restart_reset_count = restart_reset_count.saturating_add(1);
                    }
                } else {
                    running_since
                        .entry(status.service.clone())
                        .or_insert_with(Instant::now);
                }
            }

            if restart_exited && status.exited && plan.restart_on_exit {
                let attempts = restart_attempts.entry(status.service.clone()).or_insert(0);
                if restart_max_attempts > 0 && *attempts >= restart_max_attempts {
                    if supervise_verbose {
                        println!(
                            "cycle={} {} restart=false message=restart attempts exhausted ({})",
                            cycle, status.service, restart_max_attempts
                        );
                    }
                    restart_exhausted_count = restart_exhausted_count.saturating_add(1);
                    if fail_on_exhausted_restart {
                        saw_restart_exhausted = true;
                    }
                    continue;
                }
                if restart_backoff_ms > 0 {
                    std::thread::sleep(Duration::from_millis(restart_backoff_ms));
                }
                match spawn_service(plan) {
                    Ok(new_child) => {
                        *child = new_child;
                        *attempts = attempts.saturating_add(1);
                        running_since.insert(status.service.clone(), Instant::now());
                        if supervise_verbose {
                            println!(
                                "cycle={} {} restart=true attempts={} message=process respawned",
                                cycle, status.service, *attempts
                            );
                        }
                        restarted_count = restarted_count.saturating_add(1);
                    }
                    Err(error) => {
                        *attempts = attempts.saturating_add(1);
                        if supervise_verbose {
                            println!(
                                "cycle={} {} restart=false attempts={} message={}",
                                cycle, status.service, *attempts, error
                            );
                        }
                        restart_failed_count = restart_failed_count.saturating_add(1);
                    }
                }
            }
        }
        if !supervise_verbose {
            let events = format_cycle_events(
                startup_error_count,
                restarted_count,
                restart_exhausted_count,
                restart_failed_count,
                restart_reset_count,
            );
            let cycle_summary_json = json!({
                "type": "cycle_summary",
                "cycle": cycle,
                "running": running_count,
                "exited": exited_count,
                "restarted": restarted_count,
                "restart_exhausted": restart_exhausted_count,
                "restart_failed": restart_failed_count,
                "restart_reset": restart_reset_count,
                "startup_errors": startup_error_count,
                "events": events,
            });
            if let Some(dir) = &supervise_log_dir {
                append_summary_json_log(dir, &cycle_summary_json)?;
            }
            if supervise_summary_json {
                println!("{}", cycle_summary_json);
            } else {
                println!(
                    "cycle={} summary running={} exited={} restarted={} restart_exhausted={} restart_failed={} restart_reset={} startup_errors={} events={}",
                    cycle,
                    running_count,
                    exited_count,
                    restarted_count,
                    restart_exhausted_count,
                    restart_failed_count,
                    restart_reset_count,
                    startup_error_count,
                    events
                );
            }
        }
        total_running_count = total_running_count.saturating_add(running_count);
        total_exited_count = total_exited_count.saturating_add(exited_count);
        total_restarted_count = total_restarted_count.saturating_add(restarted_count);
        total_restart_exhausted_count =
            total_restart_exhausted_count.saturating_add(restart_exhausted_count);
        total_restart_failed_count =
            total_restart_failed_count.saturating_add(restart_failed_count);
        total_restart_reset_count = total_restart_reset_count.saturating_add(restart_reset_count);

        if cycles > 0 && cycle >= cycles {
            break;
        }
        std::thread::sleep(interval);
    }

    for (_, child) in &mut children {
        let _ = child.stop();
    }

    let final_events = format_cycle_events(
        startup_error_count,
        total_restarted_count,
        total_restart_exhausted_count,
        total_restart_failed_count,
        total_restart_reset_count,
    );
    let final_summary_json = json!({
        "type": "final_summary",
        "cycles": cycle,
        "running_total": total_running_count,
        "exited_total": total_exited_count,
        "restarted_total": total_restarted_count,
        "restart_exhausted_total": total_restart_exhausted_count,
        "restart_failed_total": total_restart_failed_count,
        "restart_reset_total": total_restart_reset_count,
        "startup_errors": startup_error_count,
        "fail_on_start_error": saw_start_error,
        "fail_on_exhausted_restart": saw_restart_exhausted,
        "events": final_events,
    });
    if let Some(dir) = &supervise_log_dir {
        append_summary_json_log(dir, &final_summary_json)?;
    }
    if supervise_summary_json {
        println!("{}", final_summary_json);
    } else {
        println!(
            "final_summary cycles={} running_total={} exited_total={} restarted_total={} restart_exhausted_total={} restart_failed_total={} restart_reset_total={} startup_errors={} fail_on_start_error={} fail_on_exhausted_restart={} events={}",
            cycle,
            total_running_count,
            total_exited_count,
            total_restarted_count,
            total_restart_exhausted_count,
            total_restart_failed_count,
            total_restart_reset_count,
            startup_error_count,
            saw_start_error,
            saw_restart_exhausted,
            final_events
        );
    }

    if fail_on_start_error && saw_start_error {
        return Err("start error detected in supervise-loop".to_string());
    }
    if fail_on_exhausted_restart && saw_restart_exhausted {
        return Err("restart attempts exhausted in supervise-loop".to_string());
    }

    Ok(())
}

fn format_cycle_events(
    startup_error_count: u64,
    restarted_count: u64,
    restart_exhausted_count: u64,
    restart_failed_count: u64,
    restart_reset_count: u64,
) -> String {
    let mut events = Vec::new();
    if startup_error_count > 0 {
        events.push("START_ERROR");
    }
    if restarted_count > 0 {
        events.push("RESTARTED");
    }
    if restart_exhausted_count > 0 {
        events.push("RESTART_EXHAUSTED");
    }
    if restart_failed_count > 0 {
        events.push("RESTART_FAILED");
    }
    if restart_reset_count > 0 {
        events.push("RESTART_RESET");
    }
    if events.is_empty() {
        return "NONE".to_string();
    }
    events.join(",")
}

fn append_summary_json_log(dir: &Path, value: &serde_json::Value) -> Result<(), String> {
    let path = dir.join("supervise-loop.jsonl");
    append_line(&path, &value.to_string())
}

fn append_service_status_log(
    dir: &Path,
    cycle: u64,
    status: &tauri_shell::SupervisedServiceStatus,
) -> Result<(), String> {
    let path = dir.join(format!("{}.log", status.service));
    let line = format!(
        "cycle={} started={} exited={} exit_code={:?} {}",
        cycle, status.started, status.exited, status.exit_code, status.message
    );
    append_line(&path, &line)
}

fn append_line(path: &Path, line: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    writeln!(file, "{line}").map_err(|error| format!("write {}: {error}", path.display()))
}

#[derive(Debug, PartialEq, Eq)]
struct SuperviseLogSummary {
    cycle_summaries: u64,
    final_summaries: u64,
    parse_errors: u64,
    services: Vec<ServiceLogSummary>,
}

#[derive(Debug, PartialEq, Eq)]
struct ServiceLogSummary {
    service: String,
    lines: u64,
    exited: u64,
    started_false: u64,
}

fn summarize_supervise_log_dir(dir: &Path) -> Result<SuperviseLogSummary, String> {
    let mut cycle_summaries = 0u64;
    let mut final_summaries = 0u64;
    let mut parse_errors = 0u64;
    let summary_path = dir.join("supervise-loop.jsonl");
    if summary_path.exists() {
        let raw = fs::read_to_string(&summary_path)
            .map_err(|error| format!("read {}: {error}", summary_path.display()))?;
        for line in raw.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(value) => match value.get("type").and_then(|v| v.as_str()) {
                    Some("cycle_summary") => {
                        cycle_summaries = cycle_summaries.saturating_add(1);
                    }
                    Some("final_summary") => {
                        final_summaries = final_summaries.saturating_add(1);
                    }
                    _ => {}
                },
                Err(_) => {
                    parse_errors = parse_errors.saturating_add(1);
                }
            }
        }
    }

    let mut services = Vec::new();
    let entries = fs::read_dir(dir).map_err(|error| format!("read_dir {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read_dir entry {}: {error}", dir.display()))?;
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("supervise-loop.jsonl") {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let lines = raw.lines().filter(|line| !line.trim().is_empty()).count() as u64;
        let exited = raw.matches("exited=true").count() as u64;
        let started_false = raw.matches("started=false").count() as u64;
        let service = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid service log file name: {}", path.display()))?
            .to_string();
        services.push(ServiceLogSummary {
            service,
            lines,
            exited,
            started_false,
        });
    }
    services.sort_by(|a, b| a.service.cmp(&b.service));

    Ok(SuperviseLogSummary {
        cycle_summaries,
        final_summaries,
        parse_errors,
        services,
    })
}

#[derive(Debug, Deserialize)]
struct ServicePlanFile {
    schema_version: String,
    mqtt_url: Option<String>,
    #[serde(default)]
    services: Vec<ServiceEntry>,
}

#[derive(Debug, Deserialize)]
struct ServiceEntry {
    service: String,
    binary_path: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    envs: HashMap<String, String>,
    restart_on_exit: Option<bool>,
    restart_max_attempts: Option<u32>,
    restart_backoff_ms: Option<u64>,
    restart_reset_after_ms: Option<u64>,
}

fn load_service_plan_file(path: &str) -> Result<ServicePlanFile, String> {
    let raw = fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))?;
    let config: ServicePlanFile =
        serde_json::from_str(&raw).map_err(|error| format!("parse {path}: {error}"))?;
    if config.schema_version != SERVICE_CONFIG_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {} in {} (expected {})",
            config.schema_version, path, SERVICE_CONFIG_SCHEMA_VERSION
        ));
    }
    validate_service_plan_file(&config, path)?;

    Ok(config)
}

fn validate_service_plan_file(config: &ServicePlanFile, path: &str) -> Result<(), String> {
    if let Some(mqtt_url) = config.mqtt_url.as_deref() {
        MqttBrokerEndpoint::parse(mqtt_url)
            .map_err(|error| format!("invalid mqtt_url in {path}: {error}"))?;
    }
    Ok(())
}

fn apply_service_overrides(plans: &mut Vec<ServiceStartPlan>, entries: &[ServiceEntry]) {
    for entry in entries {
        if let Some(plan) = plans.iter_mut().find(|plan| plan.service == entry.service) {
            if let Some(binary_path) = &entry.binary_path {
                plan.binary_path = PathBuf::from(binary_path);
            }
            if !entry.args.is_empty() {
                plan.args = entry.args.clone();
            }
            if !entry.envs.is_empty() {
                for (key, value) in &entry.envs {
                    if let Some((_, existing_value)) = plan.envs.iter_mut().find(|(k, _)| k == key)
                    {
                        *existing_value = value.clone();
                    } else {
                        plan.envs.push((key.clone(), value.clone()));
                    }
                }
            }
            if let Some(restart_on_exit) = entry.restart_on_exit {
                plan.restart_on_exit = restart_on_exit;
            }
            if let Some(restart_max_attempts) = entry.restart_max_attempts {
                plan.restart_max_attempts = Some(restart_max_attempts);
            }
            if let Some(restart_backoff_ms) = entry.restart_backoff_ms {
                plan.restart_backoff_ms = Some(restart_backoff_ms);
            }
            if let Some(restart_reset_after_ms) = entry.restart_reset_after_ms {
                plan.restart_reset_after_ms = Some(restart_reset_after_ms);
            }
            continue;
        }

        let envs = entry
            .envs
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        plans.push(ServiceStartPlan {
            service: entry.service.clone(),
            binary_path: PathBuf::from(
                entry
                    .binary_path
                    .clone()
                    .unwrap_or_else(|| entry.service.clone()),
            ),
            args: entry.args.clone(),
            envs,
            restart_on_exit: entry.restart_on_exit.unwrap_or(true),
            restart_max_attempts: entry.restart_max_attempts,
            restart_backoff_ms: entry.restart_backoff_ms,
            restart_reset_after_ms: entry.restart_reset_after_ms,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_mqtt_url_is_injected_to_preview_runtime() {
        let mut plans = vec![
            ServiceStartPlan {
                service: "preview-runtime".to_string(),
                binary_path: PathBuf::from("preview-runtime"),
                args: Vec::new(),
                envs: Vec::new(),
                restart_on_exit: true,
                restart_max_attempts: None,
                restart_backoff_ms: None,
                restart_reset_after_ms: None,
            },
            ServiceStartPlan {
                service: "tag-server".to_string(),
                binary_path: PathBuf::from("tag-server"),
                args: Vec::new(),
                envs: Vec::new(),
                restart_on_exit: true,
                restart_max_attempts: None,
                restart_backoff_ms: None,
                restart_reset_after_ms: None,
            },
        ];

        apply_shared_mqtt_url(&mut plans, Some("ws://127.0.0.1:8083/mqtt"));

        assert_eq!(
            vec![
                "--mqtt-url".to_string(),
                "ws://127.0.0.1:8083/mqtt".to_string()
            ],
            plans[0].args
        );
        assert!(plans[1].args.is_empty());
    }

    #[test]
    fn shared_mqtt_url_does_not_override_explicit_arg() {
        let mut plans = vec![ServiceStartPlan {
            service: "preview-runtime".to_string(),
            binary_path: PathBuf::from("preview-runtime"),
            args: vec![
                "--mqtt-url".to_string(),
                "mqtt://broker.example.local:1883".to_string(),
            ],
            envs: Vec::new(),
            restart_on_exit: true,
            restart_max_attempts: None,
            restart_backoff_ms: None,
            restart_reset_after_ms: None,
        }];

        apply_shared_mqtt_url(&mut plans, Some("ws://127.0.0.1:8083/mqtt"));

        assert_eq!(
            vec![
                "--mqtt-url".to_string(),
                "mqtt://broker.example.local:1883".to_string(),
            ],
            plans[0].args
        );
    }

    #[test]
    fn cycle_event_codes_are_stable() {
        assert_eq!("NONE", format_cycle_events(0, 0, 0, 0, 0));
        assert_eq!(
            "START_ERROR,RESTARTED,RESTART_EXHAUSTED,RESTART_FAILED,RESTART_RESET",
            format_cycle_events(1, 1, 1, 1, 1)
        );
    }

    #[test]
    fn service_plan_validation_accepts_mqtt_url() {
        let config = ServicePlanFile {
            schema_version: SERVICE_CONFIG_SCHEMA_VERSION.to_string(),
            mqtt_url: Some("mqtt://127.0.0.1:1883".to_string()),
            services: Vec::new(),
        };

        assert!(validate_service_plan_file(&config, "test.json").is_ok());
    }

    #[test]
    fn service_plan_validation_rejects_invalid_mqtt_url() {
        let config = ServicePlanFile {
            schema_version: SERVICE_CONFIG_SCHEMA_VERSION.to_string(),
            mqtt_url: Some("mqtt://:bad".to_string()),
            services: Vec::new(),
        };

        let error = validate_service_plan_file(&config, "test.json").expect_err("validation error");
        assert!(error.contains("invalid mqtt_url"));
    }

    #[test]
    fn help_text_mentions_current_service_config_schema_version() {
        let help = help_text();
        assert!(help.contains(&format!(
            "schema_version must be {SERVICE_CONFIG_SCHEMA_VERSION}"
        )));
    }

    #[test]
    fn service_config_schema_file_matches_runtime_schema_version() {
        let schema_path =
            workspace_root().join("contracts/schemas/tauri-shell-service-config.schema.json");
        let schema_raw = fs::read_to_string(&schema_path).expect("read schema");
        let schema_json: serde_json::Value =
            serde_json::from_str(&schema_raw).expect("parse schema json");
        let schema_version = schema_json
            .get("properties")
            .and_then(|v| v.get("schema_version"))
            .and_then(|v| v.get("const"))
            .and_then(|v| v.as_str())
            .expect("schema_version.const");

        assert_eq!(SERVICE_CONFIG_SCHEMA_VERSION, schema_version);
    }

    #[test]
    fn bundled_service_configs_match_runtime_schema_version() {
        for relative in [
            "config/tauri-shell.services.json",
            "config/tauri-shell.services.mosquitto.json",
        ] {
            let config_path = workspace_root().join(relative);
            let config_raw = fs::read_to_string(&config_path).expect("read config");
            let config_json: serde_json::Value =
                serde_json::from_str(&config_raw).expect("parse config json");
            let schema_version = config_json
                .get("schema_version")
                .and_then(|v| v.as_str())
                .expect("schema_version");

            assert_eq!(
                SERVICE_CONFIG_SCHEMA_VERSION,
                schema_version,
                "schema_version mismatch in {}",
                config_path.display()
            );
        }
    }

    #[test]
    fn pick_screen_relative_path_returns_selected_value() {
        let args = vec![
            "tauri-shell".to_string(),
            "--pick-screen-relative-path".to_string(),
            "--project-root".to_string(),
            "/tmp/scada-project".to_string(),
            "--absolute-path".to_string(),
            "/tmp/scada-project/config/screens/mock-main.screen.json".to_string(),
        ];

        let result = pick_screen_relative_path_response(&args)
            .expect("parse args")
            .expect("must return result");

        assert!(!result.cancelled);
        assert_eq!(
            Some("config/screens/mock-main.screen.json".to_string()),
            result.relative_path
        );
    }

    #[test]
    fn pick_screen_relative_path_can_return_cancelled() {
        let args = vec![
            "tauri-shell".to_string(),
            "--pick-screen-relative-path".to_string(),
            "--project-root".to_string(),
            "/tmp/scada-project".to_string(),
            "--cancel".to_string(),
        ];

        let result = pick_screen_relative_path_response(&args)
            .expect("parse args")
            .expect("must return result");

        assert!(result.cancelled);
        assert_eq!(None, result.relative_path);
    }

    #[test]
    fn pick_screen_relative_path_requires_absolute_path_without_cancel() {
        let args = vec![
            "tauri-shell".to_string(),
            "--pick-screen-relative-path".to_string(),
            "--project-root".to_string(),
            "/tmp/scada-project".to_string(),
        ];

        let error = pick_screen_relative_path_response(&args).expect_err("must fail");

        assert!(error.contains("--absolute-path is required"));
    }

    #[test]
    fn summarize_supervise_log_dir_collects_json_and_service_stats() {
        let temp_root = std::env::temp_dir().join(format!(
            "tauri-shell-log-summary-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_millis()
        ));
        fs::create_dir_all(&temp_root).expect("create temp root");

        fs::write(
            temp_root.join("supervise-loop.jsonl"),
            "{\"type\":\"cycle_summary\"}\nnot-json\n{\"type\":\"final_summary\"}\n",
        )
        .expect("write summary log");
        fs::write(
            temp_root.join("tag-server.log"),
            "cycle=1 started=true exited=false\ncycle=2 started=false exited=true\n",
        )
        .expect("write tag-server log");
        fs::write(
            temp_root.join("driver-manager.log"),
            "cycle=1 started=true exited=false\n",
        )
        .expect("write driver-manager log");

        let summary = summarize_supervise_log_dir(&temp_root).expect("summarize log dir");

        let _ = fs::remove_dir_all(&temp_root);

        assert_eq!(1, summary.cycle_summaries);
        assert_eq!(1, summary.final_summaries);
        assert_eq!(1, summary.parse_errors);
        assert_eq!(2, summary.services.len());
        assert_eq!("driver-manager", summary.services[0].service);
        assert_eq!(1, summary.services[0].lines);
        assert_eq!(0, summary.services[0].exited);
        assert_eq!(0, summary.services[0].started_false);
        assert_eq!("tag-server", summary.services[1].service);
        assert_eq!(2, summary.services[1].lines);
        assert_eq!(1, summary.services[1].exited);
        assert_eq!(1, summary.services[1].started_false);
    }

    fn workspace_root() -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop();
        path.pop();
        path
    }
}
