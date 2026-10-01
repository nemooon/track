import { Hono } from "hono";
import { stream } from "hono/streaming";
import { z } from "zod";
import { createAiProvider } from "../ai/providers";
import { loadConfig } from "../config";
import type { Env } from "../types";
import type { AiGenerateStreamEvent } from "../../shared/types";

const ai = new Hono<{ Bindings: Env }>();

const generateSchema = z.object({
  mode: z.enum(["weekly-report", "report-aggregation", "note-title"]),
  input: z.string().trim().min(1).max(200_000),
});

function provider(c: { env: Env }) {
  return createAiProvider(loadConfig(c.env.DATA_DIR), {
    homeDir: c.env.HOME_DIR,
    resourceDir: c.env.RESOURCE_DIR,
  });
}

ai.get("/status", (c) => c.json(provider(c).status()));

ai.post("/generate", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = generateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json(
      { error: "invalid_input", issues: parsed.error.flatten() },
      400,
    );
  }

  const selected = provider(c);
  try {
    const text = await selected.generate(parsed.data);
    return c.json({ text, provider: selected.id });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return c.json({ error: "generation_failed", message }, 503);
  }
});

ai.post("/generate-stream", async (c) => {
  const body = await c.req.json().catch(() => null);
  const parsed = generateSchema.safeParse(body);
  if (!parsed.success) {
    return c.json(
      { error: "invalid_input", issues: parsed.error.flatten() },
      400,
    );
  }

  const selected = provider(c);
  c.header("Content-Type", "application/x-ndjson; charset=utf-8");
  c.header("Cache-Control", "no-cache");
  return stream(c, async (output) => {
    const controller = new AbortController();
    let writes = Promise.resolve();
    const write = (event: AiGenerateStreamEvent) => {
      writes = writes.then(async () => {
        await output.writeln(JSON.stringify(event));
      });
      return writes;
    };
    output.onAbort(() => controller.abort());
    const keepalive = setInterval(() => {
      void write({ type: "heartbeat" });
    }, 5_000);

    try {
      const text = await selected.generate(parsed.data, {
        signal: controller.signal,
        onProgress: async (progress) => {
          await write({ type: "progress", ...progress });
        },
      });
      await write({ type: "result", text, provider: selected.id });
    } catch (error) {
      if (!output.aborted) {
        await write({
          type: "error",
          message: error instanceof Error ? error.message : String(error),
        });
      }
    } finally {
      clearInterval(keepalive);
      await writes;
    }
  });
});

export { ai };
