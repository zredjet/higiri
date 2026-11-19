import { useState } from "react";
import { Alert, Button, Dialog, Select, TextField } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import type { Priority, TodoItem } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useCreateTodo, useProjects } from "../../lib/queries";
import { PRIORITIES, PRIORITY_LABEL } from "./todoText";

interface NewTodoDialogProps {
  open: boolean;
  /** Preselected project (the project page); null picks the first one. */
  projectId: number | null;
  onClose: () => void;
  onCreated: (item: TodoItem) => void;
}

/** 「TODOを追加」: title, project, priority, due date and planned period. */
export function NewTodoDialog(props: NewTodoDialogProps) {
  if (!props.open) return null;
  return <NewTodoForm {...props} />;
}

function NewTodoForm({ projectId, onClose, onCreated }: NewTodoDialogProps) {
  const projects = useProjects(false);
  const list = projects.data ?? [];
  const [title, setTitle] = useState("");
  const [project, setProject] = useState<string>(
    projectId !== null ? String(projectId) : list[0] ? String(list[0].project.id) : "",
  );
  const [priority, setPriority] = useState<Priority>("medium");
  const [due, setDue] = useState<string | null>(null);
  const [plannedStart, setPlannedStart] = useState<string | null>(null);
  const [plannedEnd, setPlannedEnd] = useState<string | null>(null);
  const create = useCreateTodo();
  const noProject = projects.isSuccess && list.length === 0;
  const projectValue = project || (list[0] ? String(list[0].project.id) : "");

  function submit() {
    create.mutate(
      {
        project_id: Number(projectValue),
        title,
        priority,
        due_date: due,
        planned_start: plannedStart,
        planned_end: plannedEnd,
        memo_md: null,
      },
      { onSuccess: onCreated },
    );
  }

  return (
    <Dialog
      open
      onClose={create.isPending ? undefined : onClose}
      title="TODOを追加"
      footer={
        <>
          <Button onClick={onClose} disabled={create.isPending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="new-todo-form" loading={create.isPending} disabled={noProject}>
            追加する
          </Button>
        </>
      }
    >
      <form
        id="new-todo-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {noProject ? (
          <Alert tone="info">
            プロジェクトがありません。サイドナビの「プロジェクトを追加」から作成してください。
          </Alert>
        ) : null}
        {create.error ? <Alert tone="danger">{errorMessage(create.error)}</Alert> : null}
        <TextField label="タイトル" required autoFocus value={title} onChange={(e) => setTitle(e.target.value)} />
        <div className="field-grid">
          <Select
            label="プロジェクト"
            options={list.map((p) => ({ value: String(p.project.id), label: p.project.name }))}
            value={projectValue}
            onChange={(e) => setProject(e.target.value)}
          />
          <Select
            label="優先度"
            options={PRIORITIES.map((p) => ({ value: p, label: PRIORITY_LABEL[p] }))}
            value={priority}
            onChange={(e) => setPriority(e.target.value as Priority)}
          />
        </div>
        <DateField label="日限" value={due} onChange={setDue} />
        <div className="field-grid">
          <DateField label="予定開始" value={plannedStart} onChange={setPlannedStart} />
          <DateField label="予定終了" value={plannedEnd} onChange={setPlannedEnd} />
        </div>
      </form>
    </Dialog>
  );
}
