import { defineConfig, type Plugin } from "vitest/config";
import react from "@vitejs/plugin-react";

const host = process.env.TAURI_DEV_HOST;

/**
 * Fontsource CSS lists a .woff fallback after every .woff2 source. The
 * WebViews this app runs in (WebView2, WKWebView) all read .woff2, so drop
 * the fallback and keep about 11 MB of unused font files out of the bundle.
 */
function woff2Only(): Plugin {
  return {
    name: "higiri:woff2-only",
    enforce: "pre",
    transform(code, id) {
      if (!/@fontsource\/[^/]+\/[^/]+\.css$/.test(id)) return null;
      return code.replace(/,\s*url\([^)]+\.woff\)\s*format\('woff'\)/g, "");
    },
  };
}

// https://vite.dev/config/
export default defineConfig({
  plugins: [woff2Only(), react()],

  // Options for `tauri dev` / `tauri build`
  // 1. keep Rust errors visible
  clearScreen: false,
  // 2. Tauri expects a fixed port; fail if it is taken
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. do not watch the Rust side
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // A desktop app loads its bundle from disk, not over a network, so one
    // ~1 MB chunk is fine.
    chunkSizeWarningLimit: 1500,
  },

  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    environment: "node",
    passWithNoTests: true,
    // Fixed time zone so date-format tests do not depend on the machine.
    env: { TZ: "Asia/Tokyo" },
  },
});
