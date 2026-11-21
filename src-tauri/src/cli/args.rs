//! The arguments of higiri-cli (clap). The help is Japanese; clap's own
//! words (「Usage:」 and the like) stay English.

use chrono::NaiveDate;
use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand};

use crate::domain::date::parse_date;
use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status};

/// Higiri(TODO管理アプリ)のTODOを読み書きします。出力はJSONです。
#[derive(Debug, Parser)]
#[command(
    name = "higiri-cli",
    version,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// 開発ビルドのデータ(データフォルダの dev/todo.db)を使います。
    #[arg(long, global = true)]
    pub dev: bool,

    /// 説明を出します。
    #[arg(short = 'h', long, action = ArgAction::Help, global = true)]
    help: Option<bool>,

    /// 版を出します。
    #[arg(short = 'V', long, action = ArgAction::Version)]
    version: Option<bool>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// 今日のまとめ(日限注意、今日の作業、待ち、プロジェクトの進捗)を出します。ダッシュボードと同じ中身です。
    Today,
    /// TODOの一覧を出します。絞り込みと並べ替えは、アプリのTODO一覧と同じです。
    List(ListArgs),
    /// TODOを1件出します。メモ、新しい作業ログ、子TODO、先行TODOと後続TODOも出します。
    Show(ShowArgs),
    /// タイトル、メモ、待っている相手、中止の理由、作業ログからTODOを探します(最大100件)。
    Search(SearchArgs),
    /// プロジェクトの一覧を出します。
    Projects(ProjectsArgs),
    /// TODOを足します(ステータスは未着手)。作ったTODOを出します。
    Add(AddArgs),
    /// TODOのタイトル、優先度、日限、予定期間、確認日を変えます。指した項目だけが変わります。メモは変えません(書き足すときは append-memo)。
    Update(UpdateArgs),
    /// TODOのステータスを変えます。
    Status(StatusArgs),
    /// TODOに作業ログを足します。
    Log(LogArgs),
    /// TODOのメモの末尾に書き足します。メモの書き換えと削除はできません。
    AppendMemo(AppendMemoArgs),
    /// Claude Code の Skill(higiri)を ~/.claude/skills/higiri/SKILL.md に書きます。環境変数 CLAUDE_CONFIG_DIR があれば、その下の skills/higiri/ に書きます。あれば上書きします。書いた場所を出します。
    InstallSkill,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ListArgs {
    /// プロジェクトのidか名前(名前はちょうど同じもの)。指したときだけ、アーカイブしたプロジェクトのTODOも出します。
    #[arg(long, value_name = "IDか名前")]
    pub project: Option<String>,

    /// ステータス。todo,doing のようにカンマで並べます。
    #[arg(long, value_delimiter = ',', value_parser = choices(&STATUSES), value_name = "ステータス")]
    pub status: Vec<Status>,

    /// 優先度。
    #[arg(long, value_parser = choices(&PRIORITIES), value_name = "優先度")]
    pub priority: Option<Priority>,

    /// 日限での絞り込み。
    #[arg(long, value_parser = choices(&DUE_FILTERS), value_name = "日限")]
    pub due: Option<DueFilter>,

    /// 並べ方(初めは登録順)。
    #[arg(long, value_parser = choices(&SORTS), value_name = "並べ方")]
    pub sort: Option<TodoSort>,

    /// 出す件数の上限。全体の件数は total に入ります。
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..), value_name = "件数")]
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ShowArgs {
    /// TODOのid。
    #[arg(value_name = "ID")]
    pub id: i64,

    /// 出す作業ログの件数(新しい順)。
    #[arg(long, default_value_t = 5, value_name = "件数")]
    pub logs: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct SearchArgs {
    /// 探す語。空白で区切ると、すべての語を含むTODOを探します。
    #[arg(required = true, num_args = 1.., value_name = "語")]
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct ProjectsArgs {
    /// アーカイブしたプロジェクトも出します。
    #[arg(long)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AddArgs {
    /// プロジェクトのidか名前(名前はちょうど同じもの)。
    #[arg(long, value_name = "IDか名前")]
    pub project: String,

    /// タイトル。
    #[arg(long, allow_hyphen_values = true, value_name = "タイトル")]
    pub title: String,

    /// 子TODOにするときの、親のTODOのid(同じプロジェクトの、親のないTODO)。
    #[arg(long, value_name = "ID")]
    pub parent: Option<i64>,

    /// 優先度(初めは中)。
    #[arg(long, value_parser = choices(&PRIORITIES), value_name = "優先度")]
    pub priority: Option<Priority>,

    /// 日限(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub due: Option<NaiveDate>,

    /// 予定開始日(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub start: Option<NaiveDate>,

    /// 予定終了日(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub end: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
#[command(group(
    ArgGroup::new("fields")
        .required(true)
        .multiple(true)
        .args(["title", "priority", "due", "no_due", "start", "no_start", "end", "no_end", "follow_up"])
))]
pub struct UpdateArgs {
    /// TODOのid。
    #[arg(value_name = "ID")]
    pub id: i64,

