# 実装計画(v0.5.0)

v0.5.0 の3つの要件(A-1 CLIでの読み取りと、CLIに出さないプロジェクトの設定、A-2 CLIでの書き込み、A-3 Claude Code の Skill)を、フェーズ31〜36に分ける。読み取りを先にするのは、出力の形(`CliTodo`)を書き込みのサブコマンドでも返すため。画面(取り直しと設定のカード)と配布を Skill より先にするのは、Skill を試すときに、配布と同じ形の CLI と、CLI の変更がすぐ出る画面で確かめたいため。

仕様は `requirements-design.md` の「v0.5.0の詳細」にある。この計画は、フェーズごとの作業と完了条件だけを書く。フェーズ31(資料の詳細化)で、この計画と、設計書の v0.5.0 の記述を書く。

## 進め方

**体制(オーケストレーター方式)**

v0.4.0 と同じ。

- 作業場所は `dev-0.5.0` ブランチ。この資料の PR を main に取り込んでから、main から切る。リリースのタグ `v0.5.0` と同じ名前にしない。
- 計画を持つセッション(オーケストレーター)はコードを書かない。フェーズごとにサブセッションを1つ立て、作業はサブセッションが行う。
- サブセッションは同時に1つだけ動かす。フェーズの間で同じファイル(`src-tauri/src/cli/`、`Cargo.toml`、`commands/todos.rs`、`repo/db.rs`、`repo/settings.rs`、`perf.rs`、`scripts/package.mjs`、`README.md`)を触るため。worktree は使わず、同じフォルダで作業する。

**サブセッションがすること**

- 最初に `CLAUDE.md`、この計画、設計書の「v0.5.0の詳細」を読む。
- そのフェーズだけを行う。設計にない判断が要るときは、進める前にオーケストレーターに聞く。
- 終わったら `npm run check` を通し、`dev-0.5.0` にコミットする。push はしない。
- フェーズの終わりに、次の3点を報告して止まる。
  - テストと型チェックの結果
  - 利用者に確認してほしい操作の一覧(`npm run dev:sample` で入れたデータに、`--dev` を付けた CLI を当てて試す手順)
  - 設計書から変えた点
- フェーズで決まったファイルの場所や決めごとは、`CLAUDE.md` の「v0.5.0 の決めごと」に足す。

**オーケストレーターがすること(各フェーズの後)**

1. 差分を読み、`CLAUDE.md` の決めごと(層の分け方、時計を読む場所、文言)に合っているかを確かめる。CLI が内側の関数だけを通り、`cli/` に SQL がないことも確かめる。
2. `npm run check` を自分でも通す。
3. 直す点があれば、同じサブセッションに頼む。
4. 結果と、確認してほしい操作を利用者に報告して止まる。
5. 利用者の「次へ」で、次のフェーズのサブセッションを立てる。

**そのほか**

- サブセッションからの質問は、この計画と設計書で答えられるものはオーケストレーターが答え、答えられないものは利用者に聞く。
- 次のことは、利用者に確認してから行う。
  - main への取り込み
  - 版の番号(`tauri.conf.json` の `version`)を 0.5.0 に上げること
  - push、タグ、リリース、ワークフローの実行
  - 利用者の `~/.claude/skills/` に Skill を書くこと(試すときは、作業用の `CLAUDE_CONFIG_DIR` か、利用者の許しを得てから)
- CLI を試すときは、利用者の配布版の DB に触れない。`--dev` を付けて開発用の DB に当てるか、テストのように作業用のフォルダで行う。
- Windows での確認(コンソールへの出力、ZIP から出した CLI、Git Bash からの呼び出し)は、Actions の `windows.yml` で作った成果物を利用者に試してもらう。

**全フェーズで守ること**

- `npm run check`(型チェック、Rust の書式、Vitest、cargo test、clippy)を通す。日報と週報のフィクスチャとの完全一致のテストは、全フェーズで通ったままにする(日数の文を日報の関数から取り出すときも、日報の出力を変えない)。
- 画面のコマンドは足さない(48個のまま。件数のテストも変えない)。`capabilities/default.json` は変えない。`bindings.ts` は、settings に足す `cli_hidden_projects` の分(`Settings` と `SettingsInput`)だけが変わる。
- 表は変えない。settings に足すキーは `cli_hidden_projects` だけ。ほかに変えたくなったら、オーケストレーターに相談する。
- CLI は内側の関数だけを通す。`cli/` に SQL を書かない(DB の版の検査は `repo/db.rs` に置く)。時計は `commands::clock` で読む。
- CLI の出力と文言は設計書のとおりにする。利用者向けの文は日本語で、「何が起きたか+どうすればよいか」。
- `perf.rs` に、各フェーズのサブコマンドの計測を足す。一覧とガントは0.1秒以内のまま。

