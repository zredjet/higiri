import { useEffect, useState } from "react";
import { Alert, Badge, Button, Dialog, TextField } from "../../../components/aizome";
import type { TodoItem } from "../../../lib/bindings";
import { errorMessage } from "../../../lib/errors";
import { isComposing } from "../../../lib/keys";
import { useAddDependency, useSearch } from "../../../lib/queries";
import { AND_HINT, countText, EMPTY_QUERY_TEXT, NO_HITS_TEXT, SEARCH_DELAY_MS } from "../../search/searchText";
import { ADD_DEPENDENCY, PICKER_DESCRIPTION, pickBlockedReason } from "../dependencyText";
import { displayTitle, STATUS_LABEL, STATUS_TONE } from "../todoText";

/**
 * 「先に終わらせるTODOを足す」 (T-11): finds TODOs with search_todos (200ms
 * after the last key, not while a Japanese conversion is open) and joins the
 * one chosen. The TODO itself and those already joined cannot be chosen;
 * Rust refuses the rest it does not allow (a circle, a parent and its own
 * child), and its message is shown at the top.
 */
export function DependencyPickerDialog({
  item,
  joined,
  onClose,
}: {
  item: TodoItem;
  /** Ids already among 先に終わらせるTODO. */
  joined: ReadonlySet<number>;
  onClose: () => void;
}) {
  const [text, setText] = useState("");
  const [composing, setComposing] = useState(false);
  const [query, setQuery] = useState("");
  const search = useSearch(query);
  const add = useAddDependency();
  const hits = query !== "" ? (search.data?.hits ?? []) : [];
  const total = search.data?.total ?? 0;

  useEffect(() => {
    if (composing) return undefined;
    const next = text.trim();
    const timer = window.setTimeout(() => setQuery(next), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [text, composing]);

  function pick(first: TodoItem) {
    add.mutate({ todoId: item.todo.id, dependsOnId: first.todo.id }, { onSuccess: onClose });
  }

  return (
    <Dialog
      open
      size="md"
      onClose={add.isPending ? undefined : onClose}
      title={ADD_DEPENDENCY}
      description={PICKER_DESCRIPTION}
      footer={
        <Button onClick={onClose} disabled={add.isPending}>
          キャンセル
        </Button>
      }
    >
      <div className="dialog-form">
        {add.error ? <Alert tone="danger">{errorMessage(add.error)}</Alert> : null}
        <TextField
          icon="search"
          aria-label="探す語"
          placeholder="例: 要件 確定"
          description={AND_HINT}
          autoFocus
          autoComplete="off"
          spellCheck={false}
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            if (add.error) add.reset();
          }}
          onCompositionStart={() => setComposing(true)}
          onCompositionEnd={() => setComposing(false)}
          onKeyDown={(e) => {
            if (isComposing(e)) {
              // Esc cancels the conversion; it must not close the dialog.
              if (e.key === "Escape") e.stopPropagation();
              return;
            }
            if (e.key === "Enter") {
              e.preventDefault();
              setQuery(text.trim());
            }
          }}
        />
        <div aria-live="polite">
          {query === "" ? (
            <p className="text-muted">{EMPTY_QUERY_TEXT}</p>
          ) : search.error ? (
            <Alert tone="danger">{errorMessage(search.error)}</Alert>
          ) : search.data ? (
            total === 0 ? (
              <p className="text-muted">{NO_HITS_TEXT}</p>
            ) : (
              <span className="text-caption">{countText(hits.length, total)}</span>
            )
          ) : null}
        </div>
        {hits.length > 0 ? (
          <ul className="family__list dependency-picker" aria-label="検索の結果">
            {hits.map(({ item: hit }) => {
              const reason = pickBlockedReason(hit.todo.id, item.todo.id, joined);
              return (
                <li key={hit.todo.id}>
                  <button
                    type="button"
                    className="family-item"
                    disabled={reason !== null || add.isPending}
                    onClick={() => pick(hit)}
                  >
                    <span className="dependency-item__text">
                      <span className="family-item__title">{displayTitle(hit)}</span>
                      <span className="text-caption">{hit.project_name}</span>
                    </span>
                    <span className="family-item__badges">
                      {reason ? <span className="text-caption">{reason}</span> : null}
                      <Badge tone={STATUS_TONE[hit.todo.status]} dot>
                        {STATUS_LABEL[hit.todo.status]}
                      </Badge>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        ) : null}
      </div>
    </Dialog>
  );
}
