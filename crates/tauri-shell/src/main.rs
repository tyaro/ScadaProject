use std::path::PathBuf;

use scada_core::service::{default_local_services, print_health, ServiceRole};
use tauri_shell::{check_default_services, default_service_bin_dir};

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

    println!("tauri-shell skeleton");
}

fn bin_dir_arg(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--bin-dir")
        .map(|pair| PathBuf::from(&pair[1]))
}
