import { mkdirSync } from "node:fs";
import path from "node:path";

const TAURI_DIR = path.resolve(import.meta.dir, "..");
const ROOT = path.resolve(TAURI_DIR, "..");

function hostTriple(): string {
  const result = Bun.spawnSync({
    cmd: ["rustc", "-vV"],
    stdout: "pipe",
    stderr: "inherit",
  });
  if (result.exitCode !== 0) {
    throw new Error("rustcのターゲット情報を取得できませんでした");
  }
  const host = result.stdout.toString().match(/^host:\s+(\S+)$/m)?.[1];
  if (!host) throw new Error("rustc -vVのhostを解析できませんでした");
  return host;
}

const targetTriple =
  process.env.TAURI_ENV_TARGET_TRIPLE ?? process.env.CARGO_BUILD_TARGET ?? hostTriple();
const compileTarget: Record<string, Bun.Build.CompileTarget> = {
  "aarch64-apple-darwin": "bun-darwin-arm64",
  "x86_64-apple-darwin": "bun-darwin-x64",
};
const target = compileTarget[targetTriple];
if (!target) throw new Error(`未対応のTauriターゲットです: ${targetTriple}`);

const binariesDir = path.join(TAURI_DIR, "binaries");
const outfile = path.join(binariesDir, `track-cli-${targetTriple}`);
mkdirSync(binariesDir, { recursive: true });

const result = await Bun.build({
  entrypoints: [path.join(ROOT, "src/cli/index.ts")],
  compile: { target, outfile },
});
if (!result.success) {
  for (const message of result.logs) console.error(message);
  process.exit(1);
}

if (targetTriple.endsWith("-apple-darwin")) {
  const sign = Bun.spawnSync({
    cmd: ["codesign", "--force", "--options", "runtime", "--sign", "-", outfile],
    stdout: "inherit",
    stderr: "inherit",
  });
  if (sign.exitCode !== 0) process.exit(sign.exitCode);
}

console.log(`Track CLI: ${outfile}`);
