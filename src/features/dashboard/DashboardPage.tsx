import { useState, type ReactNode } from "react";
import { Alert, Badge, Button, Card, Table, type IconName, type TableColumn } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { Dashboard, ProjectSummary, Status, TodoItem, Urgency } from "../../lib/bindings";
import { formatDate, formatDateWithWeekday, monthDay } from "../../lib/date";
import { errorMessage } from "../../lib/errors";
import { useChangeStatus, useDashboard } from "../../lib/queries";
import {
  periodText,
  situationText,
  STATUS_LABEL,
  STATUS_TONE,
  URGENCY_LABEL,
  URGENCY_TONE,
} from "../todos/todoText";
import { ChangeDateDialog, type DateTarget } from "./ChangeDateDialog";

interface DashboardPageProps {
  onOpenTodo: (id: number) => void;
  /** 「TODOを追加」 (the dialog belongs to the app, so Ctrl+N opens the same one). */
  onAddTodo: () => void;
}

/** ダッシュボード (docs/ui-mock/Main.dc.html): 日限注意, 今日の作業, 待ち, 進捗. */
export function DashboardPage({ onOpenTodo, onAddTodo }: DashboardPageProps) {
  const dash = useDashboard();
  const d = dash.data;

  return (
    <>
      <PageHeader
        title="ダッシュボード"
        meta={d ? formatDateWithWeekday(d.today) : undefined}
        actions={
          <Button variant="primary" iconStart="plus" onClick={onAddTodo}>
            TODOを追加
          </Button>
        }
      />
      {dash.error ? <Alert tone="danger">{errorMessage(dash.error)}</Alert> : null}
      {d ? (
        <>
          <CountCards dash={d} />
          <AttentionSection dash={d} onOpenTodo={onOpenTodo} />
          <div className="dash-cards">
            <TodayWorkCard items={d.today_work} onOpenTodo={onOpenTodo} />
            <WaitingCard items={d.waiting} onOpenTodo={onOpenTodo} />
            <ProgressCard projects={d.projects} />
          </div>
        </>
      ) : null}
    </>
  );
}

// ---- 件数 ----------------------------------------------------------------------

function CountCards({ dash }: { dash: Dashboard }) {
  const cards: Array<{ urgency: Urgency; count: number; text: string }> = [
    { urgency: "overdue", count: dash.counts.overdue, text: "日限を過ぎています" },
    { urgency: "due_today", count: dash.counts.due_today, text: "今日が日限です" },
    {
      urgency: "due_soon",
      count: dash.counts.due_soon,
      text: `${dash.soon_days}${dash.business_days ? "営業日" : "日"}以内に日限が来ます`,
    },
    { urgency: "follow_up", count: dash.counts.follow_up, text: "待ちの確認日が来ています" },
    { urgency: "late_start", count: dash.counts.late_start, text: "予定開始日を過ぎています" },
  ];
  return (
    <section aria-label="日限注意の件数" className="count-cards">
      {cards.map((c) => (
        <div key={c.urgency} className="count-card">
          <Badge tone={URGENCY_TONE[c.urgency]} dot>
            {URGENCY_LABEL[c.urgency]}
          </Badge>
          <div className="count-card__value">
            <span className="count-card__number">{c.count}</span>
            <span className="text-caption">件</span>
          </div>
          <span className="text-caption">{c.text}</span>
        </div>
      ))}
    </section>
  );
}

// ---- 日限注意 ------------------------------------------------------------------

interface RowAction {
  label: string;
  icon: IconName;
  run: () => void;
}

