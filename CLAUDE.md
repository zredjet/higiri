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
| `docs/requirements-design.md` | 要件41件、非機能要件、リリース段階、アーキテクチャ、DDL、主要ロジック、画面構成、コマンド一覧、未決事項。v1.1 の詳細(「v1.1の詳細」の節) |
| `docs/implementation-plan.md` | MVPまでのフェーズ分けと、各フェーズの完了条件 |
| `docs/implementation-plan-v1.1.md` | v1.1 のフェーズ分け(フェーズ10〜16)と、各フェーズの完了条件。進め方(オーケストレーター方式) |
| `docs/ui-mock/` | 6画面のモック。見た目は `screenshots/`、正確な値と文言は `.dc.html` |
| `docs/aizome/` | デザインシステム Aizome の方針、トークン、部品のCSS・型定義・参考実装 |

## 作る範囲

v1.1 までを作る。設計書の「段階」が将来(v2)のものは作らない。データ構造の備え(`todos.parent_id`、`reports.kind`)は設計書どおり入れたままにする。

MVP(v1.0)は完成している。v1.1 は `docs/implementation-plan-v1.1.md` のフェーズ順に作り、仕様は設計書の「v1.1の詳細」に従う。v1.1 の決めごとは下の「v1.1 の決めごと」にまとめてあり、まだ作っていないものを含む。

設計書の「未決事項」6点のうち、残り日数の数え方は v1.1 で利用者が決めた(設定で暦日と営業日を切り替え、初期値は暦日)。残る5点は、書いてある仮定のまま進める。仮定を変える判断は利用者がする。

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
- `domain/` の関数は現在時刻を自分で取らない。今日の日付(と現在時刻)は引数で受け取る。時計を読むのは `commands/` と常駐のタイマー(`src-tauri/src/notifier.rs`)だけで、どちらも `commands::clock` を使う。通知の時刻はローカルの時刻なので、UTC からの変換には `clock::zone()`(PC のタイムゾーン)を渡す。
- 日限注意の判定は `classify(todo, today, &rules)` の1か所。ダッシュボード、リマインド、一覧のバッジ、日報のすべてがこれを使う。`rules`(`domain::urgency::Rules`)は `soon_days`、`follow_up_days`、`WorkCalendar` を持ち、`Rules::from_settings` で設定から作る。日数(`assess`)と確認日の初期値も `Rules` で数える。
- ステータス変更は必ず `change_status` を通す。実績日の自動入力と `todo_events` への記録を1トランザクションで行う。`update_todo` でステータスを直接書き換えない。
- 画面から呼べるのは設計書のコマンド一覧にあるものだけ。足すときは理由を添えて相談する。
- 画面側からのコマンド呼び出しは、tauri-specta が生成する `src/lib/bindings.ts` 経由だけにする。画面のコードに `invoke` を直接書かない。
  - `bindings.ts` は手で編集しない。`cargo test`(`bindings_are_up_to_date`)と、開発ビルドの起動時に作り直される。古いまま残っていると `cargo test` が失敗する。
  - 型のフィールド名は Rust と同じ snake_case のまま(`project_id` など)。コマンドの引数名だけは Tauri の規則で camelCase になる。
  - i64 は TS の `number` として出す(`dangerously_cast_bigints_to_number`)。ID と件数は 2^53 を超えない。
