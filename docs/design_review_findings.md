# 基本設計レビュー指摘事項

## 1. レビュー概要

`docs/scada_basic_design.md` の現時点の設計は、SCADAの基本方針として大きな破綻はない。特に以下の判断は妥当である。

- サーバ・クライアント構成にする。
- クライアントはREST APIとMQTT over WebSocketだけを使う。
- 通信ドライバは別プロセスにする。
- Mockドライバも本番と同じ `Driver -> Driver Manager -> Tag Server -> Runtime` 経路を通す。
- SVGにタグ情報を直接埋め込まず、画面定義JSON/DBを正本にする。
- SurrealDBは内部DB、設計データ、短期キャッシュに使い、長期履歴は別ストレージに分離する。

一方で、実装開始前に明確化しないと手戻りや安全上の問題になりやすい未決定事項がある。本書では優先度順に抜け・漏れ・注意点を整理する。

## 2. 優先度P0: 実装前に決めるべき事項

## 2.1 Tag Serverの責務境界

### 指摘

Tag Serverを追加したことで、タグ値、品質、履歴、アラーム、制御検証の責務が複数コンポーネントにまたがる。現状の設計では、SCADA Runtime、Tag Server、Alarm Engine、MQTT Brokerのどこが何を所有するかがまだ曖昧である。

### 決めること

| 項目 | 推奨 |
| --- | --- |
| タグ定義の正本 | SurrealDB上の公開済みタグ定義 |
| 最新値の所有者 | Tag Server |
| 品質、最終更新時刻の所有者 | Tag Server |
| MQTTタグ値配信 | Tag Server |
| アラーム判定 | Alarm Engine |
| アラーム状態の保存 | Alarm Engine、長期履歴側 |
| 画面定義の解釈 | SCADA Runtime |
| 画面へのタグ値提供 | Tag ServerからRuntimeまたはMQTT |
| 書き込み検証 | Tag Server |
| ドライバ実行管理 | Driver Manager |

### 対応案

`Tag Server`、`SCADA Runtime`、`Alarm Engine`、`Driver Manager` の責務表を基本設計に追加する。特に「誰がMQTTへpublishするか」は重複しやすいため、タグ値はTag Server、アラームはAlarm Engine、画面・操作イベントはRuntimeのように分ける。

## 2.2 タグ値モデルと品質モデル

### 指摘

タグ値のデータ構造がまだ薄い。SCADAでは値そのものより、品質、時刻、更新周期、stale判定が重要になる。

### 決めること

- `tag_id`
- `value`
- `data_type`
- `quality`
- `source_timestamp`
- `server_timestamp`
- `sequence`
- `scan_interval_ms`
- `stale_after_ms`
- `driver_id`
- `endpoint_id`
- `read_status`
- `write_status`

### 品質例

| 品質 | 意味 |
| --- | --- |
| Good | 正常 |
| Uncertain | 値はあるが信頼性に注意 |
| Bad | 使用不可 |
| Stale | 更新期限切れ |
| CommLost | 通信断 |
| OutOfRange | 工業値範囲外 |
| Manual | 手動入力 |
| Simulated | シミュレーション |

### 対応案

Tag Serverの最初の実装前に、タグ値DTOと品質コードを固定する。MQTTペイロード、RESTレスポンス、gRPCメッセージで同じ概念を使う。

## 2.3 初期同期と再接続時の整合性

### 指摘

MQTT over WebSocketでリアルタイム配信する方針は良いが、画面を開いた瞬間や再接続時に「現在値」と「差分更新」をどう整合させるかが未定義である。

### 推奨フロー

1. RuntimeまたはTag ServerのREST APIで画面に必要なタグのスナップショットを取得する。
2. MQTTで対象タグの更新を購読する。
3. `sequence` または `server_timestamp` で古い更新を破棄する。
4. MQTT再接続時はスナップショットを再取得する。

### 対応案

画面ランタイムのデータ取得方式として `REST snapshot + MQTT delta` を明記する。

## 2.4 制御指令ライフサイクル

### 指摘

REST API経由で制御要求を受ける方針は良いが、制御指令の状態遷移が未定義である。書き込みは成功/失敗だけでは足りない。

