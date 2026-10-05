import { afterEach, describe, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { discoverCodexModels, loadCodexModels } from "./models";

const directories: string[] = [];
afterEach(async () => {
  await Promise.all(directories.splice(0).map((dir) => rm(dir, { recursive: true, force: true })));
});

async function fixture(script: string) {
  const dir = await mkdtemp(path.join(tmpdir(), "track-models-"));
  directories.push(dir);
  const executable = path.join(dir, "codex");
  await writeFile(executable, `#!${process.execPath}\n${script}`);
  await chmod(executable, 0o755);
  return { dir, executable };
}

describe("Codex model discovery", () => {
  test("初期化後に一覧を取得し、ページを結合して非表示モデルを除外する", async () => {
    const { executable } = await fixture(`
      const { createInterface } = require("node:readline");
      let initialized = false;
      let ready = false;
      createInterface({ input: process.stdin }).on("line", line => {
        const msg = JSON.parse(line);
        const reply = result => console.log(JSON.stringify({ id: msg.id, result }));
        if (msg.method === "initialize") { initialized = true; reply({}); }
        else if (msg.method === "initialized" && initialized) { ready = true; }
        else if (msg.method === "model/list" && ready && msg.params.includeHidden === false) {
          reply(msg.params.cursor === "next" ? {
            data: [{ model: "future-model", displayName: "Future" }, { model: "hidden", displayName: "Hidden", hidden: true }],
            nextCursor: null,
          } : {
            data: [{ model: "gpt-6.1-sol", displayName: "GPT-6.1 Sol" }],
            nextCursor: "next",
          });
        } else { console.log(JSON.stringify({ id: msg.id, error: { message: "bad handshake" } })); }
      });
    `);
    expect(await discoverCodexModels(executable)).toEqual([
      { id: "gpt-6.1-sol", label: "GPT-6.1 Sol" },
      { id: "future-model", label: "Future" },
    ]);
  });

  test("応答しないプロセスをタイムアウトで終了する", async () => {
    const { executable } = await fixture("setInterval(() => {}, 1000);");
    await expect(discoverCodexModels(executable, 100)).rejects.toThrow("タイムアウト");
  });

  test("壊れた応答ならローカルキャッシュに戻る", async () => {
    const { dir, executable } = await fixture('console.log("invalid json");');
    await writeFile(path.join(dir, "models_cache.json"), JSON.stringify({ models: [
      { slug: "cached-model", display_name: "Cached", visibility: "list" },
      { slug: "hidden", display_name: "Hidden", visibility: "hide" },
    ] }));
    expect(await loadCodexModels(executable, dir, dir)).toMatchObject({
      source: "cache",
      models: [{ id: "cached-model", label: "Cached" }],
    });
  });

  test("実行ファイルもキャッシュもなければ現行の標準候補を返す", async () => {
    const { dir } = await fixture("");
    const catalog = await loadCodexModels(path.join(dir, "missing"), dir, dir);
    expect(catalog.source).toBe("fallback");
    expect(catalog.models.map((model) => model.id)).toEqual([
      "gpt-6.1-sol", "gpt-6-astra", "gpt-6-sol", "gpt-6-luna",
    ]);
  });
});
