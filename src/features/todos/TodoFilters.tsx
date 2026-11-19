import { Select } from "../../components/aizome";
import type { DueFilter, Priority, TodoSort } from "../../lib/bindings";
import { PRIORITIES, PRIORITY_LABEL } from "./todoText";

export interface FilterState {
  due: DueFilter;
  priority: Priority | "all";
  sort: TodoSort;
}

export const DEFAULT_FILTERS: FilterState = { due: "all", priority: "all", sort: "manual" };

const DUE_OPTIONS: Array<{ value: DueFilter; label: string }> = [
  { value: "all", label: "すべて" },
  { value: "attention", label: "日限注意だけ" },
  { value: "until_today", label: "今日まで" },
  { value: "within_week", label: "7日後まで" },
  { value: "no_due", label: "日限なし" },
];

const SORT_OPTIONS: Array<{ value: TodoSort; label: string }> = [
  { value: "manual", label: "登録順" },
  { value: "due_date", label: "日限の近い順" },
  { value: "priority", label: "優先度の高い順" },
];

/** 日限・優先度で絞り込み、並べ替える (T-4). The work is done in Rust. */
export function TodoFilters({ value, onChange }: { value: FilterState; onChange: (v: FilterState) => void }) {
  return (
    <div className="todo-filters">
      <Select
        size="sm"
        label="日限"
        options={DUE_OPTIONS}
        value={value.due}
        onChange={(e) => onChange({ ...value, due: e.target.value as DueFilter })}
      />
      <Select
        size="sm"
        label="優先度"
        options={[
          { value: "all", label: "すべて" },
          ...PRIORITIES.map((p) => ({ value: p, label: PRIORITY_LABEL[p] })),
        ]}
        value={value.priority}
        onChange={(e) => onChange({ ...value, priority: e.target.value as Priority | "all" })}
      />
      <Select
        size="sm"
        label="並べ替え"
        options={SORT_OPTIONS}
        value={value.sort}
        onChange={(e) => onChange({ ...value, sort: e.target.value as TodoSort })}
      />
    </div>
  );
}
