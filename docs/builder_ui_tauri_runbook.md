# Builder UI src-tauri Validation Runbook

## 1. Purpose

This runbook defines repeatable manual checks for the Builder UI native picker command `pick_screen_relative_path`.

Use this when validating:
- `apps/builder-ui/src-tauri` startup behavior
- native file dialog integration
- `relative_path` normalization and validation (`config/screens/*.screen.json`)

## 2. Preconditions

- Run from repository root.
- Node.js 24 is active.
- Rust stable toolchain is installed.
- Dependencies are installed in `apps/builder-ui`.

## 3. Fast Pre-Checks

```bash
cd apps/builder-ui
npm run check
npm run tauri:check
cd ../..
cargo test --manifest-path apps/builder-ui/src-tauri/Cargo.toml
```

Expected:
- Svelte/TypeScript check passes.
- src-tauri compile check passes.
- src-tauri unit tests pass.

## 4. Launch Native Builder UI

```bash
cd apps/builder-ui
npm run tauri:dev:project
```

This command sets `SCADA_PROJECT_ROOT` explicitly and launches the native shell.

## 5. Manual Validation Scenarios

### 5.1 Successful Selection

1. In the app, click `Pick via Tauri`.
2. Select a file under `config/screens/` with `.screen.json` suffix.
   Example: `config/screens/mock-main.screen.json`

Expected:
- Save As input is updated to selected `relative_path`.
- I/O status shows `Selected config/screens/mock-main.screen.json`.

### 5.2 Cancel Selection

1. Click `Pick via Tauri`.
2. Close the dialog without selecting a file.

Expected:
- Existing Save As input value remains unchanged.
- I/O status shows `File selection cancelled`.

### 5.3 Invalid Path Rejection

1. Click `Pick via Tauri`.
2. Select a file that does not satisfy `config/screens/*.screen.json`.
   Examples:
   - `README.md`
   - `config/other/a.screen.json`

Expected:
- Save As input is not overwritten with invalid value.
- I/O status shows `Path selection failed: ...`.

## 6. Troubleshooting

- Symptom: startup fails with project root error.
  - Check `SCADA_PROJECT_ROOT` is an absolute path.
  - Prefer `npm run tauri:dev:project` over `npm run tauri:dev`.

- Symptom: picker works but save-as path is rejected.
  - Confirm selected file is under `config/screens/`.
  - Confirm filename suffix is `.screen.json`.

- Symptom: command compiles but runtime behavior differs.
  - Re-run `cargo test --manifest-path apps/builder-ui/src-tauri/Cargo.toml`.
  - Re-run `scripts/check_local_ci.sh` to verify contract checks.
