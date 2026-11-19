import { useState } from "react";
import { Alert, Button, Card, RadioGroup, Table } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { ProjectSummary, Theme } from "../../lib/bindings";
import { formatTimestampDate } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { useArchiveProject, useProjects, useSettings, useUpdateSettings } from "../../lib/queries";
import { DeleteProjectDialog } from "../projects/DeleteProjectDialog";

const THEME_OPTIONS: Array<{ value: Theme; label: string }> = [
  { value: "light", label: "ライト" },
  { value: "dark", label: "ダーク" },
  { value: "system", label: "OSの設定に合わせる" },
];

/** 設定. The リマインド and データ cards come in phases 5 and 8. */
export function SettingsPage() {
  return (
    <div className="settings-page">
      <PageHeader title="設定" actions={<span className="text-caption">変更はすぐに保存されます</span>} />
      <DisplayCard />
      <ArchivedProjectsCard />
    </div>
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
          onChange={(value) =>
            update.mutate({
              reminder_frequency: s.reminder_frequency,
              soon_days: s.soon_days,
              follow_up_days: s.follow_up_days,
              theme: value as Theme,
            })
          }
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
