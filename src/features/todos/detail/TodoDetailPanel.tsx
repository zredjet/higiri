import { useState } from "react";
import { Alert } from "../../../components/aizome";
import { MenuButton } from "../../../components/Menu";
import { DetailPanel } from "../../../app/DetailPanel";
import type { TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { useDeleteTodo, useTodo } from "../../../lib/queries";
import { deleteTodoText } from "../todoText";
import { ConfirmDelete } from "./ConfirmDelete";
import { useFamily } from "./FamilySection";
import { TodoDetail } from "./TodoDetail";

/**
 * The right-hand panel for one TODO: loads it, offers deletion, shows the
 * detail. `onOpenTodo` switches the panel to another TODO (its parent or a
 * child).
 */
export function TodoDetailPanel({
  todoId,
  onClose,
  onOpenTodo,
}: {
  todoId: number;
  onClose: () => void;
  onOpenTodo: (id: number) => void;
}) {
  const query = useTodo(todoId);
  const del = useDeleteTodo();
  const [deleting, setDeleting] = useState(false);
  const item = query.data;

  return (
    <DetailPanel
      context={item?.project_name ?? ""}
      onClose={onClose}
      actions={
        item ? (
          <MenuButton
            label="その他の操作"
            items={[{ label: "削除する", danger: true, onSelect: () => setDeleting(true) }]}
          />
        ) : null
      }
    >
      {query.error ? <Alert tone="danger">{errorMessage(query.error)}</Alert> : null}
      {item ? <TodoDetail key={item.todo.id} item={item} onOpenTodo={onOpenTodo} /> : null}
      {deleting && item ? (
        <DeleteTodo
          item={item}
          pending={del.isPending}
          error={del.error}
          onClose={() => {
            del.reset();
            setDeleting(false);
          }}
          onConfirm={() =>
            del.mutate(item.todo.id, {
              onSuccess: () => {
                setDeleting(false);
                onClose();
              },
            })
          }
        />
      ) : null}
    </DetailPanel>
  );
}

/** The delete confirmation: the children go with their parent (S-1). */
function DeleteTodo({
  item,
  ...props
}: {
  item: TodoItem;
  pending: boolean;
  error: unknown;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const family = useFamily(item);
  // All children, 中止 included (the counts leave 中止 out).
  const children = family.loaded ? family.children.length : (item.children?.total ?? 0);
  return <ConfirmDelete title="TODOを削除しますか" description={deleteTodoText(item.todo.title, children)} {...props} />;
}
