use std::path::PathBuf;

use scada_core::service::{default_local_services, print_health, ServiceRole};
use tauri_shell::{
    check_default_services, create_local_runtime_config, default_service_bin_dir, mask_token,
    service_start_plan, supervise_once,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
    if args.iter().any(|arg| arg == "--print-service-plan") {
        let bin_dir = match bin_dir_arg(&args) {
            Some(path) => path,
            None => std::env::current_exe()
                .map(|path| default_service_bin_dir(&path))
                .unwrap_or_else(|_| PathBuf::from(".")),
        };
        let bind_host = bind_host_arg(&args).unwrap_or_else(|| "127.0.0.1".to_string());
        let config = create_local_runtime_config(&bind_host);

        for item in service_start_plan(&bin_dir, &config) {
            println!(
                "{} {} {}={} {}={}",
                item.service,
                item.binary_path.display(),
                item.bind_host_env.0,
                item.bind_host_env.1,
                item.token_env.0,
                mask_token(&item.token_env.1)
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
        let statuses = supervise_once(&bin_dir, &config);
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

    println!("tauri-shell skeleton");
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
