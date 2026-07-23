import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const readJson = async (relativePath) =>
  JSON.parse(await readFile(path.join(root, relativePath), "utf8"));

const packageJson = await readJson("package.json");
const packageLock = await readJson("package-lock.json");
const tauriConfig = await readJson("src-tauri/tauri.conf.json");
const cargoToml = await readFile(path.join(root, "src-tauri/Cargo.toml"), "utf8");
const cargoLock = await readFile(path.join(root, "src-tauri/Cargo.lock"), "utf8");

const cargoPackageVersion = cargoToml.match(
  /^\[package\][\s\S]*?^version = "([^"]+)"/m,
)?.[1];
const cargoLockVersion = cargoLock.match(
  /^\[\[package\]\]\nname = "track"\nversion = "([^"]+)"/m,
)?.[1];

const versions = new Map([
  ["package.json", packageJson.version],
  ["package-lock.json", packageLock.version],
  ['package-lock.json packages[""]', packageLock.packages?.[""]?.version],
  ["src-tauri/tauri.conf.json", tauriConfig.version],
  ["src-tauri/Cargo.toml", cargoPackageVersion],
  ["src-tauri/Cargo.lock", cargoLockVersion],
]);

const missing = [...versions].filter(([, version]) => !version);
if (missing.length > 0) {
  console.error(
    `バージョンを取得できませんでした: ${missing.map(([file]) => file).join(", ")}`,
  );
  process.exit(1);
}

const uniqueVersions = new Set(versions.values());
if (uniqueVersions.size !== 1) {
  console.error("アプリのバージョンが一致していません。");
  for (const [file, version] of versions) {
    console.error(`  ${file}: ${version}`);
  }
  process.exit(1);
}

process.stdout.write(`${packageJson.version}\n`);
