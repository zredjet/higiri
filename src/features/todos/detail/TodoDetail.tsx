import { useState } from "react";
import { Alert, Select, Tabs, TextField } from "../../../components/aizome";
import { DateField } from "../../../components/DateField";
import type { Priority, Status, TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { shortcutText } from "../../../lib/keys";
import { useChangeStatus, useTimeline } from "../../../lib/queries";
import { PRIORITIES, PRIORITY_LABEL, STATUSES, STATUS_LABEL, waitingMessage } from "../todoText";
import { MarkdownField } from "./MarkdownField";
import { CancelDialog, WaitDialog } from "./StatusDialogs";
import { useTodoDraft } from "./useTodoDraft";
import { WorkLogSection } from "./WorkLogSection";

/** The fields, memo and work log of one TODO (docs/ui-mock/Todos.dc.html, right side). */
export function TodoDetail({ item }: { item: TodoItem }) {
  const { todo, assessment } = item;
  const { draft, set, save, error, saving } = useTodoDraft(item);
  const status = useChangeStatus();
  const timeline = useTimeline(todo.id);
  const [asking, setAsking] = useState<null | "waiting" | "cancelled">(null);
  const [tab, setTab] = useState<"log" | "memo">("log");
  const [memoPreview, setMemoPreview] = useState(false);

  async function changeStatus(to: Status) {
    if (to === "waiting" || to === "cancelled") {
      status.reset();
      setAsking(to);
      return;
    }
    await save();
    status.mutate({ id: todo.id, change: { to, waiting_on: null, follow_up_date: null, reason: null } });
  }

  const waiting = waitingMessage(todo, assessment);
  const logCount = (timeline.data ?? []).filter((e) => e.type === "work_log").length;

  return (
    <>
      <input
        className="detail-title"
        aria-label="タイトル"
        value={draft.title}
        onChange={(e) => set("title", e.target.value)}
      />
      {waiting ? <Alert tone="info">{waiting}</Alert> : null}
      {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
      {status.error && asking === null ? <Alert tone="danger">{errorMessage(status.error)}</Alert> : null}

      <div className="detail-fields">
        <div className="detail-fields__row detail-fields__row--main">
          <Select
            label="ステータス"
            size="sm"
            value={todo.status}
            disabled={status.isPending}
            options={STATUSES.map((s) => ({
              value: s,
              label: STATUS_LABEL[s],
              disabled: s !== todo.status && !item.next_statuses.includes(s),
            }))}
            onChange={(e) => void changeStatus(e.target.value as Status)}
          />
          <Select
            label="優先度"
            size="sm"
            value={draft.priority}
            options={PRIORITIES.map((p) => ({ value: p, label: PRIORITY_LABEL[p] }))}
            onChange={(e) => set("priority", e.target.value as Priority)}
          />
          <DateField
            label="日限"
            size="sm"
            value={draft.due_date}
            onChange={(v) => set("due_date", v)}
          />
        </div>
        {todo.status === "waiting" ? (
          <div className="detail-fields__row">
            <TextField
              label="待っている相手"
              size="sm"
              value={draft.waiting_on ?? ""}
              onChange={(e) => set("waiting_on", e.target.value)}
            />
            <DateField
              label="確認日"
              size="sm"
              value={draft.follow_up_date}
              onChange={(v) => set("follow_up_date", v)}
            />
          </div>
        ) : null}
        {todo.status === "cancelled" ? (
          <TextField
            label="中止の理由"
            size="sm"
            value={draft.close_reason ?? ""}
            onChange={(e) => set("close_reason", e.target.value)}
          />
        ) : null}
        <div className="detail-fields__row">
          <DateField
            label="予定開始"
            size="sm"
            value={draft.planned_start}
            onChange={(v) => set("planned_start", v)}
          />
          <DateField
            label="予定終了"
            size="sm"
            value={draft.planned_end}
            onChange={(v) => set("planned_end", v)}
          />
        </div>
        {assessment.warnings.plan_misses_due ? (
          <Alert tone="warning">予定終了日が日限より後になっています。予定を見直すか、日限を変えてください。</Alert>
        ) : null}
        <div className="detail-fields__row">
          <DateField
            label="実績開始"
            size="sm"
            value={draft.actual_start}
            onChange={(v) => set("actual_start", v)}
          />
          <DateField
            label="実績終了"
            size="sm"
            value={draft.actual_end}
            onChange={(v) => set("actual_end", v)}
          />
        </div>
        <span className="text-caption" aria-live="polite">
          {saving ? "保存しています。" : "入力は自動で保存されます。"}
        </span>
      </div>

      <Tabs
        label="記録の種類"
        value={tab}
        onChange={(id) => setTab(id as "log" | "memo")}
        items={[
          {
            id: "log",
            label: "作業ログ",
            count: logCount,
            content: <WorkLogSection todoId={todo.id} timeline={timeline.data ?? []} />,
          },
          {
            id: "memo",
            label: "メモ",
            content: (
              <div className="detail-section">
                <MarkdownField
                  label="メモ"
                  value={draft.memo_md}
                  onChange={(v) => set("memo_md", v)}
                  preview={memoPreview}
                  onTogglePreview={() => setMemoPreview((p) => !p)}
                  description={`今の作業状況や概要を書きます。Markdownで書けます。${shortcutText("E")} でプレビューに切り替わります。入力が止まってから1秒後に保存します。`}
                  emptyPreview="メモはまだありません。「編集に戻る」から書けます。"
                  minLines={8}
                />
              </div>
            ),
          },
        ]}
      />

      {asking === "waiting" ? (
        <WaitDialog
          defaultFollowUp={item.follow_up_default}
          pending={status.isPending}
          error={status.error}
          onClose={() => setAsking(null)}
          onSubmit={async (waitingOn, followUp) => {
            await save();
            status.mutate(
              { id: todo.id, change: { to: "waiting", waiting_on: waitingOn, follow_up_date: followUp, reason: null } },
              { onSuccess: () => setAsking(null) },
            );
          }}
        />
      ) : null}
      {asking === "cancelled" ? (
        <CancelDialog
          pending={status.isPending}
          error={status.error}
          onClose={() => setAsking(null)}
          onSubmit={async (reason) => {
            await save();
            status.mutate(
              { id: todo.id, change: { to: "cancelled", waiting_on: null, follow_up_date: null, reason } },
              { onSuccess: () => setAsking(null) },
            );
          }}
        />
      ) : null}
    </>
  );
}
