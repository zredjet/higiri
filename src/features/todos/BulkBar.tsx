import { Button } from "../../components/aizome";
import { BULK_KINDS, BULK_TITLE, CLEAR_SELECTION, selectingText, type BulkKind } from "./bulkText";

/** 一括変更 (T-14): shown above the TODO list while TODOs are selected. */
export function BulkBar({
  count,
  onChange,
  onClear,
}: {
  count: number;
  onChange: (kind: BulkKind) => void;
  onClear: () => void;
}) {
  return (
    <div className="bulk-bar" role="group" aria-label="選んだTODOをまとめて変える">
      <span className="bulk-bar__count" aria-live="polite">
        {selectingText(count)}
      </span>
      {BULK_KINDS.map((kind) => (
        <Button key={kind} size="sm" onClick={() => onChange(kind)}>
          {BULK_TITLE[kind]}
        </Button>
      ))}
      <Button size="sm" variant="ghost" onClick={onClear}>
        {CLEAR_SELECTION}
      </Button>
    </div>
  );
}
