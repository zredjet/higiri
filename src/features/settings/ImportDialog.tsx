import { Alert, Button, Dialog } from "../../components/aizome";
import { errorMessage } from "../../lib/errors";
import { useImportData } from "../../lib/queries";

interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  /** The data was replaced with the file `fileName`. */
  onImported: (fileName: string) => void;
}

/**
 * 「JSONから取り込む」: asks first, then the file dialog opens on the Rust side. Closing the
 * file dialog without choosing closes this one too and changes nothing.
 */
export function ImportDialog(props: ImportDialogProps) {
  if (!props.open) return null;
  return <ImportForm {...props} />;
}

function ImportForm({ onClose, onImported }: ImportDialogProps) {
  const importData = useImportData();

  return (
    <Dialog
      open
      onClose={importData.isPending ? undefined : onClose}
      title="JSONから取り込みますか"
      description="今のデータはすべて置き換わります。取り込む前の状態は残り、復元の一覧から戻せます。"
      footer={
        <>
          <Button onClick={onClose} disabled={importData.isPending}>
            キャンセル
          </Button>
          <Button
            variant="primary"
            loading={importData.isPending}
            onClick={() =>
              importData.mutate(undefined, {
                onSuccess: (fileName) => (fileName === null ? onClose() : onImported(fileName)),
              })
            }
          >
            ファイルを選ぶ
          </Button>
        </>
      }
    >
      {importData.error ? (
        <div className="dialog-form">
          <Alert tone="danger">{errorMessage(importData.error)}</Alert>
        </div>
      ) : null}
    </Dialog>
  );
}