## フェーズ32 CLI の土台と読み取り

設計書の「CLIの形」「DBの開き方」「呼び方と出力の決まり」、「サブコマンド」の「読む」と、「CLIに出さないプロジェクト」の値とCLIでの扱い(設定画面のカードはフェーズ34)。

**最初に確かめること**(問題があれば、進める前にオーケストレーターに相談する)

- 同じパッケージに2つ目の実行ファイル(`src-tauri/src/bin/higiri-cli.rs`)を足しても、`npm run dev`、`npm run dev:sample`、`npm run build` が今までどおり動くこと(`Cargo.toml` に `default-run = "higiri"` が要るか)。
- macOS で、`tauri build --bundles app` が `Higiri.app/Contents/MacOS/` に `higiri-cli` を入れるか(入らなければ、フェーズ34で `package.mjs` が入れてから署名し直す)。
- `higiri-cli` の実行ファイルの大きさ(Tauri を使わないので、大きくなりすぎないか)。

**Rust**

- clap を足す(版は導入時点の最新安定版。derive で書く)。
- `repo/db.rs`: DB を作らず、マイグレーションを当てずに開く関数と、`_sqlx_migrations` の最新の番号を `MIGRATOR` と比べる検査。断る3つの場合と、待っても書けなかったときの文は設計書のとおり。
- データフォルダ: 配布版と同じ場所を、Tauri の API を使わずに求める(`--dev` で `dev/`)。
- settings に `cli_hidden_projects` を足す(読めない値は `[]`)。`SettingsInput` に足し、保存のときに、ないプロジェクトのidを除いて小さい順に並べ、重複を除く。`commands::projects::delete` で、消したプロジェクトのidを同じトランザクションで外す。`KEPT_ON_REPLACE` には入れない。
- `cli/`
  - 引数(サブコマンドと、共通の `--dev`)、出力(標準出力の JSON と、標準エラー出力のエラー、終了コード)。
  - 出力の形(`CliTodo` と、`show`・`today`・`search`・`projects` の形)。`TodoItem` から作る。日本語の名前は `Status::label`、`Priority::label`、`Urgency::label` を使う。
  - `urgency.text` の文は、日報の日限注意の行の括弧の中を作る関数を `domain/report.rs` から取り出して使う(日報の出力は変えない)。
  - `today`、`list`、`show`、`search`、`projects`。中身は画面のコマンドの内側の関数(ダッシュボード、一覧、1件、タイムライン、依存、検索、プロジェクト)を呼ぶ。
  - 出さないプロジェクトの扱い(設計書の「CLIでの扱い」)。`today` と `search` は、除いてから集計・並べ替えをする(件数と100件の切り方を、除いたあとで決める)。別のプロジェクトの先行TODO・後続TODOは `{ hidden: true }`。

**テスト**(メモリ上の DB か作業用のフォルダで)

- 版の検査: DB がない、DB の版が古い・新しい、で断り、何も書かない(作らない)。
- 出力の形: サンプルデータで、日限注意の区分・日数・`urgency.text` がダッシュボードと日報と同じになる。暦日と営業日の設定の両方。
- `list` の引数が画面の一覧と同じ絞り込み・並べ替えになる。`--limit` と `total`。アーカイブしたプロジェクトは `--project` で指したときだけ。
- `search` の出力に、メモと作業ログの本文(前後の文)が入らない。
- `show` の作業ログの件数(`--logs`)と、子・先行・後続。
- エラーの出し方(`kind` と終了コード)。
- `cli_hidden_projects`: 読み取り(既定値、読めない値)、保存(ないidを除く、並べる、重複を除く)、プロジェクトの削除で外れる、復元と JSON の取り込みでファイルの値に入れ替わる。
- 出さないプロジェクト: `today` の件数と一覧から除かれる、`list`(`--project` で指すと `hidden` で断る)、`show` を `hidden` で断る、`search` の `total` が除いたあとの数、`projects` の `hidden_projects`、別のプロジェクトの先行TODOが `{ hidden: true }` になる。
- `perf.rs`: 5,000件で `today`、`list`、`show`、`search`(0.5秒以内)。

