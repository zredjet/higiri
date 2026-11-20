import { useState } from "react";
import { Alert, Button, Checkbox, Dialog, Select, TextField } from "../../../components/aizome";
import { DateField } from "../../../components/DateField";
import type { DayOfWeek, RepeatInfo, RepeatInput, RepeatRule } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import {
  everyDaysOf,
  leapDayNote,
  monthEndNote,
  REPEAT_KINDS,
  type RepeatKind,
  ruleFromDue,
  toggledWeekday,
  WEEK_OPTIONS,
  WEEKDAY_LABEL,
  WEEKDAYS,
} from "../repeatText";

interface RepeatDialogProps {
  /** The TODO's 日限 (a repetition needs one): the first values come from it. */
  due: string;
  /** The current repetition, when changing it. */
  current: RepeatInfo | null;
  pending: boolean;
  error: unknown;
  onSubmit: (repeat: RepeatInput) => void;
  onRemove: () => void;
  onClose: () => void;
}

const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i);
const DAY_OPTIONS = range(1, 31).map((d) => ({ value: String(d), label: `${d}日` }));
const MONTH_OPTIONS = range(1, 12).map((m) => ({ value: String(m), label: `${m}月` }));
const WEEKDAY_OPTIONS = WEEKDAYS.map((d) => ({ value: d, label: `${WEEKDAY_LABEL[d]}曜` }));

/** Each kind's rule as first shown: the current one for its kind, the others from the 日限. */
function firstRules(due: string, current: RepeatRule | null): Record<RepeatKind, RepeatRule> {
  const out = {} as Record<RepeatKind, RepeatRule>;
  for (const { value } of REPEAT_KINDS) {
    out[value] = current?.kind === value ? current : ruleFromDue(value, due);
  }
  return out;
}

/**
 * The 周期 of a repeating TODO (T-12): the kind, its days, and the last day
 * a round may fall on. The ranges are checked by Rust; its message is shown
 * at the top.
 */
export function RepeatDialog({ due, current, pending, error, onSubmit, onRemove, onClose }: RepeatDialogProps) {
  const [kind, setKind] = useState<RepeatKind>(current?.rule.kind ?? "weekly");
  const [rules, setRules] = useState(() => firstRules(due, current?.rule ?? null));
  const [daysText, setDaysText] = useState(() => {
    const r = rules.every_days;
    return r.kind === "every_days" ? String(r.days) : "";
  });
  const [until, setUntil] = useState<string | null>(current?.until_date ?? null);
  const rule = rules[kind];
  const edit = (next: RepeatRule) => setRules((all) => ({ ...all, [next.kind]: next }));

  function submit() {
    const sent: RepeatRule = rule.kind === "every_days" ? { kind: "every_days", days: everyDaysOf(daysText) } : rule;
    onSubmit({ rule: sent, until_date: until });
  }

  return (
    <Dialog
      open
      size="sm"
      onClose={pending ? undefined : onClose}
      title={current ? "繰り返しを変える" : "繰り返しを付ける"}
      description="完了か中止にすると、次の回のTODOを未着手で作ります。日限は周期から決まり、予定期間と子TODOも同じ間隔のまま引き継ぎます。"
      footer={
        <>
          {current ? (
            <div className="dialog-footer__start">
              <Button disabled={pending} onClick={onRemove}>
                繰り返しを外す
              </Button>
            </div>
          ) : null}
          <Button onClick={onClose} disabled={pending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="repeat-form" loading={pending}>
            {current ? "変更する" : "繰り返しを付ける"}
          </Button>
        </>
      }
    >
      <form
        id="repeat-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        <Select
          label="周期"
          value={kind}
          options={REPEAT_KINDS}
          onChange={(e) => setKind(e.target.value as RepeatKind)}
        />
        {rule.kind === "weekly" ? (
          <fieldset className="repeat-weekdays">
            <legend className="az-field__label">曜日</legend>
            <div className="repeat-weekdays__checks">
              {WEEKDAYS.map((d: DayOfWeek) => (
                <Checkbox
                  key={d}
                  label={WEEKDAY_LABEL[d]}
                  checked={rule.weekdays.includes(d)}
                  onChange={() => edit({ ...rule, weekdays: toggledWeekday(rule.weekdays, d) })}
                />
              ))}
            </div>
          </fieldset>
        ) : null}
        {rule.kind === "monthly_day" ? (
          <Select
            label="日付"
            value={String(rule.day)}
            options={DAY_OPTIONS}
            description={monthEndNote(rule.day) ?? undefined}
            onChange={(e) => edit({ ...rule, day: Number(e.target.value) })}
          />
        ) : null}
        {rule.kind === "monthly_weekday" ? (
          <div className="detail-fields__row">
            <Select
              label="週"
              value={String(rule.week)}
              options={WEEK_OPTIONS.map((w) => ({ value: String(w.value), label: w.label }))}
              onChange={(e) => edit({ ...rule, week: Number(e.target.value) })}
            />
            <Select
              label="曜日"
              value={rule.weekday}
              options={WEEKDAY_OPTIONS}
              onChange={(e) => edit({ ...rule, weekday: e.target.value as DayOfWeek })}
            />
          </div>
        ) : null}
        {rule.kind === "monthly_last" ? <p className="text-caption">毎月の最後の日を日限にします。</p> : null}
        {rule.kind === "yearly" ? (
          <div className="detail-fields__row">
            <Select
              label="月"
              value={String(rule.month)}
              options={MONTH_OPTIONS}
              onChange={(e) => edit({ ...rule, month: Number(e.target.value) })}
            />
            <Select
              label="日"
              value={String(rule.day)}
              options={DAY_OPTIONS}
              description={leapDayNote(rule.month, rule.day) ?? undefined}
              onChange={(e) => edit({ ...rule, day: Number(e.target.value) })}
            />
          </div>
        ) : null}
        {rule.kind === "every_days" ? (
          <TextField
            label="日数"
            inputMode="numeric"
            suffix="日ごと"
            value={daysText}
            description="2〜365日で入力します。前の回の日限から数えます。"
            onChange={(e) => setDaysText(e.target.value)}
          />
        ) : null}
        <DateField
          label="終わりの日(任意)"
          value={until}
          onChange={setUntil}
          description="この日より後の回は作りません。空なら終わりません。"
        />
      </form>
    </Dialog>
  );
}
