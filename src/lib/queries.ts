// TanStack Query wrappers around the generated commands. After a change,
// invalidate every list it can affect.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type GanttQuery,
  type NewTodoInput,
  type ProjectInput,
  type ReportKey,
  type SettingsInput,
  type StatusChange,
  type TodoFields,
  type TodoItem,
  type TodoQuery,
  type WorkLogInput,
} from "./bindings";

export const queryKeys = {
  projects: (includeArchived: boolean) => ["projects", { includeArchived }] as const,
  allProjects: ["projects"] as const,
  todos: ["todos"] as const,
  todoList: (query: TodoQuery) => ["todos", "list", query] as const,
  todo: (id: number) => ["todos", "item", id] as const,
  timeline: (id: number) => ["todos", "timeline", id] as const,
  // Under "todos" so every TODO change refreshes it too.
  dashboard: ["todos", "dashboard"] as const,
  gantt: (query: GanttQuery) => ["todos", "gantt", query] as const,
  // Under "todos" too: a changed TODO or work log refreshes the results.
  search: (text: string) => ["todos", "search", text] as const,
  settings: ["settings"] as const,
  holidays: (start: string, end: string) => ["holidays", start, end] as const,
  allHolidays: ["holidays"] as const,
  report: (key: ReportKey) => ["reports", key] as const,
  backups: ["backups"] as const,
};

export function useProjects(includeArchived = false) {
  return useQuery({
    queryKey: queryKeys.projects(includeArchived),
    queryFn: () => commands.listProjects(includeArchived),
  });
}

export function useSettings() {
  return useQuery({ queryKey: queryKeys.settings, queryFn: () => commands.getSettings() });
}

/** Invalidates projects and TODOs (counts and badges move together). */
function useInvalidateProjectData() {
  const qc = useQueryClient();
  return () =>
    Promise.all([
      qc.invalidateQueries({ queryKey: queryKeys.allProjects }),
      qc.invalidateQueries({ queryKey: queryKeys.todos }),
    ]);
}

export function useCreateProject() {
  const invalidate = useInvalidateProjectData();
  return useMutation({
    mutationFn: (input: ProjectInput) => commands.createProject(input),
    onSuccess: invalidate,
  });
}

export function useUpdateProject() {
  const invalidate = useInvalidateProjectData();
  return useMutation({
    mutationFn: ({ id, input }: { id: number; input: ProjectInput }) => commands.updateProject(id, input),
    onSuccess: invalidate,
  });
}

export function useArchiveProject() {
  const invalidate = useInvalidateProjectData();
  return useMutation({
    mutationFn: ({ id, archived }: { id: number; archived: boolean }) =>
      commands.archiveProject(id, archived),
    onSuccess: invalidate,
  });
}

export function useDeleteProject() {
  const invalidate = useInvalidateProjectData();
  return useMutation({
    mutationFn: (id: number) => commands.deleteProject(id),
    onSuccess: invalidate,
  });
}

export function useUpdateSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: SettingsInput) => commands.updateSettings(input),
    onSuccess: (saved) => {
      qc.setQueryData(queryKeys.settings, saved);
      // 「間近」の日数, 日数の数え方 and the custom holidays change the
      // categories, the day counts and the holidays.
      return Promise.all([
        qc.invalidateQueries({ queryKey: queryKeys.allProjects }),
        qc.invalidateQueries({ queryKey: queryKeys.todos }),
        qc.invalidateQueries({ queryKey: queryKeys.allHolidays }),
      ]);
    },
  });
}

/**
 * National and custom holidays from `start` to `end` (at most 3 years), with
 * their names. `null` fetches nothing.
 */
export function useHolidays(period: { start: string; end: string } | null) {
  return useQuery({
    queryKey: queryKeys.holidays(period?.start ?? "", period?.end ?? ""),
    queryFn: () => commands.listHolidays(period?.start ?? "", period?.end ?? ""),
    enabled: period !== null,
    placeholderData: (previous) => previous,
  });
}

// ---- Dashboard -----------------------------------------------------------------

export function useDashboard() {
  return useQuery({ queryKey: queryKeys.dashboard, queryFn: () => commands.getDashboard() });
}

export function useGantt(query: GanttQuery) {
  return useQuery({
    queryKey: queryKeys.gantt(query),
    queryFn: () => commands.getGantt(query),
    placeholderData: (previous) => previous,
  });
}

// ---- TODOs -------------------------------------------------------------------

