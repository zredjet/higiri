import type { MouseEvent } from "react";
import ReactMarkdown from "react-markdown";
import rehypeSanitize from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import { openUrl } from "@tauri-apps/plugin-opener";
import { cx } from "../aizome/util";
import { isExternalHttpUrl } from "./links";

/**
 * Renders Markdown (GFM: tables, task lists, code blocks). Raw HTML is never
 * rendered, and the result is sanitized. Links never navigate the app's own
 * window: http(s) links open in the OS default browser, others do nothing.
 */
export function MarkdownView({ source, className }: { source: string; className?: string }) {
  function onClick(e: MouseEvent<HTMLDivElement>) {
    const link = (e.target as HTMLElement).closest("a");
    if (!link) return;
    e.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (isExternalHttpUrl(href)) {
      openUrl(href).catch((err: unknown) => console.error(err));
    }
  }

  return (
    // The click handler only intercepts links; keyboard users activate the
    // <a> elements themselves, which fires the same click event.
    <div className={cx("markdown", className)} onClick={onClick}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSanitize]}>
        {source}
      </ReactMarkdown>
    </div>
  );
}