### 状態案

| 状態 | 説明 |
| --- | --- |
| Requested | クライアントが要求した |
| Accepted | Runtimeが受け付けた |
| Validated | Tag Serverが権限、型、範囲を検証した |
| Sent | Driver Managerからドライバへ送信した |
| DriverAck | ドライバが受け付けた |
| DeviceAck | PLC/RTUから応答があった |
| Verified | readback等で反映確認済み |
| Timeout | 応答が期限切れ |
| Failed | 失敗 |
| Rejected | 権限、範囲、インターロック等で拒否 |

### 対応案

`ControlCommand` の詳細項目、状態遷移、タイムアウト、readback確認、冪等性キー、操作キャンセル可否を定義する。

## 2.5 認証・認可の適用境界

### 指摘

認証・認可は記載されているが、REST、MQTT、gRPC、ローカル同梱サービスそれぞれの境界が未定義である。

### 決めること

| 境界 | 必要な対策 |
| --- | --- |
| REST API | 認証、RBAC、CSRF/CORS、監査ログ |
| MQTT over WebSocket | 認証、topic ACL、再接続時の権限再確認 |
| gRPC内部通信 | mTLSまたはサービス間トークン |
| Tauriローカルサービス | localhost限定、ランダムポート、起動時トークン |
| 重要操作 | 再認証、二段階確認、操作理由入力 |

### 対応案

権限モデルを `画面権限`、`タグ読取権限`、`タグ書込権限`、`アラーム確認権限`、`設定変更権限`、`公開権限` に分ける。

## 2.6 設定公開の原子性

### 指摘

公開フローはあるが、画面、タグ、アラーム、ドライバ設定を同時に公開する際の原子性が未定義である。タグだけ更新され、画面定義が古いままになるような中途半端な状態は避ける必要がある。

### 対応案

- 公開単位は `ProjectVersion` とする。
- Runtime、Tag Server、Alarm Engine、Driver Managerは同じ `project_version_id` を読む。
- 公開時は検証済みスナップショットを作成し、最後にactive versionを切り替える。
- 失敗時は旧active versionを維持する。
- ロールバックはactive versionの切り替えとして扱う。

## 2.7 ローカル同梱サービスの運用

### 指摘

Tauri Shellが複数プロセスを起動する方針は良いが、ポート、ログ、クラッシュ、起動順、終了処理が未定義である。

### 対応案

- Tauri Shellがサービス起動順を管理する。
- 各サービスは `/health` またはgRPC health checkを持つ。
- ポートは固定ではなく、衝突時に再割当できるようにする。
- UIはサービス状態を表示する。
- ログはサービス別にローテーションする。
- 終了時は子プロセスを確実に停止する。
- ローカルAPIは `127.0.0.1` にbindし、起動時トークンを必須にする。

## 3. 優先度P1: MVP中に決めるべき事項

## 3.1 MQTTトピック設計

### 指摘

MQTTの利用方針はあるが、topic構造、retain、QoS、ACL、ペイロード形式が未定義である。

### 対応案

初期案として以下を決める。

```text
scada/{project_id}/tag/{tag_id}/value
scada/{project_id}/tag/{tag_id}/quality
scada/{project_id}/alarm/{alarm_id}/state
scada/{project_id}/driver/{driver_id}/status
scada/{project_id}/event/system
```

- QoSは初期値 `0` または `1` を用途別に決める。
- 最新値をretainするかは、RESTスナップショット方式と合わせて決める。
- ブラウザクライアントは閲覧権限のあるtopicだけ購読可能にする。

## 3.2 gRPCとRESTの契約管理

### 指摘

gRPCとRESTを併用するため、契約のずれが起きやすい。

### 対応案

- gRPCは `.proto` を正本にする。
- REST APIはOpenAPIを正本にする。
- 画面定義、タグ定義、アラーム定義、ドライバマニフェストはJSON Schemaを用意する。
- Mock Driverは契約テストの基準にする。

## 3.3 アラームエンジンの配置

### 指摘

基本設計ではAlarm Engineがあるが、Tag Server、Runtime、DBとの接続責務が薄い。

