import {
  copyFileSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import path from "node:path";

const TAURI_DIR = path.resolve(import.meta.dir, "..");
const SOURCE_DIR = path.join(TAURI_DIR, "apple-intelligence");
const BINARIES_DIR = path.join(TAURI_DIR, "binaries");
const APP_DIR = path.join(BINARIES_DIR, "TrackAIHelper.app");
const CONTENTS_DIR = path.join(APP_DIR, "Contents");
const MACOS_DIR = path.join(CONTENTS_DIR, "MacOS");
const MODULE_CACHE_DIR = path.join(BINARIES_DIR, ".swift-module-cache");
const EXECUTABLE = path.join(MACOS_DIR, "TrackAIHelper");

if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("Apple IntelligenceヘルパーはApple Silicon Macでのみビルドできます");
}

const sdk = Bun.spawnSync({
  cmd: ["xcrun", "--sdk", "macosx", "--show-sdk-path"],
  stdout: "pipe",
  stderr: "inherit",
});
if (sdk.exitCode !== 0) process.exit(sdk.exitCode);
const sdkPath = sdk.stdout.toString().trim();

const foundationModels = path.join(
  sdkPath,
  "System/Library/Frameworks/FoundationModels.framework",
);
if (!Bun.file(foundationModels).exists()) {
  throw new Error("Foundation Modelsを含むmacOS 26 SDKが見つかりません");
}

rmSync(APP_DIR, { recursive: true, force: true });
mkdirSync(MACOS_DIR, { recursive: true });
mkdirSync(MODULE_CACHE_DIR, { recursive: true });
copyFileSync(path.join(SOURCE_DIR, "Info.plist"), path.join(CONTENTS_DIR, "Info.plist"));

const compile = Bun.spawnSync({
  cmd: [
    "xcrun",
    "--sdk",
    "macosx",
    "swiftc",
    "-parse-as-library",
    "-target",
    "arm64-apple-macosx26.0",
    "-sdk",
    sdkPath,
    "-module-cache-path",
    MODULE_CACHE_DIR,
    path.join(SOURCE_DIR, "TrackAIHelper.swift"),
    "-o",
    EXECUTABLE,
  ],
  env: {
    ...process.env,
    CLANG_MODULE_CACHE_PATH: MODULE_CACHE_DIR,
  },
  stdout: "inherit",
  stderr: "inherit",
});
if (compile.exitCode !== 0) process.exit(compile.exitCode);

const sign = Bun.spawnSync({
  cmd: ["codesign", "--force", "--deep", "--sign", "-", APP_DIR],
  stdout: "inherit",
  stderr: "inherit",
});
if (sign.exitCode !== 0) process.exit(sign.exitCode);

console.log(`Apple Intelligence helper: ${APP_DIR}`);