export function useTodoList(query: TodoQuery) {
  return useQuery({
    queryKey: queryKeys.todoList(query),
    queryFn: () => commands.listTodos(query),
    placeholderData: (previous) => previous,
  });
}

export function useTodo(id: number) {
  return useQuery({ queryKey: queryKeys.todo(id), queryFn: () => commands.getTodo(id) });
}

export function useTimeline(id: number) {
  return useQuery({ queryKey: queryKeys.timeline(id), queryFn: () => commands.listTimeline(id) });
}

/**
 * 全文検索 (search_todos). `""` fetches nothing (the dialog is closed or no
 * word has been typed). The previous results stay while the next ones load.
 */
export function useSearch(text: string) {
  return useQuery({
    queryKey: queryKeys.search(text),
    queryFn: () => commands.searchTodos(text),
    enabled: text !== "",
    placeholderData: (previous) => previous,
  });
}

/** After a TODO changes: keep the returned item, refresh lists and counts. */
function useTodoChanged() {
  const qc = useQueryClient();
  return (item?: TodoItem) => {
    if (item) qc.setQueryData(queryKeys.todo(item.todo.id), item);
    return Promise.all([
      qc.invalidateQueries({ queryKey: queryKeys.todos }),
      qc.invalidateQueries({ queryKey: queryKeys.allProjects }),
    ]);
  };
}

export function useCreateTodo() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: (input: NewTodoInput) => commands.createTodo(input),
    onSuccess: (item) => changed(item),
  });
}

export function useUpdateTodo() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: ({ id, fields }: { id: number; fields: TodoFields }) => commands.updateTodo(id, fields),
    onSuccess: (item) => changed(item),
  });
}

export function useChangeStatus() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: ({ id, change }: { id: number; change: StatusChange }) => commands.changeStatus(id, change),
    onSuccess: (item) => changed(item),
  });
}

export function useDeleteTodo() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: (id: number) => commands.deleteTodo(id),
    onSuccess: () => changed(),
  });
}

export function useSaveWorkLog() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: (input: WorkLogInput) => commands.saveWorkLog(input),
    onSuccess: () => changed(),
  });
}

export function useDeleteWorkLog() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: (id: number) => commands.deleteWorkLog(id),
    onSuccess: () => changed(),
  });
}

/**
 * The saved report of the day, or a freshly generated one when none is saved
 * (R-4). Opening a day does not save anything. Loaded once per visit: the
 * screen keeps its own copy while editing, so later refetches never replace it.
 */
export function useReportSource(key: ReportKey) {
  return useQuery({
    queryKey: queryKeys.report(key),
    queryFn: async (): Promise<string> => {
      const saved = await commands.getReport(key);
      return saved ? saved.body_md : commands.generateReport(key, null);
    },
    staleTime: Infinity,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
}

/** Rebuilds the report from the day's records, keeping the 所感・連絡事項 of `currentMd`. */
export function useGenerateReport() {
  return useMutation({
    mutationFn: ({ key, currentMd }: { key: ReportKey; currentMd: string }) =>
      commands.generateReport(key, currentMd),
  });
}

export function useSaveReport() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ key, bodyMd }: { key: ReportKey; bodyMd: string }) => commands.saveReport(key, bodyMd),
    // Keep a cached copy of that day in step with what was saved.
    onSuccess: (saved, { key }) => qc.setQueryData(queryKeys.report(key), saved.body_md),
  });
}

/** Opens the save dialog (Rust side) and writes the .md file. Resolves to the path, or null when cancelled. */
export function useExportReport() {
  return useMutation({
    mutationFn: ({ key, bodyMd }: { key: ReportKey; bodyMd: string }) =>
      commands.exportReportFile(key, bodyMd),
  });
}

/** Backups in the data folder, newest first (read again each time the dialog opens). */
export function useBackups() {
  return useQuery({ queryKey: queryKeys.backups, queryFn: () => commands.listBackups(), gcTime: 0 });
}

/** Replaces all data with a backup, then reloads everything on screen. */
export function useRestoreBackup() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (fileName: string) => commands.restoreBackup(fileName),
    onSuccess: () => qc.invalidateQueries(),
  });
}

export function useOpenDataFolder() {
  return useMutation({ mutationFn: () => commands.openDataFolder() });
}

/** 「通知を試す」: shows a test OS notification (even with notifications turned off). */
export function useTestNotification() {
  return useMutation({ mutationFn: () => commands.testNotification() });
}
