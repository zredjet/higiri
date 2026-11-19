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
  - `bindings.ts` は手で編集しない。`cargo test`(`bindings_are_up_to_date`)と、開発ビルドの起動時に作り直される。古いまま残っていると `cargo test` が失敗する。
  - 型のフィールド名は Rust と同じ snake_case のまま(`project_id` など)。コマンドの引数名だけは Tauri の規則で camelCase になる。
  - i64 は TS の `number` として出す(`dangerously_cast_bigints_to_number`)。ID と件数は 2^53 を超えない。
- コマンドを足すときは、`commands::specta_builder()`、`src-tauri/build.rs` の `APP_COMMANDS`、`capabilities/default.json` の `allow-*` の3か所を揃える。capability とバインディングのずれはテストで検出する。
- コマンドは「時計を読んで内側の関数を呼ぶだけの `#[tauri::command]`」と「`&SqlitePool` と `today` / `now` を受け取る内側の関数」に分け、テストは内側の関数をメモリ上の DB で呼ぶ。
- 画面に返すエラーは `AppError { kind, message }` の1種類。`message` は利用者向けの日本語で「何が起きたか+どうすればよいか」。DB の詳細は画面に出さない。
- ステータスの遷移: 開いている4状態(未着手・作業中・待ち・保留)の間は自由。完了と中止にはその4状態から入れる。完了から戻せるのは作業中(再開)だけ、中止から戻せるのは未着手だけ。完了と中止の間は行き来できない。待ちから抜けると待ちの3項目(`waiting_on`、`waiting_since`、`follow_up_date`)を空にする。
- 公式の sql プラグインは使わない(画面側からSQLを発行する方式のため)。
- ファイルへの書き出し(日報の .md、バックアップ)は Rust 側で行う。保存先ダイアログも Rust 側で開き、画面から任意のパスを渡して書かせない。

## データの決めごと

- テーブル定義は設計書のDDLをそのまま `src-tauri/migrations/0001_init.sql` にする。列や制約を変えたいときは先に相談する。
- 日付は `YYYY-MM-DD` の文字列(PCのローカル日付)、日時は UTC の ISO 8601 文字列。日時は `2026-11-19T01:30:00.000Z` の形(ミリ秒つき、末尾 `Z`)で保存し、文字列の大小が時刻の前後と一致するようにする。
- `todo_events` の値の形:
  - `status` の行の `old_value` / `new_value` は、ステータス名の文字列(`todo`、`doing`、`waiting`、`hold`、`done`、`cancelled`)。JSON にしない。
  - `due_date` と `follow_up` の行は `YYYY-MM-DD`。空にしたときは NULL。
  - `planned` と `actual` の行は `{"start": 日付または null, "end": 日付または null}` の JSON。
  - `created` の行は、`old_value` が NULL、`new_value` が作成時のステータス名。
  - 1回のステータス変更は1行で記録する。自動で入れた日付や空に戻した項目は、その行の `note` の `auto` に入れる。
- `todo_events.note` は、すべての行で JSON のオブジェクトにする。補足がなければ `{}`。使うキーは次の表のものだけ。

  | キー | 値 | 入る行 | 中身 |
  | --- | --- | --- | --- |
  | `waiting_on` | 文字列 | 待ちにした `status` の行 | 入力された、待っている相手 |
  | `follow_up_date` | `YYYY-MM-DD` | 待ちにした `status` の行 | 入力された確認日 |
  | `reason` | 文字列 | 中止にした `status` の行 | 入力された中止の理由 |
  | `auto` | オブジェクト | `status` の行すべて(変えた列がなければ `{}`) | 遷移で自動的に変えた `todos` の列。キーは列名(`actual_start`、`actual_end`、`waiting_since`、`closed_at`、`waiting_on`、`follow_up_date`、`close_reason`)、値は変えた後の値(日付は `YYYY-MM-DD`、`closed_at` は上の日時の形)。空に戻した列は `null` |

  - 入力をそのまま入れる列(`waiting_on`、`follow_up_date`、`close_reason`)は、空に戻したときだけ `auto` に入る。入力した値は `waiting_on` / `follow_up_date` / `reason` のキーに入る。
  - 例: 作業中から待ちにした行は `{"waiting_on":"認証担当の回答","follow_up_date":"2026-11-20","auto":{"waiting_since":"2026-11-19"}}`。完了から再開した行は `{"auto":{"actual_end":null,"closed_at":null}}`。
