// Regenerates src-tauri/icons/ from the SVGs in src-tauri/icons/source/ (design D3).
//   higiri-icon.svg        full-bleed: the PNG sizes, the Windows tiles, and the
//                          .ico's 48-256 px images
//   higiri-icon-small.svg  simplified for a few pixels: the .ico's 16-32 px
//                          images and 32x32.png
//   higiri-icon-macos.svg  Apple's icon grid (824 px square on 1024 px): icon.icns
//   higiri-tray-macos.svg  one-color template image for the macOS menu bar:
//                          tray-macos.png (36 px, 18 pt at 2x)
// The Windows tray uses the color 32 px icon (tray-windows.png, the same image
// as 32x32.png). Both tray icons are embedded by src-tauri/src/tray.rs.
// Run with `npm run icons`. Android / iOS icons are not kept (desktop only).
import { execSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const icons = join(root, "src-tauri", "icons");
const source = (name) => join(icons, "source", name);
const tmp = mkdtempSync(join(tmpdir(), "higiri-icons-"));

function tauriIcon(input, out, sizes) {
  const png = sizes ? ` -p ${sizes.join(",")}` : "";
  execSync(`npm run tauri -- icon "${input}" -o "${out}"${png}`, { cwd: root, stdio: "ignore" });
}

/** An .ico whose images are PNG data (as tauri icon writes them; Windows Vista and later). */
function packIco(images) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  const entries = Buffer.alloc(16 * images.length);
  let offset = header.length + entries.length;
  images.forEach(({ size, data }, i) => {
    const at = i * 16;
    entries.writeUInt8(size >= 256 ? 0 : size, at);
    entries.writeUInt8(size >= 256 ? 0 : size, at + 1);
    entries.writeUInt16LE(1, at + 4);
    entries.writeUInt16LE(32, at + 6);
    entries.writeUInt32LE(data.length, at + 8);
    entries.writeUInt32LE(offset, at + 12);
    offset += data.length;
  });
  return Buffer.concat([header, entries, ...images.map((i) => i.data)]);
}

try {
  const full = join(tmp, "full");
  const mac = join(tmp, "mac");
  const small = join(tmp, "small");
  const large = join(tmp, "large");
  const trayMac = join(tmp, "tray-mac");
  tauriIcon(source("higiri-icon.svg"), full);
  tauriIcon(source("higiri-icon-macos.svg"), mac);
  tauriIcon(source("higiri-icon-small.svg"), small, [16, 24, 32]);
  tauriIcon(source("higiri-icon.svg"), large, [48, 64, 256]);
  tauriIcon(source("higiri-tray-macos.svg"), trayMac, [36]);

  for (const name of [
    "128x128.png",
    "128x128@2x.png",
    "icon.png",
    "StoreLogo.png",
    "Square30x30Logo.png",
    "Square44x44Logo.png",
    "Square71x71Logo.png",
    "Square89x89Logo.png",
    "Square107x107Logo.png",
    "Square142x142Logo.png",
    "Square150x150Logo.png",
    "Square284x284Logo.png",
    "Square310x310Logo.png",
  ]) {
    copyFileSync(join(full, name), join(icons, name));
  }
  copyFileSync(join(small, "32x32.png"), join(icons, "32x32.png"));
  copyFileSync(join(small, "32x32.png"), join(icons, "tray-windows.png"));
  copyFileSync(join(trayMac, "36x36.png"), join(icons, "tray-macos.png"));
  copyFileSync(join(mac, "icon.icns"), join(icons, "icon.icns"));
  const ico = [
    ...[16, 24, 32].map((size) => ({ size, data: readFileSync(join(small, `${size}x${size}.png`)) })),
    ...[48, 64, 256].map((size) => ({ size, data: readFileSync(join(large, `${size}x${size}.png`)) })),
  ];
  writeFileSync(join(icons, "icon.ico"), packIco(ico));
  console.log("Updated src-tauri/icons/");
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
