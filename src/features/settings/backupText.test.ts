import { describe, expect, it } from "vitest";
import type { Backup } from "../../lib/bindings";
import { backupDetail, backupLabel, formatBytes } from "./backupText";

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
    ).toBe("復元する前の状態(2026/11/19 20:30)");
    expect(backupDetail(daily)).toBe("2026/11/19 09:12 に作成・84 KB");
  });

  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(86016)).toBe("84 KB");
    expect(formatBytes(5 * 1024 * 1024 + 300000)).toBe("5.3 MB");
  });
});
