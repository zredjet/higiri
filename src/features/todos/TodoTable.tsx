import { Badge, Icon, Table, type TableColumn } from "../../components/aizome";
import type { TodoListRow } from "../../lib/bindings";
import { formatDate } from "../../lib/date";
import { BLOCKED_LABEL, blockedMarkTitle } from "./dependencyText";
import { REPEAT_LABEL, repeatMarkTitle } from "./repeatText";
import {
  childProgressText,
  dueNote,
  periodText,
  PRIORITY_LABEL,
  STATUS_LABEL,
  STATUS_TONE,
  subLine,
  warningTexts,
} from "./todoText";
import { foldRows, parentsWithChildRows, type TreeInfo } from "./tree";
import { useFolded } from "./useFolded";
import { useVirtualRows } from "./useVirtualRows";

interface TodoTableProps {
  rows: TodoListRow[];
  /** Adds a プロジェクト column (the all-projects list). */
  showProject: boolean;
  selectedId: number | null;
  onOpen: (id: number) => void;
  emptyText: string;
}

const treeInfo = (r: TodoListRow): TreeInfo => ({ id: r.item.todo.id, parentId: r.item.todo.parent_id, child: r.child });

/**
 * The TODO list (docs/ui-mock/Todos.dc.html), plus 優先度 and the warnings.
 * Children come indented under their parent (S-1), which can be folded; a
 * parent that does not match the filters is shown dimmed (a bridge row).
 * A repeating TODO has the repeat mark next to its title (T-12), and one
 * waiting for unfinished TODOs the link mark (T-11).
 * Long lists draw only the rows near the screen (useVirtualRows).
 */
export function TodoTable({ rows, showProject, selectedId, onOpen, emptyText }: TodoTableProps) {
  const { folded, toggle } = useFolded();
  const parents = parentsWithChildRows(rows, treeInfo);
  const drawn = foldRows(rows, folded, treeInfo);
  const { window: shown, wrapperProps } = useVirtualRows(drawn.map((r) => r.item.todo.id));
  const columns: TableColumn<TodoListRow>[] = [
    {
      key: "title",
      header: "TODO",
      render: ({ item: { todo, children, repeat, blocked_by }, child, bridge }) => {
        const sub = bridge ? null : subLine(todo);
        const closed = todo.status === "done" || todo.status === "cancelled";
        const progress = childProgressText(children);
        const canFold = parents.has(todo.id);
        const isFolded = folded.has(todo.id);
        return (
          <div className={child ? "todo-cell todo-cell--child" : "todo-cell"}>
            <span className="todo-cell__head">
              {canFold ? (
                <button
                  type="button"
                  className="tree-toggle"
                  aria-expanded={!isFolded}
                  aria-label={`「${todo.title}」の子TODOを${isFolded ? "開く" : "折りたたむ"}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    toggle(todo.id);
                  }}
                >
                  <Icon name={isFolded ? "chevron-right" : "chevron-down"} size={16} />
                </button>
              ) : null}
              <span className={closed || bridge ? "todo-cell__title is-closed" : "todo-cell__title"}>{todo.title}</span>
              {repeat ? (
                <span className="repeat-mark" title={repeatMarkTitle(repeat)}>
                  <Icon name="repeat" size={14} label={REPEAT_LABEL} />
                </span>
              ) : null}
              {blocked_by.length > 0 && !bridge ? (
                <span className="blocked-mark" title={blockedMarkTitle(blocked_by)}>
                  <Icon name="link" size={14} label={BLOCKED_LABEL} />
                </span>
              ) : null}
              {progress ? <span className="todo-cell__progress">{progress}</span> : null}
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
            render: (r: TodoListRow) => <span className="text-muted">{r.item.project_name}</span>,
          },
        ]
      : []),
    {
      key: "status",
      header: "ステータス",
      width: 96,
      render: ({ item: { todo } }) => (
        <Badge tone={STATUS_TONE[todo.status]} dot>
          {STATUS_LABEL[todo.status]}
        </Badge>
      ),
    },
    {
      key: "priority",
      header: "優先度",
      width: 64,
      render: ({ item: { todo }, bridge }) => (
        <span className={bridge ? "text-muted" : undefined}>{PRIORITY_LABEL[todo.priority]}</span>
      ),
    },
    {
      key: "due",
      header: "日限",
      width: 120,
      render: ({ item: { todo, assessment }, bridge }) => {
        const note = bridge ? null : dueNote(assessment);
        return (
          <div className="todo-cell">
            <span className={bridge ? "tabular text-muted" : "tabular"}>{todo.due_date ? formatDate(todo.due_date) : "なし"}</span>
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
      render: ({ item: { todo, assessment, due_after_parent, starts_before_dependency }, bridge }) => (
        <div className={bridge ? "todo-period text-muted" : "todo-period"}>
          <span>{periodText("予定", todo.planned_start, todo.planned_end)}</span>
          <span className="az-table__muted">{periodText("実績", todo.actual_start, todo.actual_end)}</span>
          {bridge
            ? null
            : warningTexts(assessment, due_after_parent, starts_before_dependency).map((w) => (
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
    <div {...wrapperProps} className={wrapperProps.className ? `todo-table ${wrapperProps.className}` : "todo-table"}>
      <Table<TodoListRow & { id: number }>
        columns={columns}
        rows={drawn.slice(shown.first, shown.end).map((r) => ({ ...r, id: r.item.todo.id }))}
        rowKey="id"
        selectedKey={selectedId ?? undefined}
        onRowClick={(r) => onOpen(r.item.todo.id)}
        emptyText={emptyText}
      />
    </div>
  );
}
