import { useState } from "react";
import { Alert, Button, Dialog, Select, TextField } from "../../components/aizome";
import type { Project, ProjectSummary } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useArchiveProject, useCreateProject, useUpdateProject } from "../../lib/queries";
import { colorOptions, DEFAULT_PROJECT_COLOR } from "./projectText";

interface ProjectFormDialogProps {
  /** null: closed. "new": create. A summary: edit that project. */
  target: null | "new" | ProjectSummary;
  onClose: () => void;
  onCreated: (project: Project) => void;
  onArchived: (project: Project) => void;
  /** Asks to delete (the caller shows the confirmation). */
  onDeleteRequest: (summary: ProjectSummary) => void;
}

/** Create or edit a project (name, colour, description); archive or delete from edit. */
export function ProjectFormDialog(props: ProjectFormDialogProps) {
  if (props.target === null) return null;
  const key = props.target === "new" ? "new" : String(props.target.project.id);
  return <ProjectForm key={key} {...props} target={props.target} />;
}

function ProjectForm({
  target,
  onClose,
  onCreated,
  onArchived,
  onDeleteRequest,
}: ProjectFormDialogProps & { target: "new" | ProjectSummary }) {
  const editing = target === "new" ? null : target;
  const [name, setName] = useState(editing?.project.name ?? "");
  const [color, setColor] = useState(editing?.project.color ?? DEFAULT_PROJECT_COLOR);
  const [description, setDescription] = useState(editing?.project.description ?? "");

  const create = useCreateProject();
  const update = useUpdateProject();
  const archive = useArchiveProject();
  const busy = create.isPending || update.isPending || archive.isPending;
  const error = create.error ?? update.error ?? archive.error;

  function submit() {
    const input = { name, color, description };
    if (editing) {
      update.mutate({ id: editing.project.id, input }, { onSuccess: onClose });
    } else {
      create.mutate(input, { onSuccess: onCreated });
    }
  }

  return (
    <Dialog
      open
      onClose={busy ? undefined : onClose}
      title={editing ? "プロジェクトを編集" : "プロジェクトを追加"}
      footer={
        <>
          {editing ? (
            <div className="dialog-footer__start">
              <Button
                variant="danger"
                disabled={busy}
                onClick={() => onDeleteRequest(editing)}
              >
                削除する
              </Button>
              <Button
                disabled={busy}
                loading={archive.isPending}
                onClick={() =>
                  archive.mutate(
                    { id: editing.project.id, archived: true },
                    { onSuccess: onArchived },
                  )
                }
              >
                アーカイブする
              </Button>
            </div>
          ) : null}
          <Button onClick={onClose} disabled={busy}>
            キャンセル
          </Button>
          <Button
            variant="primary"
            type="submit"
            form="project-form"
            loading={create.isPending || update.isPending}
          >
            {editing ? "保存" : "作成する"}
          </Button>
        </>
      }
    >
      {/* noValidate: the browser's own bubble is not in Japanese; the
          command returns the message to show instead. */}
      <form
        id="project-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        {error ? <Alert tone="danger">{errorMessage(error)}</Alert> : null}
        <TextField
          label="プロジェクト名"
          required
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <Select
          label="色"
          options={colorOptions(color)}
          value={color}
          onChange={(e) => setColor(e.target.value)}
        />
        <TextField
          label="説明"
          multiline
          rows={3}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
        {editing ? (
          <p className="text-caption">
            アーカイブすると、一覧・ダッシュボード・ガントから外れます。設定画面から元に戻せます。
          </p>
        ) : null}
      </form>
    </Dialog>
  );
}
