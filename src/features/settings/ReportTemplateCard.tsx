import { useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Checkbox, Dialog, Icon, Switch, TextField } from "../../components/aizome";
import type { ReportTemplate, Section } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useUpdateSettings } from "../../lib/queries";
import {
  OPTION_ITEMS,
  defaultReportTemplate,
  moveSection,
  optionValue,
  sectionName,
  setEnabled,
  setHeading,
  setOption,
  setTitle,
  type SectionKind,
} from "./reportTemplate";

/** What a save gives back: the template as saved, or the message to show. */
type SaveResult = { template: ReportTemplate } | { error: string };

/** 日報のテンプレート (R-5, v1.1): title, order, headings, shown sections and items. */
export function ReportTemplateCard({ template }: { template: ReportTemplate }) {
  const update = useUpdateSettings();
  const [error, setError] = useState<string | null>(null);
  // A new set of text fields after 「初期値に戻す」, dropping unsaved text.
  const [generation, setGeneration] = useState(0);
  const [resetting, setResetting] = useState(false);

  /** Saves `change` applied to the template as saved right now. */
  async function save(change: (current: ReportTemplate) => ReportTemplate): Promise<SaveResult> {
    try {
      const saved = await update.mutateAsync((current) => ({ report_template: change(current.report_template) }));
      return { template: saved.report_template };
    } catch (e) {
      return { error: errorMessage(e) };
    }
  }

  /** Switches, checkboxes and the order: the error shows at the top of the card. */
  async function saveNow(change: (current: ReportTemplate) => ReportTemplate) {
    setError(null);
    const result = await save(change);
    if ("error" in result) setError(result.error);
  }

  const movable = template.sections.filter((s) => s.kind !== "note");

  return (
    <Card
      title="日報のテンプレート"
      description="日報の題名、節の並び、見出し、添える項目を決めます。次に作る日報と、再生成した日報から変わります。"
    >
      <div className="settings-card__body" key={generation}>
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <DraftTextField
          label="題名"
          description="「{日付}」と「{曜日}」は、その日の日付と曜日に置き換わります。先頭の「# 」はアプリが付けます。"
          maxLength={60}
          value={template.title}
          onSave={async (text) => {
            const result = await save((t) => setTitle(t, text));
            return "error" in result ? result : { value: result.template.title };
          }}
        />
        <div className="report-sections" role="list" aria-label="日報の節">
          {template.sections.map((s) => {
            const index = movable.findIndex((m) => m.kind === s.kind);
            return (
              <SectionRow
                key={s.kind}
                section={s}
                first={index === 0}
                last={index === movable.length - 1}
                onMove={(delta) => void saveNow((t) => moveSection(t, s.kind, delta))}
                onEnabled={(on) => void saveNow((t) => setEnabled(t, s.kind, on))}
                onOption={(key, on) => void saveNow((t) => setOption(t, s.kind, key, on))}
                onHeading={async (text) => {
                  const result = await save((t) => setHeading(t, s.kind, text));
                  if ("error" in result) return result;
                  const saved = result.template.sections.find((x) => x.kind === s.kind);
                  return { value: saved?.heading ?? text };
                }}
              />
            );
          })}
        </div>
        <div className="settings-card__actions">
          <Button onClick={() => setResetting(true)}>初期値に戻す</Button>
        </div>
      </div>
      <ResetDialog
        open={resetting}
        onClose={() => setResetting(false)}
        onReset={async () => {
          const result = await save(() => defaultReportTemplate());
          if ("error" in result) return result.error;
          setError(null);
          setGeneration((g) => g + 1);
          setResetting(false);
          return null;
        }}
      />
    </Card>
  );
}

interface ResetDialogProps {
  open: boolean;
  onClose: () => void;
  /** Saves the default template; resolves to the error message, or null when saved. */
  onReset: () => Promise<string | null>;
}

