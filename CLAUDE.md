# TODO管理デスクトップアプリ

プロジェクト単位でTODOを管理し、日限が危ないものに起動直後に気づける、ローカル完結のデスクトップアプリ。Tauri 2 で作る。利用者は1人・1台、表示は日本語のみ、主対象は Windows(macOS でもビルドできる構成にする)。

要件・設計・画面モックは確定済みで `docs/` にある。このファイルは要点の抜き出しで、食い違ったら `docs/requirements-design.md` を正とする。

## アプリ名と識別子

- アプリ名は「Higiri」。`tauri.conf.json` の `productName`、ウィンドウのタイトル、サイドナビの見出しに使う。サイドナビの見出しは、モックで「TODO管理」となっている箇所をアプリ名に置き換える。
- 識別子は `com.redjet.higiri`(`tauri.conf.json` の `identifier`)。アプリのデータフォルダもこの識別子で決まる。

## 資料

| 場所 | 中身 |
| --- | --- |
| `docs/README.md` | 資料の一覧、優先順位、写しを作るときに分かっている注意点 |
| `docs/requirements-design.md` | 要件41件、非機能要件、リリース段階、アーキテクチャ、DDL、主要ロジック、画面構成、コマンド一覧、未決事項 |
| `docs/implementation-plan.md` | MVPまでのフェーズ分けと、各フェーズの完了条件 |
| `docs/ui-mock/` | 6画面のモック。見た目は `screenshots/`、正確な値と文言は `.dc.html` |
| `docs/aizome/` | デザインシステム Aizome の方針、トークン、部品のCSS・型定義・参考実装 |

## 作る範囲

MVP(v1.0)だけを作る。設計書の「段階」が v1.1 と将来のものは作らない。ただしデータ構造の備え(`todos.parent_id`、`reports.kind`)は設計書どおり最初から入れる。

設計書の「未決事項」6点は、書いてある仮定のまま進める。仮定を変える判断は利用者がする。

## 構成(変えない決めごと)

```
画面(React + TypeScript + Vite)  表示と入力だけ。SQLもファイル操作も持たない
  ↓ invoke(tauri-specta で型を共有)
commands/   入力の検証とトランザクションの境界だけの薄い層
  ├ domain/ 純粋関数。日限注意の判定、ステータス遷移、日報の組み立て
  └ repo/   SQL(sqlx)。マイグレーション、バックアップ
SQLite(アプリのデータフォルダ内の todo.db)
```

- 判定と集計は Rust 側に置く。画面側で日限注意の区分や残り日数を計算し直さない。
- `domain/` の関数は現在時刻を自分で取らない。今日の日付(と現在時刻)は引数で受け取る。時計を読むのは `commands/` だけ。
- 日限注意の判定は `classify(todo, today, soon_days)` の1か所。ダッシュボード、リマインド、一覧のバッジ、日報のすべてがこれを使う。
- ステータス変更は必ず `change_status` を通す。実績日の自動入力と `todo_events` への記録を1トランザクションで行う。`update_todo` でステータスを直接書き換えない。
- 画面から呼べるのは設計書のコマンド一覧にあるものだけ。足すときは理由を添えて相談する。
- 画面側からのコマンド呼び出しは、tauri-specta が生成する `src/lib/bindings.ts` 経由だけにする。画面のコードに `invoke` を直接書かない。
- ステータスの遷移: 開いている4状態(未着手・作業中・待ち・保留)の間は自由。完了と中止にはその4状態から入れる。完了から戻せるのは作業中(再開)だけ、中止から戻せるのは未着手だけ。完了と中止の間は行き来できない。待ちから抜けると待ちの3項目(`waiting_on`、`waiting_since`、`follow_up_date`)を空にする。
- 公式の sql プラグインは使わない(画面側からSQLを発行する方式のため)。
- ファイルへの書き出し(日報の .md、バックアップ)は Rust 側で行う。保存先ダイアログも Rust 側で開き、画面から任意のパスを渡して書かせない。

## データの決めごと

