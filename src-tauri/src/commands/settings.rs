//! 設定: get_settings, update_settings.
//!
//! `update_settings` saves in the inner function (database only), then the
//! command applies the result to the running app and the OS: the resident
//! timer and the × button (`resident::apply`), and start at login
//! (`autostart::apply`).

use serde::Deserialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, State};

use super::error::{AppError, CmdResult};
use super::validate;
use crate::domain::calendar::{CUSTOM_DAYS_MAX, CUSTOM_NAME_MAX, CustomHolidayError};
use crate::domain::notify::{self, TimesError};
use crate::domain::report_template::{HEADING_MAX, SectionKind, TITLE_MAX, TemplateError};
use crate::domain::settings::{
    CustomHoliday, DayCount, ReminderFrequency, ReportTemplate, Settings, Theme,
};
use crate::{autostart, repo, resident};

/// Days accepted for 「間近」とみなす日数 and 確認日の初期値.
const DAYS_MIN: i64 = 1;
const DAYS_MAX: i64 = 30;

/// Settings the user edits on the settings screen. `last_reminded_on`,
/// `last_notified_at` and `tray_hint_shown` are written by the app, not here.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct SettingsInput {
    pub reminder_frequency: ReminderFrequency,
    pub soon_days: u32,
    pub follow_up_days: u32,
    pub theme: Theme,
    pub notify_enabled: bool,
    /// `HH:MM`, 1 to 5, distinct (saved in time order).
    pub notify_times: Vec<String>,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub day_count: DayCount,
    /// Names 1 to 30 characters, `start <= end`, at most 31 days each.
    pub custom_holidays: Vec<CustomHoliday>,
    /// Title 1 to 60 characters; headings 1 to 40 characters, all different.
    /// Saved normalized (see `ReportTemplate::checked`).
    pub report_template: ReportTemplate,
}

