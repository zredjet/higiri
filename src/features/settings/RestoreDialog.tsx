import { useState } from "react";
import { Alert, Button, Dialog, RadioGroup } from "../../components/aizome";
import type { Backup } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useBackups, useRestoreBackup } from "../../lib/queries";
import { backupDetail, backupLabel } from "./backupText";

interface RestoreDialogProps {
  open: boolean;
  onClose: () => void;
  onRestored: (backup: Backup) => void;
}

/** 「バックアップから復元」: pick a backup; the current data is kept before restoring. */
export function RestoreDialog(props: RestoreDialogProps) {
  if (!props.open) return null;
  return <RestoreForm {...props} />;
}

function RestoreForm({ onClose, onRestored }: RestoreDialogProps) {
  const backups = useBackups();
  const restore = useRestoreBackup();
  const [selected, setSelected] = useState<string | null>(null);
  const list = backups.data ?? [];
  const chosen = list.find((b) => b.file_name === selected) ?? list[0];
  const error = backups.error ?? restore.error;

  return (
    <Dialog
      open
      onClose={restore.isPending ? undefined : onClose}
      title="バックアップから復元"
      description="選んだ時点のデータに戻します。今のデータは、復元の前に「復元する前の状態」として残します。"
      footer={
        <>
          <Button onClick={onClose} disabled={restore.isPending}>
            キャンセル
          </Button>
          <Button
            variant="primary"
            disabled={!chosen || restore.isPending}
            onClick={() => {
              if (chosen) restore.mutate(chosen.file_name, { onSuccess: () => onRestored(chosen) });
            }}
          >
            復元する
          </Button>
        </>
      }
    >
      <div className="dialog-form">
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        {backups.isSuccess && list.length === 0 ? (
          <p className="text-muted">バックアップはまだありません。その日最初の起動時に作られます。</p>
        ) : null}
        {chosen ? (
          <RadioGroup
            label="戻す時点"
            name="backup"
            options={list.map((b) => ({ value: b.file_name, label: backupLabel(b), description: backupDetail(b) }))}
            value={chosen.file_name}
            onChange={setSelected}
            disabled={restore.isPending}
          />
        ) : null}
      </div>
    </Dialog>
  );
}
