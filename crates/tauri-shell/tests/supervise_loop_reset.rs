use std::fs;
use std::process::Command;

#[test]
fn supervise_loop_emits_restart_reset_after_stable_run() {
    let temp_root = std::env::temp_dir().join(format!(
        "tauri-shell-reset-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_root).expect("create temp root");

    let sentinel_file = temp_root.join("flap-once");
    let script_path = temp_root.join("flap-test.sh");
    let config_path = temp_root.join("service-config.json");

    let script = format!(
        "#!/bin/sh\nset -eu\nSENTINEL_FILE=\"{}\"\nif [ ! -f \"$SENTINEL_FILE\" ]; then\n  touch \"$SENTINEL_FILE\"\n  exit 1\nfi\nsleep 2\n",
        sentinel_file.display()
    );
    fs::write(&script_path, script).expect("write flap script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)
            .expect("script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set execute bit");
    }

    let config = format!(
        "{{\n  \"schema_version\": \"1.0.0\",\n  \"services\": [\n    {{\"service\": \"builder-api\", \"restart_on_exit\": false}},\n    {{\"service\": \"tag-server\", \"restart_on_exit\": false}},\n    {{\"service\": \"driver-manager\", \"restart_on_exit\": false}},\n    {{\"service\": \"preview-runtime\", \"restart_on_exit\": false}},\n    {{\"service\": \"mock-driver\", \"restart_on_exit\": false}},\n    {{\n      \"service\": \"flap-test\",\n      \"binary_path\": \"{}\",\n      \"restart_on_exit\": true,\n      \"restart_max_attempts\": 5,\n      \"restart_backoff_ms\": 10,\n      \"restart_reset_after_ms\": 200\n    }}\n  ]\n}}\n",
        script_path.display()
    );
    fs::write(&config_path, config).expect("write service config");

    let output = Command::new(env!("CARGO_BIN_EXE_tauri-shell"))
        .arg("--supervise-loop")
        .arg("--service-config")
        .arg(&config_path)
        .arg("--supervise-interval-ms")
        .arg("100")
        .arg("--supervise-cycles")
        .arg("20")
        .arg("--restart-exited")
        .output()
        .expect("run tauri-shell supervise-loop");

    let _ = fs::remove_dir_all(&temp_root);

    assert!(
        output.status.success(),
        "tauri-shell exited with non-zero status: {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("restarted=1"),
        "missing restarted summary in output:\n{}",
        stdout
    );
    assert!(
        stdout.contains("RESTARTED"),
        "missing restarted event code in output:\n{}",
        stdout
    );
    assert!(
        stdout.contains("restart_reset=1"),
        "missing restart_reset summary in output:\n{}",
        stdout
    );
    assert!(
        stdout.contains("RESTART_RESET"),
        "missing restart reset event code in output:\n{}",
        stdout
    );
    assert!(
        stdout.contains("final_summary"),
        "missing final summary in output:\n{}",
        stdout
    );
}

#[test]
#[ignore = "can be timing-dependent in full-workspace cargo test; run directly when needed"]
fn supervise_loop_fails_when_restart_is_exhausted_with_fail_flag() {
    let temp_root = std::env::temp_dir().join(format!(
        "tauri-shell-exhausted-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&temp_root).expect("create temp root");

    let script_path = temp_root.join("always-fail.sh");
    let config_path = temp_root.join("service-config.json");

    fs::write(&script_path, "#!/bin/sh\nexit 1\n").expect("write fail script");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path)
            .expect("script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("set execute bit");
    }

    let config = format!(
        "{{\n  \"schema_version\": \"1.0.0\",\n  \"services\": [\n    {{\"service\": \"builder-api\", \"restart_on_exit\": false}},\n    {{\"service\": \"tag-server\", \"restart_on_exit\": false}},\n    {{\"service\": \"driver-manager\", \"restart_on_exit\": false}},\n    {{\"service\": \"preview-runtime\", \"restart_on_exit\": false}},\n    {{\"service\": \"mock-driver\", \"restart_on_exit\": false}},\n    {{\n      \"service\": \"flap-test\",\n      \"binary_path\": \"{}\",\n      \"restart_on_exit\": true,\n      \"restart_max_attempts\": 1,\n      \"restart_backoff_ms\": 10\n    }}\n  ]\n}}\n",
        script_path.display()
    );
    fs::write(&config_path, config).expect("write service config");

    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let output = Command::new(&tauri_shell_bin)
        .arg("--supervise-loop")
        .arg("--service-config")
        .arg(&config_path)
        .arg("--supervise-interval-ms")
        .arg("100")
        .arg("--supervise-cycles")
        .arg("6")
        .arg("--restart-exited")
        .arg("--supervise-fail-on-exhausted-restart")
        .output()
        .expect("run tauri-shell supervise-loop");

    let _ = fs::remove_dir_all(&temp_root);

    assert!(
        !output.status.success(),
        "tauri-shell should fail when restart is exhausted\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("restart_exhausted=") && stdout.contains("RESTART_EXHAUSTED"),
        "missing restart_exhausted summary in stdout:\n{}",
        stdout
    );
    assert!(
        stdout.contains("RESTART_EXHAUSTED"),
        "missing restart exhausted event code in stdout:\n{}",
        stdout
    );
    assert!(
        stdout.contains("final_summary"),
        "missing final summary in output:\n{}",
        stdout
    );
    assert!(
        stderr.contains("restart attempts exhausted in supervise-loop"),
        "missing fail reason in stderr:\n{}",
        stderr
    );
}

#[test]
fn supervise_loop_can_emit_json_summaries() {
    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let bin_dir = tauri_shell_bin
        .parent()
        .expect("tauri-shell binary parent")
        .to_path_buf();

    let output = Command::new(&tauri_shell_bin)
        .arg("--supervise-loop")
        .arg("--bin-dir")
        .arg(&bin_dir)
        .arg("--supervise-interval-ms")
        .arg("100")
        .arg("--supervise-cycles")
        .arg("2")
        .arg("--supervise-summary-json")
        .output()
        .expect("run tauri-shell supervise-loop json");

    assert!(
        output.status.success(),
        "tauri-shell should succeed in json summary mode\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"type\":\"cycle_summary\""),
        "missing cycle_summary json output:\n{}",
        stdout
    );
    assert!(
        stdout.contains("\"type\":\"final_summary\""),
        "missing final_summary json output:\n{}",
        stdout
    );
}

#[test]
fn supervise_loop_writes_persistent_logs_when_log_dir_is_set() {
    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let bin_dir = tauri_shell_bin
        .parent()
        .expect("tauri-shell binary parent")
        .to_path_buf();
    let log_dir = std::env::temp_dir().join(format!(
        "tauri-shell-supervise-logs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));

    let output = Command::new(&tauri_shell_bin)
        .arg("--supervise-loop")
        .arg("--bin-dir")
        .arg(&bin_dir)
        .arg("--supervise-interval-ms")
        .arg("100")
        .arg("--supervise-cycles")
        .arg("1")
        .arg("--supervise-log-dir")
        .arg(&log_dir)
        .output()
        .expect("run tauri-shell supervise-loop with log dir");

    assert!(
        output.status.success(),
        "tauri-shell should succeed with supervise log dir\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let summary_path = log_dir.join("supervise-loop.jsonl");
    let summary = fs::read_to_string(&summary_path).expect("read supervise-loop.jsonl");
    assert!(
        summary.contains("\"type\":\"cycle_summary\"")
            && summary.contains("\"type\":\"final_summary\""),
        "summary log missing expected entries:\n{}",
        summary
    );

    let mut service_log_found = false;
    for entry in fs::read_dir(&log_dir).expect("read log dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("supervise-loop.jsonl") {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("log") {
            let content = fs::read_to_string(&path).expect("read service log");
            if content.contains("cycle=1") {
                service_log_found = true;
                break;
            }
        }
    }

    let _ = fs::remove_dir_all(&log_dir);

    assert!(
        service_log_found,
        "expected at least one per-service log file with cycle status in {}",
        log_dir.display()
    );
}

#[test]
fn supervise_log_summary_can_emit_json() {
    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let bin_dir = tauri_shell_bin
        .parent()
        .expect("tauri-shell binary parent")
        .to_path_buf();
    let log_dir = std::env::temp_dir().join(format!(
        "tauri-shell-supervise-summary-json-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));

    let supervise_output = Command::new(&tauri_shell_bin)
        .arg("--supervise-loop")
        .arg("--bin-dir")
        .arg(&bin_dir)
        .arg("--supervise-interval-ms")
        .arg("100")
        .arg("--supervise-cycles")
        .arg("1")
        .arg("--supervise-log-dir")
        .arg(&log_dir)
        .output()
        .expect("run tauri-shell supervise-loop with log dir");

    assert!(
        supervise_output.status.success(),
        "tauri-shell should succeed with supervise log dir\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&supervise_output.stdout),
        String::from_utf8_lossy(&supervise_output.stderr)
    );

    let summary_output = Command::new(&tauri_shell_bin)
        .arg("--supervise-log-summary")
        .arg("--supervise-log-dir")
        .arg(&log_dir)
        .arg("--supervise-log-summary-json")
        .output()
        .expect("run tauri-shell supervise-log-summary json");

    let _ = fs::remove_dir_all(&log_dir);

    assert!(
        summary_output.status.success(),
        "tauri-shell should succeed with supervise-log-summary-json\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&summary_output.stdout),
        String::from_utf8_lossy(&summary_output.stderr)
    );

    let stdout = String::from_utf8_lossy(&summary_output.stdout);
    let value: serde_json::Value = serde_json::from_str(stdout.trim()).expect("parse json output");
    assert!(
        value
            .get("cycle_summaries")
            .and_then(|v| v.as_u64())
            .is_some(),
        "missing cycle_summaries in json output:\n{}",
        stdout
    );
    assert!(
        value.get("services").and_then(|v| v.as_array()).is_some(),
        "missing services array in json output:\n{}",
        stdout
    );
}

#[test]
fn supervise_log_summary_fails_on_parse_error_with_flag() {
    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let log_dir = std::env::temp_dir().join(format!(
        "tauri-shell-supervise-summary-fail-parse-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&log_dir).expect("create temp log dir");
    fs::write(
        log_dir.join("supervise-loop.jsonl"),
        "{\"type\":\"cycle_summary\"}\nnot-json\n",
    )
    .expect("write jsonl");

    let output = Command::new(&tauri_shell_bin)
        .arg("--supervise-log-summary")
        .arg("--supervise-log-dir")
        .arg(&log_dir)
        .arg("--supervise-log-summary-fail-on-parse-error")
        .output()
        .expect("run supervise-log-summary fail-on-parse-error");

    let _ = fs::remove_dir_all(&log_dir);

    assert!(
        !output.status.success(),
        "expected non-zero exit when parse error exists\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("parse errors detected"),
        "missing parse error failure reason in stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn supervise_log_summary_fails_on_service_exit_with_flag() {
    let tauri_shell_bin = std::path::PathBuf::from(env!("CARGO_BIN_EXE_tauri-shell"));
    let log_dir = std::env::temp_dir().join(format!(
        "tauri-shell-supervise-summary-fail-service-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));
    fs::create_dir_all(&log_dir).expect("create temp log dir");
    fs::write(
        log_dir.join("tag-server.log"),
        "cycle=1 started=true exited=true\n",
    )
    .expect("write service log");

    let output = Command::new(&tauri_shell_bin)
        .arg("--supervise-log-summary")
        .arg("--supervise-log-dir")
        .arg(&log_dir)
        .arg("--supervise-log-summary-fail-on-service-exit")
        .output()
        .expect("run supervise-log-summary fail-on-service-exit");

    let _ = fs::remove_dir_all(&log_dir);

    assert!(
        !output.status.success(),
        "expected non-zero exit when service exit exists\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("service exits detected"),
        "missing service exit failure reason in stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