impl From<&Settings> for SettingsInput {
    fn from(s: &Settings) -> SettingsInput {
        SettingsInput {
            reminder_frequency: s.reminder_frequency,
            soon_days: s.soon_days,
            follow_up_days: s.follow_up_days,
            theme: s.theme,
            notify_enabled: s.notify_enabled,
            notify_times: s.notify_times.clone(),
            close_to_tray: s.close_to_tray,
            autostart: s.autostart,
            day_count: s.day_count,
            custom_holidays: s.custom_holidays.clone(),
            report_template: s.report_template.clone(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(pool: State<'_, SqlitePool>) -> CmdResult<Settings> {
    get(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    input: SettingsInput,
) -> CmdResult<Settings> {
    let before = get(&pool).await?;
    let saved = update(&pool, input.clone()).await?;
    if saved.autostart != before.autostart
        && let Err(e) = autostart::apply(&app, saved.autostart)
    {
        // Keep the switch as the OS has it.
        let reverted = update(
            &pool,
            SettingsInput {
                autostart: before.autostart,
                ..input
            },
        )
        .await?;
        resident::apply(&app, &reverted);
        return Err(e);
    }
    resident::apply(&app, &saved);
    Ok(saved)
}

pub async fn get(pool: &SqlitePool) -> CmdResult<Settings> {
    let mut conn = pool.acquire().await?;
    Ok(repo::settings::load(&mut conn).await?)
}

pub async fn update(pool: &SqlitePool, input: SettingsInput) -> CmdResult<Settings> {
    let unit = match input.day_count {
        DayCount::Calendar => "日",
        DayCount::Business => "営業日",
    };
    validate::in_range(
        i64::from(input.soon_days),
        DAYS_MIN,
        DAYS_MAX,
        "「間近」とみなす日数",
        unit,
    )?;
    validate::in_range(
        i64::from(input.follow_up_days),
        DAYS_MIN,
        DAYS_MAX,
        "確認日の初期値",
        unit,
    )?;
    let notify_times = notify::normalize_times(&input.notify_times).map_err(times_error)?;
    let custom_holidays = input
        .custom_holidays
        .iter()
        .map(|h| h.checked().map_err(|e| holiday_error(h, e)))
        .collect::<CmdResult<Vec<_>>>()?;
    let report_template = input
        .report_template
        .checked()
        .map_err(template_error)?;
    let mut tx = pool.begin().await?;
    let current = repo::settings::load(&mut tx).await?;
    let next = Settings {
        reminder_frequency: input.reminder_frequency,
        soon_days: input.soon_days,
        follow_up_days: input.follow_up_days,
        theme: input.theme,
        notify_enabled: input.notify_enabled,
        notify_times,
        close_to_tray: input.close_to_tray,
        autostart: input.autostart,
        day_count: input.day_count,
        custom_holidays,
        report_template,
        ..current
    };
    repo::settings::save_user_settings(&mut tx, &next).await?;
    tx.commit().await?;
    Ok(next)
}

fn holiday_error(h: &CustomHoliday, e: CustomHolidayError) -> AppError {
    let name = h.name.trim();
    AppError::validation(match e {
        CustomHolidayError::EmptyName => "独自の休日の名前を入力してください。".to_owned(),
        CustomHolidayError::NameTooLong => {
            format!("独自の休日の名前は{CUSTOM_NAME_MAX}字までにしてください。")
        }
        CustomHolidayError::Reversed => format!(
            "独自の休日「{name}」の開始日が終了日より後になっています。日付を直してください。"
        ),
        CustomHolidayError::TooLong => format!(
            "独自の休日は1件{CUSTOM_DAYS_MAX}日までです。「{name}」の期間を短くするか、分けて登録してください。"
        ),
    })
}

fn template_error(e: TemplateError) -> AppError {
    let section = |k: SectionKind| k.default_heading();
    AppError::validation(match e {
        TemplateError::EmptyTitle => "日報の題名を入力してください。".to_owned(),
        TemplateError::TitleTooLong => {
            format!("日報の題名は{TITLE_MAX}字までにしてください。")
        }
        TemplateError::EmptyHeading(k) => {
            format!("「{}」の節の見出しを入力してください。", section(k))
        }
        TemplateError::HeadingTooLong(k) => format!(
            "「{}」の節の見出しは{HEADING_MAX}字までにしてください。",
            section(k)
        ),
        TemplateError::LineBreak => {
            "日報の題名と見出しに改行は使えません。1行で入力してください。".to_owned()
        }
        TemplateError::DuplicateHeading {
            heading,
            first,
            second,
        } => format!(
            "見出し「{heading}」が「{}」と「{}」の節で重なっています。節ごとに違う見出しにしてください。",
            section(first),
            section(second)
        ),
    })
}

fn times_error(e: TimesError) -> AppError {
    AppError::validation(match e {
        TimesError::Format(s) => format!(
            "通知の時刻「{s}」を読み取れませんでした。「09:00」のように、時と分を「:」で区切って入力してください。"
        ),
        TimesError::TooFew => "通知の時刻を1つ以上登録してください。通知を止めるときは「OSの通知を出す」をオフにしてください。".to_owned(),
        TimesError::TooMany => format!(
            "通知の時刻は{}つまで登録できます。いらない時刻を削除してから追加してください。",
            notify::TIMES_MAX
        ),
        TimesError::Duplicate(s) => {
            format!("通知の時刻「{s}」はすでに登録されています。別の時刻を入力してください。")
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::ErrorKind;
    use crate::domain::testing::{d, ts};
    use crate::repo::db;

    fn input(soon: u32, follow: u32) -> SettingsInput {
        SettingsInput {
            reminder_frequency: ReminderFrequency::OncePerDay,
            soon_days: soon,
            follow_up_days: follow,
            theme: Theme::Dark,
            ..SettingsInput::from(&Settings::default())
        }
    }

    fn times(list: &[&str]) -> SettingsInput {
        SettingsInput {
            notify_times: list.iter().map(|s| s.to_string()).collect(),
            ..SettingsInput::from(&Settings::default())
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

    #[tokio::test]
    async fn notification_and_tray_settings_are_saved() {
        let pool = db::open_in_memory().await.unwrap();
        let s = update(
            &pool,
            SettingsInput {
                notify_enabled: false,
                notify_times: vec!["17:30".into(), "8:00".into()],
                close_to_tray: false,
                autostart: true,
                ..SettingsInput::from(&Settings::default())
            },
        )
        .await
        .unwrap();
        assert!(!s.notify_enabled);
        // Saved as HH:MM in time order.
        assert_eq!(s.notify_times, ["08:00", "17:30"]);
        assert!(!s.close_to_tray);
        assert!(s.autostart);
        assert_eq!(get(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn notification_times_are_checked() {
        let pool = db::open_in_memory().await.unwrap();
        let message = |e: AppError| {
            assert_eq!(e.kind, ErrorKind::Validation);
            e.message
        };
        assert_eq!(
            message(update(&pool, times(&["9時"])).await.unwrap_err()),
            "通知の時刻「9時」を読み取れませんでした。「09:00」のように、時と分を「:」で区切って入力してください。"
        );
        assert_eq!(
            message(update(&pool, times(&[])).await.unwrap_err()),
            "通知の時刻を1つ以上登録してください。通知を止めるときは「OSの通知を出す」をオフにしてください。"
        );
        assert_eq!(
            message(
                update(
                    &pool,
                    times(&["08:00", "09:00", "10:00", "11:00", "12:00", "13:00"])
                )
                .await
                .unwrap_err()
            ),
            "通知の時刻は5つまで登録できます。いらない時刻を削除してから追加してください。"
        );
        assert_eq!(
            message(update(&pool, times(&["09:00", "9:00"])).await.unwrap_err()),
            "通知の時刻「09:00」はすでに登録されています。別の時刻を入力してください。"
        );
        assert_eq!(get(&pool).await.unwrap(), Settings::default());
        // Five distinct times are fine.
        let s = update(&pool, times(&["08:00", "09:00", "10:00", "11:00", "12:00"]))
            .await
            .unwrap();
        assert_eq!(s.notify_times.len(), 5);
    }

    fn holidays(list: Vec<CustomHoliday>) -> SettingsInput {
        SettingsInput {
            day_count: DayCount::Business,
            custom_holidays: list,
            ..SettingsInput::from(&Settings::default())
        }
    }

    fn holiday(name: &str, start: &str, end: &str) -> CustomHoliday {
        CustomHoliday {
            name: name.into(),
            start: d(start),
            end: d(end),
            yearly: true,
        }
    }

    #[tokio::test]
    async fn day_count_and_custom_holidays_are_saved() {
        let pool = db::open_in_memory().await.unwrap();
        let s = update(
            &pool,
            holidays(vec![holiday(" 年末年始 ", "2026-12-29", "2027-01-03")]),
        )
        .await
        .unwrap();
        assert_eq!(s.day_count, DayCount::Business);
        assert_eq!(
            s.custom_holidays,
            vec![holiday("年末年始", "2026-12-29", "2027-01-03")]
        );
        assert_eq!(get(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn custom_holidays_are_checked() {
        let pool = db::open_in_memory().await.unwrap();
        let message = |list| {
            let pool = pool.clone();
            async move { update(&pool, holidays(list)).await.unwrap_err().message }
        };
        assert_eq!(
            message(vec![holiday(" ", "2026-12-29", "2026-12-29")]).await,
            "独自の休日の名前を入力してください。"
        );
        assert_eq!(
            message(vec![holiday(&"休".repeat(31), "2026-12-29", "2026-12-29")]).await,
            "独自の休日の名前は30字までにしてください。"
        );
        assert_eq!(
            message(vec![holiday("夏季休業", "2026-08-14", "2026-08-12")]).await,
            "独自の休日「夏季休業」の開始日が終了日より後になっています。日付を直してください。"
        );
        assert_eq!(
            message(vec![holiday("夏季休業", "2026-08-01", "2026-09-01")]).await,
            "独自の休日は1件31日までです。「夏季休業」の期間を短くするか、分けて登録してください。"
        );
        assert_eq!(get(&pool).await.unwrap(), Settings::default());
        // 31 days and 30 characters are fine.
        assert!(
            update(
                &pool,
                holidays(vec![holiday(&"休".repeat(30), "2026-08-01", "2026-08-31")])
            )
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn day_errors_name_business_days() {
        let pool = db::open_in_memory().await.unwrap();
        let e = update(
            &pool,
            SettingsInput {
                soon_days: 31,
                ..holidays(vec![])
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            e.message,
            "「間近」とみなす日数は1〜30営業日の範囲で入力してください。"
        );
    }

    fn template(change: impl FnOnce(&mut ReportTemplate)) -> SettingsInput {
        let mut t = ReportTemplate::default();
        change(&mut t);
        SettingsInput {
            report_template: t,
            ..SettingsInput::from(&Settings::default())
        }
    }

    fn heading(t: &mut ReportTemplate, kind: SectionKind, text: &str) {
        t.sections
            .iter_mut()
            .find(|s| s.kind() == kind)
            .unwrap()
            .heading = text.into();
    }

    #[tokio::test]
    async fn report_template_is_saved_normalized() {
        let pool = db::open_in_memory().await.unwrap();
        let s = update(
            &pool,
            template(|t| {
                t.title = " # {日付} 業務報告 ".into();
                heading(t, SectionKind::Waiting, " ## 待ち状況 ");
                // 所感 first and 日限注意 missing.
                t.sections.rotate_right(1);
                t.sections.retain(|s| s.kind() != SectionKind::Attention);
            }),
        )
        .await
        .unwrap();
        let t = &s.report_template;
        assert_eq!(t.title, "{日付} 業務報告");
        assert_eq!(t.section(SectionKind::Waiting).unwrap().heading, "待ち状況");
        let kinds: Vec<SectionKind> = t.sections.iter().map(|s| s.kind()).collect();
        assert_eq!(
            kinds,
            [
                SectionKind::Work,
                SectionKind::Closed,
                SectionKind::Waiting,
                SectionKind::Tomorrow,
                SectionKind::Attention,
                SectionKind::Note,
            ]
        );
        assert_eq!(get(&pool).await.unwrap(), s);
    }

    #[tokio::test]
    async fn report_template_is_checked() {
        let pool = db::open_in_memory().await.unwrap();
        let message = |input| {
            let pool = pool.clone();
            async move { update(&pool, input).await.unwrap_err().message }
        };
        assert_eq!(
            message(template(|t| t.title = " ".into())).await,
            "日報の題名を入力してください。"
        );
        assert_eq!(
            message(template(|t| t.title = "題".repeat(61))).await,
            "日報の題名は60字までにしてください。"
        );
        assert_eq!(
            message(template(|t| heading(t, SectionKind::Tomorrow, ""))).await,
            "「明日の予定」の節の見出しを入力してください。"
        );
        assert_eq!(
            message(template(|t| heading(
                t,
                SectionKind::Note,
                &"見".repeat(41)
            )))
            .await,
            "「所感・連絡事項」の節の見出しは40字までにしてください。"
        );
        assert_eq!(
            message(template(|t| heading(t, SectionKind::Note, "## 待ち"))).await,
            "見出し「待ち」が「待ち」と「所感・連絡事項」の節で重なっています。節ごとに違う見出しにしてください。"
        );
        assert_eq!(
            message(template(|t| heading(t, SectionKind::Work, "作業\nログ"))).await,
            "日報の題名と見出しに改行は使えません。1行で入力してください。"
        );
        assert_eq!(get(&pool).await.unwrap(), Settings::default());
    }

    #[tokio::test]
    async fn saving_keeps_the_values_the_app_writes() {
        let pool = db::open_in_memory().await.unwrap();
        {
            let mut c = pool.acquire().await.unwrap();
            repo::settings::set_last_reminded_on(&mut c, Some(d("2026-11-19")))
                .await
                .unwrap();
            repo::settings::set_last_notified_at(&mut c, Some(ts(2026, 11, 19, 0, 0)))
                .await
                .unwrap();
            repo::settings::set_tray_hint_shown(&mut c, true)
                .await
                .unwrap();
        }
        let s = update(&pool, input(5, 2)).await.unwrap();
        assert_eq!(s.last_reminded_on, Some(d("2026-11-19")));
        assert_eq!(s.last_notified_at, Some(ts(2026, 11, 19, 0, 0)));
        assert!(s.tray_hint_shown);
        assert_eq!(get(&pool).await.unwrap(), s);
    }
}
