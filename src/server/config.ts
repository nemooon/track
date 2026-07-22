// ~/.track/config.json — 環境変数ではなくファイルに持つ。
// バンドルした .app を Finder から起動するとシェルの環境変数を引き継がないため、
// 設定は必ずファイル側を正とする (環境変数は開発時の上書き用)。
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";

export type Config = {
  /** エクスポート JSON の書き出し先。iCloud Drive を指してもよい */
  exportDir: string;
  /** 自動バックアップの間隔 (時間)。0 で無効 */
  backupIntervalHours: number;
  /** 残す自動バックアップの本数 */
  backupKeep: number;
};

export function defaultConfig(dataDir: string): Config {
  return {
    exportDir: path.join(dataDir, "exports"),
    backupIntervalHours: 24,
    backupKeep: 30,
  };
}

function configPath(dataDir: string) {
  return path.join(dataDir, "config.json");
}

export function loadConfig(dataDir: string): Config {
  const base = defaultConfig(dataDir);
  let stored: Partial<Config> = {};
  try {
    stored = JSON.parse(readFileSync(configPath(dataDir), "utf8"));
  } catch {
    // 未作成 or 壊れている場合は既定値で続行する
  }
  const cfg: Config = {
    exportDir: typeof stored.exportDir === "string" && stored.exportDir.trim()
      ? stored.exportDir
      : base.exportDir,
    backupIntervalHours:
      typeof stored.backupIntervalHours === "number" && stored.backupIntervalHours >= 0
        ? stored.backupIntervalHours
        : base.backupIntervalHours,
    backupKeep:
      typeof stored.backupKeep === "number" && stored.backupKeep >= 1
        ? stored.backupKeep
        : base.backupKeep,
  };
  // 開発時のみ環境変数で上書きできる
  if (process.env.TRACK_EXPORT_DIR) cfg.exportDir = process.env.TRACK_EXPORT_DIR;
  return cfg;
}

export function saveConfig(dataDir: string, cfg: Config): void {
  mkdirSync(dataDir, { recursive: true });
  writeFileSync(configPath(dataDir), JSON.stringify(cfg, null, 2) + "\n", "utf8");
}
