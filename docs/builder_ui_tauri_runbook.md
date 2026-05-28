# Builder UI src-tauri 検証ランブック

## 1. 目的

このランブックは、Builder UI のネイティブ picker コマンド `pick_screen_relative_path` を手動で反復検証するための手順を定義します。

次の検証時に使用します。
- `apps/builder-ui/src-tauri` の起動挙動
- ネイティブファイルダイアログ連携
- `relative_path` の正規化とバリデーション（`config/screens/*.screen.json`）

## 2. 事前条件

- リポジトリルートから実行すること。
- Node.js 24 が有効であること。
- Rust stable ツールチェーンがインストール済みであること。
- `apps/builder-ui` に依存関係がインストール済みであること。

## 3. 事前クイックチェック

```bash
cd apps/builder-ui
npm run check
npm run tauri:check
cd ../..
cargo test --manifest-path apps/builder-ui/src-tauri/Cargo.toml
```

期待結果:
- Svelte/TypeScript のチェックが通る。
- src-tauri のコンパイルチェックが通る。
- src-tauri のユニットテストが通る。

## 4. ネイティブ Builder UI の起動

1つ目のターミナルでフロントエンド開発サーバーを起動します。

```bash
cd apps/builder-ui
npm run dev
```

2つ目のターミナルでネイティブシェルを起動します。

```bash
cd apps/builder-ui
npm run tauri:dev:project
```

このコマンドは `SCADA_PROJECT_ROOT` を明示的に設定し、ネイティブシェルを起動します。

注意:
- script 名は `tauri:dev:project`（コロン区切り）です。`tauri:dev/project`（スラッシュ）は無効です。
- `tauri:dev:project` は `cargo run` を呼ぶため、`beforeDevCommand` は自動実行されません。`npm run dev` を先に起動しないと白画面になる場合があります。

## 5. 手動検証シナリオ

### 5.1 正常選択

1. アプリ上で `Pick via Tauri` をクリックする。
2. `config/screens/` 配下かつ `.screen.json` サフィックスのファイルを選択する。
   例: `config/screens/mock-main.screen.json`

期待結果:
- Save As 入力欄が、選択した `relative_path` に更新される。
- I/O ステータスに `Selected config/screens/mock-main.screen.json` が表示される。

### 5.2 選択キャンセル

1. `Pick via Tauri` をクリックする。
2. ファイルを選択せずダイアログを閉じる。

期待結果:
- 既存の Save As 入力値が変更されない。
- I/O ステータスに `File selection cancelled` が表示される。

### 5.3 不正パスの拒否

1. `Pick via Tauri` をクリックする。
2. `config/screens/*.screen.json` 条件を満たさないファイルを選択する。
   例:
   - `README.md`
   - `config/other/a.screen.json`

期待結果:
- Save As 入力欄が不正な値で上書きされない。
- I/O ステータスに `Path selection failed: ...` が表示される。

## 6. トラブルシューティング

- 症状: 起動時に project root 関連エラーで失敗する。
  - `SCADA_PROJECT_ROOT` が絶対パスになっているか確認する。
  - `npm run tauri:dev` より `npm run tauri:dev:project` を優先する。

- 症状: picker は動作するが save-as パスが拒否される。
  - 選択ファイルが `config/screens/` 配下であることを確認する。
  - ファイル名サフィックスが `.screen.json` であることを確認する。

- 症状: コマンドはコンパイルできるが実行時挙動が異なる。
  - `cargo test --manifest-path apps/builder-ui/src-tauri/Cargo.toml` を再実行する。
  - `scripts/check_local_ci.sh` を再実行し、契約チェックを確認する。
