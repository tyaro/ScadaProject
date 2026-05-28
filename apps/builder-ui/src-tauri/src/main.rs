#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::{fs, io};

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_shell::{
    is_valid_screen_relative_path, normalize_relative_screen_path, PickScreenRelativePathResult,
};

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
struct SuperviseLogSummary {
    cycle_summaries: u64,
    final_summaries: u64,
    parse_errors: u64,
    services: Vec<SuperviseLogSummaryService>,
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
struct SuperviseLogSummaryService {
    service: String,
    lines: u64,
    exited: u64,
    started_false: u64,
}

#[tauri::command]
async fn pick_screen_relative_path(
    app: tauri::AppHandle,
    initial_path: Option<String>,
) -> Result<PickScreenRelativePathResult, String> {
    let project_root = resolve_project_root(&app)?;

    let mut picker = app
        .dialog()
        .file()
        .add_filter("SCADA Screen JSON", &["json"])
        .set_title("Select screen definition");

    if let Some(initial_absolute) =
        initial_picker_absolute_path(&project_root, initial_path.as_deref())
    {
        if let Some(directory) = initial_absolute.parent() {
            picker = picker.set_directory(directory);
        }
        if let Some(file_name) = initial_absolute.file_name().and_then(|name| name.to_str()) {
            picker = picker.set_file_name(file_name);
        }
    }

    let (pick_tx, pick_rx) = mpsc::channel::<Option<FilePath>>();
    picker.pick_file(move |picked_file| {
        let _ = pick_tx.send(picked_file);
    });

    let picked_file = tauri::async_runtime::spawn_blocking(move || pick_rx.recv())
        .await
        .map_err(|error| format!("wait picker result: {error}"))?
        .map_err(|error| format!("receive picker result: {error}"))?;

    let Some(picked_file) = picked_file else {
        return Ok(PickScreenRelativePathResult::cancelled());
    };

    let absolute_path = dialog_file_path_to_path_buf(picked_file)
        .ok_or_else(|| "selected path must be local filesystem path".to_string())?;
    let relative_path = normalize_relative_screen_path(&project_root, &absolute_path)?;

    Ok(PickScreenRelativePathResult::selected(relative_path))
}

#[tauri::command]
fn read_supervise_log_summary(log_dir: String) -> Result<SuperviseLogSummary, String> {
    summarize_supervise_log_dir(Path::new(&log_dir))
}

fn summarize_supervise_log_dir(dir: &Path) -> Result<SuperviseLogSummary, String> {
    let mut cycle_summaries = 0u64;
    let mut final_summaries = 0u64;
    let mut parse_errors = 0u64;

    let summary_path = dir.join("supervise-loop.jsonl");
    if summary_path.exists() {
        let raw = fs::read_to_string(&summary_path)
            .map_err(|error| format!("read {}: {error}", summary_path.display()))?;
        for line in raw.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(value) => match value.get("type").and_then(|v| v.as_str()) {
                    Some("cycle_summary") => cycle_summaries = cycle_summaries.saturating_add(1),
                    Some("final_summary") => final_summaries = final_summaries.saturating_add(1),
                    _ => {}
                },
                Err(_) => parse_errors = parse_errors.saturating_add(1),
            }
        }
    }

    let mut services = Vec::new();
    let entries =
        fs::read_dir(dir).map_err(|error| format!("read_dir {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error: io::Error| format!("read_dir entry {}: {error}", dir.display()))?;
        let path = entry.path();

        if path.file_name().and_then(|name| name.to_str()) == Some("supervise-loop.jsonl") {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let service = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid service log file name: {}", path.display()))?
            .to_string();

        services.push(SuperviseLogSummaryService {
            service,
            lines: raw.lines().filter(|line| !line.trim().is_empty()).count() as u64,
            exited: raw.matches("exited=true").count() as u64,
            started_false: raw.matches("started=false").count() as u64,
        });
    }

    services.sort_by(|a, b| a.service.cmp(&b.service));

    Ok(SuperviseLogSummary {
        cycle_summaries,
        final_summaries,
        parse_errors,
        services,
    })
}

fn resolve_project_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    if let Some(explicit_root) = resolve_project_root_from_env()? {
        return Ok(explicit_root);
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(inferred) = infer_project_root_from(&cwd) {
            return Ok(inferred);
        }
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("resolve resource_dir: {error}"))?;
    if let Some(inferred) = infer_project_root_from(&resource_dir) {
        return Ok(inferred);
    }

    Err("failed to infer project root (set SCADA_PROJECT_ROOT)".to_string())
}

fn resolve_project_root_from_env() -> Result<Option<PathBuf>, String> {
    let Ok(explicit_root) = std::env::var("SCADA_PROJECT_ROOT") else {
        return Ok(None);
    };
    let root = PathBuf::from(explicit_root);
    if !root.is_absolute() {
        return Err("SCADA_PROJECT_ROOT must be absolute".to_string());
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("canonicalize SCADA_PROJECT_ROOT: {error}"))?;
    Ok(Some(canonical))
}

