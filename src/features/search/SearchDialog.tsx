import { Fragment, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Alert, Badge, Dialog, TextField } from "../../components/aizome";
import type { Segment, TodoItem } from "../../lib/bindings";
import { errorMessage } from "../../lib/errors";
import { isComposing, isPrimaryModifier } from "../../lib/keys";
import { useSearch } from "../../lib/queries";
import { STATUS_LABEL, STATUS_TONE, URGENCY_LABEL, URGENCY_TONE } from "../todos/todoText";
import {
  AND_HINT,
  countText,
  EMPTY_QUERY_TEXT,
  matchLabel,
  moveSelection,
  NO_HITS_TEXT,
  OVER_LIMIT_TEXT,
  SEARCH_DELAY_MS,
} from "./searchText";

interface SearchDialogProps {
  open: boolean;
  onClose: () => void;
  /** A result was chosen (Enter or a click). */
  onOpenTodo: (item: TodoItem) => void;
}

/**
 * 検索ダイアログ (T-7). Searches 200ms after the last key, never while a
 * Japanese conversion is open. ↑↓ choose a result and Enter opens it. The
 * typed words stay for the next time (selected, so typing replaces them).
 * Stays mounted while closed to keep them.
 */
export function SearchDialog({ open, onClose, onOpenTodo }: SearchDialogProps) {
  const [text, setText] = useState("");
  const [composing, setComposing] = useState(false);
  /** The words searched: `text` once typing has paused. */
  const [query, setQuery] = useState("");
  const search = useSearch(open ? query : "");
  const hits = query !== "" ? (search.data?.hits ?? []) : [];
  const total = search.data?.total ?? 0;

  const [selection, setSelection] = useState({ query: "", index: 0 });
  const selected = selection.query === query ? Math.min(selection.index, Math.max(hits.length - 1, 0)) : 0;
  const select = (index: number) => setSelection({ query, index });

  const field = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  /** Scroll the selection into view only after ↑↓, not after a hover. */
  const movedByKey = useRef(false);
  const listId = useId();
  const optionId = (todoId: number) => `${listId}-${todoId}`;

  useEffect(() => {
    if (composing) return undefined;
    const next = text.trim();
    const timer = window.setTimeout(() => setQuery(next), SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [text, composing]);

  // After the Dialog has focused its panel (child effects run first). The
  // list starts at the top again, so does the selection.
  useEffect(() => {
    if (!open) return;
    setSelection({ query: "", index: 0 });
    const input = field.current?.querySelector("input");
    input?.focus();
    input?.select();
  }, [open]);

  useEffect(() => {
    if (!movedByKey.current) return;
    movedByKey.current = false;
    list.current?.querySelector<HTMLElement>('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }, [selected, query]);

  function onKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (isComposing(e)) {
      // Esc cancels the conversion; it must not close the dialog.
      if (e.key === "Escape") e.stopPropagation();
      return;
    }
    // Ctrl+F again (App keeps the WebView's own find bar away): select the words.
    if (isPrimaryModifier(e) && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "f") {
      e.currentTarget.select();
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      movedByKey.current = true;
      select(moveSelection(selected, e.key === "ArrowDown" ? 1 : -1, hits.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const words = text.trim();
      // Typed faster than the wait: search now rather than open an old result.
      if (words !== query || search.isPlaceholderData) {
        setQuery(words);
        return;
      }
      const hit = hits[selected];
      if (hit) onOpenTodo(hit.item);
    }
  }

  const activeId = hits[selected] ? optionId(hits[selected].item.todo.id) : undefined;

  return (
    <Dialog open={open} onClose={onClose} title="検索" size="lg" className="search-dialog">
      <div ref={field}>
        <TextField
          icon="search"
          aria-label="探す語"
          placeholder="例: 認証 レビュー"
          description={AND_HINT}
          autoComplete="off"
          spellCheck={false}
          role="combobox"
          aria-expanded={hits.length > 0}
          aria-controls={listId}
          aria-activedescendant={activeId}
          aria-autocomplete="list"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onCompositionStart={() => setComposing(true)}
          onCompositionEnd={() => setComposing(false)}
          onKeyDown={onKeyDown}
        />
      </div>

      <div className="search-dialog__status" aria-live="polite">
        {query === "" ? (
          <p className="text-muted">{EMPTY_QUERY_TEXT}</p>
        ) : search.error ? null : search.data ? (
          total === 0 ? (
            <p className="text-muted">{NO_HITS_TEXT}</p>
          ) : (
            <span className="text-caption">{countText(hits.length, total)}</span>
          )
        ) : null}
      </div>
      {query !== "" && search.error ? <Alert tone="danger">{errorMessage(search.error)}</Alert> : null}
      {query !== "" && total > hits.length && hits.length > 0 ? <Alert tone="info">{OVER_LIMIT_TEXT}</Alert> : null}

      <ul ref={list} id={listId} role="listbox" aria-label="検索の結果" className="search-results">
        {hits.map((hit, i) => {
          const { todo, assessment, project_name } = hit.item;
          return (
            <li
              key={todo.id}
              id={optionId(todo.id)}
              role="option"
              aria-selected={i === selected}
              className={i === selected ? "search-hit is-selected" : "search-hit"}
              onMouseMove={() => {
                if (i !== selected) select(i);
              }}
              onClick={() => onOpenTodo(hit.item)}
            >
              <div className="search-hit__head">
                <span className="search-hit__title">
                  <Marked segments={hit.title} />
                </span>
                <span className="search-hit__badges">
                  {assessment.urgency ? (
                    <Badge tone={URGENCY_TONE[assessment.urgency]} dot>
                      {URGENCY_LABEL[assessment.urgency]}
                    </Badge>
                  ) : null}
                  <Badge tone={STATUS_TONE[todo.status]} dot>
                    {STATUS_LABEL[todo.status]}
                  </Badge>
                </span>
              </div>
              <span className="text-caption">{project_name}</span>
              {hit.matches.map((m, j) => (
                <div key={j} className="search-hit__match">
                  <span className="search-hit__field">{matchLabel(m)}</span>
                  <span className="search-hit__text">
                    <Marked segments={m.segments} />
                  </span>
                </div>
              ))}
            </li>
          );
        })}
      </ul>
    </Dialog>
  );
}

/** Text with the searched words in bold on a tinted ground. */
function Marked({ segments }: { segments: Segment[] }) {
  return (
    <>
      {segments.map((s, i) =>
        s.hit ? (
          <mark key={i} className="search-mark">
            {s.text}
          </mark>
        ) : (
          <Fragment key={i}>{s.text}</Fragment>
        ),
      )}
    </>
  );
}
