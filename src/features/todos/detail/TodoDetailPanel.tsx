import { useState } from "react";
import { Alert } from "../../../components/aizome";
import { MenuButton } from "../../../components/Menu";
import { DetailPanel } from "../../../app/DetailPanel";
import type { TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { useDeleteTodo, useTodo } from "../../../lib/queries";
import { templateSavedText } from "../templateText";
import { deleteTodoText } from "../todoText";
import { ConfirmDelete } from "./ConfirmDelete";
import { useFamily } from "./FamilySection";
import { SaveTemplateDialog } from "./SaveTemplateDialog";
import { TodoDetail } from "./TodoDetail";
import { saveOpenDrafts } from "./useTodoDraft";

/**
 * The right-hand panel for one TODO: loads it, offers 「テンプレートとして保存」
 * (a TODO without a parent, T-10) and deletion, shows the detail.
 * `onOpenTodo` switches the panel to another TODO (its parent or a child).
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
  /** The TODO whose 「テンプレートとして保存」 dialog is open. */
  const [savingTemplate, setSavingTemplate] = useState<number | null>(null);
  /** The saved template's notice, for the TODO it was saved from. */
  const [saved, setSaved] = useState<{ todoId: number; text: string } | null>(null);
  const item = query.data;

  return (
    <DetailPanel
      context={item?.project_name ?? ""}
      onClose={onClose}
      actions={
        item ? (
          <MenuButton
            label="その他の操作"
            items={[
              ...(item.todo.parent_id === null
                ? [
                    {
                      label: "テンプレートとして保存",
                      onSelect: () => {
                        const id = item.todo.id;
                        setSaved(null);
                        // The name starts as the title just typed, once it is saved.
                        void saveOpenDrafts().then(() => setSavingTemplate(id));
                      },
                    },
                  ]
                : []),
              { label: "削除する", danger: true, onSelect: () => setDeleting(true) },
            ]}
          />
        ) : null
      }
    >
      {query.error ? <Alert tone="danger">{errorMessage(query.error)}</Alert> : null}
      {saved && saved.todoId === todoId ? (
        <Alert tone="success" onClose={() => setSaved(null)}>
          {saved.text}
        </Alert>
      ) : null}
      {item ? <TodoDetail key={item.todo.id} item={item} onOpenTodo={onOpenTodo} /> : null}
      {item && savingTemplate === item.todo.id && item.todo.id === todoId ? (
        <SaveTemplateDialog
          item={item}
          onClose={() => setSavingTemplate(null)}
          onSaved={(template) => {
            setSavingTemplate(null);
            setSaved({ todoId: item.todo.id, text: templateSavedText(template.name) });
          }}
        />
      ) : null}
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
