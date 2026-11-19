import { useState } from "react";
import { Alert, Button, Icon, TextField } from "../../../components/aizome";
import { DateField } from "../../../components/DateField";
import { MarkdownView } from "../../../components/markdown/MarkdownView";
import type { TimelineEntry, WorkLog } from "../../../lib/bindings";
import { formatDate, formatDateWithWeekday, monthDay, todayYmd } from "../../../lib/date";
import { errorMessage } from "../../../lib/errors";
import { shortcutText } from "../../../lib/keys";
import { useDeleteWorkLog, useSaveWorkLog } from "../../../lib/queries";
import { ConfirmDelete } from "./ConfirmDelete";
import { MarkdownField } from "./MarkdownField";

interface FormState {
  id: number | null;
  date: string;
  body: string;
  minutes: string;
}

const emptyForm = (): FormState => ({ id: null, date: todayYmd(), body: "", minutes: "" });

/** 作業ログ: write a log (M-2) and read the timeline of logs and changes (M-5). */
export function WorkLogSection({ todoId, timeline }: { todoId: number; timeline: TimelineEntry[] }) {
  const [form, setForm] = useState<FormState>(emptyForm);
  const [preview, setPreview] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<WorkLog | null>(null);
  const save = useSaveWorkLog();
  const del = useDeleteWorkLog();

  function submit() {
    const minutes = form.minutes.trim();
    if (minutes !== "" && !/^\d+$/.test(minutes)) {
      setLocalError("作業時間は分を数字で入力してください。");
      return;
    }
    setLocalError(null);
    save.mutate(
      {
        id: form.id,
        todo_id: todoId,
        log_date: form.date || todayYmd(),
        body_md: form.body,
        minutes: minutes === "" ? null : Number(minutes),
      },
      {
        onSuccess: () => {
          setForm(emptyForm());
          setPreview(false);
        },
      },
    );
  }

  function edit(log: WorkLog) {
    save.reset();
    setLocalError(null);
    setPreview(false);
    setForm({ id: log.id, date: log.log_date, body: log.body_md, minutes: log.minutes?.toString() ?? "" });
  }

  const error = localError ?? (save.error ? errorMessage(save.error) : null);

  return (
    <div className="detail-section">
      <MarkdownField
        label={`${monthDay(form.date || todayYmd())} の作業ログ${form.id !== null ? "(編集中)" : ""}`}
        value={form.body}
        onChange={(body) => setForm((f) => ({ ...f, body }))}
        preview={preview}
        onTogglePreview={() => setPreview((p) => !p)}
        onSubmit={submit}
        description={`Markdownで書けます。${shortcutText("E")} でプレビューに切り替わります。${shortcutText("Enter")} で保存します。`}
        emptyPreview="まだ何も書いていません。"
      />
      {error ? <Alert tone="danger">{error}</Alert> : null}
      <div className="worklog-form__row">
        <DateField
          label="作業日"
          size="sm"
          value={form.date || null}
          onChange={(v) => setForm((f) => ({ ...f, date: v ?? "" }))}
        />
        <TextField
          label="作業時間"
          size="sm"
          inputMode="numeric"
          suffix="分"
          value={form.minutes}
          onChange={(e) => setForm((f) => ({ ...f, minutes: e.target.value }))}
        />
        <div className="worklog-form__actions">
          {form.id !== null ? (
            <Button size="sm" variant="ghost" onClick={() => setForm(emptyForm())}>
              キャンセル
            </Button>
          ) : null}
          <Button size="sm" loading={save.isPending} onClick={submit}>
            ログを保存
          </Button>
        </div>
      </div>

      <Timeline entries={timeline} onEdit={edit} onDelete={setDeleting} />

      {deleting ? (
        <ConfirmDelete
          title="作業ログを削除しますか"
          description={`${formatDateWithWeekday(deleting.log_date)} の作業ログを削除します。削除したものは元に戻せません。`}
          pending={del.isPending}
          error={del.error}
          onClose={() => {
            del.reset();
            setDeleting(null);
          }}
          onConfirm={() =>
            del.mutate(deleting.id, {
              onSuccess: () => {
                if (form.id === deleting.id) setForm(emptyForm());
                setDeleting(null);
              },
            })
          }
        />
      ) : null}
    </div>
  );
}

function Timeline({
  entries,
  onEdit,
  onDelete,
}: {
  entries: TimelineEntry[];
  onEdit: (log: WorkLog) => void;
  onDelete: (log: WorkLog) => void;
}) {
  if (entries.length === 0) return null;
  return (
    <div className="timeline" aria-label="作業ログと変更履歴">
      {entries.map((e) =>
        e.type === "event" ? (
          <div key={`e${e.entry.event.id}`} className="timeline__event">
            <Icon name="arrow-right" size={14} />
            <span>
              {formatDate(e.entry.event.event_date)} {e.entry.text}
            </span>
          </div>
        ) : (
          <article key={`l${e.entry.id}`} className="timeline__log">
            <div className="timeline__log-head">
              <span className="text-caption">
                {formatDateWithWeekday(e.entry.log_date)}
                {e.entry.minutes !== null ? ` ・ ${e.entry.minutes}分` : ""}
              </span>
              <span className="timeline__log-actions">
                <Button size="sm" variant="ghost" onClick={() => onEdit(e.entry)}>
                  編集
                </Button>
                <Button size="sm" variant="ghost" onClick={() => onDelete(e.entry)}>
                  削除する
                </Button>
              </span>
            </div>
            <MarkdownView source={e.entry.body_md} />
          </article>
        ),
      )}
    </div>
  );
}
