# Higiri

プロジェクト単位で TODO を管理し、日限が近いものに起動直後に気づけるデスクトップアプリです。データはこの PC の中だけに保存し、外部とは通信しません。Windows と macOS で動きます。

## できること

- プロジェクトと TODO の管理(日限、予定・実績の期間、6つのステータス)
- 起動時のリマインドと、日限注意をまとめたダッシュボード
- ガントチャート(表示のみ)
- Markdown のメモと作業ログ
- 日報の作成、コピー、.md での保存
- 毎日の自動バックアップと、バックアップからの復元

## インストール

Actions の実行結果から ZIP をダウンロードし、展開して使います。インストーラーはありません。ダウンロードには GitHub へのログインが必要です。

- Windows: [Windows build](../../actions/workflows/windows.yml) の `Higiri_<版>_windows_x64.zip`。展開した `Higiri.exe` を実行します。
- macOS(Apple Silicon): [macOS build](../../actions/workflows/macos.yml) の `Higiri_<版>_macos_arm64.zip`。展開した `Higiri.app` を「アプリケーション」フォルダに移して開きます。

どちらも有料の署名を受けていないため、初めて開くときに確認が出ます。

- Windows: 「Windows によって PC が保護されました」と出たら、「詳細情報」→「実行」を選んでください。
- macOS: 開けないときは、「システム設定」→「プライバシーとセキュリティ」で「このまま開く」を選んでください。

データはアプリとは別の場所(Windows は `%APPDATA%\com.redjet.higiri`、macOS は `~/Library/Application Support/com.redjet.higiri`)に保存されます。新しい版に入れ替えても、データは残ります。

## 開発

Node.js 24、Rust(stable)と、[Tauri の前提ツール](https://v2.tauri.app/start/prerequisites/)が必要です。

```bash
npm install
npm run dev      # ウィンドウを開いて開発する
npm run check    # 型チェック、テスト、lint
npm run build    # 配布用の ZIP を release/ に作る
```

要件・設計・画面モックは `docs/` にあります。

## ライセンス

MIT ライセンスです。ただし、Aizome デザインシステム(`docs/aizome/` と `src/components/aizome/`)は対象外です。詳しくは [LICENSE](LICENSE) を見てください。
