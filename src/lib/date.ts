// Display formats and form defaults only. Categories and day counts come from
// Rust (Assessment); never compute them here.

/** "2026-11-19" -> "2026/11/19" (dates in tables). */
export function formatDate(ymd: string): string {
  return ymd.replaceAll("-", "/");
}

/** "2026-11-09" -> "11/9" (short dates in running text, as in the mocks). */
export function monthDay(ymd: string): string {
  const [, m, d] = ymd.split("-");
  return `${Number(m)}/${Number(d)}`;
}

const WEEKDAYS = ["日", "月", "火", "水", "木", "金", "土"];

/** "2026-11-18" -> "2026/11/18(水)". */
export function formatDateWithWeekday(ymd: string): string {
  const [y, m, d] = ymd.split("-").map(Number);
  const day = new Date(y, m - 1, d).getDay();
  return `${formatDate(ymd)}(${WEEKDAYS[day]})`;
}

/** UTC timestamp -> local date "2026/11/19". */
export function formatTimestampDate(iso: string): string {
  const d = new Date(iso);
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())}`;
}

/** UTC timestamp -> local "2026/11/19 20:30". */
export function formatTimestampDateTime(iso: string): string {
  const d = new Date(iso);
  return `${formatTimestampDate(iso)} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

function toYmd(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Today on this PC as "YYYY-MM-DD" (default value of date fields). */
export function todayYmd(now: Date = new Date()): string {
  return toYmd(now);
}

/** "YYYY-MM-DD" + n calendar days (the report's previous and next day). */
export function addDaysYmd(ymd: string, days: number): string {
  const [y, m, d] = ymd.split("-").map(Number);
  return toYmd(new Date(y, m - 1, d + days));
}

/**
 * The Monday of the week "YYYY-MM-DD" falls in (週報: weeks run from Monday
 * to Sunday). Rust moves a weekly report's date to its Monday as well; this
 * is for the screen's keys and the week shown next to the heading.
 */
export function weekStartYmd(ymd: string): string {
  const [y, m, d] = ymd.split("-").map(Number);
  const sinceMonday = (new Date(y, m - 1, d).getDay() + 6) % 7;
  return toYmd(new Date(y, m - 1, d - sinceMonday));
}
