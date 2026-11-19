import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { Alert, Button } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import { MarkdownEditor } from "../../components/markdown/MarkdownEditor";
import { MarkdownView } from "../../components/markdown/MarkdownView";
import { PageHeader } from "../../components/PageHeader";
import { addDaysYmd, todayYmd } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { useExportReport, useGenerateReport, useSettings } from "../../lib/queries";
import { useReportDraft } from "./useReportDraft";

/** 日報 (R-1〜R-4): pick a day, regenerate, edit, copy, save as .md. */
export function ReportPage() {
  const [date, setDate] = useState(todayYmd);
  const { doc, loadError, saveError, edit, replace, saveNow } = useReportDraft(date);
  const generate = useGenerateReport();
  const exportFile = useExportReport();
  const settings = useSettings();
  const noteHeading =
    settings.data?.report_template.sections.find((s) => s.kind === "note")?.heading ?? "所感・連絡事項";
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

  async function regenerate() {
    if (!doc) return;
    startAction();
    try {
      const text = await generate.mutateAsync({ key: doc.key, currentMd: doc.text });
      await replace(doc.key, text);
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
      setNotice("日報をクリップボードにコピーしました。");
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
      if (path) setNotice(`日報を保存しました。保存先: ${path}`);
    } catch (e) {
      setActionError(errorMessage(e));
    }
  }

  const error = actionError ?? (loadError ? errorMessage(loadError) : saveError ? errorMessage(saveError) : null);

  return (
    <>
      <PageHeader
        title="日報"
        actions={
          <>
            <Button iconStart="download" disabled={!doc || exportFile.isPending} onClick={() => void saveFile()}>
              .md で保存
            </Button>
            <Button variant="primary" disabled={!doc} onClick={() => void copy()}>
              日報をコピー
            </Button>
          </>
        }
      />

      <div className="report-toolbar">
        <div className="report-toolbar__dates">
          <Button
            variant="ghost"
            size="sm"
            iconStart="chevron-left"
            aria-label="前の日へ"
            onClick={() => go(addDaysYmd(date, -1))}
          />
          <DateField label="対象日" size="sm" className="report-toolbar__date" value={date} onChange={go} />
          <Button
            variant="ghost"
            size="sm"
            iconStart="chevron-right"
            aria-label="次の日へ"
            onClick={() => go(addDaysYmd(date, 1))}
          />
          <Button size="sm" onClick={() => go(todayYmd())}>
            今日
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
        <section className="report-panel report-panel--editor" aria-label="日報のMarkdown">
          <div className="report-panel__head">
            <h2 className="report-panel__title">Markdown</h2>
            <span className="text-caption">直接編集できます</span>
          </div>
          <div className="report-panel__body">
            {doc ? (
              <MarkdownEditor
                // A new editor per day, so undo never brings back another day's text.
                key={doc.key.date}
                value={doc.text}
                onChange={edit}
                label="日報のMarkdown"
                font="mono"
                framed={false}
                highlightSyntax={false}
                minLines={20}
              />
            ) : null}
          </div>
        </section>

        <section className="report-panel" aria-label="日報のプレビュー">
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