    /// タイトル。
    #[arg(long, allow_hyphen_values = true, value_name = "タイトル")]
    pub title: Option<String>,

    /// 優先度。
    #[arg(long, value_parser = choices(&PRIORITIES), value_name = "優先度")]
    pub priority: Option<Priority>,

    /// 日限(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付", conflicts_with = "no_due")]
    pub due: Option<NaiveDate>,

    /// 日限を空にします。
    #[arg(long)]
    pub no_due: bool,

    /// 予定開始日(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付", conflicts_with = "no_start")]
    pub start: Option<NaiveDate>,

    /// 予定開始日を空にします。
    #[arg(long)]
    pub no_start: bool,

    /// 予定終了日(YYYY-MM-DD)。
    #[arg(long, value_parser = date, value_name = "日付", conflicts_with = "no_end")]
    pub end: Option<NaiveDate>,

    /// 予定終了日を空にします。
    #[arg(long)]
    pub no_end: bool,

    /// 確認日(YYYY-MM-DD)。待ちのTODOだけに入れられます。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub follow_up: Option<NaiveDate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct StatusArgs {
    /// TODOのid。
    #[arg(value_name = "ID")]
    pub id: i64,

    /// 新しいステータス。
    #[arg(value_parser = choices(&STATUSES), value_name = "ステータス")]
    pub status: Status,

    /// 待っている相手。waiting にするときに要ります。
    #[arg(
        long,
        allow_hyphen_values = true,
        required_if_eq("status", "waiting"),
        value_name = "相手"
    )]
    pub waiting_on: Option<String>,

    /// 確認日(YYYY-MM-DD)。waiting にするときに使います。省くと、アプリの「待ちにする」と同じ初期値になります。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub follow_up: Option<NaiveDate>,

    /// 中止の理由。cancelled にするときに要ります。
    #[arg(
        long,
        allow_hyphen_values = true,
        required_if_eq("status", "cancelled"),
        value_name = "理由"
    )]
    pub reason: Option<String>,

    /// 先行TODOが終わっていなくても、作業中にします。
    #[arg(long)]
    pub despite_blocked: bool,

    /// 開いている子TODOも一緒に、完了か中止にします。
    #[arg(long)]
    pub with_children: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct LogArgs {
    /// TODOのid。
    #[arg(value_name = "ID")]
    pub id: i64,

    /// 本文(Markdown)。- なら標準入力から読みます(UTF-8)。
    #[arg(long, allow_hyphen_values = true, value_name = "本文")]
    pub body: String,

    /// 作業した日(YYYY-MM-DD。初めは今日)。
    #[arg(long, value_parser = date, value_name = "日付")]
    pub date: Option<NaiveDate>,

    /// 作業時間(分)。
    #[arg(long, allow_negative_numbers = true, value_name = "分")]
    pub minutes: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Args)]
pub struct AppendMemoArgs {
    /// TODOのid。
    #[arg(value_name = "ID")]
    pub id: i64,

    /// 書き足す本文(Markdown)。- なら標準入力から読みます(UTF-8)。
    #[arg(long, allow_hyphen_values = true, value_name = "本文")]
    pub body: String,
}

/// `--body -`: the text is read from standard input.
pub const FROM_STDIN: &str = "-";

impl Command {
    /// The `--body` of `log` and `append-memo`.
    pub fn body_mut(&mut self) -> Option<&mut String> {
        match self {
            Command::Log(a) => Some(&mut a.body),
            Command::AppendMemo(a) => Some(&mut a.body),
            _ => None,
        }
    }
}

/// A date argument: `YYYY-MM-DD`, as stored.
fn date(s: &str) -> Result<NaiveDate, String> {
    parse_date(s)
        .ok_or_else(|| "日付は 2026-11-19 のように YYYY-MM-DD で書いてください。".to_owned())
}

/// A value the CLI names, with its help text.
type Choice<T> = (T, &'static str, &'static str);

const STATUSES: [Choice<Status>; 6] = [
    (Status::Todo, "todo", "未着手"),
    (Status::Doing, "doing", "作業中"),
    (Status::Waiting, "waiting", "待ち"),
    (Status::Hold, "hold", "保留"),
    (Status::Done, "done", "完了"),
    (Status::Cancelled, "cancelled", "中止"),
];

const PRIORITIES: [Choice<Priority>; 3] = [
    (Priority::High, "high", "高"),
    (Priority::Medium, "medium", "中"),
    (Priority::Low, "low", "低"),
];

