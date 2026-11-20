import { useState } from "react";
import { TextField } from "../../components/aizome";
import type { TodoItem } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { isComposing } from "../../lib/keys";
import { useCreateTodo } from "../../lib/queries";

/** T-5: type a title and press Enter to add a TODO to this project. */
export function QuickAdd({ projectId, onAdded }: { projectId: number; onAdded?: (item: TodoItem) => void }) {
  const [title, setTitle] = useState("");
  const create = useCreateTodo();

  return (
    <TextField
      label="クイック追加"
      placeholder="タイトルを入力して Enter で追加"
      icon="plus"
      value={title}
      disabled={create.isPending}
      error={create.error ? errorMessage(create.error) : undefined}
      onChange={(e) => {
        setTitle(e.target.value);
        if (create.error) create.reset();
      }}
      onKeyDown={(e) => {
        if (e.key !== "Enter" || isComposing(e)) return;
        e.preventDefault();
        create.mutate(
          {
            project_id: projectId,
            parent_id: null,
            template_id: null,
            title,
            priority: null,
            due_date: null,
            planned_start: null,
            planned_end: null,
            memo_md: null,
          },
          {
            onSuccess: (item) => {
              setTitle("");
              onAdded?.(item);
            },
          },
        );
      }}
    />
  );
}
