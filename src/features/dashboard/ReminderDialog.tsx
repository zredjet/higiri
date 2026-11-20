import { Badge, Button, Dialog } from "../../components/aizome";
import type { StartupReminder } from "../../lib/bindings";
import { formatDateWithWeekday } from "../../lib/date";
import { displayTitle, reminderSide, URGENCY_LABEL, URGENCY_TONE } from "../todos/todoText";

/**
 * 起動時リマインド (docs/ui-mock/Reminder.dc.html). It closes only with
 * 「確認しました」 (not the scrim, Esc or ×), which acknowledges it.
 */
export function ReminderDialog({ reminder, onClose }: { reminder: StartupReminder; onClose: () => void }) {
  const note =
    reminder.frequency === "every_launch"
      ? "日限注意があるあいだ、起動のたびに表示します。頻度は設定で変えられます。"
      : "日限注意があるあいだ、その日最初の起動のときに表示します。頻度は設定で変えられます。";

  return (
    <Dialog
      open
      size="lg"
      onClose={onClose}
      dismissible={false}
      title={`日限の近いTODOが${reminder.items.length}件あります`}
      description={`${formatDateWithWeekday(reminder.today)}の確認です。超過と本日のものから対応してください。待ちのものは相手に確認してください。`}
      footer={
        <Button variant="primary" onClick={onClose}>
          確認しました
        </Button>
      }
    >
      <div className="reminder-list">
        {reminder.items.map((item) => {
          const { todo, assessment, project_name } = item;
          const side = reminderSide(todo, assessment);
          const sub =
            assessment.urgency === "follow_up" && todo.waiting_on
              ? `${project_name} ・ ${todo.waiting_on}待ち`
              : project_name;
          return (
            <div key={todo.id} className="reminder-row">
              <div className="reminder-row__badge">
                {assessment.urgency ? (
                  <Badge tone={URGENCY_TONE[assessment.urgency]} dot>
                    {URGENCY_LABEL[assessment.urgency]}
                  </Badge>
                ) : null}
              </div>
              <div className="reminder-row__text">
                <span className="dash-strong">{displayTitle(item)}</span>
                <span className="text-caption">{sub}</span>
              </div>
              <div className="reminder-row__side">
                <span className="tabular">{side.label}</span>
                <span className={side.strong ? "reminder-row__detail is-strong" : "reminder-row__detail"} style={{ color: side.color }}>
                  {side.detail}
                </span>
              </div>
            </div>
          );
        })}
      </div>
      <span className="text-caption">{note}</span>
    </Dialog>
  );
}
