import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const vite = path.join(root, "node_modules", ".bin", "vite");

const processes = [
  spawn("bun", ["src/server/index.ts"], {
    cwd: root,
    stdio: "inherit",
  }),
  spawn(vite, ["--host", "127.0.0.1"], {
    cwd: root,
    stdio: "inherit",
  }),
];

let stopping = false;
let exitCode = 0;

function stop(signal = "SIGTERM") {
  if (stopping) return;
  stopping = true;
  for (const child of processes) {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill(signal);
    }
  }
}

function finishIfStopped() {
  if (
    stopping &&
    processes.every(
      (child) => child.exitCode !== null || child.signalCode !== null,
    )
  ) {
    process.exit(exitCode);
  }
}

for (const child of processes) {
  child.on("error", (error) => {
    console.error(`開発サーバーを起動できませんでした: ${error.message}`);
    exitCode = 1;
    stop();
  });
  child.on("exit", (code, signal) => {
    if (!stopping) {
      console.error(
        `開発サーバーが終了しました (${signal ?? `exit ${code ?? 1}`})`,
      );
      exitCode = code ?? 1;
      stop();
    }
    finishIfStopped();
  });
}

process.on("SIGINT", () => stop("SIGINT"));
process.on("SIGTERM", () => stop("SIGTERM"));