- コマンドを足すときは、`commands::specta_builder()`、`src-tauri/build.rs` の `APP_COMMANDS`、`capabilities/default.json` の `allow-*` の3か所を揃える。capability とバインディングのずれはテストで検出する。
- コマンドは「時計を読んで内側の関数を呼ぶだけの `#[tauri::command]`」と「`&SqlitePool` と `today` / `now` を受け取る内側の関数」に分け、テストは内側の関数をメモリ上の DB で呼ぶ。v1.1 で足す OS 側への反映(自動起動の登録、通知のタイマーの計算し直し、×ボタンの動き)は `#[tauri::command]` の層で行い、内側の関数は DB だけを扱う。
- 画面に返すエラーは `AppError { kind, message }` の1種類。`message` は利用者向けの日本語で「何が起きたか+どうすればよいか」。DB の詳細は画面に出さない。
- ステータスの遷移: 開いている4状態(未着手・作業中・待ち・保留)の間は自由。完了と中止にはその4状態から入れる。完了から戻せるのは作業中(再開)だけ、中止から戻せるのは未着手だけ。完了と中止の間は行き来できない。待ちから抜けると待ちの3項目(`waiting_on`、`waiting_since`、`follow_up_date`)を空にする。
- 公式の sql プラグインは使わない(画面側からSQLを発行する方式のため)。
- ファイルへの書き出し(日報の .md、バックアップ、v1.1 の JSON)と、v1.1 の JSON の読み込みは Rust 側で行う。保存先やファイルを選ぶダイアログも Rust 側で開き、画面から任意のパスを渡して読み書きさせない。

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
- `settings` は1キー1行で、値は JSON。キーは `reminder_frequency`(`"every_launch"` / `"once_per_day"`)、`soon_days`(数値)、`follow_up_days`(数値)、`last_reminded_on`(`"YYYY-MM-DD"` か `null`)、`theme`(`"light"` / `"dark"` / `"system"`)。読めない値は既定値として扱う。v1.1 で足すキーは「v1.1 の決めごと」にある。
- 接続のたびに外部キー制約を有効にし、WALモードにする。
- 適用済みのマイグレーションファイルは書き換えない。変更は新しいファイルを足す。
- DBへの接続なしで `cargo build` が通る構成にする(`query!` 系マクロを使うなら `cargo sqlx prepare` の成果物をコミットする)。
- バックアップは `VACUUM INTO` で作る。WALモードのDBファイルを単純コピーしない。
- バックアップは `src-tauri/src/backup.rs`。データフォルダの `backups/` に、その日最初の起動時に `todo-YYYY-MM-DD.db` を作り、新しい順に7個残す(起動しなかった日は数えない)。常駐中に日付が変わったときも、常駐のタイマーが `backup::run_daily` でその日のバックアップを作る(`notifier::backup_if_new_day`。1日1個、7個残すのは同じ)。復元の前には今のデータを `before-restore-<UTC日時>.db` に退避し、新しい順に3個残す。退避も復元の一覧に「復元する前の状態」として出し、復元を取り消せるようにする。
- 復元は、選んだバックアップの作業用コピーを検査してマイグレーションを当ててから、`ATTACH` して全テーブルの行を1トランザクションで入れ替える(接続プールを開いたまま、再起動なしで済む)。新しい版のアプリで作ったバックアップと、壊れたファイルは復元しない。`autostart`、`last_notified_at`、`tray_hint_shown`、`last_reminded_on`(`repo::settings::KEPT_ON_REPLACE`)は、入れ替えの同じトランザクションの中で今の値に戻す(今ない値は、入れ替えたあともない)。行の入れ替えは `backup::copy_rows` で、フェーズ15の JSON の取り込みも同じ関数を使う。復元のあとは `restore_backup` の `#[tauri::command]` の層で `resident::apply` を呼び、×ボタンの動きとタイマーの待ち時間を復元した設定に合わせる。

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
- 多重起動の防止(single-instance)は配布版だけで有効にする。開発ビルドは別のDBを使うので、インストールした版と同時に動かせるようにしている。2つ目の起動は既存のウィンドウを前面に出す(`--autostart` つきなら出さない)。ウィンドウの位置・大きさ・最大化は window-state で次回に戻す。
- 権限(`capabilities/default.json`)は、アプリのコマンドと、opener の http/https、clipboard-manager の `allow-write-text`、log の `allow-log` だけ。`core:default` は付けない(画面は Tauri のウィンドウ・イベント・パスの API を使わない)。notification と autostart のプラグインは Rust からだけ使い、画面側には権限を付けない。notification プラグインは画面に `window.Notification` の差し替えを入れ、Windows 以外では読み込み時に `is_permission_granted` を呼ぶが、権限がないので断られる(画面はこれを使わない)。
- 日報: 節の組み立ては `domain/report.rs`。過去の日は、本日の作業と本日完了・中止をその日の記録から、待ち・明日の予定・日限注意を今の TODO の状態から対象日を今日として作る(待ちは、対象日より後に待ちに入ったものを除く)。中身のない節は「- なし」。所感・連絡事項は `## 所感・連絡事項` の見出しより後ろで、再生成しても残る。設計書の出力例との完全一致を `commands/reports.rs` のテスト(`tests/fixtures/daily_report_2026-11-19.md`)で確かめている。
- 日報の画面(`src/features/report/`): 日付を開くと保存済みの日報を、なければ生成した日報を出す(開いただけでは保存しない)。編集が止まって1秒後と、再生成・コピー・.md で保存のときに `save_report` で保存する。コピーは clipboard-manager の `writeText`(権限は `clipboard-manager:allow-write-text` だけ)。.md の保存先ダイアログは dialog プラグインを Rust 側で開くので、画面側に dialog の権限は付けない。
- プロジェクトの作成はサイドナビの「プロジェクトを追加」、編集・アーカイブ・削除はプロジェクトのページの「プロジェクトを編集」から行う。アーカイブしたプロジェクトは設定画面のカードで一覧し、元に戻す・削除する。削除は確認ダイアログを挟む。
- `bundle.css` の先頭にある Google Fonts の `@import` は外し、IBM Plex Sans JP と IBM Plex Mono をアプリに同梱する。このアプリは外部と通信しない。
- Aizome にない部品(ガント、詳細パネル、Markdownエディタ、タイムライン)は同じトークンで作る。
- Aizome のダークテーマでは `--bg-subtle` と `--surface` が同じ色になる。`surface` の上に敷く網掛け(ガントの土日・プロジェクト行・見出し、進捗バーの地、Markdown のコードと表の見出し)は `src/styles/app.css` の `--surface-shade` を使う(ライトは `--bg-subtle`、ダークは `--sumi-850`)。
- 色だけで状態を伝えない。区分名やステータス名を必ず文字で添える。
- 文言は `docs/aizome/README.md` の「文章の基本」に従う(です・ます調、ボタンは動詞、「!」と絵文字は使わない)。モックにある文言はそのまま使う。
- Markdown の表示は HTML を無害化し、リンクは OS の既定ブラウザで開く。

