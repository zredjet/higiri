import { Alert, Button, Dialog } from "../../../components/aizome";
import { errorMessage } from "../../../lib/errors";

interface ConfirmDeleteProps {
  title: string;
  description: string;
  pending: boolean;
  error: unknown;
  onConfirm: () => void;
  onClose: () => void;
}

/** Destructive confirmation with a concrete verb. */
export function ConfirmDelete({ title, description, pending, error, onConfirm, onClose }: ConfirmDeleteProps) {
  return (
    <Dialog
      open
      tone="danger"
      size="sm"
      onClose={pending ? undefined : onClose}
      title={title}
      description={description}
      footer={
        <>
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="danger" loading={pending} onClick={onConfirm}>
            削除する
          </Button>
        </>
      }
    >
      {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
    </Dialog>
  );
}