- `settings` は1キー1行で、値は JSON。キーは `reminder_frequency`(`"every_launch"` / `"once_per_day"`)、`soon_days`(数値)、`follow_up_days`(数値)、`last_reminded_on`(`"YYYY-MM-DD"` か `null`)、`theme`(`"light"` / `"dark"` / `"system"`)。読めない値は既定値として扱う。
- 接続のたびに外部キー制約を有効にし、WALモードにする。
- 適用済みのマイグレーションファイルは書き換えない。変更は新しいファイルを足す。
- DBへの接続なしで `cargo build` が通る構成にする(`query!` 系マクロを使うなら `cargo sqlx prepare` の成果物をコミットする)。
- バックアップは `VACUUM INTO` で作る。WALモードのDBファイルを単純コピーしない。
- バックアップは `src-tauri/src/backup.rs`。データフォルダの `backups/` に、その日最初の起動時に `todo-YYYY-MM-DD.db` を作り、新しい順に7個残す(起動しなかった日は数えない)。復元の前には今のデータを `before-restore-<UTC日時>.db` に退避し、新しい順に3個残す。退避も復元の一覧に「復元する前の状態」として出し、復元を取り消せるようにする。
- 復元は、選んだバックアップの作業用コピーを検査してマイグレーションを当ててから、`ATTACH` して全テーブルの行を1トランザクションで入れ替える(接続プールを開いたまま、再起動なしで済む)。新しい版のアプリで作ったバックアップと、壊れたファイルは復元しない。

## 画面の決めごと

