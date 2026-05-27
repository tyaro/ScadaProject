# SCADAアプリ 開発計画書

## 1. 目的

本書は、`docs/scada_basic_design.md` の基本設計をもとに、SCADAアプリケーションの開発順序、成果物、完了条件、検証方針を定義する。

本プロジェクトでは、ビルダーとランタイムを同時に育てる。ただし初期実装では、実機や本番ドライバを待たずに動作検証できるよう、Mock通信基盤と最小ランタイム縦断を先に構築する。

## 2. 開発方針

## 2.1 基本方針

- Mockドライバも本番と同じ `Driver -> Driver Manager -> Tag Server -> Runtime -> Client` の経路を通す。
- Runtime直結のMock経路は作らない。
- SVGにタグ情報を直接埋め込まず、画面定義JSON/DBを正本にする。
- 初期表示は `REST snapshot + MQTT delta` 方式にする。
- 制御操作はREST APIで受け、`Runtime -> Tag Server -> Driver Manager -> Driver` の順に中継する。
- SurrealDBは内部DB、設計データ、短期キャッシュに使う。
- 長期履歴、長期監査ログ、大量イベント履歴は別ストレージへ逃がせる設計にする。
- Tauri v2のRust側は薄く保ち、ローカルサービス起動、OS連携、ウィンドウ管理に寄せる。
- 開発時はTauri Shell配下の子プロセスとして起動し、本番時はOSサービス、デーモン、またはコンテナサービスとして常駐できる構成にする。
- 通信ドライバは個別サービス化せず、Driver Manager配下の子プロセスとして管理する。

## 2.2 優先する縦断動作

初期MVPでは、以下の縦断動作を最優先で成立させる。

```text
Mock Driver
  -> Driver Manager
  -> Tag Server
  -> MQTT / Runtime
  -> Svelte画面
```

書き込みは以下を最優先で成立させる。

```text
Svelte画面
  -> Runtime REST API
  -> Tag Server
  -> Driver Manager
  -> Mock Driver
```

## 2.3 開発単位

| 単位 | 内容 |
| --- | --- |
| Tauri Shell | ローカルサービス起動、終了、ヘルスチェック、ログ表示 |
| Builder UI | Svelteによるタグ、画面、アラーム、プロジェクト編集画面 |
| Builder API | 設計データ保存、検証、ProjectVersion作成 |
| Preview Runtime | 画面定義解釈、REST API、画面表示用API |
| Tag Server | タグ定義、最新値、品質、短期キャッシュ、MQTT配信 |
| Driver Manager | ドライバ起動、監視、gRPC接続、値正規化 |
| Mock Driver | 模擬値生成、書き込み応答、異常状態再現 |
| Alarm Engine | タグ値購読、アラーム判定、状態管理 |

## 2.4 実行形態

同じサービス実装を、開発時と本番時で異なる起動主体から実行できるようにする。

| モード | 起動主体 | 対象 |
| --- | --- | --- |
| Local Preview | Tauri Shell | Builder API、Preview Runtime、Tag Server、Driver Manager、Mock Driver、MQTT Broker |
| Standalone Service | systemd、launchd、Windows Service | SCADA Runtime、Tag Server、Driver Manager、Alarm Engine、MQTT Broker、History Adapter |
| Container Service | Docker、Docker Compose、Kubernetes | サーバー側サービス一式 |

本番で常駐させる対象は、SCADA Runtime、Tag Server、Driver Manager、Alarm Engine、MQTT Broker/WebSocket Gateway、History Adapterとする。ドライバはDriver Managerの管理下で起動する。

## 3. フェーズ計画

## 3.0 進行状況サマリ（運用ビュー）

この節は、開発計画書上で現在地を素早く把握するための要約ビューとする。
進捗の正本は `docs/progress.md` とし、この節はその要点を反映して更新する。

| フェーズ | 状態 | 現在の補足 |
| --- | --- | --- |
| フェーズ0: Mock通信基盤 | 完了（主要完了条件を満たす） | Mock値流れ、Mock書き込み流れ、MQTT配信、ローカルsupervisor運用を確認済み |
| フェーズ1: Runtime最小縦断 | 実施中 | Runtime API/Tag ServerのエラーJSON契約を固定化し、Runtime UIで `error` / `publish_errors` / statusフォールバック表示をE2Eで検証済み。OpenAPIは `components.responses` / `components.schemas` で成功・失敗応答とrequestBody入力DTOを共通化済み。Tag Serverの操作ログ最小保存（`GET /api/v1/operation-logs`）、Preview RuntimeのSVG modify rules最小投影（`modifiers`）、Runtime UIでの `modifiers` 最小反映（色・表示・文字）まで実装済み |
| フェーズ2: Builder最小機能 | 未着手 | フェーズ1完了条件の充足後に着手 |
| フェーズ3: 現場通信・履歴・アラーム | 未着手 | 実ドライバ、履歴、Alarm Engineは後続 |
| フェーズ4: 制御・帳票・公開管理 | 未着手 | MVPスコープ外のため後続 |
| フェーズ5: 運用強化 | 未着手 | MVP後の運用強化フェーズ |

