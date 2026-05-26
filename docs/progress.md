# 開発進行状況

## 現在のフェーズ

フェーズ0: Mock通信基盤

## 完了済み

- 初期設計ドキュメント作成
- 開発計画書作成
- エージェント向け指示ファイル作成
- Rust workspace作成
- 契約ファイル雛形作成
  - gRPC `.proto`
  - Runtime OpenAPI
  - JSON Schema
- サービス雛形作成
  - `builder-api`
  - `preview-runtime`
  - `tag-server`
  - `driver-manager`
  - `mock-driver`
  - `tauri-shell`
- `scada-core` の基礎型実装
  - Tag Value
  - Quality Code
  - ControlCommand状態
  - MQTT topic生成
  - Driver write request/response
- Mock値流れの純粋ロジック
  - Mock Driver値生成
  - Driver Manager正規化
  - Tag Server最新値キャッシュ
- Mock書き込み流れの純粋ロジック
  - Tag Server WritePolicy
  - Mock Driver書き込み応答
  - Driver Manager応答反映
- Tauri Shellによるサービス一覧、ヘルスチェック、サービス起動計画表示
- Tauri Shellによるローカルサービスsupervisor最小実装
  - 子プロセス起動
  - 起動状態の一回観測
  - 停止処理
- Xcodeライセンス承諾後の `cargo test` 成功

## 現在作業中

- フェーズ0の次工程準備

## 次に行うこと

1. `tag-server` の実HTTP/REST最小APIを追加する。
2. Runtime/Builderから利用するローカルAPI境界を固める。
3. MQTT over WebSocket導入前に、タグ最新値取得と書き込み要求のREST縦断確認を行う。

## 最新検証

- `cargo fmt`: 成功
- `cargo test`: 成功
- `cargo build`: 成功
- `target/debug/tauri-shell --check-services --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --print-service-plan --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --supervise-once --bin-dir target/debug`: 成功

## セーブポイント

- `aeb188d docs: add initial SCADA design docs`
- `27e20d5 feat: scaffold phase 0 service contracts`
- `e888e41 feat: add service catalog and mqtt topics`
- `80b2f80 docs: record successful phase 0 test run`
- `7134383 feat: add mock value flow primitives`
- `5a49c3b test: cover mock value flow`
- `8d0d004 feat: add local service health checks`
- `0ad4c08 feat: add mock write flow primitives`
- `81928f5 feat: add local runtime startup plan`
- 今回の変更: `feat: add local supervisor progress tracking`