- 見た目は `docs/ui-mock/` に合わせる。部品とトークンは Aizome を使い、色・余白・角丸を直値で書かない(`var(--…)` を使う)。
- Aizome の14部品は `docs/aizome/components/bundle.js` を参考に、TypeScript の React コンポーネントとして `src/components/aizome/` に移植する。クラス名、DOM構造、aria属性は変えない。スタイルは `bundle.css` をそのまま使う。
- React は 19 を使う。移植のとき、`index.d.ts` の `JSX.IntrinsicElements` は `React.JSX.IntrinsicElements` に直し、`createPortal` は `react-dom` から import する。
- 移植済みの Aizome は `src/components/aizome/`。`tokens.css` は `docs/aizome/tokens.css` の写し、`aizome.css` は `bundle.css` から Google Fonts の `@import` だけを外した写し。どちらも書き換えない。アプリ独自のレイアウトは `src/styles/app.css` に置く。
- フォントは `@fontsource/ibm-plex-sans-jp`(400/500/600/700)と `@fontsource/ibm-plex-mono`(400/500)を `src/styles/fonts.ts` で読み込む。`vite.config.ts` のプラグインで .woff を外し、.woff2 だけを同梱する。
- 画面の切り替えはルーティングのライブラリを使わず、`src/app/routes.ts` の `Route` を状態で持つ。サイドナビの id と画面は `routes.ts` で対応させる。
- データの取得と更新は TanStack Query。フックは `src/lib/queries.ts` にまとめ、更新したら影響する一覧(プロジェクトと TODO は両方)を取り直す。
- 入力の検証は Rust 側で行い、返ってきた `AppError.message` を表示する(`src/lib/errors.ts` の `errorMessage`)。フォームには `noValidate` を付け、ブラウザ標準の英語の吹き出しを出さない。
- 一覧の絞り込み・並べ替え(`domain/listing.rs`)、ステータスの選択肢(`TodoItem.next_statuses`)、タイムラインの文(`domain/timeline.rs`)は Rust 側で作る。画面は受け取ったものを並べるだけ。
- 日付の入力は `src/components/DateField.tsx` を使う(空を null にし、macOS の WebKit が空欄に今日の日付を出すのを薄く表示する)。
- Enter で確定する入力は `src/lib/keys.ts` の `isComposing` で日本語変換中の Enter を除く。
- 詳細パネルの項目は `useTodoDraft` で入力が止まって1秒後に `update_todo` で保存する。閉じるときと別のTODOに切り替えるときにも保存する。ステータスを変える前にも保存してから `change_status` を呼ぶ。
- Markdown の編集は CodeMirror 6(`src/components/markdown/MarkdownEditor.tsx`)、表示は react-markdown + remark-gfm + rehype-sanitize(`MarkdownView.tsx`)。表示のリンクはアプリ内で開かず、http/https だけを opener プラグインの `openUrl` で既定のブラウザに渡す。
- CSP: CodeMirror が実行時に `<style>` を差し込むため、`style-src` だけ `'unsafe-inline'` を許し、`dangerousDisableAssetCspModification: ["style-src"]` で Tauri の nonce 付与を止めている。`script-src` は厳しいまま。opener の権限は `capabilities/default.json` で http/https に絞る。
- ダッシュボードの集計(日限注意の一覧と件数、今日の作業、待ち、進捗)と起動時リマインドの判定は `domain/dashboard.rs`。サイドナビのダッシュボードのバッジは `get_dashboard` の `attention_total` を使う。
- 起動時リマインドは起動時に1回 `get_startup_reminder` を呼び、「確認しました」のボタンでだけ閉じられ(画面外のクリック、Esc、× では閉じない)、閉じたら `ack_reminder`。ウィンドウが前面に来たときに日付が変わっていれば、すべての一覧を取り直して判定をやり直す(`src/app/useStartupReminder.ts`)。
- ガント: 行データ(予定・実績・待ちの区間、区分)は `get_gantt`(`domain/gantt.rs`)。待ちの区間は変更履歴から「待ちに入った日〜抜けた前日」(待ち中は今日まで)。日付から座標への変換、目盛り、表示期間、見えている行だけを描く処理は `src/features/gantt/geometry.ts`(Vitest でモックの座標と照合)。表示する日数は画面の幅に収まる分で、初期表示は今日が左から3分の1。日限のひし形はモックどおり超過(朱)と本日(琥珀)だけ色を付ける。
- TODO 一覧は200件を超えると、画面の近くの行だけを描く(`src/features/todos/virtualRows.ts` と `useVirtualRows.ts`)。行の高さは描いたあとに測り、それまでは見積もりを使う。Aizome の Table には見えている行だけを渡し、上下の行の分の高さは外側の要素の CSS(`tbody::before` / `::after`)で取る。Table の DOM は変えない。
- キーボードショートカット: Ctrl+N(新規TODO)と Ctrl+1〜4(ダッシュボード、TODO、ガント、日報)は `src/app/shortcuts.ts`。macOS は Command。ダイアログやメニューが開いているときは効かない。「TODOを追加」のダイアログは App が持ち、画面のボタンとショートカットで同じものを開く。Esc、Ctrl+Enter、Ctrl+E は詳細パネルとエディタ側にある。
- 動作ログは log プラグイン(OS のログフォルダの `higiri.log`、開発ビルドは `higiri-dev.log`。1 MB で切り替えて5個残す)。起動、バックアップ、復元、想定外のエラーだけを書き、TODO の中身は書かない。画面側は `src/lib/log.ts` の `logUnexpected` を使う(`errorMessage` が AppError 以外のエラーを渡す)。
- 多重起動の防止(single-instance)は配布版だけで有効にする。開発ビルドは別のDBを使うので、インストールした版と同時に動かせるようにしている。ウィンドウの位置・大きさ・最大化は window-state で次回に戻す。
- 権限(`capabilities/default.json`)は、アプリのコマンドと、opener の http/https、clipboard-manager の `allow-write-text`、log の `allow-log` だけ。`core:default` は付けない(画面は Tauri のウィンドウ・イベント・パスの API を使わない)。
- 日報: 節の組み立ては `domain/report.rs`。過去の日は、本日の作業と本日完了・中止をその日の記録から、待ち・明日の予定・日限注意を今の TODO の状態から対象日を今日として作る(待ちは、対象日より後に待ちに入ったものを除く)。中身のない節は「- なし」。所感・連絡事項は `## 所感・連絡事項` の見出しより後ろで、再生成しても残る。設計書の出力例との完全一致を `commands/reports.rs` のテスト(`tests/fixtures/daily_report_2026-11-19.md`)で確かめている。
- 日報の画面(`src/features/report/`): 日付を開くと保存済みの日報を、なければ生成した日報を出す(開いただけでは保存しない)。編集が止まって1秒後と、再生成・コピー・.md で保存のときに `save_report` で保存する。コピーは clipboard-manager の `writeText`(権限は `clipboard-manager:allow-write-text` だけ)。.md の保存先ダイアログは dialog プラグインを Rust 側で開くので、画面側に dialog の権限は付けない。
- プロジェクトの作成はサイドナビの「プロジェクトを追加」、編集・アーカイブ・削除はプロジェクトのページの「プロジェクトを編集」から行う。アーカイブしたプロジェクトは設定画面のカードで一覧し、元に戻す・削除する。削除は確認ダイアログを挟む。
- `bundle.css` の先頭にある Google Fonts の `@import` は外し、IBM Plex Sans JP と IBM Plex Mono をアプリに同梱する。このアプリは外部と通信しない。
- Aizome にない部品(ガント、詳細パネル、Markdownエディタ、タイムライン)は同じトークンで作る。
- Aizome のダークテーマでは `--bg-subtle` と `--surface` が同じ色になる。`surface` の上に敷く網掛け(ガントの土日・プロジェクト行・見出し、進捗バーの地、Markdown のコードと表の見出し)は `src/styles/app.css` の `--surface-shade` を使う(ライトは `--bg-subtle`、ダークは `--sumi-850`)。
- 色だけで状態を伝えない。区分名やステータス名を必ず文字で添える。
- 文言は `docs/aizome/README.md` の「文章の基本」に従う(です・ます調、ボタンは動詞、「!」と絵文字は使わない)。モックにある文言はそのまま使う。
- Markdown の表示は HTML を無害化し、リンクは OS の既定ブラウザで開く。