### 3.0.1 更新ルール

- 実装や検証を進めたときは、まず `docs/progress.md` を更新する。
- フェーズ状態が変わる変更（例: フェーズ0完了、フェーズ1着手、主要完了条件の達成）があったときは、この 3.0 節も同じコミットで更新する。
- 詳細ログや検証コマンドは `docs/progress.md` に記録し、この 3.0 節は要点のみを保持する。

## 3.1 フェーズ0: Mock通信基盤

### 目的

実機や実ドライバなしで、タグ値、品質、通信断、書き込み応答、アラーム相当状態を検証できる基盤を作る。

### 実装範囲

- Tauri Shellからのローカルサービス起動
- SurrealDB embeddedの初期化
- Builder API、Preview Runtime、Tag Server、Driver Managerのプロセス雛形
- Mock Driver、Mock Driver Manifest
- gRPC `.proto` 雛形
- REST OpenAPI雛形
- JSON Schema雛形
- タグ値DTO
- 品質コード
- ControlCommand状態定義
- MQTT topic、payload最小定義
- Driver ManagerからTag Serverへの値連携
- Tag Serverによる最新値キャッシュ
- MQTT over WebSocketによるタグ値配信
- REST API経由のMock書き込み要求

### 完了条件

- Tauri Shellから全ローカルサービスを起動、停止できる。
- Mock Driverが周期的にタグ値を生成できる。
- Driver ManagerがMock Driverを起動、監視できる。
- Tag ServerがMock値を受け取り、最新値と品質を保持できる。
- MQTT over WebSocketでタグ値を購読できる。
- REST APIでMock Driverへの書き込み要求を送れる。
- サービスごとのログを確認できる。

### テスト

- gRPC契約テスト
- ローカルサービス起動テスト
- Tag Serverタグ値モデルテスト
- MQTT publish/subscribeテスト
- Mock Driver書き込み応答テスト

## 3.2 フェーズ1: Runtime最小縦断

### 目的

Mockタグ値が `Driver Manager -> Tag Server -> Runtime -> SVG画面` へ反映され、REST API経由の操作がMock Driverへ届く最小の縦断動作を成立させる。

### 実装範囲

- Runtime REST API
- 画面定義JSONの読込
- SurrealDBからのタグ定義、画面定義読込
- Tag Serverへのタグ定義反映
- Tag Serverからの最新値取得
- `REST snapshot + MQTT delta` による初期同期
- ページ単位の画面表示
- SVGオブジェクト表示
- SVGオブジェクトとタグ値の最小バインディング
- REST APIによる書き込み要求受付
- ControlCommand状態遷移の最小実装
- 操作ログの最小保存
- Svelte監視画面の最小表示

### 完了条件

- 画面を開くとREST snapshotで初期値が表示される。
- MQTT deltaで値更新が画面に反映される。
- MQTT再接続時にスナップショットを再取得できる。
- SVGオブジェクトの色、文字、表示状態をタグ値で変更できる。
- ボタン操作からMock Driverへ書き込み要求が届く。
- ControlCommandの状態が `Requested` から最終状態まで追跡できる。

### テスト

- Runtime REST API契約テスト
- MQTT再接続テスト
- SVGバインディングテスト
- 制御指令ライフサイクルテスト
- 操作ログ保存テスト
- Runtime UIエラー表示回帰テスト（`error` / `publish_errors` / 非JSONフォールバック）

## 3.3 フェーズ2: Builder最小機能

### 目的

ビルダーで作成したタグと画面を、同梱Runtime上で即時にプレビューできる状態を作る。

### 実装範囲

- 通信タグエディタ最小機能
- タグ定義の作成、編集、削除
- タグとMock Driverアドレスのバインディング
- 画面エディタ最小機能
- SVGオブジェクト配置
- 数値表示、ランプ、ボタン相当のSVGテンプレート
- 画面オブジェクトとタグのバインディング
- Project保存
- ProjectVersionスナップショットの最小実装
- 下書きからPreview Runtimeへの即時反映

