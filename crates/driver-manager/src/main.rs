use scada_core::service::{print_health, ServiceRole};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::DriverManager);
        return;
    }

    println!("driver-manager skeleton");
}
