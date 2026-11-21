//! The arguments of higiri-cli (clap). The help is Japanese; clap's own
//! words (「Usage:」 and the like) stay English.

use clap::builder::{PossibleValue, PossibleValuesParser, TypedValueParser};
use clap::{ArgAction, Args, Parser, Subcommand};

use crate::domain::listing::{DueFilter, TodoSort};
use crate::domain::model::{Priority, Status};

/// Higiri(TODO管理アプリ)のTODOを読みます。出力はJSONです。
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

        let version = parse(&["--version"]).unwrap_err();
        assert_eq!(version.kind(), ErrorKind::DisplayVersion);
        assert_eq!(
            version.render().to_string(),
            format!("higiri-cli {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}
