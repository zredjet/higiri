import { useState } from "react";
import { Alert, Button, Dialog } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import type { TodoItem } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useUpdateTodo } from "../../lib/queries";
import { fieldsOf } from "../todos/detail/draft";
import { displayTitle } from "../todos/todoText";

export type DateTarget = "due_date" | "follow_up_date";

const LABEL: Record<DateTarget, string> = { due_date: "日限", follow_up_date: "確認日" };

/** 日限を変更 / 確認日を変更 from the dashboard (D-3). */
export function ChangeDateDialog({
  item,
  target,
  onClose,
}: {
  item: TodoItem;
  target: DateTarget;
  onClose: () => void;
}) {
  const [value, setValue] = useState<string | null>(item.todo[target]);
  const update = useUpdateTodo();
  const label = LABEL[target];

  return (
    <Dialog
      open
      size="sm"
      onClose={update.isPending ? undefined : onClose}
      title={`${label}を変更`}
      description={`「${displayTitle(item)}」の${label}を変えます。`}
      footer={
        <>
          <Button onClick={onClose} disabled={update.isPending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="change-date-form" loading={update.isPending}>
            変更する
          </Button>
        </>
      }
    >
      <form
        id="change-date-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          update.mutate(
            { id: item.todo.id, change: (current) => ({ ...fieldsOf(current), [target]: value }) },
            { onSuccess: onClose },
          );
        }}
      >
        {update.error ? <Alert tone="danger">{errorMessage(update.error)}</Alert> : null}
        <DateField label={label} required={target === "follow_up_date"} value={value} onChange={setValue} />
      </form>
    </Dialog>
  );
}
