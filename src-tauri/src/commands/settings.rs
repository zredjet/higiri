//! 設定: get_settings, update_settings.

use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::State;

use super::error::CmdResult;
use super::validate;
use crate::domain::settings::{ReminderFrequency, Settings, Theme};
use crate::repo;

/// Days accepted for 「間近」とみなす日数 and 確認日の初期値.
const DAYS_MIN: i64 = 1;
const DAYS_MAX: i64 = 30;

/// Settings the user edits on the settings screen. `last_reminded_on` is
/// written by `ack_reminder`, not here.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct SettingsInput {
    pub reminder_frequency: ReminderFrequency,
    pub soon_days: u32,
    pub follow_up_days: u32,
    pub theme: Theme,
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(pool: State<'_, SqlitePool>) -> CmdResult<Settings> {
    get(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    pool: State<'_, SqlitePool>,
    input: SettingsInput,
) -> CmdResult<Settings> {
    update(&pool, input).await
}

pub async fn get(pool: &SqlitePool) -> CmdResult<Settings> {
    let mut conn = pool.acquire().await?;
    Ok(repo::settings::load(&mut conn).await?)
}

pub async fn update(pool: &SqlitePool, input: SettingsInput) -> CmdResult<Settings> {
    validate::in_range(
        i64::from(input.soon_days),
        DAYS_MIN,
        DAYS_MAX,
        "「間近」とみなす日数",
        "日",
    )?;
    validate::in_range(
        i64::from(input.follow_up_days),
        DAYS_MIN,
        DAYS_MAX,
        "確認日の初期値",
        "日",
    )?;
    let mut tx = pool.begin().await?;
    let current = repo::settings::load(&mut tx).await?;
    let next = Settings {
        reminder_frequency: input.reminder_frequency,
        soon_days: input.soon_days,
        follow_up_days: input.follow_up_days,
        theme: input.theme,
        ..current
    };
    repo::settings::save(&mut tx, &next).await?;
    tx.commit().await?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::repo::db;

    fn input(soon: u32, follow: u32) -> SettingsInput {
        SettingsInput {
            reminder_frequency: ReminderFrequency::OncePerDay,
            soon_days: soon,
            follow_up_days: follow,
            theme: Theme::Dark,
        }
    }

    #[tokio::test]
    async fn defaults_then_update() {
        let pool = db::open_in_memory().await.unwrap();
        assert_eq!(get(&pool).await.unwrap(), Settings::default());
        let s = update(&pool, input(5, 2)).await.unwrap();
        assert_eq!(s.soon_days, 5);
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(get(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn out_of_range_days_are_rejected() {
        let pool = db::open_in_memory().await.unwrap();
        for (soon, follow) in [(0, 3), (31, 3), (3, 0), (3, 31)] {
            let e = update(&pool, input(soon, follow)).await.unwrap_err();
            assert_eq!(e.kind, ErrorKind::Validation, "{soon} {follow}");
        }
        assert_eq!(
            update(&pool, input(0, 3)).await.unwrap_err().message,
            "「間近」とみなす日数は1〜30日の範囲で入力してください。"
        );
        assert_eq!(get(&pool).await.unwrap(), Settings::default());
    }
}
