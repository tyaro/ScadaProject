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
  - `--run-mock-cycle` によるMock Driver子プロセス起動
  - `--run-mock-loop` による周期Mock投入
  - `--startup-delay-ms` によるBroker/Tag Server起動待ち
  - `--write-addr` によるMock書き込み受付HTTP
  - `/api/v1/driver-writes` でMock Driver `write` を呼び出す最小境界
  - Mock Driver JSON Lines出力の読込
  - `POST /api/v1/driver-values` によるTag Serverへの正規化値投入
- `tag-server`
  - 最新値インメモリキャッシュ
  - 古いsequenceの破棄
  - 書き込み可能タグの最小検証
  - 最小REST API
    - `GET /health`
    - `POST /api/v1/tags/snapshot`
    - `POST /api/v1/driver-values`
    - `POST /api/v1/control-commands`
  - `--mqtt-url` 指定時のタグ値MQTT publish
  - `--driver-manager` 指定時のControlCommand書き込み中継
  - `POST /api/v1/driver-values` でcache更新されたTagValueのみpublish
  - 起動時トークンによるローカルAPI認証
- `scada-core`
  - Raw Driver Value
  - Raw Driver Value JSON Lines用のJSON変換
  - Driver Write Request/Response
  - Tag Value
  - Tag Value JSON変換
  - Quality Code
  - ControlCommand状態
  - MQTT topic生成
- `mock-driver`
  - `--emit-once` によるRaw Driver Value JSON Lines出力
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
    - `--mqtt-url` によるBroker接続URL指定（`mqtt://`, `tcp://`, `ws://`）
    - Rustls default + WebSocket feature構成で `aws-lc-sys` を使用
- `tauri-shell`
  - ローカルサービス一覧表示
  - ローカルサービスの `--health` 実行
  - `--bin-dir` 指定によるサービスバイナリ探索
  - 起動時ローカルトークン生成
  - `SCADA_LOCAL_TOKEN` と `SCADA_BIND_HOST` を含むサービス起動計画表示
  - `--supervise-once` による子プロセス起動、状態観測、停止
  - `--include-rumqttd` によるローカルBroker統合
  - `--service-config <path>` によるサービス別 `binary_path` / `args` / `envs` 上書き
  - `service-config` の `schema_version` 検証（`1.0.0` のみ許可）
  - 早期終了プロセスの stderr/stdout 診断表示
  - `config/tauri-shell.services*.json` によるBroker、Tag Server、Driver Manager周期投入の同時起動設定

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
- `target/debug/tauri-shell --print-service-plan --bin-dir target/debug --service-config config/tauri-shell.services.json`: 成功
- `target/debug/tauri-shell --supervise-once --bin-dir target/debug --service-config config/tauri-shell.services.json`: 成功

## `tauri-shell` CLI仕様メモ（service-config/supervisor）

- `--service-config <path>`
  - JSONファイルからサービス別起動設定を読み込む。
  - 既存サービスの上書きと、未登録サービスの追加を許可する。
  - ルート `mqtt_url` 指定時は `preview-runtime` に `--mqtt-url <value>` を自動注入する。
  - ルート `mqtt_url` は読み込み時に `scada-core` のBroker endpoint parserで検証する。
  - `mqtt_url` 検証失敗時は起動計画作成前にエラー終了する（例: `invalid mqtt_url ... missing host`）。
  - `schema_version` は `1.0.0` 固定で、異なる値は起動前エラーにする。
- `service-config` のサービス項目
  - `service`（必須）: サービス名。
  - `binary_path`（任意）: 実行ファイルパス。
  - `args`（任意）: 起動引数配列。
  - `envs`（任意）: 環境変数マップ。
  - `restart_on_exit`（任意）: 終了時の再起動対象フラグ。
  - `restart_max_attempts`（任意）: 再起動回数上限（`0` は無制限）。
  - `restart_backoff_ms`（任意）: 再起動前待機ミリ秒。
  - `restart_reset_after_ms`（任意）: 安定稼働後に再起動試行回数をリセットするしきい値ミリ秒。
