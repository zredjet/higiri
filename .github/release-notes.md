プロジェクト単位で TODO を管理し、日限が近いものに起動直後に気づけるデスクトップアプリです。データはこの PC の中だけに保存し、外部とは通信しません。

## ダウンロード

下の「Assets」から、使う OS の ZIP をダウンロードして展開します。インストーラーはありません。

- Windows(64ビット): `Higiri_<版>_windows_x64.zip` を展開し、`Higiri.exe` を実行します。
- macOS(Apple Silicon): `Higiri_<版>_macos_arm64.zip` を展開し、`Higiri.app` を「アプリケーション」フォルダに移して開きます。

## 初めて開くとき

どちらも有料の署名を受けていないため、初めて開くときに確認が出ます。

- Windows: 「Windows によって PC が保護されました」と出たら、「詳細情報」→「実行」を選んでください。
- macOS: 開けないときは、「システム設定」→「プライバシーとセキュリティ」で「このまま開く」を選んでください。

## データの保存場所

データはアプリとは別の場所に保存されます。新しい版に入れ替えても、データは残ります。

- Windows: `%APPDATA%\com.redjet.higiri`
- macOS: `~/Library/Application Support/com.redjet.higiri`
