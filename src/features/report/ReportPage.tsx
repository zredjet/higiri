import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Alert, Button, Tabs } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import { MarkdownEditor } from "../../components/markdown/MarkdownEditor";
import { MarkdownView } from "../../components/markdown/MarkdownView";
import { PageHeader } from "../../components/PageHeader";
import { addDaysYmd, todayYmd } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { useExportReport, useGenerateReport, useSettings } from "../../lib/queries";
import {
  KIND_TABS,
  KIND_TEXT,
  copiedText,
  copyButtonText,
  reportKey,
  savedFileText,
  weekRangeText,
  type ShownKind,
} from "./reportText";
import { useReportDraft } from "./useReportDraft";
import { useReportKind } from "./useReportKind";

/**
 * 日報 (R-1〜R-4) and 週報 (R-6): pick a day (or the week of a day),
 * regenerate, edit, copy, save as .md. The kind shown is kept while the app
 * is open.
 */
export function ReportPage() {
  const [kind, setKind] = useReportKind();
  const [date, setDate] = useState(todayYmd);
  const { doc, loadError, saveError, edit, replace, saveNow } = useReportDraft(kind, reportKey(kind, date).date);
  const generate = useGenerateReport();
  const exportFile = useExportReport();
  const settings = useSettings();
  const text = KIND_TEXT[kind];
  const template = kind === "weekly" ? settings.data?.weekly_report_template : settings.data?.report_template;
  const noteHeading = template?.sections.find((s) => s.kind === "note")?.heading ?? "所感・連絡事項";
  const [notice, setNotice] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  function startAction() {
    setNotice(null);
    setActionError(null);
  }

  function go(next: string | null) {
    if (!next || next === date) return;
    startAction();
    setDate(next);
  }

  function switchKind(next: ShownKind) {
    if (next === kind) return;
    startAction();
    setKind(next);
  }

  async function regenerate() {
    if (!doc) return;
    startAction();
    try {
      const generated = await generate.mutateAsync({ key: doc.key, currentMd: doc.text });
      await replace(doc.key, generated);
    } catch (e) {
      setActionError(errorMessage(e));
    }
  }

  async function copy() {
    if (!doc) return;
    startAction();
    void saveNow();
    try {
      await writeText(doc.text);
      setNotice(copiedText(kind));
    } catch (e) {
      console.error(e);
      setActionError("クリップボードにコピーできませんでした。もう一度コピーしてください。");
    }
  }

  async function saveFile() {
    if (!doc) return;
    startAction();
    void saveNow();
    try {
      const path = await exportFile.mutateAsync({ key: doc.key, bodyMd: doc.text });
      if (path) setNotice(savedFileText(kind, path));
    } catch (e) {
      setActionError(errorMessage(e));
    }
  }

  const error = actionError ?? (loadError ? errorMessage(loadError) : saveError ? errorMessage(saveError) : null);
  const markdownLabel = `${text.name}のMarkdown`;

  return (
    <>
      <PageHeader
        title={text.name}
        meta={kind === "weekly" ? weekRangeText(date) : undefined}
        actions={
          <>
            <Button iconStart="download" disabled={!doc || exportFile.isPending} onClick={() => void saveFile()}>
              .md で保存
            </Button>
            <Button variant="primary" disabled={!doc} onClick={() => void copy()}>
              {copyButtonText(kind)}
            </Button>
          </>
        }
      />
      <Tabs items={KIND_TABS} value={kind} label="日報と週報" onChange={(id) => switchKind(id as ShownKind)} />

      <div className="report-toolbar">
        <div className="report-toolbar__dates">
          <Button
            variant="ghost"
            size="sm"
            iconStart="chevron-left"
            aria-label={text.prev}
            onClick={() => go(addDaysYmd(date, -text.step))}
          />
          <DateField label={text.dateLabel} size="sm" className="report-toolbar__date" value={date} onChange={go} />
          <Button
            variant="ghost"
            size="sm"
            iconStart="chevron-right"
            aria-label={text.next}
            onClick={() => go(addDaysYmd(date, text.step))}
          />
          <Button size="sm" onClick={() => go(todayYmd())}>
            {text.current}
          </Button>
        </div>
        <div className="report-toolbar__regen">
          <span className="text-caption">作業ログから作り直します。{noteHeading}は残ります。</span>
          <Button size="sm" disabled={!doc || generate.isPending} onClick={() => void regenerate()}>
            再生成
          </Button>
        </div>
      </div>

      {error ? <Alert tone="danger">{error}</Alert> : null}
      {notice ? (
        <Alert tone="success" onClose={() => setNotice(null)}>
          {notice}
        </Alert>
      ) : null}

      <div className="report-grid">
        <section className="report-panel report-panel--editor" aria-label={markdownLabel}>
          <div className="report-panel__head">
            <h2 className="report-panel__title">Markdown</h2>
            <span className="text-caption">直接編集できます</span>
          </div>
          <div className="report-panel__body">
            {doc ? (
              <MarkdownEditor
                // A new editor per report, so undo never brings back another report's text.
                key={`${doc.key.kind}:${doc.key.date}`}
                value={doc.text}
                onChange={edit}
                label={markdownLabel}
                font="mono"
                framed={false}
                highlightSyntax={false}
                minLines={20}
              />
            ) : null}
          </div>
        </section>

        <section className="report-panel" aria-label={`${text.name}のプレビュー`}>
          <div className="report-panel__head">
            <h2 className="report-panel__title">プレビュー</h2>
            <span className="text-caption">コピーされる内容の見え方です</span>
          </div>
          <div className="report-panel__body">{doc ? <MarkdownView source={doc.text} /> : null}</div>
        </section>
      </div>
    </>
  );
}
