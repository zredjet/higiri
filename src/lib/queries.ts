// TanStack Query wrappers around the generated commands. After a change,
// invalidate every list it can affect.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type ProjectInput, type SettingsInput } from "./bindings";

export const queryKeys = {
  projects: (includeArchived: boolean) => ["projects", { includeArchived }] as const,
  allProjects: ["projects"] as const,
  todos: ["todos"] as const,
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
