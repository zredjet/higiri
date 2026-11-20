CREATE TABLE todo_templates (
  id         INTEGER PRIMARY KEY,
  name       TEXT NOT NULL UNIQUE,         -- 選ぶときの名前
  body       TEXT NOT NULL,                -- JSON。タイトル、優先度、メモ、子TODO一式
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE todo_repeats (
  todo_id    INTEGER PRIMARY KEY REFERENCES todos(id) ON DELETE CASCADE,  -- 繰り返しの今の回
  rule       TEXT NOT NULL,                -- JSON。周期
  until_date TEXT,                         -- 繰り返しの終わりの日。NULL = 終わりなし
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE todo_dependencies (
  todo_id       INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,  -- 後のTODO(取りかかれない側)
  depends_on_id INTEGER NOT NULL REFERENCES todos(id) ON DELETE CASCADE,  -- 先に終わらせるTODO
  created_at    TEXT NOT NULL,
  PRIMARY KEY (todo_id, depends_on_id),
  CHECK (todo_id <> depends_on_id)
);
CREATE INDEX idx_todo_dependencies_depends_on ON todo_dependencies(depends_on_id);
