// TanStack Query wrappers around the generated commands. After a change,
// invalidate every list it can affect.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type BulkChange,
  type GanttQuery,
  type NewTodoInput,
  type ProjectInput,
  type RepeatInput,
  type ReportKey,
  type Settings,
  type SettingsInput,
  type StatusChange,
  type Todo,
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
  // Under "todos": a status change of either TODO shows in the lists.
  dependencies: (id: number) => ["todos", "dependencies", id] as const,
  settings: ["settings"] as const,
  holidays: (start: string, end: string) => ["holidays", start, end] as const,
  allHolidays: ["holidays"] as const,
  report: (key: ReportKey) => ["reports", key] as const,
  backups: ["backups"] as const,
  templates: ["templates"] as const,
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

/** The input of update_settings that keeps every setting as it is. */
export function settingsInputOf(s: Settings): SettingsInput {
  return {
    reminder_frequency: s.reminder_frequency,
    soon_days: s.soon_days,
    follow_up_days: s.follow_up_days,
    theme: s.theme,
    notify_enabled: s.notify_enabled,
    notify_times: s.notify_times,
    close_to_tray: s.close_to_tray,
    autostart: s.autostart,
    day_count: s.day_count,
    custom_holidays: s.custom_holidays,
    report_template: s.report_template,
    weekly_report_template: s.weekly_report_template,
    zoom: s.zoom,
  };
}

/** Settings to change; a function gets the settings as saved right now. */
export type SettingsChange = Partial<SettingsInput> | ((current: Settings) => Partial<SettingsInput>);

const SETTINGS_WRITES = { id: "settings-writes" };

/**
 * Saves some settings. update_settings takes every setting, so the others are
 * read with get_settings just before the update, and the saves run one at a
 * time in the order they were made: a heading saved when its field loses
 * focus and a switch clicked right after never write back each other's older
 * values.
 */
export function useUpdateSettings() {
  const qc = useQueryClient();
  return useMutation({
    scope: SETTINGS_WRITES,
    mutationFn: async (change: SettingsChange) => {
      const current = await commands.getSettings();
      const edits = typeof change === "function" ? change(current) : change;
      return commands.updateSettings({ ...settingsInputOf(current), ...edits });
    },
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

/** Reads a TODO as last saved here (the cache), e.g. right after a save. */
export function useLatestTodo() {
  const qc = useQueryClient();
  return (id: number) => qc.getQueryData<TodoItem>(queryKeys.todo(id));
}

export function useCreateTodo() {
  const changed = useTodoChanged();
  return useMutation({
    mutationFn: (input: NewTodoInput) => commands.createTodo(input),
    onSuccess: (item) => changed(item),
  });
}

/**
 * update_todo and change_status run one at a time, in the order they were
 * started, so no other write lands between an update's read and its write.
 */
const TODO_WRITES = { id: "todo-writes" };

/**
 * Saves a TODO's fields. update_todo takes every field, so `change` gets the
 * TODO as saved right now (read with get_todo just before the update) and
 * returns it with only the caller's edits: a Gantt drag changes the planned
 * span, the detail panel its fields edited by hand. Neither writes back older
 * values of what the other has just saved.
 */
export function useUpdateTodo() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: async ({ id, change }: { id: number; change: (current: Todo) => TodoFields }) => {
      const current = await commands.getTodo(id);
      return commands.updateTodo(id, change(current.todo));
    },
    onSuccess: (item) => changed(item),
  });
}

export function useChangeStatus() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ id, change }: { id: number; change: StatusChange }) => commands.changeStatus(id, change),
    onSuccess: (item) => changed(item),
  });
}

/** Puts a TODO under a parent, moves it, or (`parentId: null`) takes it out (S-1). */
export function useSetTodoParent() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ id, parentId }: { id: number; parentId: number | null }) => commands.setTodoParent(id, parentId),
    onSuccess: (item) => changed(item),
  });
}

/** Gives a TODO a repetition, changes it, or (`repeat: null`) takes it off (T-12). */
export function useSetTodoRepeat() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ id, repeat }: { id: number; repeat: RepeatInput | null }) => commands.setTodoRepeat(id, repeat),
    onSuccess: (item) => changed(item),
  });
}

// ---- 依存関係 (T-11) ---------------------------------------------------------------

/** 先行TODO and 後続TODO of one TODO (the detail panel). */
export function useDependencies(id: number) {
  return useQuery({ queryKey: queryKeys.dependencies(id), queryFn: () => commands.listDependencies(id) });
}

/** Makes `todoId` wait for `dependsOnId`. Rust refuses itself, twins, circles and a parent with its child. */
export function useAddDependency() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ todoId, dependsOnId }: { todoId: number; dependsOnId: number }) =>
      commands.addDependency(todoId, dependsOnId),
    onSuccess: (item) => changed(item),
  });
}

export function useRemoveDependency() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ todoId, dependsOnId }: { todoId: number; dependsOnId: number }) =>
      commands.removeDependency(todoId, dependsOnId),
    onSuccess: (item) => changed(item),
  });
}

// ---- 後続TODOのずらし (G-8) ----------------------------------------------------------

/**
 * The 後続TODO a Gantt drag would move (`[]`: nothing to ask). Read in the
 * TODO_WRITES line: it sees the saves started before it (the detail panel's
 * typed dates, an earlier drag) and writes nothing.
 */