/** Confirms 「初期値に戻す」: every change on the card is saved at once and cannot be undone. */
function ResetDialog({ open, onClose, onReset }: ResetDialogProps) {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!open) return null;

  function close() {
    setError(null);
    onClose();
  }

  return (
    <Dialog
      open
      onClose={pending ? undefined : close}
      title="日報のテンプレートを初期値に戻しますか"
      description="節の並び、見出し、出す節、項目が、初めの状態に戻ります。保存してある日報は変わりません。"
      footer={
        <>
          <Button onClick={close} disabled={pending}>
            キャンセル
          </Button>
          <Button
            variant="primary"
            loading={pending}
            onClick={async () => {
              setPending(true);
              const message = await onReset();
              setPending(false);
              setError(message);
            }}
          >
            初期値に戻す
          </Button>
        </>
      }
    >
      {error ? <Alert tone="danger">{error}</Alert> : null}
    </Dialog>
  );
}

interface SectionRowProps {
  section: Section;
  first: boolean;
  last: boolean;
  onMove: (delta: -1 | 1) => void;
  onEnabled: (on: boolean) => void;
  onOption: (key: string, on: boolean) => void;
  onHeading: (text: string) => Promise<{ value: string } | { error: string }>;
}

function SectionRow({ section, first, last, onMove, onEnabled, onOption, onHeading }: SectionRowProps) {
  const kind: SectionKind = section.kind;
  const name = sectionName(kind);
  const note = kind === "note";
  const items = OPTION_ITEMS[kind];

  return (
    <div
      className={`report-section${section.enabled ? "" : " report-section--off"}`}
      role="listitem"
      aria-label={name}
    >
      <div className="report-section__head">
        <span className="report-section__name">{name}</span>
        {note ? (
          <span className="text-caption">いつも最後に出します。再生成しても、書いた内容は残ります。</span>
        ) : (
          <div className="report-section__tools">
            <Switch label="日報に出す" checked={section.enabled} onChange={onEnabled} />
            <Button
              size="sm"
              variant="ghost"
              iconStart={<Icon name="chevron-down" size={16} className="icon-up" />}
              aria-label={`「${name}」を上へ移動する`}
              disabled={first}
              onClick={() => onMove(-1)}
            />
            <Button
              size="sm"
              variant="ghost"
              iconStart="chevron-down"
              aria-label={`「${name}」を下へ移動する`}
              disabled={last}
              onClick={() => onMove(1)}
            />
          </div>
        )}
      </div>
      <div className="report-section__body">
        <DraftTextField
          label="見出し"
          size="sm"
          maxLength={40}
          className="report-section__heading"
          value={section.heading}
          onSave={onHeading}
        />
        {items.length > 0 ? (
          <fieldset className="report-section__items">
            <legend className="az-field__label">添える項目</legend>
            <div className="report-section__checks">
              {items.map((item) => (
                <Checkbox
                  key={item.key}
                  label={item.label}
                  description={item.description}
                  checked={optionValue(section, item.key)}
                  onChange={(e) => onOption(item.key, e.target.checked)}
                />
              ))}
            </div>
          </fieldset>
        ) : null}
      </div>
    </div>
  );
}

interface DraftTextFieldProps {
  label: string;
  description?: string;
  size?: "sm" | "md";
  maxLength: number;
  className?: string;
  /** The saved value. */
  value: string;
  /** Saves the text; resolves to the value as saved (trimmed and the like), or the error. */
  onSave: (text: string) => Promise<{ value: string } | { error: string }>;
}

/**
 * A text field saved when it loses focus, like the other fields of the
 * settings screen. Text not saved yet (or refused, with its error under the
 * field until it is edited again) stays while other settings change; a new
 * saved value replaces the text only when it was not edited.
 */
function DraftTextField({ label, description, size, maxLength, className, value, onSave }: DraftTextFieldProps) {
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);
  // The saved value the text was last in step with.
  const base = useRef(value);

  useEffect(() => {
    setDraft((d) => (d === base.current ? value : d));
    base.current = value;
  }, [value]);

  async function save() {
    if (draft === base.current) {
      setError(null);
      return;
    }
    const sent = draft;
    const result = await onSave(sent);
    if ("error" in result) {
      setError(result.error);
      return;
    }
    setError(null);
    base.current = result.value;
    setDraft((d) => (d === sent ? result.value : d));
  }

  return (
    <TextField
      label={label}
      description={description}
      error={error ?? undefined}
      size={size}
      maxLength={maxLength}
      className={className}
      value={draft}
      onChange={(e) => {
        setDraft(e.target.value);
        setError(null);
      }}
      onBlur={() => void save()}
    />
  );
}
