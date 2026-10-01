import { afterEach, describe, expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { Hono } from "hono";
import { defaultConfig, saveConfig } from "../config";
import type { Env } from "../types";
import { ai } from "./ai";

const temporaryDirectories: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((directory) =>
      rm(directory, { recursive: true, force: true }),
    ),
  );
});

async function testApp() {
  const dataDir = await mkdtemp(path.join(tmpdir(), "track-ai-route-"));
  temporaryDirectories.push(dataDir);
  saveConfig(dataDir, {
    ...defaultConfig(dataDir),
    aiProvider: "custom-command",
    aiCommandExecutable: "/bin/cat",
  });

  const app = new Hono<{ Bindings: Env }>();
  app.use("*", async (c, next) => {
    c.env = {
      DATA_DIR: dataDir,
      HOME_DIR: "/tmp",
      RESOURCE_DIR: "/tmp",
    } as Env;
    await next();
  });
  app.route("/api/ai", ai);
  return app;
}

describe("AI API", () => {
  test("選択中プロバイダーの状態を返す", async () => {
    const app = await testApp();
    const response = await app.request("/api/ai/status");

    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({
      provider: "custom-command",
      available: true,
    });
  });

  test("共通APIから選択中プロバイダーで生成する", async () => {
    const app = await testApp();
    const response = await app.request("/api/ai/generate", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        mode: "note-title",
        input: "OAuth認証の確認事項",
      }),
    });

    expect(response.status).toBe(200);
    const body = (await response.json()) as {
      provider: string;
      text: string;
    };
    expect(body.provider).toBe("custom-command");
    expect(body.text).toContain("OAuth認証の確認事項");
  });

  test("生成進捗と結果をストリームで返す", async () => {
    const app = await testApp();
    const response = await app.request("/api/ai/generate-stream", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        mode: "note-title",
        input: "ストリーム出力の確認",
      }),
    });

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("application/x-ndjson");
    const events = (await response.text())
      .trim()
      .split("\n")
      .map((line) => JSON.parse(line) as { type: string; message?: string; text?: string });
    expect(events.some((event) => event.type === "progress")).toBe(true);
    expect(events.find((event) => event.type === "result")?.text).toContain(
      "ストリーム出力の確認",
    );
  });
});
