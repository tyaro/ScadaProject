use scada_core::service::{print_health, ServiceRole, LOCAL_BIND_HOST_ENV, LOCAL_TOKEN_ENV};
use tag_server::{run_server, TagServerApi};

const TAG_SERVER_PORT_ENV: &str = "SCADA_TAG_SERVER_PORT";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--health") {
        print_health(ServiceRole::TagServer);
        return;
    }
    if args.iter().any(|arg| arg == "--serve") {
        let addr = serve_addr(&args);
        let required_token = std::env::var(LOCAL_TOKEN_ENV).ok();
        let mut api = TagServerApi::phase0_mock().with_required_token(required_token);

        eprintln!("tag-server listening on {addr}");
        if let Err(error) = run_server(&addr, &mut api) {
            eprintln!("tag-server failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    println!("tag-server skeleton");
}

fn serve_addr(args: &[String]) -> String {
    if let Some(addr) = arg_value(args, "--addr") {
        return addr;
    }

    let host = arg_value(args, "--bind-host")
        .or_else(|| std::env::var(LOCAL_BIND_HOST_ENV).ok())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = arg_value(args, "--port")
        .or_else(|| std::env::var(TAG_SERVER_PORT_ENV).ok())
        .unwrap_or_else(|| "18080".to_string());

    format!("{host}:{port}")
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}