const DUE_FILTERS: [Choice<DueFilter>; 4] = [
    (DueFilter::Attention, "attention", "日限注意だけ"),
    (DueFilter::UntilToday, "until_today", "今日まで"),
    (DueFilter::WithinWeek, "within_week", "7日後まで"),
    (DueFilter::NoDue, "no_due", "日限なし"),
];

const SORTS: [Choice<TodoSort>; 3] = [
    (TodoSort::Manual, "manual", "登録順"),
    (TodoSort::DueDate, "due_date", "日限の近い順"),
    (TodoSort::Priority, "priority", "優先度の高い順"),
];

/// A parser that takes one of the names in `list`.
fn choices<T: Copy + Send + Sync + 'static>(
    list: &'static [Choice<T>],
) -> impl TypedValueParser<Value = T> {
    PossibleValuesParser::new(
        list.iter()
            .map(|&(_, name, help)| PossibleValue::new(name).help(help)),
    )
    .map(move |name| {
        list.iter()
            .find(|(_, n, _)| *n == name)
            .map(|&(value, _, _)| value)
            .expect("one of the possible values")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("higiri-cli").chain(args.iter().copied()))
    }

    #[test]
    fn the_definition_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn names_are_the_stored_ones() {
        for (status, name, label) in STATUSES {
            assert_eq!(status.as_str(), name);
            assert_eq!(status.label(), label);
        }
        for (priority, name, label) in PRIORITIES {
            assert_eq!(serde_json::to_value(priority).unwrap(), name);
            assert_eq!(priority.label(), label);
        }
        for (due, name, _) in DUE_FILTERS {
            assert_eq!(serde_json::to_value(due).unwrap(), name);
        }
        for (sort, name, _) in SORTS {
            assert_eq!(serde_json::to_value(sort).unwrap(), name);
        }
    }

    #[test]
    fn list_takes_the_screen_filters() {
        let cli = parse(&[
            "list",
            "--project",
            "社内ツール整備",
            "--status",
            "todo,doing",
            "--priority",
            "high",
            "--due",
            "until_today",
            "--sort",
            "due_date",
            "--limit",
            "10",
        ])
        .unwrap();
        assert_eq!(
            cli.command,
            Command::List(ListArgs {
                project: Some("社内ツール整備".into()),
                status: vec![Status::Todo, Status::Doing],
                priority: Some(Priority::High),
                due: Some(DueFilter::UntilToday),
                sort: Some(TodoSort::DueDate),
                limit: 10,
            })
        );
        assert!(!cli.dev);
        let Command::List(defaults) = parse(&["list"]).unwrap().command else {
            panic!("list");
        };
        assert_eq!(defaults.limit, 50);
        assert!(defaults.status.is_empty());
    }

    #[test]
    fn dev_goes_anywhere() {
        assert!(parse(&["--dev", "today"]).unwrap().dev);
        assert!(parse(&["today", "--dev"]).unwrap().dev);
        assert!(parse(&["show", "3", "--dev", "--logs", "2"]).unwrap().dev);
    }

    #[test]
    fn show_and_search_and_projects() {
        assert_eq!(
            parse(&["show", "12"]).unwrap().command,
            Command::Show(ShowArgs { id: 12, logs: 5 })
        );
        assert_eq!(
            parse(&["search", "認証", "回答"]).unwrap().command,
            Command::Search(SearchArgs {
                words: vec!["認証".into(), "回答".into()]
            })
        );
        assert_eq!(
            parse(&["projects", "--archived"]).unwrap().command,
            Command::Projects(ProjectsArgs { archived: true })
        );
        assert_eq!(
            parse(&["install-skill"]).unwrap().command,
            Command::InstallSkill
        );
    }

    #[test]
    fn the_write_sub_commands() {
        let date = |s| parse_date(s).unwrap();
        assert_eq!(
            parse(&[
                "add",
                "--project",
                "3",
                "--title",
                "- 箇条書きのような題",
                "--parent",
                "12",
                "--priority",
                "low",
                "--due",
                "2026-11-30",
                "--start",
                "2026-11-20",
                "--end",
                "2026-11-27",
            ])
            .unwrap()
            .command,
            Command::Add(AddArgs {
                project: "3".into(),
                title: "- 箇条書きのような題".into(),
                parent: Some(12),
                priority: Some(Priority::Low),
                due: Some(date("2026-11-30")),
                start: Some(date("2026-11-20")),
                end: Some(date("2026-11-27")),
            })
        );
        assert_eq!(
            parse(&[
                "update",
                "5",
                "--no-due",
                "--start",
                "2026-11-20",
                "--no-end"
            ])
            .unwrap()
            .command,
            Command::Update(UpdateArgs {
                id: 5,
                title: None,
                priority: None,
                due: None,
                no_due: true,
                start: Some(date("2026-11-20")),
                no_start: false,
                end: None,
                no_end: true,
                follow_up: None,
            })
        );
        assert_eq!(
            parse(&["status", "5", "waiting", "--waiting-on", "認証担当"])
                .unwrap()
                .command,
            Command::Status(StatusArgs {
                id: 5,
                status: Status::Waiting,
                waiting_on: Some("認証担当".into()),
                follow_up: None,
                reason: None,
                despite_blocked: false,
                with_children: false,
            })
        );
        let Command::Status(closing) = parse(&[
            "status",
            "5",
            "cancelled",
            "--reason",
            "不要",
            "--with-children",
        ])
        .unwrap()
        .command
        else {
            panic!("status");
        };
        assert_eq!(closing.reason.as_deref(), Some("不要"));
        assert!(closing.with_children);
        assert_eq!(
            parse(&["log", "5", "--body", "- 作業した", "--minutes", "-5"])
                .unwrap()
                .command,
            Command::Log(LogArgs {
                id: 5,
                body: "- 作業した".into(),
                date: None,
                minutes: Some(-5),
            })
        );
        let mut memo = parse(&["append-memo", "5", "--body", "-"]).unwrap().command;
        assert_eq!(
            memo,
            Command::AppendMemo(AppendMemoArgs {
                id: 5,
                body: FROM_STDIN.into(),
            })
        );
        assert_eq!(memo.body_mut().map(|b| b.as_str()), Some("-"));
        assert_eq!(parse(&["today"]).unwrap().command.body_mut(), None);
    }

    #[test]
    fn wrong_arguments_are_clap_errors() {
        for args in [
            &[][..],
            &["unknown"],
            &["list", "--status", "finished"],
            &["list", "--limit", "0"],
            &["list", "--due", "all"],
            &["show"],
            &["show", "x"],
            &["search"],
            &["today", "extra"],
            // Writes.
            &["add", "--title", "題"],
            &["add", "--project", "1"],
            &[
                "add",
                "--project",
                "1",
                "--title",
                "題",
                "--due",
                "2026/11/30",
            ],
            &[
                "add",
                "--project",
                "1",
                "--title",
                "題",
                "--due",
                "2026-11-31",
            ],
            &["update", "5"],
            &["update", "5", "--due", "2026-11-30", "--no-due"],
            &["update", "5", "--start", "2026-11-30", "--no-start"],
            &["update", "5", "--memo", "x"],
            &["status", "5"],
            &["status", "5", "finished"],
            &["status", "5", "waiting"],
            &["status", "5", "cancelled"],
            &["log", "5"],
            &["log", "5", "--body", "x", "--minutes", "1.5"],
            &["log", "5", "--body", "x", "--date", "11/19"],
            &["append-memo", "5"],
            &["append-memo", "--body", "x"],
            &["install-skill", "higiri"],
        ] {
            let e = parse(args).unwrap_err();
            assert_eq!(e.exit_code(), 2, "{args:?}: {e}");
        }
    }

    #[test]
    fn help_and_version_are_not_errors() {
        let help = parse(&["--help"]).unwrap_err();
        assert_eq!(help.kind(), ErrorKind::DisplayHelp);
        assert_eq!(help.exit_code(), 0);
        let text = help.render().to_string();
        assert!(text.contains("今日のまとめ"), "{text}");
        assert!(text.contains("説明を出します。"), "{text}");
        assert!(!text.contains("Print help"), "{text}");

        let help = parse(&["list", "--help"]).unwrap_err();
        assert_eq!(help.exit_code(), 0);
        let text = help.render().to_string();
        assert!(
            text.contains("todo,doing のようにカンマで並べます。"),
            "{text}"
        );
        assert!(text.contains("未着手"), "{text}");
        assert!(!text.contains("Print help"), "{text}");

        let help = parse(&["install-skill", "--help"]).unwrap_err();
        assert_eq!(help.exit_code(), 0);
        let text = help.render().to_string();
        assert!(text.contains("CLAUDE_CONFIG_DIR"), "{text}");

        let help = parse(&["status", "--help"]).unwrap_err();
        let text = help.render().to_string();
        assert!(text.contains("--despite-blocked"), "{text}");
        assert!(text.contains("--with-children"), "{text}");
        let e = parse(&["update", "5", "--due", "2026-13-01"]).unwrap_err();
        assert!(
            e.render()
                .to_string()
                .contains("YYYY-MM-DD で書いてください"),
            "{e}"
        );

        let version = parse(&["--version"]).unwrap_err();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);
        assert_eq!(
            version.render().to_string(),
            format!("higiri-cli {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}
