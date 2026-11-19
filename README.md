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

Windows 版のインストーラーは、[Actions](../../actions/workflows/windows.yml) の「Windows build」の実行結果(higiri-windows)からダウンロードできます。ダウンロードには GitHub へのログインが必要です。

## 開発

Node.js 24、Rust(stable)と、[Tauri の前提ツール](https://v2.tauri.app/start/prerequisites/)が必要です。

```bash
npm install
npm run dev      # ウィンドウを開いて開発する
npm run check    # 型チェック、テスト、lint
npm run build    # 配布物を作る
```

要件・設計・画面モックは `docs/` にあります。

## ライセンス

MIT ライセンスです。ただし、Aizome デザインシステム(`docs/aizome/` と `src/components/aizome/`)は対象外です。詳しくは [LICENSE](LICENSE) を見てください。
