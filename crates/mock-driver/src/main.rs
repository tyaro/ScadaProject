use mock_driver::MockDriver;
use scada_core::service::{print_health, ServiceRole};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::MockDriver);
        return;
    }

    let mut driver = MockDriver::new("mock-driver", "mock-endpoint");
    let values = driver.next_values();

    for value in values {
        println!(
            "mock-driver sample tag_id={} data_type={} quality={}",
            value.tag_id,
            value.value.data_type().as_str(),
            value.quality.as_str()
        );
    }
}
