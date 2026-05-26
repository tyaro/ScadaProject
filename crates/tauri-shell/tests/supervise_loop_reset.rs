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
            .as_millis()
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
        stdout.contains("events=RESTARTED") || stdout.contains("events=RESTARTED,"),
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
fn supervise_loop_fails_when_restart_is_exhausted_with_fail_flag() {
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
        .arg("4")
        .arg("--restart-exited")
        .arg("--restart-max-attempts")
        .arg("1")
        .arg("--supervise-fail-on-exhausted-restart")
        .output()
        .expect("run tauri-shell supervise-loop");

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