fn infer_project_root_from(start: &Path) -> Option<PathBuf> {
    for candidate in start.ancestors() {
        let has_workspace = candidate.join("Cargo.toml").is_file();
        let has_contracts = candidate.join("contracts").is_dir();
        let has_docs = candidate.join("docs").is_dir();
        if has_workspace && has_contracts && has_docs {
            if let Ok(canonical) = candidate.canonicalize() {
                return Some(canonical);
            }
        }
    }
    None
}

fn dialog_file_path_to_path_buf(path: FilePath) -> Option<PathBuf> {
    match path {
        FilePath::Path(path) => Some(path),
        FilePath::Url(_) => None,
    }
}

fn initial_picker_absolute_path(
    project_root: &Path,
    initial_path: Option<&str>,
) -> Option<PathBuf> {
    let raw = initial_path?.trim();
    if !is_valid_screen_relative_path(raw) {
        return None;
    }
    Some(project_root.join(raw))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pick_screen_relative_path,
            read_supervise_log_summary
        ])
        .run(tauri::generate_context!())
        .expect("failed to run builder-ui tauri shell");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn converts_local_dialog_file_path() {
        let path = dialog_file_path_to_path_buf(FilePath::Path(
            std::path::Path::new("/tmp/file.json").to_path_buf(),
        ));
        assert_eq!(
            Some(std::path::Path::new("/tmp/file.json").to_path_buf()),
            path
        );
    }

    #[test]
    fn rejects_non_local_dialog_file_path() {
        let url = url::Url::parse("https://example.com/file.json").expect("url");
        let path = dialog_file_path_to_path_buf(FilePath::Url(url));
        assert_eq!(None, path);
    }

    #[test]
    fn infer_project_root_finds_workspace_markers() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("scada-root-{suffix}"));
        let nested = root.join("apps/builder-ui/src-tauri");

        fs::create_dir_all(root.join("contracts")).expect("create contracts");
        fs::create_dir_all(root.join("docs")).expect("create docs");
        fs::create_dir_all(&nested).expect("create nested");
        fs::write(root.join("Cargo.toml"), "[workspace]\n").expect("write cargo toml");

        let inferred = infer_project_root_from(&nested).expect("infer project root");
        assert_eq!(root.canonicalize().expect("canonical root"), inferred);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn infer_project_root_returns_none_when_markers_missing() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("scada-root-missing-{suffix}"));
        fs::create_dir_all(root.join("nested")).expect("create nested");

        let inferred = infer_project_root_from(&root.join("nested"));
        assert_eq!(None, inferred);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initial_picker_absolute_path_accepts_valid_relative_path() {
        let root = Path::new("/tmp/scada-project");
        let resolved =
            initial_picker_absolute_path(root, Some("config/screens/mock-main.screen.json"));

        assert_eq!(
            Some(
                Path::new("/tmp/scada-project/config/screens/mock-main.screen.json").to_path_buf()
            ),
            resolved
        );
    }

    #[test]
    fn initial_picker_absolute_path_rejects_invalid_relative_path() {
        let root = Path::new("/tmp/scada-project");

        assert_eq!(
            None,
            initial_picker_absolute_path(root, Some("../outside.screen.json"))
        );
        assert_eq!(
            None,
            initial_picker_absolute_path(root, Some("/tmp/absolute.screen.json"))
        );
        assert_eq!(
            None,
            initial_picker_absolute_path(root, Some("config/other/mock-main.screen.json"))
        );
    }

    #[test]
    fn summarize_supervise_log_dir_collects_summary_and_service_stats() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("scada-supervise-summary-{suffix}"));
        fs::create_dir_all(&root).expect("create log dir");
        fs::write(
            root.join("supervise-loop.jsonl"),
            "{\"type\":\"cycle_summary\"}\nnot-json\n{\"type\":\"final_summary\"}\n",
        )
        .expect("write summary");
        fs::write(
            root.join("tag-server.log"),
            "cycle=1 started=true exited=false\ncycle=2 started=false exited=true\n",
        )
        .expect("write service log");

        let summary = summarize_supervise_log_dir(&root).expect("summarize");
        let _ = fs::remove_dir_all(&root);

        assert_eq!(1, summary.cycle_summaries);
        assert_eq!(1, summary.final_summaries);
        assert_eq!(1, summary.parse_errors);
        assert_eq!(1, summary.services.len());
        assert_eq!("tag-server", summary.services[0].service);
        assert_eq!(2, summary.services[0].lines);
        assert_eq!(1, summary.services[0].exited);
        assert_eq!(1, summary.services[0].started_false);
    }
}
