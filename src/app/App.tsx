import { useCallback, useEffect, useState } from "react";
import { Alert, SideNav } from "../components/aizome";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { ReminderDialog } from "../features/dashboard/ReminderDialog";
import { GanttPage } from "../features/gantt/GanttPage";
import { DeleteProjectDialog } from "../features/projects/DeleteProjectDialog";
import { ProjectFormDialog } from "../features/projects/ProjectFormDialog";
import { ReportPage } from "../features/report/ReportPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TodoDetailPanel } from "../features/todos/detail/TodoDetailPanel";
import { TodosPage } from "../features/todos/TodosPage";
import type { ProjectSummary } from "../lib/bindings";
import { errorMessage } from "../lib/errors";
import { useDashboard, useProjects, useSettings } from "../lib/queries";
import { buildNavSections } from "./nav";
import { NEW_PROJECT_NAV_ID, navIdToRoute, routeToNavId, type Route } from "./routes";
import { useAppliedTheme } from "./theme";
import { useStartupReminder } from "./useStartupReminder";

const ALL_TODOS: Route = { page: "todos", projectId: null };

/** Screens that can show the TODO detail panel (not 日報 or 設定). */
const PANEL_PAGES: Route["page"][] = ["dashboard", "todos", "gantt"];

export default function App() {
  const [route, setRouteState] = useState<Route>({ page: "dashboard" });
  /** TODO shown in the right-hand detail panel. */
  const [openTodoId, setOpenTodoId] = useState<number | null>(null);
  const closeTodo = useCallback(() => setOpenTodoId(null), []);
  const setRoute = useCallback((next: Route) => {
    setRouteState(next);
    setOpenTodoId(null);
  }, []);
  /** Project dialog: closed, creating, or editing the project with this id. */
  const [projectForm, setProjectForm] = useState<null | "new" | number>(null);
  const [deleteTarget, setDeleteTarget] = useState<ProjectSummary | null>(null);

  const projects = useProjects(false);
  const settings = useSettings();
  const dashboard = useDashboard();
  const { reminder, acknowledge } = useStartupReminder();
  useAppliedTheme(settings.data?.theme);

  // Leave a project's page once it is archived or deleted.
  useEffect(() => {
    if (
      route.page === "todos" &&
      route.projectId !== null &&
      projects.data &&
      !projects.data.some((p) => p.project.id === route.projectId)
    ) {
      setRoute(ALL_TODOS);
    }
  }, [route, projects.data, setRoute]);

  function select(id: string) {
    if (id === NEW_PROJECT_NAV_ID) {
      setProjectForm("new");
      return;
    }
    const next = navIdToRoute(id);
    if (next) setRoute(next);
  }

  const formTarget =
    projectForm === null
      ? null
      : projectForm === "new"
        ? "new"
        : (projects.data?.find((p) => p.project.id === projectForm) ?? null);

  return (
    <div className="app">
      <div className="app__nav">
        <SideNav
          header="Higiri"
          label="メインナビゲーション"
          sections={buildNavSections(projects.data ?? [], dashboard.data?.attention_total)}
          activeId={routeToNavId(route)}
          onSelect={select}
        />
      </div>

      <main className="app__main">
        {projects.error ? <Alert tone="danger">{errorMessage(projects.error)}</Alert> : null}
        {route.page === "dashboard" && <DashboardPage onOpenTodo={setOpenTodoId} />}
        {route.page === "todos" && (
          <TodosPage
            projectId={route.projectId}
            selectedTodoId={openTodoId}
            onOpenTodo={setOpenTodoId}
            onEditProject={(s) => setProjectForm(s.project.id)}
          />
        )}
        {route.page === "gantt" && <GanttPage />}
        {route.page === "report" && <ReportPage />}
        {route.page === "settings" && <SettingsPage />}
      </main>

      {openTodoId !== null && PANEL_PAGES.includes(route.page) ? (
        <TodoDetailPanel key={openTodoId} todoId={openTodoId} onClose={closeTodo} />
      ) : null}

      {reminder ? <ReminderDialog reminder={reminder} onClose={acknowledge} /> : null}

      <ProjectFormDialog
        target={formTarget}
        onClose={() => setProjectForm(null)}
        onCreated={(p) => {
          setProjectForm(null);
          setRoute({ page: "todos", projectId: p.id });
        }}
        onArchived={() => {
          setProjectForm(null);
          setRoute(ALL_TODOS);
        }}
        onDeleteRequest={(s) => {
          setProjectForm(null);
          setDeleteTarget(s);
        }}
      />
      <DeleteProjectDialog
        target={deleteTarget}
        onClose={() => setDeleteTarget(null)}
        onDeleted={() => {
          setDeleteTarget(null);
          setRoute(ALL_TODOS);
        }}
      />
    </div>
  );
}
