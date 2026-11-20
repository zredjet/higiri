import { useState } from "react";
import { Alert, Button, Icon } from "../../../components/aizome";
import type { RepeatInput, TodoItem } from "../../../lib/bindings";
import { todayYmd } from "../../../lib/date";
import { errorMessage } from "../../../lib/errors";
import { useSetTodoRepeat } from "../../../lib/queries";
import { nextDueText, repeatRuleText, untilText } from "../repeatText";
import { RepeatDialog } from "./RepeatDialog";

const isClosed = (i: TodoItem) => i.todo.status === "done" || i.todo.status === "cancelled";

/**
 * 「繰り返し」 of a TODO without a parent (T-12): its 周期 and the next round's
 * 日限, and a button that opens the 周期 dialog. A repetition needs a 日限
 * (`dueDraft`, what is typed in the panel); the panel's edits are saved
 * before the dialog opens, so Rust checks the 日限 as shown.
 */
export function RepeatField({
  item,
  dueDraft,
  save,
}: {
  item: TodoItem;
  dueDraft: string | null;
  save: () => Promise<void>;
}) {
  const setRepeat = useSetTodoRepeat();
  const [open, setOpen] = useState(false);
  const repeat = item.repeat;
  // A closed TODO gets no repetition; one it still has (an imported file) can be taken off.
  if (!repeat && isClosed(item)) return null;
  const until = repeat ? untilText(repeat) : null;

  async function openDialog() {
    setRepeat.reset();
    await save();
    setOpen(true);
  }

  function send(next: RepeatInput | null) {
    setRepeat.mutate({ id: item.todo.id, repeat: next }, { onSuccess: () => setOpen(false) });
  }

  return (
    <div className="family">
      <div className="repeat-field">
        <span className="family__label">繰り返し</span>
        {repeat ? (
          <span className="repeat-field__rule">
            <Icon name="repeat" size={16} />
            {repeatRuleText(repeat.rule)}
          </span>
        ) : (
          <span className="repeat-field__rule is-empty">なし</span>
        )}
        <Button
          size="sm"
          variant="ghost"
          disabled={(repeat === null && dueDraft === null) || setRepeat.isPending}
          onClick={() => void openDialog()}
        >
          {repeat ? "繰り返しを変える" : "繰り返しを付ける"}
        </Button>
      </div>
      <span className="text-caption">
        {repeat
          ? `${nextDueText(repeat)}${until ? ` ・ ${until}` : ""}`
          : dueDraft === null
            ? "日限を入れると、繰り返しを付けられます。"
            : "付けると、完了か中止にしたときに次の回のTODOを作ります。"}
      </span>
      {setRepeat.error && !open ? <Alert tone="danger">{errorMessage(setRepeat.error)}</Alert> : null}
      {open ? (
        <RepeatDialog
          due={dueDraft ?? todayYmd()}
          current={repeat}
          pending={setRepeat.isPending}
          error={setRepeat.error}
          onSubmit={(next) => send(next)}
          onRemove={() => send(null)}
          onClose={() => {
            setRepeat.reset();
            setOpen(false);
          }}
        />
      ) : null}
    </div>
  );
}