- テーブル定義は設計書のDDLをそのまま `src-tauri/migrations/0001_init.sql` にする。列や制約を変えたいときは先に相談する。
- 日付は `YYYY-MM-DD` の文字列(PCのローカル日付)、日時は UTC の ISO 8601 文字列。
- 接続のたびに外部キー制約を有効にし、WALモードにする。
- 適用済みのマイグレーションファイルは書き換えない。変更は新しいファイルを足す。
- DBへの接続なしで `cargo build` が通る構成にする(`query!` 系マクロを使うなら `cargo sqlx prepare` の成果物をコミットする)。
- バックアップは `VACUUM INTO` で作る。WALモードのDBファイルを単純コピーしない。

## 画面の決めごと

- 見た目は `docs/ui-mock/` に合わせる。部品とトークンは Aizome を使い、色・余白・角丸を直値で書かない(`var(--…)` を使う)。
- Aizome の14部品は `docs/aizome/components/bundle.js` を参考に、TypeScript の React コンポーネントとして `src/components/aizome/` に移植する。クラス名、DOM構造、aria属性は変えない。スタイルは `bundle.css` をそのまま使う。
- React は 19 を使う。移植のとき、`index.d.ts` の `JSX.IntrinsicElements` は `React.JSX.IntrinsicElements` に直し、`createPortal` は `react-dom` から import する。
- `bundle.css` の先頭にある Google Fonts の `@import` は外し、IBM Plex Sans JP と IBM Plex Mono をアプリに同梱する。このアプリは外部と通信しない。
- Aizome にない部品(ガント、詳細パネル、Markdownエディタ、タイムライン)は同じトークンで作る。
- 色だけで状態を伝えない。区分名やステータス名を必ず文字で添える。
- 文言は `docs/aizome/README.md` の「文章の基本」に従う(です・ます調、ボタンは動詞、「!」と絵文字は使わない)。モックにある文言はそのまま使う。
- Markdown の表示は HTML を無害化し、リンクは OS の既定ブラウザで開く。

## 品質の決めごと

- `domain/` は単体テストを書く。日付の境界(当日、N日後ちょうど、月またぎ・年またぎ)と、遷移表の全行を押さえる。
- `repo/` はメモリ上の SQLite にマイグレーションを当ててテストする。
- 画面側は、ガントの座標計算など計算を含む部分を Vitest でテストする。
- ライブラリのバージョンは記憶に頼らず、導入時点の最新安定版と互換性を確認する。tauri-specta が入れた Tauri の版で動かない場合は、手書きの型定義に切り替えて報告する。
- tauri-specta は Tauri 2 向けの安定版がないため、RC 版を使う。tauri-specta、specta、specta-typescript は互換性のある組を `=` で固定する。入れるときは、まずコマンド1つで型の生成と呼び出しを確かめてから全体に広げる。
- Tauri の権限(`capabilities/default.json`)と CSP は、使うものだけに絞る。

## 進め方

- `docs/implementation-plan.md` のフェーズ順に進め、フェーズの終わりにテスト結果と、利用者に画面で確認してほしい操作の一覧を報告して止まる。
- 設計書と違う実装にしたいとき、設計書に書いていないことを決める必要があるときは、理由と選択肢を添えて先に相談する。
- 利用者への報告とUIの文言は日本語。コード内の識別子は英語。

## コマンド

パッケージ管理は npm。初回は `npm install`。

| 用途 | コマンド |
| --- | --- |
| 開発(ウィンドウを開く) | `npm run dev`(`tauri dev`) |
| 画面だけをブラウザで開く(Tauri の API は動かない) | `npm run dev:web` |
| 画面のテスト(Vitest) | `npm test` |
| Rust のテスト | `npm run test:rust`(`cargo test --manifest-path src-tauri/Cargo.toml`) |
| 型チェック | `npm run typecheck` |
| Rust の lint(警告はエラー扱い) | `npm run lint:rust` |
| まとめて確認(型チェック、Vitest、cargo test、clippy) | `npm run check` |
| 配布物のビルド | `npm run build` |
