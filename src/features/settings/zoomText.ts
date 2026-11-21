import { DEFAULT_ZOOM, ZOOM_LEVELS } from "../../lib/bindings";

export const ZOOM_DESCRIPTION = "文字、余白、アイコン、ガントをまとめて拡大・縮小します。";

/** 表示倍率 the settings screen offers (v0.4.0): 「70%」〜「150%」, the default marked. */
export function zoomOptions(): Array<{ value: string; label: string }> {
  return ZOOM_LEVELS.map((zoom) => ({
    value: String(zoom),
    label: zoom === DEFAULT_ZOOM ? `${zoom}%(標準)` : `${zoom}%`,
  }));
}
