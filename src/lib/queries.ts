// TanStack Query wrappers around the generated commands. After a change,
// invalidate every list it can affect.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  type NewTodoInput,
  type ProjectInput,
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
  settings: ["settings"] as const,
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
      // 「間近」の日数 changes the categories.
      return Promise.all([
        qc.invalidateQueries({ queryKey: queryKeys.allProjects }),
        qc.invalidateQueries({ queryKey: queryKeys.todos }),
      ]);
    },
  });
}

// ---- Dashboard -----------------------------------------------------------------

export function useDashboard() {
  return useQuery({ queryKey: queryKeys.dashboard, queryFn: () => commands.getDashboard() });
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
