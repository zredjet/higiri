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
| `docs/requirements-design.md` | 要件49件、非機能要件、リリース段階、アーキテクチャ、DDL、主要ロジック、画面構成、コマンド一覧、未決事項。v1.1 の詳細(「v1.1の詳細」の節)、v0.3.0 の詳細(「v0.3.0の詳細」の節)と v0.4.0 の詳細(「v0.4.0の詳細」の節) |
| `docs/implementation-plan.md` | MVPまでのフェーズ分けと、各フェーズの完了条件 |
| `docs/implementation-plan-v1.1.md` | v1.1 のフェーズ分け(フェーズ10〜16)と、各フェーズの完了条件。進め方(オーケストレーター方式) |
| `docs/implementation-plan-v0.3.0.md` | v0.3.0 のフェーズ分け(フェーズ17〜24)と、各フェーズの完了条件。進め方は v1.1 と同じ(作業のブランチは `dev-0.3.0`) |
| `docs/implementation-plan-v0.4.0.md` | v0.4.0 のフェーズ分け(フェーズ25〜30)と、各フェーズの完了条件。進め方は v0.3.0 と同じ(作業のブランチは `dev-0.4.0`) |
| `docs/ui-mock/` | 6画面のモック。見た目は `screenshots/`、正確な値と文言は `.dc.html` |
| `docs/aizome/` | デザインシステム Aizome の方針、トークン、部品のCSS・型定義・参考実装 |

## 作る範囲

v0.4.0 までを作る。設計書の「段階」が「将来」のもの(時間管理、月報、ウィンドウを開かずにTODOを追加する T-13)は作らない。データ構造の備え(`reports.kind` の `monthly`)は設計書どおり入れたままにする。

MVP(v1.0。公開した版は v0.1.0)、v1.1(v0.2.0)と v0.3.0 は完成している。v1.1 は `docs/implementation-plan-v1.1.md` のフェーズ10〜16で作った。仕様は設計書の「v1.1の詳細」、決めごとは下の「v1.1 の決めごと」にある。

v0.3.0(サブTODO、TODOテンプレート、繰り返しTODO、TODO間の依存関係、週報)は `docs/implementation-plan-v0.3.0.md` のフェーズ17〜24で作った。仕様は設計書の「v0.3.0の詳細」、決めごとは下の「v0.3.0 の決めごと」にある。

v0.4.0(表示倍率、バックアップの複製先、後続TODOのずらし、一括変更)は `docs/implementation-plan-v0.4.0.md` のフェーズ25〜30で作る。仕様は設計書の「v0.4.0の詳細」、決めごとは下の「v0.4.0 の決めごと」にある。

段階は、v0.3.0 からアプリの版番号(v0.3.0 など)で呼ぶ。「v2」の呼び名は使わない(利用者が決定)。「MVP」「v1.1」は、これまでの資料のまま使う。

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
- コマンドは「時計を読んで内側の関数を呼ぶだけの `#[tauri::command]`」と「`&SqlitePool` と `today` / `now` を受け取る内側の関数」に分け、テストは内側の関数をメモリ上の DB で呼ぶ。v1.1 の OS 側への反映(自動起動の登録、通知のタイマーの計算し直し、×ボタンの動き)は `#[tauri::command]` の層で行い、内側の関数は DB だけを扱う。
- 画面に返すエラーは `AppError { kind, message }` の1種類。`message` は利用者向けの日本語で「何が起きたか+どうすればよいか」。DB の詳細は画面に出さない。
- ステータスの遷移: 開いている4状態(未着手・作業中・待ち・保留)の間は自由。完了と中止にはその4状態から入れる。完了から戻せるのは作業中(再開)だけ、中止から戻せるのは未着手だけ。完了と中止の間は行き来できない。待ちから抜けると待ちの3項目(`waiting_on`、`waiting_since`、`follow_up_date`)を空にする。
- 公式の sql プラグインは使わない(画面側からSQLを発行する方式のため)。
- ファイルへの書き出し(日報の .md、バックアップ、v1.1 の JSON)と、v1.1 の JSON の読み込みは Rust 側で行う。保存先やファイルを選ぶダイアログも Rust 側で開き、画面から任意のパスを渡して読み書きさせない。

## データの決めごと

- テーブル定義は設計書のDDLをそのまま `src-tauri/migrations/0001_init.sql` にする。列や制約を変えたいときは先に相談する。v0.3.0 で足した3つの表(`todo_templates`、`todo_repeats`、`todo_dependencies`)は、設計書の DDL のとおり `0002_v0_3_0.sql` にした(フェーズ18)。表は全部で9つ。
- 日付は `YYYY-MM-DD` の文字列(PCのローカル日付)、日時は UTC の ISO 8601 文字列。日時は `2026-11-19T01:30:00.000Z` の形(ミリ秒つき、末尾 `Z`)で保存し、文字列の大小が時刻の前後と一致するようにする。
- `todo_events` の値の形:
  - `status` の行の `old_value` / `new_value` は、ステータス名の文字列(`todo`、`doing`、`waiting`、`hold`、`done`、`cancelled`)。JSON にしない。
  - `due_date` と `follow_up` の行は `YYYY-MM-DD`。空にしたときは NULL。
  - `planned` と `actual` の行は `{"start": 日付または null, "end": 日付または null}` の JSON。
  - `created` の行は、`old_value` が NULL、`new_value` が作成時のステータス名。繰り返しで作った次の回(と子)の行は、`note` に `repeated_from` が入る(v0.3.0)。
  - 1回のステータス変更は1行で記録する。自動で入れた日付や空に戻した項目は、その行の `note` の `auto` に入れる。
- `todo_events.note` は、すべての行で JSON のオブジェクトにする。補足がなければ `{}`。使うキーは次の表のものだけ。

  | キー | 値 | 入る行 | 中身 |
  | --- | --- | --- | --- |
  | `waiting_on` | 文字列 | 待ちにした `status` の行 | 入力された、待っている相手 |
  | `follow_up_date` | `YYYY-MM-DD` | 待ちにした `status` の行 | 入力された確認日 |
  | `reason` | 文字列 | 中止にした `status` の行 | 入力された中止の理由 |
  | `repeated_from` | 数値 | 繰り返しで作った次の回(と子)の `created` の行(v0.3.0) | 前の回のTODO(子は前の回の子)の id |
  | `auto` | オブジェクト | `status` の行すべて(変えた列がなければ `{}`) | 遷移で自動的に変えた `todos` の列。キーは列名(`actual_start`、`actual_end`、`waiting_since`、`closed_at`、`waiting_on`、`follow_up_date`、`close_reason`)、値は変えた後の値(日付は `YYYY-MM-DD`、`closed_at` は上の日時の形)。空に戻した列は `null` |

  - 入力をそのまま入れる列(`waiting_on`、`follow_up_date`、`close_reason`)は、空に戻したときだけ `auto` に入る。入力した値は `waiting_on` / `follow_up_date` / `reason` のキーに入る。
  - 例: 作業中から待ちにした行は `{"waiting_on":"認証担当の回答","follow_up_date":"2026-11-20","auto":{"waiting_since":"2026-11-19"}}`。完了から再開した行は `{"auto":{"actual_end":null,"closed_at":null}}`。
- `settings` は1キー1行で、値は JSON。キーは `reminder_frequency`(`"every_launch"` / `"once_per_day"`)、`soon_days`(数値)、`follow_up_days`(数値)、`last_reminded_on`(`"YYYY-MM-DD"` か `null`)、`theme`(`"light"` / `"dark"` / `"system"`)。読めない値は既定値として扱う。v1.1 で足したキーは「v1.1 の決めごと」に、v0.3.0 で足したキー(`weekly_report_template`)は「v0.3.0 の決めごと」に、v0.4.0 で足したキー(`zoom` など)は「v0.4.0 の決めごと」にある。
- JSON の書き出し(フェーズ15)の表は、`projects`、`todos`、`work_logs`、`todo_events`、`reports`、`todo_templates`、`todo_repeats`、`todo_dependencies` の順(`json_data.rs` の `TABLES`)。`todo_events.note`、`todo_templates.body`、`todo_repeats.rule` と settings の値は、保存してある JSON の文をそのまま JSON の値として埋め込む。`schema_version` は v0.3.0 で2になった。取り込みでは、`schema_version` が1のファイル(v0.2.0 までの書き出し)は v0.3.0 の3つの表のキーがなくてよく(空として扱う)、2以上でどれかがなければ `Malformed`。
- 接続のたびに外部キー制約を有効にし、WALモードにする。
- 適用済みのマイグレーションファイルは書き換えない。変更は新しいファイルを足す。
- DBへの接続なしで `cargo build` が通る構成にする(`query!` 系マクロを使うなら `cargo sqlx prepare` の成果物をコミットする)。
- バックアップは `VACUUM INTO` で作る。WALモードのDBファイルを単純コピーしない。
- バックアップは `src-tauri/src/backup.rs`。データフォルダの `backups/` に、その日最初の起動時に `todo-YYYY-MM-DD.db` を作り、新しい順に7個残す(起動しなかった日は数えない)。常駐中に日付が変わったときも、常駐のタイマーが `backup::run_daily` でその日のバックアップを作る(`notifier::backup_if_new_day`。1日1個、7個残すのは同じ)。復元と JSON の取り込み(フェーズ15)の前には今のデータを `before-restore-<UTC日時>.db` に退避し、新しい順に3個残す。退避も復元の一覧に「復元・取り込みの前の状態」として出し、復元と取り込みを取り消せるようにする。v0.4.0 では、利用者が選んだ複製先のフォルダに毎日のバックアップ(新しい7個)を同じ名前で写す(`backup::sync_mirror`。退避は写さない。下の「v0.4.0 の決めごと」のフェーズ27)。
- 復元は、選んだバックアップの作業用コピーを検査してマイグレーションを当ててから、`ATTACH` して全テーブルの行を1トランザクションで入れ替える(接続プールを開いたまま、再起動なしで済む)。一覧から選ぶ復元(`backup::restore`)と、v0.4.0 のファイルを選ぶ復元(`backup::restore_file`。複製先のファイルなど、`backups/` の外も選べる)は、どちらも中の `restore_from` を通る。新しい版のアプリで作ったバックアップと、壊れたファイル(Higiri の DB でないものも)は復元しない。選んだファイルそのものは変えない。`autostart`、`last_notified_at`、`tray_hint_shown`、`last_reminded_on`、`zoom`、`backup_mirror_dir`、`backup_mirror_last_ok`(後の3つは v0.4.0。`repo::settings::KEPT_ON_REPLACE`)は、入れ替えの同じトランザクションの中で今の値に戻す(今ない値は、入れ替えたあともない)。退避・行の入れ替え・古い退避の削除は `backup::keep_and_replace`(行の入れ替えは中の `backup::copy_rows`)で、フェーズ15の JSON の取り込みも同じ関数を使う。入れ替える表は `backup.rs` の `DELETE_CHILD_FIRST` と `INSERT_PARENT_FIRST` に並べる(9つすべて)。v0.3.0 の表は、消すのは `todo_dependencies`、`todo_repeats` を `todos` より先、入れるのは `todos` より後(`todo_templates` はほかの表を指さない)。v0.2.0 までのバックアップは、作業用のコピーにマイグレーションを当てるので、3つの表が空になって復元される。復元のあとは `restore_backup`(と `restore_backup_file`)の `#[tauri::command]` の層で `resident::apply` を呼び、×ボタンの動きとタイマーの待ち時間を復元した設定に合わせる。

## 画面の決めごと

- 見た目は `docs/ui-mock/` に合わせる。部品とトークンは Aizome を使い、色・余白・角丸を直値で書かない(`var(--…)` を使う)。
- Aizome の14部品は `docs/aizome/components/bundle.js` を参考に、TypeScript の React コンポーネントとして `src/components/aizome/` に移植する。クラス名、DOM構造、aria属性は変えない。スタイルは `bundle.css` をそのまま使う。
- React は 19 を使う。移植のとき、`index.d.ts` の `JSX.IntrinsicElements` は `React.JSX.IntrinsicElements` に直し、`createPortal` は `react-dom` から import する。
- 移植済みの Aizome は `src/components/aizome/`。`tokens.css` は `docs/aizome/tokens.css` の写し、`aizome.css` は `bundle.css` から Google Fonts の `@import` だけを外した写し。どちらも書き換えない。アプリ独自のレイアウトは `src/styles/app.css` に置く。
- フォントは `@fontsource/ibm-plex-sans-jp`(400/500/600/700)と `@fontsource/ibm-plex-mono`(400/500)を `src/styles/fonts.ts` で読み込む。`vite.config.ts` のプラグインで .woff を外し、.woff2 だけを同梱する。
- 画面の切り替えはルーティングのライブラリを使わず、`src/app/routes.ts` の `Route` を状態で持つ。サイドナビの id と画面は `routes.ts` で対応させる。
- データの取得と更新は TanStack Query。フックは `src/lib/queries.ts` にまとめ、更新したら影響する一覧(プロジェクトと TODO は両方)を取り直す。
  - `main.tsx` の `QueryClient` の既定は、queries と mutations の両方が `retry: false` と `networkMode: "always"`。コマンドは外部と通信しないので、PC がオフラインになっても読み込みと保存を止めない(既定の `"online"` のままだと、ウィンドウの `offline` イベントで止まる)。
  - TODO の書き込み(`update_todo` と `change_status`)は、mutation の `scope`(`TODO_WRITES`)で1本の列に並べ、始めた順に1つずつ動かす。
  - `update_todo` はすべての項目を受け取るので、`useUpdateTodo` は `{ id, change }` を受け取り、送る直前に `get_todo` で今の TODO を読んで `change(今の todo)` の結果を送る。呼び出し側は `{ ...fieldsOf(current), 変える項目 }` のように自分が変える項目だけを重ねる(ガントのドラッグ、詳細パネル、ダッシュボードの日付の変更)。ほかの保存が入れたばかりの値を古い値で上書きしないため。
  - 設定の保存(`useUpdateSettings`)も同じ考え方で、`scope`(`SETTINGS_WRITES`)で1本の列に並べ、送る直前に `get_settings` で今の設定を読み、呼び出し側が渡した変える項目だけを重ねて `update_settings` に送る。変える項目は `Partial<SettingsInput>` か、今の設定を受け取る関数(`SettingsChange`)。`SettingsInput` を設定から作るのは `queries.ts` の `settingsInputOf`。見出しの欄を離れたときの保存と、その直後にほかのカードで押したスイッチの保存が、互いに古い値で上書きしないため(フェーズ14)。