### 完了条件

- Builder UIでタグを作成し、Mock Driverの値と紐付けられる。
- Builder UIで画面を作成し、SVGオブジェクトを配置できる。
- 画面オブジェクトにタグをバインドできる。
- 下書き設定をPreview Runtimeで即時確認できる。
- ProjectVersionとして設定スナップショットを保存できる。

### テスト

- JSON Schema検証テスト
- Project保存/読込テスト
- ProjectVersion作成テスト
- Builder API契約テスト
- Preview Runtime反映テスト

## 3.4 フェーズ3: 現場通信・履歴・アラーム

### 目的

Mock基盤で固めた経路に、実ドライバ、履歴保存、アラーム判定を追加する。

### 実装範囲

- 実ドライバ用マニフェスト
- ドライバ管理画面
- Modbus TCPまたはOPC UAの初期ドライバ
- 履歴保存アダプタ
- 長期履歴ストレージ連携
- トレンド表示
- 設備詳細画面
- Alarm Engine
- アラーム設定ツール最小機能
- アラーム発生、確認、復旧、履歴

### 完了条件

- 実ドライバをDriver Managerから起動、停止、監視できる。
- 実ドライバの値がTag Serverへ入り、画面へ表示される。
- 履歴保存アダプタ経由で時系列履歴を保存できる。
- トレンド画面で履歴値を表示できる。
- Alarm Engineがタグ値からアラームを発生、復旧できる。
- アラーム状態がMQTTで配信される。

### テスト

- 実ドライバ接続テスト
- 履歴保存アダプタテスト
- トレンド検索テスト
- アラーム状態遷移テスト
- アラームMQTT配信テスト

## 3.5 フェーズ4: 制御・帳票・公開管理

### 目的

運用に必要な制御操作、帳票、プロジェクト公開管理、差分、承認、ロールバックを整える。

### 実装範囲

- 制御指令のreadback確認
- 操作権限、再認証、操作理由入力
- 帳票出力
- アラーム通知
- イベント履歴
- 画面管理
- オブジェクトライブラリ管理
- ProjectVersion単位の原子的公開
- 設定差分
- 承認フロー
- ロールバック

### 完了条件

- 制御指令の送信、応答、readback、失敗、タイムアウトを追跡できる。
- 重要操作に再認証や操作理由入力を適用できる。
- ProjectVersion単位で公開、失敗時維持、ロールバックができる。
- 画面、タグ、アラーム、ドライバ設定の差分を確認できる。
- 帳票を出力できる。

### テスト

- 制御指令readbackテスト
- 権限テスト
- 公開/ロールバックテスト
- 差分検証テスト
- 帳票出力テスト

## 3.6 フェーズ5: 運用強化

### 目的

本番運用に向けて、可用性、監査性、保守性、性能を強化する。

### 実装範囲

- バックアップ、リストア
- SurrealDB migration強化
- 監査ログ改ざん耐性
- ログ保管基盤連携
- OSサービス/デーモン化
- Docker ComposeまたはKubernetes用の起動定義
- サービス起動順、依存関係、再起動ポリシー
- 冗長化検討
- 外部システム連携
- 性能改善
- 長時間運転テスト

### 完了条件

- 設定、短期キャッシュ、長期履歴、監査ログのバックアップ単位が明確である。
- 復旧手順を実行できる。
- 長時間運転でメモリ、CPU、MQTT再接続、ログ肥大が許容範囲に収まる。
- 主要操作が監査ログとして追跡できる。
- UIを終了しても、Tag Server、Driver Manager、Runtime、Alarm Engine、MQTT Brokerが継続動作できる。
- OSサービスまたはコンテナサービスとして起動、停止、再起動、ヘルスチェックができる。

### テスト

- バックアップ/リストアテスト
- 長時間運転テスト
- 負荷テスト
- 監査ログ検証テスト
- 障害復旧テスト
- サービス起動/停止テスト
- サービス再起動ポリシーテスト

## 4. MVPスコープ

初期MVPは、フェーズ0からフェーズ2までを対象とする。

### MVPに含める

- Mock Driver
- Driver Manager最小
- Tag Server最小
- SurrealDB embedded
- MQTT over WebSocketタグ値配信
- REST snapshot + MQTT delta
- Runtime最小
- SVG画面表示
- タグエディタ最小
- 画面エディタ最小
- Project保存
- ProjectVersion最小
- Mock書き込み
- ControlCommand状態最小

### MVPに含めない

