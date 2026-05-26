use std::time::Duration;

use driver_manager::{
    run_mock_driver_cycle, run_mock_driver_cycle_with_normalizer, DriverManagerCycleConfig,
    ValueNormalizer,
};
use scada_core::service::{print_health, ServiceRole};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::DriverManager);
        return;
    }
    if args.iter().any(|arg| arg == "--run-mock-cycle") {
        let config = mock_cycle_config(&args);

        match run_mock_driver_cycle(&config) {
            Ok(result) => {
                println!(
                    "driver-manager mock-cycle raw_values={} posted_values={} tag_server_status={} tag_server_body={}",
                    result.raw_values,
                    result.posted_values,
                    result.tag_server_status,
                    result.tag_server_body
                );
                if !(200..300).contains(&result.tag_server_status) {
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("driver-manager mock-cycle failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--run-mock-loop") {
        let config = mock_cycle_config(&args);
        let startup_delay_ms = arg_value(&args, "--startup-delay-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let cycles = arg_value(&args, "--cycles")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        let interval_ms = arg_value(&args, "--interval-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(config.scan_interval_ms);
        let mut normalizer = ValueNormalizer::new(config.scan_interval_ms, config.stale_after_ms);
        let mut cycle = 0u64;

        if startup_delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(startup_delay_ms));
        }

        loop {
            cycle = cycle.saturating_add(1);
            match run_mock_driver_cycle_with_normalizer(&config, &mut normalizer) {
                Ok(result) => {
                    println!(
                        "driver-manager mock-loop cycle={} raw_values={} posted_values={} tag_server_status={} tag_server_body={}",
                        cycle,
                        result.raw_values,
                        result.posted_values,
                        result.tag_server_status,
                        result.tag_server_body
                    );
                    if !(200..300).contains(&result.tag_server_status) {
                        std::process::exit(1);
                    }
                }
                Err(error) => {
                    eprintln!("driver-manager mock-loop failed: {error}");
                    std::process::exit(1);
                }
            }

            if cycles > 0 && cycle >= cycles {
                break;
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
        }
        return;
    }

    println!("driver-manager skeleton");
}

fn mock_cycle_config(args: &[String]) -> DriverManagerCycleConfig {
    DriverManagerCycleConfig {
        mock_driver_bin: arg_value(args, "--mock-driver-bin")
            .unwrap_or_else(|| "mock-driver".to_string()),
        tag_server_url: arg_value(args, "--tag-server")
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string()),
        project_id: arg_value(args, "--project-id").unwrap_or_else(|| "demo".to_string()),
        token: arg_value(args, "--token").or_else(|| std::env::var("SCADA_LOCAL_TOKEN").ok()),
        scan_interval_ms: arg_value(args, "--scan-interval-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1000),
        stale_after_ms: arg_value(args, "--stale-after-ms")
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(3000),
        server_timestamp: arg_value(args, "--server-timestamp")
            .unwrap_or_else(|| "1970-01-01T00:00:01Z".to_string()),
    }
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}
