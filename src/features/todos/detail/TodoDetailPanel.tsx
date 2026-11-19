import { useState } from "react";
import { Alert } from "../../../components/aizome";
import { MenuButton } from "../../../components/Menu";
import { DetailPanel } from "../../../app/DetailPanel";
import { errorMessage } from "../../../lib/errors";
import { useDeleteTodo, useTodo } from "../../../lib/queries";
import { ConfirmDelete } from "./ConfirmDelete";
import { TodoDetail } from "./TodoDetail";

/** The right-hand panel for one TODO: loads it, offers deletion, shows the detail. */
export function TodoDetailPanel({ todoId, onClose }: { todoId: number; onClose: () => void }) {
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
      {item ? <TodoDetail key={item.todo.id} item={item} /> : null}
      {deleting && item ? (
        <ConfirmDelete
          title="TODOを削除しますか"
          description={`「${item.todo.title}」と、その作業ログ・変更履歴をすべて削除します。削除したものは元に戻せません。不要になっただけなら、ステータスを「中止」にすると記録が残ります。`}
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
