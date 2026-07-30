import { Hono } from "hono";
import { streamSSE } from "hono/streaming";
import { subscribeToChanges, type AppChangeEvent } from "../changeEvents";
import type { Env } from "../types";

const events = new Hono<{ Bindings: Env }>();
// Bun.serve の既定アイドルタイムアウト（10秒）より短い間隔で接続を維持する。
const KEEPALIVE_INTERVAL_MS = 5_000;

events.get("/", (c) =>
  streamSSE(c, async (stream) => {
    const pending: AppChangeEvent[] = [];
    let wake: (() => void) | undefined;

    const unsubscribe = subscribeToChanges((event) => {
      pending.push(event);
      wake?.();
    });

    stream.onAbort(() => wake?.());

    try {
      await stream.writeSSE({
        event: "track.connected",
        data: JSON.stringify({ connectedAt: new Date().toISOString() }),
        retry: 1_000,
      });

      while (!stream.aborted) {
        if (pending.length === 0) {
          await new Promise<void>((resolve) => {
            let settled = false;
            const finish = () => {
              if (settled) return;
              settled = true;
              clearTimeout(timeout);
              wake = undefined;
              resolve();
            };
            const timeout = setTimeout(finish, KEEPALIVE_INTERVAL_MS);
            wake = finish;
          });
        }

        if (stream.aborted) break;

        const changes = pending.splice(0);
        if (changes.length === 0) {
          await stream.writeSSE({
            event: "track.keepalive",
            data: new Date().toISOString(),
          });
          continue;
        }

        for (const change of changes) {
          await stream.writeSSE({
            event: change.type,
            data: JSON.stringify(change),
          });
        }
      }
    } finally {
      unsubscribe();
    }
  }),
);

export { events };