- `--supervise-loop`
  - 監視ループを実行する。`--supervise-cycles 0` は無限実行。
  - `--supervise-interval-ms` で監視間隔を指定する。
  - `--restart-exited` 併用時、終了プロセスを再起動する。
  - 再起動ポリシーは `service-config` の値を優先し、未指定時はCLI引数の既定値を使う。
  - 既定は要約ログを出力し、`--supervise-verbose` 指定時のみサービス詳細ログを出力する。
  - `--supervise-summary-json` 指定時は `cycle_summary` / `final_summary` をJSON行で出力する。
  - `--supervise-fail-on-start-error` 指定時、起動失敗を検出すると非0終了する。
  - `--supervise-fail-on-exhausted-restart` 指定時、再起動試行枯渇を検出すると非0終了する。
- `supervise-loop` 既定値
  - `--supervise-interval-ms`: `1000`
  - `--supervise-cycles`: `0`（無限）
  - `--restart-max-attempts`: `0`（無制限）
  - `--restart-backoff-ms`: `0`
  - `--restart-reset-after-ms`: `0`（リセット無効）
  - `--restart-exited`: 未指定時は無効
  - `--supervise-verbose`: 未指定時は無効（要約ログ）
  - `--supervise-summary-json`: 未指定時は無効（テキスト要約）
  - `--supervise-fail-on-start-error`: 未指定時は無効
  - `--supervise-fail-on-exhausted-restart`: 未指定時は無効
- 再起動ポリシーの優先順位
  - サービス別 `service-config` 値
  - CLI引数の既定値
  - いずれも未指定の場合は上記 `supervise-loop` 既定値
- 実運用ルール（推奨）
  - 常駐監視の通常運転は「要約ログ + JSON要約」のどちらかに固定し、詳細ログは障害調査時のみ使う。
  - CI/ジョブ連携では `--supervise-fail-on-start-error` と `--supervise-fail-on-exhausted-restart` を併用する。
  - 監視側の判定は `events` と数値キー（`restart_exhausted`, `restart_failed`, `startup_errors`）を組み合わせる。
- `supervise-loop` イベントコード仕様（要約ログの `events`）
  - `START_ERROR`: 起動失敗が1件以上ある。
  - `RESTARTED`: 再起動成功が1件以上ある。
  - `RESTART_EXHAUSTED`: 再起動回数上限到達が1件以上ある。
  - `RESTART_FAILED`: 再起動実行が失敗したものが1件以上ある。
  - `RESTART_RESET`: 安定稼働による再起動回数リセットが1件以上ある。
  - `NONE`: 上記イベントが1件もない。
- `--supervise-summary-json` 出力例（監視連携向け）
```json
{"type":"cycle_summary","cycle":1,"running":5,"exited":0,"restarted":0,"restart_exhausted":0,"restart_failed":0,"restart_reset":0,"startup_errors":0,"events":"NONE"}
{"type":"cycle_summary","cycle":2,"running":0,"exited":5,"restarted":5,"restart_exhausted":0,"restart_failed":0,"restart_reset":0,"startup_errors":0,"events":"RESTARTED"}
{"type":"final_summary","cycles":2,"running_total":5,"exited_total":5,"restarted_total":5,"restart_exhausted_total":0,"restart_failed_total":0,"restart_reset_total":0,"startup_errors":0,"fail_on_start_error":false,"fail_on_exhausted_restart":false,"events":"RESTARTED"}
```
- `--supervise-summary-json` 最小パーサ仕様（必須キー）
  - `cycle_summary`:
    - `type` (`"cycle_summary"`)
    - `cycle` (integer)
    - `running` (integer)
    - `exited` (integer)
    - `restarted` (integer)
    - `restart_exhausted` (integer)
    - `restart_failed` (integer)
    - `restart_reset` (integer)
    - `startup_errors` (integer)
    - `events` (string)
  - `final_summary`:
    - `type` (`"final_summary"`)
    - `cycles` (integer)
    - `running_total` (integer)
    - `exited_total` (integer)
    - `restarted_total` (integer)
    - `restart_exhausted_total` (integer)
    - `restart_failed_total` (integer)
    - `restart_reset_total` (integer)
    - `startup_errors` (integer)
    - `fail_on_start_error` (boolean)
    - `fail_on_exhausted_restart` (boolean)
    - `events` (string)
- JSON Schema
  - `contracts/schemas/tauri-shell-supervise-summary.schema.json`