## v1.1 の決めごと

v1.1 は `docs/implementation-plan-v1.1.md` のフェーズ10〜16で作る。この節には、まだ作っていないものも含む。ファイルの場所など実装で決まることは、作ったフェーズでこのファイルの各節に足す。仕様の詳細は設計書の「v1.1の詳細」にある。

- テーブル定義は変えない(マイグレーションを足さない)。新しい値は `settings` のキーに入れ、検索は既存の列を `LIKE` で引く(FTS5 は使わない)。
- 足すコマンドは次の5つだけ(27個から32個になる。フェーズ10の `test_notification` とフェーズ11の `list_holidays` で今は29個)。上の「構成」の3か所をそろえ、件数のテスト(`commands/mod.rs`)も直す。

  | コマンド | フェーズ | 用途 |
  | --- | --- | --- |
  | `test_notification` | 10 | 設定画面の「通知を試す」 |
  | `list_holidays` | 11 | 期間(3年まで)の休日(祝日と独自の休日、名前つき)を返す。ガントの網掛けと設定画面に使う |
  | `search_todos` | 12 | 全文検索。当たった TODO と、当たった場所・前後の文を、最大100件と全体の件数で返す |
  | `export_data` | 15 | 全データを JSON で書き出す。保存先のダイアログは Rust 側で開く |
  | `import_data` | 15 | JSON を取り込む。ファイルを選ぶダイアログは Rust 側で開き、検査してから全データを置き換える |

