//! `todo_templates` table (TODOテンプレート, v0.3.0).
//!
//! `body` is stored as JSON ([`TemplateBody`]). A body that cannot be read
//! (or does not pass its check) is an error, like any other stored value.
//! Templates point at no other table: deleting TODOs leaves them as they are.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, SqliteConnection};

use super::{RepoError, Result, read_ts, ts_param};
use crate::domain::template::{TemplateBody, TodoTemplate};

/// The fields of a template that are written (new or replaced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateFields {
    /// Unique; the database refuses a name that is taken (UNIQUE).
    pub name: String,
    pub body: TemplateBody,
}

#[derive(FromRow)]
struct TemplateRow {
    id: i64,
    name: String,
    body: String,
    created_at: String,
    updated_at: String,
}

impl TemplateRow {
    fn into_template(self) -> Result<TodoTemplate> {
        let body = TemplateBody::from_json(&self.body).ok_or(RepoError::Corrupt {
            column: "todo_templates.body",
            value: self.body.clone(),
        })?;
        Ok(TodoTemplate {
            id: self.id,
            name: self.name,
            body,
            created_at: read_ts("todo_templates.created_at", self.created_at)?,
            updated_at: read_ts("todo_templates.updated_at", self.updated_at)?,
        })
    }
}

fn not_found(id: i64) -> RepoError {
    RepoError::NotFound {
        entity: "todo template",
        id,
    }
}

/// Every template, by name.
pub async fn list(conn: &mut SqliteConnection) -> Result<Vec<TodoTemplate>> {
    sqlx::query_as::<_, TemplateRow>("SELECT * FROM todo_templates ORDER BY name, id")
        .fetch_all(&mut *conn)
        .await?
        .into_iter()
        .map(TemplateRow::into_template)
        .collect()
}

pub async fn get(conn: &mut SqliteConnection, id: i64) -> Result<TodoTemplate> {
    sqlx::query_as::<_, TemplateRow>("SELECT * FROM todo_templates WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| not_found(id))?
        .into_template()
}

pub async fn insert(
    conn: &mut SqliteConnection,
    fields: &TemplateFields,
    now: DateTime<Utc>,
) -> Result<TodoTemplate> {
    sqlx::query_as::<_, TemplateRow>(
        "INSERT INTO todo_templates (name, body, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?3)
         RETURNING *",
    )
    .bind(&fields.name)
    .bind(fields.body.to_json())
    .bind(ts_param(now))
    .fetch_one(&mut *conn)
    .await?
    .into_template()
}

/// Replaces the name and the body.
pub async fn update(
    conn: &mut SqliteConnection,
    id: i64,
    fields: &TemplateFields,
    now: DateTime<Utc>,
) -> Result<TodoTemplate> {
    sqlx::query_as::<_, TemplateRow>(
        "UPDATE todo_templates SET name = ?2, body = ?3, updated_at = ?4
         WHERE id = ?1
         RETURNING *",
    )
    .bind(id)
    .bind(&fields.name)
    .bind(fields.body.to_json())
    .bind(ts_param(now))
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| not_found(id))?
    .into_template()
}

