//! The one error type returned to the screens.
//!
//! `message` is Japanese text for the user: what happened, then what to do.

use serde::Serialize;

use crate::domain::status::TransitionError;
use crate::repo::RepoError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// The input cannot be saved as is. The user can fix it.
    Validation,
    /// The record no longer exists (deleted elsewhere).
    NotFound,
    /// Unexpected failure (database, file system).
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

pub type CmdResult<T> = Result<T, AppError>;

impl AppError {
    pub fn validation(message: impl Into<String>) -> AppError {
        AppError {
            kind: ErrorKind::Validation,
            message: message.into(),
        }
    }

    pub fn internal() -> AppError {
        AppError {
            kind: ErrorKind::Internal,
            message:
                "データの読み書きに失敗しました。アプリを再起動してから、もう一度操作してください。"
                    .into(),
        }
    }
}

impl From<RepoError> for AppError {
    fn from(e: RepoError) -> AppError {
        if e.is_constraint_violation() {
            return AppError::validation(
                "入力した内容を保存できませんでした。日付の前後と、必須の項目を確かめてください。",
            );
        }
        match e {
            RepoError::NotFound { entity, .. } => AppError {
                kind: ErrorKind::NotFound,
                message: match entity {
                    "project" => "プロジェクトが見つかりません。削除された可能性があります。画面を開き直してください。",
                    "todo" => "TODOが見つかりません。削除された可能性があります。画面を開き直してください。",
                    "work_log" => "作業ログが見つかりません。削除された可能性があります。TODOを開き直してください。",
                    "todo template" => "テンプレートが見つかりません。削除された可能性があります。画面を開き直してください。",
                    _ => "対象が見つかりません。画面を開き直してください。",
                }
                .into(),
            },
            other => {
                log::error!("internal error: {other}");
                AppError::internal()
            }
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> AppError {
        RepoError::from(e).into()
    }
}

impl From<TransitionError> for AppError {
    fn from(e: TransitionError) -> AppError {
        AppError::validation(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_names_the_record() {
        let e: AppError = RepoError::NotFound {
            entity: "todo",
            id: 3,
        }
        .into();
        assert_eq!(e.kind, ErrorKind::NotFound);
        assert!(e.message.starts_with("TODOが見つかりません。"));
    }

    #[test]
    fn corrupt_data_is_internal_and_hides_details() {
        let e: AppError = RepoError::Corrupt {
            column: "todos.status",
            value: "x".into(),
        }
        .into();
        assert_eq!(e, AppError::internal());
    }

    #[test]
    fn serializes_as_kind_and_message() {
        let json =
            serde_json::to_string(&AppError::validation("タイトルを入力してください。")).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"validation","message":"タイトルを入力してください。"}"#
        );
    }
}