- バージョニング方針（監視連携互換）
  - 既存キーの削除・名称変更・型変更は互換破壊とみなし、同一フェーズ内では行わない。
  - 後方互換の拡張は「任意キーの追加」のみ許可する。
  - `type` の既存値（`cycle_summary`, `final_summary`）の意味は変更しない。
  - 互換破壊が必要な場合は新しい schema ファイルを追加し、旧スキーマを一定期間併存させる。
  - 監視側は未知キーを無視し、必須キーのみで動作する実装を推奨する。
- 監視アラート条件の運用例
  - Critical: `type=cycle_summary` かつ `events` に `RESTART_EXHAUSTED` を含む。
  - Warning: `type=cycle_summary` かつ `events` に `START_ERROR` を含む。
  - Warning: `type=cycle_summary` かつ `restart_failed > 0`。
  - Info: `type=cycle_summary` かつ `events` に `RESTARTED` を含む。
  - Recovery: `type=cycle_summary` かつ `events` に `RESTART_RESET` を含む。
  - Critical (終了判定): `type=final_summary` かつ `fail_on_start_error=true` または `fail_on_exhausted_restart=true`。
- 外部監視連携の最小案
  - Webhook連携（最小）:
    - `tauri-shell --supervise-loop --supervise-summary-json` の標準出力JSON行をログ収集プロセスで受信する。
    - 受信側で `type` と `events` を評価して、閾値一致時にWebhook通知を送信する。
    - `final_summary` 受信時は run 終了としてまとめ通知を送信する。
  - Exporter連携（最小）:
    - JSON行をExporterが読み、`restart_exhausted`, `restart_failed`, `startup_errors` をメトリクス化する。
    - `events` はラベルではなくカウンタへ分解して高カーディナリティを避ける。
    - `final_summary` を run 単位メトリクス（total系）として反映する。
  - 実装上の注意:
    - 監視側は `cycle_summary` の欠落に備えてタイムアウト監視を併用する。
    - `events` はカンマ区切り文字列のため、空白トリム後に分解して判定する。
- `supervise-loop` CLI実行例（運用向け）
  - テキスト要約（既定）
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.json --supervise-interval-ms 1000 --restart-exited`
  - テキスト詳細（トラブルシュート）
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.json --supervise-interval-ms 1000 --restart-exited --supervise-verbose`
  - JSON要約（監視連携）
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.json --supervise-interval-ms 1000 --restart-exited --supervise-summary-json`
  - JSON要約 + 失敗時非0終了（CI/ジョブ連携）
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.json --supervise-interval-ms 1000 --restart-exited --supervise-summary-json --supervise-fail-on-start-error --supervise-fail-on-exhausted-restart`
- 外部Broker運用例（mosquitto / TCP MQTT）
  - Homebrew導入:
    - `brew install mosquitto`
  - 単体起動:
    - `/opt/homebrew/opt/mosquitto/sbin/mosquitto -p 1883 -v`
  - `preview-runtime` 単体疎通:
    - `target/debug/preview-runtime --mqtt-receive-once --project-id demo --mqtt-url mqtt://127.0.0.1:1883`
    - `target/debug/preview-runtime --mqtt-publish-delta --project-id demo --mqtt-url mqtt://127.0.0.1:1883 --delta-payload '<tag-value-json>'`
  - `tauri-shell` 経由:
    - `target/debug/tauri-shell --print-service-plan --bin-dir target/debug --service-config config/tauri-shell.services.mosquitto.json`
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.mosquitto.json --restart-exited --supervise-summary-json`
  - 使い分け:
    - `config/tauri-shell.services.json`: `rumqttd` + WebSocket (`ws://127.0.0.1:8083/mqtt`) の検証用。
    - `config/tauri-shell.services.mosquitto.json`: `mosquitto` + TCP MQTT (`mqtt://127.0.0.1:1883`) の検証用。
  - 注意:
    - macOS sandbox環境ではローカルポートbindや接続に追加権限が必要な場合がある。
    - `config/tauri-shell.services.mosquitto.json` の `binary_path` はHomebrew標準パス前提。
