---
name: higiri
description: Higiri(TODO管理アプリ)のTODOを読み、追加し、ステータスを変え、作業ログを付ける。今日やるべき作業を提案する。TODO、日限、やること、作業ログ、日報の材料の話が出たら使う。
allowed-tools:
  - Bash(higiri-cli today *)
  - Bash(higiri-cli list *)
  - Bash(higiri-cli show *)
  - Bash(higiri-cli search *)
  - Bash(higiri-cli projects *)
---

# Higiri

Higiri は、利用者の PC で動く TODO 管理アプリ。`higiri-cli` で、その TODO を読み書きする。Higiri は日本語だけのアプリなので、利用者への返事は日本語で書く。

## 呼び方

- `higiri-cli` の名前で呼ぶ(フルパスにしない)。1回に1つのサブコマンドだけを呼び、パイプや `&&` でほかのコマンドとつながない。出力の JSON は jq などで加工せず、そのまま読む(読むサブコマンドが確認なしに動くのは、単独で呼んだときだけ)。
- 成功すると、標準出力に JSON を1行出す。断ったときと失敗したときは、標準エラー出力に `{"error":{"kind":"…","message":"…"}}` を出す(終了コード1)。`message` は利用者向けの日本語なので、そのまま伝えてよい。引数の書き方が違うと終了コード2で、`higiri-cli <サブコマンド> --help` で書き方を確かめる。
- TODO は id で指す(名前で指す引数はない。探すときは `search`)。プロジェクトは id か名前(ちょうど同じもの)。日付は `YYYY-MM-DD`。
- ステータスは `todo`(未着手)、`doing`(作業中)、`waiting`(待ち)、`hold`(保留)、`done`(完了)、`cancelled`(中止)。優先度は `high`、`medium`、`low`。出力には日本語の名前(`status_label` など)が添えてある。

**読む**(確認なしに動く)

- `higiri-cli today`: 今日のまとめ。日限注意の件数と一覧(`attention`)、今日の作業(`today_work`)、待ち(`waiting`)、プロジェクトの進捗(`projects`)。
- `higiri-cli list`: TODO の一覧(初めは50件。`total` は全体の件数)。`--project <idか名前>`、`--status todo,doing`、`--priority high`、`--due attention`(ほかに `until_today`、`within_week`、`no_due`)、`--sort due_date`(ほかに `manual`、`priority`)、`--limit N`。アーカイブしたプロジェクトの TODO は、`--project` で指したときだけ出る。
- `higiri-cli show <id>`: 1件。メモ(`memo_md`)、新しい作業ログ5件(`--logs N` で変える)、子TODO(`child_todos`)、先行TODO(`depends_on`)と後続TODO(`dependents`)も出す。
- `higiri-cli search <語…>`: タイトル、メモ、待っている相手、中止の理由、作業ログから探す(最大100件)。語を並べると、すべてを含む TODO を探す。当たった場所の名前が `matched` に入る(本文は出ない)。
- `higiri-cli projects`: プロジェクトの一覧。`--archived` で、アーカイブしたものも出す。

**書く**(呼ぶたびに Claude Code の確認が出る)

- `higiri-cli add --project <idか名前> --title <タイトル>`: TODO を足す(未着手)。任意で `--parent <id>`(子TODOにする)、`--priority`、`--due`、`--start`、`--end`。返すのは作った TODO。
- `higiri-cli update <id>`: 指した項目だけを変える。`--title`、`--priority`、`--due <日付>` / `--no-due`、`--start` / `--no-start`、`--end` / `--no-end`、`--follow-up <日付>`(待ちの TODO だけ)。返すのは変えた TODO。
- `higiri-cli status <id> <ステータス>`: ステータスを変える。待ちには `--waiting-on <相手>` が要る(`--follow-up` を省くと、アプリと同じ確認日の初期値)。中止には `--reason <理由>` が要る。返すのは `{"todo":…,"closed_children":[{"id","title"}],"next_round":{"id","due_date"} か null,"repeat_ended":true か false}`。一緒に閉じた子TODO と、繰り返しで作った次の回(`repeat_ended` が true なら、終わりの日を過ぎたので作らなかったこと)を利用者に伝える。
- `higiri-cli log <id> --body <本文>`: 作業ログを足す。任意で `--date`(初めは今日)、`--minutes <分>`。返すのは `{"id","todo_id","log_date","minutes","body_md"}`。
- `higiri-cli append-memo <id> --body <本文>`: メモの末尾に、空行を1つ挟んで書き足す。返すのは変えた TODO と、書き足したあとのメモ(`memo_md`)。

複数行の本文は、`--body -` にして標準入力で渡す。

```bash
higiri-cli log 1 --body - <<'EOF'
- 切り戻し手順の章を書いた
- レビューを依頼した
EOF
```

## 守ること

1. 日付、日数、日限注意の区分は、出力のまま使う。数え直さない(営業日の設定や、休日のための前倒しがあり、数え直すとアプリの表示と食い違う)。今日の日付は、`today` の出力の `today`。日限注意の文は `urgency.text`(「日限 11/19、2日超過」など)、日限までの日数は `days_until_due`(`business_days` が true なら営業日)をそのまま使う。
2. 提案: 「今日なにをやればいい?」のように聞かれたら、`today` を読み、3〜5件を理由を添えて出す。
   - 順は、超過(`overdue`)→ 本日(`due_today`)→ 要確認(`follow_up`。待っている相手に確かめる作業として出す)→ 間近(`due_soon`)→ 着手遅れ(`late_start`)→ 今日の作業(`today_work`)の残り。同じ区分の中は、優先度の高い順。
   - `blocked_by` のある TODO は、先行TODO を先に出すか、理由(先行TODO が終わっていないこと)を添えて後ろに回す。
   - 理由には、`urgency.text`、`warnings` の `label`、待っている相手(`waiting.on`)を使う。
   - アプリには見積の時間がない。時間の割り振りを頼まれたら、使える時間を利用者に聞いてから行う。
