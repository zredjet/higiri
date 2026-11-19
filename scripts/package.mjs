// Builds the release executable and packs it into a ZIP in release/ (no installer):
//   Windows: Higiri.exe -> release/Higiri_<version>_windows_x64.zip
//   macOS:   Higiri.app -> release/Higiri_<version>_macos_<arch>.zip
// Run with `npm run build`. The GitHub Actions workflows use the same script.
import { execFileSync, execSync } from "node:child_process";
import { copyFileSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const { productName, version } = conf;
const out = join(root, "release");
const arch = process.arch === "arm64" ? "arm64" : "x64";

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

function tauriBuild(flags) {
  execSync(`npm run tauri -- build ${flags}`, { cwd: root, stdio: "inherit" });
}

let zip;
if (process.platform === "win32") {
  // The executable carries the screens and icons itself; nothing else is needed
  // (Windows 10/11 provide the WebView2 runtime).
  tauriBuild("--no-bundle");
  const staging = join(out, "staging");
  mkdirSync(staging);
  copyFileSync(join(root, "src-tauri", "target", "release", "higiri.exe"), join(staging, `${productName}.exe`));
  zip = join(out, `${productName}_${version}_windows_${arch}.zip`);
  execFileSync(
    "powershell",
    ["-NoProfile", "-Command", `Compress-Archive -Path '${join(staging, `${productName}.exe`)}' -DestinationPath '${zip}'`],
    { stdio: "inherit" },
  );
  rmSync(staging, { recursive: true, force: true });
} else if (process.platform === "darwin") {
  // .app only (ad-hoc signed by tauri-build, see bundle.macOS in tauri.conf.json).
  tauriBuild("--bundles app");
  const app = join(root, "src-tauri", "target", "release", "bundle", "macos", `${productName}.app`);
  zip = join(out, `${productName}_${version}_macos_${arch}.zip`);
  // ditto keeps the bundle's symlinks, permissions and signature intact.
  execFileSync("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, zip], { stdio: "inherit" });
} else {
  throw new Error(`Packaging is set up for Windows and macOS only (this is ${process.platform}).`);
}

console.log(`\nCreated ${zip}`);
