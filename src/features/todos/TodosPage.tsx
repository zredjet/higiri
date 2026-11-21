import { useEffect, useState } from "react";
import { Alert, Button, Tabs } from "../../components/aizome";
import { PageHeader } from "../../components/PageHeader";
import type { ProjectSummary, Status, TodoQuery } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useProjects, useTodoList } from "../../lib/queries";
import { projectStatsText } from "../projects/projectText";
import { BulkBar } from "./BulkBar";
import { BulkDialog } from "./BulkDialog";
import { bulkDoneText, type BulkKind } from "./bulkText";
import { saveOpenDrafts } from "./detail/useTodoDraft";
import { QuickAdd } from "./QuickAdd";
import { allToggled, clicked, headerState, NO_SELECTION, pruned, selectableIds, selectedIn } from "./selection";
import { DEFAULT_FILTERS, TodoFilters, type FilterState } from "./TodoFilters";
import { TodoTable } from "./TodoTable";
import { STATUSES, STATUS_LABEL } from "./todoText";
import { useFolded } from "./useFolded";
import { useSelection } from "./useSelection";

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

  // 一括変更 (T-14): the selection holds only TODOs this list shows.
  const { folded } = useFolded();
  const { selection, setSelection } = useSelection();
  const [bulk, setBulk] = useState<BulkKind | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const rows = list.data?.rows;
  const selectable = rows ? selectableIds(rows, folded) : null;
  useEffect(() => {
    if (rows) setSelection((s) => pruned(s, selectableIds(rows, folded)));
  }, [rows, folded, setSelection]);
  const selected = selectable ? selectedIn(selection, selectable) : [];

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
      {notice ? (
        <Alert tone="success" onClose={() => setNotice(null)}>
          {notice}
        </Alert>
      ) : null}
      {selected.length > 0 ? (
        <BulkBar
          count={selected.length}
          // The detail panel's typed edits are saved first: the confirmation sees them.
          onChange={(kind) => {
            setNotice(null);
            void saveOpenDrafts().then(() => setBulk(kind));
          }}
          onClear={() => setSelection(NO_SELECTION)}
        />
      ) : null}
      {list.data && selectable ? (
        <TodoTable
          rows={list.data.rows}
          showProject={projectId === null}
          selectedId={selectedTodoId}
          onOpen={onOpenTodo}
          emptyText={emptyText}
          selection={{
            selected: selection.ids,
            header: headerState(selection, selectable),
            onToggle: (id, shift) => setSelection((s) => clicked(s, selectable, id, shift)),
            onToggleAll: () => setSelection((s) => allToggled(s, selectable)),
          }}
        />
      ) : null}
      {bulk ? (
        <BulkDialog
          kind={bulk}
          ids={selected}
          showProject={projectId === null}
          onClose={() => setBulk(null)}
          onDone={(change, result) => {
            setBulk(null);
            setNotice(bulkDoneText(change, result));
          }}
        />
      ) : null}
    </>
  );
}
