// Bundles the Node sidecar into a single Windows .exe via `bun build --compile`.
// On non-Windows hosts (i.e. Linux dev), this script just typechecks the sidecar
// TS source; the actual exe compilation must be done on Windows.
import { spawnSync } from "node:child_process";
import { mkdirSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const root = resolve(__dirname, "..");
const binariesDir = resolve(root, "src-tauri", "binaries");
mkdirSync(binariesDir, { recursive: true });

const typecheck = spawnSync(
  "npm",
  ["run", "typecheck", "-w", "@dahara/sidecar"],
  { cwd: root, stdio: "inherit" },
);
if (typecheck.status !== 0) {
  process.exit(typecheck.status ?? 1);
}

if (process.platform === "win32") {
  const out = resolve(
    binariesDir,
    "dahara-fuel-calc-sidecar-x86_64-pc-windows-msvc.exe",
  );
  const r = spawnSync(
    "bun",
    [
      "build",
      "--compile",
      "--target=bun-windows-x64",
      "packages/sidecar/src/index.ts",
      `--outfile=${out}`,
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (r.status !== 0) process.exit(r.status ?? 1);
  try {
    const stats = statSync(out);
    console.log(`[build-sidecar] exe created: ${out} (${stats.size} bytes)`);
  } catch {
    console.error(`[build-sidecar] expected output not found: ${out}`);
    process.exit(1);
  }
  process.exit(0);
}

console.log(
  `[build-sidecar] non-Windows host (${process.platform}). Typechecked sidecar TS; ` +
    "run this on Windows to produce the .exe.",
);

