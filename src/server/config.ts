// ~/.track/config.json — 環境変数ではなくファイルに持つ。
// バンドルした .app を Finder から起動するとシェルの環境変数を引き継がないため、
// 設定は必ずファイル側を正とする (環境変数は開発時の上書き用)。
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import type { AiProviderId } from "../shared/types";

export type Config = {
  /** エクスポート JSON の書き出し先。iCloud Drive を指してもよい */
  exportDir: string;
  /** 自動バックアップの間隔 (時間)。0 で無効 */
  backupIntervalHours: number;
  /** 残す自動バックアップの本数 */
  backupKeep: number;
  /** 文章生成に使うAIプロバイダー */
  aiProvider: AiProviderId;
  /** 空なら一般的なPATHからcodexを探す */
  aiCodexExecutable: string;
  /** 空ならCodex側の既定モデルを使う */
  aiCodexModel: string;
  /** 利用者が明示的に指定するローカルコマンド */
  aiCommandExecutable: string;
  /** シェルを介さず、そのままコマンドへ渡す引数 */
  aiCommandArgs: string[];
};

export function defaultConfig(dataDir: string): Config {
  return {
    exportDir: path.join(dataDir, "exports"),
    backupIntervalHours: 24,
    backupKeep: 30,
    aiProvider: "apple-intelligence",
    aiCodexExecutable: "",
    aiCodexModel: "",
    aiCommandExecutable: "",
    aiCommandArgs: [],
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
  const provider =
    stored.aiProvider === "codex" ||
    stored.aiProvider === "custom-command" ||
    stored.aiProvider === "apple-intelligence"
      ? stored.aiProvider
      : base.aiProvider;
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
    aiProvider: provider,
    aiCodexExecutable:
      typeof stored.aiCodexExecutable === "string"
        ? stored.aiCodexExecutable.trim()
        : base.aiCodexExecutable,
    aiCodexModel:
      typeof stored.aiCodexModel === "string"
        ? stored.aiCodexModel.trim()
        : base.aiCodexModel,
    aiCommandExecutable:
      typeof stored.aiCommandExecutable === "string"
        ? stored.aiCommandExecutable.trim()
        : base.aiCommandExecutable,
    aiCommandArgs: Array.isArray(stored.aiCommandArgs)
      ? stored.aiCommandArgs
          .filter((arg): arg is string => typeof arg === "string")
          .map((arg) => arg.trim())
          .filter(Boolean)
      : base.aiCommandArgs,
  };
  // 開発時のみ環境変数で上書きできる
  if (process.env.TRACK_EXPORT_DIR) cfg.exportDir = process.env.TRACK_EXPORT_DIR;
  return cfg;
}

export function saveConfig(dataDir: string, cfg: Config): void {
  mkdirSync(dataDir, { recursive: true });
  writeFileSync(configPath(dataDir), JSON.stringify(cfg, null, 2) + "\n", "utf8");
}
