#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

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
    if let Ok(explicit_root) = std::env::var("SCADA_PROJECT_ROOT") {
        let root = PathBuf::from(explicit_root);
        if !root.is_absolute() {
            return Err("SCADA_PROJECT_ROOT must be absolute".to_string());
        }
        return Ok(root);
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("resolve resource_dir: {error}"))?;
    let project_root = resource_dir.join("..").join("..").join("..").join("..");
    let canonical = project_root
        .canonicalize()
        .map_err(|error| format!("canonicalize inferred project root: {error}"))?;

    if !canonical.is_absolute() {
        return Err("inferred project root is not absolute".to_string());
    }

    Ok(canonical)
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
}