- 入力の検証は Rust 側で行い、返ってきた `AppError.message` を表示する(`src/lib/errors.ts` の `errorMessage`)。フォームには `noValidate` を付け、ブラウザ標準の英語の吹き出しを出さない。
- 一覧の絞り込み・並べ替え(`domain/listing.rs`)、ステータスの選択肢(`TodoItem.next_statuses`)、タイムラインの文(`domain/timeline.rs`)は Rust 側で作る。画面は受け取ったものを並べるだけ。
- 日付の入力は `src/components/DateField.tsx` を使う(空を null にし、macOS の WebKit が空欄に今日の日付を出すのを薄く表示する)。
- Enter で確定する入力は `src/lib/keys.ts` の `isComposing` で日本語変換中の Enter を除く。
- 詳細パネルの項目は `useTodoDraft` で入力が止まって1秒後に `update_todo` で保存する。閉じるときと別のTODOに切り替えるときにも保存する。ステータスを変える前にも保存してから `change_status` を呼ぶ。
  - 項目ごとに「手で変えて、まだ保存していないか」(`touched`)を持つ。TODO がほかで変わったとき(ガントのドラッグ)は、手で変えていない項目だけが新しい値を取る。保存では手で変えた項目だけを、その時点の TODO に重ねて送る(どちらも `src/features/todos/detail/draft.ts` の `mergeDraft`)。保存が終わったら、送ったあとに打ち直していない項目は手で変えた印を外し、保存された値を出す(`settleSave`。タイトルの前後の空白が除かれるなど)。
  - `fieldsOf` も `draft.ts` にある。
- Markdown の編集は CodeMirror 6(`src/components/markdown/MarkdownEditor.tsx`)、表示は react-markdown + remark-gfm + rehype-sanitize(`MarkdownView.tsx`)。表示のリンクはアプリ内で開かず、http/https だけを opener プラグインの `openUrl` で既定のブラウザに渡す。
- CSP: CodeMirror が実行時に `<style>` を差し込むため、`style-src` だけ `'unsafe-inline'` を許し、`dangerousDisableAssetCspModification: ["style-src"]` で Tauri の nonce 付与を止めている。`script-src` は厳しいまま。opener の権限は `capabilities/default.json` で http/https に絞る。
- ダッシュボードの集計(日限注意の一覧と件数、今日の作業、待ち、進捗)と起動時リマインドの判定は `domain/dashboard.rs`。サイドナビのダッシュボードのバッジは `get_dashboard` の `attention_total` を使う。
- 起動時リマインドは起動時に1回 `get_startup_reminder` を呼び、「確認しました」のボタンでだけ閉じられ(画面外のクリック、Esc、× では閉じない)、閉じたら `ack_reminder`。ウィンドウが前面に来たときに日付が変わっていれば、すべての一覧を取り直して判定をやり直す(`src/app/useStartupReminder.ts`)。
- ガント: 行データ(予定・実績・待ちの区間、区分)は `get_gantt`(`domain/gantt.rs`)。待ちの区間は変更履歴から「待ちに入った日〜抜けた前日」(待ち中は今日まで)。日付から座標への変換、目盛り、表示期間、見えている行だけを描く処理は `src/features/gantt/geometry.ts`(Vitest でモックの座標と照合)。表示する日数は画面の幅に収まる分で、初期表示は今日が左から3分の1。日限のひし形はモックどおり超過(朱)と本日(琥珀)だけ色を付ける。予定のバーはドラッグで動かせる(v1.1 のフェーズ13)。
- TODO 一覧は200件を超えると、画面の近くの行だけを描く(`src/features/todos/virtualRows.ts` と `useVirtualRows.ts`)。行の高さは描いたあとに測り、それまでは見積もりを使う。Aizome の Table には見えている行だけを渡し、上下の行の分の高さは外側の要素の CSS(`tbody::before` / `::after`)で取る。Table の DOM は変えない。
- キーボードショートカット: Ctrl+N(新規TODO)、Ctrl+F(検索ダイアログ、v1.1)、Ctrl+1〜4(ダッシュボード、TODO、ガント、日報)は `src/app/shortcuts.ts`。macOS は Command。ダイアログやメニューが開いているときは効かない(Ctrl+F は、そのときも `preventDefault` だけはして WebView の検索バーを出さない)。「TODOを追加」と検索のダイアログは App が持ち、画面のボタン(サイドナビ)とショートカットで同じものを開く。Esc、Ctrl+Enter、Ctrl+E は詳細パネルとエディタ側にある。画面の文にショートカットを書くときは `src/lib/keys.ts` の `shortcutText` で作る(macOS では「Command+E」と出す。エディタの `Mod-` のキーは macOS では Command だけが効くため)。
- 動作ログは log プラグイン(OS のログフォルダの `higiri.log`、開発ビルドは `higiri-dev.log`。1 MB で切り替えて5個残す)。起動、バックアップ、復元、JSON の書き出しと取り込み、通知と自動起動の結果、想定外のエラーだけを書き、TODO の中身は書かない。起動のたびに、画面の読み込みが始まってからダッシュボードを出すまでの時間も書く(`App.tsx` の「dashboard shown … ms after the page started loading」)。画面側は `src/lib/log.ts` の `logUnexpected` を使う(`errorMessage` が AppError 以外のエラーを渡す)。
- 多重起動の防止(single-instance)は配布版だけで有効にする。開発ビルドは別のDBを使うので、インストールした版と同時に動かせるようにしている。2つ目の起動は既存のウィンドウを前面に出す(`--autostart` つきなら出さない)。ウィンドウの位置・大きさ・最大化は window-state で次回に戻す。
- 権限(`capabilities/default.json`)は、アプリのコマンドと、opener の http/https、clipboard-manager の `allow-write-text`、log の `allow-log` だけ。`core:default` は付けない(画面は Tauri のウィンドウ・イベント・パスの API を使わない)。notification と autostart のプラグインは Rust からだけ使い、画面側には権限を付けない。notification プラグインは画面に `window.Notification` の差し替えを入れ、Windows 以外では読み込み時に `is_permission_granted` を呼ぶが、権限がないので断られる(画面はこれを使わない)。
- 日報: 節の組み立ては `domain/report.rs`。過去の日は、本日の作業と本日完了・中止をその日の記録から、待ち・明日の予定・日限注意を今の TODO の状態から対象日を今日として作る(待ちは、対象日より後に待ちに入ったものを除く)。中身のない節は「- なし」。節の並び・見出し・出す節・添える項目は日報テンプレート(v1.1 のフェーズ14)で変えられ、所感・連絡事項はいつも最後で、その見出しより後ろが再生成しても残る(見出しの探し方は下の「v1.1 の決めごと」のフェーズ14)。既定のテンプレートでの、設計書の出力例との完全一致を `commands/reports.rs` のテスト(`tests/fixtures/daily_report_2026-11-19.md`)で確かめている。週報(v0.3.0)は `domain/weekly_report.rs` で、下の「v0.3.0 の決めごと」のフェーズ23にある。
- 日報の画面(`src/features/report/`): 日付を開くと保存済みの日報を、なければ生成した日報を出す(開いただけでは保存しない)。上の「日報」「週報」の切り替えで週報も同じ画面で扱う(v0.3.0 のフェーズ23)。編集が止まって1秒後と、再生成・コピー・.md で保存のときに `save_report` で保存する。コピーは clipboard-manager の `writeText`(権限は `clipboard-manager:allow-write-text` だけ)。.md の保存先ダイアログは dialog プラグインを Rust 側で開くので、画面側に dialog の権限は付けない。
- プロジェクトの作成はサイドナビの「プロジェクトを追加」、編集・アーカイブ・削除はプロジェクトのページの「プロジェクトを編集」から行う。アーカイブしたプロジェクトは設定画面のカードで一覧し、元に戻す・削除する。削除は確認ダイアログを挟む。
- `bundle.css` の先頭にある Google Fonts の `@import` は外し、IBM Plex Sans JP と IBM Plex Mono をアプリに同梱する。このアプリは外部と通信しない。
- Aizome にない部品(ガント、詳細パネル、Markdownエディタ、タイムライン)は同じトークンで作る。
- Aizome のダークテーマでは `--bg-subtle` と `--surface` が同じ色になる。`surface` の上に敷く網掛け(ガントの土日・プロジェクト行・見出し、進捗バーの地、Markdown のコードと表の見出し)は `src/styles/app.css` の `--surface-shade` を使う(ライトは `--bg-subtle`、ダークは `--sumi-850`)。
- 色だけで状態を伝えない。区分名やステータス名を必ず文字で添える。
- 文言は `docs/aizome/README.md` の「文章の基本」に従う(です・ます調、ボタンは動詞、「!」と絵文字は使わない)。モックにある文言はそのまま使う。
- Markdown の表示は HTML を無害化し、リンクは OS の既定ブラウザで開く。

## v1.1 の決めごと

v1.1 は `docs/implementation-plan-v1.1.md` のフェーズ10〜16で作った。仕様の詳細は設計書の「v1.1の詳細」にある。フェーズごとに作ったもの(ファイルの場所と細かい決めごと)は、下の各項目の「作ったもの」にある。

- テーブル定義は変えない(マイグレーションを足さない)。新しい値は `settings` のキーに入れ、検索は既存の列を `LIKE` で引く(FTS5 は使わない)。
- v1.1 で足したコマンドは次の5つだけで、コマンドは全部で32個(MVP の27個と合わせて)。コマンドを足すときは、上の「構成」の3か所をそろえ、件数のテスト(`commands/mod.rs`)も直す。

  | コマンド | フェーズ | 用途 |
  | --- | --- | --- |
  | `test_notification` | 10 | 設定画面の「通知を試す」 |
  | `list_holidays` | 11 | 期間(3年まで)の休日(祝日と独自の休日、名前つき)を返す。ガントの網掛けと設定画面に使う |
  | `search_todos` | 12 | 全文検索。当たった TODO と、当たった場所・前後の文を、最大100件と全体の件数で返す |
  | `export_data` | 15 | 全データを JSON で書き出す。保存先のダイアログは Rust 側で開く |
  | `import_data` | 15 | JSON を取り込む。ファイルを選ぶダイアログは Rust 側で開き、検査してから全データを置き換える |

