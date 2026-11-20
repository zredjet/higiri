import { useState } from "react";
import { Alert, Button, Card, Dialog, Table, TextField } from "../../components/aizome";
import type { TemplateSummary } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { useDeleteTodoTemplate, useRenameTodoTemplate, useTodoTemplates } from "../../lib/queries";
import { deleteTemplateText, NO_TEMPLATES, templateChildCount } from "../todos/templateText";

/**
 * TODOのテンプレート (T-10): the templates by name with their number of
 * children, renaming and deleting. A template's contents are changed by
 * saving a TODO made from it again under the same name (the detail panel).
 */
export function TodoTemplateCard() {
  const templates = useTodoTemplates();
  const [renaming, setRenaming] = useState<TemplateSummary | null>(null);
  const [deleting, setDeleting] = useState<TemplateSummary | null>(null);
  const list = templates.data ?? [];

  return (
    <Card
      title="TODOのテンプレート"
      description="子TODOを含むTODOの一式です。「TODOを追加」で選ぶと、タイトルと優先度が入り、メモと子TODOも一緒に作られます。中身を変えるときは、テンプレートから作ったTODOを直し、同じ名前で保存し直してください。"
    >
      {templates.error ? <Alert tone="danger">{errorMessage(templates.error)}</Alert> : null}
      {templates.isSuccess && list.length === 0 ? <p className="text-muted">{NO_TEMPLATES}</p> : null}
      {list.length > 0 ? (
        <Table<TemplateSummary>
          rowKey="id"
          rows={list}
          columns={[
            { key: "name", header: "名前", render: (t) => t.name },
            { key: "children", header: "子TODO", numeric: true, width: 90, render: templateChildCount },
            {
              key: "actions",
              header: "操作",
              align: "right",
              width: 220,
              render: (t) => (
                <div className="table-actions">
                  <Button size="sm" variant="ghost" aria-label={`${t.name}の名前を変える`} onClick={() => setRenaming(t)}>
                    名前を変える
                  </Button>
                  <Button size="sm" variant="ghost" aria-label={`${t.name}を削除する`} onClick={() => setDeleting(t)}>
                    削除する
                  </Button>
                </div>
              ),
            },
          ]}
        />
      ) : null}
      {renaming ? <RenameDialog key={renaming.id} target={renaming} onClose={() => setRenaming(null)} /> : null}
      {deleting ? <DeleteDialog target={deleting} onClose={() => setDeleting(null)} /> : null}
    </Card>
  );
}

function RenameDialog({ target, onClose }: { target: TemplateSummary; onClose: () => void }) {
  const [name, setName] = useState(target.name);
  const rename = useRenameTodoTemplate();

  return (
    <Dialog
      open
      size="sm"
      onClose={rename.isPending ? undefined : onClose}
      title="テンプレートの名前を変える"
      footer={
        <>
          <Button onClick={onClose} disabled={rename.isPending}>
            キャンセル
          </Button>
          <Button variant="primary" type="submit" form="rename-template-form" loading={rename.isPending}>
            変更する
          </Button>
        </>
      }
    >
      <form
        id="rename-template-form"
        className="dialog-form"
        noValidate
        onSubmit={(e) => {
          e.preventDefault();
          rename.mutate({ id: target.id, name }, { onSuccess: onClose });
        }}
      >
        {rename.error ? <Alert tone="danger">{errorMessage(rename.error)}</Alert> : null}
        <TextField
          label="テンプレートの名前"
          required
          autoFocus
          description="40字まで入力できます。"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </form>
    </Dialog>
  );
}

function DeleteDialog({ target, onClose }: { target: TemplateSummary; onClose: () => void }) {
  const del = useDeleteTodoTemplate();

  return (
    <Dialog
      open
      tone="danger"
      size="sm"
      onClose={del.isPending ? undefined : onClose}
      title="テンプレートを削除しますか"
      description={deleteTemplateText(target.name)}
      footer={
        <>
          <Button onClick={onClose} disabled={del.isPending}>
            キャンセル
          </Button>
          <Button variant="danger" loading={del.isPending} onClick={() => del.mutate(target.id, { onSuccess: onClose })}>
            削除する
          </Button>
        </>
      }
    >
      {del.error ? <Alert tone="danger">{errorMessage(del.error)}</Alert> : null}
    </Dialog>
  );
}