function AttentionSection({ dash, onOpenTodo }: { dash: Dashboard; onOpenTodo: (id: number) => void }) {
  const status = useChangeStatus();
  const [dateEdit, setDateEdit] = useState<{ item: TodoItem; target: DateTarget } | null>(null);

  function changeTo(item: TodoItem, to: Status) {
    status.mutate({ id: item.todo.id, change: { to, waiting_on: null, follow_up_date: null, reason: null } });
  }

  /** The two buttons of a row, as in the mock. */
  function actions(item: TodoItem): [RowAction, RowAction] {
    const u = item.assessment.urgency;
    const first: RowAction =
      u === "late_start"
        ? { label: "着手する", icon: "arrow-right", run: () => changeTo(item, "doing") }
        : item.todo.status === "waiting"
          ? { label: "作業に戻す", icon: "arrow-right", run: () => changeTo(item, "doing") }
          : { label: "完了にする", icon: "check", run: () => changeTo(item, "done") };
    const second: RowAction =
      u === "follow_up"
        ? { label: "確認日を変更", icon: "calendar", run: () => setDateEdit({ item, target: "follow_up_date" }) }
        : { label: "日限を変更", icon: "calendar", run: () => setDateEdit({ item, target: "due_date" }) };
    return [first, second];
  }

  const columns: TableColumn<TodoItem>[] = [
    {
      key: "kind",
      header: "区分",
      width: 100,
      render: (i) =>
        i.assessment.urgency ? (
          <Badge tone={URGENCY_TONE[i.assessment.urgency]} dot>
            {URGENCY_LABEL[i.assessment.urgency]}
          </Badge>
        ) : null,
    },
    { key: "title", header: "TODO", render: (i) => <span className="dash-strong">{i.todo.title}</span> },
    { key: "project", header: "プロジェクト", render: (i) => <span className="text-muted">{i.project_name}</span> },
    {
      key: "due",
      header: "日限",
      width: 100,
      render: (i) => <span className="tabular">{i.todo.due_date ? formatDate(i.todo.due_date) : "なし"}</span>,
    },
    {
      key: "state",
      header: "状況",
      width: 160,
      render: (i) => <span className="dash-nowrap">{situationText(i.todo, i.assessment)}</span>,
    },
    {
      key: "actions",
      header: "操作",
      align: "right",
      width: 240,
      render: (i) => (
        <div className="table-actions">
          {actions(i).map((a) => (
            <Button
              key={a.label}
              size="sm"
              variant="ghost"
              iconStart={a.icon}
              disabled={status.isPending}
              onClick={(e) => {
                e.stopPropagation();
                a.run();
              }}
            >
              {a.label}
            </Button>
          ))}
        </div>
      ),
    },
  ];

  return (
    <section className="dash-section">
      <div className="dash-section__head">
        <h2 className="section-title">日限注意</h2>
        <span className="text-caption">区分の重い順、日限の古い順に並べています</span>
      </div>
      {status.error ? <Alert tone="danger">{errorMessage(status.error)}</Alert> : null}
      <Table<TodoItem>
        columns={columns}
        rows={dash.attention.map((i) => ({ ...i, id: i.todo.id }))}
        rowKey="id"
        onRowClick={(i) => onOpenTodo(i.todo.id)}
        emptyText="日限注意のTODOはありません。日限や予定を入れたTODOは、近づくとここに出ます。"
      />
      {dateEdit ? (
        <ChangeDateDialog item={dateEdit.item} target={dateEdit.target} onClose={() => setDateEdit(null)} />
      ) : null}
    </section>
  );
}

// ---- カード ----------------------------------------------------------------------

function DashRow({ title, sub, right, onClick }: { title: string; sub: string; right: ReactNode; onClick: () => void }) {
  return (
    <button type="button" className="dash-row" onClick={onClick}>
      <span className="dash-row__text">
        <span className="dash-strong">{title}</span>
        <span className="text-caption">{sub}</span>
      </span>
      {right}
    </button>
  );
}

function TodayWorkCard({ items, onOpenTodo }: { items: TodoItem[]; onOpenTodo: (id: number) => void }) {
  return (
    <Card title="今日の作業" description="作業中と、今日が予定期間に入るTODOです。">
      {items.length === 0 ? (
        <p className="text-muted">今日の作業はありません。予定期間を入れたTODOが、その期間に入るとここに出ます。</p>
      ) : (
        <div className="dash-rows">
          {items.map(({ todo, project_name }) => (
            <DashRow
              key={todo.id}
              title={todo.title}
              sub={`${project_name} ・ ${periodText("予定", todo.planned_start, todo.planned_end)}`}
              right={
                <Badge tone={STATUS_TONE[todo.status]} dot>
                  {STATUS_LABEL[todo.status]}
                </Badge>
              }
              onClick={() => onOpenTodo(todo.id)}
            />
          ))}
        </div>
      )}
    </Card>
  );
}

function WaitingCard({ items, onOpenTodo }: { items: TodoItem[]; onOpenTodo: (id: number) => void }) {
  return (
    <Card title="待ち" description="相手の作業を待っているTODOです。">
      {items.length === 0 ? (
        <p className="text-muted">待ちのTODOはありません。</p>
      ) : (
        <div className="dash-rows">
          {items.map(({ todo, assessment }) => (
            <DashRow
              key={todo.id}
              title={todo.title}
              sub={`${todo.waiting_on ?? ""}待ち${assessment.waiting_day ? ` ・ ${assessment.waiting_day}日目` : ""}`}
              right={
                assessment.urgency === "follow_up" ? (
                  <Badge tone="warning" dot>
                    要確認
                  </Badge>
                ) : (
                  <span className="text-caption dash-nowrap">
                    {todo.follow_up_date ? `確認日 ${monthDay(todo.follow_up_date)}` : ""}
                  </span>
                )
              }
              onClick={() => onOpenTodo(todo.id)}
            />
          ))}
        </div>
      )}
    </Card>
  );
}

function ProgressCard({ projects }: { projects: ProjectSummary[] }) {
  return (
    <Card title="プロジェクトの進捗" description="中止を除いた件数で数えています。">
      {projects.length === 0 ? (
        <p className="text-muted">プロジェクトはまだありません。サイドナビの「プロジェクトを追加」から作成してください。</p>
      ) : (
        <div className="progress-list">
          {projects.map((p) => {
            const ratio = p.progress_total > 0 ? p.progress_done / p.progress_total : 0;
            return (
              <div key={p.project.id} className="progress-item">
                <div className="progress-item__head">
                  <span className="dash-strong">{p.project.name}</span>
                  <span className="text-caption">
                    {p.progress_done} / {p.progress_total} 完了 ・ 日限注意 {p.attention_count}件
                  </span>
                </div>
                <div className="progress-bar" role="img" aria-label={`${p.progress_total}件中${p.progress_done}件完了`}>
                  <div className="progress-bar__fill" style={{ width: `${Math.round(ratio * 1000) / 10}%` }} />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </Card>
  );
}
