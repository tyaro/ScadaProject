# 開発進行状況

## 現在のフェーズ

フェーズ0: Mock通信基盤

## 完了済み

- 初期設計ドキュメント作成
- 開発計画書作成
- エージェント向け指示ファイル作成
- 開発ツール方針更新
  - Rust latest stable
  - Node.js 24 LTS
  - ライブラリ、クレート、ツールは可能な限り最新安定版を使用
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

- `rustc --version --verbose`: `rustc 1.95.0`, host `aarch64-apple-darwin`
- `cargo --version --verbose`: `cargo 1.95.0`, host `aarch64-apple-darwin`
- `nvm use 24 && node --version`: `v24.16.0`
- `nvm use 24 && npm --version`: `11.13.0`
- `cargo fmt`: 成功
- `cargo test`: 成功
- `cargo build`: 成功
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
- `2f52046 feat: add tag server rest api`
- 今回の変更: `chore: update toolchains and version policy`

## 注意メモ

- Rust 1.66で発生していた増分コンパイルキャッシュICEは、Rust 1.95.0へ更新後の通常 `cargo test` で再発していない。
- rustup本体はx86_64エミュレーションだが、実際に使用される `rustc` と `cargo` は `aarch64-apple-darwin`。
- HomebrewはmacOS 26.5を未対応扱いして失敗するため、Node 24 LTSはnvmで導入した。
- Codexプロセスの既存PATHは起動時のNode 18を保持している場合がある。新規ログインシェルでは `.zprofile` 経由でnvm defaultのNode 24が有効になる。
- ローカルポートbindとcurl確認はサンドボックス外権限で実施した。
