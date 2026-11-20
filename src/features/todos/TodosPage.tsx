import { useState } from "react";
import { Alert, Button, Tabs } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { ProjectSummary, Status, TodoQuery } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useProjects, useTodoList } from "../../lib/queries";
import { projectStatsText } from "../projects/projectText";
import { QuickAdd } from "./QuickAdd";
import { DEFAULT_FILTERS, TodoFilters, type FilterState } from "./TodoFilters";
import { TodoTable } from "./TodoTable";
import { STATUSES, STATUS_LABEL } from "./todoText";

interface TodosPageProps {
  /** null: all active projects. */
  projectId: number | null;
  selectedTodoId: number | null;
  onOpenTodo: (id: number) => void;
  onEditProject: (summary: ProjectSummary) => void;
  /** 「TODOを追加」 (the dialog belongs to the app, so Ctrl+N opens the same one). */
  onAddTodo: () => void;
}

type Tab = Status | "all";

/** TODO一覧: status tabs, filters, quick add (project page only), the table. */
export function TodosPage(props: TodosPageProps) {
  // Each project keeps its own tab and filters while the page is shown.
  return <TodosView key={props.projectId ?? "all"} {...props} />;
}

function TodosView({ projectId, selectedTodoId, onOpenTodo, onEditProject, onAddTodo }: TodosPageProps) {
  const projects = useProjects(false);
  const [tab, setTab] = useState<Tab>("all");
  const [filters, setFilters] = useState<FilterState>(DEFAULT_FILTERS);

  const query: TodoQuery = {
    project_id: projectId,
    statuses: tab === "all" ? null : [tab],
    priority: filters.priority === "all" ? null : filters.priority,
    due: filters.due,
    sort: filters.sort,
  };
  const list = useTodoList(query);

  const summary = projectId === null ? null : projects.data?.find((p) => p.project.id === projectId);
  if (projectId !== null && !summary) return null;

  const filtered = filters.due !== "all" || filters.priority !== "all";
  const emptyText = filtered
    ? "条件に合うTODOはありません。絞り込みを変えてください。"
    : tab !== "all"
      ? `「${STATUS_LABEL[tab]}」のTODOはありません。`
      : projectId !== null
        ? "TODOはまだありません。クイック追加にタイトルを入力して追加してください。"
        : "TODOはまだありません。「TODOを追加」から追加してください。";

  const counts = list.data?.counts;
  const tabs = [
    { id: "all", label: "すべて", count: list.data?.total },
    ...STATUSES.map((s) => ({ id: s, label: STATUS_LABEL[s], count: counts?.[s] })),
  ];

  return (
    <>
      <PageHeader
        title={summary ? summary.project.name : "すべてのTODO"}
        subtitle={summary ? projectStatsText(summary) : undefined}
        actions={
          <>
            {summary ? <Button onClick={() => onEditProject(summary)}>プロジェクトを編集</Button> : null}
            <Button variant="primary" iconStart="plus" onClick={onAddTodo}>
              TODOを追加
            </Button>
          </>
        }
      />
      <Tabs items={tabs} value={tab} onChange={(id) => setTab(id as Tab)} label="ステータスで絞り込み" />
      <TodoFilters value={filters} onChange={setFilters} />
      {projectId !== null ? <QuickAdd projectId={projectId} /> : null}
      {list.error ? <Alert tone="danger">{errorMessage(list.error)}</Alert> : null}
      {list.data ? (
        <TodoTable
          rows={list.data.rows}
          showProject={projectId === null}
          selectedId={selectedTodoId}
          onOpen={onOpenTodo}
          emptyText={emptyText}
        />
      ) : null}
    </>
  );
}