## 品質の決めごと

- `domain/` は単体テストを書く。日付の境界(当日、N日後ちょうど、月またぎ・年またぎ)と、遷移表の全行を押さえる。
- `repo/` はメモリ上の SQLite にマイグレーションを当ててテストする。
- 画面側は、ガントの座標計算など計算を含む部分を Vitest でテストする。
- ライブラリのバージョンは記憶に頼らず、導入時点の最新安定版と互換性を確認する。tauri-specta が入れた Tauri の版で動かない場合は、手書きの型定義に切り替えて報告する。
- tauri-specta は Tauri 2 向けの安定版がないため、RC 版を使う。tauri-specta、specta、specta-typescript は互換性のある組を `=` で固定する(今は `tauri-specta =2.0.0-rc.29`、`specta =2.0.0-rc.28`、`specta-typescript =0.0.15`)。上げるときは3つを同時に上げ、まずコマンド1つで型の生成と呼び出しを確かめてから全体に広げる。
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
| 開発用のサンプルデータを入れて開く(DBが空のときだけ入る) | `npm run dev:sample` |
| 画面だけをブラウザで開く(Tauri の API は動かない) | `npm run dev:web` |
| 画面のテスト(Vitest) | `npm test` |
| Rust のテスト | `npm run test:rust`(`cargo test --manifest-path src-tauri/Cargo.toml`) |
| 型チェック | `npm run typecheck` |
| Rust の lint(警告はエラー扱い) | `npm run lint:rust` |
| まとめて確認(型チェック、Vitest、cargo test、clippy) | `npm run check` |
| 性能の確認(TODO 5,000件・作業ログ 20,000件でコマンドの時間を測る。最適化ビルド) | `npm run test:perf` |
| 配布物のビルド(実行ファイルの ZIP を `release/` に作る) | `npm run build` |

- 配布物は実行ファイルの ZIP で、インストーラーと .dmg は作らない。`npm run build`(`scripts/package.mjs`)が、Windows では `Higiri.exe`、macOS では ad-hoc 署名した `Higiri.app` を ZIP にして `release/` に置く(macOS は `ditto` で固めて、署名と実行権限を保つ)。GitHub Actions の `.github/workflows/windows.yml` と `macos.yml` も同じスクリプトで作り、ZIP を包み直さずに(`archive: false`)成果物に上げる。Actions では Vitest と cargo test を通してから作る(型チェックは build:web の中で行い、clippy は手元の `npm run check` だけで動かす)。配布版の設定(`[profile.release]`)は、ビルド時間を優先して LTO を使わない。Actions のページから手動で動かすか、`v` で始まるタグを push する。改行は `.gitattributes` で LF に揃えている(生成物とフィクスチャをバイト単位で比べるテストがあるため)。
- 開発ビルドの DB は、配布版と混ざらないように、データフォルダの下の `dev/todo.db` を使う(macOS は `~/Library/Application Support/com.redjet.higiri/dev/`、Windows は `%APPDATA%\com.redjet.higiri\dev\`)。サンプルデータを入れ直すときは、アプリを閉じてこの `dev` フォルダを消してから `npm run dev:sample` で起動する。
- サンプルデータはモックの3プロジェクト・11件で、日付は「モックの今日(2026-11-19)」が実際の今日になるようにずらして入れる。ステータスの変化は `change_status` と同じ遷移関数で再現するので、変更履歴も残る。
