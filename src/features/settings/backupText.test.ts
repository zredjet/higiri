import { describe, expect, it } from "vitest";
import type { Backup } from "../../lib/bindings";
import {
  backupDetail,
  backupLabel,
  formatBytes,
  mirrorAlertText,
  mirrorDirText,
  mirrorLastOkText,
  restoredFileText,
} from "./backupText";

const daily: Backup = {
  file_name: "todo-2026-11-19.db",
  kind: "daily",
  date: "2026-11-19",
  created_at: "2026-11-19T00:12:00.000Z",
  size_bytes: 86016,
};

describe("backup text", () => {
  it("names daily backups by day and kept copies by time", () => {
    expect(backupLabel(daily)).toBe("2026/11/19(木)の自動バックアップ");
    expect(
      backupLabel({
        ...daily,
        file_name: "before-restore-20261119T113000.000Z.db",
        kind: "before_restore",
        date: null,
        created_at: "2026-11-19T11:30:00.000Z",
      }),
    ).toBe("復元・取り込みの前の状態(2026/11/19 20:30)");
    expect(backupDetail(daily)).toBe("2026/11/19 09:12 に作成・84 KB");
  });

  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(86016)).toBe("84 KB");
    expect(formatBytes(5 * 1024 * 1024 + 300000)).toBe("5.3 MB");
  });

  it("says where the mirror is and when it was last in step", () => {
    expect(mirrorDirText("/Volumes/USB/higiri")).toBe("複製先: /Volumes/USB/higiri");
    expect(mirrorLastOkText("2026-11-19")).toBe("最後に複製できた日: 11/19");
    expect(mirrorLastOkText(null)).toBe("最後に複製できた日: まだありません");
  });

  it("tells how many days the mirror missed", () => {
    expect(mirrorAlertText({ last_ok: "2026-11-16", days: 3 })).toBe(
      "バックアップを複製先に複製できない日が3日続いています(最後に複製できた日 11/16)。複製先のフォルダ(外付けのドライブなど)がつながっているか確かめてください。",
    );
    expect(mirrorAlertText({ last_ok: null, days: 4 })).toBe(
      "バックアップを複製先に複製できない日が4日続いています。複製先のフォルダ(外付けのドライブなど)がつながっているか確かめてください。",
    );
  });

  it("names the restored file", () => {
    expect(restoredFileText("todo-2026-11-18.db")).toBe(
      "「todo-2026-11-18.db」から復元しました。元に戻すときは「復元・取り込みの前の状態」を選んで復元してください。",
    );
  });
});
