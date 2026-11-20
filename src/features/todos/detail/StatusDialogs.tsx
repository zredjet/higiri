import { useState } from "react";
import { Alert, Button, Dialog, TextField } from "../../../components/aizome";
import { DateField } from "../../../components/DateField";
import { errorMessage } from "../../../lib/errors";
import { closeChildrenQuestion } from "../todoText";

interface BaseProps {
  onClose: () => void;
  pending: boolean;
  error: unknown;
}

/** 待ちにする: who we wait for and when to check (T-8). */
export function WaitDialog({
  defaultFollowUp,
  onSubmit,
  onClose,
  pending,
  error,
}: BaseProps & { defaultFollowUp: string; onSubmit: (waitingOn: string, followUp: string | null) => void }) {
  const [waitingOn, setWaitingOn] = useState("");
  const [followUp, setFollowUp] = useState<string | null>(defaultFollowUp);
  return (
    <Dialog
      open
      size="sm"
      onClose={pending ? undefined : onClose}
      title="待ちにする"
      description="相手の作業を待つあいだ「待ち」にします。確認日が来ると「要確認」として知らせます。"
      footer={
        <>
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="wait-form" loading={pending}>
            待ちにする
          </Button>
        </>
      }
    >
      <form
        id="wait-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit(waitingOn, followUp);
        }}
      >
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        <TextField
          label="待っている相手"
          required
          autoFocus
          placeholder="例: 認証担当の回答"
          value={waitingOn}
          onChange={(e) => setWaitingOn(e.target.value)}
        />
        <DateField label="確認日" required value={followUp} onChange={setFollowUp} />
      </form>
    </Dialog>
  );
}

/**
 * 中止にする: keep the TODO and record why (T-9). `childrenNote` tells that
 * the open children are cancelled too (S-1).
 */
export function CancelDialog({
  onSubmit,
  onClose,
  pending,
  error,
  childrenNote,
}: BaseProps & { onSubmit: (reason: string) => void; childrenNote?: string }) {
  const [reason, setReason] = useState("");
  return (
    <Dialog
      open
      size="sm"
      onClose={pending ? undefined : onClose}
      title="中止にする"
      description={`削除せずに記録を残します。中止したTODOは、一覧の「中止」タブから見られます。${childrenNote ?? ""}`}
      footer={
        <>
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="cancel-form" loading={pending}>
            中止にする
          </Button>
        </>
      }
    >
      <form
        id="cancel-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          onSubmit(reason);
        }}
      >
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        <TextField
          label="中止の理由"
          required
          autoFocus
          multiline
          rows={2}
          value={reason}
          onChange={(e) => setReason(e.target.value)}
        />
      </form>
    </Dialog>
  );
}

/**
 * 完了にする for a parent with open children (S-1): they are completed
 * together. The detail panel and the dashboard ask with this.
 */
export function DoneWithChildrenDialog({
  openChildren,
  onConfirm,
  onClose,
  pending,
  error,
}: BaseProps & { openChildren: number; onConfirm: () => void }) {
  return (
    <Dialog
      open
      size="sm"
      onClose={pending ? undefined : onClose}
      title="完了にする"
      description={closeChildrenQuestion(openChildren)}
      footer={
        <>
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="primary" loading={pending} onClick={onConfirm}>
            完了にする
          </Button>
        </>
      }
    >
      {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
    </Dialog>
  );
}
