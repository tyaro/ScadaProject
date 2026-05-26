use std::path::PathBuf;

use preview_runtime::{
    default_snapshot_tags, format_snapshot_summary, load_screen_definition,
    resolve_tag_ids_from_screen, TagServerClient,
};
use scada_core::service::{print_health, ServiceRole, LOCAL_TOKEN_ENV};

const TAG_SERVER_URL_ENV: &str = "SCADA_TAG_SERVER_URL";
const DEFAULT_SCREEN_PATH: &str = "config/screens/mock-main.screen.json";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::PreviewRuntime);
        return;
    }
    if args.iter().any(|arg| arg == "--snapshot") {
        let tag_server_url = arg_value(&args, "--tag-server")
            .or_else(|| std::env::var(TAG_SERVER_URL_ENV).ok())
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string());
        let project_id = arg_value(&args, "--project-id").unwrap_or_else(|| "demo".to_string());
        let tags = {
            let values = arg_values(&args, "--tag");
            if values.is_empty() {
                default_snapshot_tags()
            } else {
                values
            }
        };
        let token = arg_value(&args, "--token").or_else(|| std::env::var(LOCAL_TOKEN_ENV).ok());
        let client = match TagServerClient::from_base_url(&tag_server_url) {
            Ok(client) => client.with_token(token),
            Err(error) => {
                eprintln!("preview-runtime snapshot failed: {error}");
                std::process::exit(1);
            }
        };

        match client.fetch_snapshot(&project_id, &tags) {
            Ok(snapshot) => println!("{}", format_snapshot_summary(&snapshot)),
            Err(error) => {
                eprintln!("preview-runtime snapshot failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.iter().any(|arg| arg == "--snapshot-screen") {
        let tag_server_url = arg_value(&args, "--tag-server")
            .or_else(|| std::env::var(TAG_SERVER_URL_ENV).ok())
            .unwrap_or_else(|| "http://127.0.0.1:18080".to_string());
        let screen_path = arg_value(&args, "--screen")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_SCREEN_PATH));
        let definition = match load_screen_definition(&screen_path) {
            Ok(definition) => definition,
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        };
        let tags = resolve_tag_ids_from_screen(&definition);
        if tags.is_empty() {
            eprintln!(
                "preview-runtime snapshot-screen failed: no tags resolved from {}",
                screen_path.display()
            );
            std::process::exit(1);
        }
        let token = arg_value(&args, "--token").or_else(|| std::env::var(LOCAL_TOKEN_ENV).ok());
        let client = match TagServerClient::from_base_url(&tag_server_url) {
            Ok(client) => client.with_token(token),
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        };

        match client.fetch_snapshot(&definition.project_id, &tags) {
            Ok(snapshot) => {
                println!(
                    "screen={} objects={} tags={}",
                    definition.screen_id,
                    definition.objects.len(),
                    tags.len()
                );
                println!("{}", format_snapshot_summary(&snapshot));
            }
            Err(error) => {
                eprintln!("preview-runtime snapshot-screen failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }

    println!("preview-runtime skeleton");
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn arg_values(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}
