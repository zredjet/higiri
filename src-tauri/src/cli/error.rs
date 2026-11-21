//! The errors higiri-cli writes to standard error, one line of JSON:
//! `{"error":{"kind":"validation","message":"…"}}`.
//!
//! `kind` is the screens' (`validation`, `not_found`, `internal`) plus
//! `needs_confirmation` and `hidden`. `message` is Japanese text for the
//! user: what happened, then what to do.

use serde::Serialize;

use crate::commands::error::{AppError, ErrorKind};
use crate::repo::db::OpenError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliErrorKind {
    /// The input cannot be used as is. The user can fix it.
    Validation,
    /// The record (or the data) is not there.
    NotFound,
    /// Unexpected failure (database, file system).
    Internal,
    /// The screen would ask first: run again with the flag the message names
    /// (v0.5.0 phase 33).
    NeedsConfirmation,
    /// In a project the CLI does not show (the settings screen's
    /// 「CLIに出すプロジェクト」).
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct CliError {
    pub kind: CliErrorKind,
    pub message: String,
}

/// What standard error gets: the error under `error`.
#[derive(Serialize)]
struct Written<'a> {
    error: &'a CliError,
}

impl CliError {
    pub fn new(kind: CliErrorKind, message: impl Into<String>) -> CliError {
        CliError {
            kind,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> CliError {
        CliError::new(CliErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> CliError {
        CliError::new(CliErrorKind::NotFound, message)
    }

    pub fn internal() -> CliError {
        CliError::new(
            CliErrorKind::Internal,
            "データの読み書きに失敗しました。もう一度実行してください。続くときは、Higiri を再起動してから実行してください。",
        )
    }

    /// A TODO of a project the CLI does not show.
    pub fn hidden_todo() -> CliError {
        CliError::new(
            CliErrorKind::Hidden,
            "このTODOは、CLIに出さない設定のプロジェクトにあります。CLIで扱うときは、Higiri の設定画面の「CLIに出すプロジェクト」で変えてください。",
        )
    }

    /// A project the CLI does not show, named in an argument.
    pub fn hidden_project() -> CliError {
        CliError::new(
            CliErrorKind::Hidden,
            "このプロジェクトは、CLIに出さない設定です。CLIで扱うときは、Higiri の設定画面の「CLIに出すプロジェクト」で変えてください。",
        )
    }

    /// The TODO `id` is not there.
    pub fn todo_not_found(id: i64) -> CliError {
        CliError::not_found(format!(
            "TODO(id {id})が見つかりません。削除された可能性があります。higiri-cli list か search でidを確かめてください。"
        ))
    }

    /// Going 作業中 while TODOs to finish first are unfinished: `names` are
    /// theirs (those of projects the CLI does not show are not named).
    pub fn blocked(names: &[String]) -> CliError {
        CliError::new(
            CliErrorKind::NeedsConfirmation,
            format!(
                "次の{}件の先行TODOが、まだ完了も中止もしていません({})。このまま作業中にするときは --despite-blocked を付けてください。",
                names.len(),
                names.join("、")
            ),
        )
    }

    /// Closing a parent whose children `names` are open: they would be
    /// closed with it, to `to` (「完了」「中止」).
    pub fn open_children(names: &[String], to: &str) -> CliError {
        CliError::new(
            CliErrorKind::NeedsConfirmation,
            format!(
                "開いている子TODOが{}件あります({})。子TODOも一緒に{to}にするときは --with-children を付けてください。",
                names.len(),
                names.join("、")
            ),
        )
    }

    /// Today's daily backup could not be taken, so nothing was written.
    pub fn no_backup() -> CliError {
        CliError::new(
            CliErrorKind::Internal,
            "バックアップを作れなかったため、書き込みをやめました。Higiri を起動して、バックアップが作られるか確かめてください。",
        )
    }

    /// The database stayed locked: the app is writing (a restore, a JSON
    /// import).
    pub fn busy() -> CliError {
        CliError::new(
            CliErrorKind::Internal,
            "Higiri がデータを書き換えている最中です。少し待ってから、もう一度実行してください。",
        )
    }

    /// The data folder could not be found (no home folder).
    pub fn no_data_folder() -> CliError {
        CliError::new(
            CliErrorKind::Internal,
            "Higiri のデータフォルダの場所が分かりませんでした。ログインしているユーザーで実行してください。",
        )
    }

    /// The JSON line for standard error.
    pub fn to_line(&self) -> String {
        serde_json::to_string(&Written { error: self }).unwrap_or_else(|_| {
            r#"{"error":{"kind":"internal","message":"エラーを書き出せませんでした。"}}"#.to_owned()
        })
    }
}

impl From<AppError> for CliError {
    /// The screens' errors keep their text, except the unexpected ones,
    /// whose text speaks of restarting the app, and the one of a write that
    /// waited too long for the lock, which speaks of the screens.
    fn from(e: AppError) -> CliError {
        if e == AppError::busy() {
            return CliError::busy();
        }
        match e.kind {
            ErrorKind::Validation => CliError::validation(e.message),
            ErrorKind::NotFound => CliError::not_found(e.message),
            ErrorKind::Internal => CliError::internal(),
        }
    }
}

impl From<OpenError> for CliError {
    fn from(e: OpenError) -> CliError {
        match e {
            OpenError::Missing => CliError::not_found(
                "Higiri のデータが見つかりません。先に Higiri を一度起動してください。",
            ),
            OpenError::Older => CliError::validation(
                "Higiri のデータが古い版のままです。新しい版の Higiri を一度起動してから、もう一度実行してください。",
            ),
            OpenError::Newer => CliError::validation(
                "higiri-cli が Higiri より古い版です。Higiri と同じ ZIP に入っている higiri-cli を使ってください。",
            ),
            OpenError::Busy => CliError::busy(),
            OpenError::Failed(_) => CliError::internal(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_as_one_line_under_error() {
        assert_eq!(
            CliError::validation("タイトルを入力してください。").to_line(),
            r#"{"error":{"kind":"validation","message":"タイトルを入力してください。"}}"#
        );
        assert_eq!(
            CliError::new(CliErrorKind::NeedsConfirmation, "x").to_line(),
            r#"{"error":{"kind":"needs_confirmation","message":"x"}}"#
        );
        let hidden = CliError::hidden_todo().to_line();
        assert!(hidden.starts_with(r#"{"error":{"kind":"hidden","message":"このTODOは、"#));
        assert!(!hidden.contains('\n'));
    }

    #[test]
    fn screen_errors_keep_their_kind() {
        let e: CliError = AppError::validation("日付を直してください。").into();
        assert_eq!(e, CliError::validation("日付を直してください。"));
        let e: CliError = AppError::internal().into();
        assert_eq!(e, CliError::internal());
        assert!(!e.message.contains("アプリを再起動"));
        let e: CliError = AppError::busy().into();
        assert_eq!(e, CliError::busy());
        assert_eq!(
            e.message,
            "Higiri がデータを書き換えている最中です。少し待ってから、もう一度実行してください。"
        );
    }

    #[test]
    fn refused_databases_say_what_to_do() {
        let cases = [
            (
                OpenError::Missing,
                CliErrorKind::NotFound,
                "Higiri のデータが見つかりません。先に Higiri を一度起動してください。",
            ),
            (
                OpenError::Older,
                CliErrorKind::Validation,
                "Higiri のデータが古い版のままです。新しい版の Higiri を一度起動してから、もう一度実行してください。",
            ),
            (
                OpenError::Newer,
                CliErrorKind::Validation,
                "higiri-cli が Higiri より古い版です。Higiri と同じ ZIP に入っている higiri-cli を使ってください。",
            ),
            (
                OpenError::Busy,
                CliErrorKind::Internal,
                "Higiri がデータを書き換えている最中です。少し待ってから、もう一度実行してください。",
            ),
        ];
        for (open, kind, message) in cases {
            assert_eq!(CliError::from(open), CliError::new(kind, message));
        }
    }
}