- 自動起動、×ボタンの動き、通知の時刻、日数の数え方、日報テンプレートは `update_settings` で扱う。OS 側への反映は `#[tauri::command]` の層で行う(上の「構成」)。復元と取り込みで settings が入れ替わったときも同じく反映する。
- `settings` に足したキー(読めない値は既定値として扱う)

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
  - 作ったもの(フェーズ12)
    - `repo/search.rs`: `candidates` が、すべての語を含む TODO の ID・タイトル・更新日時を返す。語ごとに、タイトル・メモ・待っている相手・中止の理由・作業ログの本文(`EXISTS`)を `LIKE … ESCAPE '\'` で引き、語どうしは AND。`%`、`_`、`\` は文字として引く。SQL は `QueryBuilder` で組み、語はすべてバインドする(sqlx 0.9 は組み立てた文字列をそのまま受け付けない)。アーカイブしたプロジェクトも含める。
    - `domain/search.rs`: `split_terms`(空白と全角の空白で分ける。同じ語は、英字の大文字・小文字を区別せずに1つにまとめる)、`rank`(並び順)、`search_hit`(当たった場所と前後の文)。当たったかどうかは、`LIKE` と同じく英字だけ大文字・小文字を区別せずに見る。
    - 並び順: タイトルに語が1つでも入っているものが先。その中は TODO の更新日時(`todos.updated_at`。作業ログを足しただけでは変わらない)の新しい順、同じなら ID の大きい順。上から100件を返し、`total` に全体の件数を入れる。100件に切るのは並べたあとなので、`repo` は当たったものをすべて返し、`domain` で並べて切る。
    - 返す形: `SearchResult { hits: [SearchHit { item: TodoItem, title: [Segment], matches: [SearchMatch] }], total }`。`Segment` は `{text, hit}`。`title` はタイトルの全文(切り詰めない)。`matches` は、語を含む項目だけを、メモ・待っている相手・中止の理由・作業ログの順に入れる。`SearchMatch { field, log_date, segments }` の `field` は `memo` / `waiting_on` / `close_reason` / `work_log`。
    - 前後の文: 項目ごとに1つ。最初に当たった所の前後30字(文字の数で数える)を出し、その範囲に入るほかの語も `hit` にする。範囲の端で切れる語は最後まで含める。続く改行は空白1つにし、範囲の端の空白は除く。切った端には「…」を付ける。Markdown の記号は取り除かない。
    - 作業ログは、語を1つでも含むものを日付の新しい順(同じ日なら ID の大きい順)に3件まで。
    - `commands/search.rs`: `search_todos(text)`。語が空(空白だけ)なら0件を返す(エラーにしない)。判定は `Rules::from_settings` と `todo_item` で付ける。TODO と作業ログは `repo::todos::list_by_ids`、`repo::work_logs::list_by_todos` で100件分だけ読む。
    - 画面: `src/features/search/SearchDialog.tsx`(文言と小さな計算は `searchText.ts`)。App が開いたままにして、前に打った語を残し、開くたびに語の全体を選び、先頭の結果を選ぶ。打ち終えて200ms(`SEARCH_DELAY_MS`)で引き、変換中(`compositionstart`〜`compositionend`)は引かない。変換中の Esc ではダイアログを閉じない。↑↓ で選び(キーで動かしたときだけ見える所までスクロールする)、Enter かクリックで開く。打ってから200msたたないうちの Enter は、開かずにすぐ引く。
    - 選んだ TODO は、プロジェクトが稼働中ならそのページ、アーカイブ済みなら全体の TODO に移り、詳細パネルを開く(`App.tsx` の `openSearchResult`)。
    - 検索のフックは `queries.ts` の `useSearch`。キーは `["todos", "search", 語]` で、TODO と作業ログの変更で取り直す。ダイアログを閉じている間は引かない。
    - 1件の行: タイトル、日限注意の区分とステータスのバッジ、プロジェクト名、当たった場所ごとの見出し(「メモ」「待っている相手」「中止の理由」「作業ログ 11/19」)と前後の文。当たった語は `<mark class="search-mark">`(太字と `--warning-soft` の地)。選んでいる行は `--primary-soft` の地と `--focus` の枠。
    - 文言: 語が空なら「タイトル、メモ、作業ログなどから探します。語を入れてください。」、0件なら「当てはまるTODOはありません。語を変えて検索してください。」、件数は「3件」(100件を超えたら「523件のうち100件」と、「100件を超えました。語を足して絞ってください。」の案内)。入力欄の下に「空白で区切ると、すべての語を含むTODOを探します。」。
    - サイドナビの「検索」(`search` のアイコン)は、1つ目のまとまりの「日報」の下(`nav.ts`。id は `routes.ts` の `SEARCH_NAV_ID`)。
    - perf(`perf.rs` の `SEARCHES`): 全件に当たる「作業」、待っている相手の「回答」、中止の理由の「不要」、2語で数件の「作業ログ 1234」、英字の大文字・小文字の「todo 0042」、0件の「該当なし」を測る。
- ガントのドラッグ(フェーズ13)
  - 予定期間は `update_todo` で保存し、新しいコマンドは足さない。ドラッグの日数の計算は `geometry.ts` の純粋関数にして Vitest で確かめる。
  - 作ったもの(フェーズ13)
    - `geometry.ts`: `canDrag`(開いている4状態で、予定の開始日と終了日が両方あるもの)、`isDrag`(縦横どちらかに3px(`DRAG_THRESHOLD`)以上動いたらドラッグ。それ未満はクリック)、`dayDelta`(px から日数。最も近い日に吸着し、左右で同じ丸め)、`dragSpan`(`move` は長さを変えない。`start` と `end` は1日の長さで止める)、`barGrips`(つかむ範囲)、`dragLabelX`(日付の表示の位置)。日数は暦日で数える(営業日の設定でも休日を飛ばさない)。
    - つかむ範囲(`GRIP`): 端はバーの外側4pxと内側6pxまで。ただし中央に移動の範囲を8px残し、バーがそれより短ければバー全体を移動にする。月表示の1日のバー(4px)は、バーをつかむと移動、すぐ外側をつかむと伸び縮み。範囲の外に出ている端にはつかむ所を作らない。上下はバーより3pxずつ広い。
    - `useBarDrag.ts`: ドラッグの状態(`preview`)、保存中の位置(`saving`)、保存のエラーを `GanttPage` の高さで持つ(行は見えている分だけ描くので、行が描かれなくなっても続く)。ポインターは `window` で追い、つかんだ所は `setPointerCapture` する。ドラッグを始めたら `saveOpenDrafts`(`useTodoDraft.ts`)で詳細パネルの保存待ちの入力を先に保存する。Esc で取り消す(詳細パネルは閉じない)。ドラッグや Esc のあとのクリックは、どこに落ちても捨てる。`pointercancel` とウィンドウのフォーカスが外れたときは、保存せずにやめる。日付が変わっていなければ保存しない。
    - 離したら `useUpdateTodo` で、その時点の TODO に新しい予定期間を重ねて保存する。保存して一覧を取り直すまでは新しい位置で描き、失敗したら元の位置に戻して、ガントの上に `Alert`(×で閉じられる。次のドラッグでも消える)で `errorMessage` の文を出す。
    - ドラッグ中は、バーに `--focus` の枠を付け、バーの上に日付(「11/20〜11/27」。`rowLabel.ts` の `dragText`)を出す。カーソルは `app.css` の `.gantt-grip--move`(grab)、`.gantt-grip--resize`(ew-resize)と、ドラッグ中にルート要素に付ける `gantt-dragging--move` / `--resize`。
- 日報テンプレート(フェーズ14)
  - 既定のテンプレートでは、今の出力(フィクスチャ)と完全に一致させる。所感・連絡事項は常に最後に置き、見出しを変えても前に書いた所感を残す。
  - 作ったもの(フェーズ14)
    - `domain/report_template.rs`: `ReportTemplate { title, sections }`。節は `Section { heading, enabled, body }` で、`body`(`SectionBody`)が種類と項目を持つ(JSON では `kind` と `options` が節に並ぶ。`#[serde(flatten)]` と `tag = "kind", content = "options"`)。既定(`Default`)は今の日報と同じ形。
    - JSON のキー: 種類(`kind`)は `work`(本日の作業)、`closed`(本日完了・中止)、`waiting`(待ち)、`tomorrow`(明日の予定)、`attention`(日限注意)、`note`(所感・連絡事項)。項目(`options`)は `work` が `project_headings`・`status`・`due_date`・`work_time`、`closed` が `close_reason`、`waiting` が `waiting_on`・`waiting_days`・`follow_up_date`、`tomorrow` が `planned_period`、`attention` が `days`・`waiting_mark`・`follow_up_and_late_start`(既定はこれだけ `false`)、`note` は `{}`。
    - 正規化(`normalized`): 種類の重複は前のものを残し、足りない種類は既定の節を所感・連絡事項の前に足し、所感・連絡事項を最後に移していつも出す。`checked` は正規化に加えて、題名と見出しの前後の空白と、頭の `#` と空白(「## 待ち」→「待ち」。「#1 進捗」はそのまま)を除き、検査する(`TemplateError`)。題名は空にできず60字まで(`TITLE_MAX`。`{日付}` なども1字ずつ数える)、見出しは空にできず40字まで(`HEADING_MAX`)、改行は使えない、6つの見出し(出さない節も含む)はすべて違うものにする。
    - 題名と見出しは文だけを持ち、`# ` と `## ` はアプリが付ける。題名の `{日付}` は `2026-11-19`、`{曜日}` は `木`(`fill_title`)。
    - settings の `report_template` は、読めない値と検査に通らない値は全体を既定値にし、読むときに正規化する(`repo::settings` の `read_report_template`)。`update_settings` は `checked` で検査し、正規化したものを保存する。エラーの文は `commands/settings.rs` の `template_error`(節は既定の見出しで呼ぶ。「「明日の予定」の節の見出しを入力してください。」)。
    - `report.rs`: `generate_body(input, &template)` は題名と、出す節をテンプレートの順に組み立てる。`compose(body, note, &template)` が所感・連絡事項の見出しを最後に付ける。項目を外したときの書式: 括弧の中が空になったら括弧ごと出さない。本日の作業でステータスと日限を外して作業時間だけのときは「**タイトル** 1.5h」と空白を入れる。プロジェクトの見出しを外すと「- プロジェクト名 / **タイトル**(…)」になり、空のときは「- なし」を見出しの直後に置く(ほかの節と同じ)。日限注意の「休日のため前倒し」は日限に付くので、日数を外しても出す。
    - 要確認と着手遅れを含めるとき(`follow_up_and_late_start`)の行は、起動時リマインドに合わせて「【要確認】P / T(確認日 11/19、待ち3日目)」「【着手遅れ】P / T(予定開始 11/17、2日経過)」。「待ち3日目」「2日経過」は日数(`days`)の項目で出す。営業日の設定では「2営業日経過」。
    - 所感・連絡事項の切り出し(`split_note(full, &template)`、中は `note_line`): (1) 今のテンプレートの見出しと既定の見出し(「所感・連絡事項」)のうち、本文で先に出てくる行。今のテンプレートで出す節の見出しになっている文では探さない。(2) どちらもなければ、節の見出し(今のテンプレートの全節と既定)のどれでもない最初の `## ` の行(見出しを2回以上変えたときの、昔の所感の見出し)。(3) それもなければ所感なし(所感の見出しの行を手で消した日報。MVP と同じ)。迷ったときは多めに残す側に倒すので、節の見出しを変えていたころの日報では、生成した部分の一部が所感の側に重なって残ることがある(手で書いた所感は消えない)。`## ` は行頭のものだけを見る(字下げしたものと `### ` は対象外。作業ログの本文は字下げされているので、ログの中の `## ` では切らない)。`save_report` の `note_md` も同じ関数で切り出す。この順番はオーケストレーターと決めたもので、テストは A→B→C と変えたあとの再生成、所感の見出しの行を消した日報、所感の中の「## 所感・連絡事項」、ログの本文の `## ` を押さえている。
    - 「初期値に戻す」の既定のテンプレートは、tauri-specta の `constant` で `bindings.ts` に `DEFAULT_REPORT_TEMPLATE` として出す(`commands::specta_builder`)。画面はこれを送る。新しいコマンドは足していない。押すと確認のダイアログ(「日報のテンプレートを初期値に戻しますか」、ボタンは「キャンセル」「初期値に戻す」)を出す。設定画面は変更をすぐ保存するので、押し間違いで変更がすべて消えないようにするため。
    - 画面: 設定画面の「日報のテンプレート」のカード(`src/features/settings/ReportTemplateCard.tsx`。変える処理は `reportTemplate.ts`、Vitest あり)。題名の欄、節ごとのまとまり(節の名前、「日報に出す」のスイッチ、上へ・下へのボタン、見出しの欄、添える項目のチェック)、「初期値に戻す」のボタン。所感・連絡事項は動かせず、スイッチもない(いつも出して最後)。スイッチ・チェック・並べ替えはすぐ保存し、エラーはカードの上に出す。題名と見出しは、ほかの設定の欄(「間近」の日数)と同じく欄を離れたときに保存し(`DraftTextField`)、エラーは欄の下に出して、打ち直すと消す。保存していない文は、ほかの保存があっても残す。上への矢印は Aizome に `chevron-up` がないので、`chevron-down` を `.icon-up` で回して使う。
    - 日報の画面の「再生成」の横の文は、所感・連絡事項の見出しを今のテンプレートから取る(既定ではモックの文のまま)。
- JSON のエクスポートとインポート(フェーズ15)
  - 取り込みは全データの置き換え。復元と同じく、作業用の DB で検査してから今のデータを `before-restore-<UTC日時>.db` に退避し、`ATTACH` して1トランザクションで入れ替える。`autostart`、`last_notified_at`、`tray_hint_shown`、`last_reminded_on` は、復元と同じく今の値のまま残す。
  - 作ったもの(フェーズ15)
    - `src-tauri/src/json_data.rs`: 書き出し(`export_json`、ファイルに書く `export_to`)と取り込み(`import`、中身は `import_bytes`)。`commands/json_data.rs` が `export_data` と `import_data`。
    - ファイルの形: 設計書の形(`format` は `"higiri-export"`、`format_version` は1、`schema_version` は `_sqlx_migrations` の適用済みの最新の番号、`app_version`、`exported_at`、5つの表、`settings`)。表は `projects`、`todos`、`work_logs`、`todo_events`、`reports` の順で(v0.3.0 で3つの表を後ろに足した。「データの決めごと」と「v0.3.0 の決めごと」のフェーズ18)、行は ID(`reports` は種類と開始日)の順。行は列名をキーにし、値は保存されているまま(整数・文字列・null)。`todo_events.note` と settings の値は、保存してある JSON の文をそのまま(キーの順も変えずに)JSON の値として埋め込む(serde_json の `raw_value`)。1行に1つの行を置き、ほかは2字の字下げ。settings はキーの順。
    - 書き出し: 表(v0.3.0 からは8つ)と settings を1つのトランザクションで読む。保存先と同じフォルダの `<名前>.part` に書いてから名前を変え、失敗したら `.part` を消す(途中までのファイルを残さない)。JSON として読めない settings の値は書き出さない(アプリは既定値として読むので、取り込んでも同じになる)。JSON として読めない `note` は文字列として書き出す(アプリが書くことはない。その行は取り込みで「読めない値」として断られる)。
    - 取り込みの順: (1) 先頭の BOM を除く。(2) 先頭の項目だけを読む(ほかのキーも構文は確かめる)。JSON として読めなければ `NotJson`、`format` が違えば `NotOurs`、`format_version` が1より大きいか `schema_version` がこの版の最新より大きければ `TooNew`、版の値がおかしければ `Malformed`。(3) 5つの表と settings を読む(どれかがなければ `Malformed`。ほかのキーは無視。v0.3.0 の3つの表は `schema_version` が2以上のときだけ要る)。(4) `backups/import-work.db` を作り、ファイルの `schema_version` までマイグレーションを当てて(`MIGRATOR.run_to`)、外部キーを遅らせた1つのトランザクションで行を入れる。列は作業用の DB の `pragma_table_info` と照らし、知らない列と型の違う値(INTEGER の列に整数以外、TEXT の列に文字列以外)は `Malformed`。ない列は表の既定値になる。外部キー・CHECK・NOT NULL・主キーの重複は `Constraint`。(5) 残りのマイグレーションを当て、アプリの読み取り(`repo::projects::list`、`repo::todos::list`、`repo::work_logs::list_all`、`repo::events::list_all`、`repo::reports::list_all`。v0.3.0 で `repo::templates::list`、`repo::repeats::list`、`repo::dependencies::list` を足した)で全行を読み直す。読めない値(日付の書き方、ステータス名、優先度、変更履歴の種類と `note` の形、テンプレートの `body` と周期の `rule`)は `Unreadable`。(6) `backup::keep_and_replace` で退避して入れ替える。断ったときは退避も入れ替えもせず、作業用の DB は成否にかかわらず消す。
    - 行は同じ列の並びが続く間、1つの INSERT にまとめる(1文の値は4,000個まで)。ID はファイルの値のまま入れる。`AUTOINCREMENT` の表はない(`sqlite_sequence` もない)ので、取り込んだあとの新しい行は今ある最大の ID の次になる。
    - settings: ファイルの値をそのまま入れる。知らないキーも残し(アプリは無視する)、読めない値も残す(アプリは既定値として読む。DB のときと同じ)。`KEPT_ON_REPLACE` の4つ(v0.4.0 で `zoom`、`backup_mirror_dir`、`backup_mirror_last_ok` を足して7つ)は `copy_rows` が今の値に戻す。
    - エラーの文は `commands/json_data.rs` の `import_error`。断ったときは「〜ため、取り込めませんでした。今のデータは変わっていません。」に、どうすればよいか(「Higiriの「JSONで書き出す」で作ったファイルを選んでください。」「アプリを新しい版にしてから取り込んでください。」「書き出したあとに手で直した所があれば元に戻すか、書き出し直したファイルを選んでください。」)を続ける。
    - ダイアログは .md の保存と同じく `spawn_blocking` で開く(保存は `blocking_save_file`、既定の名前は `higiri-YYYY-MM-DD.json`。取り込みは `blocking_pick_file`)。どちらも JSON の絞り込みを付ける。選ばずに閉じたら `null` を返し、何もしない。`export_data` は保存先のパス、`import_data` はファイル名を返す。取り込みのあとは `resident::apply` を呼ぶ(復元と同じ)。
    - 動作ログ: 書き出しと取り込みをしたこと、取り込みで作った退避のファイル名、断った理由の種類(読めない値は列の名前まで)だけを書く(件数・中身・パスは書かない。`ImportError` は値を持たない)。
    - 画面: 設定画面の「データ」のカードに、説明の文と「JSONで書き出す」「JSONから取り込む」のボタン。書き出したら「JSONに書き出しました。保存先: …」。取り込みは確認のダイアログ(`src/features/settings/ImportDialog.tsx`。題は「JSONから取り込みますか」、文は設計書のもの、ボタンは「キャンセル」「ファイルを選ぶ」)から開き、断られたらダイアログの中にエラーを出す(ダイアログは開いたまま、別のファイルを選べる)。ファイルを選ばずに閉じたら確認のダイアログも閉じる。取り込んだら「「(ファイル名)」から取り込みました。元に戻すときは「バックアップから復元」で「復元・取り込みの前の状態」を選んで復元してください。」。フックは `queries.ts` の `useExportData`、`useImportData`(取り込んだらすべての一覧を取り直す)。
    - 退避の表示名は「復元・取り込みの前の状態(日時)」(`backupText.ts`)。復元のダイアログと案内の文も合わせた。
    - perf(`perf.rs`): 5,000件・作業ログ20,000件で、書き出し(約9.5 MB)と、それを同じ DB に取り込む時間を測る。

## v0.3.0 の決めごと

v0.3.0 は `docs/implementation-plan-v0.3.0.md` のフェーズ17〜24で作る。仕様の詳細は設計書の「v0.3.0の詳細」にある。フェーズごとに作ったもの(ファイルの場所と細かい決めごと)は、各フェーズの終わりにここへ足す。

- 作業のブランチは `dev-0.3.0`(リリースのタグ `v0.3.0` と同じ名前にしない)。
- 表を3つ足す(`todo_templates`、`todo_repeats`、`todo_dependencies`。`0002_v0_3_0.sql`)。既存の表の列と制約は変えない。`todo_templates.body` と `todo_repeats.rule` の JSON の形は設計書にある。JSON の書き出しでは、`todo_events.note` と同じく JSON の値として埋め込む。
- v0.3.0 で足すコマンドは次の9つで、コマンドは全部で41個(v1.1 までの32個と合わせて)。週報は日報のコマンドが `kind: "weekly"` を受け付ける形にし、コマンドを足さない。

  | コマンド | フェーズ | 用途 |
  | --- | --- | --- |
  | `set_todo_parent` | 19 | 親を付ける・付け替える・外す。子を作るのは `create_todo` の `parent_id` |
  | `list_todo_templates` | 20 | テンプレートの一覧 |
  | `save_todo_template` | 20 | 親のないTODOと子からテンプレートを保存する(同じ名前の置き換えは `replace`) |
  | `rename_todo_template` | 20 | テンプレートの名前を変える |
  | `delete_todo_template` | 20 | テンプレートを消す。テンプレートから作るのは `create_todo` の `template_id` |
  | `set_todo_repeat` | 21 | 繰り返しの周期を付ける・変える・外す。次の回は `change_status` が作る |
  | `add_dependency` | 22 | 先行TODOを結ぶ |
  | `remove_dependency` | 22 | 依存を外す |
  | `list_dependencies` | 22 | 先行TODOと後続TODOの一覧 |

- `settings` に足すキー

  | キー | 値 | 既定値 | フェーズ |
  | --- | --- | --- | --- |
  | `weekly_report_template` | 週報テンプレートの構造(設計書の「週報(R-6)」) | 既定の週報の形 | 23 |

- 親子は1段だけ。親と子は同じプロジェクト。どちらもアプリ側で守る。
- `change_status` は、子が残っている親を閉じると子も閉じ(子ごとに1行記録)、周期のあるTODOを閉じると次の回を作る。どちらも同じトランザクション。
- 判定(`classify`)は子にもそのまま使う。依存関係では日限注意の区分を増やさない。依存は止めずに知らせるだけ(作業中にするときの確認は画面側)。
- 子・テンプレート・繰り返し・依存の検査は Rust 側。画面は返ってきた `AppError.message` を出す。
- データの土台(フェーズ18)
  - 画面とコマンドは変えていない(コマンドは32個のまま)。
  - 作ったもの(フェーズ18)
    - `src-tauri/migrations/0002_v0_3_0.sql`: 設計書の DDL のとおり。新しい DB にも v0.2.0 の DB(`0001` だけの DB)にも当たり、今の行は変わらない(`repo/db.rs` のテスト)。
    - `domain/template.rs`: `TemplateBody { title, priority, memo_md, children }` と `TemplateChild { title, priority, memo_md, due_offset_days, depends_on }`。JSON は設計書の形で、読むときに `depends_on` がなければ `[]`、`due_offset_days` は `null` を `None` にする。`checked` はタイトル(親と子)の前後の空白を除いて空なら断り、`depends_on` が範囲の外・自分・同じ位置の2回・循環(子どうしの依存をたどって戻る)なら断る(`TemplateBodyError`)。`from_json` は読んでから `checked` を通し、通らなければ `None`。保存したテンプレートは `TodoTemplate`。
    - `domain/repeat.rs`: `RepeatRule`(`kind` で分ける serde の enum。`daily`、`weekly { weekdays }`、`monthly_day { day }`、`monthly_weekday { week, weekday }`、`monthly_last`、`yearly { month, day }`、`every_days { days }`)と曜日の `DayOfWeek`(`mon`〜`sun`)。どちらも specta の型を付けてある(フェーズ21で `set_todo_repeat` と `TodoItem.repeat` からバインディングに出た)。`check` は範囲の検査(`RepeatRuleError`。`weekdays` は1〜7個で重複なし、`day` は1〜31、`week` は1〜4か-1、`yearly` は暦にある月日で2/29は可、`days` は2〜365)。`from_json` は読んでから `check` を通す。周期の行は `TodoRepeat`。
    - `domain/model.rs`: 依存の行 `Dependency { todo_id, depends_on_id, created_at }`。
    - `repo/templates.rs`(一覧は名前の順、1件、追加、名前と本文の更新、削除。書く値は `TemplateFields`)、`repo/repeats.rs`(一覧は `todo_id` の順、1件は `Option`、追加、周期と終わりの日の更新、削除)、`repo/dependencies.rs`(一覧は `todo_id, depends_on_id` の順、1件は `Option`、追加、削除)。読めない `body` と `rule`(検査に通らないものも)は `RepoError::Corrupt`(列は `todo_templates.body`、`todo_repeats.rule`)。名前の重複、依存の組の重複、自分自身への依存は表の制約が断る(呼ぶ側が `is_constraint_violation` で見る)。テストの `repo::testing::todo` で TODO を1件作れる。
    - `json_data.rs`: `Table.since`(その表を足したマイグレーションの番号。5つの表は1、v0.3.0 の3つは2)。ファイルの `schema_version` より新しい表のキーはなくてよく、ある場合は空の配列だけを受け付ける(行があれば、その版の作業用の DB にない表なので `Malformed`)。`body` と `rule` は `note` と同じく JSON の値として埋め込み、取り込みでは JSON の文のまま入れる。
- サブTODO(フェーズ19)
  - コマンドは `set_todo_parent(id, parent_id)` を足して33個。子を作るのは `create_todo` の入力の `parent_id`。
  - 作ったもの(フェーズ19)
    - `domain/subtodo.rs`: 親子の検査(`check_parent`、作成のときの `check_new_child`。`ParentError` の文をそのまま画面に出す)、`Family`(TODO の集まりから、親のタイトルと日限、子の数を作る。同じ TODO を2回渡しても1回と数える)、`ChildCounts { total, done, open }`(`total` と `done` は中止を除く。`open` は開いている子)、`display_title`(「親 / 子」)。
    - 検査の順: 自分自身 → 別のプロジェクト → 親にしようとした TODO が子(孫になる) → 子のある TODO を子にする。`create_todo` は別のプロジェクトと孫だけを見る(アーカイブしたプロジェクトへの追加は今までどおり断る)。閉じた TODO も親・子にできる。付け替えは `updated_at` を変え、変更履歴には残さない(`repo::todos::set_parent`)。繰り返しの検査はフェーズ21で足した(下の「繰り返しTODO」)。依存の検査(依存で結ばれた2件は親子にしない)はフェーズ22で足した(下の「依存関係」)。
    - `TodoItem` の `parent_title`、`children`、`due_after_parent` は `Family` から付ける(`views::todo_item(todo, project_name, &family, today, rules)`。フェーズ21・22で引数が増え、今は `todo_item(todo, project_name, &family, &links, today, rules)`)。一覧・ダッシュボード・ガント・日報は読み込んだ全 TODO で `Family` を作る。1件(`get_todo` と、作成・更新・ステータス変更・付け替えの戻り値)は親と子を引いて作る(`commands/todos.rs` の `item`。子は `repo::todos::list_children`)。検索は当たった TODO の親と子を足して作る。`due_after_parent` は、開いている子の日限が親の日限より後(休日に寄せる前の日付で比べる。同じ日は警告しない)。
    - 一覧: `TodoList.rows`(`TodoListRow { item, child, bridge }`。v0.2.0 までの `items` から変えた)。並べ替えたあと `listing::tree_rows` で木の順にする(親どうし、同じ親の子どうしは選んだ並べ替えの順)。絞り込み(優先度・日限・状態のタブ)は1件ずつ判定し、当たった子の親が当たっていなければ、親をつなぎの行(`bridge`)にする。タブの件数は当たった TODO だけを数える。`Family` は絞り込む前の全 TODO で作るので、つなぎの行の「子 2/3」も全部の子で数える。
    - アプリが作らない形の親子(取り込んだファイルなど。親が一覧にない子、親に親がある子、自分を親にしている TODO)は、親の行として出す(`tree_rows`)。
    - ガント: `GanttRow.child`、`bridge`。プロジェクトごとに `tree_rows` で並べる。完了・中止を隠す設定で親だけが隠れて子が出るときは、親をつなぎの行(予定・実績・待ちの区間を空)にする。
    - `change_status`: 閉じた結果になったら(`close_children`)、開いている子を親と同じ行き先(中止なら同じ理由。待ちの項目は渡さない)で1件ずつ `transition` に通し、書いて記録する。閉じている子は変えない。親を再開しても子は閉じたまま。子の遷移が断られたら(子の実績開始日が今日より後など)全体を断り、親も子も変えない(オーケストレーターが決定)。エラーは「子TODO「…」を完了にできません。」に遷移のエラーの文を続ける。
    - 日報: 行のタイトルは `Lookup::title`(「プロジェクト / 親 / 子」、本日の作業の太字は「**親 / 子**」)。子を親の下にまとめることはしない(並びは今までどおり)。
    - 画面
      - `src/features/todos/tree.ts`: 折りたたみで描く行(`foldRows`)、開閉のボタンを付ける親(`parentsWithChildRows`)、`toggled`。Vitest あり。見えている行だけを描く処理(`useVirtualRows`)には、折りたたんだあとの行を渡す。
      - `src/features/todos/useFolded.ts`: 折りたたんだ親の集まり。親ごとに1つ、アプリを開いている間だけ覚え、一覧とガントで共有する(オーケストレーターが決定)。初めはすべて開いている。
      - 一覧(`TodoTable.tsx`): 子は `todo-cell--child` で字下げ。開閉のボタン(`tree-toggle`、`aria-expanded`)は表のセルの左の余白に置き、ボタンのない行とタイトルの位置をそろえる。「子 2/3」は `childProgressText`(中止を除いた数が0なら出さない)。つなぎの行は文字を薄くし、日限の下の文と警告は出さない。「親の日限より後」は `warningTexts(a, due_after_parent)` の最後に並ぶ。
      - ガント(`GanttPage.tsx`): 名前の欄に開閉のボタン(左の余白、行の高さいっぱい)、子の字下げ(`gantt-name--child`)、つなぎの行の薄い文字(`gantt-name--bridge`)、「子 2/3」、子の「親の日限より後」の警告アイコン(`gantt-name__warning`)。つなぎの行はバー・ひし形・ドラッグがない。プロジェクトの見出しの件数はつなぎの行を数えない。行の読み上げ名(`rowLabel.ts`)は「親 / 子:…」で、日限の括弧に「親の日限より後」を足し、つなぎの行は「親:完了(子TODOだけを表示しています)」。
      - 詳細パネル(`detail/FamilySection.tsx`): `useFamily`(そのプロジェクトの全 TODO の `list_todos` から、子と親の候補を取る)。`ParentField`(子のない TODO だけ。「親のTODO」の選択、「「親」を開く」、「親から外す」。候補は同じプロジェクトの親のない TODO で自分を除き、閉じたものは「(完了)」のように添える)。`ChildrenSection`(親のない TODO だけ。子の一覧(日限注意の区分とステータス。押すと開く)、「2/3 完了」、子がすべて閉じて親が開いていれば「子TODOはすべて終わりました。」、タイトルを打って Enter で足す欄)。親や子を開くと、パネルがその TODO に切り替わる(`TodoDetailPanel` の `onOpenTodo`)。付け替えのフックは `queries.ts` の `useSetTodoParent`(`TODO_WRITES` の列に並べる)。
      - ガントの警告は、名前の欄に「親の日限より後」の1つだけ(オーケストレーターが決定)。ガントのモックに警告アイコンがないので、一覧の2つの警告(計画が間に合わない・予定超過)はガントに足さない。色だけで伝えないよう、アイコンに読み上げ名とマウスを載せたときの文(どちらも「親の日限より後」)を付け、行の読み上げ名にも入れる。設計書の S-1 の「今の警告アイコンと同じ並びで」は一覧の書き方として扱う。
      - 子の日限が親の日限より後なら、詳細パネルにも「予定終了日が日限より後」と同じく警告を出す。
      - 閉じるときの確認: 完了は `StatusDialogs.tsx` の `DoneWithChildrenDialog`(詳細パネルとダッシュボードの「完了にする」)。中止は `CancelDialog` の説明に `cancelChildrenNote` を続ける。
      - 削除の確認の文は `deleteTodoText`(子の数は中止も含めて、プロジェクトの一覧から数える)。
      - 表示の名前は `todoText.ts` の `displayTitle`(ダッシュボードの日限注意・今日の作業・待ち、リマインド、日付の変更のダイアログ)。検索の結果はタイトルの前に「親 / 」を付ける。
    - サンプル(`sample.rs`): 社内ツール整備に親「開発環境の標準化」と子3件(1件は完了)。作業中・待ちにせず、予定開始を明後日以降、日限を「間近」より先にし、モックの日に閉じないので、日報のフィクスチャとダッシュボードの日限注意は変わらない。社内ツール整備の進捗は 2/7、ガントの行は7行になる。
    - perf(`perf.rs`): 5,000件のうち1,000件を子にする(500件の親に2件ずつ。`CHILDREN`)。
- TODOテンプレート(フェーズ20)
  - コマンドは `list_todo_templates`、`save_todo_template(todo_id, name, replace)`、`rename_todo_template(id, name)`、`delete_todo_template(id)` を足して37個。テンプレートから作るのは `create_todo` の入力の `template_id`。
  - 子どうしの依存(`depends_on`)は、フェーズ22で保存と使うときの結びを足した(下の「依存関係」)。
  - 作ったもの(フェーズ20)
    - `domain/template.rs`: `checked_name`(前後の空白を除いて1〜40字。`NAME_MAX`。字は文字の数で数える。`NameError` の文をそのまま画面に出す)、`TemplateBody::from_todos(parent, children)`(フェーズ22で子どうしの依存の引数を足した。中止した子を除き、子は `id` の順。`due_offset_days` は日限の暦日の差で、どちらかの日限が空なら `None`)、`TemplateBody::new_children(due)`(子の日限 = 入力の日限 + 日数。日限が空か日数が `None` なら空。日付の範囲を超えるときも空)、`TemplateSummary`(`list_todo_templates` などが返す `{ id, name, title, priority, child_titles, updated_at }`。`TodoTemplate::summary`)。
    - `repo/templates.rs`: `id_by_name`(名前がちょうど同じもの)。
    - `commands/templates.rs`: 4つのコマンド。保存は、子のTODOなら「子TODOからはテンプレートを作れません。親のTODOから保存してください。」、同じ名前があって `replace` が `false` なら「同じ名前のテンプレートがあります。別の名前を入力してください。」(名前の変更も、表の UNIQUE で断られたときも同じ文)。`replace` なら同じ行を書き換える(`id` と `created_at` は残る)。保存の前に `TemplateBody::checked` を通す(検査に通らない本文を保存すると一覧が読めなくなるため。タイトルが空のTODOは取り込んだデータにしかなく、「タイトルが空のTODOがあるため、テンプレートにできません。…」で断る)。名前の変更で今と同じ名前なら何も書かない。見つからないテンプレートは「テンプレートが見つかりません。削除された可能性があります。画面を開き直してください。」(`error.rs` の `todo template`)。
    - `create_todo`(`commands/todos.rs` の `create`): `template_id` があれば、親を作ってから子をテンプレートの順に作る(1トランザクション)。親のタイトル・優先度・プロジェクト・日限・予定期間は入力のもの。メモは、入力の `memo_md` が `null` ならテンプレートのもの(画面はいつも `null` を送る)。子は未着手で、予定期間は空、プロジェクトは親と同じ。変更履歴の「作成」を親と子に1件ずつ。`parent_id` と一緒なら「テンプレートから作るTODOは、子TODOにできません(親子は1段だけです)。親のTODOを選ばずに追加してください。」で断る。
    - 画面
      - `src/features/todos/templateText.ts`: 文言と `templateNamed`(前後の空白を除いて同じ名前)、`templateOptions`(「使わない」が先頭)。Vitest あり。
      - 「TODOを追加」(`NewTodoDialog.tsx`): テンプレートが1つ以上あるときだけ、一番上に「テンプレート」の選択(初めは「使わない」)。選ぶとタイトルと優先度を入れ、子TODOのタイトルの一覧(`template-preview`)と「日限を入れると、子の日限も決まります。」を出す(子がなければ「子TODOはありません。」)。「使わない」に戻しても、入っているタイトルと優先度は消さない。選んだテンプレートが消されていたら、Rust 側が断る(子なしで作らない)。
      - 詳細パネル: 「テンプレートとして保存」は、右上の「その他の操作」のメニュー(「削除する」の上)に置く。親のないTODOだけ。押すと詳細パネルの保存待ちの入力を保存してから(`saveOpenDrafts`。フェーズ20で Promise を返すようにした)、名前のダイアログ(`detail/SaveTemplateDialog.tsx`。初めの値はタイトル)を開く。保存の直前にも `saveOpenDrafts` を待つ。同じ名前が一覧にあれば、同じダイアログを「テンプレートを置き換えますか」に切り替え(ボタンは「名前を変える」「置き換える」)、`replace: true` で送る。保存したら、パネルの上に「テンプレート「…」を保存しました。「TODOを追加」で選ぶと使えます。」(×で閉じる。別のTODOに切り替えると出さない)。
      - 設定画面: 「日報のテンプレート」の下に「TODOのテンプレート」のカード(`src/features/settings/TodoTemplateCard.tsx`)。表(名前、子TODOの件数、「名前を変える」「削除する」)、名前の変更のダイアログ(「テンプレートの名前を変える」、ボタンは「キャンセル」「変更する」)、削除の確認(「テンプレートを削除しますか」)。1つもなければ「テンプレートはありません。TODOの詳細パネルの「テンプレートとして保存」で作れます。」。
      - フックは `queries.ts` の `useTodoTemplates`(キーは `["templates"]`)、`useSaveTodoTemplate`、`useRenameTodoTemplate`、`useDeleteTodoTemplate`(どれもテンプレートの一覧を取り直す)。テンプレートから作るのは `useCreateTodo` のまま(プロジェクトと TODO の一覧を取り直す)。復元と取り込みはすべてを取り直すので、テンプレートも取り直す。
    - サンプル(`sample.rs`): テンプレート「月次締め」(子3件。日数は −3、−2、0。フェーズ22で、報告書の作成がほかの2件を待つようにした)。TODO は足さないので、日報のフィクスチャとダッシュボードは変わらない。
    - perf(`perf.rs`): テンプレート30個(子5件ずつ。`TEMPLATES`、`TEMPLATE_CHILDREN`)。`list_todo_templates`、`save_todo_template`(同じ名前の置き換え)、テンプレートから作って消す(`create_todo` + `delete_todo`)時間を測る。
- 繰り返しTODO(フェーズ21)
  - コマンドは `set_todo_repeat(id, repeat)` を足して38個。`repeat` は `RepeatInput { rule, until_date }` か `null`(外す)。次の回は `change_status` が作る。
  - 作ったもの(フェーズ21)
    - `domain/repeat.rs`
      - `next_due(rule, prev_due, today, &rules)`: (1) 前の回の日限より後で周期に合う最初の日。(2) それが今日より前なら、今日以降で周期に合う最初の日(探し始めを `max(前の回の日限の翌日, 今日)` にする)。(3) 営業日の設定では、寄せた日限(`effective_due`)が「前の回の日限以前」または「今日より前」になる日を飛ばす(1つの規則。暦日の設定では何も変わらない)。`every_days` は前の回の日限から数える。毎月の日付は、その月にない日を月末に、毎年の 2/29 は、うるう年でない年は 2/28 にする。第N◯曜日は chrono の `from_weekday_of_month_opt`、最終週は月末から戻る。飛ばすのは400回まで(休日がどれだけ続いても足りる数)。
      - (3) の「今日より前」は、休日に遅れて閉じたときに、作った直後から超過になる回を作らないため(設計書の「過ぎた回を積まない」の意図どおり。オーケストレーターが決定)。テストは、毎日・前の回が木曜・土曜に完了 → 月曜の回と、平日に遅れて閉じたとき(金曜に木曜の回を閉じる → 金曜の回で「本日」)を押さえている。
      - `TodoRepeat::next_round_due(due, today, &rules)`: 次の回の日限。終わりの日(`until_date`)より後なら `None`(同じ日は作る)。日限がない(取り込んだデータだけ)ときも `None`。`TodoRepeat::info` が `TodoItem.repeat`(`RepeatInfo { rule, until_date, next_due }`)を作る。`next_due` は今日閉じたときの次の回の日限。
      - `check_target(todo, until_date)`: 子 → 閉じている → 日限なし → 終わりの日が今の日限より前、の順に断る(`RepeatTargetError` の文をそのまま画面に出す)。`RepeatRuleError` にも画面に出す文を付けた。
      - `RoundDates`(日限と予定期間。`of(todo)`、`shifted(days)`。片方だけの予定期間は、ある方だけずらす)、`shift`、`copied_children(子)`(中止を除く)、`DayOfWeek::of(date)`、`RepeatsByTodo` と `by_todo`。
    - `repo/repeats.rs`: `move_to(from_todo, to_todo, now)`(`todo_id` を書き換える。`created_at` は残る)。`repo/todos.rs`: `touch`(`updated_at` だけを書く)。
    - `commands/repeats.rs`: `set_todo_repeat`(中は `set`)と、`change_status` から呼ぶ `next_round`。付ける・変えるは `check_target` → `RepeatRule::check` の順に見る。外すときは検査しない(閉じたTODOに残った周期も外せる)。付ける・変える・外すは TODO の `updated_at` を変え(付け替えと同じ)、変更履歴には残さない。周期がなくて `null` なら何もしない。
    - `change_status_impl`: 閉じた結果になったら、閉じる前の子を読んでから `close_children`(子の一覧を受け取る形にした)、続けて `next_round`(同じトランザクション)。`next_round` は、周期がなければ何もしない。次の回の日限が決まらなければ(終わりの日を過ぎる・日限なし)周期を消す。決まれば、前の回をずらした日付で写し(`parent_id` とプロジェクトは前の回のまま)、`copied_children` の子を同じ日数ずらして写し、周期を次の回へ移す。写すのはタイトル・優先度・メモと日付だけで、未着手から始める(作業ログ・実績・待ち・中止の項目は写さない)。子の遷移が断られて全体を断ったときは、次の回も作らない。閉じたTODOがアーカイブしたプロジェクトにあっても作る。
    - 変更履歴: 次の回と子の「作成」は `events::repeated(from, …)`(`note` は `{"repeated_from":前の回のid}`、子は前の回の子の id)。`EventNote.repeated_from`(数値)。タイムラインは「繰り返しで作成」。JSON の取り込みでは `repeated_from` が数値でないと `Unreadable`。
    - `update_todo`: 日限があって周期のあるTODOの日限を空にする入力を「繰り返しのあるTODOは、日限を空にできません。繰り返しを外してから日限を消してください。」で断る(日限を動かすのはよい。終わりの日より後にしても断らず、閉じたときに周期が消える)。`set_todo_parent`: 周期のあるTODOを子にするのを「繰り返しのあるTODOは、ほかのTODOの子にできません。先に繰り返しを外してください。」で断る(`check_parent` の後)。
    - `TodoItem.repeat`: 1件は `repo::repeats::get`、一覧・ダッシュボード・ガント・検索は `repo::repeats::list` をまとめて読み、`todo_item(todo, project_name, &family, repeat, today, rules)` に渡す(`gantt::build` と `dashboard::build` は `&RepeatsByTodo` を受け取る)。フェーズ22で周期と依存を `Links` にまとめ、検索も1件と同じ `links_of(ids)` で読むようにした(下の「依存関係」の `views.rs`)。
    - 画面
      - `src/features/todos/repeatText.ts`: 周期の文(`repeatRuleText`。「毎週 月・木」「毎月 第2火曜」「毎月 最終金曜」「毎月 15日」「毎月 月末」「毎年 3/31」「10日ごと」)、「次の回の日限 12/9」(`nextDueText`。ないときは「次の回はありません(終わりの日を過ぎます)」)、「2027/03/31 まで」、印の文(「繰り返し:毎週 月」)、ダイアログの初めの値(`ruleFromDue`。毎週は日限の曜日、毎月(日付)は日限の日、第N◯曜日は日限の週(29日以降は最終)と曜日、毎年は日限の月日、N日ごとは14日)。日付の計算はしない(初めの値の曜日を読むだけ)。Vitest あり。
      - 詳細パネルの「繰り返し」(`detail/RepeatField.tsx`): 親のないTODOだけ。周期の文と印、「次の回の日限 …」と終わりの日、「繰り返しを付ける」「繰り返しを変える」のボタン。押すと詳細パネルの保存待ちの入力を保存してから周期のダイアログを開く。周期がないときの説明は「付けると、完了か中止にしたときに次の回のTODOを作ります。」、日限が入っていなければ押せず「日限を入れると、繰り返しを付けられます。」。閉じたTODOは、周期がなければ欄を出さない。
      - 周期のダイアログ(`detail/RepeatDialog.tsx`): 題は「繰り返しを付ける」「繰り返しを変える」。周期の種類(毎日、毎週、毎月(日付)、毎月(第N◯曜日)、毎月(月末)、毎年、N日ごと)、曜日のチェック、日付・週・曜日・月の選択、日数の欄(整数でなければ 0 を送り、Rust の文を出す)、「終わりの日(任意)」。種類を切り替えても、種類ごとの入力は残る。ボタンは「キャンセル」と「繰り返しを付ける」/「変更する」、変えるときは左に「繰り返しを外す」。エラーはダイアログの上に出す。
      - 閉じたあとの知らせ(オーケストレーターが決定): 詳細パネルで周期のあるTODOを完了・中止にしたら、パネルの上に「次の回(日限 12/7)を作りました。」、閉じる前の `next_due` が `null`(終わりの日を過ぎる)なら「繰り返しの終わりの日を過ぎたため、次の回は作りませんでした。」(`repeatText.ts` の `closedRepeatText`。×で閉じる。別のTODOに切り替えると出さない)。日付は閉じる前の `TodoItem.repeat.next_due` で、詳細パネルの保存のあとのキャッシュから読む(`queries.ts` の `useLatestTodo`)。コマンドの戻り値は変えていない。出すのは `change_status` が通ったときだけで、子も閉じるときの確認や中止の理由のダイアログで「キャンセル」したら何も出さない。ダッシュボードの「完了にする」からは出さない。
      - 一覧のタイトルの右とガントの名前の欄の右に、繰り返しの印(`repeat-mark`。`repeat` のアイコン、読み上げ名「繰り返し」、マウスを載せると「繰り返し:毎週 月」)。ガントの行の読み上げ名の最後に「、繰り返し 毎週 月」。
      - `repeat` のアイコンは `src/components/aizome/Icon.tsx` に足した(Aizome にない。24pxの枠、線幅1.75、丸い線端)。
      - 削除の確認の文は `deleteTodoText(title, children, repeats)` で「繰り返しも止まります。」を添える。
      - フックは `queries.ts` の `useSetTodoRepeat`(`TODO_WRITES` の列に並べる)。
    - サンプル(`sample.rs`): 社内ツール整備に「社内ツールの更新確認」(優先度 低、未着手、日限はモックの 11/26、予定 11/25〜11/26)。周期は毎週、ずらしたあとの日限の曜日(モックの日なら木曜)。日報のフィクスチャとダッシュボードの日限注意は変わらない。社内ツール整備の進捗は 2/8、ガントの行は8行になる。
    - perf(`perf.rs`): 開いている親のないTODO 300件に、7種類の周期を順に付ける(`REPEATS`)。`set_todo_repeat`(周期を行き来)と、子5件のTODOを閉じて次の回を作る `change_status`(前後で作って消すので件数は変わらない。測るのは `change_status` だけ)を測る。
- 依存関係(フェーズ22)
  - 画面とエラーの文の呼び名は「先行TODO」(先に終わらせるTODO)と「後続TODO」(それを待つTODO)にそろえる(利用者が決定)。コードの識別子は `depends_on`(先行)、`dependents`・`todo_id`(後続)、`blocked_by` のまま。
  - コマンドは `add_dependency(todo_id, depends_on_id)`、`remove_dependency(todo_id, depends_on_id)`、`list_dependencies(todo_id)` を足して41個。`todo_id` が後続、`depends_on_id` が先行。`add` と `remove` は後続TODOの `TodoItem` を返す。
  - 止めずに知らせるだけ。`change_status` は先行TODOが終わっていなくても作業中にする(確認は画面側)。依存の区分は増やさない。
  - オーケストレーターが決めたこと: (1) `starts_before_dependency` は、終わっていない(完了でも中止でもない)先行TODOとだけ比べる。終わった先行TODOとの予定の食い違いは警告しない。(2) 作業中にするときの確認は、作業中へ変えるすべての入口に出す(詳細パネルのステータスの選択、ダッシュボードの「作業に戻す」(待ちから)と「着手する」(未着手から))。
  - 作ったもの(フェーズ22)
    - `domain/dependency.rs`
      - `Prerequisite`(後続TODOの id と、先行TODOの id・タイトル・親のタイトル・プロジェクト名・ステータス・予定終了。`repo::dependencies::prerequisites` が JOIN で読む)、`Dependencies`(後続TODOごとに、依存を結んだ順。同じ組を2回渡しても1回)。
      - `blocked_by(todo)`: 開いているTODOだけ。終わっていない(完了でも中止でもない)先行TODOを `BlockingTodo { id, title, parent_title, project_name }` で返す(子の先行TODOは画面が「親 / 子」で出す)。閉じたTODOでは空。
      - `starts_before_dependency(todo)`: 開いているTODOの予定開始が、終わっていない先行TODOのどれかの予定終了より前(両方の日付があるときだけ。同じ日は警告しない)。先行TODOを完了・中止にすると警告は消え、再開すると戻る(`commands/dependencies.rs` のテスト)。
      - `depends_on(id)`: 先行TODOの id(終わったものも)。`GanttRow.depends_on` に入れる。
      - `check_new(todo, first, &既存の依存)`: 自分自身 → 親と自分の子(どちらの向きも) → 同じ組 → 循環、の順に断る(`DependencyError` の文をそのまま画面に出す)。兄弟、ほかの親の子、別のプロジェクト、閉じたTODO、アーカイブしたプロジェクトのTODOは結べる。循環の文は「「…」は、このTODOの後続にあたるため、先行TODOにすると循環になります。ほかのTODOを選んでください。」(後続の後続も「後続にあたる」と呼ぶ)。
      - `creates_cycle`: 先行TODOから、さらにその先行をたどって後続TODOに戻るか(すべての依存をたどる。再帰しない。1本の依存は1回だけ見る)。
      - `positions_among(ids, &依存)`: 両端が `ids` の中にある依存を、`ids` の中の位置の組で返す(テンプレートと繰り返しで使う)。
    - `repo/dependencies.rs`: `among(ids)`(両端が ids の中)、`neighbours(id)`(先行TODOと後続TODOの id。どちらも結んだ順)、`prerequisites(None か Some(ids))`。
    - `views.rs`: `Links { repeats, dependencies }`。`todo_item(todo, project_name, &family, &links, today, rules)`(フェーズ21の `repeat` の引数を `links` にした)。`gantt::build` と `dashboard::build` も `&Links` を受け取る。一覧・ダッシュボード・ガントは `commands::todos::all_links`(周期と依存をまとめて読む)、1件と検索と `list_dependencies` は `links_of(ids)`。1件の `TodoItem` は `commands::todos::items`(親・子・周期・依存を引いて作る。`item` もこれを使う)。
    - `commands/dependencies.rs`: 3つのコマンド。結ぶ・外すは後続TODOの `updated_at` を変え(付け替えと同じ)、変更履歴には残さない。外す依存がなければ「依存が見つかりません。外された可能性があります。画面を開き直してください。」(`error.rs` の `dependency of todo`)。
    - `set_todo_parent`: 依存で結ばれた2件(どちらの向きも)を親子にすることを `ParentError::Linked`(「依存で結ばれたTODOどうしは、親子にできません。先に詳細パネルの「先行TODO」か「後続TODO」から外してください。」)で断る(繰り返しの検査の後)。同じ先行TODOを持つだけの2件は親子にできる。
    - テンプレート: `TemplateBody::from_todos(parent, children, &依存)` が、入れる子(中止を除いた子)どうしの依存を `depends_on` に入れる(位置は小さい順)。入れない子と、一式の外とのつながりは落とす。`save_todo_template` は `repo::dependencies::among` で子どうしの依存を読む。`NewChild.depends_on` を足し、`create_todo` は子を作ってから、位置を作った子の id に直して結ぶ。
    - 繰り返し: `next_round` が、写した子(`copied_children`)どうしの依存を `positions_among` で写す。一式の外とのつながり(外の先行TODOを持つ子、子や親を先行TODOに持つ外のTODO)は写さない。前の回の依存はそのまま残る。
    - 画面
      - `src/features/todos/dependencyText.ts`: 文言(「先行TODOが未完了」、確認のダイアログの題と問い、詳細パネルの見出し「先行TODO」「後続TODO」とボタン「先行TODOを足す」、選択のダイアログの説明)と `asksBeforeDoing`、`blockingTitle`、`pickBlockedReason`。Vitest あり。警告の「先行TODOより前に始まる予定」は `todoText.ts` の `STARTS_BEFORE_DEPENDENCY_TEXT` で、`warningTexts(a, due_after_parent, starts_before_dependency)` の最後に並ぶ。
      - `link` のアイコンを `Icon.tsx` に足した(`repeat` と同じく、24pxの枠、線幅1.75、丸い線端)。
      - 一覧(`TodoTable.tsx`)とガントの名前の欄: `blocked_by` があれば、繰り返しの印の右に `blocked-mark`(`link` のアイコン、読み上げ名「先行TODOが未完了」、マウスを載せると「先行TODOが未完了:要件の確定、…」)。つなぎの行には出さない。ガントの行の読み上げ名の最後に「、先行TODOが未完了(…)」。
      - 詳細パネル(`detail/DependencySection.tsx`、子TODOの欄の下): 「先行TODO」と「後続TODO」の2つの欄(それぞれ別の `section`)。一覧は「親 / 子」の名前、別のプロジェクトならプロジェクト名、ステータス。押すと開き、×で依存を外す(読み上げ名は「「…」との依存を外す」)。ないときは「先行TODOはありません。このTODOより先に終わらせたいTODOがあれば足してください。」「後続TODOはありません。」。`starts_before_dependency` なら「予定開始日が、先行TODOの予定終了日より前になっています。予定を見直してください。」、「先行TODOを足す」のボタン(詳細パネルの保存待ちの入力を保存してから開く)。閉じたTODOにも出す。フックは `queries.ts` の `useDependencies`(キーは `["todos", "dependencies", id]`。TODO の変更で取り直す)、`useAddDependency`、`useRemoveDependency`(どちらも `TODO_WRITES` の列に並べる)。
      - 選択のダイアログ(`detail/DependencyPickerDialog.tsx`、題は「先行TODOを足す」): 語を打って200msで `search_todos` を引く(変換中は引かない。変換中の Esc ではダイアログを閉じない)。結果は「親 / 子」の名前、プロジェクト名、ステータス。自分と、足してある先行TODOは押せない(「このTODO」「足してあります」)。それ以外は押すと結び、循環・親と自分の子は Rust の文をダイアログの上に出す(開いたまま選び直せる)。
      - 作業中にするときの確認(`StatusDialogs.tsx` の `StartBlockedDialog`。幅は `md`): 題は「先行TODOが終わっていません」、文は「次の2件が、まだ完了も中止もしていません。このまま作業中にしますか。」、終わっていない先行TODOを「名前(プロジェクト名)」で並べ、ボタンは「キャンセル」「作業中にする」。詳細パネルのステータスの選択と、ダッシュボードの「作業に戻す」「着手する」から出す(上のオーケストレーターの決定)。閉じたTODOの再開(完了 → 作業中)では `blocked_by` が空なので出ない。
      - ガントの矢印(`geometry.ts` の `arrowBetween`、`dependencyArrows`。Vitest でモックの座標と照合): 先行TODOの予定のバーの終わりから、後続TODOの予定のバーの始まりへ。後続のバーが `2 * ARROW.gap`(16px)以上右で始まるときは1回曲がり、それより左なら、先行のバーの終わりから出て、先行の行の後続側の縁(下なら下の縁、上なら上の縁)を通り、後続のバーの始まりの左から入る。どちらかが表示されていない(完了・中止を隠している、折りたたんでいる、プロジェクトで絞っている)か予定のバーがないときと、描いている行(見えている行と前後の余分)の範囲に縦でかからないとき、表の左右の外に出るときは描かない。ドラッグ中は動かしている位置で引く。線と矢じりは `--text-muted`、選んでいるTODOにつながる矢印は `--primary-text` で少し太く。矢印は読み上げない(`aria-hidden`。行の読み上げ名で伝える)。クリックは下のバーに通す。凡例に「依存」を足した。
    - サンプル(`sample.rs`): 依存を3本。結合テスト計画の作成の先行TODOに移行手順書のドラフト(予定が後なので警告なし)、性能試験の環境準備の先行TODOにAPI設計書のレビュー対応(予定開始 11/17 が先行の予定終了 11/19 より前なので警告。ガントの矢印は回り込む)、新メンバーでの試行の先行TODOにセットアップ手順書の作成(兄弟)。テンプレート「月次締め」の報告書の作成は、ほかの2件が先行(`depends_on: [0, 1]`)。日限注意の区分は変わらないので、日報のフィクスチャとダッシュボードの日限注意は変わらない。`json_data.rs` のテストの `add_v0_3_0_rows` は、サンプルの依存も消してから自分の行を入れる。
    - perf(`perf.rs`): 依存を1,000本(`DEPENDENCIES`。後続TODOの id がいつも大きいので循環はない。たいていは id が60以内の近くのTODO(プロジェクトが順に回るので、別のプロジェクトのことが多い)、1割はずっと前のTODO。親と自分の子の組は飛ばす)。テンプレートの子は1つ前の子が先行。`list_dependencies` と、結んで外す(`add_dependency` + `remove_dependency`。循環の検査がすべての依存を読む)を測る。
- 週報(フェーズ23)
  - コマンドは足していない(41個のまま)。日報の4つのコマンド(`generate_report`、`save_report`、`get_report`、`export_report_file`)が `kind: "weekly"` を受け付ける。`date` は週のどの日でもよく、Rust 側で月曜に寄せる。月報は「月報はまだ作れません。日報か週報を選んでください。」で断る。
  - 作ったもの(フェーズ23)
    - `domain/report_template.rs`: 週報用の型を足した(`WeeklyReportTemplate`、`WeeklySection`、`WeeklySectionBody`、`WeeklySectionKind`、`WeeklyWorkOptions`、`WeeklyClosedOptions`、`NextWeekOptions`。待ちと日限注意の項目は日報の `WaitingOptions`、`AttentionOptions` をそのまま使う)。日報の型を広げなかったのは、同じ `kind`(`work`、`closed`)でも日報と週報で項目が違い、1つの型にすると JSON とバインディングで日報の形が変わるため。正規化・検査は、2つの型に共通のトレイト(`Kind`、`TemplateSection`)の上に1回だけ書いた(`normalize`、`check`)。所感の見出しは `Template` トレイトの `note_heading`。`TemplateError` は節の種類で型を分けた(`TemplateError<K>`)。
    - 週報のテンプレートの JSON の形は設計書の「週報(R-6)」のとおり。題名は `{開始日}` と `{終了日}`(`2026-11-16` の形。`WeeklyReportTemplate::fill_title`)。既定は `週報 {開始日}〜{終了日}`。項目は `log_body` と `follow_up_and_late_start` だけ既定が `false`。
    - settings の `weekly_report_template`: 読み方は `report_template` と同じ(読めない値と検査に通らない値は全体を既定値、読むときに正規化。`repo::settings` の `read_weekly_report_template`)。`SettingsInput.weekly_report_template` を `update_settings` が `checked` で検査し、エラーの文は日報と同じ書き方で「週報の題名を入力してください。」「「来週の予定」の節の見出しを入力してください。」(`commands/settings.rs` の `template_error(e, "週報")`)。「初期値に戻す」の既定は `bindings.ts` の `DEFAULT_WEEKLY_REPORT_TEMPLATE`(`commands::specta_builder` の `constant`)。
    - `domain/weekly_report.rs`: `week_start`(月曜)、`week_end`(日曜)、`Week { start, end, base }`(`base` は日曜と今日の早いほう。先の週を開いたときは今日)、`generate_body(input, &template)`。待ちと日限注意の行は日報の `report::waiting_lines`、`report::attention_lines` に基準日を渡して作る(営業日の書き方と「休日のため前倒し」も日報と同じ)。所感・連絡事項は日報の `report::compose`、`report::split_note`(どちらもテンプレートの型を問わない形にした)。日報の出力は変えていない。
    - 今週の作業: その週の作業ログを、日報と同じ順(プロジェクト、優先度、並び順)でTODOごとにまとめる。行は「- **T**(ステータス / 日限 11/20 / 作業日 11/17・11/19)3.5h」(日限は開いているTODOだけ。作業日は「・」でつなぐ)。作業時間は時間を入れたログだけを足し、日報と同じく、TODOごとの合計が0分より大きいときだけ出す(時間のないログと0分のログだけのTODOには出さない)。節の頭の「作業時間の合計 8.0h」も、週の合計が0分より大きいときだけ出す(オーケストレーターが決定)。合計の行の前後は空行。プロジェクトの見出しを外したときは「- P / **T**…」で、合計の行のあとに空行を1つ置く。本文を出すとき(`log_body`)は、行の下に「  - 11/17」を、その下に本文を4字下げて入れる(同じ日のログは書いた順)。
    - 今週完了・中止: 週の中の変更履歴で完了・中止になり、今もその状態のTODO。閉じた日は、週の中でその状態にした最後の変更の日(完了 → 再開 → 完了なら後のほう)。並びは完了が先、それぞれ閉じた時刻の順。「(11/19、理由:…)」。
    - 来週の予定: 作業中のTODOと、予定開始日が来週の日曜以前の未着手TODO(アーカイブしていないプロジェクト、表示の順)。予定期間は未着手だけ、日限は日限のあるものすべてに添える(設計書の出力例のとおり)。来週の日曜は暦日で数える。
    - `repo/work_logs.rs` と `repo/events.rs` に `list_between(start, end)`(両端を含む)を足した。
    - `commands/reports.rs`: `Period`(日報はその日、週報は月曜〜日曜)。`generate(pool, key, current_md, today)` に今日を足した(週報の基準日のため。日報は使わない)。保存は `reports` の `weekly`(開始日は月曜、終了日は日曜)。.md の既定の名前は `週報_YYYY-MM-DD.md`(月曜の日付)、ダイアログの題は「週報を保存」。
    - フィクスチャ: `tests/fixtures/weekly_report_2026-11-16.md`。日報のテストと同じサンプルに、その週のほかの日の作業ログ(時間のないログ、子TODOのログを含む)と前の週の日曜のログを足し(`commands/reports.rs` のテストの `WEEK_LOGS`)、2026-11-19 に開いた週報と完全一致を確かめる。サンプル(`sample.rs`)は変えていない。
    - 画面
      - `src/features/report/reportText.ts`: 日報と週報の文言(`KIND_TEXT`。前後のボタンは1日・1週、「今日」「今週」、日付の欄の名前「対象日」「対象の週」)、`reportKey`(週報は月曜を鍵にする)、`weekRangeText`(「11/16(月)〜11/22(日)」)。Vitest あり。月曜は `src/lib/date.ts` の `weekStartYmd`(表示と鍵のため。保存の月曜は Rust が決める)。
      - `src/features/report/useReportKind.ts`: 開いている種類(日報か週報か)。アプリを開いている間だけ覚える(`useFolded` と同じ作り)。日付は種類の間で共有し、週報では選んだ日の週を開く。
      - `ReportPage.tsx`: 見出しの下に Aizome の `Tabs`(「日報」「週報」。ガントの表示単位と同じ置き方)。週報では見出しを「週報」にし、横に週の範囲を出す。ボタンは「週報をコピー」、知らせは「週報をクリップボードにコピーしました。」「週報を保存しました。保存先: …」。`useReportDraft(kind, date)` は種類と日付(週報は月曜)で1つの文書を持つ。
      - 設定画面: 「日報のテンプレート」の下に「週報のテンプレート」のカード。`ReportTemplateCard` は `kind`(`daily` / `weekly`)で日報と週報の両方に使う。違うところ(名前、題名の説明、節の名前、添える項目)は `reportTemplate.ts` の `TEMPLATE_SPECS`、`WEEKLY_OPTION_ITEMS`。設定の読み書きは `templateOf`、`templateEdits`。編集の関数(`setTitle` など)は2つの型のどちらにも使える。
    - perf(`perf.rs`): `generate_report (weekly)` を足した。

## v0.4.0 の決めごと

v0.4.0 は `docs/implementation-plan-v0.4.0.md` のフェーズ25〜30で作る。仕様の詳細は設計書の「v0.4.0の詳細」にある。フェーズごとに作ったもの(ファイルの場所と細かい決めごと)は、各フェーズの終わりにここへ足す。

- 作業のブランチは `dev-0.4.0`(資料の PR を main に取り込んでから main から切る。リリースのタグ `v0.4.0` と同じ名前にしない)。
- 表は変えない(マイグレーションを足さない)。`schema_version` は2のまま。新しい値は `settings` のキーに入れる。
- ウィンドウを開かずにTODOを追加する機能(T-13)は将来に回した(利用者が決定)。相談で出た案は設計書の「v0.4.0の詳細」の「将来に回したもの」にある。v0.4.0 では global-shortcut のプラグインを入れない。
- v0.4.0 で足すコマンドは次の7つで、コマンドは全部で48個(v0.3.0 までの41個と合わせて)。表示倍率は `update_settings` で扱う。

  | コマンド | フェーズ | 用途 |
  | --- | --- | --- |
  | `choose_backup_mirror` | 27 | 複製先のフォルダを選ぶ(ダイアログは Rust 側)。選んだらすぐ複製し、成功したときだけ保存する |
  | `clear_backup_mirror` | 27 | 複製をやめる(複製先のファイルは消さない) |
  | `restore_backup_file` | 27 | ファイルを選んで復元する(ダイアログは Rust 側)。検査と入れ替えは今の復元と同じ |
  | `preview_dependent_shift` | 28 | ガントのドラッグで予定を後ろへ動かしたときに、ずらす後続TODOと新しい予定を返す |
  | `shift_with_dependents` | 28 | ドラッグしたTODOと後続TODOの予定期間を1トランザクションで書く |
  | `preview_bulk_change` | 29 | 一括変更で、変えるもの・除くもの(理由つき)・知らせを返す |
  | `apply_bulk_change` | 29 | 一括変更を1トランザクションで行う |

- `settings` に足すキー

  | キー | 値 | 既定値 | フェーズ |
  | --- | --- | --- | --- |
  | `zoom` | `70` / `80` / `90` / `100` / `110` / `125` / `150` | `100` | 26 |
  | `backup_mirror_dir` | 複製先のフォルダ(絶対パス)か `null` | `null` | 27 |
  | `backup_mirror_last_ok` | 複製先にそろえられた、いちばん新しい毎日のバックアップの日付か `null` | `null` | 27 |

  - `backup_mirror_dir` と `backup_mirror_last_ok` は `update_settings` の入力(`SettingsInput`)に入れない(複製先は Rust 側のダイアログで選んだものだけを保存する)。
  - 復元と取り込みでは、`zoom`、`backup_mirror_dir`(どちらも PC ごとの値)と `backup_mirror_last_ok`(アプリが書く値)を今の値のまま残す(`KEPT_ON_REPLACE` に足す)。
- 表示倍率は webview の `set_zoom` で画面ごと変える。Aizome の写し(`tokens.css`、`aizome.css`)は書き換えず、幅で切り替える指定は `src/styles/app.css` に置く。反映は `update_settings` の `#[tauri::command]` の層で行い、起動時はウィンドウを出す前に当てる。
- バックアップの複製は毎日のバックアップだけ。複製先の直下に `todo-YYYY-MM-DD.db` の名前で置き(`.part` に書いてから名前を変える)、消すのはその名前のファイルと自分が残した `.part` だけ。複製できなくても本体のバックアップは成功のまま。常駐のタイマーは複製先をメモリに持ち、DB を読まずにやり直す。
- 後続TODOのずらしは「足りない分だけ」(利用者が決定)。入口はガントのドラッグだけ。動かすのは予定期間だけで、日限は動かさない。計算は `domain/` の純粋関数で、確認と保存で同じ関数を使い、保存のときに計算し直す。
- 一括変更で変えられないものは、確認の時点で理由を示して除き、残りで進める(利用者が決定)。保存の途中の想定外の失敗だけは全体を断る。ステータスは1件ずつ今の遷移を通す(子も閉じ、繰り返しの次の回も作る)。待ちと削除は入れない。先行TODOが終わっていないTODOを作業中にするときの確認は、一括変更のダイアログの確認の欄で出す。
- ずらす日数は、後続のずらしも一括変更も暦日で数える(ガントのドラッグと同じ)。
- 表示倍率(フェーズ26)
  - コマンドは足していない(41個のまま)。`update_settings` が `SettingsInput.zoom` を受け取る。
  - 作ったもの(フェーズ26)
    - `domain/settings.rs`: `Settings.zoom`(百分率)、`ZOOM_LEVELS`(`[70, 80, 90, 100, 110, 125, 150]`)、`DEFAULT_ZOOM`(100)、`is_zoom_level`、`zoom_factor`(100 → 1.0)。`ZOOM_LEVELS` と `DEFAULT_ZOOM` は tauri-specta の `constant` で `bindings.ts` に出す(`commands::specta_builder`)。画面の選択肢はこれから作る。
    - `repo::settings`: キー `zoom`。一覧にない値と読めない値(`125.0`、`"125"`、`null` など)は既定値の100として読む(`read_zoom`)。`save_user_settings` が書く。`KEPT_ON_REPLACE` は5つ(`zoom` を足した。フェーズ27で7つ)。
    - `commands/settings.rs`: 一覧にない値は「表示倍率は一覧から選んでください。」で断る(ほかの設定の検査のあと、保存の前)。
    - 反映: `resident.rs` の `Zoom`(今の倍率を `AtomicU32` で持つ managed state)、`apply_zoom`(メインのウィンドウの webview に `set_zoom(zoom_factor(..))`)。`resident::apply`(`update_settings`、復元、取り込みのあと)は、倍率が変わったときだけ当て直す。起動時は setup で設定を読んだ直後(ウィンドウを出す前)に当てる。さらに、メインの webview のページの読み込みが終わるたびに当て直す(`lib.rs` の `on_page_load` → `resident::on_page_loaded`。WebView2 はサイトごとに倍率を覚え、ページの読み込みで戻すことがあるため)。当てられなかったときは動作ログに書くだけ。
    - Ctrl+ホイールなど WebView の標準の拡大・縮小は今までどおり効かない(`zoomHotkeysEnabled` は既定の false のまま)。`npm run dev:web` では倍率は効かない。
    - macOS(WKWebView の `pageZoom`)では、倍率を上げると CSS の幅が減って並びが組み直されることを確かめた(150%・幅1280でダッシュボードの件数のカードが5列から3列+2列に折り返す)。Windows(WebView2 の `ICoreWebView2Controller::put_ZoomFactor`)はブラウザの拡大と同じ仕組みで、`devicePixelRatio` が変わって組み直される(実機ではまだ見ていない)。
    - 画面: 設定の「表示」のカードの「配色」の下に「表示倍率」の `Select`(`src/features/settings/zoomText.ts` の `zoomOptions`。「70%」〜「150%」、100は「100%(標準)」。説明は `ZOOM_DESCRIPTION`。Vitest あり)。選んだらすぐ `useUpdateSettings` で保存し、Rust が反映する。`settingsInputOf` に `zoom` を足した。
    - 幅の備え(`src/styles/app.css`。Aizome の写しは変えていない): 150% では幅1280のウィンドウが853px、最小の960×600が640×400px 相当になる。
      - `.app__main > *` に `min-width: 528px`(ガントの名前の欄302pxと1週間分の日付)。それより狭いと `.app__main`(`overflow: auto`)の中で横にスクロールする。サイドナビと詳細パネルは動かない。
      - 1280px 未満(`max-width: 1279.98px`)では、詳細パネルを右に重ねて出す(`position: absolute`、幅440px、`max-width: calc(100% - サイドナビの幅)`、`--shadow-lg`、`z-index` はガントの見出しの sticky より1つ上)。並べて出すと一覧とガントが数列分しか残らないため。TODO一覧の表には `min-width: 840px` を付け、セルを何行にも折らずに表の枠の中で横にスクロールさせる(ダッシュボードの日限注意の表は、もとから横にスクロールする)。表の外側の要素に `todo-table` のクラスを足した(`TodoTable.tsx`)。1280px 以上(100%の既定の大きさ)は今までと同じ(パネルは横に並び、一覧の列は詰まる)。
      - ガントの画面(`.app__main--fill`)は `overflow: hidden` をやめた。高さが足りないとき(150%では見出し・絞り込み・凡例で半分ほどを使う)は画面が縦にスクロールし、ガントは `min-height: 240px` を保つ。
      - 起動時リマインドの一覧(`max(88px, min(400px, 50vh, 100vh - 300px))`)、検索の結果(`max(96px, min(480px, 55vh, 100vh - 250px))`)、先行TODOの選択の結果(`max(96px, min(320px, 100vh - 330px))`)は、背の低いウィンドウでは一覧の高さを先に縮める。「確認しました」などのボタンが、ダイアログの中をスクロールしなくても見えるようにするため。ほかのダイアログは、Aizome のとおり画面の高さを超えると中がスクロールする。
      - 画面を切り替えたら `.app__main` のスクロールを左上に戻す(`App.tsx`)。画面どうしで同じ要素を使うので、前の画面のスクロールが残り、ガントの期間のボタンなどが上に隠れたまま開くことがあったため。
      - 150%・幅1280で、ダッシュボードの日限注意の表と TODO一覧の表が枠の中で横にスクロールするのは、このままでよい(列をまとめる作りにはしない。利用者が決定)。
- バックアップの複製先(フェーズ27)
  - コマンドは `choose_backup_mirror`、`clear_backup_mirror`、`restore_backup_file` を足して44個。どれもダイアログを開く `#[tauri::command]` と、場所を受け取る内側の関数(`commands/backup.rs` の `choose_mirror`、`clear_mirror`、`restore_file`)に分け、テストは内側の関数を作業用のフォルダで呼ぶ。ダイアログは `json_data.rs` の `pick`(`spawn_blocking`)で開く。3つとも、終わったら `resident::apply` で常駐のタイマーに設定を渡す。
  - 作ったもの(フェーズ27)
    - `domain/backup_mirror.rs`: `missed(last_ok, 毎日のバックアップの日付)`(`last_ok` より後の個数。`null` ならすべて)、`behind`(1個以上。タイマーがやり直すか)、`alert(複製先があるか, last_ok, 日付)`(3個以上(`ALERT_DAYS`)で `BackupMirrorAlert { last_ok, days }`)。日数は起動した日(毎日のバックアップがある日)で数える。
    - `domain/settings.rs`: `Settings.backup_mirror_dir`(文字列か `null`)、`backup_mirror_last_ok`(日付か `null`)。`SettingsInput` には入れない。`repo::settings`: 絶対パスでない値と読めない値は `null` として読む。書くのは `set_backup_mirror(dir, last_ok)`(2つを一緒に。選んだ・やめたとき)と、タイマーの `set_backup_mirror_last_ok(dir, last_ok)`(DB の複製先が `dir` のままのときだけ書く。複製している間に複製先を変えられても、新しい複製先の日付を上書きしない)。`save` も2つを書く。`KEPT_ON_REPLACE` は7つ。
    - `backup.rs`
      - `sync_mirror(data_dir, 複製先)`: (1) 複製先がない(フォルダでない)ときは失敗にし、フォルダを作らない(抜いたドライブの場所に作らないため)。(2) 複製先の `todo-YYYY-MM-DD.db.part` を消す。(3) `backups/` の新しい7個のうち、複製先に同じ名前のファイルがないものを `.part` に写し、`sync_all` してから名前を変える(失敗したら `.part` を消す)。(4) 複製先の `todo-YYYY-MM-DD.db` を新しい7個に減らす(`names_to_prune`。`before-restore-…` の名前やほかのファイル、フォルダには触れない)。戻り値は `backups/` のいちばん新しい毎日のバックアップの日付。ファイルの操作は同期なので、非同期の側からは `backup::mirror`(`spawn_blocking`)で呼ぶ。
      - `daily_dates(data_dir)`: `backups/` の毎日のバックアップの日付(新しい順)。
      - `is_within(path, base)`: 両方を `canonicalize` して、大文字・小文字を区別せずに部品で比べる(`..` やリンクも解く)。複製先にデータフォルダとその中を選べないようにするのと、今使っている `todo.db` を復元に選べないようにするのに使う。
      - 復元は `restore`(一覧の名前)と `restore_file`(選んだファイルの場所)から、中の `restore_from` に分けた。`restore_file` は、データフォルダの今使っている `todo.db` と、同じフォルダの `todo.db-wal`、`todo.db-shm` を `BackupError::InUse` で断る(ただのファイルとして写すと、`-wal` に残っている最近の変更がない状態に戻ることがあるため。オーケストレーターが決定)。比べるのは `canonicalize` したパス(`is_within`)なので、大文字・小文字の違う名前や、`..`、シンボリックリンク経由で選ばれても断る。選んだファイルを写せないときは `BackupError::Source`。選んだファイルは変えない(作業用のコピーにマイグレーションを当てる)。`backups/` の中のファイルも選べる。
    - `commands/backup.rs`
      - `choose_mirror`: データフォルダとその中は「アプリのデータフォルダの中は選べません。…」で断る。すぐに複製し、成功したときだけ2つのキーを保存する。失敗は「選んだフォルダに複製できませんでした。書き込めるフォルダか確かめてから、もう一度選んでください。」。戻り値は `BackupMirror { dir, last_ok }`(`dir` はダイアログが返したパスのまま)。前の複製先のファイルは消さない。
      - `clear_mirror`: 2つのキーを `null` にする。複製先のファイルは消さない。
      - `restore_backup_file`: `.db` の絞り込みのファイルのダイアログ。戻り値はファイル名。エラーの文は一覧からの復元と同じ(`restore_error`)。今使っている `todo.db` は「今使っているデータのファイルは選べません。バックアップのファイル(todo-2026-11-18.db など)を選んでください。」、読めないファイルは JSON の取り込みと同じ「ファイルを読めませんでした。…」。
      - 動作ログには、選んだ・やめた・復元したことと、失敗の種類(`io::ErrorKind`)だけを書く。パスは書かない(選んだファイルの復元は「a chosen file」と書く)。
    - 常駐のタイマー(`notifier.rs`): `MirrorSettings`(複製先、`last_ok`、その日に失敗を書いたか、前回が失敗だったか)を `Notifier` がメモリに持つ。起きるたびに `mirror_if_behind` が、`backups/` の一覧(DB ではない)と `last_ok` を `backup_mirror::behind` で比べ、そろっていなければ `backup::mirror` で写す。写せたら `set_backup_mirror_last_ok` で保存し、メモリも進める。写せなければ、次に起きたとき(最長10分後)にやり直す。動作ログは、失敗をその日の最初の1回だけ、前回が失敗で今回写せたら「works again」を1回、ふだんの成功は1日1回の「copied」。`Notifier::update` は、同じ複製先なら `last_ok` を新しいほうにし(タイマーが書く前に読んだ設定で戻さない)、違う複製先(か、なし)なら入れ替える。
    - 起動時の複製は、setup の中では待たず、setup の最後に動かすタイマーの最初の起床(すぐに起きる。`run_daily` のあと)で行う(オーケストレーターが決定)。遅いドライブや、つながらないネットワークのフォルダで、ウィンドウが出るのが遅れないようにするため(非機能要件の「起動からダッシュボードまで2秒」も守る)。タイマーはウィンドウを出すかどうかにかかわらず動くので、`--autostart` でトレイだけで起動したときも起動の直後に写す(作業用の HOME で .app を `--autostart` つきで起動して確かめた)。
    - タイマーが写したことは画面に送らない(画面にイベントの権限を付けないため)。設定画面とダッシュボードは、ウィンドウが前面に来たときと画面を開き直したときに取り直すので(TanStack Query の既定)、そこで知らせが消え、最後に複製できた日が進む。
    - ダッシュボード: `Dashboard.backup_mirror_alert`。`dashboard::build` は `null` のままにし、`get_dashboard` の中(`commands/dashboard.rs` の `with_backup_alert`)で `backups/` の一覧と settings から入れる。常駐のタイマーの件数(`dashboard::get`)は一覧を読まない。
    - 画面
      - `backupText.ts`: `MIRROR_INTRO`、`mirrorDirText`(「複製先: …」)、`mirrorLastOkText`(「最後に複製できた日: 11/19」。まだなければ「まだありません」)、`mirrorAlertText`(日数は `days`。`last_ok` が `null` なら括弧の「最後に複製できた日」を出さない。「まだありません」と合わせてオーケストレーターが決定)、`restoredFileText`。Vitest あり。
      - 設定の「データ」のカード: バックアップの文とボタンの下に「バックアップの複製先」のまとまり(`SettingsPage.tsx` の `BackupMirrorSection`)。複製先がないときは説明と「複製先を選ぶ」、あるときは複製先のパス(長いパスは折り返す。`app.css` の `.backup-mirror__dir`)、最後に複製できた日、「複製先を変える」「複製をやめる」。知らせの条件に当たるときは、まとまりの見出しの下にダッシュボードと同じ文の `Alert`(warning。`useDashboard` の `backup_mirror_alert`)。エラーはまとまりの中に出す。「複製をやめる」に確認は挟まない(もう一度選べば戻り、ファイルも消さないため)。
      - 復元のダイアログ(`RestoreDialog`): フッターの「キャンセル」と「復元する」の間に「ファイルを選んで復元」。ファイルを選ばずに閉じたら、復元のダイアログは開いたまま。復元したら知らせ(`restoredFileText`)を出し、すべての一覧を取り直す。
      - ダッシュボード: 見出しのすぐ下に、`backup_mirror_alert` があれば `Alert`(warning、×なし)。
      - フックは `queries.ts` の `useChooseBackupMirror`、`useClearBackupMirror`(設定とダッシュボードを取り直す)、`useRestoreBackupFile`(すべての一覧を取り直す)。
    - perf(`perf.rs`): 毎日のバックアップ7個を空の複製先に写す時間と、知らせつきの `get_dashboard` を測る。

## 品質の決めごと

- `domain/` は単体テストを書く。日付の境界(当日、N日後ちょうど、月またぎ・年またぎ)と、遷移表の全行を押さえる。
- `repo/` はメモリ上の SQLite にマイグレーションを当ててテストする。
- 画面側は、ガントの座標計算など計算を含む部分を Vitest でテストする。
- ライブラリのバージョンは記憶に頼らず、導入時点の最新安定版と互換性を確認する。tauri-specta が入れた Tauri の版で動かない場合は、手書きの型定義に切り替えて報告する。
- tauri-specta は Tauri 2 向けの安定版がないため、RC 版を使う。tauri-specta、specta、specta-typescript は互換性のある組を `=` で固定する(今は `tauri-specta =2.0.0-rc.29`、`specta =2.0.0-rc.28`、`specta-typescript =0.0.15`)。上げるときは3つを同時に上げ、まずコマンド1つで型の生成と呼び出しを確かめてから全体に広げる。
- Tauri の権限(`capabilities/default.json`)と CSP は、使うものだけに絞る。

## 進め方

- `docs/implementation-plan.md`(MVP、フェーズ0〜8)、`docs/implementation-plan-v1.1.md`(v1.1、フェーズ10〜16)、`docs/implementation-plan-v0.3.0.md`(v0.3.0、フェーズ17〜24)、`docs/implementation-plan-v0.4.0.md`(v0.4.0、フェーズ25〜30)のフェーズ順に進め、フェーズの終わりにテスト結果と、利用者に画面で確認してほしい操作の一覧を報告して止まる。
- v1.1、v0.3.0、v0.4.0 はオーケストレーター方式で進める。フェーズごとにサブセッションが作業してコミットし、オーケストレーターが差分を確かめてから利用者に報告する(`implementation-plan-v1.1.md`、`implementation-plan-v0.3.0.md`、`implementation-plan-v0.4.0.md` の「進め方」)。
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
| Rust の書式の確認(直すときは `cargo fmt --manifest-path src-tauri/Cargo.toml`) | `npm run fmt:rust` |
| Rust の lint(警告はエラー扱い) | `npm run lint:rust` |
| まとめて確認(型チェック、Rust の書式、Vitest、cargo test、clippy) | `npm run check` |
| 性能の確認(TODO 5,000件・作業ログ 20,000件でコマンドの時間を測る。最適化ビルド。検索、営業日の設定、TODO の保存(`get_todo` と `update_todo`)、テンプレート、繰り返し、依存関係、週報、起動の Rust 側、JSON の書き出しと取り込みも測る。計測のデータには子TODO・繰り返し・依存関係も入る) | `npm run test:perf` |
| アプリアイコンを原画から作り直す | `npm run icons` |
| 配布物のビルド(実行ファイルの ZIP を `release/` に作る) | `npm run build` |

- 配布物は実行ファイルの ZIP で、インストーラーと .dmg は作らない。`npm run build`(`scripts/package.mjs`)が、Windows では `Higiri.exe`、macOS では ad-hoc 署名した `Higiri.app` を ZIP にして `release/` に置く(macOS は `ditto` で固めて、署名と実行権限を保つ)。リリースは `v` で始まるタグ(`v0.1.0` など。版は `tauri.conf.json` の `version` と揃える)を push すると、`.github/workflows/release.yml` が下書きのリリースを作り、Windows と macOS の ZIP を添付してから公開する。リリースの説明は `.github/release-notes.md` で、タグを push する前に「vX.Y.Z で足したこと」「直したこと」の節をその版の中身に書き直す。`windows.yml` と `macos.yml` は手動で動かす確認用で、ZIP を包み直さずに(`archive: false`)成果物に上げる。どれも同じスクリプトで ZIP を作り、Rust のキャッシュを共有する(ジョブ名を `build` で揃えている)。Actions では Vitest と cargo test を通してから作る(型チェックは build:web の中で行い、clippy と Rust の書式の確認は手元の `npm run check` だけで動かす)。配布版の設定(`[profile.release]`)は、ビルド時間を優先して LTO を使わない。改行は `.gitattributes` で LF に揃えている(生成物とフィクスチャをバイト単位で比べるテストがあるため)。
- アプリアイコンは D3 案(藍の判子に白いチェック、右上に日限を表す朱のひし形。色は Aizome の `sumi-50`・`ai-700`・`shu-500`)。原画は `src-tauri/icons/source/` の SVG で、`higiri-icon.svg`(全面。PNG と .ico の 48〜256px)、`higiri-icon-small.svg`(16〜32px 用に内側の枠を省き、チェックを太くした簡略版)、`higiri-icon-macos.svg`(Apple のグリッドに合わせた .icns 用)。トレイのアイコンは、Windows が `higiri-icon-small.svg` の 32px(`tray-windows.png`)、macOS がメニューバー用の単色テンプレート画像 `higiri-tray-macos.svg` の 36px(`tray-macos.png`、18pt の2倍)で、どちらも `tray.rs` に埋め込む。`npm run icons`(`scripts/icons.mjs`)で `src-tauri/icons/` を作り直す。生成されたファイルは手で編集しない。
- `README.md` の画面の画像は `docs/readme/`(v0.3.0 で撮った)。撮り直すときは、`npx tauri build --debug --bundles app` で作った `Higiri.app` の中の実行ファイルを、`HOME` を作業用のフォルダに差し替えて `--sample-data` つきで起動し(利用者の開発用の DB に触れない)、`screencapture -x -o -l <ウィンドウの番号>` で窓だけを撮って、`sips -Z 1600` で縮める。
- 開発ビルドの DB は、配布版と混ざらないように、データフォルダの下の `dev/todo.db` を使う(macOS は `~/Library/Application Support/com.redjet.higiri/dev/`、Windows は `%APPDATA%\com.redjet.higiri\dev\`)。サンプルデータを入れ直すときは、アプリを閉じてこの `dev` フォルダを消してから `npm run dev:sample` で起動する。
- サンプルデータはモックの3プロジェクト・11件に、v0.3.0 の機能を試すデータ(子TODOのある親1件と子3件、繰り返しのTODO 1件、依存3本、テンプレート「月次締め」)を足したもの(TODO は16件)。モックと見比べる11件の表示(ダッシュボードの日限注意、日報)は変えていない。日付は「モックの今日(2026-11-19)」が実際の今日になるようにずらして入れる。ステータスの変化は `change_status` と同じ遷移関数で再現するので、変更履歴も残る。
