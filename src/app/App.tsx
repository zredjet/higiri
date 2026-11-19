import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, SideNav } from "../components/aizome";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { ReminderDialog } from "../features/dashboard/ReminderDialog";
import { GanttPage } from "../features/gantt/GanttPage";
import { DeleteProjectDialog } from "../features/projects/DeleteProjectDialog";
import { ProjectFormDialog } from "../features/projects/ProjectFormDialog";
import { ReportPage } from "../features/report/ReportPage";
import { SettingsPage } from "../features/settings/SettingsPage";
import { TodoDetailPanel } from "../features/todos/detail/TodoDetailPanel";
import { NewTodoDialog } from "../features/todos/NewTodoDialog";
import { TodosPage } from "../features/todos/TodosPage";
import type { ProjectSummary } from "../lib/bindings";
import { errorMessage } from "../lib/errors";
import { overlayOpen } from "../lib/keys";
import { logInfo } from "../lib/log";
import { useDashboard, useProjects, useSettings } from "../lib/queries";
import { buildNavSections } from "./nav";
import { NEW_PROJECT_NAV_ID, navIdToRoute, routeToNavId, type Route } from "./routes";
import { shortcutFor } from "./shortcuts";
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
  /** 「TODOを追加」 dialog, with the project it starts on (null: the first project). */
  const [newTodo, setNewTodo] = useState<{ projectId: number | null } | null>(null);

  const projects = useProjects(false);
  const settings = useSettings();
  const dashboard = useDashboard();
  const { reminder, acknowledge } = useStartupReminder();
  useAppliedTheme(settings.data?.theme);

  // Startup time for the operation log (once per launch).
  const startupLogged = useRef(false);
  useEffect(() => {
    if (dashboard.data && !startupLogged.current) {
      startupLogged.current = true;
      logInfo(`dashboard shown ${Math.round(performance.now())} ms after the page started loading`);
    }
  }, [dashboard.data]);

  const currentProjectId = route.page === "todos" ? route.projectId : null;
  const openNewTodo = useCallback(() => setNewTodo({ projectId: currentProjectId }), [currentProjectId]);

  // Ctrl+N (new TODO) and Ctrl+1〜4 (screens); Command on macOS.
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.defaultPrevented || e.isComposing || overlayOpen()) return;
      const shortcut = shortcutFor(e);
      if (!shortcut) return;
      e.preventDefault();
      if (shortcut.kind === "new_todo") openNewTodo();
      else setRoute(shortcut.route);
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [openNewTodo, setRoute]);

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

      <main className={route.page === "gantt" ? "app__main app__main--fill" : "app__main"}>
        {projects.error ? <Alert tone="danger">{errorMessage(projects.error)}</Alert> : null}
        {route.page === "dashboard" && <DashboardPage onOpenTodo={setOpenTodoId} onAddTodo={openNewTodo} />}
        {route.page === "todos" && (
          <TodosPage
            projectId={route.projectId}
            selectedTodoId={openTodoId}
            onOpenTodo={setOpenTodoId}
            onEditProject={(s) => setProjectForm(s.project.id)}
            onAddTodo={openNewTodo}
          />
        )}
        {route.page === "gantt" && <GanttPage selectedTodoId={openTodoId} onOpenTodo={setOpenTodoId} />}
        {route.page === "report" && <ReportPage />}
        {route.page === "settings" && <SettingsPage />}
      </main>

      {openTodoId !== null && PANEL_PAGES.includes(route.page) ? (
        <TodoDetailPanel key={openTodoId} todoId={openTodoId} onClose={closeTodo} />
      ) : null}

      {reminder ? <ReminderDialog reminder={reminder} onClose={acknowledge} /> : null}

      <NewTodoDialog
        open={newTodo !== null}
        projectId={newTodo?.projectId ?? null}
        onClose={() => setNewTodo(null)}
        onCreated={(item) => {
          setNewTodo(null);
          // 日報 and 設定 have no detail panel: show the new TODO in the list.
          if (!PANEL_PAGES.includes(route.page)) setRouteState(ALL_TODOS);
          setOpenTodoId(item.todo.id);
        }}
      />
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
