import { useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Checkbox, Dialog, Icon, Switch, TextField } from "../../components/aizome";
import type { ReportTemplate, WeeklyReportTemplate } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useUpdateSettings } from "../../lib/queries";
import {
  TEMPLATE_SPECS,
  moveSection,
  optionValue,
  setEnabled,
  setHeading,
  setOption,
  setTitle,
  templateEdits,
  templateOf,
  type AnySection,
  type AnyTemplate,
  type TemplateSpec,
} from "./reportTemplate";

/** What a save gives back: the template as saved, or the message to show. */
type SaveResult = { template: AnyTemplate } | { error: string };

type ReportTemplateCardProps =
  | { kind: "daily"; template: ReportTemplate }
  | { kind: "weekly"; template: WeeklyReportTemplate };

/**
 * 日報のテンプレート (R-5, v1.1) and 週報のテンプレート (R-6, v0.3.0): title,
 * order, headings, shown sections and items. The two cards are built the same
 * way; `kind` picks the template, its sections and their items.
 */
export function ReportTemplateCard({ kind, template }: ReportTemplateCardProps) {
  const spec = TEMPLATE_SPECS[kind];
  const update = useUpdateSettings();
  const [error, setError] = useState<string | null>(null);
  // A new set of text fields after 「初期値に戻す」, dropping unsaved text.
  const [generation, setGeneration] = useState(0);
  const [resetting, setResetting] = useState(false);

  /** Saves `change` applied to the template as saved right now. */
  async function save(change: (current: AnyTemplate) => AnyTemplate): Promise<SaveResult> {
    try {
      const saved = await update.mutateAsync((current) => templateEdits(kind, change(templateOf(kind, current))));
      return { template: templateOf(kind, saved) };
    } catch (e) {
      return { error: errorMessage(e) };
    }
  }

  /** Switches, checkboxes and the order: the error shows at the top of the card. */
  async function saveNow(change: (current: AnyTemplate) => AnyTemplate) {
    setError(null);
    const result = await save(change);
    if ("error" in result) setError(result.error);
  }

  const sections: AnySection[] = template.sections;
  const movable = sections.filter((s) => s.kind !== "note");

  return (
    <Card
      title={`${spec.name}のテンプレート`}
      description={`${spec.name}の題名、節の並び、見出し、添える項目を決めます。次に作る${spec.name}と、再生成した${spec.name}から変わります。`}
    >
      <div className="settings-card__body" key={generation}>
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <DraftTextField
          label="題名"
          description={spec.titleDescription}
          maxLength={60}
          value={template.title}
          onSave={async (text) => {
            const result = await save((t) => setTitle(t, text));
            return "error" in result ? result : { value: result.template.title };
          }}
        />
        <div className="report-sections" role="list" aria-label={`${spec.name}の節`}>
          {sections.map((s) => {
            const index = movable.findIndex((m) => m.kind === s.kind);
            return (
              <SectionRow
                key={s.kind}
                spec={spec}
                section={s}
                first={index === 0}
                last={index === movable.length - 1}
                onMove={(delta) => void saveNow((t) => moveSection(t, s.kind, delta))}
                onEnabled={(on) => void saveNow((t) => setEnabled(t, s.kind, on))}
                onOption={(key, on) => void saveNow((t) => setOption(t, s.kind, key, on))}
                onHeading={async (text) => {
                  const result = await save((t) => setHeading(t, s.kind, text));
                  if ("error" in result) return result;
                  const saved = (result.template.sections as AnySection[]).find((x) => x.kind === s.kind);
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
        name={spec.name}
        open={resetting}
        onClose={() => setResetting(false)}
        onReset={async () => {
          const result = await save(() => spec.defaultTemplate());
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
  /** 「日報」「週報」 */
  name: string;
  open: boolean;
  onClose: () => void;
  /** Saves the default template; resolves to the error message, or null when saved. */
  onReset: () => Promise<string | null>;
}

/** Confirms 「初期値に戻す」: every change on the card is saved at once and cannot be undone. */
function ResetDialog({ name, open, onClose, onReset }: ResetDialogProps) {
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
      title={`${name}のテンプレートを初期値に戻しますか`}
      description={`節の並び、見出し、出す節、項目が、初めの状態に戻ります。保存してある${name}は変わりません。`}
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
  spec: TemplateSpec;
  section: AnySection;
  first: boolean;
  last: boolean;
  onMove: (delta: -1 | 1) => void;
  onEnabled: (on: boolean) => void;
  onOption: (key: string, on: boolean) => void;
  onHeading: (text: string) => Promise<{ value: string } | { error: string }>;
}

function SectionRow({ spec, section, first, last, onMove, onEnabled, onOption, onHeading }: SectionRowProps) {
  const kind = section.kind;
  const name = spec.sectionName(kind);
  const note = kind === "note";
  const items = spec.optionItems(kind);

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
            <Switch label={`${spec.name}に出す`} checked={section.enabled} onChange={onEnabled} />
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
