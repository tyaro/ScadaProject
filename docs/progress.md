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
- Tag Serverの最小REST API実装
  - `GET /health`
  - `POST /api/v1/tags/snapshot`
  - `POST /api/v1/control-commands`
  - 起動時トークンによるローカルAPI認証
- Xcodeライセンス承諾後の `cargo test` 成功

## 現在作業中

- REST APIを使ったRuntime/Builder境界の準備

## 次に行うこと

1. `preview-runtime` からTag Server snapshot APIを呼び出す最小Runtime境界を追加する。
2. 画面ランタイム用の初期タグ値取得フローをRESTで縦断確認する。
3. その後、MQTT over WebSocketによるdelta購読へ進む。

## 最新検証

- `cargo fmt`: 成功
- `CARGO_INCREMENTAL=0 cargo test`: 成功
- `CARGO_INCREMENTAL=0 cargo build`: 成功
- `target/debug/tauri-shell --check-services --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --print-service-plan --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --supervise-once --bin-dir target/debug`: 成功
- `target/debug/tag-server --serve --addr 127.0.0.1:18080`: 起動成功
- `curl http://127.0.0.1:18080/health`: 成功
- `curl POST /api/v1/tags/snapshot`: 成功
- `curl POST /api/v1/control-commands`: 成功

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
- `2012b5f feat: add local supervisor progress tracking`
- 今回の変更: `feat: add tag server rest api`

## 注意メモ

- この環境のRust 1.66では増分コンパイルキャッシュでICEが発生したため、今回の検証は `CARGO_INCREMENTAL=0` 付きで実施した。
- ローカルポートbindとcurl確認はサンドボックス外権限で実施した。
