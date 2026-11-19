import { useState } from "react";
import { Alert, Button, Card, RadioGroup, Table, TextField } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { ProjectSummary, ReminderFrequency, Settings, SettingsInput, Theme } from "../../lib/bindings";
import { formatTimestampDate } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import {
  useArchiveProject,
  useOpenDataFolder,
  useProjects,
  useSettings,
  useUpdateSettings,
} from "../../lib/queries";
import { DeleteProjectDialog } from "../projects/DeleteProjectDialog";
import { backupLabel } from "./backupText";
import { RestoreDialog } from "./RestoreDialog";

const THEME_OPTIONS: Array<{ value: Theme; label: string }> = [
  { value: "light", label: "ライト" },
  { value: "dark", label: "ダーク" },
  { value: "system", label: "OSの設定に合わせる" },
];

const REMINDER_OPTIONS: Array<{ value: ReminderFrequency; label: string; description: string }> = [
  {
    value: "every_launch",
    label: "起動のたびに表示",
    description: "日限注意が1件でもあれば、起動するたびにダイアログを出します。",
  },
  { value: "once_per_day", label: "1日1回だけ表示", description: "その日最初の起動のときだけダイアログを出します。" },
];

function inputOf(s: Settings): SettingsInput {
  return {
    reminder_frequency: s.reminder_frequency,
    soon_days: s.soon_days,
    follow_up_days: s.follow_up_days,
    theme: s.theme,
  };
}

/** 設定 (docs/ui-mock/Settings.dc.html). */
export function SettingsPage() {
  const settings = useSettings();
  return (
    <div className="settings-page">
      <PageHeader title="設定" actions={<span className="text-caption">変更はすぐに保存されます</span>} />
      {settings.data ? <ReminderCard key={settings.dataUpdatedAt} settings={settings.data} /> : null}
      <DisplayCard />
      <DataCard />
      <ArchivedProjectsCard />
    </div>
  );
}

/** データ: the data folder and restoring a backup (C-1). */
function DataCard() {
  const openFolder = useOpenDataFolder();
  const [restoring, setRestoring] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  return (
    <Card title="データ" description="データはこのPCの中だけに保存しています。">
      <div className="settings-card__body settings-card__body--tight">
        {openFolder.error ? <Alert tone="danger">{errorMessage(openFolder.error)}</Alert> : null}
        {notice ? (
          <Alert tone="success" onClose={() => setNotice(null)}>
            {notice}
          </Alert>
        ) : null}
        <p className="settings-card__text">1日1回、その日最初の起動時に自動でバックアップし、7日分を残します。</p>
        <div className="settings-card__actions">
          <Button iconStart="folder" disabled={openFolder.isPending} onClick={() => openFolder.mutate()}>
            データフォルダを開く
          </Button>
          <Button
            onClick={() => {
              setNotice(null);
              setRestoring(true);
            }}
          >
            バックアップから復元
          </Button>
        </div>
      </div>
      <RestoreDialog
        open={restoring}
        onClose={() => setRestoring(false)}
        onRestored={(b) => {
          setRestoring(false);
          setNotice(`「${backupLabel(b)}」から復元しました。元に戻すときは「復元する前の状態」を選んで復元してください。`);
        }}
      />
    </Card>
  );
}

