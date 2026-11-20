import { useState } from "react";
import { Alert, Badge, Button, Select, TextField } from "../../../components/aizome";
import type { TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { isComposing } from "../../../lib/keys";
import { useCreateTodo, useSetTodoParent, useTodoList } from "../../../lib/queries";
import { CHILDREN_ALL_CLOSED, childDoneText, STATUS_LABEL, STATUS_TONE, URGENCY_LABEL, URGENCY_TONE } from "../todoText";

/**
 * The TODOs of this one's project that the detail panel needs (S-1): its
 * children, and the TODOs it may be put under (no parent, not itself).
 * Read from the project's whole list, in its order.
 */
export function useFamily(item: TodoItem) {
  const { id, project_id } = item.todo;
  const list = useTodoList({ project_id, statuses: null, priority: null, due: null, sort: null });
  const rows = list.data && !list.isPlaceholderData ? list.data.rows : null;
  const items = (rows ?? []).map((r) => r.item).filter((i) => i.todo.project_id === project_id);
  return {
    loaded: rows !== null,
    children: items.filter((i) => i.todo.parent_id === id),
    candidates: items.filter((i) => i.todo.parent_id === null && i.todo.id !== id),
  };
}

const isClosed = (i: TodoItem) => i.todo.status === "done" || i.todo.status === "cancelled";

/**
 * 「親のTODO」: a child shows its parent (opens it), can move to another
 * parent or leave it; a TODO without children can pick a parent here.
 * Not shown for a parent (one level only).
 */
export function ParentField({
  item,
  candidates,
  loaded,
  onOpenTodo,
}: {
  item: TodoItem;
  candidates: TodoItem[];
  loaded: boolean;
  onOpenTodo: (id: number) => void;
}) {
  const setParent = useSetTodoParent();
  const parentId = item.todo.parent_id;
  const options = [
    { value: "", label: "なし" },
    ...candidates.map((c) => ({
      value: String(c.todo.id),
      label: isClosed(c) ? `${c.todo.title}(${STATUS_LABEL[c.todo.status]})` : c.todo.title,
    })),
  ];
  // Until the project's list is loaded, keep the current parent selectable.
  if (parentId !== null && !candidates.some((c) => c.todo.id === parentId)) {
    options.push({ value: String(parentId), label: item.parent_title ?? "" });
  }
  const change = (next: number | null) => {
    setParent.reset();
    setParent.mutate({ id: item.todo.id, parentId: next });
  };

  return (
    <div className="family">
      <Select
        label="親のTODO"
        size="sm"
        value={parentId === null ? "" : String(parentId)}
        options={options}
        disabled={setParent.isPending || !loaded}
        description={parentId === null ? "選ぶと、そのTODOの子TODOになります。" : undefined}
        onChange={(e) => change(e.target.value === "" ? null : Number(e.target.value))}
      />
      {parentId !== null ? (
        <div className="family__actions">
          <button type="button" className="family__link" onClick={() => onOpenTodo(parentId)}>
            「{item.parent_title}」を開く
          </button>
          <Button size="sm" variant="ghost" disabled={setParent.isPending} onClick={() => change(null)}>
            親から外す
          </Button>
        </div>
      ) : null}
      {setParent.error ? <Alert tone="danger">{errorMessage(setParent.error)}</Alert> : null}
    </div>
  );
}

/**
 * 「子TODO」 (a TODO without a parent): its children with their status and
 * 日限注意, the progress, and a field that adds a child on Enter.
 */
export function ChildrenSection({
  item,
  childItems,
  onOpenTodo,
}: {
  item: TodoItem;
  childItems: TodoItem[];
  onOpenTodo: (id: number) => void;
}) {
  const [title, setTitle] = useState("");
  const create = useCreateTodo();
  const counts = item.children;
  const allClosed = counts !== null && counts.open === 0 && !isClosed(item);

  return (
    <section className="family" aria-label="子TODO">
      <div className="family__head">
        <span className="family__label">子TODO</span>
        {counts && counts.total > 0 ? <span className="text-caption">{childDoneText(counts)}</span> : null}
      </div>
      {childItems.length > 0 ? (
        <ul className="family__list">
          {childItems.map((c) => (
            <li key={c.todo.id}>
              <button type="button" className="family-item" onClick={() => onOpenTodo(c.todo.id)}>
                <span className={isClosed(c) ? "family-item__title is-closed" : "family-item__title"}>{c.todo.title}</span>
                <span className="family-item__badges">
                  {c.assessment.urgency ? (
                    <Badge tone={URGENCY_TONE[c.assessment.urgency]} dot>
                      {URGENCY_LABEL[c.assessment.urgency]}
                    </Badge>
                  ) : null}
                  <Badge tone={STATUS_TONE[c.todo.status]} dot>
                    {STATUS_LABEL[c.todo.status]}
                  </Badge>
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : null}
      {allClosed ? <Alert tone="success">{CHILDREN_ALL_CLOSED}</Alert> : null}
      <TextField
        aria-label="子TODOを追加"
        size="sm"
        placeholder="子TODOのタイトルを入力して Enter で追加"
        icon="plus"
        value={title}
        disabled={create.isPending}
        error={create.error ? errorMessage(create.error) : undefined}
        onChange={(e) => {
          setTitle(e.target.value);
          if (create.error) create.reset();
        }}
        onKeyDown={(e) => {
          if (e.key !== "Enter" || isComposing(e)) return;
          e.preventDefault();
          create.mutate(
            {
              project_id: item.todo.project_id,
              parent_id: item.todo.id,
              template_id: null,
              title,
              priority: null,
              due_date: null,
              planned_start: null,
              planned_end: null,
              memo_md: null,
            },
            { onSuccess: () => setTitle("") },
          );
        }}
      />
    </section>
  );
}
