# コンテキストリセット用ハンドオフ

このファイルは、会話コンテキストをリセットした後に作業を再開するための最短メモである。

## 最初に読むもの

1. `AGENTS.md`
2. `docs/progress.md`
3. `docs/development_plan.md`
4. `docs/phase0_implementation_notes.md`
5. 必要に応じて `docs/scada_basic_design.md`

## 現在位置

- フェーズ: フェーズ0 Mock通信基盤
- 現在の直近コミット: `24b817a chore: use rumqtt rustls websocket transport`
- 作業ツリー前提: リセット前時点ではクリーン
- 次の主作業: Preview Runtimeの `REST snapshot + MQTT delta` 常時購読統合

## 確定済み方針

- SCADAはサーバークライアント構成。
- クライアントはサーバーのREST APIとMQTT over WebSocketを通して監視操作する。
- 書き込みは、リアルタイム性が不要なものはREST APIを優先する。
- MockでもRuntime直結経路は作らない。
- 値の基本経路は `Driver -> Driver Manager -> Tag Server -> Runtime -> Client`。
- 制御操作の基本経路は `Client -> Runtime REST API -> Tag Server -> Driver Manager -> Driver`。
- SVGにタグ情報やmodify情報を直接埋め込まない。画面定義JSON/DBを正本にする。
- SurrealDBは内部DB、設計データ、短期状態、短期キャッシュに使う。長期保存は別ストレージへ逃がす。
- 通信ドライバはDriver Manager配下の子プロセスとして管理する。
- Tag Serverなどのサーバー側コンポーネントは、最終的にOSサービス、デーモン、またはコンテナサービスとして常駐可能にする。
- MQTT実装は `rumqttc` を使う。
- `rumqttc` は default Rustls + `websocket` feature 構成。`aws-lc-sys` を含む構成で `cargo test` 成功済み。

## 実装済みの主なもの

- Rust workspaceと各crate雛形
  - `scada-core`
  - `builder-api`
  - `preview-runtime`
  - `tag-server`
  - `driver-manager`
  - `mock-driver`
  - `tauri-shell`
- 契約雛形
  - `contracts/proto/scada.proto`
  - `contracts/openapi/runtime.yaml`
  - `contracts/schemas/*.schema.json`
- `scada-core`
  - Tag Value、Quality Code、ControlCommand状態、MQTT topic生成、サービス定義
- `mock-driver`
  - Mock値生成、Mock書き込み応答
- `driver-manager`
  - Raw Driver Value正規化、sequence採番、書き込み応答反映
- `tag-server`
  - 最新値インメモリキャッシュ
  - stale sequence破棄
  - 書き込み可能タグ検証
  - 最小REST API: `GET /health`, `POST /api/v1/tags/snapshot`, `POST /api/v1/control-commands`
  - 起動時トークン認証
- `preview-runtime`
  - Tag Server REST snapshotクライアント
  - 画面定義JSON読込
  - `ScreenDefinition + TagSnapshot -> ScreenProjection`
  - MQTT delta payload/topic検証
  - sequenceによるdelta適用/stale破棄
  - rumqtt単発受信/送信CLI
- `tauri-shell`
  - サービス一覧、ヘルスチェック、起動計画表示、最小supervisor

## 重要なコマンド

```bash
cargo fmt
cargo test
cargo build
target/debug/tauri-shell --check-services --bin-dir target/debug
target/debug/tauri-shell --print-service-plan --bin-dir target/debug
target/debug/tauri-shell --supervise-once --bin-dir target/debug
target/debug/tag-server --serve --addr 127.0.0.1:18080
target/debug/preview-runtime --snapshot-screen --screen config/screens/mock-main.screen.json --tag-server http://127.0.0.1:18080
target/debug/preview-runtime --snapshot-screen --screen config/screens/mock-main.screen.json --tag-server http://127.0.0.1:18080 --simulate-delta
```

## 次に実装すること

1. `preview-runtime --snapshot-screen` でREST snapshot取得後、rumqtt購読ループに入るコマンドを追加する。
2. 受信したdeltaを `apply_delta_to_projection` に流し、投影サマリーを更新出力する。
3. MQTT切断/再接続時の方針を実装する。
   - 再接続後はREST snapshotを再取得する。
   - その後、delta購読へ復帰する。
   - 古いdeltaはsequenceで破棄する。
4. Broker実体をどう起動するかを決める。
   - 開発初期は外部プロセスまたは軽量ローカルBrokerでよい。
   - 将来はTag Server/Driver Manager構成にBroker/WebSocket Gatewayを組み込む。

## 注意点

- ローカルポートbindやcurlでの縦断確認は、環境によってサンドボックス外権限が必要になる。
- Node.jsは24 LTS方針。既存CodexプロセスのPATHがNode 18を指す場合があるため、必要ならnvm経由で確認する。
- Rustはlatest stable方針。過去にRust 1.66で増分コンパイルキャッシュICEがあったが、Rust 1.95.0では再発していない。
- `target/debug` にスペース付きの重複バイナリ名が見える場合があるが、現時点の作業対象ではない。
- 機能方針やデータフローを変える場合は、必ず設計書と開発計画も同じ変更で更新する。
