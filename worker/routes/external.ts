import { Hono } from "hono";
import type { Env } from "../types";
import { getKotEvents } from "../fixtures/kot";
import { getOutlookEvents } from "../fixtures/outlook";

const external = new Hono<{ Bindings: Env }>();

external.get("/kot/events", (c) => {
  const from = c.req.query("from");
  const to = c.req.query("to");
  if (!from || !to) return c.json({ error: "from and to required" }, 400);
  const events = getKotEvents(new Date(from), new Date(to));
  return c.json(events);
});

external.get("/outlook/events", (c) => {
  const from = c.req.query("from");
  const to = c.req.query("to");
  if (!from || !to) return c.json({ error: "from and to required" }, 400);
  const events = getOutlookEvents(new Date(from), new Date(to));
  return c.json(events);
});

export { external };
