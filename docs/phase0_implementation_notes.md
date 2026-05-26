# フェーズ0 実装メモ

## 現在の実装範囲

フェーズ0の最初のセーブポイントとして、外部依存なしでビルド可能なRustワークスペースと契約ファイルを追加する。

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
- `scada-core`
  - Raw Driver Value
  - Driver Write Request/Response
  - Tag Value
  - Quality Code
  - ControlCommand状態
  - MQTT topic生成
- `tauri-shell`
  - ローカルサービス一覧表示
  - ローカルサービスの `--health` 実行
  - `--bin-dir` 指定によるサービスバイナリ探索

## まだ実装しないもの

- 実gRPCサーバー
- 実RESTサーバー
- SurrealDB接続
- MQTT Broker接続
- Tauri v2アプリ本体

これらはフェーズ0の次のセーブポイントで、依存関係を追加しながら実装する。

## 検証状況

- `cargo fmt`: 成功
- `cargo check`: 成功
- `cargo test`: 成功
- `cargo build`: 成功
- `target/debug/tauri-shell --check-services --bin-dir target/debug`: 成功

## 縦断テスト

- `Mock Driver -> Driver Manager -> Tag Server` の値流れ: 成功
- `Tag Server WritePolicy -> Mock Driver -> Driver Manager response handling` の書き込み流れ: 成功

Xcodeライセンス承諾後、ワークスペース全体のテストが成功した。
