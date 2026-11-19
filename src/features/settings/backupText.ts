import type { Backup } from "../../lib/bindings";
import { formatDateWithWeekday, formatTimestampDateTime } from "../../lib/date";

/** "84 KB", "1.2 MB". */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/** 「2026/11/19(木)の自動バックアップ」 or 「復元・取り込みの前の状態(2026/11/19 20:30)」. */
export function backupLabel(b: Backup): string {
  if (b.kind === "daily" && b.date) return `${formatDateWithWeekday(b.date)}の自動バックアップ`;
  return `復元・取り込みの前の状態(${formatTimestampDateTime(b.created_at)})`;
}

/** 「2026/11/19 09:12 に作成・84 KB」. */
export function backupDetail(b: Backup): string {
  return `${formatTimestampDateTime(b.created_at)} に作成・${formatBytes(b.size_bytes)}`;
}
