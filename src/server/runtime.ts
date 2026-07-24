import {
  chmodSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";

export type RuntimeInfo = {
  pid: number;
  baseUrl: string;
  apiBase: string;
  startedAt: string;
};

const RUNTIME_FILE = "runtime.json";

export function runtimePath(dataDir: string): string {
  return path.join(dataDir, RUNTIME_FILE);
}

export function writeRuntimeInfo(
  dataDir: string,
  baseUrl: string,
  pid = process.pid,
): RuntimeInfo {
  const normalizedBase = baseUrl.replace(/\/+$/, "");
  const info: RuntimeInfo = {
    pid,
    baseUrl: normalizedBase,
    apiBase: `${normalizedBase}/api`,
    startedAt: new Date().toISOString(),
  };
  const destination = runtimePath(dataDir);
  const temporary = `${destination}.${pid}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(info, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
  });
  renameSync(temporary, destination);
  chmodSync(destination, 0o600);
  return info;
}

export function clearRuntimeInfo(dataDir: string, pid = process.pid): void {
  const destination = runtimePath(dataDir);
  try {
    const current = JSON.parse(readFileSync(destination, "utf8")) as Partial<RuntimeInfo>;
    if (current.pid !== pid) return;
    rmSync(destination);
  } catch {
    // 未作成・既に削除済み・別プロセスが書き換え中なら何もしない。
  }
}
