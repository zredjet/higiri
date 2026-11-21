import { Button, Dialog } from "../../components/aizome";
import type { DependentShift } from "../../lib/bindings";
import {
  SHIFT_DIALOG_TITLE,
  SHIFT_ONLY_THIS,
  SHIFT_WITH_DEPENDENTS,
  shiftDatesText,
  shiftQuestion,
  shiftTitle,
} from "./shiftText";

/**
 * Asked when a dropped bar would make 後続TODO start before it ends (G-8):
 * the 後続TODO in the dependencies' order, each with its plan now and moved.
 * 「キャンセル」, Esc and × undo the drag.
 */
export function ShiftDialog({
  shifts,
  projectId,
  onCancel,
  onOnlyThis,
  onWithDependents,
}: {
  shifts: DependentShift[];
  /** The dragged TODO's project: other projects are named. */
  projectId: number | null;
  onCancel: () => void;
  onOnlyThis: () => void;
  onWithDependents: () => void;
}) {
  return (
    <Dialog
      open
      size="md"
      onClose={onCancel}
      title={SHIFT_DIALOG_TITLE}
      description={shiftQuestion(shifts.length)}
      footer={
        <>
          <Button onClick={onCancel}>キャンセル</Button>
          <Button onClick={onOnlyThis}>{SHIFT_ONLY_THIS}</Button>
          <Button variant="primary" onClick={onWithDependents}>
            {SHIFT_WITH_DEPENDENTS}
          </Button>
        </>
      }
    >
      <ul className="shift-list">
        {shifts.map((s) => (
          <li key={s.id} className="shift-list__item">
            <span className="shift-list__name">
              <span>{shiftTitle(s)}</span>
              {s.project_id !== projectId ? <span className="text-caption">{s.project_name}</span> : null}
            </span>
            <span className="shift-list__dates">{shiftDatesText(s)}</span>
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
