import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("使い方: npm run version:set -- 0.3.0");
  process.exit(1);
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const resolve = (relativePath) => path.join(root, relativePath);

const packageJson = JSON.parse(await readFile(resolve("package.json"), "utf8"));
const packageLock = JSON.parse(
  await readFile(resolve("package-lock.json"), "utf8"),
);
const tauriConfig = JSON.parse(
  await readFile(resolve("src-tauri/tauri.conf.json"), "utf8"),
);

const cargoTomlPath = resolve("src-tauri/Cargo.toml");
const cargoToml = await readFile(cargoTomlPath, "utf8");
const cargoTomlPattern = /^(\[package\][\s\S]*?^version = ")[^"]+(")/m;
if (!cargoTomlPattern.test(cargoToml)) {
  console.error("src-tauri/Cargo.toml のバージョンを取得できませんでした。");
  process.exit(1);
}
const updatedCargoToml = cargoToml.replace(
  cargoTomlPattern,
  `$1${version}$2`,
);

const cargoLockPath = resolve("src-tauri/Cargo.lock");
const cargoLock = await readFile(cargoLockPath, "utf8");
const cargoLockPattern =
  /^(\[\[package\]\]\nname = "track"\nversion = ")[^"]+(")/m;
if (!cargoLockPattern.test(cargoLock)) {
  console.error("src-tauri/Cargo.lock のバージョンを取得できませんでした。");
  process.exit(1);
}
const updatedCargoLock = cargoLock.replace(
  cargoLockPattern,
  `$1${version}$2`,
);

packageJson.version = version;
packageLock.version = version;
packageLock.packages[""].version = version;
tauriConfig.version = version;

await Promise.all([
  writeFile(
    resolve("package.json"),
    `${JSON.stringify(packageJson, null, 2)}\n`,
  ),
  writeFile(
    resolve("package-lock.json"),
    `${JSON.stringify(packageLock, null, 2)}\n`,
  ),
  writeFile(
    resolve("src-tauri/tauri.conf.json"),
    `${JSON.stringify(tauriConfig, null, 2)}\n`,
  ),
  writeFile(cargoTomlPath, updatedCargoToml),
  writeFile(cargoLockPath, updatedCargoLock),
]);

console.log(`Trackのバージョンを ${version} に更新しました。`);
