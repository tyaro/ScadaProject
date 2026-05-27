#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_shell::{normalize_relative_screen_path, PickScreenRelativePathResult};

#[tauri::command]
fn pick_screen_relative_path(
    app: tauri::AppHandle,
    initial_path: Option<String>,
) -> Result<PickScreenRelativePathResult, String> {
    let project_root = resolve_project_root(&app)?;

    let mut picker = app
        .dialog()
        .file()
        .add_filter("SCADA Screen JSON", &["json"])
        .set_title("Select screen definition");

    if let Some(initial) = initial_path {
        let initial_absolute = project_root.join(initial);
        if let Some(directory) = initial_absolute.parent() {
            picker = picker.set_directory(directory);
        }
        if let Some(file_name) = initial_absolute.file_name().and_then(|name| name.to_str()) {
            picker = picker.set_file_name(file_name);
        }
    }

    let Some(picked_file) = picker.blocking_pick_file() else {
        return Ok(PickScreenRelativePathResult::cancelled());
    };

    let absolute_path = dialog_file_path_to_path_buf(picked_file)
        .ok_or_else(|| "selected path must be local filesystem path".to_string())?;
    let relative_path = normalize_relative_screen_path(&project_root, &absolute_path)?;

    Ok(PickScreenRelativePathResult::selected(relative_path))
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![pick_screen_relative_path])
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
}
