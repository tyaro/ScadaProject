# フェーズ0 実装メモ

## 現在の実装範囲

フェーズ0のセーブポイントとして、Rustワークスペース、契約ファイル、Mock通信基盤、最小REST snapshot経路を追加する。

## 追加するもの

- Rust workspace
- `scada-core`
- `builder-api`
- `preview-runtime`
- `tag-server`
- `driver-manager`
- `mock-driver`
- `tauri-shell`
- gRPC `.proto` 雛形
- Runtime OpenAPI雛形
- JSON Schema雛形
- Mock Driver Manifest

## 追加済みのコアロジック

- `mock-driver`
  - 周期値生成の土台
  - Mockタグ書き込み応答
- `driver-manager`
  - Raw Driver ValueからTag Valueへの正規化
  - sequence採番
  - Driver書き込み応答からControlCommand状態への反映
- `tag-server`
  - 最新値インメモリキャッシュ
  - 古いsequenceの破棄
  - 書き込み可能タグの最小検証
  - 最小REST API
    - `GET /health`
    - `POST /api/v1/tags/snapshot`
    - `POST /api/v1/control-commands`
  - 起動時トークンによるローカルAPI認証
- `scada-core`
  - Raw Driver Value
  - Driver Write Request/Response
  - Tag Value
  - Quality Code
  - ControlCommand状態
  - MQTT topic生成
- `preview-runtime`
  - Tag Server snapshot API向け最小HTTPクライアント
  - `serde` / `serde_json` によるsnapshot DTO
  - `--snapshot` による初期タグ値取得確認
- `tauri-shell`
  - ローカルサービス一覧表示
  - ローカルサービスの `--health` 実行
  - `--bin-dir` 指定によるサービスバイナリ探索
  - 起動時ローカルトークン生成
  - `SCADA_LOCAL_TOKEN` と `SCADA_BIND_HOST` を含むサービス起動計画表示
  - `--supervise-once` による子プロセス起動、状態観測、停止

## まだ実装しないもの

- 実gRPCサーバー
- 本番向けRESTフレームワーク化
- SurrealDB接続
- MQTT Broker接続
- Tauri v2アプリ本体

RESTサーバーは、依存追加前の最小実装として `tag-server --serve` まで追加済み。
今後はRuntime/Builder境界、SurrealDB接続、MQTT Broker接続、Tauri v2アプリ本体を順に実装する。

## 検証状況

- `cargo fmt`: 成功
- `cargo check`: 成功
- `cargo test`: 成功
- `cargo build`: 成功
- `target/debug/tauri-shell --check-services --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --print-service-plan --bin-dir target/debug`: 成功
- `target/debug/tauri-shell --supervise-once --bin-dir target/debug`: 成功
- `target/debug/tag-server --serve --addr 127.0.0.1:18080`: 起動成功
- `curl http://127.0.0.1:18080/health`: 成功
- `curl POST /api/v1/tags/snapshot`: 成功
- `curl POST /api/v1/control-commands`: 成功
- `target/debug/preview-runtime --snapshot --tag-server http://127.0.0.1:18080 --project-id demo`: 成功

## 縦断テスト

- `Mock Driver -> Driver Manager -> Tag Server` の値流れ: 成功
- `Tag Server WritePolicy -> Mock Driver -> Driver Manager response handling` の書き込み流れ: 成功
- `Tag Server REST snapshot -> Preview Runtime` の初期値取得: 成功

Xcodeライセンス承諾後、ワークスペース全体のテストが成功した。
Rust 1.66の増分コンパイルキャッシュでICEが発生していたが、Rust 1.95.0更新後は通常の `cargo test` が成功している。
