import { useState } from "react";
import { Alert, Button, Dialog, Select, TextField } from "../../components/aizome";
import { DateField } from "../../components/DateField";
import type { Priority, TodoItem } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useCreateTodo, useProjects, useTodoTemplates } from "../../lib/queries";
import { TEMPLATE_DUE_HINT, TEMPLATE_NO_CHILDREN, templateOptions } from "./templateText";
import { PRIORITIES, PRIORITY_LABEL } from "./todoText";

interface NewTodoDialogProps {
  open: boolean;
  /** Preselected project (the project page); null picks the first one. */
  projectId: number | null;
  onClose: () => void;
  onCreated: (item: TodoItem) => void;
}

/**
 * 「TODOを追加」: title, project, priority, due date and planned period. A
 * template (T-10), when there is one, fills in the title and priority and
 * brings its memo and children.
 */
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
  const templates = useTodoTemplates();
  const templateList = templates.data ?? [];
  /** "" = 使わない. */
  const [templateId, setTemplateId] = useState("");
  const template = templateList.find((t) => String(t.id) === templateId) ?? null;
  const create = useCreateTodo();
  const noProject = projects.isSuccess && list.length === 0;
  const projectValue = project || (list[0] ? String(list[0].project.id) : "");

  function submit() {
    create.mutate(
      {
        project_id: Number(projectValue),
        parent_id: null,
        // A template deleted in the meantime is refused (not made without its children).
        template_id: templateId === "" ? null : Number(templateId),
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
        {templateList.length > 0 ? (
          <div className="template-choice">
            <Select
              label="テンプレート"
              options={templateOptions(templateList)}
              value={templateId}
              onChange={(e) => {
                const next = templateList.find((t) => String(t.id) === e.target.value);
                setTemplateId(e.target.value);
                if (next) {
                  setTitle(next.title);
                  setPriority(next.priority);
                }
              }}
            />
            {template ? (
              <div className="template-preview" aria-live="polite">
                <span className="template-preview__label">子TODO</span>
                {template.child_titles.length > 0 ? (
                  <>
                    <ul className="template-preview__list">
                      {template.child_titles.map((title, i) => (
                        <li key={i}>{title}</li>
                      ))}
                    </ul>
                    <span className="text-caption">{TEMPLATE_DUE_HINT}</span>
                  </>
                ) : (
                  <span className="text-caption">{TEMPLATE_NO_CHILDREN}</span>
                )}
              </div>
            ) : null}
          </div>
        ) : null}
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