完了条件: 上のテストが通る。`npm run dev:sample` で入れたデータに `cargo run --bin higiri-cli -- --dev today` などを当てると、画面と同じ区分・日数・件数が出る。settings の `cli_hidden_projects` に入れたプロジェクトのTODOは、どのサブコマンドにも出ない。要件 A-1(設定画面のカードを除く)。

## フェーズ33 書き込み

設計書の「サブコマンド」の「書く」と、「書き込みのしかた」。

**Rust**

- `add`、`update`、`status`、`log`、`append-memo`。それぞれ `commands::todos::create`、`commands::todos::update`(今の TODO に指した項目だけを重ねる)、`commands::todos::change_one`、`commands::work_logs::save`、`commands::todos::update`(メモだけを書き足す)を呼ぶ。
  - `change_one` は今 `pub(super)` なので、`cli/` から呼べるように公開の範囲を `pub(crate)` に広げる(中身は変えない)。
- 確認が要る操作(`--despite-blocked`、`--with-children`)と `needs_confirmation` の文。
- `status` の出力の `closed_children`、`next_round`、`repeat_ended`。
- 書く前のバックアップ(その日の毎日のバックアップがなければ `backup::run_daily`。作れなければ書かずに断る)。
- `log --body -` と `append-memo --body -` の標準入力の読み取り。
- 出さないプロジェクトのTODOへの書き込みと、出さないプロジェクトへの追加を `hidden` で断る。

**テスト**

- `add`: 子TODO(同じプロジェクト、孫にしない)、アーカイブしたプロジェクトへの追加を断る、プロジェクトを名前で指す(同じ名前が2つなら断る)。
- `update`: 指した項目だけが変わり、ほかの項目(メモを含む)は変わらない。変更履歴は画面の `update_todo` と同じ。繰り返しのあるTODOの日限を空にするのを断る。
- `status`: 変更履歴・実績日の自動入力が画面の `change_status` と同じ。待ち(相手が要る、確認日の初期値)、中止(理由が要る)。先行TODOが終わっていないときと、開いている子があるときに、引数がなければ断り何も変えない。引数があれば子も閉じ、`closed_children` に出る。繰り返しの次の回が `next_round` に出る。終わりの日を過ぎたときの `repeat_ended`。
- `log`: 日付の初期値が今日、空の本文と範囲外の作業時間を断る。
- `append-memo`: 空のメモと、末尾に空行や空白のあるメモへの書き足し(空行を1つ挟む)。前の部分とほかの項目が変わらない。空の本文を断る。変更履歴の行が増えない。
- 出さないプロジェクト: 5つの書くサブコマンドが `hidden` で断り、何も変えない(バックアップも作らない)。
- バックアップ: その日のバックアップがなければ書く前に作る。あれば作らない。読むサブコマンドでは作らない。作れないときは書かない。
- `perf.rs`: `add`、`status`、`log`、`append-memo`。

完了条件: 上のテストが通る。CLI で足したTODO、変えたステータス、足した作業ログ、書き足したメモが、アプリの画面(開き直したとき)とタイムラインに、画面で操作したときと同じく出る。要件 A-2。

## フェーズ34 画面と配布

設計書の「画面の取り直し」、「CLIに出さないプロジェクト」の設定画面、「配布とREADME」。

**画面**

- `main.tsx`: TanStack Query の `focusManager` に、ウィンドウの `focus` のイベントを足す(今の `visibilitychange` と合わせる)。イベントをつなぐ処理は小さな関数にして、Vitest で確かめる。
- 日報の画面で編集中の文が取り直されないこと、詳細パネルで打っている途中の項目が残ることを確かめる。
- 設定画面の「CLIに出すプロジェクト」のカード(設計書のとおり。`SettingsPage.tsx` の「データ」のカードの下)。並べ方とスイッチから送る値を作る処理は小さな関数にして、Vitest で確かめる。`settingsInputOf` に `cli_hidden_projects` を足す。

**配布**