/** リマインド: frequency (D-2), 「間近」の日数 and the default follow-up (D-4). */
function ReminderCard({ settings }: { settings: Settings }) {
  const update = useUpdateSettings();
  const [soon, setSoon] = useState(String(settings.soon_days));
  const [followUp, setFollowUp] = useState(String(settings.follow_up_days));
  const [localError, setLocalError] = useState<string | null>(null);

  /** Saves the number fields when they hold a changed whole number. */
  function saveDays() {
    if (!/^\d+$/.test(soon.trim()) || !/^\d+$/.test(followUp.trim())) {
      setLocalError("日数は数字で入力してください。");
      return;
    }
    setLocalError(null);
    const next = { ...inputOf(settings), soon_days: Number(soon), follow_up_days: Number(followUp) };
    if (next.soon_days === settings.soon_days && next.follow_up_days === settings.follow_up_days) return;
    update.mutate(next);
  }

  const error = localError ?? (update.error ? errorMessage(update.error) : null);

  return (
    <Card title="リマインド" description="日限注意を知らせる頻度と、区分の基準を決めます。">
      <div className="settings-card__body">
        {error ? <Alert tone="danger">{error}</Alert> : null}
        <RadioGroup
          label="起動時リマインドの表示"
          name="reminder-frequency"
          options={REMINDER_OPTIONS}
          value={settings.reminder_frequency}
          onChange={(value) =>
            update.mutate({ ...inputOf(settings), reminder_frequency: value as ReminderFrequency })
          }
        />
        <div className="settings-card__grid">
          <TextField
            label="「間近」とみなす日数"
            inputMode="numeric"
            suffix="日以内"
            description="日限がこの日数以内に来るTODOを「間近」に出します。"
            value={soon}
            onChange={(e) => setSoon(e.target.value)}
            onBlur={saveDays}
          />
          <TextField
            label="確認日の初期値"
            inputMode="numeric"
            suffix="日後"
            description="「待ち」にした日から数えます。TODOごとに変えられます。"
            value={followUp}
            onChange={(e) => setFollowUp(e.target.value)}
            onBlur={saveDays}
          />
        </div>
      </div>
    </Card>
  );
}

function DisplayCard() {
  const settings = useSettings();
  const update = useUpdateSettings();
  const s = settings.data;

  return (
    <Card title="表示">
      {update.error ? <Alert tone="danger">{errorMessage(update.error)}</Alert> : null}
      {s ? (
        <RadioGroup
          label="配色"
          name="theme"
          orientation="horizontal"
          options={THEME_OPTIONS}
          value={s.theme}
          onChange={(value) => update.mutate({ ...inputOf(s), theme: value as Theme })}
        />
      ) : null}
    </Card>
  );
}

function ArchivedProjectsCard() {
  const projects = useProjects(true);
  const restore = useArchiveProject();
  const [deleteTarget, setDeleteTarget] = useState<ProjectSummary | null>(null);
  const archived = (projects.data ?? []).filter((p) => p.project.archived_at !== null);

  return (
    <Card
      title="アーカイブしたプロジェクト"
      description="一覧・ダッシュボード・ガントから外したプロジェクトです。元に戻すと、また表示されます。"
    >
      {restore.error ? <Alert tone="danger">{errorMessage(restore.error)}</Alert> : null}
      {projects.isSuccess && archived.length === 0 ? (
        <p className="text-muted">
          アーカイブしたプロジェクトはありません。プロジェクトのページの「プロジェクトを編集」からアーカイブできます。
        </p>
      ) : null}
      {archived.length > 0 ? (
        <Table<ProjectSummary>
          rowKey="id"
          rows={archived}
          columns={[
            { key: "name", header: "プロジェクト", render: (p) => p.project.name },
            { key: "todos", header: "TODO", numeric: true, width: 80, render: (p) => `${p.todo_count}件` },
            {
              key: "archived",
              header: "アーカイブした日",
              width: 140,
              render: (p) => (p.project.archived_at ? formatTimestampDate(p.project.archived_at) : ""),
            },
            {
              key: "actions",
              header: "操作",
              align: "right",
              width: 200,
              render: (p) => (
                <div className="table-actions">
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={restore.isPending}
                    onClick={() => restore.mutate({ id: p.project.id, archived: false })}
                  >
                    元に戻す
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setDeleteTarget(p)}>
                    削除する
                  </Button>
                </div>
              ),
            },
          ]}
        />
      ) : null}
      <DeleteProjectDialog
        target={deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onDeleted={() => setDeleteTarget(null)}
      />
    </Card>
  );
}
