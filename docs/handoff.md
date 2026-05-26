# コンテキストリセット用ハンドオフ

このファイルは、会話コンテキストをリセットした後に作業を再開するための最短ガイドである。

## 進捗管理ポリシー

- 進捗の正本（Single Source of Truth）は `docs/progress.md` とする。
- `docs/handoff.md` には再開手順と参照先のみを記載し、進捗詳細は書かない。
- 実装進捗、次タスク、検証ログ、運用メモの更新は `docs/progress.md` のみ更新する。

## 最初に読むもの

1. `AGENTS.md`
2. `docs/progress.md`
3. `docs/development_plan.md`
4. `docs/phase0_implementation_notes.md`
5. 必要に応じて `docs/scada_basic_design.md`

## 次セッション引き継ぎチェックリスト

1. `git status --short` で未コミット差分を確認する。
2. `docs/progress.md` の「現在のフェーズ」「現在作業中」「次に行うこと」「最新検証」を読む。
3. `docs/progress.md` の最新検証コマンドを必要な範囲で再実行する。
4. 新しい実装・仕様変更を行ったら `docs/progress.md` を更新する。

## 現セッションからの再開メモ

- 未コミット差分は多い。特に `preview-runtime` MQTT抽象、`tauri-shell` supervisor/service-config、`config/*.json`、`contracts/schemas/*.json`、関連docsを確認する。
- 外部Broker検証は `rumqttd` と `mosquitto` の両方で実施済み。`mosquitto` は Homebrew 導入済み。
- 再開直後の最小確認は `cargo test -p scada-core -p preview-runtime -p tauri-shell`。
- ローカルポートbindを含む縦断確認はサンドボックス外権限が必要になる場合がある。

## 注意点

- ローカルポートbindやcurlでの縦断確認は、環境によってサンドボックス外権限が必要になる。
- 機能方針やデータフローを変更する場合は、設計書と開発計画も同じ変更で更新する。