- `scripts/package.mjs`: Windows の ZIP に `higiri-cli.exe` を `Higiri.exe` と並べて入れる。macOS は `Higiri.app/Contents/MacOS/higiri-cli` に入れて署名する(フェーズ32の確かめの結果による)。
- `.github/workflows/` の3つ(`release.yml`、`windows.yml`、`macos.yml`)が、CLI を含めて作ることを確かめる。
- macOS で、ZIP から展開した `Higiri.app` の中の CLI がターミナルから動くか(ダウンロードしたファイルの印)を確かめる。動かなければ、README に手順を書く。

**README**

- 冒頭の文を「アプリは外部と通信しません」にする。
- インストールの節に、CLI の場所と、PATH に足す手順(Windows、macOS)。
- CLI の節: できること、`install-skill`、AI に読ませた内容は AI の提供元に送られること、「CLIに出すプロジェクト」で外せること(設計書の文)。

完了条件: Vitest が通る。アプリとターミナルを並べ、CLI でステータスを変えてからアプリのウィンドウを押すと、画面に出る。設定画面でプロジェクトの「CLIに出す」をオフにすると、そのプロジェクトのTODOが CLI に出なくなり、オンに戻すと出る。`npm run build` の ZIP から出した CLI が動く(Windows は Actions の成果物を利用者に試してもらう)。

## フェーズ35 Skill

設計書の「Skill」。

**Rust**

- `cli/SKILL.md`(`include_str!` で埋め込む)と `install-skill`。

**SKILL.md**

- `name`、`description`、`allowed-tools`(読むサブコマンドだけ)と、本文の7点(設計書のとおり)。
- CLI の出力の例(`today` と `show` を1つずつ)を短く載せる。

**テスト**

- `install-skill` が、作業用の `CLAUDE_CONFIG_DIR` の下に書き、上書きする。
- SKILL.md の先頭の項目が読め、`allowed-tools` に書くサブコマンドが入っていない。

**Claude Code での確認**(作業用の `CLAUDE_CONFIG_DIR` と `--dev` の DB で)

- 「今日なにをやればいい?」で `today` を読み、確認なしに動き、理由つきで3〜5件を出す。日数を数え直していない。
- 「この作業を TODO のログに付けて」で、本文とTODOを見せてから、確認を挟んで `log` を呼ぶ。
- 「これを完了にして」で、子のある親なら `needs_confirmation` の中身を伝え、答えを聞いてから引数を付ける。
- 「これを消して」で、中止を勧める。
- 「メモに書き足して」で、書き足す文を見せて確かめてから `append-memo` を呼ぶ。
- CLI に出さないプロジェクトのTODOを頼まれたら、`hidden` の中身を伝え、ほかの手段で読もうとしない。

完了条件: 上のテストが通り、Claude Code での確認の6つが期待どおりに動く。要件 A-3。

## フェーズ36 仕上げ

- `npm run test:perf` で、CLI のサブコマンドを含めて目標(起動2秒、一覧とガントの操作0.1秒、CLI の1回0.5秒)を確かめる。
- `README.md`、設計書の非機能要件の表(オフライン、セキュリティ、配布)、`CLAUDE.md` を最終確認する。
- 画面の権限が増えていないこと(`capabilities/default.json` が v0.4.0 と同じ)と、コマンドが48個のままであることを確かめる。
- サンプルデータで、CLI のすべてのサブコマンドと Skill を一通り試せることを確かめる。
- 版の番号(`tauri.conf.json` の `version`)を 0.5.0 に上げることと、リリースの説明(`.github/release-notes.md` の「v0.5.0 で足したこと」「直したこと」)は、利用者に確認してから行う。「直したこと」には、v0.4.0 の公開のあとに直したタブの並びのスクロールバーを入れる。表を変えないので、v0.5.0 のバックアップと JSON は v0.4.0 でも読めることを書くかどうかも、利用者と決める。

完了条件: 非機能要件の表を、v0.5.0 の分も含めて1行ずつ確かめ、結果を報告する。`npm run test:perf` の結果が目標に収まる。

## v0.5.0 に入れないもの

- MCP のサーバー(後から足すときの考え方は、設計書の「後からMCPを足すとき」)。
- CLI での、削除、復元、JSON の取り込み、設定の変更、プロジェクトの作成・編集、メモの書き換え(書き足しだけができる)、テンプレート・繰り返し・依存・一括変更・後続のずらし。
- 将来: 時間管理(タイマー、見積と実績の比較)、月報、ウィンドウを開かずにTODOを追加(T-13)。`reports.kind` の `monthly` は今のまま残す。