- 自動起動、×ボタンの動き、通知の時刻、日数の数え方、日報テンプレートは `update_settings` で扱う。OS 側への反映は `#[tauri::command]` の層で行う(上の「構成」)。復元と取り込みで settings が入れ替わったときも同じく反映する。
- `settings` に足すキー(読めない値は既定値として扱う)

  | キー | 値 | 既定値 | フェーズ |
  | --- | --- | --- | --- |
  | `notify_enabled` | 真偽 | `true` | 10 |
  | `notify_times` | `["HH:MM", …]`(1〜5個、重複なし) | `["09:00", "12:55"]` | 10 |
  | `last_notified_at` | UTC の日時(上の日時の形)か `null` | `null` | 10 |
  | `close_to_tray` | 真偽 | `true` | 10 |
  | `autostart` | 真偽 | `false` | 10 |
  | `tray_hint_shown` | 真偽 | `false` | 10 |
  | `day_count` | `"calendar"` / `"business"` | `"calendar"` | 11 |
  | `custom_holidays` | `[{name, start, end, yearly}]` | `[]` | 11 |
  | `report_template` | 日報テンプレートの構造(設計書の「日報テンプレート」) | 今の日報と同じ形 | 14 |

  - `last_notified_at` と `tray_hint_shown` は、`last_reminded_on` と同じくアプリが書く値で、`update_settings` の入力(`SettingsInput`)には入れない(設定画面の保存で古い値に戻さないため)。
  - 復元と取り込みでは、PC ごとの値の `autostart` と、アプリが書く値の `last_notified_at`、`tray_hint_shown`、`last_reminded_on` を今の値のまま残す。
- 常駐と通知(フェーズ10)
  - notification と autostart のプラグインは Rust からだけ使う。トレイはプラグインではなく Tauri 本体の `tray-icon` の機能で作る。
  - 通知は決まった時刻(`notify_times`)にだけ出す。1分おきの監視はしない。常駐のタイマーは、時計を `commands::clock` で読み、件数をダッシュボードと同じ集計(`domain::dashboard`)で出す。どの時刻を通知するか、次に起きるまでの待ち時間(最長10分)は、`domain/` の純粋関数で決める。
  - 日限注意が0件なら通知しない。その時刻も通知済みとして `last_notified_at` を保存する。
  - `notify_enabled` がオフでも、トレイの案内(初めて×で隠したときの1回)と「通知を試す」は出す。
  - 常駐のタイマーが起きたとき(最長10分おき)に日付が変わっていたら、`backup::run_daily` でその日のバックアップを取る。
  - 自動起動は `--autostart` の引数つきで登録し、そのときはウィンドウを出さずにトレイだけで起動する。single-instance の2つ目の起動も、`--autostart` つきならウィンドウを出さない。自動起動を OS に登録するのは配布版だけ。
  - ×ボタンは、`close_to_tray` がオンなら隠してトレイに残す。トレイを作れなかったときは、隠さずに終了する。
  - 作ったもの(フェーズ10)
    - `domain/notify.rs`: 時刻の読み取りと検査(`normalize_times`。`H:MM` も受け、全角の数字とコロンは半角として読む。保存は `HH:MM` で時刻の順)、`pending_slot`、`next_wake`、通知の文(`message`、`tray_hint`、`test_message`)。
    - `notifier.rs`: 常駐のタイマー(tokio のタスク)。通知に使う設定(`NotifySettings`: `notify_enabled`、`notify_times`、`last_notified_at`)は `Notifier` がメモリに持ち、起きるたびに DB を読まない。DB に触るのは、通知する時刻が来たとき(件数の集計と `last_notified_at` の保存)と、日付が変わってバックアップを取るときだけ。1回分の処理は `tick`(メモリ上の設定を引数で受け取る。メモリ上の DB でテストする)と `backup_if_new_day`。設定の保存と復元のあとは `Notifier::update` で設定を入れ替えて起こす(`tokio::sync::Notify`)。メモリの `last_notified_at` より古い値では上書きしない。通知を出すのは `notifier::show`。
    - `resident.rs`: ウィンドウを出す `show_main`(最小化を戻す → 表示 → 前面)、×ボタン(`on_close_requested`)、トレイの案内、`--autostart` の判定、設定を動いているアプリに反映する `apply`(`close_to_tray` の `AtomicBool` と、`Notifier::update`)。
    - `tray.rs`: トレイ。メニューは「Higiri を開く」「終了」。Windows は左クリックでもウィンドウを開き、macOS はクリックでメニューを開く。
    - `autostart.rs`: 自動起動の登録と解除(`apply`)。プラグインは配布版だけで読み込み、配布版は起動のたびに登録し直す(`sync_at_startup`)。App Translocation の判定は実行ファイルのパスに `AppTranslocation` があるか。
    - `win_toast.rs`(Windows の配布版だけ): `HKCU\Software\Classes\AppUserModelId\com.redjet.higiri` に `DisplayName` と `IconUri`(データフォルダの `toast-icon.png`)を書く。
    - `commands/notify.rs`: `test_notification`。
    - `update_settings` の `#[tauri::command]` は、保存のあとに `resident::apply` と、`autostart` が変わったときの `autostart::apply` を呼ぶ。OS への登録に失敗したら `autostart` を元の値で保存し直してエラーを返す。
    - アプリが書く値は1キーずつ書く(`repo::settings::set_last_reminded_on`、`set_last_notified_at`、`set_tray_hint_shown`)。`update_settings` は利用者が変える値だけを書く(`save_user_settings`)。
    - 通知のクリックでウィンドウを出すのは Windows だけ(利用者が決定)。notification プラグイン(2.8.1)の `on_action` で `tap` を受ける。macOS では `on_action` を登録しない(登録すると、通知センターに残る通知ごとに、待ちのスレッドとメインスレッドのタイマーが動き続けるため)。macOS のクリックはアプリを前面に出すだけで、ウィンドウは Dock かメニューバーから開く。
    - main ウィンドウは `tauri.conf.json` で `visible: false` にし、setup の最後に出す。macOS の Dock のクリック(`RunEvent::Reopen`)でも出す。
    - 設定画面の「通知と常駐」のカードは `SettingsPage.tsx` の `ResidentCard`。開発ビルドかどうかは `import.meta.env.DEV` で見て、自動起動のスイッチを押せなくする。
