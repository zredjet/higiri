import { useState } from "react";
import { Alert, Button, Dialog, Icon, RadioGroup, Select, TextField } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import {
  BULK_STATUSES,
  type BulkChange,
  type BulkPreview,
  type BulkResult,
  type BulkTodo,
  type Priority,
  type Status,
} from "../../lib/bindings";
import { errorMessage, isAppError } from "../../lib/errors";
import { useApplyBulkChange, usePreviewBulkChange } from "../../lib/queries";
import { blockingLine } from "./dependencyText";
import { saveOpenDrafts } from "./detail/useTodoDraft";
import {
  applyLabel,
  BLOCKED_TEXT,
  BULK_TITLE,
  bulkTodoTitle,
  closingChildrenText,
  daysOf,
  DUE_MODES,
  nextRoundsText,
  SHIFT_DAYS_NOTE,
  skippedText,
  targetsText,
  type BulkKind,
  type DueMode,
} from "./bulkText";
import { PRIORITIES, PRIORITY_LABEL, STATUS_LABEL } from "./todoText";

interface BulkDialogProps {
  kind: BulkKind;
  /** The selected TODOs, in the list's order. */
  ids: number[];
  /** The all-projects list: the lists name the project. */
  showProject: boolean;
  onClose: () => void;
  onDone: (change: BulkChange, result: BulkResult) => void;
}

/**
 * 一括変更 (T-14): the input on top, and under it what the change does to
 * the selected TODOs, asked again of Rust (preview_bulk_change) each time
 * the input changes: how many change, which are left out and why, and for a
 * status the TODOs waiting for unfinished ones, the children closed with
 * their parent and the next rounds. The input's errors are Rust's.
 */
export function BulkDialog({ kind, ids, showProject, onClose, onDone }: BulkDialogProps) {
  const [dueMode, setDueMode] = useState<DueMode>("set");
  const [date, setDate] = useState<string | null>(null);
  const [daysText, setDaysText] = useState("");
  const [priority, setPriority] = useState<Priority | "">("");
  const [status, setStatus] = useState<Status | "">("");
  const [reason, setReason] = useState("");
  const [reasonTouched, setReasonTouched] = useState(false);

  const days = daysOf(daysText);
  const change = changeOf();
  const preview = usePreviewBulkChange(ids, change);
  const apply = useApplyBulkChange();

  function changeOf(): BulkChange | null {
    switch (kind) {
      case "due":
        if (dueMode === "set") return date ? { kind: "set_due", date } : null;
        return days === null ? null : { kind: "shift_due", days };
      case "planned":
        return days === null ? null : { kind: "shift_planned", days };
      case "priority":
        return priority ? { kind: "priority", priority } : null;
      case "status":
        return status ? { kind: "status", status, reason: status === "cancelled" ? reason : null } : null;
    }
  }

  // The input's own error (Rust's text) goes under the input; anything else on top.
  const inputError = preview.error && isAppError(preview.error) && preview.error.kind === "validation";
  const fieldError =
    change && inputError && !(kind === "status" && !reasonTouched) ? errorMessage(preview.error) : undefined;
  const result: BulkPreview | undefined = change && !preview.error ? preview.data : undefined;
  const count = result?.targets.length ?? 0;
  const pending = apply.isPending;

  async function submit() {
    if (!change || count === 0) return;
    await saveOpenDrafts();
    try {
      const done = await apply.mutateAsync({ ids, change });
      onDone(change, done);
    } catch {
      // Shown in the dialog (apply.error).
    }
  }

  const daysField = (
    <TextField
      label="ずらす日数"
      inputMode="numeric"
      suffix="日"
      value={daysText}
      description={SHIFT_DAYS_NOTE}
      error={fieldError}
      onChange={(e) => setDaysText(e.target.value)}
    />
  );

  return (
    <Dialog
      open
      size="md"
      onClose={pending ? undefined : onClose}
      title={BULK_TITLE[kind]}
      footer={
        <>
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="bulk-form" loading={pending} disabled={!result || count === 0}>
            {applyLabel(count)}
          </Button>
        </>
      }
    >
      <form
        id="bulk-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        {apply.error ? <Alert tone="danger">{errorMessage(apply.error)}</Alert> : null}
        {preview.error && !inputError ? <Alert tone="danger">{errorMessage(preview.error)}</Alert> : null}
        {kind === "due" ? (
          <>
            <RadioGroup
              label="変え方"
              orientation="horizontal"
              options={DUE_MODES}
              value={dueMode}
              onChange={(v) => setDueMode(v as DueMode)}
            />
            {dueMode === "set" ? (
              <DateField label="日限" value={date} error={fieldError} onChange={setDate} />
            ) : (
              daysField
            )}
          </>
        ) : null}
        {kind === "planned" ? daysField : null}
        {kind === "priority" ? (
          <Select
            label="優先度"
            placeholder="選んでください"
            value={priority}
            options={PRIORITIES.map((p) => ({ value: p, label: PRIORITY_LABEL[p] }))}
            onChange={(e) => setPriority(e.target.value as Priority)}
          />
        ) : null}
        {kind === "status" ? (
          <>
            <Select
              label="ステータス"
              placeholder="選んでください"
              value={status}
              options={BULK_STATUSES.map((s) => ({ value: s, label: STATUS_LABEL[s] }))}
              onChange={(e) => setStatus(e.target.value as Status)}
            />
            {status === "cancelled" ? (
              <TextField
                label="中止の理由"
                value={reason}
                description="選んだTODOのすべてに、同じ理由を入れます。"
                error={fieldError}
                onChange={(e) => {
                  setReason(e.target.value);
                  setReasonTouched(true);
                }}
              />
            ) : null}
          </>
        ) : null}
        {result ? <Confirmation preview={result} to={status || null} showProject={showProject} /> : null}
      </form>
    </Dialog>
  );
}

/** The confirmation: what changes, what is left out and why, and what else happens. */
function Confirmation({
  preview,
  to,
  showProject,
}: {
  preview: BulkPreview;
  to: Status | null;
  showProject: boolean;
}) {
  const name = (t: BulkTodo) => (
    <span className="bulk-list__name">
      <span>{bulkTodoTitle(t)}</span>
      {showProject ? <span className="text-caption">{t.project_name}</span> : null}
    </span>
  );
  return (
    <section className="bulk-confirm" aria-label="変わる内容">
      <p className="bulk-confirm__lead" aria-live="polite">
        {targetsText(preview.targets.length)}
      </p>
      {to && preview.closing_children > 0 ? <p>{closingChildrenText(preview.closing_children, to)}</p> : null}
      {preview.next_rounds > 0 ? <p>{nextRoundsText(preview.next_rounds)}</p> : null}
      {preview.blocked.length > 0 ? (
        <div className="bulk-confirm__group">
          <p className="bulk-confirm__warning">
            <Icon name="alert-triangle" size={16} />
            {BLOCKED_TEXT}
          </p>
          <ul className="bulk-list">
            {preview.blocked.map((b) => (
              <li key={b.todo.id} className="bulk-list__item">
                {name(b.todo)}
                <span className="bulk-list__note">先行TODO:{b.blocked_by.map(blockingLine).join("、")}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {preview.skipped.length > 0 ? (
        <div className="bulk-confirm__group">
          <p>{skippedText(preview.skipped.length)}</p>
          <ul className="bulk-list">
            {preview.skipped.map((s) => (
              <li key={s.todo.id} className="bulk-list__item">
                {name(s.todo)}
                <span className="bulk-list__note">{s.reason}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