export function usePreviewDependentShift() {
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ todoId, start, end }: { todoId: number; start: string; end: string }) =>
      commands.previewDependentShift(todoId, start, end),
  });
}

/**
 * 「後続もずらす」: the dragged TODO's 予定期間 and its 後続TODO's, worked out
 * again by Rust, in one transaction. Resolves after the lists are fetched again.
 */
export function useShiftWithDependents() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ todoId, start, end }: { todoId: number; start: string; end: string }) =>
      commands.shiftWithDependents(todoId, start, end),
    onSuccess: () => changed(),
  });
}

// ---- 一括変更 (T-14) ------------------------------------------------------------------

/**
 * What a bulk change would do to the selected TODOs (`change: null`: the
 * input is not typed yet, nothing is asked). Under "todos": a TODO changed
 * meanwhile shows. The previous result stays while the next one loads.
 */
export function usePreviewBulkChange(ids: number[], change: BulkChange | null) {
  return useQuery({
    queryKey: ["todos", "bulk-preview", ids, change] as const,
    queryFn: () => commands.previewBulkChange(ids, change as BulkChange),
    enabled: change !== null && ids.length > 0,
    placeholderData: (previous) => previous,
  });
}

/**
 * Gives the change to the selected TODOs, worked out again by Rust, in one
 * transaction. In the TODO_WRITES line. Resolves after the lists are fetched
 * again.
 */
export function useApplyBulkChange() {
  const changed = useTodoChanged();
  return useMutation({
    scope: TODO_WRITES,
    mutationFn: ({ ids, change }: { ids: number[]; change: BulkChange }) => commands.applyBulkChange(ids, change),
    onSuccess: () => changed(),
  });
}

// ---- TODOテンプレート (T-10) ------------------------------------------------------

/** Every template, by name. A TODO is made from one with create_todo (`template_id`). */
export function useTodoTemplates() {
  return useQuery({ queryKey: queryKeys.templates, queryFn: () => commands.listTodoTemplates() });
}

/** Refreshes the templates after a change (they belong to no project or TODO list). */
function useTemplatesChanged() {
  const qc = useQueryClient();
  return () => qc.invalidateQueries({ queryKey: queryKeys.templates });
}

/**
 * 「テンプレートとして保存」: a TODO without a parent and its children. A template with
 * the same name is replaced only with `replace` (the dialog asks first).
 */
export function useSaveTodoTemplate() {
  const changed = useTemplatesChanged();
  return useMutation({
    mutationFn: ({ todoId, name, replace }: { todoId: number; name: string; replace: boolean }) =>
      commands.saveTodoTemplate(todoId, name, replace),
    onSuccess: () => changed(),
  });
}

export function useRenameTodoTemplate() {
  const changed = useTemplatesChanged();
  return useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) => commands.renameTodoTemplate(id, name),
    onSuccess: () => changed(),
  });
}

export function useDeleteTodoTemplate() {
  const changed = useTemplatesChanged();
  return useMutation({
    mutationFn: (id: number) => commands.deleteTodoTemplate(id),
    onSuccess: () => changed(),
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

/**
 * 「ファイルを選んで復元」: opens the file dialog (Rust side) and restores the chosen backup, then reloads
 * everything on screen. Resolves to the file name, or null when cancelled (nothing changed).
 */
export function useRestoreBackupFile() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => commands.restoreBackupFile(),
    onSuccess: (fileName) => (fileName === null ? undefined : qc.invalidateQueries()),
  });
}

/** The settings and the dashboard's notice change with the backup mirror. */
function useInvalidateBackupMirror() {
  const qc = useQueryClient();
  return () =>
    Promise.all([
      qc.invalidateQueries({ queryKey: queryKeys.settings }),
      qc.invalidateQueries({ queryKey: queryKeys.dashboard }),
    ]);
}

/**
 * 「複製先を選ぶ」「複製先を変える」: opens the folder dialog (Rust side), copies the daily backups there and
 * saves it. Resolves to the mirror, or null when cancelled (nothing changed).
 */
export function useChooseBackupMirror() {
  const invalidate = useInvalidateBackupMirror();
  return useMutation({
    mutationFn: () => commands.chooseBackupMirror(),
    onSuccess: (mirror) => (mirror === null ? undefined : invalidate()),
  });
}

/** 「複製をやめる」: no mirror from now on (its files stay). */
export function useClearBackupMirror() {
  const invalidate = useInvalidateBackupMirror();
  return useMutation({ mutationFn: () => commands.clearBackupMirror(), onSuccess: invalidate });
}

/** 「JSONで書き出す」: opens the save dialog (Rust side) and writes all data. Resolves to the path, or null when cancelled. */
export function useExportData() {
  return useMutation({ mutationFn: () => commands.exportData() });
}

/**
 * 「JSONから取り込む」: opens the file dialog (Rust side) and replaces all data, then reloads
 * everything on screen. Resolves to the file name, or null when cancelled (nothing changed).
 */
export function useImportData() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => commands.importData(),
    onSuccess: (fileName) => (fileName === null ? undefined : qc.invalidateQueries()),
  });
}

export function useOpenDataFolder() {
  return useMutation({ mutationFn: () => commands.openDataFolder() });
}

/** 「通知を試す」: shows a test OS notification (even with notifications turned off). */
export function useTestNotification() {
  return useMutation({ mutationFn: () => commands.testNotification() });
}