3. 作業ログ: 頼まれたら、このセッションでしたことを数行の Markdown にまとめ、付ける TODO を決める。リポジトリの CLAUDE.md に「Higiri のプロジェクト: …」の行があれば、そのプロジェクトの TODO から選ぶ(`list --project <名前>`)。なければ `search` で探す。本文と付ける TODO(id とタイトル)を見せて確かめてから、`log` を呼ぶ。作業時間(`--minutes`)は、利用者が言ったときだけ入れる(推測で入れない)。
4. 追加: タイトル、プロジェクト、日限を確かめてから `add` を呼ぶ。日限を入れないときは、日限注意に出てこないことを伝える。
5. `kind` が `needs_confirmation` で断られたら、`message` の中身(終わっていない先行TODO、一緒に閉じる子TODO の名前)を利用者に伝え、答えを聞く。進めてよいと言われたら、`message` にある引数(`--despite-blocked` か `--with-children`)を付けてやり直す。消したいと言われたら、CLI に削除はないので、中止(`status <id> cancelled --reason <理由>`)を勧めて理由を聞く。
6. メモと作業ログの本文は、要るときだけ `show` で読む。メモに書き足すときは、書き足す文を見せて確かめてから `append-memo` を呼ぶ。メモの書き換えと削除はできない。
7. DB のファイル(`todo.db`)や SQL を直接触らない。CLI にない操作(TODO と作業ログの削除、メモの書き換え、プロジェクトの作成と編集、テンプレート、繰り返し、依存、一括変更、設定、バックアップからの復元)は、アプリの画面で行うよう伝える。
8. `kind` が `hidden` で断られたら、その TODO(かプロジェクト)は CLI に出さない設定になっていると伝える。設定を変えるのは利用者で、Higiri の設定画面の「CLIに出すプロジェクト」で行う。ほかの手段(DB のファイル、バックアップ、JSON の書き出し)で読もうとしない。先行TODO・後続TODO の `{"hidden":true}` も、中身を調べない。

ほかの断られ方:

- `message` が「Higiri がデータを書き換えている最中です。…」(アプリが復元などで書いている)なら、数秒待ってから1回だけやり直す。また断られたら、利用者に伝える。
- `validation` と `not_found` は、`message` を利用者に伝える。id の取り違えなど、直し方がはっきりしているものは、直してよいか確かめてからやり直す。

## 出力の例

どちらも、実際の出力から一部を省いたもの(`…`)。

`higiri-cli today`:

```json
{"today":"2026-11-21","day_count":"calendar","day_count_label":"暦日","soon_days":3,
 "attention":{"total":5,
  "counts":{"overdue":1,"due_today":1,"due_soon":1,"follow_up":1,"late_start":1},
  "todos":[
   {"id":7,"title":"ライセンス棚卸し","parent":null,"project":{"id":2,"name":"社内ツール整備"},
    "status":"todo","status_label":"未着手","priority":"medium","priority_label":"中",
    "due_date":"2026-11-19",…,
    "urgency":{"kind":"overdue","label":"超過","text":"日限 11/19、2日超過"},
    "days_until_due":-2,"business_days":false,"waiting":null,"close_reason":null,
    "warnings":[],"blocked_by":[],"children":null,"repeat":null,…},
   {"id":4,"title":"性能試験の環境準備",…,
    "urgency":{"kind":"late_start","label":"着手遅れ","text":"予定開始 11/19、2日経過"},
    …,"blocked_by":[{"id":2,"title":"API設計書のレビュー対応"}],…},
   …]},
 "today_work":[{"id":1,"title":"移行手順書のドラフト",…},…],
 "waiting":[{"id":10,"title":"募集要項ページの原稿確認",…,
   "waiting":{"on":"人事部の原稿確認","since":"2026-11-19","follow_up_date":"2026-11-21","day":3},…},…],
 "projects":[{"id":1,"name":"基幹システム更改","progress_done":1,"progress_total":5,"attention_count":3},…],
 "backup_mirror_alert":null}
```

`higiri-cli show 1`:

```json
{"id":1,"title":"移行手順書のドラフト","parent":null,"project":{"id":1,"name":"基幹システム更改"},
 "status":"doing","status_label":"作業中","priority":"medium","priority_label":"中",
 "due_date":"2026-11-21","planned_start":"2026-11-13","planned_end":"2026-11-20",
 "actual_start":"2026-11-14","actual_end":null,
 "urgency":{"kind":"due_today","label":"本日","text":"日限 11/21"},
 "days_until_due":0,"business_days":false,"waiting":null,"close_reason":null,
 "warnings":[{"kind":"plan_overrun","label":"予定超過"}],
 "blocked_by":[],"children":null,"repeat":null,"updated_at":"2026-11-14T09:00:00.000Z",
 "memo_md":"",
 "work_logs":[{"log_date":"2026-11-21","minutes":120,"body_md":"- 切り戻し手順の章を書き終え、レビューを依頼した"}],
 "work_logs_total":1,
 "child_todos":[],
 "depends_on":[],
 "dependents":[{"id":3,"title":"結合テスト計画の作成","status":"todo","status_label":"未着手"}]}
```
