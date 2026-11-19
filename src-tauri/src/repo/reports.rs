//! `reports` table (saved daily reports; weekly / monthly later).

use chrono::NaiveDate;
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_required_date, read_ts, ts_param};
use crate::domain::date::format_date;
use crate::domain::report::{Report, ReportKind};

#[derive(FromRow)]
struct ReportRow {
    kind: String,
    period_start: String,
    period_end: String,
    body_md: String,
    note_md: String,
    updated_at: String,
}

impl ReportRow {
    fn into_report(self) -> Result<Report> {
        let kind = ReportKind::parse(&self.kind).ok_or(RepoError::Corrupt {
            column: "reports.kind",
            value: self.kind.clone(),
        })?;
        Ok(Report {
            kind,
            period_start: read_required_date("reports.period_start", self.period_start)?,
            period_end: read_required_date("reports.period_end", self.period_end)?,
            body_md: self.body_md,
            note_md: self.note_md,
            updated_at: read_ts("reports.updated_at", self.updated_at)?,
        })
    }
}

pub async fn get(
    conn: &mut SqliteConnection,
    kind: ReportKind,
    start: NaiveDate,
) -> Result<Option<Report>> {
    sqlx::query_as::<_, ReportRow>("SELECT * FROM reports WHERE kind = ?1 AND period_start = ?2")
        .bind(kind.as_str())
        .bind(format_date(start))
        .fetch_optional(&mut *conn)
        .await?
        .map(ReportRow::into_report)
        .transpose()
}

/// Every saved report (checking that imported data can be read).
pub async fn list_all(conn: &mut SqliteConnection) -> Result<Vec<Report>> {
    sqlx::query_as::<_, ReportRow>("SELECT * FROM reports ORDER BY kind, period_start")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(ReportRow::into_report)
        .collect()
}

/// Inserts or replaces the report of (kind, period_start).
pub async fn upsert(conn: &mut SqliteConnection, r: &Report) -> Result<Report> {
    sqlx::query_as::<_, ReportRow>(
        "INSERT INTO reports (kind, period_start, period_end, body_md, note_md, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(kind, period_start) DO UPDATE SET
           period_end = excluded.period_end,
           body_md = excluded.body_md,
           note_md = excluded.note_md,
           updated_at = excluded.updated_at
         RETURNING *",
    )
    .bind(r.kind.as_str())
    .bind(format_date(r.period_start))
    .bind(format_date(r.period_end))
    .bind(&r.body_md)
    .bind(&r.note_md)
    .bind(ts_param(r.updated_at))
    .fetch_one(&mut *conn)
    .await?
    .into_report()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{d, ts};
    use crate::repo::testing::conn;

    fn report(body: &str, note: &str, at: chrono::DateTime<chrono::Utc>) -> Report {
        Report {
            kind: ReportKind::Daily,
            period_start: d("2026-11-19"),
            period_end: d("2026-11-19"),
            body_md: body.into(),
            note_md: note.into(),
            updated_at: at,
        }
    }

    #[tokio::test]
    async fn upsert_then_get_and_overwrite() {
        let (_pool, mut c) = conn().await;
        assert_eq!(
            get(&mut c, ReportKind::Daily, d("2026-11-19"))
                .await
                .unwrap(),
            None
        );
        let first = upsert(&mut c, &report("本文1", "所感1", ts(2026, 11, 19, 9, 0)))
            .await
            .unwrap();
        assert_eq!(
            get(&mut c, ReportKind::Daily, d("2026-11-19"))
                .await
                .unwrap(),
            Some(first)
        );
        let second = upsert(&mut c, &report("本文2", "所感2", ts(2026, 11, 19, 10, 0)))
            .await
            .unwrap();
        assert_eq!(second.body_md, "本文2");
        let rows: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM reports")
            .fetch_one(&mut *c)
            .await
            .unwrap();
        assert_eq!(rows.0, 1);
        // Another kind with the same start is a different row.
        assert_eq!(
            get(&mut c, ReportKind::Weekly, d("2026-11-19"))
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn the_check_constraint_guards_the_kind() {
        let (_pool, mut c) = conn().await;
        let err: RepoError = sqlx::query(
            "INSERT INTO reports (kind, period_start, period_end, body_md, updated_at)
             VALUES ('yearly', '2026-01-01', '2026-12-31', '', '2026-01-01T00:00:00.000Z')",
        )
        .execute(&mut *c)
        .await
        .unwrap_err()
        .into();
        assert!(err.is_constraint_violation());
    }
}