- 営業日(フェーズ11)
  - 判定は `classify(todo, today, &rules)` の1か所のまま。休日と営業日の計算も `domain/` の純粋関数にする。
  - 暦日の設定(初期値)では、今の判定と表示を何も変えない。既存のテストとフィクスチャはそのまま通す。
  - 待ち日数と確認日からの日数は、営業日の設定でも暦日で数える。
  - 「営業日」の文言と「休日のため前倒し」は、判定の結果(`business_days`、`due_shifted`)を見て出す。画面で日数を数え直さない。確認日の初期値も Rust 側で計算して返す。
  - 祝日は今の祝日法の規則(2020年以降のもの)から計算し、2020〜2099年だけを対象にする(その外の年は土日だけが休み)。2019年以前の改正の歴史は入れない。外部から取り込まない。
  - 作ったもの(フェーズ11)
    - `domain/holidays.rs`: 祝日の計算(`national_holidays(year)`、全年分を1回だけ作る `all()`、期間で引く `between`)。振替休日と国民の休日は、名前を「振替休日」「国民の休日」にする(内閣府の一覧ではどちらも「休日」)。内閣府の一覧(2020〜2027年)との照合のテストは `tests/fixtures/cao_holidays_2020-2027.txt` を使う。
    - `domain/calendar.rs`: `DayCount`(`calendar` / `business`)、`CustomHoliday`(`checked` で検査。名前は前後の空白を除いて1〜30字、`start <= end`、31日まで)、`WorkCalendar`。`WorkCalendar` は平日に当たる休日(祝日と独自の休日)を並べて持ち、`business_days_in(a, b]` は週の計算とその二分探索で数える。`add_business_days`、`previous_business_day`、`effective_due`(営業日の設定のときだけ前の営業日に寄せる)、`holidays_in`(`list_holidays` の中身)もここ。
    - 毎年くり返す独自の休日は、最初の年から2099年まで(最初の年が2100年以降ならその年だけ)を休みにする。最初の年より前の年には効かない。2/29 は、うるう年でない年は 2/28 にする。
    - 営業日の設定での日数: 「あとN営業日」は今日の次の日から寄せた日限まで、「N営業日超過」と着手遅れの日数は最低1。`Assessment.business_days` は営業日の設定ならいつも true、`due_shifted` は開いている TODO の日限が休日に当たるときだけ true。
    - `TodoItem.follow_up_default` に確認日の初期値を入れる(`Rules::follow_up_default`)。画面の「待ちにする」はこれを使う。`Dashboard.business_days` で「N営業日以内」を出す。
    - 日報: 「明日の予定」は `Rules::next_day`(営業日の設定なら次の営業日)。日限注意の行は「日限 11/22、休日のため前倒し、あと1営業日、待ち」の順に添える。
    - 一覧の「今日まで」は `effective_due` で寄せた日限で比べる。「7日後まで」は暦日のまま。
    - settings の `custom_holidays` は、1件でも検査に通らない値があれば全体を既定値(`[]`)として読む。`update_settings` の日数の範囲のエラーは、営業日の設定なら「〜30営業日」と書く。
    - `list_holidays(start, end)`: 期間は開始日から3年(36か月)未満まで。祝日と独自の休日を、暦日の設定でも返す。同じ日に祝日と独自の休日があれば両方を返す(祝日が先)。
    - 画面の文言は `todoText.ts` の `dayUnit`、`overdueText`、`daysLeftText`、`SHIFTED_TEXT`。一覧の日限の下は「あと1営業日(休日のため前倒し)」、ダッシュボードとリマインドは「 ・ 休日のため前倒し」、ガントの行の名前は「日限 11/22(今日、休日のため前倒し)」。ガントのひし形の右には、超過の日数(前倒しなら括弧で添える)と、本日で前倒しのときの「休日のため前倒し」を出す(`rowLabel.ts` の `dueMarkText`)。
    - ガント: 休日は `useHolidays`(`list_holidays`)で表示期間の分を取り、`weekendRuns(range, holidays)` で土日と同じく網掛けする(月表示は今までどおり網掛けしない)。見出しの上の段に、月の名前と重ならない所だけ休日の名前を出し(`holidayLabels`)、日表示では日付の数字にマウスを載せると名前が出る。1日の名前は祝日を優先し、独自の休日だけの日は名前を「・」でつなぐ(`holidayNames`)。
    - 設定画面の「日数の数え方」のカードは `SettingsPage.tsx` の `DayCountCard`。独自の休日を足すとき、終了日が空なら開始日を入れ、「毎年くり返す」は初めからオン。リマインドのカードの単位は、営業日の設定なら「営業日以内」「営業日後」。
