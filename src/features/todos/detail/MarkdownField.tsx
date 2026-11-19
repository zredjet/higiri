import { useId, type KeyboardEvent, type ReactNode } from "react";
import { Button } from "../../../components/aizome";
import { MarkdownEditor } from "../../../components/markdown/MarkdownEditor";
import { MarkdownView } from "../../../components/markdown/MarkdownView";

interface MarkdownFieldProps {
  label: ReactNode;
  value: string;
  onChange: (value: string) => void;
  preview: boolean;
  onTogglePreview: () => void;
  description: ReactNode;
  onSubmit?: () => void;
  placeholder?: string;
  minLines?: number;
  /** Shown instead of an empty preview. */
  emptyPreview: string;
}

function isModE(e: KeyboardEvent) {
  return (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "e";
}

/** A labelled Markdown editor with an edit / preview switch (M-3, Ctrl+E). */
export function MarkdownField({
  label,
  value,
  onChange,
  preview,
  onTogglePreview,
  description,
  onSubmit,
  placeholder,
  minLines,
  emptyPreview,
}: MarkdownFieldProps) {
  const labelId = useId();
  return (
    <div className="az-field">
      <div className="md-field__head">
        <span id={labelId} className="az-field__label">
          {label}
        </span>
        <Button variant="ghost" size="sm" onClick={onTogglePreview} aria-pressed={preview}>
          {preview ? "編集に戻る" : "プレビュー"}
        </Button>
      </div>
      {preview ? (
        <div
          className="md-preview"
          tabIndex={0}
          role="region"
          aria-labelledby={labelId}
          onKeyDown={(e) => {
            if (isModE(e)) {
              e.preventDefault();
              onTogglePreview();
            }
          }}
        >
          {value.trim() ? <MarkdownView source={value} /> : <p className="text-muted">{emptyPreview}</p>}
        </div>
      ) : (
        <MarkdownEditor
          value={value}
          onChange={onChange}
          label={typeof label === "string" ? label : "Markdown"}
          placeholder={placeholder}
          onSubmit={onSubmit}
          onTogglePreview={onTogglePreview}
          minLines={minLines}
        />
      )}
      <p className="az-field__desc">{description}</p>
    </div>
  );
}
