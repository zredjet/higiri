// Temporary check screen for phase 2: calls the commands through the generated
// bindings and shows the raw results. Replaced by the real layout in phase 3.
import { useEffect, useState } from "react";
import {
  commands,
  type AppError,
  type ProjectSummary,
  type Status,
  type TodoItem,
  type Urgency,
} from "../lib/bindings";

const STATUS_LABEL: Record<Status, string> = {
  todo: "未着手",
  doing: "作業中",
  waiting: "待ち",
  hold: "保留",
  done: "完了",
  cancelled: "中止",
};

const URGENCY_LABEL: Record<Urgency, string> = {
  overdue: "超過",
  due_today: "本日",
  due_soon: "間近",
  follow_up: "要確認",
  late_start: "着手遅れ",
};

function errorText(e: unknown): string {
  if (typeof e === "object" && e !== null && "message" in e) {
    return (e as AppError).message;
  }
  return String(e);
}

export default function App() {
  const [projects, setProjects] = useState<ProjectSummary[] | null>(null);
  const [todos, setTodos] = useState<TodoItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      commands.listProjects(false),
      commands.listTodos({ project_id: null, statuses: null }),
    ])
      .then(([p, t]) => {
        setProjects(p);
        setTodos(t);
      })
      .catch((e: unknown) => setError(errorText(e)));
  }, []);

  return (
    <main>
      <h1>Higiri</h1>
      {error && <p role="alert">{error}</p>}
      {!error && (projects === null || todos === null) && <p>読み込んでいます。</p>}

      {projects && (
        <section>
          <h2>list_projects</h2>
          <table>
            <thead>
              <tr>
                <th>プロジェクト</th>
                <th>完了 / 総数(中止を除く)</th>
                <th>日限注意</th>
              </tr>
            </thead>
            <tbody>
              {projects.map((p) => (
                <tr key={p.project.id}>
                  <td>{p.project.name}</td>
                  <td>
                    {p.progress_done} / {p.progress_total}
                  </td>
                  <td>{p.attention_count}件</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {todos && (
        <section>
          <h2>list_todos</h2>
          <table>
            <thead>
              <tr>
                <th>プロジェクト</th>
                <th>TODO</th>
                <th>ステータス</th>
                <th>日限</th>
                <th>区分</th>
                <th>日限まで(日)</th>
                <th>待ち(日目)</th>
              </tr>
            </thead>
            <tbody>
              {todos.map(({ todo, project_name, assessment }) => (
                <tr key={todo.id}>
                  <td>{project_name}</td>
                  <td>{todo.title}</td>
                  <td>{STATUS_LABEL[todo.status]}</td>
                  <td>{todo.due_date ?? "なし"}</td>
                  <td>{assessment.urgency ? URGENCY_LABEL[assessment.urgency] : "なし"}</td>
                  <td>{assessment.days_until_due ?? "-"}</td>
                  <td>{assessment.waiting_day ?? "-"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}
    </main>
  );
}
