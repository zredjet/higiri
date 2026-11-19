// Display formats only. Day counts and categories come from Rust (Assessment);
// do not compute them here.

/** "2026-11-19" -> "2026/11/19" (dates in tables). */
export function formatDate(ymd: string): string {
  return ymd.replaceAll("-", "/");
}

/** UTC timestamp -> local date "2026/11/19". */
export function formatTimestampDate(iso: string): string {
  const d = new Date(iso);
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}/${m}/${day}`;
}