- 全文検索(フェーズ12)
  - 入口は検索ダイアログで、Ctrl+F(macOS は Command+F)とサイドナビの「検索」で開く。ダイアログは App が持つ。「検索」は画面ではないので `Route` には足さない。
  - 語の分割、前後の文の切り出し、並び順は Rust 側で作る。画面は受け取ったものを並べるだけ。
- ガントのドラッグ(フェーズ13)
  - 予定期間は `update_todo` で保存し、新しいコマンドは足さない。ドラッグの日数の計算は `geometry.ts` の純粋関数にして Vitest で確かめる。
- 日報テンプレート(フェーズ14)
  - 既定のテンプレートでは、今の出力(フィクスチャ)と完全に一致させる。所感・連絡事項は常に最後に置き、見出しを変えても前に書いた所感を残す。
- JSON のエクスポートとインポート(フェーズ15)
  - 取り込みは全データの置き換え。復元と同じく、作業用の DB で検査してから今のデータを `before-restore-<UTC日時>.db` に退避し、`ATTACH` して1トランザクションで入れ替える。`autostart`、`last_notified_at`、`tray_hint_shown`、`last_reminded_on` は、復元と同じく今の値のまま残す。

## 品質の決めごと

- `domain/` は単体テストを書く。日付の境界(当日、N日後ちょうど、月またぎ・年またぎ)と、遷移表の全行を押さえる。
- `repo/` はメモリ上の SQLite にマイグレーションを当ててテストする。
- 画面側は、ガントの座標計算など計算を含む部分を Vitest でテストする。
- ライブラリのバージョンは記憶に頼らず、導入時点の最新安定版と互換性を確認する。tauri-specta が入れた Tauri の版で動かない場合は、手書きの型定義に切り替えて報告する。
- tauri-specta は Tauri 2 向けの安定版がないため、RC 版を使う。tauri-specta、specta、specta-typescript は互換性のある組を `=` で固定する(今は `tauri-specta =2.0.0-rc.29`、`specta =2.0.0-rc.28`、`specta-typescript =0.0.15`)。上げるときは3つを同時に上げ、まずコマンド1つで型の生成と呼び出しを確かめてから全体に広げる。
- Tauri の権限(`capabilities/default.json`)と CSP は、使うものだけに絞る。

