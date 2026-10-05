import { spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import { z } from "zod";
import type { CodexModelCatalog, CodexModelSuggestion } from "../../shared/types";
import { resolveExecutable } from "./process";

// Official Codex models as of 2026-10-05; only used when discovery is unavailable.
const FALLBACK_MODELS: CodexModelSuggestion[] = [
  { id: "gpt-6.1-sol", label: "GPT-6.1 Sol" },
  { id: "gpt-6-astra", label: "GPT-6 Astra" },
  { id: "gpt-6-sol", label: "GPT-6 Sol" },
  { id: "gpt-6-luna", label: "GPT-6 Luna" },
];

const modelPageSchema = z.object({
  data: z.array(z.object({
    model: z.string().min(1),
    displayName: z.string().min(1),
    hidden: z.boolean().optional(),
  })),
  nextCursor: z.string().nullable().optional(),
});

/** Read the authenticated client's picker, without creating a thread or turn. */
export function discoverCodexModels(
  executable: string,
  timeoutMs = 15_000,
): Promise<CodexModelSuggestion[]> {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, ["app-server"], {
      cwd: tmpdir(),
      stdio: ["pipe", "pipe", "pipe"],
    });
    const lines = createInterface({ input: child.stdout });
    const models = new Map<string, CodexModelSuggestion>();
    const cursors = new Set<string>();
    let requestId = 1;
    let outputBytes = 0;
    let settled = false;
    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      lines.close();
      child.stdin.end();
      child.kill("SIGKILL");
      if (error) reject(error);
      else resolve([...models.values()]);
    };
    const timer = setTimeout(() => finish(new Error("モデル一覧の取得がタイムアウトしました。")), timeoutMs);
    const send = (message: unknown) => child.stdin.write(`${JSON.stringify(message)}\n`);
    const list = (cursor?: string) => send({
      id: requestId,
      method: "model/list",
      params: { limit: 100, includeHidden: false, ...(cursor ? { cursor } : {}) },
    });
    child.on("error", (error) => finish(error));
    child.stdin.on("error", (error) => finish(error));
    child.on("close", () => finish(new Error("Codexがモデル一覧を返さずに終了しました。")));
    // Drain diagnostics; do not expose potentially sensitive stderr in the UI.
    child.stderr.resume();
    child.stdout.on("data", (chunk: Buffer) => {
      outputBytes += chunk.length;
      if (outputBytes > 2 * 1024 * 1024) finish(new Error("モデル一覧の応答が大きすぎます。"));
    });
    lines.on("line", (line) => {
      if (settled) return;
      try {
        const message = JSON.parse(line);
        if (message.id !== 0 && message.id !== requestId) return;
        if (message.error) throw new Error("Codexがモデル一覧の取得を拒否しました。");
        if (message.id === 0) {
          send({ method: "initialized", params: {} });
          list();
          return;
        }
        const page = modelPageSchema.parse(message.result);
        for (const model of page.data) {
          if (!model.hidden) models.set(model.model, { id: model.model, label: model.displayName });
        }
        if (page.nextCursor) {
          if (cursors.has(page.nextCursor) || cursors.size >= 20) {
            throw new Error("モデル一覧のページを取得できませんでした。");
          }
          cursors.add(page.nextCursor);
          requestId++;
          list(page.nextCursor);
        } else {
          if (!models.size) throw new Error("利用できるモデルがありませんでした。");
          finish();
        }
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
      }
    });
    send({
      id: 0,
      method: "initialize",
      params: { clientInfo: { name: "track", title: "Track", version: "1.0.0" } },
    });
  });
}

export async function loadCodexModels(
  requestedExecutable: string,
  homeDir: string,
  codexHome = process.env.CODEX_HOME || path.join(homeDir, ".codex"),
): Promise<CodexModelCatalog> {
  const executable = resolveExecutable(requestedExecutable || "codex", homeDir);
  if (executable) {
    try {
      const models = await discoverCodexModels(executable);
      return { models, source: "codex", detail: "Codexから取得したモデル候補です。表示中は5分ごとに自動更新します。" };
    } catch {
      // Older clients and offline sessions can still use their last local catalog.
    }
  }
  try {
    const cache = z.object({
      models: z.array(z.object({
        slug: z.string().min(1),
        display_name: z.string().min(1),
        visibility: z.string(),
      })),
    }).parse(JSON.parse(await readFile(
      path.join(codexHome, "models_cache.json"),
      "utf8",
    )));
    const models = cache.models.filter((model) => model.visibility === "list")
      .map((model) => ({ id: model.slug, label: model.display_name }));
    if (models.length) return {
      models,
      source: "cache",
      detail: "Codexへ接続できないため、前回のモデル候補を表示しています。最新の一覧とは異なる場合があります。",
    };
  } catch {
    // No usable cache: retain manual model entry and clearly label built-in choices.
  }
  return {
    models: FALLBACK_MODELS,
    source: "fallback",
    detail: "モデル一覧を取得できないため、2026年10月5日時点の標準候補を表示しています。Codexの実行ファイルとログイン状態を確認してください。",
  };
}
