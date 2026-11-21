import type { Backup, BackupMirrorAlert } from "../../lib/bindings";
import { formatDateWithWeekday, formatTimestampDateTime, monthDay } from "../../lib/date";

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

// ---- バックアップの複製先 (v0.4.0) ----

/** The settings screen's text while there is no mirror. */
export const MIRROR_INTRO =
  "複製先を選ぶと、毎日のバックアップを外付けのドライブやクラウドの同期フォルダにも複製します。PCが壊れても、複製先から戻せます。";

/** 「複製先: /Volumes/USB/higiri」. */
export function mirrorDirText(dir: string): string {
  return `複製先: ${dir}`;
}

/** 「最後に複製できた日: 11/19」; 「まだありません」 before the first copy. */
export function mirrorLastOkText(lastOk: string | null): string {
  return `最後に複製できた日: ${lastOk ? monthDay(lastOk) : "まだありません"}`;
}

/**
 * The notice on the dashboard and the settings screen: 「バックアップを複製先に複製できない日が3日続いています
 * (最後に複製できた日 11/16)。…」. The days are those Rust counted; with nothing copied yet, the date is left out.
 */
export function mirrorAlertText(alert: BackupMirrorAlert): string {
  const last = alert.last_ok ? `(最後に複製できた日 ${monthDay(alert.last_ok)})` : "";
  return `バックアップを複製先に複製できない日が${alert.days}日続いています${last}。複製先のフォルダ(外付けのドライブなど)がつながっているか確かめてください。`;
}

/** After 「ファイルを選んで復元」. */
export function restoredFileText(fileName: string): string {
  return `「${fileName}」から復元しました。元に戻すときは「復元・取り込みの前の状態」を選んで復元してください。`;
}