- `--help` 出力例（抜粋）
```text
supervise-loop:
  --supervise-loop
  --supervise-interval-ms <ms>        default: 1000
  --supervise-cycles <n>              default: 0 (infinite)
  --restart-exited
  --restart-max-attempts <n>          default: 0 (unlimited)
  --restart-backoff-ms <ms>           default: 0
  --restart-reset-after-ms <ms>       default: 0 (disabled)
  --supervise-verbose                 per-service detailed logs
  --supervise-summary-json            emit cycle_summary/final_summary as JSON lines
  --supervise-fail-on-start-error     exit non-zero if any service start fails
  --supervise-fail-on-exhausted-restart
                                      exit non-zero if restart attempts are exhausted
```
- `--help` と docs の同期運用ルール
  - `tauri-shell` のCLIオプションを追加・変更したコミットでは、同じ変更で `--help` 文言と本ドキュメントを同時更新する。
  - レビュー時は `target/debug/tauri-shell --help` を実行し、`supervise-loop` セクションの差分を確認する。
  - 文言差分がある場合は、実装（`--help`）を正本として docs 側を合わせる。
  - 互換破壊を伴う変更時は `docs/progress.md` の「完了済み」「最新検証」に変更点と確認結果を追記する。
- JSON要約スキーマ運用手順（検証/更新フロー）
  - 新しいJSON要約キーを追加する場合は、先に `contracts/schemas/tauri-shell-supervise-summary.schema.json` を更新する。
  - 実装更新後に `cargo test -p tauri-shell` を実行し、`supervise_loop_reset` を含む回帰を確認する。
  - `docs/phase0_implementation_notes.md` の「最小パーサ仕様」「出力例」「イベントコード仕様」を同じ変更で更新する。
  - 互換破壊を伴う場合は、新スキーマファイルを追加し、旧スキーマを残したまま移行手順を明記する。
- CLI仕様と進捗記録の整合チェック結果
  - `--supervise-verbose`: 実装済み、`progress.md` 完了項目と一致。
  - `--supervise-summary-json`: 実装済み、JSON Schema追加済み、`progress.md` 完了項目と一致。
  - `--supervise-fail-on-start-error`: 実装済み、`progress.md` 完了項目と一致。
  - `--supervise-fail-on-exhausted-restart`: 実装済み、`progress.md` 完了項目と一致。
  - `--help` の supervisor説明: 実装済み、`progress.md` 最新検証に反映済み。
- `supervise-loop` オプション対応表（目的別早見表）
  - 常駐監視を始める（最小）:
    - `--supervise-loop --restart-exited`
  - 監視間隔を調整する:
    - `--supervise-interval-ms <ms>`
  - 有限回だけ確認する:
    - `--supervise-cycles <n>`
  - 再起動回数を制限する:
    - `--restart-max-attempts <n>`
  - 再起動前に待機する:
    - `--restart-backoff-ms <ms>`
  - 安定稼働後に再起動回数をリセットする:
    - `--restart-reset-after-ms <ms>`
  - サービス詳細ログで調査する:
    - `--supervise-verbose`
  - 監視連携向けにJSON出力する:
    - `--supervise-summary-json`
  - 起動失敗時に非0終了する:
    - `--supervise-fail-on-start-error`
  - 再起動枯渇時に非0終了する:
    - `--supervise-fail-on-exhausted-restart`
- supervisor運用 最小Runbook（起動/確認/障害時対応）
  - 起動:
    - `target/debug/tauri-shell --supervise-loop --bin-dir target/debug --service-config config/tauri-shell.services.json --restart-exited --supervise-summary-json`
  - 稼働確認:
    - `type=cycle_summary` が継続出力されることを確認する。
    - `events=NONE` を基準状態とし、`RESTARTED` は情報イベントとして扱う。
  - 障害一次対応:
    - `RESTART_EXHAUSTED` 発生時は Critical 扱いで対象サービス設定（`restart_max_attempts` / `restart_backoff_ms`）を確認する。
    - `START_ERROR` 発生時はバイナリパス・実行権限・依存プロセス（Brokerなど）を確認する。
    - `restart_failed > 0` 発生時は詳細確認のため `--supervise-verbose` で再実行する。
  - ジョブ/CI運用:
    - `--supervise-fail-on-start-error --supervise-fail-on-exhausted-restart` を付与し、非0終了を失敗判定に使う。
- supervisor運用 ログ例（JSON要約）
  - 正常サイクル例:
```json
{"type":"cycle_summary","cycle":12,"running":6,"exited":0,"restarted":0,"restart_exhausted":0,"restart_failed":0,"restart_reset":0,"startup_errors":0,"events":"NONE"}
```
  - 再起動発生例:
```json
{"type":"cycle_summary","cycle":13,"running":5,"exited":1,"restarted":1,"restart_exhausted":0,"restart_failed":0,"restart_reset":0,"startup_errors":0,"events":"RESTARTED"}
```
  - 枯渇発生例（Critical）:
```json
{"type":"cycle_summary","cycle":21,"running":4,"exited":2,"restarted":0,"restart_exhausted":1,"restart_failed":0,"restart_reset":0,"startup_errors":0,"events":"RESTART_EXHAUSTED"}
```
  - 終了要約例:
```json
{"type":"final_summary","cycles":30,"running_total":154,"exited_total":26,"restarted_total":12,"restart_exhausted_total":1,"restart_failed_total":0,"restart_reset_total":2,"startup_errors":0,"fail_on_start_error":false,"fail_on_exhausted_restart":true,"events":"RESTARTED,RESTART_EXHAUSTED,RESTART_RESET"}
```
- `supervise-loop` JSON要約のサンプルパーサ（jq）
  - Criticalイベントのみ抽出:
```bash
target/debug/tauri-shell ... --supervise-summary-json \
  | jq -c 'select(.type=="cycle_summary" and (.events|split(",")|index("RESTART_EXHAUSTED")))'
```
  - final_summary の要点表示:
```bash
target/debug/tauri-shell ... --supervise-summary-json \
  | jq -r 'select(.type=="final_summary") | "cycles=\(.cycles) exhausted=\(.restart_exhausted_total) failed=\(.restart_failed_total) events=\(.events)"'
```
  - Warning以上（START_ERROR / RESTART_FAILED / RESTART_EXHAUSTED）抽出:
```bash
target/debug/tauri-shell ... --supervise-summary-json \
  | jq -c 'select(.type=="cycle_summary") | . as $r
    | ($r.events|split(",")) as $e
    | select(($e|index("START_ERROR")) or ($e|index("RESTART_FAILED")) or ($e|index("RESTART_EXHAUSTED")) )'
```

## 縦断テスト

- `Mock Driver -> Driver Manager -> Tag Server` の値流れ: 成功
- `mock-driver --emit-once -> driver-manager --run-mock-cycle -> POST /api/v1/driver-values -> Tag Server snapshot` のプロセス境界: 成功
- `driver-manager --run-mock-loop --cycles 2 -> POST /api/v1/driver-values -> Tag Server snapshot` の周期投入: 成功
- `Driver Manager -> POST /api/v1/driver-values -> Tag Server MQTT publish -> Preview Runtime receive-once`: 成功
- `tauri-shell --supervise-loop --service-config config/tauri-shell.services.mosquitto.json` によるBroker/Tag Server/Driver Manager同時起動: 成功
- `Tag Server WritePolicy -> Mock Driver -> Driver Manager response handling` の書き込み流れ: 成功
- `POST /api/v1/control-commands -> Tag Server -> Driver Manager /api/v1/driver-writes -> Mock Driver`: 成功
  - `mock.running.001` 書き込みでControlCommandが `DriverAck` になることを確認
- `Tag Server REST snapshot -> Preview Runtime` の初期値取得: 成功
- `Screen Definition -> Tag resolve -> Tag Server REST snapshot -> Preview Runtime` の初期値取得: 成功
- `Screen Definition + TagSnapshot -> ScreenProjection` のオブジェクト投影: 成功
- `MQTT tag value delta -> ScreenProjection` のsequence適用: 成功
- `rumqtt publish/receive (単発CLI)` の経路: 実装済み（Broker実接続の常時購読は次フェーズ）
- `rumqtt WebSocket transport` のコンパイル経路: 成功
- `tauri-shell --service-config` による `rumqttd` 起動統合: 成功
- `tauri-shell --supervise-loop` の `restart-reset-after-ms` 発火確認: 成功

Xcodeライセンス承諾後、ワークスペース全体のテストが成功した。
Rust 1.66の増分コンパイルキャッシュでICEが発生していたが、Rust 1.95.0更新後は通常の `cargo test` が成功している。
