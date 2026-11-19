import { Alert, Button, Dialog } from "../../components/aizome";
import type { ProjectSummary } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useDeleteProject } from "../../lib/queries";

interface DeleteProjectDialogProps {
  /** null: closed. */
  target: ProjectSummary | null;
  onClose: () => void;
  onDeleted: (projectId: number) => void;
}

/** Confirms deleting a project and everything in it. */
export function DeleteProjectDialog({ target, onClose, onDeleted }: DeleteProjectDialogProps) {
  const del = useDeleteProject();
  if (!target) return null;
  const { project, todo_count } = target;

  function close() {
    del.reset();
    onClose();
  }

  return (
    <Dialog
      open
      tone="danger"
      size="sm"
      onClose={del.isPending ? undefined : close}
      title="プロジェクトを削除しますか"
      description={`「${project.name}」と、その中のTODO ${todo_count}件、作業ログ、変更履歴をすべて削除します。削除したものは元に戻せません。残しておきたい場合は、アーカイブしてください。`}
      footer={
        <>
          <Button onClick={close} disabled={del.isPending}>
            キャンセル
          </Button>
          <Button
            variant="danger"
            loading={del.isPending}
            onClick={() =>
              del.mutate(project.id, {
                onSuccess: () => {
                  del.reset();
                  onDeleted(project.id);
                },
              })
            }
          >
            削除する
          </Button>
        </>
      }
    >
      {del.error ? <Alert tone="danger">{errorMessage(del.error)}</Alert> : null}
    </Dialog>
  );
}
