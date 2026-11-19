import { useEffect, useRef } from "react";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, placeholder as placeholderExt } from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { cx } from "../aizome/util";

/** Markdown colours from the Aizome tokens (no italics, per Aizome). */
const highlight = HighlightStyle.define([
  { tag: tags.heading, fontWeight: "600", color: "var(--primary-text)" },
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontWeight: "600" },
  { tag: [tags.link, tags.url], color: "var(--link)" },
  { tag: tags.monospace, fontFamily: "var(--font-mono)" },
  { tag: [tags.processingInstruction, tags.list, tags.quote, tags.contentSeparator], color: "var(--text-muted)" },
]);

/** Text metrics per font: notes and memos use the sans font; the report editor is monospaced (mock: 13px/20px). */
const FONTS = {
  sans: { family: "var(--font-sans)", size: "14px", lineHeight: 22 },
  mono: { family: "var(--font-mono)", size: "13px", lineHeight: 20 },
} as const;

function theme(font: keyof typeof FONTS, minLines: number) {
  const f = FONTS[font];
  return EditorView.theme({
    "&": { backgroundColor: "transparent", color: "var(--text)", fontSize: f.size, width: "100%" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": { fontFamily: f.family, lineHeight: `${f.lineHeight}px` },
    ".cm-content": { padding: "0", caretColor: "var(--text)", minHeight: `${minLines * f.lineHeight}px` },
    ".cm-line": { padding: "0" },
    ".cm-placeholder": { color: "var(--text-subtle)" },
    ".cm-cursor": { borderLeftColor: "var(--text)" },
    "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": {
      backgroundColor: "var(--primary-soft)",
    },
  });
}

export interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Accessible name of the editing area. */
  label: string;
  placeholder?: string;
  /** Ctrl+Enter (Cmd+Enter on macOS). */
  onSubmit?: () => void;
  /** Ctrl+E (Cmd+E on macOS). */
  onTogglePreview?: () => void;
  /** Minimum height of the text area in lines. Default 4. */
  minLines?: number;
  /** "mono" for text edited as Markdown source (the report). Default "sans". */
  font?: keyof typeof FONTS;
  /** false drops Aizome's input frame (the editor fills a card instead). Default true. */
  framed?: boolean;
  /** false shows the source as plain text, without Markdown colouring. Default true. */
  highlightSyntax?: boolean;
  autoFocus?: boolean;
  className?: string;
}

/**
 * CodeMirror 6 in Aizome's input frame. Controlled: `value` replaces the
 * document when it differs from what the editor holds.
 */
export function MarkdownEditor({
  value,
  onChange,
  label,
  placeholder,
  onSubmit,
  onTogglePreview,
  minLines = 4,
  font = "sans",
  framed = true,
  highlightSyntax = true,
  autoFocus,
  className,
}: MarkdownEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  // Latest callbacks, so the editor is created once.
  const handlers = useRef({ onChange, onSubmit, onTogglePreview });
  handlers.current = { onChange, onSubmit, onTogglePreview };

  useEffect(() => {
    if (!host.current) return undefined;
    const v = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([
            {
              key: "Mod-Enter",
              run: () => {
                handlers.current.onSubmit?.();
                return handlers.current.onSubmit !== undefined;
              },
            },
            {
              key: "Mod-e",
              run: () => {
                handlers.current.onTogglePreview?.();
                return handlers.current.onTogglePreview !== undefined;
              },
            },
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          markdown(),
          highlightSyntax ? syntaxHighlighting(highlight) : [],
          EditorView.lineWrapping,
          theme(font, minLines),
          placeholder ? placeholderExt(placeholder) : [],
          EditorView.contentAttributes.of({ "aria-label": label, "aria-multiline": "true" }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) handlers.current.onChange(u.state.doc.toString());
          }),
        ],
      }),
    });
    view.current = v;
    if (autoFocus) v.focus();
    return () => {
      v.destroy();
      view.current = null;
    };
    // Created once; later `value` changes are synced below.
  }, []);

  useEffect(() => {
    const v = view.current;
    if (v && v.state.doc.toString() !== value) {
      v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: value } });
    }
  }, [value]);

  return (
    <div
      className={cx(framed && "az-input", framed && "az-input--multiline", "md-editor", className)}
      onClick={() => view.current?.focus()}
    >
      <div ref={host} className="md-editor__host" />
    </div>
  );
}
