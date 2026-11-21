//! バックアップの複製先 (v0.4.0, C-3): whether the mirror misses daily backups,
//! and when the dashboard says so. Pure; the files are handled by `backup.rs`.
//!
//! Days are the days a daily backup was taken (the app was started), read
//! from the backups folder: a week without a launch and then one failure is
//! one day, not seven.

use chrono::NaiveDate;
use serde::Serialize;

/// Daily backups missing from the mirror before the dashboard says so.
pub const ALERT_DAYS: usize = 3;

/// 「バックアップを複製先に複製できない日が3日続いています」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub struct BackupMirrorAlert {
    /// The newest daily backup that reached the mirror (null: none yet).
    pub last_ok: Option<NaiveDate>,
    /// Daily backups taken since then (at least [`ALERT_DAYS`]).
    pub days: i64,
}

/// Daily backups (by their dates) newer than `last_ok`; all of them when
/// nothing reached the mirror yet.
pub fn missed(last_ok: Option<NaiveDate>, daily: &[NaiveDate]) -> usize {
    daily
        .iter()
        .filter(|&&d| last_ok.is_none_or(|ok| d > ok))
        .count()
}

/// Whether the newest daily backup has not reached the mirror yet (the
/// resident timer copies again then).
pub fn behind(last_ok: Option<NaiveDate>, daily: &[NaiveDate]) -> bool {
    missed(last_ok, daily) > 0
}

/// The dashboard's notice: a mirror is set and [`ALERT_DAYS`] or more daily
/// backups did not reach it.
pub fn alert(
    mirror_set: bool,
    last_ok: Option<NaiveDate>,
    daily: &[NaiveDate],
) -> Option<BackupMirrorAlert> {
    if !mirror_set {
        return None;
    }
    let days = missed(last_ok, daily);
    (days >= ALERT_DAYS).then_some(BackupMirrorAlert {
        last_ok,
        days: days as i64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::d;

    fn days(list: &[&str]) -> Vec<NaiveDate> {
        list.iter().map(|s| d(s)).collect()
    }

    #[test]
    fn no_mirror_no_notice() {
        let daily = days(&["2026-11-19", "2026-11-18", "2026-11-17", "2026-11-16"]);
        assert_eq!(alert(false, Some(d("2026-11-10")), &daily), None);
        assert_eq!(alert(false, None, &daily), None);
    }

    #[test]
    fn two_missed_days_are_not_told_three_are() {
        let last_ok = Some(d("2026-11-16"));
        let two = days(&["2026-11-18", "2026-11-17", "2026-11-16", "2026-11-13"]);
        assert_eq!(missed(last_ok, &two), 2);
        assert_eq!(alert(true, last_ok, &two), None);

        let three = days(&["2026-11-19", "2026-11-18", "2026-11-17", "2026-11-16"]);
        assert_eq!(
            alert(true, last_ok, &three),
            Some(BackupMirrorAlert { last_ok, days: 3 })
        );
    }

    #[test]
    fn days_are_launch_days_not_calendar_days() {
        // A week without a launch, then one failed day: one missed backup.
        let last_ok = Some(d("2026-11-09"));
        let daily = days(&["2026-11-19", "2026-11-09", "2026-11-08"]);
        assert_eq!(missed(last_ok, &daily), 1);
        assert_eq!(alert(true, last_ok, &daily), None);
    }

    #[test]
    fn nothing_mirrored_yet_counts_every_daily_backup() {
        let daily = days(&["2026-11-19", "2026-11-18"]);
        assert_eq!(alert(true, None, &daily), None);
        assert!(behind(None, &daily));
        let daily = days(&["2026-11-19", "2026-11-18", "2026-11-17"]);
        assert_eq!(
            alert(true, None, &daily),
            Some(BackupMirrorAlert {
                last_ok: None,
                days: 3
            })
        );
    }

    #[test]
    fn behind_only_when_the_newest_backup_is_missing() {
        let daily = days(&["2026-11-19", "2026-11-18"]);
        assert!(!behind(Some(d("2026-11-19")), &daily));
        assert!(behind(Some(d("2026-11-18")), &daily));
        assert!(!behind(None, &[]));
        assert!(!behind(Some(d("2026-11-19")), &[]));
    }
}
