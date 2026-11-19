import { Badge, Icon, Table, type TableColumn } from "../../components/aizome";
import type { TodoItem } from "../../lib/bindings";
import { formatDate } from "../../lib/date";
import {
  dueNote,
  periodText,
  PRIORITY_LABEL,
  STATUS_LABEL,
  STATUS_TONE,
  subLine,
  warningTexts,
} from "./todoText";

interface TodoTableProps {
  items: TodoItem[];
  /** Adds a プロジェクト column (the all-projects list). */
  showProject: boolean;
  selectedId: number | null;
  onOpen: (id: number) => void;
  emptyText: string;
}

/** The TODO list (docs/ui-mock/Todos.dc.html), plus 優先度 and the warnings. */
export function TodoTable({ items, showProject, selectedId, onOpen, emptyText }: TodoTableProps) {
  const columns: TableColumn<TodoItem>[] = [
    {
      key: "title",
      header: "TODO",
      render: ({ todo }) => {
        const sub = subLine(todo);
        return (
          <div className="todo-cell">
            <span className={todo.status === "done" || todo.status === "cancelled" ? "todo-cell__title is-closed" : "todo-cell__title"}>
              {todo.title}
            </span>
            {sub ? <span className="todo-cell__sub">{sub}</span> : null}
          </div>
        );
      },
    },
    ...(showProject
      ? [
          {
            key: "project",
            header: "プロジェクト",
            width: 160,
            render: (i: TodoItem) => <span className="text-muted">{i.project_name}</span>,
          },
        ]
      : []),
    {
      key: "status",
      header: "ステータス",
      width: 96,
      render: ({ todo }) => (
        <Badge tone={STATUS_TONE[todo.status]} dot>
          {STATUS_LABEL[todo.status]}
        </Badge>
      ),
    },
    {
      key: "priority",
      header: "優先度",
      width: 64,
      render: ({ todo }) => PRIORITY_LABEL[todo.priority],
    },
    {
      key: "due",
      header: "日限",
      width: 120,
      render: ({ todo, assessment }) => {
        const note = dueNote(assessment);
        return (
          <div className="todo-cell">
            <span className="tabular">{todo.due_date ? formatDate(todo.due_date) : "なし"}</span>
            {note ? (
              <span className="todo-note" style={{ color: note.color }}>
                <Icon name={note.icon} size={14} />
                {note.text}
              </span>
            ) : null}
          </div>
        );
      },
    },
    {
      key: "period",
      header: "期間",
      width: 140,
      render: ({ todo, assessment }) => (
        <div className="todo-period">
          <span>{periodText("予定", todo.planned_start, todo.planned_end)}</span>
          <span className="az-table__muted">{periodText("実績", todo.actual_start, todo.actual_end)}</span>
          {warningTexts(assessment).map((w) => (
            <span key={w} className="todo-note" style={{ color: "var(--warning)" }}>
              <Icon name="alert-triangle" size={14} />
              {w}
            </span>
          ))}
        </div>
      ),
    },
  ];

  return (
    <Table<TodoItem>
      columns={columns}
      rows={items.map((i) => ({ ...i, id: i.todo.id }))}
      rowKey="id"
      selectedKey={selectedId ?? undefined}
      onRowClick={(i) => onOpen(i.todo.id)}
      emptyText={emptyText}
    />
  );
}