pub async fn delete(conn: &mut SqliteConnection, id: i64) -> Result<()> {
    let done = sqlx::query("DELETE FROM todo_templates WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    if done.rows_affected() == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::Priority;
    use crate::domain::template::TemplateChild;
    use crate::domain::testing::ts;
    use crate::repo::testing::{conn, project, todo};
    use crate::repo::todos;

    fn fields(name: &str, title: &str) -> TemplateFields {
        TemplateFields {
            name: name.into(),
            body: TemplateBody {
                title: title.into(),
                priority: Priority::High,
                memo_md: "## 手順\n- 集計する".into(),
                children: vec![
                    TemplateChild {
                        title: "売上の集計".into(),
                        priority: Priority::High,
                        memo_md: String::new(),
                        due_offset_days: Some(-3),
                        depends_on: vec![],
                    },
                    TemplateChild {
                        title: "報告書の作成".into(),
                        priority: Priority::Medium,
                        memo_md: String::new(),
                        due_offset_days: None,
                        depends_on: vec![0],
                    },
                ],
            },
        }
    }

    #[tokio::test]
    async fn insert_get_list_update_delete() {
        let (_pool, mut c) = conn().await;
        let b = insert(
            &mut c,
            &fields("週次", "週次の確認"),
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        let a = insert(
            &mut c,
            &fields("月次締め", "月次締め"),
            ts(2026, 11, 19, 2, 0),
        )
        .await
        .unwrap();
        assert_eq!(a.body, fields("", "月次締め").body);
        assert_eq!(a.created_at, ts(2026, 11, 19, 2, 0));
        assert_eq!(get(&mut c, a.id).await.unwrap(), a);
        // By name (月 < 週 in UTF-8 order).
        let names: Vec<String> = list(&mut c)
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name)
            .collect();
        assert_eq!(names, ["月次締め", "週次"]);

        let mut changed = fields("月次締め(新)", "月次締め");
        changed.body.children.pop();
        let updated = update(&mut c, a.id, &changed, ts(2026, 11, 20, 0, 0))
            .await
            .unwrap();
        assert_eq!(updated.name, "月次締め(新)");
        assert_eq!(updated.body.children.len(), 1);
        assert_eq!(updated.created_at, a.created_at);
        assert_eq!(updated.updated_at, ts(2026, 11, 20, 0, 0));

        delete(&mut c, b.id).await.unwrap();
        assert!(matches!(
            get(&mut c, b.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(matches!(
            delete(&mut c, b.id).await,
            Err(RepoError::NotFound { .. })
        ));
        assert!(matches!(
            update(&mut c, b.id, &changed, ts(2026, 11, 20, 0, 0)).await,
            Err(RepoError::NotFound { .. })
        ));
        assert_eq!(list(&mut c).await.unwrap(), vec![updated]);
    }

    #[tokio::test]
    async fn the_body_is_stored_as_the_designed_json() {
        let (_pool, mut c) = conn().await;
        let t = insert(
            &mut c,
            &fields("月次締め", "月次締め"),
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        let stored: (String,) = sqlx::query_as("SELECT body FROM todo_templates WHERE id = ?1")
            .bind(t.id)
            .fetch_one(&mut *c)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&stored.0).unwrap();
        assert_eq!(v["priority"], "high");
        assert_eq!(v["children"][0]["due_offset_days"], -3);
        assert_eq!(v["children"][1]["due_offset_days"], serde_json::Value::Null);
        assert_eq!(v["children"][1]["depends_on"], serde_json::json!([0]));
    }

    #[tokio::test]
    async fn names_are_unique() {
        let (_pool, mut c) = conn().await;
        insert(&mut c, &fields("月次締め", "A"), ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        let other = insert(&mut c, &fields("週次", "B"), ts(2026, 11, 19, 1, 0))
            .await
            .unwrap();
        let err = insert(&mut c, &fields("月次締め", "C"), ts(2026, 11, 19, 2, 0))
            .await
            .unwrap_err();
        assert!(err.is_constraint_violation(), "{err}");
        let err = update(
            &mut c,
            other.id,
            &fields("月次締め", "B"),
            ts(2026, 11, 19, 2, 0),
        )
        .await
        .unwrap_err();
        assert!(err.is_constraint_violation(), "{err}");
    }

    #[tokio::test]
    async fn an_unreadable_body_is_an_error() {
        let (_pool, mut c) = conn().await;
        for (i, body) in [
            "not json",
            r#"{"title":"T"}"#,
            // JSON of the right shape that does not pass the check.
            r#"{"title":"T","priority":"medium","memo_md":"","children":[
                {"title":"子","priority":"high","memo_md":"","due_offset_days":0,"depends_on":[0]}]}"#,
        ]
        .into_iter()
        .enumerate()
        {
            sqlx::query(
                "INSERT INTO todo_templates (id, name, body, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '2026-11-19T00:00:00.000Z', '2026-11-19T00:00:00.000Z')",
            )
            .bind(100 + i as i64)
            .bind(format!("壊れた{i}"))
            .bind(body)
            .execute(&mut *c)
            .await
            .unwrap();
            let err = get(&mut c, 100 + i as i64).await.unwrap_err();
            assert!(
                matches!(
                    err,
                    RepoError::Corrupt {
                        column: "todo_templates.body",
                        ..
                    }
                ),
                "{body}: {err}"
            );
        }
        assert!(list(&mut c).await.is_err());
    }

    #[tokio::test]
    async fn deleting_todos_and_projects_leaves_templates() {
        let (_pool, mut c) = conn().await;
        let p = project(&mut c, "P").await;
        let t = todo(&mut c, p.id, "月次締め").await;
        let saved = insert(
            &mut c,
            &fields("月次締め", "月次締め"),
            ts(2026, 11, 19, 1, 0),
        )
        .await
        .unwrap();
        todos::delete(&mut c, t.id).await.unwrap();
        crate::repo::projects::delete(&mut c, p.id).await.unwrap();
        assert_eq!(list(&mut c).await.unwrap(), vec![saved]);
    }
}
