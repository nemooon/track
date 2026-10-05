import { accessSync, constants, realpathSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";

const MAX_STDOUT_BYTES = 2 * 1024 * 1024;
const MAX_STDERR_BYTES = 1024 * 1024;
const DEFAULT_TIMEOUT_MS = 3 * 60 * 1000;

function expandHome(value: string, home: string): string {
  if (value === "~") return home;
  if (value.startsWith("~/")) return path.join(home, value.slice(2));
  return value;
}

function isExecutable(candidate: string): boolean {
  try {
    accessSync(candidate, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

export function executableSearchDirectories(home = homedir()): string[] {
  const configured = (process.env.PATH ?? "").split(path.delimiter).filter(Boolean);
  return [
    ...configured,
    path.join(home, ".local/bin"),
    path.join(home, ".bun/bin"),
    path.join(home, ".npm-global/bin"),
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
  ].filter((entry, index, all) => all.indexOf(entry) === index);
}

export function newestCodexExecutable(candidates: string[]): string | null {
  const seen = new Set<string>();
  let selected: string | null = null;
  let selectedVersion = [-1, -1, -1];
  for (const candidate of candidates) {
    if (!isExecutable(candidate)) continue;
    const realPath = realpathSync(candidate);
    if (seen.has(realPath)) continue;
    seen.add(realPath);
    selected ??= candidate;
    const result = spawnSync(candidate, ["--version"], {
      encoding: "utf8",
      timeout: 1_000,
      maxBuffer: 64 * 1024,
    });
    if (result.status !== 0) continue;
    const match = result.stdout?.match(/codex-cli\s+(\d+)\.(\d+)\.(\d+)/);
    if (!match) continue;
    const version = match.slice(1).map(Number);
    const differing = version.findIndex((part, index) => part !== selectedVersion[index]);
    if (differing >= 0 && version[differing]! > selectedVersion[differing]!) {
      selected = candidate;
      selectedVersion = version;
    }
  }
  return selected;
}

export function resolveExecutable(
  requested: string,
  home = homedir(),
): string | null {
  const value = expandHome(requested.trim(), home);
  if (!value) return null;

  if (path.isAbsolute(value)) return isExecutable(value) ? value : null;
  if (value.includes("/") || value.includes("\\")) return null;

  const directories = executableSearchDirectories(home);
  const candidates = directories.map((directory) =>
    path.join(directory, value),
  );
  if (value === "codex") {
    // npm scripts prepend project dependencies to PATH. Prefer the user's CLI
    // and desktop installation over the SDK's bundled, potentially older CLI.
    const isProjectDependency = (directory: string) =>
      directory.split(/[\\/]/).includes("node_modules");
    const external = directories.filter((directory) => !isProjectDependency(directory));
    const bundled = directories.filter(isProjectDependency);
    // Compare the PATH CLI with desktop copies. The desktop CLI may be newer
    // and receive a newer model catalog even when Homebrew is already installed.
    const installed = [
      external.map((directory) => path.join(directory, value)).find(isExecutable),
      "/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex",
      "/Applications/Codex.app/Contents/Resources/codex-cli/bin/codex",
      "/Applications/ChatGPT.app/Contents/Resources/codex",
      "/Applications/Codex.app/Contents/Resources/codex",
    ].filter((candidate): candidate is string => Boolean(candidate));
    return newestCodexExecutable(installed)
      ?? bundled.map((directory) => path.join(directory, value)).find(isExecutable)
      ?? null;
  }
  return candidates.find(isExecutable) ?? null;
}

export type ProcessResult = {
  stdout: string;
  stderr: string;
};

export function runProcess(
  executable: string,
  args: string[],
  input: string,
  timeoutMs = DEFAULT_TIMEOUT_MS,
): Promise<ProcessResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, {
      env: process.env,
      stdio: ["pipe", "pipe", "pipe"],
    });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    let settled = false;

    const finish = (error?: Error, result?: ProcessResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error);
      else resolve(result!);
    };

    const timer = setTimeout(() => {
      child.kill("SIGKILL");
      finish(new Error(`コマンドが${Math.round(timeoutMs / 1000)}秒以内に完了しませんでした。`));
    }, timeoutMs);

    child.once("error", (error) => {
      finish(new Error(`コマンドを起動できません: ${error.message}`));
    });
    child.stdout.on("data", (chunk: Buffer) => {
      stdoutBytes += chunk.length;
      if (stdoutBytes > MAX_STDOUT_BYTES) {
        child.kill("SIGKILL");
        finish(new Error("コマンドの出力が大きすぎます。"));
        return;
      }
      stdout.push(chunk);
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderrBytes += chunk.length;
      if (stderrBytes <= MAX_STDERR_BYTES) stderr.push(chunk);
    });
    child.once("close", (code, signal) => {
      const result = {
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      };
      if (code === 0 && !signal) {
        finish(undefined, result);
        return;
      }
      const detail = result.stderr.trim();
      finish(
        new Error(
          detail ||
            `コマンドが${signal ? `シグナル ${signal}` : `終了コード ${code ?? 1}`}で終了しました。`,
        ),
      );
    });

    child.stdin.on("error", (error: NodeJS.ErrnoException) => {
      if (error.code !== "EPIPE") finish(error);
    });
    child.stdin.end(input);
  });
}
