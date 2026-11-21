// Builds the release executables and packs them into a ZIP in release/ (no installer):
//   Windows: Higiri.exe and higiri-cli.exe -> release/Higiri_<version>_windows_x64.zip
//   macOS:   Higiri.app (higiri-cli inside) -> release/Higiri_<version>_macos_<arch>.zip
// Run with `npm run build`. The GitHub Actions workflows use the same script.
import { execFileSync, execSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const conf = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const { productName, version } = conf;
const out = join(root, "release");
const arch = process.arch === "arm64" ? "arm64" : "x64";
// The command-line tool (v0.5.0), the second binary of the same Cargo package.
const cli = "higiri-cli";

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

function tauriBuild(flags) {
  execSync(`npm run tauri -- build ${flags}`, { cwd: root, stdio: "inherit" });
}

let zip;
if (process.platform === "win32") {
  // The executable carries the screens and icons itself; nothing else is needed
  // (Windows 10/11 provide the WebView2 runtime). cargo builds every binary of
  // the package, so higiri-cli.exe is built too; it goes next to Higiri.exe.
  tauriBuild("--no-bundle");
  const release = join(root, "src-tauri", "target", "release");
  const staging = join(out, "staging");
  mkdirSync(staging);
  const files = [
    [join(release, "higiri.exe"), join(staging, `${productName}.exe`)],
    [join(release, `${cli}.exe`), join(staging, `${cli}.exe`)],
  ];
  for (const [from, to] of files) copyFileSync(from, to);
  zip = join(out, `${productName}_${version}_windows_${arch}.zip`);
  const paths = files.map(([, to]) => `'${to}'`).join(",");
  execFileSync("powershell", ["-NoProfile", "-Command", `Compress-Archive -Path ${paths} -DestinationPath '${zip}'`], {
    stdio: "inherit",
  });
  rmSync(staging, { recursive: true, force: true });
} else if (process.platform === "darwin") {
  // .app only (ad-hoc signed by tauri-build, see bundle.macOS in tauri.conf.json).
  // The bundler puts every binary of the package in Contents/MacOS and signs
  // them with the app, so higiri-cli is inside and the same version.
  tauriBuild("--bundles app");
  const app = join(root, "src-tauri", "target", "release", "bundle", "macos", `${productName}.app`);
  if (!existsSync(join(app, "Contents", "MacOS", cli))) {
    throw new Error(`${productName}.app has no Contents/MacOS/${cli}; the ZIP would ship without the CLI.`);
  }
  zip = join(out, `${productName}_${version}_macos_${arch}.zip`);
  // ditto keeps the bundle's symlinks, permissions and signature intact.
  execFileSync("ditto", ["-c", "-k", "--sequesterRsrc", "--keepParent", app, zip], { stdio: "inherit" });
} else {
  throw new Error(`Packaging is set up for Windows and macOS only (this is ${process.platform}).`);
}

console.log(`\nCreated ${zip}`);
