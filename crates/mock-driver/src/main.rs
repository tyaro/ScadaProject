use scada_core::service::{print_health, ServiceRole};
use scada_core::tag::{QualityCode, TagValue, TagValueData};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::MockDriver);
        return;
    }

    let sample = TagValue {
        tag_id: "mock.temperature.001".to_string(),
        value: TagValueData::Float(25.0),
        quality: QualityCode::Simulated,
        source_timestamp: "1970-01-01T00:00:00Z".to_string(),
        server_timestamp: "1970-01-01T00:00:00Z".to_string(),
        sequence: 1,
        scan_interval_ms: 1000,
        stale_after_ms: 3000,
        driver_id: "mock-driver".to_string(),
        endpoint_id: "mock-endpoint".to_string(),
        read_status: "ok".to_string(),
        write_status: "idle".to_string(),
    };

    println!(
        "mock-driver sample tag_id={} data_type={} quality={} sequence={}",
        sample.tag_id,
        sample.data_type().as_str(),
        sample.quality.as_str(),
        sample.sequence
    );
}
