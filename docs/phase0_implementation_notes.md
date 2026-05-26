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
  - 画面定義JSON読込
  - `tag_bindings` からタグ一覧を解決
  - `--snapshot-screen` による画面定義ベースの初期タグ値取得確認
  - 画面オブジェクト投影モデル
    - `ScreenDefinition + TagSnapshot -> ScreenProjection`
    - bindingごとの value/quality/sequence 解決
  - Runtime delta適用基盤
    - MQTT tag value topicとpayloadの整合確認
    - sequenceが新しいdeltaのみ投影へ適用
    - stale deltaの破棄
  - rumqttベースのMQTT入出力
    - `--mqtt-receive-once` による単発受信
    - `--mqtt-publish-delta` による単発配信
    - Rustls default + WebSocket feature構成で `aws-lc-sys` を使用
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
- MQTT常時購読ループの本統合
- Tauri v2アプリ本体

RESTサーバーは、依存追加前の最小実装として `tag-server --serve` まで追加済み。
今後はRuntime/Builder境界、SurrealDB接続、MQTT常時購読ループ統合、Tauri v2アプリ本体を順に実装する。

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
- `target/debug/preview-runtime --snapshot-screen --screen config/screens/mock-main.screen.json --tag-server http://127.0.0.1:18080`: 成功
- `--snapshot-screen` の投影サマリー出力: 成功
- `target/debug/preview-runtime --snapshot-screen --screen config/screens/mock-main.screen.json --tag-server http://127.0.0.1:18080 --simulate-delta`: 成功
- `cargo test` (rumqtt追加後): 成功
- `cargo test` (`rumqttc` default Rustls + `websocket`, `aws-lc-sys` あり): 成功

## 縦断テスト

- `Mock Driver -> Driver Manager -> Tag Server` の値流れ: 成功
- `Tag Server WritePolicy -> Mock Driver -> Driver Manager response handling` の書き込み流れ: 成功
- `Tag Server REST snapshot -> Preview Runtime` の初期値取得: 成功
- `Screen Definition -> Tag resolve -> Tag Server REST snapshot -> Preview Runtime` の初期値取得: 成功
- `Screen Definition + TagSnapshot -> ScreenProjection` のオブジェクト投影: 成功
- `MQTT tag value delta -> ScreenProjection` のsequence適用: 成功
- `rumqtt publish/receive (単発CLI)` の経路: 実装済み（Broker実接続の常時購読は次フェーズ）
- `rumqtt WebSocket transport` のコンパイル経路: 成功

Xcodeライセンス承諾後、ワークスペース全体のテストが成功した。
Rust 1.66の増分コンパイルキャッシュでICEが発生していたが、Rust 1.95.0更新後は通常の `cargo test` が成功している。
