import { mkdirSync } from "node:fs";
import path from "node:path";

const TAURI_DIR = path.resolve(import.meta.dir, "..");
const ROOT = path.resolve(TAURI_DIR, "..");

function hostTriple(): string {
  const result = Bun.spawnSync({ cmd: ["rustc", "-vV"], stdout: "pipe", stderr: "inherit" });
  if (result.exitCode !== 0) throw new Error("rustcのターゲット情報を取得できませんでした");

  const host = result.stdout.toString().match(/^host:\s+(\S+)$/m)?.[1];
  if (!host) throw new Error("rustc -vVのhostを解析できませんでした");
  return host;
}

const targetTriple =
  process.env.TAURI_ENV_TARGET_TRIPLE ?? process.env.CARGO_BUILD_TARGET ?? hostTriple();

const bunTarget: Record<string, Bun.Build.CompileTarget> = {
  "aarch64-apple-darwin": "bun-darwin-arm64",
  "x86_64-apple-darwin": "bun-darwin-x64",
};

const compileTarget = bunTarget[targetTriple];
if (!compileTarget) {
  throw new Error(`未対応のTauriターゲットです: ${targetTriple}`);
}

const libsqlNativePackage: Record<string, string> = {
  "aarch64-apple-darwin": "@libsql/darwin-arm64",
  "x86_64-apple-darwin": "@libsql/darwin-x64",
};
const nativePackage = libsqlNativePackage[targetTriple];

const binariesDir = path.join(TAURI_DIR, "binaries");
const outfile = path.join(binariesDir, `track-server-${targetTriple}`);
mkdirSync(binariesDir, { recursive: true });

const result = await Bun.build({
  entrypoints: [path.join(ROOT, "src/server/index.ts")],
  plugins: [
    {
      name: "embed-libsql-native-addon",
      setup(build) {
        // libsqlは実行時のtarget名を使ったdynamic requireになっている。
        // Bun compileが.nodeを内包できるよう、対象archのstatic requireへ置換する。
        build.onLoad({ filter: /node_modules\/libsql\/index\.js$/ }, async ({ path: modulePath }) => {
          const source = await Bun.file(modulePath).text();
          const dynamicRequire = "return require(`@libsql/${target}`);";
          if (!source.includes(dynamicRequire)) {
            throw new Error("libsqlのネイティブアドオン読み込み箇所を検出できませんでした");
          }
          return {
            contents: source.replace(dynamicRequire, `return require("${nativePackage}");`),
            loader: "js",
          };
        });
      },
    },
  ],
  compile: {
    target: compileTarget,
    outfile,
  },
});

if (!result.success) {
  for (const message of result.logs) console.error(message);
  process.exit(1);
}

// BunはMach-Oへbundleを注入するため、生成直後のlinker署名は無効になる。
// Tauriへ渡す前にad-hoc再署名し、Developer ID署名時にも正しい入力にする。
if (targetTriple.endsWith("-apple-darwin")) {
  const entitlements = path.join(TAURI_DIR, "Entitlements.plist");
  const sign = Bun.spawnSync({
    cmd: [
      "codesign",
      "--force",
      "--options",
      "runtime",
      "--entitlements",
      entitlements,
      "--sign",
      "-",
      outfile,
    ],
    stdout: "inherit",
    stderr: "inherit",
  });
  if (sign.exitCode !== 0) process.exit(sign.exitCode);
}

console.log(`Bun sidecar: ${outfile}`);