## 進め方

- `docs/implementation-plan.md`(MVP、フェーズ0〜8)と `docs/implementation-plan-v1.1.md`(v1.1、フェーズ10〜16)のフェーズ順に進め、フェーズの終わりにテスト結果と、利用者に画面で確認してほしい操作の一覧を報告して止まる。
- v1.1 はオーケストレーター方式で進める。フェーズごとにサブセッションが作業してコミットし、オーケストレーターが差分を確かめてから利用者に報告する(`implementation-plan-v1.1.md` の「進め方」)。
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
| アプリアイコンを原画から作り直す | `npm run icons` |
| 配布物のビルド(実行ファイルの ZIP を `release/` に作る) | `npm run build` |

- 配布物は実行ファイルの ZIP で、インストーラーと .dmg は作らない。`npm run build`(`scripts/package.mjs`)が、Windows では `Higiri.exe`、macOS では ad-hoc 署名した `Higiri.app` を ZIP にして `release/` に置く(macOS は `ditto` で固めて、署名と実行権限を保つ)。リリースは `v` で始まるタグ(`v0.1.0` など。版は `tauri.conf.json` の `version` と揃える)を push すると、`.github/workflows/release.yml` が下書きのリリースを作り、Windows と macOS の ZIP を添付してから公開する。`windows.yml` と `macos.yml` は手動で動かす確認用で、ZIP を包み直さずに(`archive: false`)成果物に上げる。どれも同じスクリプトで ZIP を作り、Rust のキャッシュを共有する(ジョブ名を `build` で揃えている)。Actions では Vitest と cargo test を通してから作る(型チェックは build:web の中で行い、clippy は手元の `npm run check` だけで動かす)。配布版の設定(`[profile.release]`)は、ビルド時間を優先して LTO を使わない。改行は `.gitattributes` で LF に揃えている(生成物とフィクスチャをバイト単位で比べるテストがあるため)。
- アプリアイコンは D3 案(藍の判子に白いチェック、右上に日限を表す朱のひし形。色は Aizome の `sumi-50`・`ai-700`・`shu-500`)。原画は `src-tauri/icons/source/` の SVG で、`higiri-icon.svg`(全面。PNG と .ico の 48〜256px)、`higiri-icon-small.svg`(16〜32px 用に内側の枠を省き、チェックを太くした簡略版)、`higiri-icon-macos.svg`(Apple のグリッドに合わせた .icns 用)。トレイのアイコンは、Windows が `higiri-icon-small.svg` の 32px(`tray-windows.png`)、macOS がメニューバー用の単色テンプレート画像 `higiri-tray-macos.svg` の 36px(`tray-macos.png`、18pt の2倍)で、どちらも `tray.rs` に埋め込む。`npm run icons`(`scripts/icons.mjs`)で `src-tauri/icons/` を作り直す。生成されたファイルは手で編集しない。
- 開発ビルドの DB は、配布版と混ざらないように、データフォルダの下の `dev/todo.db` を使う(macOS は `~/Library/Application Support/com.redjet.higiri/dev/`、Windows は `%APPDATA%\com.redjet.higiri\dev\`)。サンプルデータを入れ直すときは、アプリを閉じてこの `dev` フォルダを消してから `npm run dev:sample` で起動する。
- サンプルデータはモックの3プロジェクト・11件で、日付は「モックの今日(2026-11-19)」が実際の今日になるようにずらして入れる。ステータスの変化は `change_status` と同じ遷移関数で再現するので、変更履歴も残る。
