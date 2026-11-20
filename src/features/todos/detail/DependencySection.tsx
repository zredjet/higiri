import { useState } from "react";
import { Alert, Badge, Button } from "../../../components/aizome";
import type { TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { useDependencies, useRemoveDependency } from "../../../lib/queries";
import {
  ADD_DEPENDENCY,
  DEPENDENTS_LABEL,
  DEPENDS_ON_LABEL,
  NO_DEPENDENTS_TEXT,
  NO_DEPENDS_ON_TEXT,
  removeLabel,
  STARTS_BEFORE_ALERT,
} from "../dependencyText";
import { displayTitle, STATUS_LABEL, STATUS_TONE } from "../todoText";
import { DependencyPickerDialog } from "./DependencyPickerDialog";

const isClosed = (i: TodoItem) => i.todo.status === "done" || i.todo.status === "cancelled";

/**
 * 「先に終わらせるTODO」 and 「これを待っているTODO」 (T-11): each with its
 * status (opens it on a click, × takes the dependency off), and the button
 * that opens the search to add one. `save` stores the panel's edits first,
 * so the plan compared is the one shown.
 */
export function DependencySection({
  item,
  save,
  onOpenTodo,
}: {
  item: TodoItem;
  save: () => Promise<void>;
  onOpenTodo: (id: number) => void;
}) {
  const id = item.todo.id;
  const deps = useDependencies(id);
  const remove = useRemoveDependency();
  const [picking, setPicking] = useState(false);
  const dependsOn = deps.data?.depends_on ?? [];
  const dependents = deps.data?.dependents ?? [];

  async function openPicker() {
    remove.reset();
    await save();
    setPicking(true);
  }

  function list(items: TodoItem[], pair: (other: TodoItem) => { todoId: number; dependsOnId: number }) {
    return (
      <ul className="family__list">
        {items.map((other) => (
          <li key={other.todo.id} className="dependency-row">
            <button type="button" className="family-item" onClick={() => onOpenTodo(other.todo.id)}>
              <span className="dependency-item__text">
                <span className={isClosed(other) ? "family-item__title is-closed" : "family-item__title"}>
                  {displayTitle(other)}
                </span>
                {other.todo.project_id !== item.todo.project_id ? (
                  <span className="text-caption">{other.project_name}</span>
                ) : null}
              </span>
              <span className="family-item__badges">
                <Badge tone={STATUS_TONE[other.todo.status]} dot>
                  {STATUS_LABEL[other.todo.status]}
                </Badge>
              </span>
            </button>
            <Button
              size="sm"
              variant="ghost"
              iconStart="x"
              className="dependency-row__remove"
              aria-label={removeLabel(displayTitle(other))}
              title={removeLabel(displayTitle(other))}
              disabled={remove.isPending}
              onClick={() => {
                remove.reset();
                remove.mutate(pair(other));
              }}
            />
          </li>
        ))}
      </ul>
    );
  }

  return (
    <>
      <section className="family" aria-label={DEPENDS_ON_LABEL}>
        <div className="family__head">
          <span className="family__label">{DEPENDS_ON_LABEL}</span>
        </div>
        {dependsOn.length > 0 ? (
          list(dependsOn, (first) => ({ todoId: id, dependsOnId: first.todo.id }))
        ) : deps.data ? (
          <span className="text-caption">{NO_DEPENDS_ON_TEXT}</span>
        ) : null}
        {item.starts_before_dependency ? <Alert tone="warning">{STARTS_BEFORE_ALERT}</Alert> : null}
        <div>
          <Button size="sm" variant="ghost" iconStart="plus" onClick={() => void openPicker()}>
            {ADD_DEPENDENCY}
          </Button>
        </div>
      </section>

      <section className="family" aria-label={DEPENDENTS_LABEL}>
        <div className="family__head">
          <span className="family__label">{DEPENDENTS_LABEL}</span>
        </div>
        {dependents.length > 0 ? (
          list(dependents, (later) => ({ todoId: later.todo.id, dependsOnId: id }))
        ) : deps.data ? (
          <span className="text-caption">{NO_DEPENDENTS_TEXT}</span>
        ) : null}
        {deps.error ? <Alert tone="danger">{errorMessage(deps.error)}</Alert> : null}
        {remove.error ? <Alert tone="danger">{errorMessage(remove.error)}</Alert> : null}
      </section>
      {picking ? (
        <DependencyPickerDialog
          item={item}
          joined={new Set(dependsOn.map((d) => d.todo.id))}
          onClose={() => setPicking(false)}
        />
      ) : null}
    </>
  );
}
