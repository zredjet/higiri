import { useState } from "react";
import { Alert, Button, Dialog, TextField } from "../../../components/aizome";
import type { TemplateSummary, TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { useSaveTodoTemplate, useTodoTemplates } from "../../../lib/queries";
import { replaceTemplateQuestion, SAVE_TEMPLATE_DESCRIPTION, templateNamed } from "../templateText";
import { saveOpenDrafts } from "./useTodoDraft";

interface SaveTemplateDialogProps {
  /** A TODO without a parent (T-10). */
  item: TodoItem;
  onClose: () => void;
  onSaved: (template: TemplateSummary) => void;
}

/**
 * 「テンプレートとして保存」: the name (the TODO's title at first). When a
 * template has that name already, asks before replacing it.
 */
export function SaveTemplateDialog({ item, onClose, onSaved }: SaveTemplateDialogProps) {
  const [name, setName] = useState(item.todo.title);
  const [confirming, setConfirming] = useState(false);
  /** Saving the panel's waiting edits first. */
  const [flushing, setFlushing] = useState(false);
  const templates = useTodoTemplates();
  const save = useSaveTodoTemplate();
  const busy = save.isPending || flushing;

  async function submit(replace: boolean) {
    save.reset();
    if (!replace && templateNamed(templates.data ?? [], name)) {
      setConfirming(true);
      return;
    }
    // What was typed in the panel just now goes into the template too.
    setFlushing(true);
    await saveOpenDrafts();
    setFlushing(false);
    save.mutate({ todoId: item.todo.id, name, replace }, { onSuccess: onSaved });
  }

  if (confirming) {
    return (
      <Dialog
        open
        size="sm"
        onClose={busy ? undefined : onClose}
        title="テンプレートを置き換えますか"
        description={replaceTemplateQuestion(name)}
        footer={
          <>
            <Button
              disabled={busy}
              onClick={() => {
                save.reset();
                setConfirming(false);
              }}
            >
              名前を変える
            </Button>
            <Button variant="primary" loading={busy} onClick={() => void submit(true)}>
              置き換える
            </Button>
          </>
        }
      >
        {save.error ? <Alert tone="danger">{errorMessage(save.error)}</Alert> : null}
      </Dialog>
    );
  }

  return (
    <Dialog
      open
      size="sm"
      onClose={busy ? undefined : onClose}
      title="テンプレートとして保存"
      description={SAVE_TEMPLATE_DESCRIPTION}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="save-template-form" loading={busy}>
            保存する
          </Button>
        </>
      }
    >
      <form
        id="save-template-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          void submit(false);
        }}
      >
        {save.error ? <Alert tone="danger">{errorMessage(save.error)}</Alert> : null}
        <TextField
          label="テンプレートの名前"
          required
          autoFocus
          description="「TODOを追加」で、この名前で選びます。40字まで入力できます。"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </form>
    </Dialog>
  );
}