### 対応案

- Alarm EngineはTag Serverの値更新ストリームを購読する。
- アラーム状態はAlarm Engineが管理する。
- アラーム通知はMQTTへpublishする。
- アラーム履歴は長期履歴ストレージまたはRDBへ保存する。
- アラーム確認はREST APIで受け、Alarm Engineが状態更新する。

## 3.4 SurrealDBのスキーマ・マイグレーション

### 指摘

SurrealDBを内部DBに使う方針は良いが、スキーマ変更、プロジェクトファイル互換性、バックアップが未定義である。

### 対応案

- 全定義に `schema_version` を持たせる。
- Builder起動時にマイグレーションを実行する。
- Project export/import形式を定義する。
- embedded利用とserver利用で同じRepository層を使う。
- SurrealDBのバックアップ/リストア手順を作る。

## 3.5 画面ランタイムの性能設計

### 指摘

SVG DOMで描画する方針は妥当だが、大量オブジェクト・高頻度タグ更新で詰まる可能性がある。

### 対応案

- 画面単位で購読タグを絞る。
- タグ更新を一定間隔でbatch applyする。
- DOM更新は値が変わった要素だけに限定する。
- 表示中でないページの購読を停止する。
- SVGテンプレートはサニタイズ済みassetとして管理する。

## 3.6 時刻同期とタイムゾーン

### 指摘

履歴、アラーム、監査では時刻の扱いが重要だが、時刻基準が未定義である。

### 対応案

- 保存時刻はUTCを基本にする。
- 表示時刻はユーザーまたは設備ローカルタイムゾーンで変換する。
- `source_timestamp` と `server_timestamp` を分ける。
- サーバー、Driver Manager、ドライバ実行環境の時刻同期方針を決める。

## 3.7 ドライバの安全性

### 指摘

ドライバを後から追加できる構成は便利だが、任意プロセス実行に近くなる。

### 対応案

- ドライバマニフェスト署名を検討する。
- 実行可能ディレクトリを制限する。
- ドライバのネットワーク/ファイル権限をマニフェストに明記する。
- ドライバ更新時は互換性チェックを行う。
- ドライバ別ログ、クラッシュ回数、再起動履歴を保存する。

## 4. 優先度P2: 後続設計で詰める事項

## 4.1 バックアップ・リストア

SurrealDB、プロジェクトスナップショット、長期履歴DB、監査ログを別々に扱うため、バックアップ単位と復旧順序を決める必要がある。

## 4.2 冗長化

Tag Server、MQTT Broker、長期履歴DB、Runtimeを冗長化する場合、active/standbyか水平分散かを決める必要がある。MVPでは単一構成でよいが、責務分離だけは崩さない。

## 4.3 監査ログの改ざん耐性

監査ログは単なるDB保存だけでは弱い。ハッシュチェーン、WORMストレージ、外部ログ保管基盤などを後続で検討する。

## 4.4 テスト戦略

以下のテスト区分を用意する。

- gRPC契約テスト
- REST API契約テスト
- Tag Serverのタグ値モデルテスト
- Mock Driver統合テスト
- 画面ランタイムのSVGバインディングテスト
- MQTT再接続テスト
- 制御指令ライフサイクルテスト
- 公開/ロールバックテスト

## 5. 基本設計への反映推奨

次回の基本設計更新では、以下の章を追加することを推奨する。

1. コンポーネント責務詳細
2. タグ値・品質モデル
3. MQTTトピック設計
4. 制御指令ライフサイクル
5. 認証・認可境界
6. 設定公開の原子性
7. ローカル同梱サービス運用
8. SurrealDBスキーマ・マイグレーション
9. テスト戦略

## 6. 結論

現状の基本設計は、アーキテクチャの方向性としては問題ない。ただし、Tag Serverを中心にしたデータ所有権、MQTT/REST/gRPCの契約、制御指令の状態管理、設定公開の原子性を決めないまま実装すると、MVP後に手戻りが大きくなる。

最初の実装前に最低限決めるべきものは、タグ値DTO、品質コード、REST snapshot + MQTT delta、ControlCommand状態遷移、ProjectVersion公開方式である。
