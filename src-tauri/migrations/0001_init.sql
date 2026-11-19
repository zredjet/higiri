CREATE TABLE projects (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  color       TEXT NOT NULL DEFAULT '#3B6FD4',
  description TEXT NOT NULL DEFAULT '',
  sort_order  INTEGER NOT NULL DEFAULT 0,
  archived_at TEXT,                        -- NULL = 稼働中
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE TABLE todos (
  id             INTEGER PRIMARY KEY,
  project_id     INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  parent_id      INTEGER REFERENCES todos(id) ON DELETE CASCADE,  -- サブTODO用。MVPでは常にNULL
  title          TEXT NOT NULL,
  status         TEXT NOT NULL DEFAULT 'todo'
                 CHECK (status IN ('todo', 'doing', 'waiting', 'hold', 'done', 'cancelled')),
  priority       INTEGER NOT NULL DEFAULT 2,  -- 1=高 2=中 3=低
  due_date       TEXT,                        -- 日限
  planned_start  TEXT,                        -- 予定期間
  planned_end    TEXT,
  actual_start   TEXT,                        -- 実績期間
  actual_end     TEXT,
  waiting_on     TEXT,                        -- 待ち:誰・何を待っているか
  waiting_since  TEXT,                        -- 待ち:待ちに入った日(自動)
  follow_up_date TEXT,                        -- 待ち:次に確認する日(確認日)
  closed_at      TEXT,                        -- 完了・中止にした日時
  close_reason   TEXT,                        -- 中止の理由
  memo_md        TEXT NOT NULL DEFAULT '',    -- メモ(現在の作業状況)
  sort_order     INTEGER NOT NULL DEFAULT 0,
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL,
  CHECK (planned_start IS NULL OR planned_end IS NULL OR planned_start <= planned_end),
  CHECK (actual_start  IS NULL OR actual_end  IS NULL OR actual_start  <= actual_end)
);
CREATE INDEX idx_todos_project   ON todos(project_id, status);
CREATE INDEX idx_todos_due       ON todos(due_date) WHERE status NOT IN ('done', 'cancelled');
CREATE INDEX idx_todos_follow_up ON todos(follow_up_date) WHERE status = 'waiting';
CREATE INDEX idx_todos_parent    ON todos(parent_id);

CREATE TABLE work_logs (
  id         INTEGER PRIMARY KEY,
  todo_id    INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
  log_date   TEXT NOT NULL,                -- 作業した日
  body_md    TEXT NOT NULL,
  minutes    INTEGER,                      -- 作業時間(任意)
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_work_logs_date ON work_logs(log_date);
CREATE INDEX idx_work_logs_todo ON work_logs(todo_id, log_date);

CREATE TABLE todo_events (
  id          INTEGER PRIMARY KEY,
  todo_id     INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,
  event_date  TEXT NOT NULL,               -- 発生した日
  kind        TEXT NOT NULL,               -- 'created' | 'status' | 'due_date' | 'planned' | 'actual' | 'follow_up'
  old_value   TEXT,
  new_value   TEXT,
  note        TEXT,                        -- 待ちの相手、中止の理由など
  occurred_at TEXT NOT NULL
);
CREATE INDEX idx_todo_events_date ON todo_events(event_date);
CREATE INDEX idx_todo_events_todo ON todo_events(todo_id);

CREATE TABLE reports (
  kind         TEXT NOT NULL CHECK (kind IN ('daily', 'weekly', 'monthly')),
  period_start TEXT NOT NULL,              -- 日報はその日、週報は週の初日
  period_end   TEXT NOT NULL,
  body_md      TEXT NOT NULL,              -- 保存時点の全文
  note_md      TEXT NOT NULL DEFAULT '',   -- 所感・連絡事項(手入力)
  updated_at   TEXT NOT NULL,
  PRIMARY KEY (kind, period_start)
);

CREATE TABLE settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL                      -- JSON
);
