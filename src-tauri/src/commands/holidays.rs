//! 休日: list_holidays (v1.1). National and custom holidays with their names,
//! for the Gantt shading and the settings screen.

use chrono::{Months, NaiveDate};
use sqlx::SqlitePool;
use tauri::State;

use super::error::{AppError, CmdResult};
use crate::domain::calendar::{Holiday, WorkCalendar};
use crate::repo;

/// Longest period `list_holidays` answers (the design: 3 years).
const MAX_MONTHS: u32 = 36;

/// Holidays from `start` to `end` (both included, at most 3 years), in date
/// order. Listed whatever the day count setting is.
#[tauri::command]
#[specta::specta]
pub async fn list_holidays(
    pool: State<'_, SqlitePool>,
    start: NaiveDate,
    end: NaiveDate,
) -> CmdResult<Vec<Holiday>> {
    list(&pool, start, end).await
}

pub async fn list(pool: &SqlitePool, start: NaiveDate, end: NaiveDate) -> CmdResult<Vec<Holiday>> {
    if start > end {
        return Err(AppError::validation(
            "休日を調べる期間の開始日が終了日より後になっています。日付を直してください。",
        ));
    }
    if start
        .checked_add_months(Months::new(MAX_MONTHS))
        .is_some_and(|limit| end >= limit)
    {
        return Err(AppError::validation(
            "休日を調べる期間は3年までにしてください。",
        ));
    }
    let mut conn = pool.acquire().await?;
    let settings = repo::settings::load(&mut conn).await?;
    let calendar = WorkCalendar::new(settings.day_count, settings.custom_holidays);
    Ok(calendar.holidays_in(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::{self, SettingsInput};
    use crate::domain::settings::Settings;
    use crate::domain::testing::{d, year_end_holiday};
    use crate::repo::db;

    #[tokio::test]
    async fn lists_national_and_custom_holidays() {
        let pool = db::open_in_memory().await.unwrap();
        let names = |hs: Vec<Holiday>| {
            hs.into_iter()
                .map(|h| format!("{} {}", h.date, h.name))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(list(&pool, d("2026-11-01"), d("2026-11-30")).await.unwrap()),
            vec!["2026-11-03 文化の日", "2026-11-23 勤労感謝の日"]
        );

        // Custom holidays are listed in 暦日 too (the Gantt shades them).
        settings::update(
            &pool,
            SettingsInput {
                custom_holidays: vec![year_end_holiday()],
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        let hs = list(&pool, d("2027-12-31"), d("2028-01-01")).await.unwrap();
        assert_eq!(
            hs.iter()
                .map(|h| (h.date, h.name.as_str(), h.custom))
                .collect::<Vec<_>>(),
            vec![
                (d("2027-12-31"), "年末年始", true),
                (d("2028-01-01"), "元日", false),
                (d("2028-01-01"), "年末年始", true),
            ]
        );
    }

    #[tokio::test]
    async fn the_period_is_checked() {
        let pool = db::open_in_memory().await.unwrap();
        assert!(list(&pool, d("2026-01-01"), d("2028-12-31")).await.is_ok());
        assert_eq!(
            list(&pool, d("2026-01-01"), d("2029-01-01"))
                .await
                .unwrap_err()
                .message,
            "休日を調べる期間は3年までにしてください。"
        );
        assert_eq!(
            list(&pool, d("2026-11-02"), d("2026-11-01"))
                .await
                .unwrap_err()
                .message,
            "休日を調べる期間の開始日が終了日より後になっています。日付を直してください。"
        );
    }
}
