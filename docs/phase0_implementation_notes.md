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
- `cargo test`: macOSのXcodeライセンス未承諾によりリンク段階で失敗

`cargo test` の失敗はコード起因ではなく、ローカル環境で `sudo xcodebuild -license` の承諾が必要な状態のため発生している。Xcodeライセンス承諾後に再実行する。
