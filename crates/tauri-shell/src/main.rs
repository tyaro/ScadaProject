use scada_core::service::{default_local_services, print_health, ServiceRole};

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

    println!("tauri-shell skeleton");
}