- 実PLC接続
- 長期履歴保存
- 本格的なアラーム通知
- 帳票
- 承認フロー
- 冗長化
- 長期監査ログ保全
- 複雑なオブジェクトライブラリ

## 5. 初期決定事項

開発開始前に以下を決定する。

| 項目 | 内容 |
| --- | --- |
| タグ値DTO | `tag_id`、`value`、`quality`、`source_timestamp`、`server_timestamp`、`sequence`等 |
| 品質コード | Good、Uncertain、Bad、Stale、CommLost、OutOfRange、Manual、Simulated |
| ControlCommand状態 | Requested、Accepted、Validated、Sent、DriverAck、DeviceAck、Verified、Timeout、Failed、Rejected |
| MQTT topic | `scada/{project_id}/tag/{tag_id}/value` 等 |
| MQTT QoS/retain | 用途別に決定 |
| REST snapshot API | 画面単位のタグ最新値取得API |
| gRPC proto | Driver、Driver Manager、Tag Server境界 |
| JSON Schema | 画面、タグ、アラーム、ドライバマニフェスト |
| SurrealDB schema_version | 定義データの互換性管理 |
| ProjectVersion | 公開単位、active切替、ロールバック方式 |
| ローカルサービス認証 | `127.0.0.1` bind、ランダムポート、起動時トークン |

## 6. 成果物一覧

| 成果物 | フェーズ |
| --- | --- |
| gRPC `.proto` | フェーズ0 |
| REST OpenAPI | フェーズ0 |
| JSON Schema | フェーズ0 |
| Mock Driver | フェーズ0 |
| Driver Manager最小 | フェーズ0 |
| Tag Server最小 | フェーズ0 |
| Runtime最小 | フェーズ1 |
| SVG画面ランタイム | フェーズ1 |
| Svelte監視画面 | フェーズ1 |
| タグエディタ最小 | フェーズ2 |
| 画面エディタ最小 | フェーズ2 |
| ProjectVersion最小 | フェーズ2 |
| 実ドライバ | フェーズ3 |
| Alarm Engine | フェーズ3 |
| 履歴保存アダプタ | フェーズ3 |
| 公開管理 | フェーズ4 |
| 帳票 | フェーズ4 |
| バックアップ/リストア | フェーズ5 |
| サービス/デーモン化定義 | フェーズ5 |

## 7. リスクと対策

| リスク | 対策 |
| --- | --- |
| Mockと本番経路が乖離する | Mockも本番と同じDriver Manager、Tag Server経由にする |
| Tauri Rust側が肥大化する | Tauri Shellはプロセス管理とOS連携に限定する |
| MQTT再接続で値が欠落する | REST snapshot + MQTT deltaを採用する |
| 制御結果が曖昧になる | ControlCommand状態遷移を実装する |
| 設定公開が中途半端になる | ProjectVersion単位でactiveを切り替える |
| SurrealDBスキーマ変更で壊れる | schema_versionとmigrationを用意する |
| SVG由来のセキュリティリスク | SVGをサニタイズし、タグ情報は埋め込まない |
| 長期履歴が肥大化する | 長期履歴ストレージへ分離する |
| UI終了で監視処理が止まる | 本番では主要コンポーネントをサービス/デーモンとして常駐させる |
| ドライバが個別サービス化して管理が散らばる | ドライバはDriver Manager配下の子プロセスとして管理する |

## 8. 実装順序

1. 契約定義: DTO、品質コード、ControlCommand、gRPC、OpenAPI、JSON Schema
2. ローカルサービス起動: Tauri Shell、SurrealDB、各プロセス雛形
3. Mock Driver: 値生成、品質、書き込み応答
4. Driver Manager: 起動、監視、gRPC接続
5. Tag Server: 最新値、品質、短期キャッシュ、MQTT配信
6. Runtime: snapshot API、画面定義読込、SVG反映
7. Svelte監視画面: 値表示、操作、再接続
8. Builder API: Project保存、定義検証
9. タグエディタ: タグ作成、Mockアドレス紐付け
10. 画面エディタ: SVG配置、タグバインディング
11. ProjectVersion: スナップショット、プレビュー反映
12. サービス化: systemd、launchd、Windows Service、Docker Compose等の起動定義

## 9. 次のアクション

1. フェーズ0のリポジトリ構成を決める。
2. `.proto`、OpenAPI、JSON Schemaの置き場所を決める。
3. SurrealDB embeddedの起動方式を検証する。
4. Mock Driverのタグ値パターンを定義する。
5. Tauri Shellから複数サービスを起動する最小サンプルを作る。
